//! Reader for iRacing's saved lap files: `.blap` (best lap) and `.olap`
//! (optimal lap), found in Documents\iRacing\lapfiles.
//!
//! The format isn't published. This is what version 3 files contain, worked
//! out from real files (all little-endian):
//!
//! ```text
//! 0     "BLAP"  magic
//! 4     u32     version (3)
//! 16    char[]  driver name (NUL-terminated)
//! 144   char[]  car id, e.g. "formulair04"
//! 1342  char[]  track id, e.g. "watkinsglen\2021\fullcourse"
//! 1460  f32     lap time, seconds
//! 1468  u32     sector count N
//! 1472  N x 32-byte sectors:
//!         f32 start (m), f32 end (m), u32 record count, f32 record spacing (m),
//!         f32, f32 (unknown), f32 sector time (s), u32 (unknown, 3)
//! 1472 + 32N    records, 28 bytes each, one every ~2 m round the lap:
//!         f32 seconds since the sector started
//!         f32 sideways position on track (m, roughly)
//!         f32 heading (rad)
//!         f32 pitch, f32 roll (rad, roughly), f32 unused
//!         u8 throttle (0-255), u8 brake (0-255), u8 clutch, u8 gear (1 = mid-shift)
//! ```
//!
//! Steering isn't stored, so references from these files have no steering
//! channel: no reference wheel and no steering cues. (.ibt files do record it.)

use std::path::Path;

use crate::ibt::{IbtLap, IbtSummary};
use crate::reference::{RefKind, RefMeta, RefSample, ReferenceLap};
use crate::vars::cstr;

const MAGIC: &[u8; 4] = b"BLAP";
const OFF_DRIVER: usize = 16;
const OFF_CAR: usize = 144;
const OFF_TRACK: usize = 1342;
const OFF_LAP_TIME: usize = 1460;
const OFF_SECTOR_COUNT: usize = 1468;
const OFF_SECTORS: usize = 1472;
const SECTOR_SIZE: usize = 32;
const RECORD_SIZE: usize = 28;

#[derive(Debug, Clone)]
pub struct BlapSector {
    pub start_m: f32,
    pub end_m: f32,
    pub records: usize,
    pub spacing_m: f32,
    pub time: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct BlapRecord {
    /// Seconds since the lap started (sector times added up).
    pub time: f32,
    /// Metres since the start line.
    pub dist: f32,
    pub lateral: f32,
    pub heading: f32,
    pub throttle: f32,
    pub brake: f32,
    pub gear: i32,
}

#[derive(Debug, Clone)]
pub struct BlapFile {
    pub driver: String,
    pub car_key: String,
    pub track_key: String,
    pub lap_time: f32,
    pub sectors: Vec<BlapSector>,
    pub records: Vec<BlapRecord>,
    pub optimal: bool,
}

fn f32_at(b: &[u8], at: usize) -> Option<f32> {
    b.get(at..at + 4).map(|x| f32::from_le_bytes(x.try_into().unwrap()))
}
fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|x| u32::from_le_bytes(x.try_into().unwrap()))
}

