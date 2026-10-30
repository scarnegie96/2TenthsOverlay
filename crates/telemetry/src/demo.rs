//! A small fake race so overlays can be built and tested without iRacing.
//!
//! 22 cars in two classes lap a 5.1 km circuit. Every car follows the same
//! speed profile (braking zones, corners, straights) scaled by its own pace,
//! so the player's inputs, relative gaps, positions and fuel all behave like
//! the real thing. The session info is emitted as iRacing-style YAML and goes
//! through the same parser as live data.

use crate::model::{car_left_right as clr, track_surface, RawFrame, MAX_CARS};

const TRACK_M: f32 = 5_100.0;
const V_MAX: f32 = 78.0;
const ACCEL: f32 = 7.5;
const DECEL: f32 = 15.0;
const PROFILE_STEPS: usize = 2_000;
const PLAYER: usize = 6;
const PIT_CAR: usize = 21;
const RACE_LAPS: i32 = 30;
const FUEL_PER_LAP: f32 = 2.62;

/// (position on lap, minimum speed m/s)
const CORNERS: &[(f32, f32)] = &[
    (0.06, 24.0),
    (0.17, 41.0),
    (0.31, 19.0),
    (0.43, 52.0),
    (0.52, 33.0),
    (0.66, 22.0),
    (0.79, 47.0),
    (0.90, 28.0),
];

const NAMES: &[&str] = &[
    "Lena Hoffmann", "Marco Bellini", "Tom Okafor", "Sakura Ito", "Jörg Lindqvist", "Diego Alvarez",
    "You", "Priya Raman", "Callum Reid", "Noah Fischer", "Elif Demir", "Mateus Costa", "Owen \"Ozzy\" Walsh",
    "Hannah Kowalski", "Rafael Duarte", "Ines Moreau", "Kenji Sato", "Liam O'Brien", "Zoe Clarke: TeamZ",
    "Pieter de Vries", "Amara Nwosu", "Felix Brandt",
];

pub struct DemoSource {
    frame: RawFrame,
    t: f64,
    /// v(pct) sampled around the lap
    profile: Vec<f32>,
    /// time to reach pct at reference pace, normalised 0..1
    cum: Vec<f32>,
    ref_lap: f32,
    progress: Vec<f64>,
    pace: Vec<f32>,
    class_est: Vec<f32>,
    lap_start: Vec<f64>,
    best: Vec<f32>,
    last: Vec<f32>,
    fuel: f32,
    yaml_sent: bool,
    n: usize,
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

fn is_gt4(i: usize) -> bool {
    i >= 14
}

impl Default for DemoSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoSource {
    pub fn new() -> Self {
        let n = NAMES.len();
        let profile: Vec<f32> = (0..PROFILE_STEPS).map(|i| speed_at(i as f32 / PROFILE_STEPS as f32)).collect();
        let step_m = TRACK_M / PROFILE_STEPS as f32;
        let mut cum = Vec::with_capacity(PROFILE_STEPS + 1);
        let mut acc = 0.0f32;
        cum.push(0.0);
        for v in &profile {
            acc += step_m / v;
            cum.push(acc);
        }
        let ref_lap = acc;
        for c in cum.iter_mut() {
            *c /= ref_lap;
        }

        let mut pace = vec![1.0; n];
        let mut class_est = vec![0.0; n];
        let mut progress = vec![0.0; n];
        for i in 0..n {
            let base = if is_gt4(i) { 99.4 } else { 92.1 };
            let target = base + hash(i as u32 + 7) * 2.4 + if i == PLAYER { 0.2 } else { 0.0 };
            pace[i] = ref_lap / target;
            class_est[i] = base;
            // Spread the field: GT3 ahead, GT4 a little behind, a few gaps for interest.
            let order = if is_gt4(i) { i as f64 + 1.5 } else { i as f64 };
            progress[i] = 4.0 - order * 0.028 - hash(i as u32 + 99) as f64 * 0.01;
        }
        progress[PIT_CAR] = 3.02;

        let mut frame = RawFrame::with_capacity();
        frame.player_car_idx = PLAYER as i32;
        frame.session_num = 2;
        frame.session_state = 4;
        frame.is_on_track = true;
        frame.track_temp = 31.4;
        frame.air_temp = 22.8;

        let mut s = Self {
            frame,
            t: 1_240.0,
            profile,
            cum,
            ref_lap,
            progress,
            pace,
            class_est: class_est.clone(),
            lap_start: vec![0.0; n],
            best: (0..n).map(|i| class_est[i] + 0.4 + hash(i as u32 + 3) * 1.8).collect(),
            last: (0..n).map(|i| class_est[i] + 0.9 + hash(i as u32 + 5) * 2.0).collect(),
            fuel: 41.7,
            yaml_sent: false,
            n,
        };
        for i in 0..n {
            let pct = s.progress[i].fract() as f32;
            s.lap_start[i] = s.t - (s.cum_at(pct) * s.ref_lap / s.pace[i]) as f64;
        }
        s.frame.lap_best = s.best[PLAYER];
        s
    }

