//! Reader for iRacing `.ibt` telemetry files (Alt+L in the car, saved to
//! Documents\iRacing\telemetry).
//!
//! An .ibt uses the same layout as the live shared memory, plus a small
//! "disk sub-header" with the record count, and the samples stored one after
//! another instead of in a rotating buffer:
//!
//! ```text
//! irsdk_header (112) | disk sub-header (32) | var headers | session YAML | row 0 | row 1 | ...
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::reference::{RefKind, RefMeta, RefSample, ReferenceLap};
use crate::session::{self, SessionInfo};
use crate::vars::{self, VarHeader, VarType, VAR_HEADER_SIZE};

const HEADER_SIZE: usize = 112;
const SUB_HEADER_SIZE: usize = 32;
const H_TICK_RATE: usize = 8;
const H_SESSION_INFO_LEN: usize = 16;
const H_SESSION_INFO_OFFSET: usize = 20;
const H_NUM_VARS: usize = 24;
const H_VAR_HEADER_OFFSET: usize = 28;
const H_BUF_LEN: usize = 36;
const H_BUF0_OFFSET: usize = 52;
const SUB_RECORD_COUNT: usize = HEADER_SIZE + 28;

pub struct IbtFile {
    bytes: Vec<u8>,
    vars: HashMap<String, VarHeader>,
    yaml: String,
    data_offset: usize,
    row_len: usize,
    rows: usize,
}

/// One complete lap found in the file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IbtLap {
    pub lap: i32,
    pub lap_time: f32,
    /// No pit road and no off-track moments.
    pub clean: bool,
    #[serde(skip)]
    samples: Vec<RefSample>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IbtSummary {
    pub track_name: String,
    pub track_config: String,
    pub car_name: String,
    pub driver: String,
    pub laps: Vec<IbtLap>,
    pub kind: RefKind,
}

impl IbtLap {
    /// A lap entry with no samples, for listing.
    pub fn summary_only(lap: i32, lap_time: f32, clean: bool) -> Self {
        Self { lap, lap_time, clean, samples: Vec::new() }
    }
}

impl IbtFile {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
        Self::from_bytes(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        let bad = || "This doesn't look like an iRacing .ibt telemetry file.".to_string();
        let get = |at| vars::le_i32(&bytes, at).ok_or_else(bad);
        let num_vars = get(H_NUM_VARS)?.max(0) as usize;
        let var_off = get(H_VAR_HEADER_OFFSET)?.max(0) as usize;
        let yaml_len = get(H_SESSION_INFO_LEN)?.max(0) as usize;
        let yaml_off = get(H_SESSION_INFO_OFFSET)?.max(0) as usize;
        let row_len = get(H_BUF_LEN)?.max(0) as usize;
        let data_offset = get(H_BUF0_OFFSET)?.max(0) as usize;
        let records = get(SUB_RECORD_COUNT)?.max(0) as usize;

        if num_vars == 0 || row_len == 0 || data_offset > bytes.len() {
            return Err(bad());
        }
        let var_bytes = bytes.get(var_off..var_off + num_vars * VAR_HEADER_SIZE).ok_or_else(bad)?;
        let vars = vars::parse_headers(var_bytes, num_vars);
        let yaml = session::decode_latin1(bytes.get(yaml_off..yaml_off + yaml_len).ok_or_else(bad)?);
        // The record count is 0 if iRacing didn't close the file cleanly; trust the size then.
        let fit = (bytes.len() - data_offset) / row_len;
        let rows = if records == 0 { fit } else { records.min(fit) };

        Ok(Self { bytes, vars, yaml, data_offset, row_len, rows })
    }

    pub fn session(&self) -> Option<SessionInfo> {
        session::parse(&self.yaml).ok()
    }

    /// The session info YAML as stored in the file (its state at the end of the recording).
    pub fn session_yaml(&self) -> &str {
        &self.yaml
    }

    /// Samples a second the file was recorded at (iRacing's `tickRate`; 60 unless the header says otherwise).
    pub fn tick_rate(&self) -> f64 {
        vars::le_i32(&self.bytes, H_TICK_RATE).filter(|&r| (1..=1000).contains(&r)).unwrap_or(60) as f64
    }

    /// The variables the file recorded, and sample `i`'s bytes: what `replay` maps into frames.
    pub(crate) fn vars(&self) -> &HashMap<String, VarHeader> {
        &self.vars
    }

