//! A comparison lap, stored as "time elapsed at each point round the lap".
//!
//! Live delta against it is simply: our lap time so far minus the
//! reference's time at the same lap position. Positive = we're slower.

use serde::{Deserialize, Serialize};

/// Resolution of the stored profile: one point per 0.1% of the lap.
pub const REF_BINS: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceLap {
    pub track_name: String,
    pub track_config: String,
    pub car_name: String,
    pub driver: String,
    pub lap_number: i32,
    pub lap_time: f32,
    /// File it came from, for display.
    pub source: String,
    /// `times[k]` = seconds into the lap at `k / REF_BINS` of the lap. Length REF_BINS + 1.
    pub times: Vec<f32>,
}

impl ReferenceLap {
    /// Builds a profile from (lap fraction, seconds since lap start) samples.
    /// The samples must start at 0 and end at 1 (callers add the line crossings).
    pub fn from_samples(samples: &[(f32, f32)]) -> Option<Vec<f32>> {
        // Keep only strictly increasing positions; drops resets and jitter.
        let mut pts: Vec<(f32, f32)> = Vec::with_capacity(samples.len());
        for &(p, t) in samples {
            if pts.last().is_none_or(|&(lp, lt)| p > lp && t >= lt) {
                pts.push((p, t));
            }
        }
        if pts.len() < 20 || pts[0].0 > 0.001 || pts[pts.len() - 1].0 < 0.999 {
            return None;
        }
        let mut times = Vec::with_capacity(REF_BINS + 1);
        let mut j = 0;
        for k in 0..=REF_BINS {
            let p = k as f32 / REF_BINS as f32;
            while j + 2 < pts.len() && pts[j + 1].0 < p {
                j += 1;
            }
            let (p0, t0) = pts[j];
            let (p1, t1) = pts[(j + 1).min(pts.len() - 1)];
            let t = if p1 > p0 { t0 + (p - p0) / (p1 - p0) * (t1 - t0) } else { t0 };
            times.push(t);
        }
        Some(times)
    }

    /// Seconds into the reference lap at lap fraction `pct`.
    pub fn time_at(&self, pct: f32) -> f32 {
        let x = pct.clamp(0.0, 1.0) * REF_BINS as f32;
        let i = (x.floor() as usize).min(REF_BINS - 1);
        let f = x - i as f32;
        self.times[i] + (self.times[i + 1] - self.times[i]) * f
    }

    /// Our time so far minus the reference's at the same point. Positive = slower.
    pub fn delta(&self, pct: f32, lap_current: f32) -> f32 {
        lap_current - self.time_at(pct)
    }

    pub fn matches(&self, track_name: &str, track_config: &str) -> bool {
        self.track_name.eq_ignore_ascii_case(track_name) && self.track_config.eq_ignore_ascii_case(track_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_profile() {
        // A 100 s lap driven at constant pace.
        let samples: Vec<(f32, f32)> = (0..=200).map(|i| (i as f32 / 200.0, i as f32 / 2.0)).collect();
        let times = ReferenceLap::from_samples(&samples).unwrap();
        let r = ReferenceLap {
            track_name: "Ring".into(),
            track_config: "GP".into(),
            car_name: String::new(),
            driver: String::new(),
            lap_number: 3,
            lap_time: 100.0,
            source: String::new(),
            times,
        };
        assert!((r.time_at(0.25) - 25.0).abs() < 1e-3);
        assert!((r.time_at(1.0) - 100.0).abs() < 1e-3);
        // At 50% after 49.5 s we're 0.5 s up.
        assert!((r.delta(0.5, 49.5) + 0.5).abs() < 1e-3);
        assert!(r.matches("ring", "gp"));
    }
}
