//! Car dynamics for the Dynamics widget: g-forces, body attitude, slip angle,
//! and a live understeer/oversteer estimate.
//!
//! Balance works by learning, for your car, how fast it normally rotates for
//! a given amount of steering at a given speed (gentle cornering, well below
//! the grip limit). When cornering hard we compare the actual rotation with
//! that expectation: less rotation than expected is understeer, more is
//! oversteer. A big slip angle with opposite lock is always oversteer.

use serde::Serialize;

use crate::model::RawFrame;

const G: f32 = 9.80665;

#[derive(Debug, Clone, Copy, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DynamicsPayload {
    /// Sideways g, positive = accelerating to the left (turning left).
    pub lat_g: f32,
    /// Fore-aft g, positive = accelerating forwards, negative = braking.
    pub long_g: f32,
    /// Vertical g (1.0 sitting still); spikes over kerbs and compressions.
    pub vert_g: f32,
    /// Degrees, positive = nose up.
    pub pitch_deg: f32,
    /// Degrees, positive = rolling to the right (left side up).
    pub roll_deg: f32,
    /// Degrees between where the car points and where it's going, positive = sliding left.
    pub slip_deg: f32,
    /// Degrees per second, positive = rotating left.
    pub yaw_rate_deg: f32,
    /// -1 heavy understeer .. 0 neutral .. +1 heavy oversteer. None until learnt
    /// or when not cornering hard enough to judge.
    pub balance: Option<f32>,
}

#[derive(Default)]
pub struct DynamicsTracker {
    /// Least-squares fit of yaw rate against speed x steering, in gentle corners.
    sxx: f64,
    sxy: f64,
    samples: u32,
    balance: Option<f32>,
}

impl DynamicsTracker {
    /// How rotation relates to steering for this car, once there's enough data.
    fn gain(&self) -> Option<f32> {
        (self.samples >= 180 && self.sxx > 1e-6).then(|| (self.sxy / self.sxx) as f32)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, f: &RawFrame) -> DynamicsPayload {
        let lat_g = f.lat_accel / G;
        let long_g = f.long_accel / G;
        let vx = f.velocity_x.max(0.1);
        let slip = f.velocity_y.atan2(vx);
        let x = f.speed * f.steering;

        // Learn from gentle, steady cornering where the car does what it's told.
        let gentle = f.speed > 15.0 && (0.15..0.6).contains(&lat_g.abs()) && f.steering.abs() > 0.03 && slip.abs() < 0.03;
        if gentle && f.is_on_track && x.signum() == f.yaw_rate.signum() {
            // Forget slowly so a car or setup change catches up.
            self.sxx = self.sxx * 0.9995 + (x * x) as f64;
            self.sxy = self.sxy * 0.9995 + (x * f.yaw_rate) as f64;
            self.samples = self.samples.saturating_add(1);
        }

        let hard = f.speed > 12.0 && lat_g.abs() > 0.5;
        let countersteer = f.steering.abs() > 0.05 && f.steering.signum() != f.yaw_rate.signum() && f.yaw_rate.abs() > 0.1;
        let raw = if hard && slip.abs() > 0.06 && countersteer {
            // Sliding with opposite lock: oversteer whatever the model says.
            Some(1.0)
        } else if let (true, Some(k)) = (hard && f.steering.abs() > 0.05, self.gain()) {
            let expected = k * x;
            (expected.abs() > 0.02).then(|| ((f.yaw_rate / expected - 1.0) / 0.4).clamp(-1.0, 1.0))
        } else {
            None
        };
        self.balance = match (raw, self.balance) {
            (Some(r), Some(b)) => Some(b + (r - b) * 0.12),
            (Some(r), None) => Some(r),
            (None, Some(b)) if hard => Some(b),
            _ => None,
        };

        DynamicsPayload {
            lat_g,
            long_g,
            vert_g: f.vert_accel / G,
            pitch_deg: f.pitch.to_degrees(),
            roll_deg: f.roll.to_degrees(),
            slip_deg: slip.to_degrees(),
            yaw_rate_deg: f.yaw_rate.to_degrees(),
            balance: self.balance,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corner(steer: f32, yaw_rate: f32, lat_g: f32) -> RawFrame {
        let mut f = RawFrame::with_capacity();
        f.is_on_track = true;
        f.speed = 40.0;
        f.velocity_x = 40.0;
        f.steering = steer;
        f.yaw_rate = yaw_rate;
        f.lat_accel = lat_g * G;
        f
    }

    #[test]
    fn converts_units() {
        let mut t = DynamicsTracker::default();
        let mut f = corner(0.0, 0.0, 1.5);
        f.long_accel = -2.0 * G;
        f.vert_accel = G;
        f.pitch = -0.02;
        f.velocity_y = 40.0 * 0.05f32.tan();
        let d = t.update(&f);
        assert!((d.lat_g - 1.5).abs() < 1e-4);
        assert!((d.long_g + 2.0).abs() < 1e-4);
        assert!((d.vert_g - 1.0).abs() < 1e-4);
        assert!((d.pitch_deg + 1.146).abs() < 0.01);
        assert!((d.slip_deg - 2.865).abs() < 0.01);
    }

    #[test]
    fn learns_then_spots_understeer_and_oversteer() {
        let mut t = DynamicsTracker::default();
        // Gentle corners: yaw rate = 0.01 x speed x steer.
        for i in 0..300 {
            let steer = 0.1 + (i % 10) as f32 * 0.01;
            t.update(&corner(steer, 0.01 * 40.0 * steer, 0.4));
        }
        assert!(t.gain().is_some());
        // Hard corner, rotating as expected: neutral.
        let d = t.update(&corner(0.3, 0.12, 1.4));
        assert!(d.balance.unwrap().abs() < 0.05, "{:?}", d.balance);
        // Same lock, much less rotation: understeer.
        let mut t2 = DynamicsTracker { balance: None, ..t };
        let mut b = 0.0;
        for _ in 0..60 {
            b = t2.update(&corner(0.3, 0.07, 1.4)).balance.unwrap();
        }
        assert!(b < -0.5, "understeer balance {b}");
        // Sliding with opposite lock: oversteer.
        let mut f = corner(-0.2, 0.3, 1.2);
        f.velocity_y = 40.0 * 0.12f32.tan();
        let mut b = 0.0;
        for _ in 0..60 {
            b = t2.update(&f).balance.unwrap();
        }
        assert!(b > 0.5, "oversteer balance {b}");
    }
}
