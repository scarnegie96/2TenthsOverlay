//! Every car's gap to you on track (the Relative's: seconds, positive ahead of
//! you) recorded through the session, for Race pace's "Relative to you" view.
//! Each car's line is kept whether it's near you or not, so whoever becomes your
//! neighbour on the road already has its history. The cars shown are the
//! nearest on the road either side of you right now (`nearest`), picked ten
//! times a second like the Relative and Battle.

use std::collections::HashMap;

use telemetry::model::{CarStatus, RelativeRow};

/// Seconds between stored points to start with; doubled (and every car's points halved) whenever
/// a car has more than `MAX_POINTS`, so a long race still fits.
const START_EVERY_S: f64 = 1.0;
const MAX_POINTS: usize = 2000;
/// A line breaks where a car went unrecorded for this many sample intervals (the garage, a tow),
/// or where its gap jumped by more than `JUMP_S` between two points: the Relative's gap wraps round
/// at half a lap, so a car that far away flips from ahead to behind.
const HOLE: f64 = 3.0;
const JUMP_S: f32 = 5.0;
/// The ends of a change over a lap are each averaged over this many seconds either side.
const SMOOTH_S: f64 = 1.5;

#[derive(Debug, Default)]
pub struct RelGaps {
    key: String,
    last: f64,
    every: f64,
    /// Per car: (session time, gap to you, positive ahead).
    cars: HashMap<i32, Vec<(f64, f32)>>,
}

impl RelGaps {
    /// Records the Relative at session time `t` (`key` names the session; a new one starts afresh).
    pub fn push(&mut self, rows: &[RelativeRow], t: f64, key: &str) {
        if key != self.key || t < self.last - 1.0 {
            *self = Self { key: key.to_string(), last: f64::NEG_INFINITY, every: START_EVERY_S, ..Self::default() };
        }
        if t - self.last < self.every {
            return;
        }
        self.last = t;
        for r in rows.iter().filter(|r| !r.is_player && r.status != CarStatus::Out) {
            self.cars.entry(r.driver.car_idx).or_default().push((t, r.gap));
        }
        if self.cars.values().any(|p| p.len() > MAX_POINTS) {
            for p in self.cars.values_mut() {
                let mut k = 0;
                p.retain(|_| {
                    k += 1;
                    k % 2 == 1
                });
            }
            self.every *= 2.0;
        }
    }

    /// Car `car`'s line from session time `from` on, as unbroken runs of (time, gap) points.
    pub fn runs(&self, car: i32, from: f64) -> Vec<Vec<(f64, f32)>> {
        let mut out: Vec<Vec<(f64, f32)>> = Vec::new();
        let Some(points) = self.cars.get(&car) else { return out };
        let mut prev: Option<(f64, f32)> = None;
        for &(t, g) in points.iter().filter(|(t, _)| *t >= from) {
            let broken = prev.is_none_or(|(pt, pg)| t - pt > self.every * HOLE || (g - pg).abs() > JUMP_S);
            if broken {
                out.push(Vec::new());
            }
            if let Some(run) = out.last_mut() {
                run.push((t, g));
            }
            prev = Some((t, g));
        }
        out
    }

    /// How much you've gained on car `car` over the last `lap_time` seconds: its gap a lap ago minus
    /// its gap now (`gap_now`), so positive when it fell back relative to you (the car ahead coming
    /// back to you, or the one behind dropping away). None without an unbroken lap of its line.
    pub fn gained(&self, car: i32, gap_now: f32, now: f64, lap_time: f32) -> Option<f32> {
        if lap_time.is_nan() || lap_time <= 1.0 {
            return None;
        }
        let then = now - lap_time as f64;
        let run = self.runs(car, then - SMOOTH_S - self.every).pop()?;
        // The run must reach back a lap and up to now.
        if run.first()?.0 > then || now - run.last()?.0 > self.every * HOLE {
            return None;
        }
        let near: Vec<f32> = run.iter().filter(|(t, _)| (t - then).abs() <= SMOOTH_S.max(self.every)).map(|(_, g)| *g).collect();
        if near.is_empty() {
            return None;
        }
        let gap_then = near.iter().sum::<f32>() / near.len() as f32;
        Some(gap_then - gap_now)
    }
}

