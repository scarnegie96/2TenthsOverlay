//! A small fake race so overlays can be built and tested without iRacing.
//!
//! 22 cars in two classes lap a 5.1 km circuit. Every car follows the same
//! speed profile (braking zones, corners, straights) scaled by its own pace,
//! so the player's inputs, relative gaps, positions and fuel all behave like
//! the real thing. The session info is emitted as iRacing-style YAML and goes
//! through the same parser as live data.

use crate::model::{car_left_right as clr, track_surface, RawFrame, MAX_CARS};
use crate::pit::{self, PitCommand, PitService};

const TRACK_M: f32 = 5_100.0;
const V_MAX: f32 = 78.0;
const ACCEL: f32 = 7.5;
const DECEL: f32 = 15.0;
const PROFILE_STEPS: usize = 2_000;
const PLAYER: usize = 6;
const PIT_CAR: usize = 21;
/// A driver who has retired: parked in the garage, out of the race.
const RETIRED: usize = 13;
const RACE_LAPS: i32 = 30;
/// Cars that dice with the player: (car idx, mean offset m, swing m, period s).
const BATTLE: &[(usize, f32, f32, f32)] = &[(7, -4.0, 9.0, 22.0), (8, 14.0, 11.0, 31.0)];
const FUEL_PER_LAP: f32 = 2.62;
/// Cold pressures set in the garage, kPa, LF, RF, LR, RR.
const TIRE_PRESSURES: [f32; 4] = [165.0, 165.0, 162.0, 162.0];

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
    /// What's ticked for the next stop; the overlay's pit commands change it.
    pit: PitService,
    prev_speed: f32,
    long_accel: f32,
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

struct DemoClass {
    id: i32,
    name: &'static str,
    car: &'static str,
    color: &'static str,
    est_lap: f32,
}

const CLASSES: [DemoClass; 3] = [
    DemoClass { id: 4074, name: "LMP2", car: "Prototype", color: "0xf04e7a", est_lap: 86.3 },
    DemoClass { id: 4029, name: "GT3", car: "GT3 Evo", color: "0xffda59", est_lap: 92.1 },
    DemoClass { id: 4048, name: "GT4", car: "GT4 Coupe", color: "0x33ceff", est_lap: 99.4 },
];

