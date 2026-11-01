//! Weather maths from `src/lib/weather.ts`: compass points, the wind
//! relative to the car, and samples with trends over the last few minutes.
//!
//! Angles are compass bearings in radians, clockwise from north. iRacing's
//! WindDir is the direction the wind blows *from*, at the start/finish line.

#[cfg(test)]
use std::f64::consts::TAU;

pub const SKIES: [&str; 4] = ["Clear", "Partly cloudy", "Mostly cloudy", "Overcast"];
/// irsdk_TrackWetness, 1..7 (0 = unknown).
pub const WETNESS: [&str; 8] = ["", "Dry", "Mostly dry", "Very lightly wet", "Lightly wet", "Moderately wet", "Very wet", "Extremely wet"];
#[cfg(test)]
const COMPASS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

#[cfg(test)]
pub fn wrap(a: f64) -> f64 {
    ((a % TAU) + TAU) % TAU
}

/// "SW" for a bearing (the compass draws only N, E, S and W; kept for parity).
#[cfg(test)]
pub fn compass_point(rad: f64) -> &'static str {
    COMPASS[((wrap(rad).to_degrees() / 45.0).round() as usize) % 8]
}

/// Where the wind comes from, seen from the driver's seat: 0 = dead ahead, π/2 = from the right.
#[cfg(test)]
pub fn wind_from_car(wind_dir: f64, heading: f64) -> f64 {
    wrap(wind_dir - heading)
}

/// The wind split along and across the car: `head` is positive for a
/// headwind, `cross` positive when it comes from the right.
#[cfg(test)]
pub fn wind_components(speed: f64, wind_dir: f64, heading: f64) -> (f64, f64) {
    let rel = wind_from_car(wind_dir, heading);
    (speed * rel.cos(), speed * rel.sin())
}

/// A reading worth watching for change, sampled now and then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// ms of session time
    pub at: f64,
    pub value: f64,
}

pub const EVERY_MS: f64 = 15_000.0;
pub const KEEP_MS: f64 = 4.0 * 60_000.0;

/// Keeps one sample every `every_ms`, for `keep_ms`.
pub fn add_sample(list: &mut Vec<Sample>, at: f64, value: f64, every_ms: f64, keep_ms: f64) {
    if let Some(last) = list.last() {
        if at - last.at < every_ms && at >= last.at {
            return;
        }
        // Time went backwards (new session, replay): start again.
        if at < last.at {
            list.clear();
        } else {
            list.retain(|s| at - s.at <= keep_ms);
        }
    }
    list.push(Sample { at, value });
}

/// Which way a reading has moved over the last few minutes: +1 rising, -1
/// falling, 0 steady or not enough history. Compares the latest sample with
/// the oldest one inside `window_ms`, which needs to span at least
/// `min_span_ms`, and the change must be bigger than `threshold`.
pub fn trend(list: &[Sample], threshold: f64, window_ms: f64, min_span_ms: f64) -> i32 {
    let Some(last) = list.last() else { return 0 };
    let Some(first) = list.iter().find(|s| last.at - s.at <= window_ms) else { return 0 };
    if last.at - first.at < min_span_ms {
        return 0;
    }
    let d = last.value - first.value;
    if d > threshold {
        1
    } else if d < -threshold {
        -1
    } else {
        0
    }
}

/// `trend` over the usual three minutes, needing ninety seconds of history.
pub fn trend3(list: &[Sample], threshold: f64) -> i32 {
    trend(list, threshold, 3.0 * 60_000.0, 90_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn compass_points() {
        assert_eq!(compass_point(0.0), "N");
        assert_eq!(compass_point(PI / 4.0), "NE");
        assert_eq!(compass_point(PI / 2.0), "E");
        assert_eq!(compass_point(PI), "S");
        assert_eq!(compass_point(5.0 * PI / 4.0), "SW");
        assert_eq!(compass_point(-PI / 2.0), "W");
        assert_eq!(compass_point(TAU - 0.1), "N");
        assert_eq!(compass_point(7.0 * PI / 4.0), "NW");
    }

    #[test]
    fn wind_relative_to_the_car() {
        // Wind from the north, car heading north: dead ahead.
        assert!(wind_from_car(0.0, 0.0).abs() < 1e-9);
        // Car heading east, wind from the north: from the left (3π/2).
        assert!((wind_from_car(0.0, PI / 2.0) - 3.0 * PI / 2.0).abs() < 1e-9);
        // Car heading west, wind from the north: from the right.
        let (head, cross) = wind_components(10.0, 0.0, 3.0 * PI / 2.0);
        assert!(head.abs() < 1e-9 && (cross - 10.0).abs() < 1e-9);
        // Car heading south into a northerly: a tailwind.
        let (head, _) = wind_components(10.0, 0.0, PI);
        assert!((head + 10.0).abs() < 1e-9);
    }

    #[test]
    fn samples_every_fifteen_seconds() {
        let mut l = Vec::new();
        add_sample(&mut l, 0.0, 1.0, EVERY_MS, KEEP_MS);
        add_sample(&mut l, 5_000.0, 2.0, EVERY_MS, KEEP_MS);
        assert_eq!(l.len(), 1);
        add_sample(&mut l, 15_000.0, 2.0, EVERY_MS, KEEP_MS);
        assert_eq!(l.len(), 2);
        // Old samples fall off after four minutes.
        add_sample(&mut l, 300_000.0, 3.0, EVERY_MS, KEEP_MS);
        assert_eq!(l.len(), 1);
        // Time going backwards starts again.
        add_sample(&mut l, 1_000.0, 4.0, EVERY_MS, KEEP_MS);
        assert_eq!(l, vec![Sample { at: 1_000.0, value: 4.0 }]);
    }

    #[test]
    fn trends() {
        let mut l = Vec::new();
        assert_eq!(trend3(&l, 0.5), 0);
        for k in 0..=12 {
            add_sample(&mut l, k as f64 * 15_000.0, 20.0 + k as f64 * 0.1, EVERY_MS, KEEP_MS);
        }
        // 3 min: +1.2 over the window.
        assert_eq!(trend3(&l, 0.5), 1);
        assert_eq!(trend3(&l, 2.0), 0);
        // Falling.
        let mut f = Vec::new();
        for k in 0..=12 {
            add_sample(&mut f, k as f64 * 15_000.0, 20.0 - k as f64 * 0.1, EVERY_MS, KEEP_MS);
        }
        assert_eq!(trend3(&f, 0.5), -1);
        // Too little history (under 90 s).
        let short = [Sample { at: 0.0, value: 0.0 }, Sample { at: 60_000.0, value: 10.0 }];
        assert_eq!(trend3(&short, 0.5), 0);
    }
}
