//! Saved race reports (`crates/telemetry/src/report.rs` writes them) and what
//! the Races page works out from them: `src/lib/report.ts`, with the cases of
//! `scripts/check-report.ts` as tests.
//!
//! Everything here comes straight from recorded data: a lap time is the time
//! between two recorded line crossings, the order on a lap is the order cars
//! crossed the line, a gap is the difference between two crossing times.
//! Where something wasn't recorded (the first lap after Tenths started, a car
//! that retired), it's left out, not estimated.

use std::collections::BTreeMap;

use serde::Deserialize;
use telemetry::laps::LapRecord;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportClass {
    pub class_id: i32,
    pub name: String,
    pub color: String,
    pub cars: i32,
    pub strength_of_field: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Crossing {
    pub lap: i32,
    /// Session seconds.
    pub t: f64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportStop {
    pub lap: i32,
    pub lane_s: f32,
    pub stall_s: f32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportStint {
    pub start: f32,
    pub end: f32,
    pub open: bool,
    pub compound: i32,
    pub partial: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportCar {
    pub car_idx: i32,
    pub user_id: i64,
    pub name: String,
    pub team: String,
    pub number: String,
    pub class_id: i32,
    pub irating: i32,
    pub license: String,
    pub is_player: bool,
    pub start_position: Option<i32>,
    pub finish_position: i32,
    pub finish_class_position: i32,
    pub laps_completed: i32,
    pub finished: bool,
    pub out: bool,
    pub crossings: Vec<Crossing>,
    pub stints: Vec<ReportStint>,
    pub stops: u32,
    pub pit_stops: Vec<ReportStop>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RaceReport {
    pub version: u32,
    pub id: String,
    pub started_at: u64,
    pub saved_at: u64,
    pub track: String,
    pub track_config: String,
    pub track_length_km: f32,
    pub car: String,
    pub session_type: String,
    pub official: bool,
    pub team_racing: bool,
    pub total_laps: Option<i32>,
    pub player_car_idx: i32,
    pub player_class_id: i32,
    pub finished: bool,
    pub classes: Vec<ReportClass>,
    pub tire_compounds: Vec<String>,
    pub cars: Vec<ReportCar>,
    pub player_laps: Vec<LapRecord>,
    pub incidents: i32,
    pub incident_limit: Option<i32>,
}

impl RaceReport {
    pub fn parse(text: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("That race report can't be read: {e}"))?;
        if !v.is_object() {
            return Err("That race report can't be read: not a report.".into());
        }
        serde_json::from_value(v).map_err(|e| format!("That race report can't be read: {e}"))
    }

    pub fn me(&self) -> Option<&ReportCar> {
        self.cars.iter().find(|c| c.is_player)
    }
}

/// Lap number to seconds, for laps with both ends recorded.
pub fn lap_times(car: &ReportCar) -> BTreeMap<i32, f64> {
    let at: BTreeMap<i32, f64> = car.crossings.iter().map(|c| (c.lap, c.t)).collect();
    let mut out = BTreeMap::new();
    for c in &car.crossings {
        if let Some(prev) = at.get(&(c.lap - 1)) {
            if c.t > *prev {
                out.insert(c.lap, c.t - prev);
            }
        }
    }
    out
}

/// For each lap, the cars in the order they crossed the line to complete it.
pub fn order_by_lap(cars: &[&ReportCar]) -> BTreeMap<i32, Vec<i32>> {
    let mut by_lap: BTreeMap<i32, Vec<(i32, f64)>> = BTreeMap::new();
    for car in cars {
        for c in &car.crossings {
            by_lap.entry(c.lap).or_default().push((car.car_idx, c.t));
        }
    }
    by_lap
        .into_iter()
        .map(|(lap, mut list)| {
            list.sort_by(|a, b| a.1.total_cmp(&b.1));
            (lap, list.into_iter().map(|x| x.0).collect())
        })
        .collect()
}

/// Car index to (lap to position across the line on that lap, from 1).
pub fn positions_by_lap(cars: &[&ReportCar]) -> BTreeMap<i32, BTreeMap<i32, f64>> {
    let mut out: BTreeMap<i32, BTreeMap<i32, f64>> = cars.iter().map(|c| (c.car_idx, BTreeMap::new())).collect();
    for (lap, order) in order_by_lap(cars) {
        for (k, car) in order.into_iter().enumerate() {
            if let Some(m) = out.get_mut(&car) {
                m.insert(lap, (k + 1) as f64);
            }
        }
    }
    out
}

/// Car index to (lap to seconds behind the first of `cars` to complete that lap).
pub fn gaps_by_lap(cars: &[&ReportCar]) -> BTreeMap<i32, BTreeMap<i32, f64>> {
    let mut first: BTreeMap<i32, f64> = BTreeMap::new();
    for car in cars {
        for c in &car.crossings {
            let e = first.entry(c.lap).or_insert(f64::INFINITY);
            *e = e.min(c.t);
        }
    }
    cars.iter().map(|car| (car.car_idx, car.crossings.iter().map(|c| (c.lap, c.t - first[&c.lap])).collect())).collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LapStats {
    pub laps: usize,
    pub best: Option<f64>,
    pub median: Option<f64>,
}

/// Best and median of a set of lap times.
pub fn stats(times: &[f64]) -> LapStats {
    if times.is_empty() {
        return LapStats { laps: 0, best: None, median: None };
    }
    let mut s = times.to_vec();
    s.sort_by(f64::total_cmp);
    let mid = s.len() >> 1;
    let median = if s.len() % 2 == 1 { s[mid] } else { (s[mid - 1] + s[mid]) / 2.0 };
    LapStats { laps: s.len(), best: Some(s[0]), median: Some(median) }
}

/// Round ticks for an axis from `lo` to `hi`: whole numbers, at most `max` of them.
pub fn int_ticks(lo: f64, hi: f64, max: usize) -> Vec<f64> {
    let step = (((hi - lo) / max as f64).ceil() as i64).max(1);
    let mut out = Vec::new();
    let mut v = lo.ceil() as i64;
    while v as f64 <= hi {
        out.push(v as f64);
        v += step;
    }
    out
}

/// A stable colour for the k-th of n cars, as `colorFor` in Races.tsx (hsl 200+, 70%, 62%).
pub fn hsl_color(k: usize, n: usize) -> [u8; 3] {
    let hue = ((k as f64 * 360.0 / n.max(1) as f64 + 200.0).round() as i64 % 360) as f64;
    let (s, l): (f64, f64) = (0.70, 0.62);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [((r + m) * 255.0).round() as u8, ((g + m) * 255.0).round() as u8, ((b + m) * 255.0).round() as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(idx: i32, crossings: &[(i32, f64)]) -> ReportCar {
        ReportCar { car_idx: idx, crossings: crossings.iter().map(|&(lap, t)| Crossing { lap, t }).collect(), ..Default::default() }
    }

    // A leads; B passes A on lap 3; C starts recording at lap 2 and retires after lap 3.
    fn field() -> (ReportCar, ReportCar, ReportCar) {
        (
            car(0, &[(1, 100.0), (2, 190.0), (3, 282.0), (4, 372.0)]),
            car(1, &[(1, 101.0), (2, 191.5), (3, 281.0), (4, 370.0)]),
            car(2, &[(2, 200.0), (3, 295.0)]),
        )
    }

    #[test]
    fn lap_times_only_where_both_ends_were_recorded() {
        let (a, _, c) = field();
        let t = lap_times(&a);
        assert!(!t.contains_key(&1));
        assert_eq!(t[&2], 90.0);
        assert_eq!(t[&4], 90.0);
        // No lap time for the first recorded crossing.
        let tc = lap_times(&c);
        assert!(!tc.contains_key(&2));
        assert_eq!(tc[&3], 95.0);
    }

    #[test]
    fn order_across_the_line() {
        let (a, b, c) = field();
        assert_eq!(order_by_lap(&[&a, &b, &c])[&3], vec![1, 0, 2]);
    }

    #[test]
    fn positions_change_at_the_pass() {
        let (a, b, c) = field();
        let pos = positions_by_lap(&[&a, &b, &c]);
        assert_eq!(pos[&0][&2], 1.0);
        assert_eq!(pos[&0][&3], 2.0);
        assert_eq!(pos[&1][&3], 1.0);
        // A retired car has no position after its last lap.
        assert!(!pos[&2].contains_key(&4));
    }

    #[test]
    fn gaps_at_the_line() {
        let (a, b, c) = field();
        let gaps = gaps_by_lap(&[&a, &b, &c]);
        assert_eq!(gaps[&0][&1], 0.0);
        assert_eq!(gaps[&1][&1], 1.0);
        assert_eq!(gaps[&0][&3], 1.0);
        assert_eq!(gaps[&2][&3], 14.0);
    }

    #[test]
    fn best_and_median() {
        let s = stats(&[92.0, 90.0, 95.0, 91.0]);
        assert_eq!((s.best, s.median, s.laps), (Some(90.0), Some(91.5), 4));
        assert_eq!(stats(&[]).best, None);
        assert_eq!(stats(&[3.0, 1.0, 2.0]).median, Some(2.0));
    }

    #[test]
    fn ticks_and_colours() {
        assert_eq!(int_ticks(1.0, 5.0, 10), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(int_ticks(1.0, 30.0, 10), vec![1.0, 4.0, 7.0, 10.0, 13.0, 16.0, 19.0, 22.0, 25.0, 28.0]);
        // hsl(200 70% 62%): a light blue.
        let [r, g, b] = hsl_color(0, 4);
        assert!(b > g && g > r, "{r} {g} {b}");
    }

    #[test]
    fn parses_what_the_telemetry_writes() {
        let r = RaceReport::parse(
            r##"{"version":1,"id":"x","startedAt":5,"track":"Spa","playerCarIdx":3,"playerClassId":2,"finished":true,
            "classes":[{"classId":2,"name":"GT3","color":"#ffda59","cars":2,"strengthOfField":2400}],
            "cars":[{"carIdx":3,"isPlayer":true,"crossings":[{"lap":1,"t":10.5}],"stints":[{"start":0,"end":8.5,"open":false,"endedBy":"pit","compound":0,"partial":false}],"pitStops":[{"lap":8,"laneS":21.5,"stallS":9.0}]}],
            "playerLaps":[{"lap":1,"time":92.5,"clean":true,"pit":false,"offTrack":false,"incidents":0,"fuelUsed":2.5,"trackTemp":30.0,"airTemp":22.0,"personalBest":false,"at":1}],
            "incidents":4,"incidentLimit":17}"##,
        )
        .unwrap();
        assert_eq!(r.me().unwrap().car_idx, 3);
        assert_eq!(r.cars[0].pit_stops[0].lane_s, 21.5);
        assert_eq!(r.player_laps[0].fuel_used, Some(2.5));
        assert_eq!(r.classes[0].strength_of_field, Some(2400));
        assert!(RaceReport::parse("[1]").is_err());
    }
}