impl BlapFile {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
        let optimal = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("olap"));
        Self::from_bytes(&bytes, optimal)
    }

    pub fn from_bytes(b: &[u8], optimal: bool) -> Result<Self, String> {
        let unsupported = || "This lap file isn't in a format Tenths understands (expected an iRacing .blap/.olap, version 3).".to_string();
        if b.len() < OFF_SECTORS || &b[0..4] != MAGIC {
            return Err(unsupported());
        }
        if u32_at(b, 4) != Some(3) {
            return Err(unsupported());
        }
        let n_sectors = u32_at(b, OFF_SECTOR_COUNT).ok_or_else(unsupported)? as usize;
        if !(1..=64).contains(&n_sectors) {
            return Err(unsupported());
        }
        let mut sectors = Vec::with_capacity(n_sectors);
        for k in 0..n_sectors {
            let o = OFF_SECTORS + k * SECTOR_SIZE;
            sectors.push(BlapSector {
                start_m: f32_at(b, o).ok_or_else(unsupported)?,
                end_m: f32_at(b, o + 4).ok_or_else(unsupported)?,
                records: u32_at(b, o + 8).ok_or_else(unsupported)? as usize,
                spacing_m: f32_at(b, o + 12).ok_or_else(unsupported)?,
                time: f32_at(b, o + 24).ok_or_else(unsupported)?,
            });
        }
        // The file must be exactly header + sectors + records, or we've misread it.
        let total: usize = sectors.iter().map(|s| s.records).sum();
        let data = OFF_SECTORS + n_sectors * SECTOR_SIZE;
        if data + total * RECORD_SIZE != b.len() {
            return Err(unsupported());
        }

        let mut records = Vec::with_capacity(total);
        let mut at = data;
        let mut time_base = 0.0f32;
        let mut last_gear = 0;
        for s in &sectors {
            for j in 0..s.records {
                let t = f32_at(b, at).unwrap();
                let raw_gear = b[at + 27] as i32;
                // 1 shows up only for a sample or two while changing gear.
                let gear = if raw_gear == 1 && last_gear > 2 { last_gear } else { raw_gear };
                last_gear = gear;
                records.push(BlapRecord {
                    time: time_base + t,
                    dist: s.start_m + j as f32 * s.spacing_m,
                    lateral: f32_at(b, at + 4).unwrap(),
                    heading: f32_at(b, at + 8).unwrap(),
                    throttle: b[at + 24] as f32 / 255.0,
                    brake: b[at + 25] as f32 / 255.0,
                    gear,
                });
                at += RECORD_SIZE;
            }
            time_base += s.time;
        }

        Ok(Self {
            driver: cstr(&b[OFF_DRIVER..OFF_DRIVER + 64]),
            car_key: cstr(&b[OFF_CAR..OFF_CAR + 64]),
            track_key: cstr(&b[OFF_TRACK..OFF_TRACK + 64]),
            lap_time: f32_at(b, OFF_LAP_TIME).unwrap_or(0.0),
            sectors,
            records,
            optimal,
        })
    }

    pub fn length_m(&self) -> f32 {
        self.sectors.last().map(|s| s.end_m).unwrap_or(0.0)
    }

    /// Readable track name from the id: "watkinsglen\2021\fullcourse" -> ("watkinsglen", "fullcourse").
    fn track_names(&self) -> (String, String) {
        let parts: Vec<&str> = self.track_key.split(['\\', '/']).collect();
        let name = parts.first().copied().unwrap_or("").to_string();
        let config = if parts.len() > 1 { parts[parts.len() - 1].to_string() } else { String::new() };
        (name, config)
    }

    pub fn summary(&self) -> IbtSummary {
        let (track_name, track_config) = self.track_names();
        IbtSummary {
            track_name,
            track_config,
            car_name: self.car_key.clone(),
            driver: self.driver.clone(),
            laps: vec![IbtLap::summary_only(1, self.lap_time, true)],
            kind: RefKind::Blap,
        }
    }

    pub fn reference(&self, source: &str) -> Result<ReferenceLap, String> {
        let len = self.length_m();
        if len <= 0.0 || self.records.len() < 20 {
            return Err("That lap file has no usable data.".into());
        }
        let first = self.records[0];
        let last = self.records[self.records.len() - 1];
        let sample = |r: &BlapRecord, pct: f32, time: f32| RefSample {
            pct,
            time,
            speed: None,
            throttle: r.throttle,
            brake: r.brake,
            gear: r.gear,
            steering: None,
            heading: Some(r.heading),
            yaw_rate: None,
        };
        let mut samples: Vec<RefSample> = self.records.iter().map(|r| sample(r, r.dist / len, r.time)).collect();
        samples.push(sample(&last, 1.0, self.lap_time));
        samples[0] = sample(&first, 0.0, 0.0);

        let (track_name, track_config) = self.track_names();
        let meta = RefMeta {
            track_name,
            track_config,
            track_key: self.track_key.clone(),
            car_name: self.car_key.clone(),
            car_key: self.car_key.clone(),
            driver: self.driver.clone(),
            lap_number: 0,
            lap_time: self.lap_time,
            source: source.to_string(),
            kind: RefKind::Blap,
            length_m: len,
        };
        ReferenceLap::build(meta, &samples).ok_or_else(|| "That lap file has no usable data.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real Formula iR-04 best lap at Watkins Glen, supplied for testing.
    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/formulair04-watkinsglen.blap");

    #[test]
    fn reads_real_blap() {
        let f = BlapFile::from_bytes(FIXTURE, false).unwrap();
        assert_eq!(f.car_key, "formulair04");
        assert_eq!(f.track_key, r"watkinsglen\2021\fullcourse");
        assert!((f.lap_time - 106.7726).abs() < 1e-3);
        assert_eq!(f.sectors.len(), 5);
        assert_eq!(f.records.len(), 2703);
        // Sector times add up to the lap.
        let sum: f32 = f.sectors.iter().map(|s| s.time).sum();
        assert!((sum - f.lap_time).abs() < 0.01);
        // Time runs forward and ends at the lap time.
        assert!(f.records.windows(2).all(|w| w[1].time >= w[0].time));
        assert!((f.records.last().unwrap().time - f.lap_time).abs() < 0.1);
        // No mid-shift "gear 1" left over.
        assert!(f.records.iter().all(|r| (4..=7).contains(&r.gear)));

        let r = f.reference("test.blap").unwrap();
        assert!(r.matches("", "", "watkinsglen 2021 fullcourse"));
        let speeds: Vec<f32> = (0..=100).map(|i| r.speed_at(i as f32 / 100.0).unwrap() * 3.6).collect();
        let max = speeds.iter().cloned().fold(0.0, f32::max);
        let min = speeds.iter().cloned().fold(f32::MAX, f32::min);
        assert!((200.0..240.0).contains(&max), "top speed {max}");
        assert!((60.0..140.0).contains(&min), "slowest {min}");
        assert!(r.curvature.iter().any(|k| k.abs() > 0.01), "has corners");
        assert!(r.steering.is_empty());
    }

    #[test]
    fn rejects_other_files() {
        assert!(BlapFile::from_bytes(b"BLAP\x03\x00\x00\x00 too short", false).is_err());
        assert!(BlapFile::from_bytes(&[0u8; 4000], false).is_err());
        let mut bad = FIXTURE.to_vec();
        bad.pop();
        assert!(BlapFile::from_bytes(&bad, false).is_err());
    }
}