    fn idx(pct: f32) -> usize {
        ((pct.rem_euclid(1.0)) * PROFILE_STEPS as f32) as usize % PROFILE_STEPS
    }

    fn v_at(&self, pct: f32) -> f32 {
        self.profile[Self::idx(pct)]
    }

    fn cum_at(&self, pct: f32) -> f32 {
        self.cum[Self::idx(pct)]
    }

    /// The YAML is delivered once, like iRacing does on connect.
    pub fn take_session_yaml(&mut self) -> Option<String> {
        if self.yaml_sent {
            return None;
        }
        self.yaml_sent = true;
        Some(self.session_yaml())
    }

    /// Advances the simulation by `dt` seconds and returns the new frame.
    pub fn step(&mut self, dt: f32) -> &RawFrame {
        self.t += dt as f64;
        let n = self.n;
        let lap_of = |p: f64| p.floor() as i32;

        for i in 0..n {
            if i == PIT_CAR {
                continue;
            }
            let pct = self.progress[i].fract() as f32;
            let wobble = 1.0 + 0.004 * ((self.t as f32 * 0.05 + i as f32).sin());
            let dp = (self.v_at(pct) * self.pace[i] * wobble * dt / TRACK_M) as f64;
            let before = lap_of(self.progress[i]);
            self.progress[i] += dp;
            if lap_of(self.progress[i]) > before {
                let lap_time = (self.t - self.lap_start[i]) as f32;
                self.last[i] = lap_time;
                if lap_time < self.best[i] {
                    self.best[i] = lap_time;
                }
                self.lap_start[i] = self.t;
            }
        }

        // Player inputs from the speed profile.
        let pct = self.progress[PLAYER].fract() as f32;
        let speed = self.v_at(pct) * self.pace[PLAYER];
        let ahead = self.v_at(pct + 0.004) * self.pace[PLAYER];
        let (throttle, brake) = if ahead < speed - 0.4 {
            (0.0, (0.55 + (speed - ahead) / 6.0).min(1.0))
        } else if ahead > speed + 0.2 || speed > V_MAX * self.pace[PLAYER] - 1.0 {
            (1.0, 0.0)
        } else {
            (0.42, 0.0)
        };
        let (gear, rpm) = gearbox(speed);
        let dp = speed * dt / TRACK_M;
        self.fuel = (self.fuel - dp * FUEL_PER_LAP * (0.85 + 0.3 * throttle)).max(0.0);

        // Positions by race progress.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| self.progress[b].total_cmp(&self.progress[a]));
        let leader = order[0];
        let lap_current = (self.t - self.lap_start[PLAYER]) as f32;
        let delta_best = lap_current - self.cum_at(pct) * self.best[PLAYER];
        let mut class_counts = [0i32; 2];

        let f = &mut self.frame;
        for (pos, &i) in order.iter().enumerate() {
            let class = is_gt4(i) as usize;
            class_counts[class] += 1;
            let p = self.progress[i];
            let pct = p.fract() as f32;
            f.car_position[i] = pos as i32 + 1;
            f.car_class_position[i] = class_counts[class];
            f.car_lap[i] = lap_of(p) + 1;
            f.car_lap_completed[i] = lap_of(p);
            f.car_lap_dist_pct[i] = pct;
            f.car_est_time[i] = self.cum[Self::idx(pct)] * self.class_est[i];
            f.car_f2_time[i] = ((self.progress[leader] - p) as f32 * self.class_est[i]).max(0.0);
            f.car_last_lap[i] = self.last[i];
            f.car_best_lap[i] = self.best[i];
            f.car_on_pit_road[i] = false;
            f.car_track_surface[i] = track_surface::ON_TRACK;
        }
        f.car_on_pit_road[PIT_CAR] = true;
        f.car_track_surface[PIT_CAR] = track_surface::IN_PIT_STALL;
        for i in n..MAX_CARS {
            f.car_track_surface[i] = track_surface::NOT_IN_WORLD;
            f.car_lap_dist_pct[i] = -1.0;
        }

        // Side-by-side detection for the spotter.
        let mut left = 0;
        let mut right = 0;
        for i in 0..n {
            if i == PLAYER || i == PIT_CAR {
                continue;
            }
            let mut d = f.car_lap_dist_pct[i] - pct;
            d -= d.round();
            if d.abs() * TRACK_M < 5.5 {
                if i % 2 == 0 { left += 1 } else { right += 1 }
            }
        }
        f.car_left_right = match (left, right) {
            (0, 0) => clr::CLEAR,
            (1, 0) => clr::CAR_LEFT,
            (0, 1) => clr::CAR_RIGHT,
            (l, 0) if l > 1 => clr::TWO_CARS_LEFT,
            (0, r) if r > 1 => clr::TWO_CARS_RIGHT,
            _ => clr::CAR_LEFT_RIGHT,
        };

        f.tick += 1;
        f.session_time = self.t;
        f.session_time_remain = 604_800.0;
        f.session_laps_remain = (RACE_LAPS - f.car_lap_completed[leader]).max(0);
        f.speed = speed;
        f.rpm = rpm;
        f.gear = gear;
        f.throttle = throttle;
        f.brake = brake;
        f.clutch = 0.0;
        f.steering = steering_at(pct);
        f.lap = f.car_lap[PLAYER];
        f.lap_completed = f.car_lap_completed[PLAYER];
        f.lap_dist_pct = pct;
        f.lap_current = lap_current;
        f.lap_last = self.last[PLAYER];
        f.lap_best = self.best[PLAYER];
        f.delta_best = delta_best;
        f.delta_best_ok = true;
        f.fuel_level = self.fuel;
        f.fuel_level_pct = self.fuel / 120.0;
        f.incidents = 2;
        f.on_pit_road = false;
        f.is_on_track = true;
        &self.frame
    }

