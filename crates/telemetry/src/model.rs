//! Data types: the normalized raw frame every source produces, and the
//! payloads the engine sends to the UI.

use serde::Serialize;

pub const MAX_CARS: usize = 64;

/// `CarIdxTrackSurface` / `PlayerTrackSurface` values (irsdk_TrkLoc).
pub mod track_surface {
    pub const NOT_IN_WORLD: i32 = -1;
    pub const OFF_TRACK: i32 = 0;
    pub const IN_PIT_STALL: i32 = 1;
    pub const APPROACHING_PITS: i32 = 2;
    pub const ON_TRACK: i32 = 3;
}

/// `CarLeftRight` values (irsdk_CarLeftRight).
/// `SessionFlags` bits (irsdk_Flags). The Flags widget decodes the rest.
pub mod session_flags {
    pub const GREEN: u32 = 0x0004;
    pub const YELLOW: u32 = 0x0008;
    pub const BLUE: u32 = 0x0020;
    pub const YELLOW_WAVING: u32 = 0x0100;
}

pub mod car_left_right {
    pub const OFF: i32 = 0;
    pub const CLEAR: i32 = 1;
    pub const CAR_LEFT: i32 = 2;
    pub const CAR_RIGHT: i32 = 3;
    pub const CAR_LEFT_RIGHT: i32 = 4;
    pub const TWO_CARS_LEFT: i32 = 5;
    pub const TWO_CARS_RIGHT: i32 = 6;
}

/// One telemetry sample, normalized from whichever source produced it.
/// Field names follow the iRacing variable they come from.
#[derive(Debug, Clone, Default)]
pub struct RawFrame {
    pub tick: i32,
    pub session_num: i32,
    pub session_state: i32,
    pub session_flags: u32,
    pub session_time: f64,
    /// Seconds; iRacing reports a huge value when the session is lap-limited.
    pub session_time_remain: f64,
    /// `SessionLapsRemainEx`; 32767 when unlimited.
    pub session_laps_remain: i32,

    pub player_car_idx: i32,
    pub is_on_track: bool,
    pub on_pit_road: bool,

    pub speed: f32,
    pub rpm: f32,
    pub gear: i32,
    pub throttle: f32,
    pub brake: f32,
    /// `BrakeABSactive`: the ABS is working on the brakes.
    pub brake_abs_active: bool,
    /// Pedal position, 0 = released, 1 = pressed (already inverted from iRacing's `Clutch`).
    pub clutch: f32,
    pub steering: f32,
    /// rad/s
    pub yaw_rate: f32,
    /// m/s², car frame. Lateral positive = left, longitudinal positive = forwards.
    pub lat_accel: f32,
    pub long_accel: f32,
    pub vert_accel: f32,
    /// rad
    pub pitch: f32,
    pub roll: f32,
    /// m/s, car frame: X forwards, Y left.
    pub velocity_x: f32,
    pub velocity_y: f32,

    /// `EngineWarnings` bitfield.
    pub engine_warnings: u32,
    /// None when the car doesn't expose the channel.
    pub oil_temp: Option<f32>,
    pub oil_press: Option<f32>,
    pub water_temp: Option<f32>,
    pub fuel_press: Option<f32>,
    pub voltage: Option<f32>,
    /// `dcBrakeBias` and friends; None when the car has no adjustable bias.
    pub brake_bias: Option<f32>,
    pub brake_bias_fine: Option<f32>,
    pub peak_brake_bias: Option<f32>,

    pub lap: i32,
    pub lap_completed: i32,
    pub lap_dist_pct: f32,
    pub lap_current: f32,
    pub lap_last: f32,
    pub lap_best: f32,
    /// Live deltas from iRacing (the engine fills in `reference`).
    pub deltas: Deltas,

    pub fuel_level: f32,
    pub fuel_level_pct: f32,

    /// Shift-light RPMs for the current gear (PlayerCarSLLastRPM / PlayerCarSLBlinkRPM).
    /// None when not published or zero.
    pub sl_last_rpm: Option<f32>,
    pub sl_blink_rpm: Option<f32>,

    pub car_left_right: i32,
    pub incidents: i32,
    /// PlayerCarTeamIncidentCount: what the incident limit counts in team events.
    pub team_incidents: i32,
    /// FastRepairUsed
    pub fast_repairs_used: i32,
    pub track_temp: f32,
    pub air_temp: f32,