    pub(crate) fn row_bytes(&self, i: usize) -> Option<&[u8]> {
        (i < self.rows).then(|| self.row(i))
    }

    /// Number of samples in the file.
    pub fn row_count(&self) -> usize {
        self.rows
    }

    /// One variable's value in sample `row`, or None if the file doesn't have it.
    pub fn value(&self, row: usize, name: &str) -> Option<f64> {
        (row < self.rows).then(|| self.reader(name)(self.row(row))).flatten()
    }

    fn row(&self, i: usize) -> &[u8] {
        let at = self.data_offset + i * self.row_len;
        &self.bytes[at..at + self.row_len]
    }

    fn reader<'a>(&'a self, name: &str) -> impl Fn(&[u8]) -> Option<f64> + 'a {
        let v = self.vars.get(name);
        move |row| v.and_then(|v| vars::read_num(row, v, 0))
    }

    /// Finds every complete lap: started and finished on the start/finish line
    /// with the car on track the whole time.
    pub fn laps(&self) -> Vec<IbtLap> {
        let lap_v = self.reader("Lap");
        let pct_v = self.reader("LapDistPct");
        let time_v = self.reader("SessionTime");
        let on_track_v = self.reader("IsOnTrack");
        let pit_v = self.reader("OnPitRoad");
        let surface_v = self.reader("PlayerTrackSurface");
        let speed_v = self.reader("Speed");
        let throttle_v = self.reader("Throttle");
        let brake_v = self.reader("Brake");
        let gear_v = self.reader("Gear");
        let steer_v = self.reader("SteeringWheelAngle");
        let yaw_v = self.reader("YawRate");

        struct Building {
            lap: i32,
            samples: Vec<(f64, RefSample)>,
            dirty: bool,
        }
        let mut laps = Vec::new();
        let mut cur: Option<Building> = None;

        for i in 0..self.rows {
            let row = self.row(i);
            let (Some(lap), Some(pct), Some(t)) = (lap_v(row), pct_v(row), time_v(row)) else { continue };
            let (lap, pct) = (lap as i32, pct as f32);
            if on_track_v(row) == Some(0.0) || pct < 0.0 {
                cur = None;
                continue;
            }
            if cur.as_ref().is_some_and(|b| b.lap != lap) {
                let b = cur.take().unwrap();
                if lap == b.lap + 1 {
                    if let Some(done) = finish(b.lap, &b.samples, (pct, t), b.dirty) {
                        laps.push(done);
                    }
                }
            }
            let b = cur.get_or_insert_with(|| Building { lap, samples: Vec::new(), dirty: false });
            let f = |v: Option<f64>| v.map(|x| x as f32);
            b.samples.push((
                t,
                RefSample {
                    pct,
                    time: 0.0,
                    speed: f(speed_v(row)),
                    throttle: f(throttle_v(row)).unwrap_or(0.0),
                    brake: f(brake_v(row)).unwrap_or(0.0),
                    gear: gear_v(row).unwrap_or(0.0) as i32,
                    steering: f(steer_v(row)),
                    heading: None,
                    yaw_rate: f(yaw_v(row)),
                },
            ));
            // Off track (surface 0) or on pit road taints the lap.
            if pit_v(row) == Some(1.0) || surface_v(row) == Some(0.0) {
                b.dirty = true;
            }
        }
        laps
    }

    pub fn summary(&self) -> IbtSummary {
        let s = self.session().unwrap_or_default();
        let player = s.player();
        IbtSummary {
            track_name: s.track_name.clone(),
            track_config: s.track_config.clone(),
            car_name: player.map(|p| p.car_name.clone()).unwrap_or_default(),
            driver: player.map(|p| p.user_name.clone()).unwrap_or_default(),
            laps: self.laps(),
            kind: RefKind::Ibt,
        }
    }

    /// Turns one lap of this file into a comparison lap.
    pub fn reference(&self, lap: i32, source: &str) -> Result<ReferenceLap, String> {
        let session = self.session().unwrap_or_default();
        let player = session.player();
        let found = self
            .laps()
            .into_iter()
            .find(|l| l.lap == lap)
            .ok_or_else(|| format!("Lap {lap} isn't a complete lap in this file."))?;
        let meta = RefMeta {
            track_name: session.track_name.clone(),
            track_config: session.track_config.clone(),
            track_key: session.track_key.clone(),
            car_name: player.map(|p| p.car_name.clone()).unwrap_or_default(),
            car_key: player.map(|p| p.car_path.clone()).unwrap_or_default(),
            driver: player.map(|p| p.user_name.clone()).unwrap_or_default(),
            lap_number: lap,
            lap_time: found.lap_time,
            source: source.to_string(),
            kind: RefKind::Ibt,
            length_m: session.track_length_km * 1000.0,
        };
        ReferenceLap::build(meta, &found.samples).ok_or_else(|| "That lap doesn't have enough data to compare against.".into())
    }
}

