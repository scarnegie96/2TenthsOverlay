//! Replays an iRacing `.ibt` telemetry file as if it were the live sim.
//!
//! An .ibt stores the same variables as the shared memory, one sample per row,
//! so each row is mapped into a [`RawFrame`] (the same mapping `irsdk.rs` does
//! for live data) and handed to the engine at the file's own rate, scaled by a
//! speed. Everything downstream (panels, laps, stints, the coach) can't tell
//! the difference. Works on every platform; it needs no iRacing.
//!
//! Limits, from what the file holds:
//! * Other cars appear only if the file recorded the `CarIdx*` arrays with real
//!   values for them; otherwise those panels stay mostly empty.
//! * The session info is the file's copy, a snapshot from when the recording began. In
//!   practice and qualifying the other cars' laps and lap times come from its results table,
//!   so they stay as they were then; your own row is live.
//! * Pit commands have nothing to talk to and are dropped.

use std::collections::HashMap;
use std::path::Path;

use crate::ibt::IbtFile;
use crate::model::{Deltas, RawFrame, MAX_CARS};
use crate::pit::PitService;
use crate::vars::{self, VarHeader};

const PIT_PRESSURE_VARS: [&str; 4] = ["PitSvLFP", "PitSvRFP", "PitSvLRP", "PitSvRRP"];
const TIRE_TEMP_VARS: [[&str; 3]; 4] = [
    ["LFtempCL", "LFtempCM", "LFtempCR"],
    ["RFtempCL", "RFtempCM", "RFtempCR"],
    ["LRtempCL", "LRtempCM", "LRtempCR"],
    ["RRtempCL", "RRtempCM", "RRtempCR"],
];
const TIRE_WEAR_VARS: [[&str; 3]; 4] = [
    ["LFwearL", "LFwearM", "LFwearR"],
    ["RFwearL", "RFwearM", "RFwearR"],
    ["LRwearL", "LRwearM", "LRwearR"],
    ["RRwearL", "RRwearM", "RRwearR"],
];
const TIRE_PRESSURE_VARS: [&str; 4] = ["LFcoldPressure", "RFcoldPressure", "LRcoldPressure", "RRcoldPressure"];

pub struct ReplaySource {
    file: IbtFile,
    /// Next row to play.
    pos: usize,
    /// Rows due but not yet played (the fraction left over between calls).
    owed: f64,
    rate: f64,
    name: String,
    yaml_pending: bool,
}

impl ReplaySource {
    pub fn open(path: &Path) -> Result<Self, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        Self::from_file(IbtFile::open(path)?, name)
    }

    pub fn from_file(file: IbtFile, name: String) -> Result<Self, String> {
        if file.row_count() == 0 {
            return Err("This .ibt file has no samples in it.".into());
        }
        let rate = file.tick_rate();
        Ok(Self { file, pos: 0, owed: 0.0, rate, name, yaml_pending: true })
    }

    /// The file's name, for the status line.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The file's session info, once, before the first frame.
    pub fn take_session_yaml(&mut self) -> Option<String> {
        std::mem::take(&mut self.yaml_pending).then(|| self.file.session_yaml().to_string())
    }

    pub fn finished(&self) -> bool {
        self.pos >= self.file.row_count()
    }

    /// Rows played and rows in the file.
    pub fn progress(&self) -> (usize, usize) {
        (self.pos, self.file.row_count())
    }

    /// Start again from the first row.
    pub fn rewind(&mut self) {
        self.pos = 0;
        self.owed = 0.0;
        self.yaml_pending = true;
    }

    /// `dt` seconds of real time at `speed` times real time: how many rows
    /// are now due. Never more than are left.
    pub fn advance(&mut self, dt: f32, speed: f64) -> usize {
        self.owed += dt as f64 * speed.max(0.0) * self.rate;
        let due = self.owed.floor();
        self.owed -= due;
        (due as usize).min(self.file.row_count() - self.pos)
    }

    /// Reads the next row into `f`. False at the end of the file.
    pub fn read(&mut self, f: &mut RawFrame) -> bool {
        let Some(row) = self.file.row_bytes(self.pos) else { return false };
        read_frame(self.file.vars(), row, self.pos as i32, f);
        self.pos += 1;
        true
    }
}

