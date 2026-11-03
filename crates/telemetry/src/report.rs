//! Race reports: a record of every race you drive, saved as JSON when you
//! finish (and again as the rest of the field finishes), or when the session
//! ends if you didn't.
//!
//! What's in one: the event (track, car, classes and strength of field), every
//! driver's grid and finishing positions, the moment each of them crossed the
//! line on every lap (from which the front end works out lap times, gaps and
//! positions lap by lap), their tyre stints and pit stops, and your own laps
//! with fuel used, incidents and track temperature. Nothing about your inputs
//! (throttle, brake, steering) is kept.
//!
//! Files go in `reports/` in the app data folder, one per race, named by when
//! the race was recorded, the track and the car.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::finish::state;
use crate::laps::LapRecord;
use crate::model::{ClassSummary, RawFrame, MAX_CARS};
use crate::pitloss::TimedStop;
use crate::session::SessionInfo;
use crate::stints::CarStints;

/// irsdk_SessionState racing: crossings before the green flag aren't race laps.
const RACING: i32 = 4;
/// After the first save, how often to update the file as others finish.
const RESAVE_EVERY: Duration = Duration::from_secs(10);

/// Where reports are saved; None in the demo.
#[derive(Debug, Clone)]
pub struct ReportStore {
    pub dir: PathBuf,
    /// Saving is optional (Settings → Races): while this is false nothing is written.
    pub enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl ReportStore {
    /// Always saving, for tools and tests.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into(), enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)) }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RaceReport {
    pub version: u32,
    /// File name without `.json`.
    pub id: String,
    /// Unix seconds when the race was first recorded.
    pub started_at: u64,
    /// Unix seconds of this save.
    pub saved_at: u64,
    pub track: String,
    pub track_config: String,
    pub track_length_km: f32,
    pub car: String,
    pub session_type: String,
    pub official: bool,
    pub team_racing: bool,
    /// Scheduled race laps, when it's a lap race.
    pub total_laps: Option<i32>,
    pub player_car_idx: i32,
    pub player_class_id: i32,
    /// You took the chequered flag (false for a race you left or retired from).
    pub finished: bool,
    pub classes: Vec<ClassSummary>,
    /// Tyre names by compound number (your car's list), for the stints.
    pub tire_compounds: Vec<String>,
    pub cars: Vec<ReportCar>,
    /// Your laps: time, clean or not, pit, off track, incidents, fuel used, temperatures.
    pub player_laps: Vec<LapRecord>,
    /// Incident points at the end (yours, or the team's in team events).
    pub incidents: i32,
    pub incident_limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportCar {
    pub car_idx: i32,
    pub user_id: i64,
    pub name: String,
    pub team: String,
    pub number: String,
    pub class_id: i32,
    pub irating: i32,
    pub license: String,
    pub is_player: bool,
    /// Class position on the grid, when known.
    pub start_position: Option<i32>,
    /// Position overall and in class at the end (or when last seen).
    pub finish_position: i32,
    pub finish_class_position: i32,
    pub laps_completed: i32,
    /// Took the chequered flag.
    pub finished: bool,
    /// Left the sim before the end (retired, towed, quit).
    pub out: bool,
    /// Session time each lap was completed, by lap number (lap 1 is the first race lap).
    pub crossings: Vec<Crossing>,
    pub stints: Vec<crate::stints::Stint>,
    pub stops: u32,
    pub pit_stops: Vec<ReportStop>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Crossing {
    pub lap: i32,
    /// Session seconds.
    pub t: f64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportStop {
    /// Completed laps when the car came in.
    pub lap: i32,
    /// Pit entry to exit.
    pub lane_s: f32,
    /// Standing in the box.
    pub stall_s: f32,
}

#[derive(Debug, Default, Clone)]
struct CarLog {
    last_lap: Option<i32>,
    crossings: Vec<Crossing>,
    finished: bool,
    position: i32,
    class_position: i32,
    out: bool,
}

/// Collects a race as it happens and decides when to save it.
#[derive(Debug, Default)]
pub struct RaceRecorder {
    session_num: Option<i32>,
    race: bool,
    started_at: u64,
    id: Option<String>,
    cars: BTreeMap<i32, CarLog>,
    dirty: bool,
    saved_at: Option<Instant>,
    /// The flag was out (so later crossings are finishes).
    checkered: bool,
}

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl RaceRecorder {
    /// True when `f` belongs to a different session from the one being
    /// recorded: save what there is (see `wants_final_save`) before updating.
    pub fn session_changed(&self, f: &RawFrame) -> bool {
        self.session_num.is_some_and(|n| n != f.session_num)
    }

    pub fn update(&mut self, f: &RawFrame, race: bool) {
        if self.session_num != Some(f.session_num) {
            *self = Self { session_num: Some(f.session_num), race, started_at: unix_now(), ..Self::default() };
        }
        self.race = race;
        if !race {
            return;
        }
        if f.session_state >= state::CHECKERED {
            self.checkered = true;
        }
        let racing = f.session_state >= RACING;
        let n = f.car_lap_completed.len().min(MAX_CARS);
        for i in 0..n {
            let lap = f.car_lap_completed[i];
            let surface = f.car_track_surface.get(i).copied().unwrap_or(-1);
            let car = self.cars.entry(i as i32).or_default();
            let pos = f.car_position.get(i).copied().unwrap_or(0);
            if pos > 0 {
                car.position = pos;
                car.class_position = f.car_class_position.get(i).copied().unwrap_or(0);
            }
            car.out = surface == crate::model::track_surface::NOT_IN_WORLD && car.last_lap.is_some();
            if lap < 0 {
                continue;
            }
            match car.last_lap {
                Some(prev) if lap > prev && racing && !car.finished => {
                    car.crossings.push(Crossing { lap, t: f.session_time });
                    if self.checkered {
                        car.finished = true;
                    }
                    self.dirty = true;
                }
                _ => {}
            }
            car.last_lap = Some(lap);
        }
    }

    /// Whether `car_idx` has taken the flag (as far as we've seen).
    pub fn finished(&self, car_idx: i32) -> bool {
        self.cars.get(&car_idx).is_some_and(|c| c.finished)
    }

    /// While you've finished: save now, then again every few seconds while more cars finish.
    pub fn wants_save(&self, player_finished: bool) -> bool {
        self.race && player_finished && (self.saved_at.is_none() || (self.dirty && self.saved_at.is_some_and(|t| t.elapsed() >= RESAVE_EVERY)))
    }

    /// At the end of the session: worth saving if you raced at least a lap.
    pub fn wants_final_save(&self, player_car_idx: i32) -> bool {
        self.race && (self.dirty || self.saved_at.is_none()) && self.cars.get(&player_car_idx).is_some_and(|c| !c.crossings.is_empty())
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.saved_at = Some(Instant::now());
    }

    /// Builds the report from what's been recorded and the other trackers.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        s: &SessionInfo,
        session_num: i32,
        classes: Vec<ClassSummary>,
        stints: &[CarStints],
        stops: &[TimedStop],
        player_laps: &[LapRecord],
        player_finished: bool,
        incidents: i32,
        grid: impl Fn(i32) -> Option<i32>,
    ) -> RaceReport {
        let player = s.player();
        let car_name = player.map(|p| p.car_name.clone()).unwrap_or_default();
        let id = self
            .id
            .get_or_insert_with(|| report_id(self.started_at, &s.track_name, &car_name))
            .clone();
        let def = s.session(session_num);
        let cars = s
            .drivers
            .iter()
            .filter(|d| !d.is_spectator && !d.is_pace_car && d.car_idx >= 0)
            .map(|d| {
                let log = self.cars.get(&d.car_idx).cloned().unwrap_or_default();
                let st = stints.iter().find(|c| c.car_idx == d.car_idx);
                ReportCar {
                    car_idx: d.car_idx,
                    user_id: d.user_id,
                    name: d.user_name.clone(),
                    team: d.team_name.clone(),
                    number: d.car_number.clone(),
                    class_id: d.class_id,
                    irating: d.irating,
                    license: d.license.clone(),
                    is_player: Some(d.car_idx) == player.map(|p| p.car_idx),
                    start_position: grid(d.car_idx),
                    finish_position: log.position,
                    finish_class_position: log.class_position,
                    laps_completed: log.crossings.last().map(|c| c.lap).unwrap_or(0),
                    finished: log.finished,
                    out: log.out && !log.finished,
                    crossings: log.crossings,
                    stints: st.map(|c| c.stints.clone()).unwrap_or_default(),
                    stops: st.map(|c| c.stops).unwrap_or(0),
                    pit_stops: stops
                        .iter()
                        .filter(|p| p.car_idx == d.car_idx)
                        .map(|p| ReportStop { lap: p.lap, lane_s: p.lane_s, stall_s: p.stall_s })
                        .collect(),
                }
            })
            // Cars that never turned a lap or got a position were never in the race.
            .filter(|c| c.finish_position > 0 || !c.crossings.is_empty())
            .collect();
        RaceReport {
            version: 1,
            id,
            started_at: self.started_at,
            saved_at: unix_now(),
            track: s.track_name.clone(),
            track_config: s.track_config.clone(),
            track_length_km: s.track_length_km,
            car: car_name,
            session_type: def.map(|d| d.session_type.clone()).unwrap_or_default(),
            official: s.official,
            team_racing: s.team_racing,
            total_laps: def.and_then(|d| d.laps),
            player_car_idx: player.map(|p| p.car_idx).unwrap_or(-1),
            player_class_id: player.map(|p| p.class_id).unwrap_or(-1),
            finished: player_finished,
            classes,
            tire_compounds: s.tire_compounds.clone(),
            cars,
            player_laps: player_laps.to_vec(),
            incidents,
            incident_limit: s.incident_limit,
        }
    }
}

/// `20261101-183045_spa_gt3`: when, where and in what; safe as a file name.
pub fn report_id(started_at: u64, track: &str, car: &str) -> String {
    let slug = |s: &str| -> String {
        let mut out = String::new();
        for c in s.chars() {
            if c.is_ascii_alphanumeric() {
                out.push(c.to_ascii_lowercase());
            } else if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
        }
        out.trim_end_matches('-').chars().take(40).collect()
    };
    format!("{}_{}_{}", utc_stamp(started_at), slug(track), slug(car))
}

/// Unix seconds as `YYYYMMDD-HHMMSS` (UTC).
fn utc_stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

impl ReportStore {
    /// Writes in the background (temporary file, then rename), so the telemetry thread never waits on the disk.
    pub fn save(&self, report: &RaceReport) {
        let Ok(text) = serde_json::to_string(report) else { return };
        let dir = self.dir.clone();
        let id = report.id.clone();
        std::thread::spawn(move || {
            if let Err(e) = write(&dir, &id, &text) {
                log!("[telemetry] couldn't save race report {id}: {e}");
            }
        });
    }
}

fn write(dir: &Path, id: &str, text: &str) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("{id}.json"));
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(state: i32, t: f64, laps: &[i32]) -> RawFrame {
        let mut f = RawFrame::with_capacity();
        f.session_num = 1;
        f.session_state = state;
        f.session_time = t;
        for (i, l) in laps.iter().enumerate() {
            f.car_lap_completed[i] = *l;
            f.car_position[i] = i as i32 + 1;
            f.car_class_position[i] = i as i32 + 1;
            f.car_track_surface[i] = crate::model::track_surface::ON_TRACK;
        }
        f
    }

    #[test]
    fn records_crossings_from_the_green_to_the_flag() {
        let mut r = RaceRecorder::default();
        r.update(&frame(3, 0.0, &[0, 0]), true); // formation: nothing yet
        r.update(&frame(4, 10.0, &[0, 0]), true);
        r.update(&frame(4, 100.0, &[1, 0]), true);
        r.update(&frame(4, 101.5, &[1, 1]), true);
        r.update(&frame(4, 199.0, &[2, 1]), true);
        r.update(&frame(state::CHECKERED, 250.0, &[2, 1]), true);
        r.update(&frame(state::CHECKERED, 298.0, &[3, 1]), true); // leader finishes
        r.update(&frame(state::CHECKERED, 302.0, &[3, 2]), true); // second finishes
        r.update(&frame(state::CHECKERED, 400.0, &[4, 2]), true); // cool-down lap: ignored
        let a = &r.cars[&0];
        let b = &r.cars[&1];
        assert_eq!(a.crossings.iter().map(|c| c.lap).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(b.crossings.iter().map(|c| (c.lap, c.t)).collect::<Vec<_>>(), vec![(1, 101.5), (2, 302.0)]);
        assert!(a.finished && b.finished);
        assert!(r.wants_final_save(1));
        assert!(r.wants_save(true));
        assert!(!r.wants_save(false));
    }

    #[test]
    fn not_outside_races_and_new_sessions_start_again() {
        let mut r = RaceRecorder::default();
        r.update(&frame(4, 10.0, &[0]), false);
        r.update(&frame(4, 100.0, &[1]), false);
        assert!(r.cars.is_empty() && !r.wants_final_save(0));
        let mut next = frame(4, 5.0, &[0]);
        next.session_num = 2;
        assert!(r.session_changed(&next));
        r.update(&next, true);
        assert!(!r.session_changed(&next));
    }

    #[test]
    fn ids_are_safe_file_names() {
        // 2026-11-01 18:30:45 UTC
        assert_eq!(report_id(1_793_557_845, "Circuit de Spa-Francorchamps", "BMW M4 GT3"), "20261101-183045_circuit-de-spa-francorchamps_bmw-m4-gt3");
        assert_eq!(utc_stamp(951_782_400), "20000229-000000"); // a leap day
        assert_eq!(report_id(0, "", ""), "19700101-000000__");
    }
}
