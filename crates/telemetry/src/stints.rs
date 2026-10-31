//! Every car's stints: how many laps each has run between pit stops, and on
//! which tyre compound.
//!
//! iRacing doesn't publish stints, so we build them from what it does give for
//! every car: laps completed and distance round the lap (race progress), on
//! pit road or in the pit stall, in the world or not, and the tyre compound
//! (`CarIdxTireCompound`). A stint runs from leaving the pit lane (or the
//! start, or when we first see the car on track) to entering it again, or to
//! the car leaving the sim (retired, towed, quit). Each entry to the pit lane
//! counts as a stop. iRacing doesn't say whether another car changed tyres,
//! only what compound it's on, so a fuel-only stop still starts a new stint.

use std::collections::HashMap;

use serde::Serialize;

use crate::model::{track_surface, RawFrame, MAX_CARS};

/// Why a stint ended.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StintEnd {
    /// Into the pit lane.
    Pit,
    /// Left the sim: retired, towed or quit.
    Out,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Stint {
    /// Race progress (laps completed plus distance round the lap) at the start.
    pub start: f32,
    /// At the end, or where the car is now for the stint it's on.
    pub end: f32,
    /// Still running.
    pub open: bool,
    pub ended_by: Option<StintEnd>,
    /// Index into the session's tyre list (`DriverInfo.DriverTires`), -1 if unknown.
    pub compound: i32,
    /// We only saw part of it (Tenths started, or the car was first seen, mid-stint).
    pub partial: bool,
}