/// Maps one row of recorded variables into a normalized frame (as `Irsdk::read_frame` does for live data).
pub fn read_frame(vars: &HashMap<String, VarHeader>, row: &[u8], tick: i32, f: &mut RawFrame) {
    let num = |name: &str, idx: usize| vars.get(name).and_then(|v| vars::read_num(row, v, idx));
    let f32_ = |name: &str| num(name, 0).unwrap_or(0.0) as f32;
    let f64_ = |name: &str| num(name, 0).unwrap_or(0.0);
    let i32_ = |name: &str| num(name, 0).unwrap_or(0.0) as i32;
    let bool_ = |name: &str| num(name, 0).unwrap_or(0.0) != 0.0;
    let opt = |name: &str| num(name, 0).map(|v| v as f32);

    f.tick = tick;
    f.session_num = i32_("SessionNum");
    f.session_state = i32_("SessionState");
    f.session_flags = num("SessionFlags", 0).unwrap_or(0.0) as u32;
    f.session_time = f64_("SessionTime");
    f.session_time_remain = f64_("SessionTimeRemain");
    f.session_laps_remain = i32_("SessionLapsRemainEx");

    f.player_car_idx = i32_("PlayerCarIdx");
    f.is_on_track = bool_("IsOnTrack");
    f.on_pit_road = bool_("OnPitRoad");

    f.speed = f32_("Speed");
    f.rpm = f32_("RPM");
    f.gear = i32_("Gear");
    f.throttle = f32_("Throttle");
    f.brake = f32_("Brake");
    f.brake_abs_active = bool_("BrakeABSactive");
    // A missing clutch reads as released, not as fully pressed.
    f.clutch = opt("Clutch").map(|c| 1.0 - c).unwrap_or(0.0);
    f.steering = f32_("SteeringWheelAngle");
    f.yaw_rate = f32_("YawRate");
    f.lat_accel = f32_("LatAccel");
    f.long_accel = f32_("LongAccel");
    f.vert_accel = f32_("VertAccel");
    f.pitch = f32_("Pitch");
    f.roll = f32_("Roll");
    f.velocity_x = f32_("VelocityX");
    f.velocity_y = f32_("VelocityY");

    f.sl_last_rpm = opt("PlayerCarSLLastRPM").filter(|&r| r > 0.0);
    f.sl_blink_rpm = opt("PlayerCarSLBlinkRPM").filter(|&r| r > 0.0);
    f.engine_warnings = num("EngineWarnings", 0).unwrap_or(0.0) as u32;
    f.oil_temp = opt("OilTemp");
    f.oil_press = opt("OilPress");
    f.water_temp = opt("WaterTemp");
    f.fuel_press = opt("FuelPress");
    f.voltage = opt("Voltage");
    f.brake_bias = opt("dcBrakeBias");
    f.brake_bias_fine = opt("dcBrakeBiasFine");
    f.peak_brake_bias = opt("dcPeakBrakeBias");

    f.lap = i32_("Lap");
    f.lap_completed = i32_("LapCompleted");
    f.lap_dist_pct = f32_("LapDistPct");
    f.lap_current = f32_("LapCurrentLapTime");
    f.lap_last = f32_("LapLastLapTime");
    f.lap_best = f32_("LapBestLapTime");
    let delta = |name: &str, ok: &str| bool_(ok).then(|| f32_(name));
    f.deltas = Deltas {
        best: delta("LapDeltaToBestLap", "LapDeltaToBestLap_OK"),
        optimal: delta("LapDeltaToOptimalLap", "LapDeltaToOptimalLap_OK"),
        session_best: delta("LapDeltaToSessionBestLap", "LapDeltaToSessionBestLap_OK"),
        session_optimal: delta("LapDeltaToSessionOptimalLap", "LapDeltaToSessionOptimalLap_OK"),
        // (sic) iRacing's variable name really has the extra "l".
        last: delta("LapDeltaToSessionLastlLap", "LapDeltaToSessionLastlLap_OK"),
        reference: None,
    };

    f.fuel_level = f32_("FuelLevel");
    f.fuel_level_pct = f32_("FuelLevelPct");
    f.car_left_right = i32_("CarLeftRight");
    f.incidents = i32_("PlayerCarMyIncidentCount");
    f.team_incidents = i32_("PlayerCarTeamIncidentCount");
    f.fast_repairs_used = i32_("FastRepairUsed");
    f.track_temp = f32_("TrackTempCrew");
    f.air_temp = f32_("AirTemp");

    f.time_of_day = opt("SessionTimeOfDay");
    f.skies = num("Skies", 0).map(|v| v as i32);
    f.track_wetness = num("TrackWetness", 0).map(|v| v as i32);
    f.declared_wet = bool_("WeatherDeclaredWet");
    f.wind_speed = opt("WindVel");
    f.wind_dir = opt("WindDir");
    // Documented as a percentage but published as a fraction; accept either.
    f.humidity = opt("RelativeHumidity").map(|h| if h > 1.0 { h / 100.0 } else { h });
    let frac = |v: f32| if v > 1.0 { v / 100.0 } else { v };
    f.precipitation = opt("Precipitation").map(frac);
    f.fog_level = opt("FogLevel").map(frac);
    f.yaw_north = opt("YawNorth");

    for k in 0..4 {
        for j in 0..3 {
            f.tire_temp[k][j] = f32_(TIRE_TEMP_VARS[k][j]);
            f.tire_wear[k][j] = f32_(TIRE_WEAR_VARS[k][j]);
        }
        f.tire_cold_pressure[k] = f32_(TIRE_PRESSURE_VARS[k]);
    }
    f.tire_compound = i32_("PlayerTireCompound");
    f.tire_sets_used = i32_("TireSetsUsed");
    f.tire_sets_available = num("TireSetsAvailable", 0).map(|v| v as i32).unwrap_or(255);
    f.in_pit_stall = bool_("PlayerCarInPitStall");

    f.pit_service = vars.get("PitSvFlags").map(|_| PitService {
        flags: num("PitSvFlags", 0).unwrap_or(0.0) as u32,
        fuel: f32_("PitSvFuel"),
        pressure: PIT_PRESSURE_VARS.map(f32_),
        compound: i32_("PitSvTireCompound"),
    });
    f.pit_sv_status = i32_("PlayerCarPitSvStatus");
    f.fast_repairs_available = num("FastRepairAvailable", 0).map(|v| v as i32);

    // Per-car arrays: a car the file doesn't record keeps "not in the world".
    let n = MAX_CARS;
    f.car_lap.resize(n, 0);
    f.car_lap_completed.resize(n, 0);
    f.car_lap_dist_pct.resize(n, -1.0);
    f.car_position.resize(n, 0);
    f.car_class_position.resize(n, 0);
    f.car_on_pit_road.resize(n, false);
    f.car_track_surface.resize(n, -1);
    f.car_tire_compound.resize(n, -1);
    f.car_est_time.resize(n, 0.0);
    f.car_f2_time.resize(n, 0.0);
    f.car_last_lap.resize(n, -1.0);
    f.car_best_lap.resize(n, -1.0);

    fn fill<T: Copy>(num: &dyn Fn(&str, usize) -> Option<f64>, name: &str, out: &mut [T], conv: impl Fn(f64) -> T) {
        for (i, slot) in out.iter_mut().enumerate() {
            if let Some(v) = num(name, i) {
                *slot = conv(v);
            }
        }
    }
    // A real recording holds only your own car: no CarIdx arrays at all. Put you in your own
    // slot from the scalar channels, so the per-car panels (Standings, Relative, Radar) see you.
    if !vars.contains_key("CarIdxLapDistPct") {
        if let Some(me) = usize::try_from(f.player_car_idx).ok().filter(|&i| i < n) {
            f.car_lap[me] = f.lap;
            f.car_lap_completed[me] = f.lap_completed;
            f.car_lap_dist_pct[me] = f.lap_dist_pct;
            f.car_position[me] = i32_("PlayerCarPosition");
            f.car_class_position[me] = i32_("PlayerCarClassPosition");
            f.car_on_pit_road[me] = f.on_pit_road;
            f.car_track_surface[me] = num("PlayerTrackSurface", 0).map(|v| v as i32).unwrap_or(3);
            f.car_tire_compound[me] = f.tire_compound;
            f.car_last_lap[me] = f.lap_last;
            f.car_best_lap[me] = f.lap_best;
        }
    }
    fill(&num, "CarIdxLap", &mut f.car_lap, |v| v as i32);
    fill(&num, "CarIdxLapCompleted", &mut f.car_lap_completed, |v| v as i32);
    fill(&num, "CarIdxLapDistPct", &mut f.car_lap_dist_pct, |v| v as f32);
    fill(&num, "CarIdxPosition", &mut f.car_position, |v| v as i32);
    fill(&num, "CarIdxClassPosition", &mut f.car_class_position, |v| v as i32);
    fill(&num, "CarIdxOnPitRoad", &mut f.car_on_pit_road, |v| v != 0.0);
    fill(&num, "CarIdxTrackSurface", &mut f.car_track_surface, |v| v as i32);
    fill(&num, "CarIdxTireCompound", &mut f.car_tire_compound, |v| v as i32);
    fill(&num, "CarIdxEstTime", &mut f.car_est_time, |v| v as f32);
    fill(&num, "CarIdxF2Time", &mut f.car_f2_time, |v| v as f32);
    fill(&num, "CarIdxLastLapTime", &mut f.car_last_lap, |v| v as f32);
    fill(&num, "CarIdxBestLapTime", &mut f.car_best_lap, |v| v as f32);
}

