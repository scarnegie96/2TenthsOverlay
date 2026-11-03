//! Your own laps this session, kept so the Delta widget has something to compare against when
//! iRacing doesn't publish its `LapDeltaTo…` variables (their `_OK` flags are off, as seen in races).
//! Each clean lap is stored as the time into the lap at every 0.1% (like an imported reference
//! lap, `reference.rs`); the best and the last fill in `Deltas::best`, `session_best` and `last`
//! wherever iRacing's are missing.

use crate::model::RawFrame;
use crate::reference::REF_BINS;

/// Share of the lap's 0.1% steps that must have been seen for the lap to count.
const MIN_COVERAGE: f32 = 0.9;
/// A forward jump bigger than this in one frame is a reset or a tow, not driving.
const MAX_STEP: f32 = 0.05;

#[derive(Debug, Default)]
pub struct OwnLaps {
    session_num: Option<i32>,
    /// iRacing's lap counter for the lap being recorded.
    lap: Option<i32>,
    /// Seconds into the lap per 0.1% step (NaN where not seen yet). Empty until a lap starts at the line.
    times: Vec<f32>,
    /// Nothing has spoiled this lap so far (pits, off the world, a reset).
    clean: bool,
    /// The last frame: lap fraction, time into the lap, session time.
    prev: Option<(f32, f32, f64)>,
    best: Option<Lap>,
    last: Option<Lap>,
}

#[derive(Debug, Clone)]
struct Lap {
    time: f32,
    /// Seconds into the lap at `k / REF_BINS`, REF_BINS + 1 long.
    times: Vec<f32>,
}

impl Lap {
    fn time_at(&self, pct: f32) -> f32 {
        let x = pct.clamp(0.0, 1.0) * REF_BINS as f32;
        let i = (x.floor() as usize).min(REF_BINS - 1);
        self.times[i] + (self.times[i + 1] - self.times[i]) * (x - i as f32)
    }
}

/// The deltas worked out from your own laps: positive = slower.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OwnDeltas {
    pub best: Option<f32>,
    pub last: Option<f32>,
}

impl OwnLaps {
    /// Call every frame. Returns the deltas against your best and last clean laps this session.
    pub fn update(&mut self, f: &RawFrame) -> OwnDeltas {
        if self.session_num != Some(f.session_num) {
            *self = Self { session_num: Some(f.session_num), ..Self::default() };
        }
        let pct = f.lap_dist_pct;
        let driving = f.is_on_track && pct >= 0.0;
        if !driving {
            // Out of the car or off the world: this lap can't count, and there's nothing to compare.
            self.clean = false;
            self.prev = None;
            return OwnDeltas::default();
        }

        let new_lap = self.lap != Some(f.lap);
        if new_lap {
            // Crossed the line (or first seen): finish the lap that was running, start the next.
            if let (Some((p0, t0, s0)), true) = (self.prev, self.lap.is_some()) {
                self.finish(p0, t0, s0, pct, f.session_time);
            }
            self.lap = Some(f.lap);
            // Only a lap we see start at the line can be timed from it.
            let at_line = pct < 0.02 && self.prev.is_some();
            self.times = if at_line { vec![f32::NAN; REF_BINS + 1] } else { Vec::new() };
            self.clean = at_line;
        } else if let Some((p0, _, _)) = self.prev {
            let step = pct - p0;
            if !(-0.01..=MAX_STEP).contains(&step) {
                self.clean = false;
            }
        }
        if f.on_pit_road {
            self.clean = false;
        }
        if self.clean && !self.times.is_empty() {
            let n = REF_BINS as f32;
            match self.prev.filter(|&(p0, _, _)| !new_lap && pct > p0) {
                // Each 0.1% step passed since the last frame, at the time it was passed (in between).
                Some((p0, t0, _)) => {
                    let first = ((p0 * n).floor() as usize + 1).min(REF_BINS);
                    let last = ((pct * n).floor() as usize).min(REF_BINS);
                    for k in first..=last {
                        if self.times[k].is_nan() {
                            let at = (k as f32 / n - p0) / (pct - p0);
                            self.times[k] = t0 + (f.lap_current - t0) * at;
                        }
                    }
                }
                None => {
                    let k = ((pct * n).floor() as usize).min(REF_BINS);
                    if self.times[k].is_nan() {
                        self.times[k] = f.lap_current;
                    }
                }
            }
        }
        self.prev = Some((pct, f.lap_current, f.session_time));

        let delta = |l: &Option<Lap>| l.as_ref().map(|l| f.lap_current - l.time_at(pct));
        OwnDeltas { best: delta(&self.best), last: delta(&self.last) }
    }