/// The `n` nearest cars on the road ahead of you (nearest first) and behind you (nearest first),
/// from the Relative. Cars on pit road are passed over unless you're on it too; cars out of the
/// sim always.
pub fn nearest(rows: &[RelativeRow], n: usize) -> (Vec<&RelativeRow>, Vec<&RelativeRow>) {
    let in_pits = rows.iter().any(|r| r.is_player && r.on_pit_road);
    let others = || rows.iter().filter(|r| !r.is_player && r.status != CarStatus::Out && (in_pits || !r.on_pit_road));
    let mut ahead: Vec<&RelativeRow> = others().filter(|r| r.gap >= 0.0).collect();
    ahead.sort_by(|a, b| a.gap.total_cmp(&b.gap));
    ahead.truncate(n);
    let mut behind: Vec<&RelativeRow> = others().filter(|r| r.gap < 0.0).collect();
    behind.sort_by(|a, b| b.gap.total_cmp(&a.gap));
    behind.truncate(n);
    (ahead, behind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use telemetry::model::{DriverCell, LapInfo};

    fn row(car: i32, gap: f32, player: bool) -> RelativeRow {
        RelativeRow {
            driver: DriverCell {
                car_idx: car,
                name: format!("Car {car}"),
                abbrev: String::new(),
                number: car.to_string(),
                class_id: 1,
                class_name: String::new(),
                class_color: String::new(),
                irating: 0,
                user_id: car as i64,
                license: String::new(),
                license_color: String::new(),
            },
            position: 0,
            class_position: 0,
            gap,
            lap_status: 0,
            laps: LapInfo::default(),
            on_pit_road: false,
            status: CarStatus::Track,
            is_player: player,
            compound: -1,
        }
    }

    #[test]
    fn nearest_on_the_road_either_side() {
        let mut rows = vec![row(1, 20.0, false), row(2, 4.0, false), row(3, 1.0, false), row(0, 0.0, true), row(4, -0.5, false), row(5, -7.0, false), row(6, -30.0, false)];
        let (a, b) = nearest(&rows, 2);
        let ids = |v: &[&RelativeRow]| v.iter().map(|r| r.driver.car_idx).collect::<Vec<_>>();
        assert_eq!((ids(&a), ids(&b)), (vec![3, 2], vec![4, 5]));
        // Car 3 pits: the next one up the road comes in.
        rows[2].on_pit_road = true;
        assert_eq!(ids(&nearest(&rows, 2).0), vec![2, 1]);
        // Fewer cars than asked for: as many as there are.
        assert_eq!(ids(&nearest(&rows, 5).1), vec![4, 5, 6]);
    }

    /// Car 4's gap to you is `gap(t)`, pushed every 0.1 s (as the field arrives) up to `until`.
    fn record(gap: impl Fn(f64) -> f32, until: f64) -> RelGaps {
        let mut h = RelGaps::default();
        let mut k = 0;
        loop {
            let t = k as f64 / 10.0;
            if t > until {
                break;
            }
            h.push(&[row(0, 0.0, true), row(4, gap(t), false)], t, "s");
            k += 1;
        }
        h
    }

    #[test]
    fn what_you_gained_over_a_lap() {
        // 5 s ahead and coming back 0.3 s a lap (100 s laps).
        let h = record(|t| (5.0 - 0.003 * t) as f32, 500.0);
        let g = h.gained(4, 3.5, 500.0, 100.0).unwrap();
        assert!((g - 0.3).abs() < 0.01, "{g}");
        // Not a lap of it yet.
        assert!(record(|_| 3.0, 60.0).gained(4, 3.0, 60.0, 100.0).is_none());
        // A car you passed counts the whole swing: 1 s ahead a lap ago, 2 s behind now.
        let h = record(|t| (1.0 - 0.03 * t) as f32, 100.0);
        let g = h.gained(4, -2.0, 100.0, 100.0).unwrap();
        assert!((g - 3.0).abs() < 0.05, "{g}");
    }

    #[test]
    fn lines_break_where_the_gap_wraps_or_the_car_was_gone() {
        // Half a lap away the Relative's gap flips from +45 to -45.
        let h = record(|t| if t < 50.0 { 44.0 } else { -44.0 }, 100.0);
        assert_eq!(h.runs(4, 0.0).len(), 2);
        // Gone for 20 s.
        let mut h = record(|_| 2.0, 30.0);
        h.push(&[row(0, 0.0, true), row(4, 2.0, false)], 50.0, "s");
        assert_eq!(h.runs(4, 0.0).len(), 2);
        assert!(h.gained(4, 2.0, 50.0, 40.0).is_none(), "not unbroken");
    }

    #[test]
    fn a_long_race_fits() {
        let h = record(|_| 1.0, 6000.0);
        let n: usize = h.runs(4, 0.0).iter().map(|r| r.len()).sum();
        assert!(n <= MAX_POINTS && n > MAX_POINTS / 4, "{n}");
        assert_eq!(h.runs(4, 0.0).len(), 1, "thinning doesn't break the line");
    }
}
