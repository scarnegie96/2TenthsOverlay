//! How much time a pit stop costs at this track, learned from the stops we
//! see this session.
//!
//! A stop's loss is the time from entering the pit lane to leaving it, minus
//! the time cars of that class normally take to cover the same stretch of
//! track at racing speed. The racing time comes from a per-class profile of
//! `CarIdxEstTime` (iRacing's estimate of how long a car of that class takes
//! to reach each point of the lap), learned from every car on track.
//!
//! Only real stops count: the car must stop in its pit box. Drive-throughs,
//! tows (leaving the world), stops with long repairs and other outliers are
//! left out. Nothing is reported for a class until it has `MIN_STOPS` stops,
//! so the Standings shows no rejoin estimate until there's a session average.

use std::collections::HashMap;

use serde::Serialize;

use crate::model::{track_surface, RawFrame, MAX_CARS};

/// Stops needed in a class before its average is used.
pub const MIN_STOPS: usize = 2;
/// Profile resolution: points round the lap.
const BINS: usize = 200;
/// A stop with more than this standing in the box (repairs, a penalty) isn't typical.
const MAX_STALL_S: f64 = 120.0;

/// One timed stop.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimedStop {
    pub car_idx: i32,
    pub class_id: i32,
    /// Pit entry to pit exit.
    pub lane_s: f32,
    /// Standing in the pit box.
    pub stall_s: f32,
    /// Lane time minus the racing time for the same stretch.
    pub loss_s: f32,
}

/// The average loss for one class, once there are enough stops.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PitLoss {
    pub class_id: i32,
    /// Average seconds a stop costs.
    pub loss_s: f32,
    /// Average time standing in the box.
    pub stall_s: f32,
    /// Stops the average is from.
    pub stops: u32,
}

#[derive(Debug, Clone, Copy)]
struct InLane {
    t0: f64,
    pct0: f32,
    stall: f64,
    stall_since: Option<f64>,
}

/// Seconds a class takes to reach each point of the lap.
#[derive(Debug, Clone)]
struct Profile {
    est: Vec<f32>,
    seen: Vec<bool>,
    lap: f32,
}

impl Profile {
    fn new() -> Self {
        Self { est: vec![0.0; BINS], seen: vec![false; BINS], lap: 0.0 }
    }

    fn learn(&mut self, pct: f32, est: f32) {
        if !(0.0..1.0).contains(&pct) || est <= 0.0 {
            return;
        }
        let b = ((pct * BINS as f32) as usize).min(BINS - 1);
        self.est[b] = if self.seen[b] { self.est[b] * 0.9 + est * 0.1 } else { est };
        self.seen[b] = true;
        // EstTime near the end of the lap is close to a whole lap.
        if pct > 0.98 {
            self.lap = self.lap.max(est / pct);
        }
    }

    /// Estimated time at a point, filling gaps by interpolating between known bins.
    fn at(&self, pct: f32) -> Option<f32> {
        let b = ((pct.clamp(0.0, 0.9999) * BINS as f32) as usize).min(BINS - 1);
        if self.seen[b] {
            return Some(self.est[b]);
        }
        let below = (0..b).rev().find(|&k| self.seen[k]);
        let above = (b + 1..BINS).find(|&k| self.seen[k]);
        match (below, above) {
            (Some(lo), Some(hi)) => {
                let f = (b - lo) as f32 / (hi - lo) as f32;
                Some(self.est[lo] + (self.est[hi] - self.est[lo]) * f)
            }
            _ => None,
        }
    }

    /// Racing time from one point of the lap to another, going forwards (across the line if need be).
    fn between(&self, from: f32, to: f32) -> Option<f32> {
        let a = self.at(from)?;
        let b = self.at(to)?;
        if to >= from {
            Some(b - a)
        } else if self.lap > 0.0 {
            Some(self.lap - a + b)
        } else {
            None
        }
    }
}