/// Cars 0-4 are LMP2, 5-14 GT3 (the player is 6), 15-21 GT4.
fn class_of(i: usize) -> usize {
    match i {
        0..=4 => 0,
        5..=14 => 1,
        _ => 2,
    }
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
            let base = CLASSES[class_of(i)].est_lap;
            let target = base + hash(i as u32 + 7) * 2.4 + if i == PLAYER { 0.2 } else { 0.0 };
            pace[i] = ref_lap / target;
            class_est[i] = base;
            // Spread the field: GT3 ahead, GT4 a little behind, a few gaps for interest.
            let order = if class_of(i) == 2 { i as f64 + 1.5 } else { i as f64 };
            progress[i] = 4.0 - order * 0.028 - hash(i as u32 + 99) as f64 * 0.01;
        }
        progress[PIT_CAR] = 3.02;
        // Retired on lap 2.
        progress[RETIRED] = 1.55;
        for &(i, base, _, _) in BATTLE {
            progress[i] = progress[PLAYER] + (base / TRACK_M) as f64;
        }

        let mut frame = RawFrame::with_capacity();
        frame.player_car_idx = PLAYER as i32;
        frame.session_num = 2;
        frame.session_state = 4;
        frame.is_on_track = true;
        frame.track_temp = 31.4;
        frame.air_temp = 22.8;
        frame.skies = Some(1);
        frame.track_wetness = Some(1);
        frame.wind_speed = Some(3.4);
        frame.wind_dir = Some(5.5);
        frame.humidity = Some(0.56);

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
            // Four tires and some fuel ticked, as iRacing often starts.
            pit: PitService {
                flags: pit::flags::TIRES.iter().fold(pit::flags::FUEL, |a, b| a | b),
                fuel: 30.0,
                pressure: TIRE_PRESSURES,
                compound: 0,
            },
            prev_speed: 0.0,
            long_accel: 0.0,
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

    /// Ticks or unticks pit service, as iRacing would.
    pub fn pit_command(&mut self, cmd: PitCommand) {
        self.pit.apply(cmd);
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
            if i == PIT_CAR || i == RETIRED {
                continue;
            }
            let pct = self.progress[i].fract() as f32;
            let before = lap_of(self.progress[i]);
            if let Some(&(_, base, amp, period)) = BATTLE.iter().find(|b| b.0 == i) {
                // Scripted battle: hover around the player so the radar has something to show.
                let player = self.progress[PLAYER] + (self.v_at(self.progress[PLAYER].fract() as f32) * self.pace[PLAYER] * dt / TRACK_M) as f64;
                let offset_m = base + amp * (self.t as f32 * std::f32::consts::TAU / period).sin();
                self.progress[i] = player + (offset_m / TRACK_M) as f64;
            } else {
                let mut wobble = 1.0 + 0.004 * ((self.t as f32 * 0.05 + i as f32).sin());
                if i == PLAYER {
                    // Gain in some corners and lose in others, differently each lap,
                    // so the delta graph has something to show.
                    let lap = self.progress[i].floor() as f32;
                    wobble *= 1.0 + 0.012 * (pct * std::f32::consts::TAU * 3.0 + lap * 1.7).sin()
                        + 0.006 * (pct * std::f32::consts::TAU * 7.0 + lap * 0.6).sin();
                }
                self.progress[i] += (self.v_at(pct) * self.pace[i] * wobble * dt / TRACK_M) as f64;
            }
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
        // Each comparison lap is our reference pace profile scaled to its lap time.
        let at = |lap_time: f32| lap_current - self.cum_at(pct) * lap_time;
        let deltas = crate::model::Deltas {
            best: Some(at(self.best[PLAYER] - 0.35)),
            optimal: Some(at(self.best[PLAYER] - 0.6)),
            session_best: Some(at(self.best[PLAYER])),
            session_optimal: Some(at(self.best[PLAYER] - 0.25)),
            last: Some(at(self.last[PLAYER])),
            reference: None,
        };
        let mut class_counts = [0i32; 3];

        let f = &mut self.frame;
        for (pos, &i) in order.iter().enumerate() {
            let class = class_of(i);
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
        f.car_track_surface[RETIRED] = track_surface::NOT_IN_WORLD;
        f.car_lap_dist_pct[RETIRED] = -1.0;
        for i in n..MAX_CARS {
            f.car_track_surface[i] = track_surface::NOT_IN_WORLD;
            f.car_lap_dist_pct[i] = -1.0;
        }

        // Side-by-side detection for the radar.
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
        // Race started at 14:00 in the sim.
        f.time_of_day = Some(14.0 * 3600.0 + self.t as f32);
        f.session_time_remain = 604_800.0;
        f.session_laps_remain = (RACE_LAPS - f.car_lap_completed[leader]).max(0);
        f.speed = speed;
        f.rpm = rpm;
        f.gear = gear;
        f.throttle = throttle;
        f.brake = brake;
        f.clutch = 0.0;
        f.steering = steering_at(pct);
        // Roughly: the car turns at a rate set by steering and speed.
        // Turning rate from steering, with a balance that shifts with speed:
        // pushes wide in fast corners, rotates a bit more in slow ones.
        let balance = if speed > 45.0 { 0.82 } else if speed < 28.0 { 1.12 } else { 1.0 };
        f.yaw_rate = f.steering * speed * 0.016 * balance;
        // Rough body physics, enough to make the Dynamics widget move believably.
        let raw_long = if self.prev_speed > 0.0 { (speed - self.prev_speed) / dt } else { 0.0 };
        self.prev_speed = speed;
        self.long_accel += (raw_long.clamp(-30.0, 15.0) - self.long_accel) * 0.25;
        f.long_accel = self.long_accel;
        f.lat_accel = speed * f.yaw_rate;
        let kerb = if (pct * 37.0).fract() < 0.02 { 3.5 * (self.t as f32 * 40.0).sin() } else { 0.0 };
        f.vert_accel = 9.80665 + 0.0006 * speed * speed + kerb;
        f.pitch = 0.0012 * f.long_accel;
        f.roll = 0.0016 * f.lat_accel;
        // The rear steps out a little in the slowest corners.
        let slide = if speed < 30.0 { 0.035 } else { 0.012 };
        f.velocity_x = speed;
        f.velocity_y = -slide * f.yaw_rate * speed;

        // Engine gauges that respond to revs, and a water temperature warning
        // that comes on for a few seconds every so often so the light can be seen.
        use crate::instruments::engine_warning as w;
        let load = (rpm - 4_200.0) / 4_300.0;
        let hot = (self.t % 45.0) > 39.0;
        f.oil_temp = Some(98.0 + 8.0 * load + (self.t as f32 * 0.01).sin() * 2.0);
        f.oil_press = Some(2.2 + 3.0 * load);
        f.water_temp = Some(if hot { 112.0 } else { 88.0 + 4.0 * load });
        f.fuel_press = Some(3.8);
        f.voltage = Some(13.8);
        f.brake_bias = Some(if (self.t % 60.0) > 30.0 { 54.0 } else { 54.5 });
        let mut warn = 0;
        if hot {
            warn |= w::WATER_TEMP;
        }
        if rpm > 8_400.0 {
            warn |= w::REV_LIMITER;
        }
        f.engine_warnings = warn;
        // A local yellow and a blue flag now and then, for the Flags widget.
        use crate::model::session_flags as fl;
        f.session_flags = match self.t % 70.0 {
            t if (20.0..27.0).contains(&t) => fl::GREEN | fl::YELLOW | fl::YELLOW_WAVING,
            t if (50.0..55.0).contains(&t) => fl::GREEN | fl::BLUE,
            _ => fl::GREEN,
        };
        // Heading: one clockwise lap of the compass per lap, swinging through
        // the corners. YawNorth turns anticlockwise, so it's the negative.
        let bearing = pct * std::f32::consts::TAU + 0.25 * (pct * std::f32::consts::TAU * 8.0).sin();
        f.yaw_north = Some(-bearing);
        // Weather that changes slowly: a light shower comes and goes every 10
        // minutes, the track gets a little damp behind it, and the wind veers.
        let cycle = ((self.t / 600.0).fract() * std::f64::consts::TAU).sin().max(0.0) as f32;
        f.precipitation = Some(0.35 * cycle * cycle);
        f.skies = Some(if cycle > 0.5 { 3 } else if cycle > 0.1 { 2 } else { 1 });
        f.track_wetness = Some(if cycle > 0.8 { 3 } else if cycle > 0.4 { 2 } else { 1 });
        f.wind_dir = Some((5.5 + 0.4 * (self.t as f32 / 240.0).sin()).rem_euclid(std::f32::consts::TAU));
        f.wind_speed = Some(3.4 + 1.2 * (self.t as f32 / 37.0).sin().abs());
        f.humidity = Some(0.56 + 0.3 * cycle);
        f.fog_level = Some(0.0);
        f.lap = f.car_lap[PLAYER];
        f.lap_completed = f.car_lap_completed[PLAYER];
        f.lap_dist_pct = pct;
        f.lap_current = lap_current;
        f.lap_last = self.last[PLAYER];
        f.lap_best = self.best[PLAYER];
        f.deltas = deltas;
        f.fuel_level = self.fuel;
        f.fuel_level_pct = self.fuel / 120.0;
        f.incidents = 2;
        f.on_pit_road = false;
        f.is_on_track = true;
        // Readings from the last stop: a little hotter and more worn on the right.
        f.tire_temp = [[78.0, 82.5, 86.0], [91.0, 88.5, 84.0], [80.5, 83.0, 85.5], [89.0, 87.0, 83.5]];
        f.tire_wear = [[0.94, 0.93, 0.91], [0.86, 0.88, 0.9], [0.95, 0.94, 0.94], [0.89, 0.9, 0.92]];
        f.tire_cold_pressure = TIRE_PRESSURES;
        f.tire_compound = 0;
        f.tire_sets_used = 2;
        f.tire_sets_available = 3;
        f.in_pit_stall = false;
        f.pit_service = Some(self.pit);
        f.pit_sv_status = 0;
        f.fast_repairs_available = Some(1);
        &self.frame
    }

    pub fn session_yaml(&self) -> String {
        let mut y = String::from(
            r#"---
WeekendInfo:
 TrackName: demo ring
 TrackDisplayName: Demo Ring
 TrackConfigName: Grand Prix
 TrackLength: 5.10 km
 TeamRacing: 0
 WeekendOptions:
  IncidentLimit: 17
  FastRepairsLimit: 1
SplitTimeInfo:
 Sectors:
 - SectorNum: 0
   SectorStartPct: 0.000000
 - SectorNum: 1
   SectorStartPct: 0.120000
 - SectorNum: 2
   SectorStartPct: 0.250000
 - SectorNum: 3
   SectorStartPct: 0.400000
 - SectorNum: 4
   SectorStartPct: 0.580000
 - SectorNum: 5
   SectorStartPct: 0.720000
 - SectorNum: 6
   SectorStartPct: 0.860000
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
 DriverCarSLLastRPM: 8300.000
 DriverCarSLBlinkRPM: 8400.000
 DriverCarRedLine: 8600.000
 DriverTires:
 - TireIndex: 0
   TireCompoundType: "Hard"
 - TireIndex: 1
   TireCompoundType: "Wet"
 Drivers:
"#,
        );
        for (i, name) in NAMES.iter().enumerate() {
            let class = &CLASSES[class_of(i)];
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
                car = class.car,
                cid = class.id,
                cls = class.name,
                col = class.color,
                est = self.class_est[i],
                sr = 1.5 + hash(i as u32 + 17) * 2.9,
            ));
        }
        // A qualifying order that's close to, but not the same as, race pace,
        // so the Standings has positions gained and lost to show.
        let mut quali: Vec<usize> = (0..NAMES.len()).collect();
        quali.sort_by(|&a, &b| {
            let key = |i: usize| class_of(i) as f32 * 100.0 + i as f32 + (hash(i as u32 + 23) - 0.5) * 9.0;
            key(a).total_cmp(&key(b))
        });
        y.push_str("QualifyResultsInfo:\n Results:\n");
        for (pos, i) in quali.iter().enumerate() {
            y.push_str(&format!(" - Position: {pos}\n   ClassPosition: 0\n   CarIdx: {i}\n"));
        }
        y
    }
}

