//! A comparison lap: elapsed time plus what the driver was doing (speed,
//! pedals, gear, steering, how sharply the car was turning) at every point
//! round the lap.
//!
//! Live delta against it is: our lap time so far minus the reference's time at
//! the same lap position. Positive = we're slower. The other channels drive
//! the ghost inputs and the coaching cues.

use serde::{Deserialize, Serialize};

/// Resolution of the stored profile: one point per 0.1% of the lap.
pub const REF_BINS: usize = 1000;

/// Where a reference lap came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    /// A lap recorded in an .ibt telemetry file.
    #[default]
    Ibt,
    /// iRacing's saved best lap (.blap) or optimal lap (.olap).
    Blap,
}

/// One raw sample from a source lap.
#[derive(Debug, Clone, Copy, Default)]
pub struct RefSample {
    /// Lap fraction 0..1.
    pub pct: f32,
    /// Seconds since the lap started.
    pub time: f32,
    /// m/s; derived from time if the source doesn't store it.
    pub speed: Option<f32>,
    pub throttle: f32,
    pub brake: f32,
    pub gear: i32,
    /// Steering wheel angle, radians (positive = left), when recorded.
    pub steering: Option<f32>,
    /// Car heading, radians. Used to work out how sharply it's turning.
    pub heading: Option<f32>,
    /// rad/s, the other way to get turning rate.
    pub yaw_rate: Option<f32>,
}