impl Stint {
    pub fn laps(&self) -> f32 {
        (self.end - self.start).max(0.0)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CarStints {
    pub car_idx: i32,
    pub stints: Vec<Stint>,
    /// Times into the pit lane.
    pub stops: u32,
    pub in_pits: bool,
    /// Left the sim.
    pub out: bool,
}

#[derive(Debug, Default)]
pub struct StintTracker {
    session_num: Option<i32>,
    cars: HashMap<i32, CarStints>,
    /// Pit lane last frame, per car.
    was_in_pits: Vec<bool>,
}

fn progress(f: &RawFrame, i: usize) -> Option<f32> {
    let pct = *f.car_lap_dist_pct.get(i)?;
    if pct < 0.0 {
        return None;
    }
    Some(f.car_lap_completed.get(i).copied().unwrap_or(0).max(0) as f32 + pct.clamp(0.0, 1.0))
}

impl StintTracker {
    pub fn update(&mut self, f: &RawFrame) {
        if self.session_num != Some(f.session_num) {
            *self = Self { session_num: Some(f.session_num), was_in_pits: vec![false; MAX_CARS], ..Self::default() };
        }
        let n = f.car_track_surface.len().min(MAX_CARS);
        for i in 0..n {
            let surface = f.car_track_surface[i];
            let out = surface == track_surface::NOT_IN_WORLD;
            let in_pits = !out && (f.car_on_pit_road.get(i).copied().unwrap_or(false) || surface == track_surface::IN_PIT_STALL);
            let compound = f.car_tire_compound.get(i).copied().unwrap_or(-1);
            let was = std::mem::replace(&mut self.was_in_pits[i], in_pits);
            let p = progress(f, i);
            let key = i as i32;

            if out {
                // Close whatever was running where the car was last seen.
                if let Some(c) = self.cars.get_mut(&key) {
                    if let Some(s) = c.stints.last_mut().filter(|s| s.open) {
                        s.open = false;
                        s.ended_by = Some(StintEnd::Out);
                    }
                    c.out = true;
                    c.in_pits = false;
                }
                continue;
            }
            let Some(p) = p else { continue };
            let c = self.cars.entry(key).or_insert_with(|| CarStints {
                car_idx: key,
                stints: Vec::new(),
                stops: 0,
                in_pits,
                out: false,
            });
            c.out = false;
            c.in_pits = in_pits;

            // Progress going backwards by more than a lap: a reset; start this car again.
            if c.stints.last().is_some_and(|s| p + 1.0 < s.end) {
                c.stints.clear();
                c.stops = 0;
            }

            let open = c.stints.last().is_some_and(|s| s.open);
            if in_pits {
                if open {
                    let s = c.stints.last_mut().unwrap();
                    s.end = p;
                    s.open = false;
                    s.ended_by = Some(StintEnd::Pit);
                }
                if !was && (open || !c.stints.is_empty()) {
                    c.stops += 1;
                } else if !was && c.stints.is_empty() && p > 0.5 {
                    // First seen coming into the pits mid-race: still a stop.
                    c.stops += 1;
                }
            } else if !open {
                // Out of the pits, at the start, or first seen on track (or back after a tow).
                let first = c.stints.is_empty();
                c.stints.push(Stint {
                    start: p,
                    end: p,
                    open: true,
                    ended_by: None,
                    compound,
                    // A first stint picked up well into the lap count, not from the pits.
                    partial: first && !was && p > 0.5,
                });
            } else {
                let s = c.stints.last_mut().unwrap();
                s.end = s.end.max(p);
                // The compound can update just after the car leaves the pits.
                if compound >= 0 {
                    s.compound = compound;
                }
            }
        }
    }

    /// Every car we've seen, by car index.
    pub fn payload(&self) -> Vec<CarStints> {
        let mut v: Vec<CarStints> = self.cars.values().cloned().collect();
        v.sort_by_key(|c| c.car_idx);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> RawFrame {
        let mut f = RawFrame::with_capacity();
        f.session_num = 1;
        f
    }

    /// Car 0 at race progress `p`, on track or in the pits, on a compound.
    fn at(f: &mut RawFrame, p: f32, pits: bool, compound: i32) {
        f.car_lap_completed[0] = p.floor() as i32;
        f.car_lap_dist_pct[0] = p.fract();
        f.car_track_surface[0] = if pits { track_surface::IN_PIT_STALL } else { track_surface::ON_TRACK };
        f.car_on_pit_road[0] = pits;
        f.car_tire_compound[0] = compound;
    }

    #[test]
    fn stints_split_at_pit_stops() {
        let mut t = StintTracker::default();
        let mut f = frame();
        // Lap 0 to 12.9 on compound 0, pit, out on compound 1, run to 20.3.
        let mut p = 0.0;
        while p < 12.9 {
            at(&mut f, p, false, 0);
            t.update(&f);
            p += 0.05;
        }
        at(&mut f, 12.95, true, 0);
        t.update(&f);
        at(&mut f, 13.02, true, 0);
        t.update(&f);
        let mut p = 13.05;
        while p < 20.3 {
            at(&mut f, p, false, 1);
            t.update(&f);
            p += 0.05;
        }
        let c = &t.payload()[0];
        assert_eq!(c.stops, 1);
        assert_eq!(c.stints.len(), 2);
        assert!(!c.stints[0].partial && c.stints[0].start < 0.01);
        assert!((c.stints[0].laps() - 12.95).abs() < 0.06, "{:?}", c.stints[0]);
        assert_eq!((c.stints[0].ended_by, c.stints[0].compound), (Some(StintEnd::Pit), 0));
        assert!(c.stints[1].open && c.stints[1].compound == 1);
        assert!((c.stints[1].laps() - 7.2).abs() < 0.1, "{:?}", c.stints[1]);
    }

    #[test]
    fn retiring_closes_the_stint_and_joining_late_is_partial() {
        let mut t = StintTracker::default();
        let mut f = frame();
        at(&mut f, 8.2, false, 0);
        t.update(&f);
        at(&mut f, 9.5, false, 0);
        t.update(&f);
        f.car_track_surface[0] = track_surface::NOT_IN_WORLD;
        f.car_lap_dist_pct[0] = -1.0;
        t.update(&f);
        let c = &t.payload()[0];
        assert!(c.out);
        assert!(c.stints[0].partial, "first seen on lap 8: we missed the start of it");
        assert_eq!(c.stints[0].ended_by, Some(StintEnd::Out));
        assert!((c.stints[0].laps() - 1.3).abs() < 1e-3);
    }

    #[test]
    fn new_session_starts_afresh() {
        let mut t = StintTracker::default();
        let mut f = frame();
        at(&mut f, 3.0, false, 0);
        t.update(&f);
        f.session_num = 2;
        at(&mut f, 0.1, false, 0);
        t.update(&f);
        let c = &t.payload()[0];
        assert_eq!(c.stints.len(), 1);
        assert!(c.stints[0].start < 0.2);
    }
}