#[derive(Debug, Default)]
pub struct PitLossTracker {
    session_num: Option<i32>,
    profiles: HashMap<i32, Profile>,
    lanes: HashMap<i32, InLane>,
    was_in_lane: Vec<bool>,
    /// Seen on track this session, so a stop isn't confused with starting from the pits.
    seen_on_track: Vec<bool>,
    stops: Vec<TimedStop>,
}

impl PitLossTracker {
    /// `class_of` gives a car's class id.
    pub fn update(&mut self, f: &RawFrame, class_of: impl Fn(i32) -> Option<i32>) {
        if self.session_num != Some(f.session_num) {
            *self = Self {
                session_num: Some(f.session_num),
                was_in_lane: vec![false; MAX_CARS],
                seen_on_track: vec![false; MAX_CARS],
                ..Self::default()
            };
        }
        let t = f.session_time;
        let n = f.car_track_surface.len().min(MAX_CARS);
        for i in 0..n {
            let car = i as i32;
            let Some(class) = class_of(car) else { continue };
            let surface = f.car_track_surface[i];
            let pct = f.car_lap_dist_pct.get(i).copied().unwrap_or(-1.0);
            let on_road = f.car_on_pit_road.get(i).copied().unwrap_or(false);
            let out = surface == track_surface::NOT_IN_WORLD || pct < 0.0;
            let in_lane = !out && (on_road || surface == track_surface::IN_PIT_STALL);
            let was = std::mem::replace(&mut self.was_in_lane[i], in_lane);

            if out {
                // A tow or leaving the session: not a stop we can time.
                self.lanes.remove(&car);
                continue;
            }
            if !in_lane && surface == track_surface::ON_TRACK {
                self.seen_on_track[i] = true;
                if let Some(est) = f.car_est_time.get(i) {
                    self.profiles.entry(class).or_insert_with(Profile::new).learn(pct, *est);
                }
            }
            match (was, in_lane) {
                // Into the lane from the track (not starting from the pits).
                (false, true) if self.seen_on_track[i] => {
                    self.lanes.insert(car, InLane { t0: t, pct0: pct, stall: 0.0, stall_since: None });
                }
                (true, true) => {
                    if let Some(l) = self.lanes.get_mut(&car) {
                        let stalled = surface == track_surface::IN_PIT_STALL;
                        match (stalled, l.stall_since) {
                            (true, None) => l.stall_since = Some(t),
                            (false, Some(s0)) => {
                                l.stall += t - s0;
                                l.stall_since = None;
                            }
                            _ => {}
                        }
                    }
                }
                (true, false) => {
                    if let Some(mut l) = self.lanes.remove(&car) {
                        if let Some(s0) = l.stall_since {
                            l.stall += t - s0;
                        }
                        self.finish(car, class, l, t, pct);
                    }
                }
                _ => {}
            }
        }
    }

    fn finish(&mut self, car: i32, class: i32, l: InLane, t: f64, pct1: f32) {
        let lane = t - l.t0;
        // Drive-throughs, very long stops and silly values aren't typical stops.
        if l.stall < 0.5 || l.stall > MAX_STALL_S || !(3.0..400.0).contains(&lane) {
            return;
        }
        let Some(racing) = self.profiles.get(&class).and_then(|p| p.between(l.pct0, pct1)) else { return };
        let loss = lane as f32 - racing;
        if !(0.0..300.0).contains(&loss) {
            return;
        }
        self.stops.push(TimedStop { car_idx: car, class_id: class, lane_s: lane as f32, stall_s: l.stall as f32, loss_s: loss });
    }