/// Drives the demo for `laps` laps and records the player's car as an .ibt.
pub fn record_ibt(demo: &mut DemoSource, laps: f32) -> Vec<u8> {
    let yaml = demo.session_yaml();
    let mut rows = Vec::new();
    let dt = 1.0 / 60.0;
    let steps = (laps * 92.0 / dt) as usize;
    for _ in 0..steps {
        let f = demo.step(dt);
        rows.push(vec![
            f.session_time,
            f.lap as f64,
            f.lap_dist_pct as f64,
            1.0,
            0.0,
            3.0,
            f.speed as f64,
            f.throttle as f64,
            f.brake as f64,
            f.gear as f64,
            f.steering as f64,
            f.yaw_rate as f64,
        ]);
    }
    crate::ibt::write(&yaml, crate::ibt::LAP_COLUMNS, &rows)
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
    fn imported_lap_gives_live_delta() {
        // Record a few demo laps to an .ibt, import the first complete one,
        // then drive on and check the reference delta behaves.
        let mut demo = DemoSource::new();
        let file = crate::ibt::IbtFile::from_bytes(record_ibt(&mut demo, 3.2)).unwrap();
        let laps = file.summary().laps;
        assert!(laps.len() >= 2, "complete laps: {}", laps.len());
        let reference = file.reference(laps[0].lap, "demo.ibt").unwrap();
        assert!((80.0..110.0).contains(&reference.lap_time));

        let mut engine = Engine::new();
        engine.set_session(crate::session::parse(&demo.session_yaml()).unwrap());
        engine.set_reference(Some(reference));
        assert!(engine.reference_payload().matches_track);
        let mut seen = 0;
        for _ in 0..60 * 120 {
            let f = demo.step(1.0 / 60.0).clone();
            let t = engine.tick(&f);
            if let Some(d) = t.deltas.reference {
                seen += 1;
                assert!(d.abs() < 5.0, "delta {d} at {}", f.lap_dist_pct);
            }
        }
        assert!(seen > 60 * 100);
    }

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

    #[test]
    fn demo_takes_pit_commands() {
        let mut demo = DemoSource::new();
        let info = crate::session::parse(&demo.take_session_yaml().unwrap()).unwrap();
        let mut engine = Engine::new();
        engine.set_session(info);
        demo.pit_command(PitCommand::Clear);
        demo.pit_command(PitCommand::Fuel { litres: 18 });
        demo.pit_command(PitCommand::Tire { corner: 0, kpa: 170 });
        demo.pit_command(PitCommand::FastRepair);
        let f = demo.step(1.0 / 60.0).clone();
        let pit = engine.field(&f).pit;
        assert!(pit.available);
        assert_eq!(pit.tires, [true, false, false, false]);
        assert_eq!(pit.pressures[0], 170.0);
        assert!(pit.fuel && pit.fast_repair && !pit.tearoff);
        assert_eq!(pit.fuel_amount, 18.0);
        assert_eq!(pit.fast_repairs_left, Some(1));
    }
}
