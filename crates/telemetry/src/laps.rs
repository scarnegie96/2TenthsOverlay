//! The lap log: every lap you complete, with whether it was clean, and your
//! all-time personal best for the car and track, saved to disk between sessions.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::model::{track_surface, RawFrame};

/// Laps kept in the payload; older ones are dropped from the widget, not the file.
const PAYLOAD_LAPS: usize = 100;
/// Laps kept in each car-and-track file.
const STORED_LAPS: usize = 1000;
/// How long to wait for iRacing's own lap time before using ours.
const OFFICIAL_TIME_WAIT: f64 = 3.0;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LapRecord {
    pub lap: i32,
    /// None when the lap couldn't be timed (joined mid-lap without an official time).
    pub time: Option<f32>,
    /// Driven on track the whole way, off pit road, with no new incidents.
    pub clean: bool,
    /// Touched pit road: an in or out lap.
    pub pit: bool,
    pub off_track: bool,
    /// Incident points picked up on this lap.
    pub incidents: i32,
    /// Litres.
    pub fuel_used: Option<f32>,
    pub track_temp: f32,
    pub air_temp: f32,
    /// Beat the saved personal best when it was set.
    pub personal_best: bool,
    /// Unix seconds.
    pub at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PersonalBest {
    pub time: f32,
    /// Unix seconds.
    pub at: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LapLogPayload {
    /// This session's laps, oldest first.
    pub laps: Vec<LapRecord>,
    /// Fastest clean lap this session.
    pub session_best: Option<f32>,
    /// All-time for this car and track (this session only in the demo).
    pub personal_best: Option<PersonalBest>,
    /// Whether laps are being saved to disk.
    pub saving: bool,
}

/// Where lap files live. One JSON file per car and track.
#[derive(Debug, Clone)]
pub struct LapStore {
    pub dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct StoredLaps {
    #[serde(default)]
    track: String,
    #[serde(default)]
    car: String,
    #[serde(default)]
    best: Option<PersonalBest>,
    #[serde(default)]
    laps: Vec<LapRecord>,
}

impl LapStore {
    fn path(&self, track: &str, car: &str) -> PathBuf {
        let clean = |s: &str| -> String {
            s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
        };
        self.dir.join(format!("{}__{}.json", clean(track), clean(car)))
    }

    fn load(&self, track: &str, car: &str) -> StoredLaps {
        fs::read_to_string(self.path(track, car))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn append(&self, track: &str, car: &str, lap: &LapRecord, best: Option<&PersonalBest>) {
        let mut data = self.load(track, car);
        data.track = track.to_string();
        data.car = car.to_string();
        data.best = best.cloned();
        data.laps.push(lap.clone());
        if data.laps.len() > STORED_LAPS {
            data.laps.drain(..data.laps.len() - STORED_LAPS);
        }
        let path = self.path(track, car);
        let write = || -> std::io::Result<()> {
            fs::create_dir_all(&self.dir)?;
            let tmp = path.with_extension("json.tmp");
            fs::write(&tmp, serde_json::to_string(&data).unwrap_or_default())?;
            fs::rename(&tmp, &path)
        };
        if let Err(err) = write() {
            eprintln!("[telemetry] couldn't save lap to {}: {err}", path.display());
        }
    }
}

/// A lap that has ended, waiting for iRacing's official time.
#[derive(Debug, Clone)]
struct Pending {
    record: LapRecord,
    /// Our own timing, in case iRacing never reports one.
    own_time: Option<f32>,
    ended_at: f64,
    /// `LapLastLapTime` when the lap ended; a new value is this lap's time.
    lap_last_before: f32,
}

#[derive(Default)]
pub struct LapLog {
    session_num: i32,
    store: Option<LapStore>,
    key: Option<(String, String)>,
    laps: Vec<LapRecord>,
    personal_best: Option<PersonalBest>,
    // The lap in progress.
    lap_completed: Option<i32>,
    /// None when we joined mid-lap.
    started_at: Option<f64>,
    start_fuel: f32,
    start_incidents: i32,
    pit: bool,
    off: bool,
    last_fuel: f32,
    pending: Option<Pending>,
}

impl LapLog {
    /// Sets where to save laps (None for the demo) and which car and track
    /// they belong to, loading the saved personal best.
    pub fn configure(&mut self, store: Option<LapStore>, track: &str, car: &str) {
        let key = (!track.is_empty() && !car.is_empty()).then(|| (track.to_string(), car.to_string()));
        let same_store = self.store.as_ref().map(|s| &s.dir) == store.as_ref().map(|s| &s.dir);
        if key == self.key && same_store {
            return;
        }
        self.personal_best = match (&store, &key) {
            (Some(s), Some((t, c))) => s.load(t, c).best,
            _ => None,
        };
        self.store = store;
        self.key = key;
    }

    pub fn update(&mut self, f: &RawFrame) {
        if f.session_num != self.session_num {
            self.session_num = f.session_num;
            self.laps.clear();
            self.pending = None;
            self.lap_completed = None;
        }
        self.resolve_pending(f);

        if !f.is_on_track {
            self.lap_completed = None;
            return;
        }
        let surface = f.car_track_surface.get(f.player_car_idx.max(0) as usize).copied();
        if f.on_pit_road {
            self.pit = true;
        }
        if surface == Some(track_surface::OFF_TRACK) {
            self.off = true;
        }
        let refuelled = f.fuel_level > self.last_fuel + 0.05;
        self.last_fuel = f.fuel_level;

        match self.lap_completed {
            None => self.start_lap(f, None),
            Some(prev) if f.lap_completed == prev + 1 => {
                let own_time = self.started_at.map(|s| (f.session_time - s) as f32);
                let incidents = (f.incidents - self.start_incidents).max(0);
                let record = LapRecord {
                    lap: f.lap_completed,
                    time: None,
                    clean: self.started_at.is_some() && !self.pit && !self.off && incidents == 0,
                    pit: self.pit || f.on_pit_road,
                    off_track: self.off,
                    incidents,
                    fuel_used: (self.started_at.is_some() && !refuelled && !self.pit)
                        .then(|| self.start_fuel - f.fuel_level)
                        .filter(|u| *u > 0.0),
                    track_temp: f.track_temp,
                    air_temp: f.air_temp,
                    personal_best: false,
                    at: now(),
                };
                // A lap still waiting is as good as it'll get.
                if let Some(p) = self.pending.take() {
                    self.finish(p.record, p.own_time);
                }
                self.pending = Some(Pending { record, own_time, ended_at: f.session_time, lap_last_before: f.lap_last });
                self.start_lap(f, Some(f.session_time));
            }
            // Reset, tow or a jump in laps: time the next one from scratch.
            Some(prev) if f.lap_completed != prev => self.start_lap(f, None),
            Some(_) => {}
        }
    }

    fn start_lap(&mut self, f: &RawFrame, at: Option<f64>) {
        self.lap_completed = Some(f.lap_completed);
        self.started_at = at;
        self.start_fuel = f.fuel_level;
        self.last_fuel = f.fuel_level;
        self.start_incidents = f.incidents;
        self.pit = f.on_pit_road;
        self.off = false;
    }

    fn resolve_pending(&mut self, f: &RawFrame) {
        let Some(p) = &self.pending else { return };
        let official = f.lap_last > 0.0 && f.lap_last != p.lap_last_before;
        let waited = f.session_time - p.ended_at > OFFICIAL_TIME_WAIT || f.session_time < p.ended_at;
        if official || waited {
            let p = self.pending.take().expect("checked above");
            let time = if official { Some(f.lap_last) } else { p.own_time };
            self.finish(p.record, time);
        }
    }

    fn finish(&mut self, mut record: LapRecord, time: Option<f32>) {
        record.time = time.filter(|t| *t > 1.0);
        if let (true, Some(t)) = (record.clean, record.time) {
            if self.personal_best.as_ref().is_none_or(|b| t < b.time) {
                record.personal_best = true;
                self.personal_best = Some(PersonalBest { time: t, at: record.at });
            }
        }
        if let (Some(store), Some((track, car))) = (&self.store, &self.key) {
            store.append(track, car, &record, self.personal_best.as_ref());
        }
        self.laps.push(record);
    }

    pub fn payload(&self) -> LapLogPayload {
        let start = self.laps.len().saturating_sub(PAYLOAD_LAPS);
        LapLogPayload {
            laps: self.laps[start..].to_vec(),
            session_best: self
                .laps
                .iter()
                .filter(|l| l.clean)
                .filter_map(|l| l.time)
                .min_by(|a, b| a.total_cmp(b)),
            personal_best: self.personal_best.clone(),
            saving: self.store.is_some() && self.key.is_some(),
        }
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> RawFrame {
        let mut f = RawFrame::with_capacity();
        f.is_on_track = true;
        f.player_car_idx = 0;
        f.car_track_surface[0] = track_surface::ON_TRACK;
        f.fuel_level = 50.0;
        f.track_temp = 30.0;
        f
    }

    /// Drives one lap of `secs` seconds; iRacing reports `official` a second after the line.
    fn lap(log: &mut LapLog, f: &mut RawFrame, secs: f64, official: Option<f32>, off: bool) {
        let end = f.session_time + secs;
        while f.session_time < end {
            f.session_time += 0.5;
            f.fuel_level -= 0.01;
            f.car_track_surface[0] = if off && f.session_time > end - secs / 2.0 && f.session_time < end - secs / 2.0 + 1.0 {
                track_surface::OFF_TRACK
            } else {
                track_surface::ON_TRACK
            };
            log.update(f);
        }
        f.lap_completed += 1;
        log.update(f);
        f.session_time += 1.0;
        if let Some(t) = official {
            f.lap_last = t;
        }
        log.update(f);
    }

    #[test]
    fn logs_laps_and_marks_the_clean_best() {
        let mut log = LapLog::default();
        let mut f = frame();
        log.update(&f);
        lap(&mut log, &mut f, 80.0, Some(80.4), false); // joined mid-lap: not clean, but official time
        lap(&mut log, &mut f, 90.0, Some(90.1), false);
        lap(&mut log, &mut f, 88.0, Some(88.2), true);
        lap(&mut log, &mut f, 91.0, None, false); // no official time: our own after the wait
        f.session_time += 5.0;
        log.update(&f);

        let p = log.payload();
        assert_eq!(p.laps.len(), 4);
        assert_eq!(p.laps[0].time, Some(80.4));
        assert!(!p.laps[0].clean);
        assert!(p.laps[1].clean && p.laps[1].personal_best);
        assert!(p.laps[2].off_track && !p.laps[2].clean);
        assert!(p.laps[3].clean && !p.laps[3].personal_best);
        assert!((p.laps[3].time.unwrap() - 92.0).abs() < 0.6);
        assert_eq!(p.session_best, Some(90.1));
        assert_eq!(p.personal_best.map(|b| b.time), Some(90.1));
        assert!(p.laps[1].fuel_used.is_some());
    }

    #[test]
    fn incidents_and_pit_spoil_a_lap() {
        let mut log = LapLog::default();
        let mut f = frame();
        log.update(&f);
        lap(&mut log, &mut f, 80.0, Some(80.0), false);
        f.incidents = 1;
        lap(&mut log, &mut f, 80.0, Some(81.0), false);
        f.on_pit_road = true;
        lap(&mut log, &mut f, 80.0, Some(82.0), false);
        let p = log.payload();
        assert_eq!(p.laps[1].incidents, 1);
        assert!(!p.laps[1].clean);
        assert!(p.laps[2].pit && !p.laps[2].clean);
        assert_eq!(p.session_best, None);
    }

    #[test]
    fn personal_best_survives_between_sessions() {
        let dir = std::env::temp_dir().join(format!("tenths-laps-{}", now() ^ std::process::id() as u64));
        let store = LapStore { dir: dir.clone() };

        let mut log = LapLog::default();
        log.configure(Some(store.clone()), "spa up", "porsche 992");
        let mut f = frame();
        log.update(&f);
        lap(&mut log, &mut f, 80.0, Some(80.0), false);
        lap(&mut log, &mut f, 85.0, Some(85.5), false);
        assert!(log.payload().saving);

        let mut next = LapLog::default();
        next.configure(Some(store), "spa up", "porsche 992");
        assert_eq!(next.payload().personal_best.map(|b| b.time), Some(85.5));
        assert!(next.payload().laps.is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