    /// Average loss per class, for classes with at least `MIN_STOPS` stops.
    /// Stops more than half as long again as the median are left out.
    pub fn payload(&self) -> Vec<PitLoss> {
        let mut classes: Vec<i32> = self.stops.iter().map(|s| s.class_id).collect();
        classes.sort();
        classes.dedup();
        classes
            .into_iter()
            .filter_map(|c| {
                let mut v: Vec<&TimedStop> = self.stops.iter().filter(|s| s.class_id == c).collect();
                if v.len() < MIN_STOPS {
                    return None;
                }
                v.sort_by(|a, b| a.loss_s.total_cmp(&b.loss_s));
                let median = v[v.len() / 2].loss_s;
                let kept: Vec<&&TimedStop> = v.iter().filter(|s| s.loss_s <= median * 1.5).collect();
                if kept.len() < MIN_STOPS {
                    return None;
                }
                let n = kept.len() as f32;
                Some(PitLoss {
                    class_id: c,
                    loss_s: kept.iter().map(|s| s.loss_s).sum::<f32>() / n,
                    stall_s: kept.iter().map(|s| s.stall_s).sum::<f32>() / n,
                    stops: kept.len() as u32,
                })
            })
            .collect()
    }

    pub fn stops(&self) -> &[TimedStop] {
        &self.stops
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 100 s lap at even pace; the pit lane runs from 95% to 3% of the next lap.
    /// Car 0 drives into the lane at a third of racing speed and stops `stall` seconds.
    fn run(t: &mut PitLossTracker, f: &mut RawFrame, clock: &mut f64, stall: f64) {
        let dt = 0.05;
        let mut pct = 0.5f32;
        let mut stalled = 0.0;
        let mut pitted = false;
        // Up to the entry at racing speed (1% of the lap per second), learning the profile.
        loop {
            let in_lane = !(0.03..0.95).contains(&pct);
            let at_box = (0.0..0.005).contains(&pct) && stalled < stall;
            f.car_lap_dist_pct[0] = pct;
            f.car_est_time[0] = pct * 100.0;
            f.car_on_pit_road[0] = in_lane;
            f.car_track_surface[0] = if at_box { track_surface::IN_PIT_STALL } else { track_surface::ON_TRACK };
            f.session_time = *clock;
            t.update(f, |_| Some(1));
            *clock += dt;
            pitted |= in_lane;
            if at_box {
                stalled += dt;
                continue;
            }
            let speed = if in_lane { 0.01 / 3.0 } else { 0.01 };
            let next = pct + speed * dt as f32;
            if pct < 0.5 && next >= 0.5 && pitted {
                break;
            }
            pct = next.fract();
        }
    }

    #[test]
    fn learns_the_loss_and_waits_for_enough_stops() {
        let mut t = PitLossTracker::default();
        let mut f = RawFrame::with_capacity();
        f.session_num = 1;
        let mut clock = 0.0;
        // A lap to learn the class's pace first.
        for k in 0..2000 {
            let pct = k as f32 / 2000.0;
            f.car_lap_dist_pct[1] = pct;
            f.car_est_time[1] = pct * 100.0;
            f.car_track_surface[1] = track_surface::ON_TRACK;
            f.car_lap_dist_pct[0] = -1.0;
            f.car_track_surface[0] = track_surface::NOT_IN_WORLD;
            f.session_time = clock;
            t.update(&f, |_| Some(1));
            clock += 0.05;
        }
        run(&mut t, &mut f, &mut clock, 20.0);
        assert_eq!(t.stops().len(), 1, "{:?}", t.stops());
        assert!(t.payload().is_empty(), "one stop isn't an average yet");
        run(&mut t, &mut f, &mut clock, 24.0);
        let p = t.payload();
        assert_eq!(p.len(), 1);
        // 8% of the lap at a third of the speed: 24 s instead of 8 s, so 16 s,
        // plus 20 or 24 s standing: 38 s on average.
        assert!((p[0].loss_s - 38.0).abs() < 1.0, "{p:?} {:?}", t.stops());
        assert!((p[0].stall_s - 22.0).abs() < 0.2, "{p:?}");
        assert_eq!(p[0].stops, 2);
    }

    #[test]
    fn drive_throughs_are_not_stops() {
        let mut t = PitLossTracker::default();
        let mut f = RawFrame::with_capacity();
        f.session_num = 1;
        let mut clock = 0.0;
        run(&mut t, &mut f, &mut clock, 0.0);
        assert!(t.stops().is_empty());
    }
}