/// Closes a lap: works out exactly when it crossed the line at each end
/// (between samples) and expresses every sample as seconds since the start.
fn finish(lap: i32, s: &[(f64, RefSample)], next: (f32, f64), dirty: bool) -> Option<IbtLap> {
    if s.len() < 60 {
        return None;
    }
    let (t0, first) = s[0];
    let (tl, last) = s[s.len() - 1];
    let (p0, pl) = (first.pct, last.pct);
    // Must start just after the line and end just before it.
    if p0 > 0.05 || pl < 0.95 || next.0 > 0.05 {
        return None;
    }
    // Reject tows and resets: position should only move forward.
    if s.windows(2).any(|w| w[1].1.pct + 0.01 < w[0].1.pct) {
        return None;
    }
    let (t1, second) = s[1];
    let rate = if second.pct > p0 { (t1 - t0) / (second.pct - p0) as f64 } else { 0.0 };
    let start = t0 - p0 as f64 * rate;
    let span = (next.0 + 1.0 - pl) as f64;
    let end = if span > 0.0 { tl + (1.0 - pl) as f64 / span * (next.1 - tl) } else { tl };
    let lap_time = (end - start) as f32;
    if lap_time < 10.0 {
        return None;
    }

    let mut samples = Vec::with_capacity(s.len() + 2);
    samples.push(RefSample { pct: 0.0, time: 0.0, ..first });
    samples.extend(s.iter().map(|&(t, x)| RefSample { time: (t - start) as f32, ..x }));
    samples.push(RefSample { pct: 1.0, time: lap_time, ..last });
    Some(IbtLap { lap, lap_time, clean: !dirty, samples })
}

/// Writes a minimal .ibt. Used by tests and the `make_ibt` example so the
/// import path can be exercised without iRacing.
pub fn write(yaml: &str, columns: &[(&str, VarType)], rows: &[Vec<f64>]) -> Vec<u8> {
    let counted: Vec<(&str, VarType, usize)> = columns.iter().map(|&(n, t)| (n, t, 1)).collect();
    write_arrays(yaml, &counted, rows)
}

/// `write` for columns that hold several values (iRacing's `CarIdx*` arrays have 64).
/// A row lists each column's values in order, `count` of them per column.
pub fn write_arrays(yaml: &str, columns: &[(&str, VarType, usize)], rows: &[Vec<f64>]) -> Vec<u8> {
    let mut offsets = Vec::new();
    let mut row_len = 0;
    for (_, ty, count) in columns {
        offsets.push(row_len);
        row_len += ty.size() * count;
    }
    let var_off = HEADER_SIZE + SUB_HEADER_SIZE;
    let yaml_off = var_off + columns.len() * VAR_HEADER_SIZE;
    let yaml_bytes: Vec<u8> = yaml.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }).collect();
    let data_off = yaml_off + yaml_bytes.len() + 1;

    let mut out = vec![0u8; data_off];
    let mut put = |at: usize, v: i32| out[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(0, 2); // version
    put(4, 1); // status
    put(8, 60); // tick rate
    put(H_SESSION_INFO_LEN, yaml_bytes.len() as i32);
    put(H_SESSION_INFO_OFFSET, yaml_off as i32);
    put(H_NUM_VARS, columns.len() as i32);
    put(H_VAR_HEADER_OFFSET, var_off as i32);
    put(32, 1); // numBuf
    put(H_BUF_LEN, row_len as i32);
    put(H_BUF0_OFFSET, data_off as i32);
    put(SUB_RECORD_COUNT, rows.len() as i32);

    for (k, (name, ty, count)) in columns.iter().enumerate() {
        let at = var_off + k * VAR_HEADER_SIZE;
        out[at..at + 4].copy_from_slice(&ty.to_raw().to_le_bytes());
        out[at + 4..at + 8].copy_from_slice(&(offsets[k] as i32).to_le_bytes());
        out[at + 8..at + 12].copy_from_slice(&(*count as i32).to_le_bytes());
        out[at + 16..at + 16 + name.len()].copy_from_slice(name.as_bytes());
    }
    out[yaml_off..yaml_off + yaml_bytes.len()].copy_from_slice(&yaml_bytes);

    for row in rows {
        let mut next = 0;
        for (_, ty, count) in columns {
            for _ in 0..*count {
                let v = row.get(next).copied().unwrap_or(0.0);
                next += 1;
                match ty {
                    VarType::Char | VarType::Bool => out.push(v as u8),
                    VarType::Int => out.extend_from_slice(&(v as i32).to_le_bytes()),
                    VarType::BitField => out.extend_from_slice(&(v as u32).to_le_bytes()),
                    VarType::Float => out.extend_from_slice(&(v as f32).to_le_bytes()),
                    VarType::Double => out.extend_from_slice(&v.to_le_bytes()),
                }
            }
        }
    }
    out
}