    // Conditions. None when the variable isn't published.
    /// SessionTimeOfDay, seconds since midnight in the sim.
    pub time_of_day: Option<f32>,
    /// Skies: 0 clear, 1 partly cloudy, 2 mostly cloudy, 3 overcast.
    pub skies: Option<i32>,
    /// TrackWetness (irsdk_TrackWetness): 1 dry .. 7 extremely wet, 0 unknown.
    pub track_wetness: Option<i32>,
    pub declared_wet: bool,
    /// m/s
    pub wind_speed: Option<f32>,
    /// radians
    pub wind_dir: Option<f32>,
    /// 0..1
    pub humidity: Option<f32>,
    /// Precipitation, 0..1. Only published since iRacing added rain.
    pub precipitation: Option<f32>,
    /// FogLevel, 0..1.
    pub fog_level: Option<f32>,
    /// YawNorth: the car's heading relative to north, radians.
    pub yaw_north: Option<f32>,

    /// Tires in LF, RF, LR, RR order. iRacing only refreshes temps and wear
    /// while the car sits in its pit box; on track they hold the last reading.
    pub tire_temp: [[f32; 3]; 4],
    /// Tread remaining, 0..1, for the left / middle / right of each tire.
    pub tire_wear: [[f32; 3]; 4],
    pub tire_cold_pressure: [f32; 4],
    pub tire_compound: i32,
    pub tire_sets_used: i32,
    pub tire_sets_available: i32,
    pub in_pit_stall: bool,

    /// What's ticked for the next pit stop (`PitSv*`). None when not published (spectating).
    pub pit_service: Option<crate::pit::PitService>,
    /// PlayerCarPitSvStatus (irsdk_PitSvStatus): 0 none, 1 in progress, 2 done, 100+ a problem.
    pub pit_sv_status: i32,
    /// FastRepairAvailable; 255 when unlimited.
    pub fast_repairs_available: Option<i32>,

    pub car_lap: Vec<i32>,
    pub car_lap_completed: Vec<i32>,
    pub car_lap_dist_pct: Vec<f32>,
    pub car_position: Vec<i32>,
    pub car_class_position: Vec<i32>,
    pub car_on_pit_road: Vec<bool>,
    pub car_track_surface: Vec<i32>,
    /// CarIdxTireCompound: index into the session's tyre list, -1 if unknown.
    pub car_tire_compound: Vec<i32>,
    pub car_est_time: Vec<f32>,
    pub car_f2_time: Vec<f32>,
    pub car_last_lap: Vec<f32>,
    pub car_best_lap: Vec<f32>,
}

impl RawFrame {
    pub fn with_capacity() -> Self {
        Self {
            car_lap: vec![0; MAX_CARS],
            car_lap_completed: vec![0; MAX_CARS],
            car_lap_dist_pct: vec![-1.0; MAX_CARS],
            car_position: vec![0; MAX_CARS],
            car_class_position: vec![0; MAX_CARS],
            car_on_pit_road: vec![false; MAX_CARS],
            car_track_surface: vec![track_surface::NOT_IN_WORLD; MAX_CARS],
            car_tire_compound: vec![-1; MAX_CARS],
            car_est_time: vec![0.0; MAX_CARS],
            car_f2_time: vec![0.0; MAX_CARS],
            car_last_lap: vec![-1.0; MAX_CARS],
            car_best_lap: vec![-1.0; MAX_CARS],
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Payloads sent to the UI (camelCase JSON).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    None,
    Iracing,
    Demo,
    /// An .ibt file played back (`--replay`).
    Replay,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPayload {
    pub source: SourceKind,
    /// True when a sim (real or demo) is delivering frames.
    pub connected: bool,
    pub message: String,
}

/// Sent once per session-info change.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionPayload {
    pub track_name: String,
    pub track_config: String,
    pub track_length_km: f32,
    pub session_type: String,
    pub player_car_idx: i32,
    pub player_class_id: i32,
    pub car_name: String,
    pub fuel_max_ltr: f32,
    pub est_lap_time: f32,
    pub shift_rpm: f32,
    pub redline_rpm: f32,
    pub multiclass: bool,
    /// An official session, where iRating changes.
    pub official: bool,
    /// Tire compound names indexed by compound number, e.g. ["Hard", "Wet"].
    pub tire_compounds: Vec<String>,
    /// Where each timing sector starts, as a fraction of the lap (first is 0).
    pub sectors: Vec<f32>,
}

/// The imported comparison lap currently loaded.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReferencePayload {
    pub loaded: bool,
    pub track_name: String,
    pub track_config: String,
    pub car_name: String,
    pub driver: String,
    pub lap_time: f32,
    pub source: String,
    /// True when it's for the track and layout of the current session.
    pub matches_track: bool,
    /// False when the reference is a different car from yours (when both are known).
    pub matches_car: bool,
    /// "ibt" or "blap".
    pub kind: crate::reference::RefKind,
    /// Steering was recorded (otherwise the ghost wheel is estimated).
    pub has_steering: bool,
}

/// Sent every telemetry frame (60 Hz). Keep this small.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TickPayload {
    pub session_time: f64,
    pub is_on_track: bool,
    pub on_pit_road: bool,
    /// m/s
    pub speed: f32,
    pub rpm: f32,
    /// When to shift up in the current gear: where the car's shift lights blink.
    pub shift_rpm: Option<f32>,
    pub gear: i32,
    pub throttle: f32,
    pub brake: f32,
    /// The ABS is working on the brakes.
    pub abs_active: bool,
    pub clutch: f32,
    /// radians, positive = left
    pub steering: f32,
    pub lap: i32,
    pub lap_dist_pct: f32,
    pub lap_current: f32,
    pub lap_last: Option<f32>,
    pub lap_best: Option<f32>,
    /// Your best clean sectors this session added up.
    pub lap_optimal: Option<f32>,
    /// Seconds versus each comparison lap at this point of the lap; positive = slower.
    pub deltas: Deltas,
    pub fuel_level: f32,
    pub car_left_right: i32,
    pub incidents: i32,
    /// Cars within radar range, for the radar widget.
    pub nearby: Vec<NearbyCar>,
    /// Ghost inputs and the live coaching cue, when a reference lap for this track is loaded.
    pub coach: Option<crate::coach::CoachPayload>,
    /// g-forces, attitude and balance for the Dynamics widget.
    pub dynamics: crate::dynamics::DynamicsPayload,
    /// Warning lights, engine gauges and brake bias.
    pub instruments: crate::instruments::InstrumentsPayload,
    /// The car's compass heading, radians clockwise from north. For the
    /// weather widget's wind compass, which turns with the car.
    pub heading: Option<f32>,
}