    fn session_yaml(&self) -> String {
        let mut y = String::from(
            r#"---
WeekendInfo:
 TrackName: demo ring
 TrackDisplayName: Demo Ring
 TrackConfigName: Grand Prix
 TrackLength: 5.10 km
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: unlimited
   SessionTime: 1800.0000 sec
   SessionType: Practice
 - SessionNum: 2
   SessionLaps: 30
   SessionTime: unlimited
   SessionType: Race
DriverInfo:
 DriverCarIdx: 6
 DriverCarFuelMaxLtr: 120.000
 DriverCarEstLapTime: 92.1000
 DriverCarSLShiftRPM: 8100.000
 DriverCarRedLine: 8600.000
 Drivers:
"#,
        );
        for (i, name) in NAMES.iter().enumerate() {
            let gt4 = is_gt4(i);
            let ir = 1200 + (hash(i as u32 + 11) * 3200.0) as i32;
            let lic = ["A", "B", "C"][(hash(i as u32 + 13) * 2.99) as usize];
            let lic_color = match lic {
                "A" => "0x0153db",
                "B" => "0x00c702",
                _ => "0xfeec04",
            };
            let abbrev = name.split_whitespace().last().unwrap_or(name);
            y.push_str(&format!(
                " - CarIdx: {i}\n   UserName: {name}\n   AbbrevName: {abbrev}\n   CarNumber: \"{num}\"\n   CarScreenNameShort: {car}\n   CarClassID: {cid}\n   CarClassShortName: {cls}\n   CarClassColor: {col}\n   CarClassEstLapTime: {est:.4}\n   IRating: {ir}\n   LicString: {lic} {sr:.2}\n   LicColor: {lic_color}\n   IsSpectator: 0\n   CarIsPaceCar: 0\n",
                num = 3 + i * 7 % 97,
                car = if gt4 { "GT4 Coupe" } else { "GT3 Evo" },
                cid = if gt4 { 4048 } else { 4029 },
                cls = if gt4 { "GT4" } else { "GT3" },
                col = if gt4 { "0x33ceff" } else { "0xffda59" },
                est = self.class_est[i],
                sr = 1.5 + hash(i as u32 + 17) * 2.9,
            ));
        }
        y
    }
}

fn speed_at(pct: f32) -> f32 {
    let mut v = V_MAX;
    for &(c, vc) in CORNERS {
        let mut d = (pct - c) * TRACK_M;
        d -= (d / TRACK_M).round() * TRACK_M;
        // Before the corner we brake hard, after it we accelerate.
        let a = if d < 0.0 { DECEL } else { ACCEL };
        let reachable = (vc * vc + 2.0 * a * d.abs()).sqrt();
        v = v.min(reachable);
    }
    v
}

fn steering_at(pct: f32) -> f32 {
    let mut s = 0.0;
    for (k, &(c, vc)) in CORNERS.iter().enumerate() {
        let mut d = (pct - c) * TRACK_M;
        d -= (d / TRACK_M).round() * TRACK_M;
        let dir = if k % 2 == 0 { 1.0 } else { -1.0 };
        s += dir * (1.4 - vc / 60.0) * (-(d / 60.0).powi(2)).exp();
    }
    s
}

fn gearbox(speed: f32) -> (i32, f32) {
    const TOP: [f32; 6] = [22.0, 33.0, 44.0, 55.0, 66.0, 90.0];
    let gear = TOP.iter().position(|&t| speed < t).unwrap_or(5) + 1;
    let low = if gear == 1 { 0.0 } else { TOP[gear - 2] * 0.78 };
    let high = TOP[gear - 1];
    let rpm = 4_200.0 + ((speed - low) / (high - low)).clamp(0.0, 1.0) * 4_300.0;
    (gear as i32, rpm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;

    #[test]
    fn demo_runs_through_engine() {
        let mut demo = DemoSource::new();
        let yaml = demo.take_session_yaml().unwrap();
        let info = crate::session::parse(&yaml).expect("demo yaml parses");
        assert_eq!(info.drivers.len(), NAMES.len());
        assert_eq!(info.driver(12).unwrap().user_name, "Owen \"Ozzy\" Walsh");
        assert_eq!(info.driver(18).unwrap().user_name, "Zoe Clarke: TeamZ");

        let mut engine = Engine::new();
        engine.set_session(info);
        for _ in 0..(60 * 200) {
            let f = demo.step(1.0 / 60.0).clone();
            engine.tick(&f);
        }
        let f = demo.step(1.0 / 60.0).clone();
        let field = engine.field(&f);
        assert_eq!(field.standings.len(), NAMES.len());
        assert!(field.relative.iter().any(|r| r.is_player));
        assert!(field.fuel.per_lap.is_some(), "fuel samples after 200s: {:?}", field.fuel);
        let per_lap = field.fuel.per_lap.unwrap();
        assert!((per_lap - FUEL_PER_LAP).abs() < 0.6, "per lap {per_lap}");
        // Ref lap time sanity: roughly a 90-100 s lap.
        assert!((80.0..110.0).contains(&demo.ref_lap), "ref lap {}", demo.ref_lap);
    }
}
