//! The maths behind the Battle widget (`src/lib/battle.ts`): the gap to the
//! car ahead or behind in your class, and how much it changes lap by lap.
//! Gaps come from the race history: every car's gap to its class leader over
//! time, so the gap between two cars at any moment is the difference of theirs.

use telemetry::model::StandingRow;

/// A car's gap to its class leader over time, as flat pairs: t0, g0, t1, g1...
pub type Trace = [f32];

/// Closing or opening by less than this (s a lap) counts as holding steady.
pub const STEADY: f64 = 0.02;

/// The trace's gap at session time `t` (straight line between samples), or None outside it.
pub fn gap_at(points: &Trace, t: f64) -> Option<f64> {
    let n = points.len() / 2;
    if n == 0 || t < points[0] as f64 || t > points[(n - 1) * 2] as f64 {
        return None;
    }
    for i in 1..n {
        let t1 = points[i * 2] as f64;
        if t1 < t {
            continue;
        }
        let t0 = points[(i - 1) * 2] as f64;
        let g0 = points[(i - 1) * 2 + 1] as f64;
        let g1 = points[i * 2 + 1] as f64;
        return Some(if t1 == t0 { g1 } else { g0 + (g1 - g0) * (t - t0) / (t1 - t0) });
    }
    Some(points[(n - 1) * 2 + 1] as f64)
}

/// Seconds between two cars at `t`, positive when `other` is behind `me`.
/// None when either has no sample then.
pub fn interval_at(me: &Trace, other: &Trace, t: f64) -> Option<f64> {
    Some(gap_at(other, t)? - gap_at(me, t)?)
}

/// The interval averaged over the `window` seconds up to `t`. Timing only
/// updates as cars pass timing lines, so a single reading jumps about; the
/// average over part of a lap doesn't.
pub fn smooth_interval_at(me: &Trace, other: &Trace, t: f64, window: f64) -> Option<f64> {
    let steps = 8;
    let (mut sum, mut n) = (0.0, 0);
    for k in 0..=steps {
        if let Some(v) = interval_at(me, other, t - window * k as f64 / steps as f64) {
            sum += v;
            n += 1;
        }
    }
    (n > 0).then(|| sum / n as f64)
}