/// Live delta against each kind of comparison lap. None when iRacing (or the
/// reference) has nothing valid to compare against yet.
#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Deltas {
    /// iRacing's "best lap" reference.
    pub best: Option<f32>,
    /// Best sectors stitched together.
    pub optimal: Option<f32>,
    /// Your best lap this session.
    pub session_best: Option<f32>,
    pub session_optimal: Option<f32>,
    /// Your previous lap.
    pub last: Option<f32>,
    /// An imported lap (.ibt), computed by us.
    pub reference: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearbyCar {
    pub car_idx: i32,
    /// Metres along the track, positive = ahead of the player.
    pub dist: f32,
    /// Best guess at the lane: -1 left, 0 in line, 1 right (from CarLeftRight).
    pub side: i32,
    pub class_color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverCell {
    pub car_idx: i32,
    pub name: String,
    pub abbrev: String,
    pub number: String,
    pub class_id: i32,
    pub class_name: String,
    pub class_color: String,
    pub irating: i32,
    /// iRacing customer id (`UserID`), the same in every session; 0 when unknown.
    pub user_id: i64,
    pub license: String,
    pub license_color: String,
}

/// How a driver's last lap compares.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LapFlag {
    #[default]
    None,
    /// Their own best lap of the session (green).
    Personal,
    /// The fastest lap in their class this session (purple).
    Fastest,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LapInfo {
    pub last_lap: Option<f32>,
    pub best_lap: Option<f32>,
    pub last_flag: LapFlag,
    /// Holds the fastest lap in its class.
    pub class_fastest: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelativeRow {
    #[serde(flatten)]
    pub driver: DriverCell,
    pub position: i32,
    pub class_position: i32,
    /// Seconds. Positive = this car is ahead of the player on track.
    pub gap: f32,
    /// +1 = a lap or more ahead of the player, -1 = a lap or more behind, 0 = same lap.
    pub lap_status: i32,
    #[serde(flatten)]
    pub laps: LapInfo,
    pub on_pit_road: bool,
    pub status: CarStatus,
    pub is_player: bool,
    /// Tyre compound fitted (`CarIdxTireCompound`): an index into the session's
    /// `tire_compounds`, or -1 when unknown.
    pub compound: i32,
}

/// Where a car is, for the pit and out symbols in the timing tables.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum CarStatus {
    #[default]
    Track,
    /// Between the pit lane lines.
    PitLane,
    /// Stopped in its pit box.
    PitStall,
    /// Not in the sim: in the garage, towed, or left the session.
    Out,
}

impl CarStatus {
    pub fn of(f: &RawFrame, car_idx: usize) -> Self {
        let surface = f.car_track_surface.get(car_idx).copied().unwrap_or(track_surface::NOT_IN_WORLD);
        if surface == track_surface::NOT_IN_WORLD {
            CarStatus::Out
        } else if surface == track_surface::IN_PIT_STALL {
            CarStatus::PitStall
        } else if f.car_on_pit_road.get(car_idx).copied().unwrap_or(false) {
            CarStatus::PitLane
        } else {
            CarStatus::Track
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingRow {
    #[serde(flatten)]
    pub driver: DriverCell,
    pub position: i32,
    pub class_position: i32,
    pub lap: i32,
    /// How far round the current lap the car is (0..1), -1 when it isn't on track.
    pub lap_pct: f32,
    /// Seconds behind the class leader (race) or best-lap difference (practice / qualifying).
    pub gap: Option<f32>,
    pub laps_down: i32,
    /// Seconds to the car one place ahead in class.
    pub interval: Option<f32>,
    /// Class position at the start of the race; None outside races or when unknown.
    pub start_position: Option<i32>,
    #[serde(flatten)]
    pub laps: LapInfo,
    pub on_pit_road: bool,
    pub status: CarStatus,
    pub is_player: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FuelPayload {
    pub level: f32,
    pub max: f32,
    /// Average litres per green lap (last 5).
    pub per_lap: Option<f32>,
    pub last_lap_used: Option<f32>,
    /// Least and most litres used on a clean green lap this session.
    pub min_per_lap: Option<f32>,
    pub max_per_lap: Option<f32>,
    pub laps_of_fuel: Option<f32>,
    /// Laps the player still has to drive, including the rest of this lap.
    pub laps_to_go: Option<f32>,
    /// Litres needed to finish beyond what's in the tank (negative = spare).
    pub to_finish: Option<f32>,
    pub samples: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionClock {
    pub session_type: String,
    pub time_remain: Option<f64>,
    pub laps_remain: Option<i32>,
    pub total_laps: Option<i32>,
    pub flags: u32,
    pub track_temp: f32,
    pub air_temp: f32,
    /// SoF of the player's class.
    pub strength_of_field: Option<i32>,
    /// Every class in the field with its car count and SoF, fastest class first.
    pub classes: Vec<ClassSummary>,
    /// Seconds since midnight in the sim.
    pub time_of_day: Option<f32>,
    pub skies: Option<i32>,
    pub track_wetness: Option<i32>,
    pub declared_wet: bool,
    /// m/s
    pub wind_speed: Option<f32>,
    /// radians
    pub wind_dir: Option<f32>,
    /// 0..1
    pub humidity: Option<f32>,
    /// Precipitation, 0..1, when iRacing publishes it.
    pub precipitation: Option<f32>,
    /// Fog, 0..1.
    pub fog_level: Option<f32>,
    /// The player's count, or the team's in team events (what the limit applies to).
    pub incidents: i32,
    /// None when unlimited.
    pub incident_limit: Option<i32>,
    pub fast_repairs_used: i32,
    /// None when unlimited.
    pub fast_repairs_limit: Option<i32>,
    /// You've taken the chequered flag in this race.
    pub player_finished: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClassSummary {
    pub class_id: i32,
    pub name: String,
    pub color: String,
    /// Entries in the class (spectators and the pace car excluded).
    pub cars: i32,
    pub strength_of_field: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TireCorner {
    /// Carcass temperature °C, left / middle / right as seen from behind the car.
    pub temp: [f32; 3],
    /// Tread remaining 0..1, left / middle / right.
    pub wear: [f32; 3],
    /// Cold pressure set in the garage, kPa.
    pub cold_pressure: f32,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TiresPayload {
    /// LF, RF, LR, RR
    pub corners: Vec<TireCorner>,
    pub compound: String,
    pub sets_used: i32,
    /// None when unlimited.
    pub sets_available: Option<i32>,
    /// True while values are live (car in its pit box).
    pub live: bool,
    /// False until iRacing has reported any reading this session.
    pub has_data: bool,
}

/// What iRacing has ticked for your next pit stop.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PitPayload {
    /// False when iRacing doesn't publish pit service (spectating).
    pub available: bool,
    /// Tires to change, LF, RF, LR, RR.
    pub tires: [bool; 4],
    /// kPa each tire will be set to.
    pub pressures: [f32; 4],
    /// The compound that will be fitted.
    pub compound: String,
    pub fuel: bool,
    /// Litres to add.
    pub fuel_amount: f32,
    pub tearoff: bool,
    pub fast_repair: bool,
    /// None when unlimited or unknown.
    pub fast_repairs_left: Option<i32>,
    /// irsdk_PitSvStatus: 0 none, 1 in progress, 2 done, 100+ a problem (too far forward...).
    pub status: i32,
    pub in_pit_stall: bool,
}

/// Sent at 10 Hz: everything about the field and strategy.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FieldPayload {
    pub clock: SessionClock,
    pub relative: Vec<RelativeRow>,
    pub standings: Vec<StandingRow>,
    pub fuel: FuelPayload,
    pub tires: TiresPayload,
    pub pit: PitPayload,
    pub lap_log: crate::laps::LapLogPayload,
    /// Every car's stints this session.
    pub stints: Vec<crate::stints::CarStints>,
    /// Average pit stop loss per class, once enough stops have been timed.
    pub pit_loss: Vec<crate::pitloss::PitLoss>,
}