/// The columns `write` uses for a driving lap.
pub const LAP_COLUMNS: &[(&str, VarType)] = &[
    ("SessionTime", VarType::Double),
    ("Lap", VarType::Int),
    ("LapDistPct", VarType::Float),
    ("IsOnTrack", VarType::Bool),
    ("OnPitRoad", VarType::Bool),
    ("PlayerTrackSurface", VarType::Int),
    ("Speed", VarType::Float),
    ("Throttle", VarType::Float),
    ("Brake", VarType::Float),
    ("Gear", VarType::Int),
    ("SteeringWheelAngle", VarType::Float),
    ("YawRate", VarType::Float),
];

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "---\nWeekendInfo:\n TrackDisplayName: Test Ring\n TrackConfigName: GP\n TrackLength: 4.00 km\nDriverInfo:\n DriverCarIdx: 0\n Drivers:\n - CarIdx: 0\n   UserName: Tester\n   CarScreenNameShort: Test Car\n";

    /// Drives 3.4 laps at 60 Hz: lap 1 joins mid-lap, laps 2 and 3 are full
    /// (lap 3 visits pit road), lap 4 is unfinished.
    fn sample_file() -> Vec<u8> {
        let lap_time: f64 = 80.0;
        let mut rows = Vec::new();
        let mut t: f64 = 100.0;
        let mut progress: f64 = 1.6; // start 60% into lap 1
        while progress < 4.4 {
            let lap = progress.floor();
            let pct = progress - lap;
            let pit = if lap == 3.0 && pct > 0.5 && pct < 0.52 { 1.0 } else { 0.0 };
            let braking = (0.3..0.35).contains(&pct);
            rows.push(vec![t, lap, pct, 1.0, pit, 3.0, 50.0, if braking { 0.0 } else { 1.0 }, if braking { 0.8 } else { 0.0 }, 4.0, 0.2, 0.1]);
            t += 1.0 / 60.0;
            progress += (1.0 / 60.0) / lap_time;
        }
        write(YAML, LAP_COLUMNS, &rows)
    }

    #[test]
    fn finds_complete_laps() {
        let f = IbtFile::from_bytes(sample_file()).unwrap();
        let s = f.summary();
        assert_eq!(s.track_name, "Test Ring");
        assert_eq!(s.car_name, "Test Car");
        assert_eq!(s.driver, "Tester");
        let laps: Vec<(i32, bool)> = s.laps.iter().map(|l| (l.lap, l.clean)).collect();
        assert_eq!(laps, vec![(2, true), (3, false)]);
        for l in &s.laps {
            assert!((l.lap_time - 80.0).abs() < 0.01, "lap time {}", l.lap_time);
        }
    }

    #[test]
    fn builds_reference_from_lap() {
        let f = IbtFile::from_bytes(sample_file()).unwrap();
        let r = f.reference(2, "test.ibt").unwrap();
        assert!((r.time_at(0.5) - 40.0).abs() < 0.01);
        assert!((r.brake_at(0.32).unwrap() - 0.8).abs() < 1e-3);
        assert!(r.throttle_at(0.6).unwrap() > 0.99);
        assert_eq!(r.gear_at(0.5), Some(4));
        assert!((r.steering_at(0.5).unwrap() - 0.2).abs() < 1e-3);
        assert!((r.speed_at(0.5).unwrap() - 50.0).abs() < 1e-3);
        assert!(r.matches("Test Ring", "GP", ""));
        assert!(f.reference(1, "x").is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(IbtFile::from_bytes(vec![0u8; 50]).is_err());
    }
}