/// The latest moment both traces have a sample for (history is stored every couple of seconds).
fn last_shared(a: &Trace, b: &Trace, now: f64) -> f64 {
    let end = |p: &Trace| if p.len() >= 2 { p[p.len() - 2] as f64 } else { f64::NEG_INFINITY };
    now.min(end(a).min(end(b)))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trend {
    /// Seconds the gap shrinks by each lap (negative when it grows).
    pub closing_per_lap: f64,
    /// Laps the trend is measured over.
    pub laps: u32,
}

/// How fast the gap is closing, from its size now and `laps` laps ago (the
/// time of a lap given by `lap_time`), measured on what history there is.
/// None without a lap's worth of history.
pub fn trend(me: &Trace, other: &Trace, now: f64, lap_time: f64, laps: u32) -> Option<Trend> {
    if lap_time.is_nan() || lap_time <= 0.0 || laps == 0 {
        return None;
    }
    let now = last_shared(me, other, now);
    // Each end is averaged over a third of a lap, so one timing jump doesn't swing it.
    let window = lap_time / 3.0;
    let gap_now = smooth_interval_at(me, other, now, window)?;
    // Go back as many whole laps as there's history for, up to `laps`.
    let first = |p: &Trace| p.first().map(|v| *v as f64).unwrap_or(f64::INFINITY);
    let start = first(me).max(first(other)) + window;
    let have = ((now - start) / lap_time).floor();
    let n = (laps as f64).min(have);
    if n < 1.0 {
        return None;
    }
    let then = smooth_interval_at(me, other, now - n * lap_time, window)?;
    Some(Trend { closing_per_lap: (then.abs() - gap_now.abs()) / n, laps: n as u32 })
}

/// Where a car is round the race, for `laps_between`.
#[derive(Debug, Clone, Copy)]
pub struct Place {
    pub lap: i32,
    pub lap_pct: f32,
    pub laps_down: i32,
}

impl From<&StandingRow> for Place {
    fn from(r: &StandingRow) -> Self {
        Self { lap: r.lap, lap_pct: r.lap_pct, laps_down: r.laps_down }
    }
}

/// Whole laps between two cars on the road, from where each is round the lap.
/// (Not the difference of their laps down to the leader: those are each
/// rounded down, so cars a fraction of a lap apart can differ by one.)
pub fn laps_between(a: Place, b: Place) -> i32 {
    if a.lap_pct < 0.0 || b.lap_pct < 0.0 {
        return (a.laps_down - b.laps_down).abs();
    }
    ((a.lap as f64 + a.lap_pct as f64) - (b.lap as f64 + b.lap_pct as f64)).abs().floor() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    // f32 traces: the checks use a little slack on the values.
    fn near(a: Option<f64>, b: f64, eps: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() < eps)
    }

    /// Two cars over 10 laps of 100 s, sampled every 2 s: you (to the class
    /// leader) lose nothing, the car ahead of you is 5 s up and you take 0.3 s
    /// a lap off it.
    fn traces() -> (Vec<f32>, Vec<f32>) {
        let (mut me, mut ahead) = (Vec::new(), Vec::new());
        let mut t = 0;
        while t <= 1000 {
            me.extend([t as f32, 20.0]);
            ahead.extend([t as f32, (20.0 - (5.0 - 0.003 * t as f64)) as f32]);
            t += 2;
        }
        (me, ahead)
    }

    #[test]
    fn flat_trace_and_outside_it() {
        let (me, _) = traces();
        assert!(near(gap_at(&me, 500.0), 20.0, 1e-6), "flat trace");
        assert!(gap_at(&me, -1.0).is_none() && gap_at(&[], 0.0).is_none(), "outside the trace");
        assert!(near(gap_at(&[0.0, 1.0, 10.0, 2.0], 5.0), 1.5, 1e-6), "between samples");
    }

    #[test]
    fn interval_is_negative_when_in_front() {
        let (me, ahead) = traces();
        assert!(near(interval_at(&me, &ahead, 1000.0), -2.0, 1e-4), "ahead by 2 s at the end");
    }

    #[test]
    fn trend_closing_per_lap() {
        let (me, ahead) = traces();
        let t = trend(&me, &ahead, 1000.0, 100.0, 3).unwrap();
        assert!((t.closing_per_lap - 0.3).abs() < 1e-4 && t.laps == 3, "closing 0.3 s a lap, got {t:?}");
        assert!(trend(&me, &ahead, 50.0, 100.0, 3).is_none(), "no trend without a lap of history");
        assert_eq!(trend(&me, &ahead, 250.0, 100.0, 5).unwrap().laps, 2, "trend over the history there is");
    }

    #[test]
    fn steady_through_noise() {
        let (me, ahead) = traces();
        // A jumpy reading doesn't swing the trend much: +-0.5 s noise at every other sample.
        let noisy: Vec<f32> = ahead
            .iter()
            .enumerate()
            .map(|(i, v)| if i % 2 == 1 { v + if (i >> 1) % 2 == 1 { 0.5 } else { -0.5 } } else { *v })
            .collect();
        let t = trend(&me, &noisy, 1000.0, 100.0, 3).unwrap();
        assert!((t.closing_per_lap - 0.3).abs() < 0.05, "{t:?}");
    }

    #[test]
    fn needs_a_lap_time() {
        let (me, ahead) = traces();
        assert!(trend(&me, &ahead, 1000.0, 0.0, 3).is_none());
        assert!(trend(&me, &ahead, 1000.0, 100.0, 0).is_none());
    }

    #[test]
    fn laps_apart_comes_from_where_the_cars_are() {
        let row = |lap, lap_pct, laps_down| Place { lap, lap_pct, laps_down };
        // Leader 1.2 laps ahead of me and 0.9 ahead of the car in front: one lap down vs none, but 0.3 laps apart.
        assert_eq!(laps_between(row(10, 0.6, 0), row(10, 0.3, 1)), 0, "cars a fraction of a lap apart aren't a lap apart");
        assert_eq!(laps_between(row(11, 0.5, 0), row(10, 0.3, 1)), 1, "a car a lap and a bit ahead is a lap apart");
        assert_eq!(laps_between(row(10, -1.0, 0), row(9, 0.2, 2)), 2, "off track falls back to laps down");
    }
}