#[derive(Debug, Clone, Default)]
pub struct RefMeta {
    pub track_name: String,
    pub track_config: String,
    /// iRacing's internal track id, e.g. "watkinsglen 2021 fullcourse".
    pub track_key: String,
    pub car_name: String,
    /// iRacing's internal car id, e.g. "formulair04".
    pub car_key: String,
    pub driver: String,
    pub lap_number: i32,
    pub lap_time: f32,
    pub source: String,
    pub kind: RefKind,
    pub length_m: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceLap {
    pub track_name: String,
    pub track_config: String,
    #[serde(default)]
    pub track_key: String,
    pub car_name: String,
    #[serde(default)]
    pub car_key: String,
    pub driver: String,
    pub lap_number: i32,
    pub lap_time: f32,
    /// File it came from, for display.
    pub source: String,
    #[serde(default)]
    pub kind: RefKind,
    /// Lap length in metres, if known.
    #[serde(default)]
    pub length_m: f32,
    /// `times[k]` = seconds into the lap at `k / REF_BINS`. Length REF_BINS + 1.
    pub times: Vec<f32>,
    // The channels below have the same layout; empty when the source lacks them.
    #[serde(default)]
    pub speed: Vec<f32>,
    #[serde(default)]
    pub throttle: Vec<f32>,
    #[serde(default)]
    pub brake: Vec<f32>,
    #[serde(default)]
    pub gear: Vec<i8>,
    #[serde(default)]
    pub steering: Vec<f32>,
    /// How sharply the car is turning, rad per metre (1 / corner radius), signed.
    #[serde(default)]
    pub curvature: Vec<f32>,
}

fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

/// Resamples (position, value) points onto the REF_BINS grid by linear interpolation.
fn resample(points: &[(f32, f32)]) -> Vec<f32> {
    let mut out = Vec::with_capacity(REF_BINS + 1);
    let mut j = 0;
    for k in 0..=REF_BINS {
        let p = k as f32 / REF_BINS as f32;
        while j + 2 < points.len() && points[j + 1].0 < p {
            j += 1;
        }
        let (p0, v0) = points[j];
        let (p1, v1) = points[(j + 1).min(points.len() - 1)];
        out.push(if p1 > p0 { v0 + ((p - p0) / (p1 - p0)).clamp(0.0, 1.0) * (v1 - v0) } else { v0 });
    }
    out
}

/// Centred moving average, `half` bins either side.
fn smooth(v: &[f32], half: usize) -> Vec<f32> {
    (0..v.len())
        .map(|i| {
            let a = i.saturating_sub(half);
            let b = (i + half).min(v.len() - 1);
            v[a..=b].iter().sum::<f32>() / (b - a + 1) as f32
        })
        .collect()
}

impl ReferenceLap {
    /// Builds a reference from samples running from the start line (pct 0,
    /// time 0) to the finish (pct 1, time = lap time).
    pub fn build(meta: RefMeta, samples: &[RefSample]) -> Option<Self> {
        // Keep strictly increasing positions; drops resets and jitter.
        let mut s: Vec<RefSample> = Vec::with_capacity(samples.len());
        for x in samples {
            if s.last().is_none_or(|l: &RefSample| x.pct > l.pct && x.time >= l.time) {
                s.push(*x);
            }
        }
        if s.len() < 20 || s[0].pct > 0.001 || s[s.len() - 1].pct < 0.999 {
            return None;
        }
        let length = if meta.length_m > 0.0 { meta.length_m } else { 0.0 };
        let col = |f: &dyn Fn(&RefSample) -> f32| resample(&s.iter().map(|x| (x.pct, f(x))).collect::<Vec<_>>());

        let times = col(&|x| x.time);

        // Speed: as recorded, else distance over time (needs the lap length).
        let speed = if s.iter().all(|x| x.speed.is_some()) {
            col(&|x| x.speed.unwrap_or(0.0))
        } else if length > 0.0 {
            let dt = REF_BINS as f32 / length; // bins per metre
            let raw: Vec<f32> = (0..=REF_BINS)
                .map(|k| {
                    let a = k.saturating_sub(2);
                    let b = (k + 2).min(REF_BINS);
                    let t = times[b] - times[a];
                    if t > 0.0 { (b - a) as f32 / dt / t } else { 0.0 }
                })
                .collect();
            smooth(&raw, 2)
        } else {
            Vec::new()
        };

        let steering = if s.iter().all(|x| x.steering.is_some()) {
            col(&|x| x.steering.unwrap_or(0.0))
        } else {
            Vec::new()
        };

        // Gear holds its last value rather than interpolating.
        let mut gear = Vec::with_capacity(REF_BINS + 1);
        let mut j = 0;
        for k in 0..=REF_BINS {
            let p = k as f32 / REF_BINS as f32;
            while j + 1 < s.len() && s[j + 1].pct <= p {
                j += 1;
            }
            gear.push(s[j].gear.clamp(-1, 12) as i8);
        }

        // Curvature: change in heading per metre, or yaw rate over speed.
        let curvature = if length > 0.0 && s.iter().all(|x| x.heading.is_some()) {
            let pts: Vec<(f32, f32)> = s
                .windows(2)
                .map(|w| {
                    let ds = (w[1].pct - w[0].pct) * length;
                    let dh = wrap_angle(w[1].heading.unwrap_or(0.0) - w[0].heading.unwrap_or(0.0));
                    ((w[0].pct + w[1].pct) / 2.0, if ds > 0.01 { dh / ds } else { 0.0 })
                })
                .collect();
            smooth(&resample(&pts), 3)
        } else if s.iter().all(|x| x.yaw_rate.is_some()) && !speed.is_empty() {
            let yaw = col(&|x| x.yaw_rate.unwrap_or(0.0));
            let raw: Vec<f32> = yaw.iter().zip(&speed).map(|(y, v)| if *v > 3.0 { y / v } else { 0.0 }).collect();
            smooth(&raw, 3)
        } else {
            Vec::new()
        };

        Some(Self {
            track_name: meta.track_name,
            track_config: meta.track_config,
            track_key: meta.track_key,
            car_name: meta.car_name,
            car_key: meta.car_key,
            driver: meta.driver,
            lap_number: meta.lap_number,
            lap_time: meta.lap_time,
            source: meta.source,
            kind: meta.kind,
            length_m: length,
            times,
            speed,
            throttle: col(&|x| x.throttle),
            brake: col(&|x| x.brake),
            gear,
            steering,
            curvature,
        })
    }

    fn sample(channel: &[f32], pct: f32) -> Option<f32> {
        if channel.len() != REF_BINS + 1 {
            return None;
        }
        let x = pct.clamp(0.0, 1.0) * REF_BINS as f32;
        let i = (x.floor() as usize).min(REF_BINS - 1);
        let f = x - i as f32;
        Some(channel[i] + (channel[i + 1] - channel[i]) * f)
    }