/// Records the demo race as an .ibt with every channel `read_frame` uses, other
/// cars included: a stand-in for a real recording when there isn't one to hand
/// (tests, the `make_replay_ibt` example). `laps` of the demo's lap length.
pub fn record_full_ibt(demo: &mut crate::demo::DemoSource, laps: f32) -> Vec<u8> {
    use crate::vars::VarType::{BitField, Bool, Double, Float, Int};
    type Get = fn(&RawFrame) -> Vec<f64>;
    // One entry per channel: name, type, how many values, and where the value comes from.
    let mut cols: Vec<(&str, crate::vars::VarType, usize, Get)> = vec![
        ("SessionNum", Int, 1, |f| vec![f.session_num as f64]),
        ("SessionState", Int, 1, |f| vec![f.session_state as f64]),
        ("SessionFlags", BitField, 1, |f| vec![f.session_flags as f64]),
        ("SessionTime", Double, 1, |f| vec![f.session_time]),
        ("SessionTimeRemain", Double, 1, |f| vec![f.session_time_remain]),
        ("SessionLapsRemainEx", Int, 1, |f| vec![f.session_laps_remain as f64]),
        ("PlayerCarIdx", Int, 1, |f| vec![f.player_car_idx as f64]),
        ("IsOnTrack", Bool, 1, |f| vec![f.is_on_track as u8 as f64]),
        ("OnPitRoad", Bool, 1, |f| vec![f.on_pit_road as u8 as f64]),
        ("PlayerTrackSurface", Int, 1, |f| vec![f.car_track_surface.get(f.player_car_idx.max(0) as usize).copied().unwrap_or(3) as f64]),
        ("Speed", Float, 1, |f| vec![f.speed as f64]),
        ("RPM", Float, 1, |f| vec![f.rpm as f64]),
        ("Gear", Int, 1, |f| vec![f.gear as f64]),
        ("Throttle", Float, 1, |f| vec![f.throttle as f64]),
        ("Brake", Float, 1, |f| vec![f.brake as f64]),
        ("BrakeABSactive", Bool, 1, |f| vec![f.brake_abs_active as u8 as f64]),
        ("Clutch", Float, 1, |f| vec![1.0 - f.clutch as f64]),
        ("SteeringWheelAngle", Float, 1, |f| vec![f.steering as f64]),
        ("YawRate", Float, 1, |f| vec![f.yaw_rate as f64]),
        ("LatAccel", Float, 1, |f| vec![f.lat_accel as f64]),
        ("LongAccel", Float, 1, |f| vec![f.long_accel as f64]),
        ("VertAccel", Float, 1, |f| vec![f.vert_accel as f64]),
        ("Pitch", Float, 1, |f| vec![f.pitch as f64]),
        ("Roll", Float, 1, |f| vec![f.roll as f64]),
        ("VelocityX", Float, 1, |f| vec![f.velocity_x as f64]),
        ("VelocityY", Float, 1, |f| vec![f.velocity_y as f64]),
        ("Lap", Int, 1, |f| vec![f.lap as f64]),
        ("LapCompleted", Int, 1, |f| vec![f.lap_completed as f64]),
        ("LapDistPct", Float, 1, |f| vec![f.lap_dist_pct as f64]),
        ("LapCurrentLapTime", Float, 1, |f| vec![f.lap_current as f64]),
        ("LapLastLapTime", Float, 1, |f| vec![f.lap_last as f64]),
        ("LapBestLapTime", Float, 1, |f| vec![f.lap_best as f64]),
        ("FuelLevel", Float, 1, |f| vec![f.fuel_level as f64]),
        ("FuelLevelPct", Float, 1, |f| vec![f.fuel_level_pct as f64]),
        ("CarLeftRight", Int, 1, |f| vec![f.car_left_right as f64]),
        ("PlayerCarMyIncidentCount", Int, 1, |f| vec![f.incidents as f64]),
        ("TrackTempCrew", Float, 1, |f| vec![f.track_temp as f64]),
        ("AirTemp", Float, 1, |f| vec![f.air_temp as f64]),
        ("PlayerTireCompound", Int, 1, |f| vec![f.tire_compound as f64]),
        ("PlayerCarInPitStall", Bool, 1, |f| vec![f.in_pit_stall as u8 as f64]),
    ];
    // Per-car arrays.
    cols.extend_from_slice(&[
        ("CarIdxLap", Int, MAX_CARS, |f| f.car_lap.iter().map(|&v| v as f64).collect()),
        ("CarIdxLapCompleted", Int, MAX_CARS, |f| f.car_lap_completed.iter().map(|&v| v as f64).collect()),
        ("CarIdxLapDistPct", Float, MAX_CARS, |f| f.car_lap_dist_pct.iter().map(|&v| v as f64).collect()),
        ("CarIdxPosition", Int, MAX_CARS, |f| f.car_position.iter().map(|&v| v as f64).collect()),
        ("CarIdxClassPosition", Int, MAX_CARS, |f| f.car_class_position.iter().map(|&v| v as f64).collect()),
        ("CarIdxOnPitRoad", Bool, MAX_CARS, |f| f.car_on_pit_road.iter().map(|&v| v as u8 as f64).collect()),
        ("CarIdxTrackSurface", Int, MAX_CARS, |f| f.car_track_surface.iter().map(|&v| v as f64).collect()),
        ("CarIdxEstTime", Float, MAX_CARS, |f| f.car_est_time.iter().map(|&v| v as f64).collect()),
        ("CarIdxLastLapTime", Float, MAX_CARS, |f| f.car_last_lap.iter().map(|&v| v as f64).collect()),
        ("CarIdxBestLapTime", Float, MAX_CARS, |f| f.car_best_lap.iter().map(|&v| v as f64).collect()),
    ]);

    let yaml = demo.session_yaml();
    let dt = 1.0 / 60.0;
    let steps = (laps * 92.0 / dt) as usize;
    let rows: Vec<Vec<f64>> = (0..steps)
        .map(|_| {
            let f = demo.step(dt);
            cols.iter().flat_map(|c| pad((c.3)(f), c.2)).collect()
        })
        .collect();
    let layout: Vec<(&str, crate::vars::VarType, usize)> = cols.iter().map(|c| (c.0, c.1, c.2)).collect();
    crate::ibt::write_arrays(&yaml, &layout, &rows)
}