    /// The lap ended between the last frame (lap fraction `p0`, `t0` into the lap, session time
    /// `s0`) and this one (`p1` into the next lap at session time `s1`).
    fn finish(&mut self, p0: f32, t0: f32, s0: f64, p1: f32, s1: f64) {
        let times = std::mem::take(&mut self.times);
        if !self.clean || times.is_empty() || p0 < 0.9 || p1 > 0.1 {
            return;
        }
        // When it crossed the line, by where it was each side of it.
        let gone = (1.0 - p0).max(0.0);
        let share = if gone + p1 > 0.0 { gone / (gone + p1) } else { 0.0 };
        let time = t0 + ((s1 - s0).max(0.0) as f32) * share;
        if let Some(lap) = complete(times, time) {
            if self.best.as_ref().is_none_or(|b| lap.time < b.time) {
                self.best = Some(lap.clone());
            }
            self.last = Some(lap);
        }
    }
}

/// Fills the steps between the ones seen (straight lines), pinned to 0 at the line and `time` at
/// the end. None when too little of the lap was seen or the times don't run forward.
fn complete(mut times: Vec<f32>, time: f32) -> Option<Lap> {
    let seen = times.iter().filter(|t| !t.is_nan()).count();
    if (seen as f32) < MIN_COVERAGE * REF_BINS as f32 || time <= 1.0 {
        return None;
    }
    times[0] = 0.0;
    times[REF_BINS] = time;
    let mut last = 0;
    for k in 1..=REF_BINS {
        if times[k].is_nan() {
            continue;
        }
        if times[k] < times[last] {
            return None;
        }
        for j in last + 1..k {
            times[j] = times[last] + (times[k] - times[last]) * (j - last) as f32 / (k - last) as f32;
        }
        last = k;
    }
    Some(Lap { time, times })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drives from half a second before the line, then a 90 s lap, then 100 s laps, at 60 frames
    /// a second, until session time `until`; `pits` puts the car on pit road throughout.
    fn drive(until: f64, pits: bool) -> (OwnLaps, RawFrame, OwnDeltas) {
        let mut o = OwnLaps::default();
        let mut f = RawFrame::with_capacity();
        f.is_on_track = true;
        f.on_pit_road = pits;
        let mut d = OwnDeltas::default();
        let mut k = 0;
        loop {
            let t = k as f64 / 60.0;
            if t > until {
                break;
            }
            let (lap, into, len) = if t < 0.5 {
                (1, 89.5 + t, 90.0)
            } else if t < 90.5 {
                (2, t - 0.5, 90.0)
            } else {
                let n = ((t - 90.5) / 100.0).floor();
                (3 + n as i32, t - 90.5 - n * 100.0, 100.0)
            };
            f.lap = lap;
            f.lap_current = into as f32;
            f.lap_dist_pct = (into / len) as f32;
            f.session_time = t;
            d = o.update(&f);
            k += 1;
        }
        (o, f, d)
    }

    #[test]
    fn nothing_to_compare_before_a_whole_lap() {
        let (o, _, d) = drive(60.0, false);
        assert!(o.best.is_none());
        assert_eq!(d, OwnDeltas::default());
    }

    #[test]
    fn deltas_against_your_own_best_and_last_laps() {
        // Half way round the 100 s lap: 50 s in, where the 90 s lap took 45 s.
        let (o, _, d) = drive(140.5, false);
        let best = o.best.as_ref().expect("the 90 s lap counts");
        assert!((best.time - 90.0).abs() < 0.05, "{}", best.time);
        assert!((d.best.unwrap() - 5.0).abs() < 0.05, "{d:?}");
        assert!((d.last.unwrap() - 5.0).abs() < 0.05, "{d:?}");
        // A lap later the last lap is the 100 s one, but the best is still the 90 s one.
        let (o, _, d) = drive(240.5, false);
        assert!((o.last.as_ref().unwrap().time - 100.0).abs() < 0.05);
        assert!((d.best.unwrap() - 5.0).abs() < 0.05, "{d:?}");
        assert!(d.last.unwrap().abs() < 0.05, "{d:?}");
    }

    #[test]
    fn a_lap_through_the_pits_does_not_count() {
        let (o, _, d) = drive(140.5, true);
        assert!(o.best.is_none());
        assert_eq!(d, OwnDeltas::default());
    }
}