    /// Seconds into the reference lap at lap fraction `pct`.
    pub fn time_at(&self, pct: f32) -> f32 {
        Self::sample(&self.times, pct).unwrap_or(0.0)
    }
    pub fn speed_at(&self, pct: f32) -> Option<f32> {
        Self::sample(&self.speed, pct)
    }
    pub fn throttle_at(&self, pct: f32) -> Option<f32> {
        Self::sample(&self.throttle, pct)
    }
    pub fn brake_at(&self, pct: f32) -> Option<f32> {
        Self::sample(&self.brake, pct)
    }
    pub fn steering_at(&self, pct: f32) -> Option<f32> {
        Self::sample(&self.steering, pct)
    }
    pub fn curvature_at(&self, pct: f32) -> Option<f32> {
        Self::sample(&self.curvature, pct)
    }
    pub fn gear_at(&self, pct: f32) -> Option<i32> {
        if self.gear.len() != REF_BINS + 1 {
            return None;
        }
        let i = ((pct.clamp(0.0, 1.0) * REF_BINS as f32).round() as usize).min(REF_BINS);
        Some(self.gear[i] as i32)
    }

    /// Our time so far minus the reference's at the same point. Positive = slower.
    pub fn delta(&self, pct: f32, lap_current: f32) -> f32 {
        lap_current - self.time_at(pct)
    }

    /// Same track and layout? Uses iRacing's internal track id when both sides
    /// have it (e.g. .blap files), otherwise the display names.
    pub fn matches(&self, track_name: &str, track_config: &str, track_key: &str) -> bool {
        if !self.track_key.is_empty() && !track_key.is_empty() {
            return normalize_key(&self.track_key) == normalize_key(track_key);
        }
        self.track_name.eq_ignore_ascii_case(track_name) && self.track_config.eq_ignore_ascii_case(track_config)
    }

    pub fn has_inputs(&self) -> bool {
        !self.throttle.is_empty() && !self.brake.is_empty()
    }
}

/// "watkinsglen\2021\fullcourse" and "watkinsglen 2021 fullcourse" are the same track.
pub fn normalize_key(s: &str) -> String {
    s.to_ascii_lowercase()
        .split(|c: char| c == '\\' || c == '/' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constant_lap() -> ReferenceLap {
        // A 100 s, 5 km lap at constant pace (50 m/s), heading turning steadily.
        let samples: Vec<RefSample> = (0..=500)
            .map(|i| {
                let pct = i as f32 / 500.0;
                RefSample {
                    pct,
                    time: pct * 100.0,
                    throttle: 1.0,
                    gear: 5,
                    heading: Some(pct * std::f32::consts::TAU),
                    ..Default::default()
                }
            })
            .collect();
        let meta = RefMeta {
            track_name: "Ring".into(),
            track_config: "GP".into(),
            track_key: "ring\\gp".into(),
            length_m: 5000.0,
            lap_time: 100.0,
            ..Default::default()
        };
        ReferenceLap::build(meta, &samples).unwrap()
    }

    #[test]
    fn interpolates_profile() {
        let r = constant_lap();
        assert!((r.time_at(0.25) - 25.0).abs() < 1e-3);
        assert!((r.time_at(1.0) - 100.0).abs() < 1e-3);
        // At 50% after 49.5 s we're 0.5 s up.
        assert!((r.delta(0.5, 49.5) + 0.5).abs() < 1e-3);
        assert!((r.speed_at(0.5).unwrap() - 50.0).abs() < 0.1);
        assert_eq!(r.gear_at(0.3), Some(5));
        // One full turn over 5 km.
        let k = std::f32::consts::TAU / 5000.0;
        assert!((r.curvature_at(0.5).unwrap() - k).abs() < k * 0.05);
        assert!(r.steering_at(0.5).is_none());
    }

    #[test]
    fn matches_by_key_or_name() {
        let r = constant_lap();
        assert!(r.matches("whatever", "", "ring gp"));
        assert!(!r.matches("Ring", "GP", "other track"));
        let mut r2 = r.clone();
        r2.track_key.clear();
        assert!(r2.matches("ring", "gp", "anything"));
    }
}