fn pad(mut v: Vec<f64>, count: usize) -> Vec<f64> {
    v.resize(count, 0.0);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::DemoSource;

    fn source(laps: f32) -> ReplaySource {
        let mut demo = DemoSource::new();
        let file = IbtFile::from_bytes(record_full_ibt(&mut demo, laps)).expect("demo ibt");
        ReplaySource::from_file(file, "demo.ibt".into()).expect("replay")
    }

    #[test]
    fn plays_every_row_in_order_and_ends() {
        let mut r = source(0.5);
        let (_, total) = r.progress();
        assert!(total > 100, "{total} rows");
        let mut f = RawFrame::with_capacity();
        let mut last_t = f64::MIN;
        let mut n = 0;
        while r.read(&mut f) {
            assert!(f.session_time >= last_t, "time went backwards at row {n}");
            last_t = f.session_time;
            n += 1;
        }
        assert_eq!(n, total);
        assert!(r.finished());
        assert_eq!(r.advance(1.0, 1.0), 0);
    }

    #[test]
    fn pace_follows_the_file_rate_and_speed() {
        let mut r = source(0.5);
        let rate = r.rate;
        let total: usize = (0..60).map(|_| r.advance(1.0 / 60.0, 1.0)).sum();
        assert!((total as f64 - rate).abs() <= 1.0, "{total} rows in a second at {rate} Hz");
        let mut r = source(0.5);
        let fast: usize = (0..60).map(|_| r.advance(1.0 / 60.0, 4.0)).sum();
        assert!((fast as f64 - 4.0 * rate).abs() <= 1.0, "{fast} rows in a second at 4x");
    }

    #[test]
    fn session_yaml_comes_once_then_again_after_rewind() {
        let mut r = source(0.1);
        assert!(r.take_session_yaml().is_some_and(|y| y.contains("WeekendInfo")));
        assert!(r.take_session_yaml().is_none());
        r.rewind();
        assert!(r.take_session_yaml().is_some());
    }

    #[test]
    fn frames_match_the_demo_that_was_recorded() {
        let mut demo = DemoSource::new();
        let file = IbtFile::from_bytes(record_full_ibt(&mut demo, 0.5)).unwrap();
        let mut r = ReplaySource::from_file(file, "x".into()).unwrap();
        let mut f = RawFrame::with_capacity();
        for _ in 0..300 {
            assert!(r.read(&mut f));
        }
        assert!(f.speed > 0.0, "car should be moving");
        assert!(f.rpm > 0.0);
        assert!(f.player_car_idx >= 0);
        assert_eq!(f.car_lap_dist_pct.len(), MAX_CARS);
    }

    #[test]
    fn rejects_a_file_with_no_rows() {
        assert!(ReplaySource::open(Path::new("/nonexistent.ibt")).is_err());
    }

    #[test]
    fn a_recording_of_only_your_car_still_fills_your_own_slot() {
        use crate::vars::VarType::{Double, Float, Int};
        // What a real .ibt holds: scalar channels for the player, no CarIdx arrays.
        let cols = [
            ("SessionTime", Double),
            ("PlayerCarIdx", Int),
            ("Lap", Int),
            ("LapCompleted", Int),
            ("LapDistPct", Float),
            ("LapBestLapTime", Float),
            ("LapLastLapTime", Float),
            ("PlayerCarPosition", Int),
        ];
        let rows = vec![vec![10.0, 7.0, 4.0, 3.0, 0.25, 52.984, 53.437, 0.0]];
        let file = IbtFile::from_bytes(crate::ibt::write("---\n", &cols, &rows)).unwrap();
        let mut r = ReplaySource::from_file(file, "x".into()).unwrap();
        let mut f = RawFrame::with_capacity();
        assert!(r.read(&mut f));
        assert_eq!(f.player_car_idx, 7);
        assert_eq!(f.car_lap_completed[7], 3);
        assert!((f.car_lap_dist_pct[7] - 0.25).abs() < 1e-6);
        assert!((f.car_best_lap[7] - 52.984).abs() < 1e-3);
        assert!((f.car_last_lap[7] - 53.437).abs() < 1e-3);
        // Everyone else stays "not in the world".
        assert_eq!(f.car_lap_dist_pct[3], -1.0);
    }
}
