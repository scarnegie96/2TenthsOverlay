//! Records every car's gap to its class leader through a race, for the Race
//! pace chart and the Battle widget (`src/lib/raceHistory.ts`). Fed from the
//! field updates by the widgets' `observe`, so it keeps filling while they're
//! hidden. Starts afresh in a new session, or when session time goes backwards.

use std::collections::HashMap;

use telemetry::model::{CarStatus, FieldPayload, StandingRow};

/// Seconds of session time between stored points (it doubles as the race goes on).
const START_EVERY_S: f64 = 2.0;
/// Points kept per car before the history is thinned out.
const MAX_POINTS: usize = 2400;

/// One standings row, as the history needs it.
#[derive(Debug, Clone)]
pub struct Sample {
    pub car_idx: i32,
    pub name: String,
    pub abbrev: String,
    pub class_id: i32,
    pub class_color: String,
    pub is_player: bool,
    pub class_position: i32,
    pub lap: i32,
    pub gap: Option<f32>,
    pub laps_down: i32,
    pub status: CarStatus,
}

impl Sample {
    pub fn of(r: &StandingRow) -> Self {
        Self {
            car_idx: r.driver.car_idx,
            name: r.driver.name.clone(),
            abbrev: r.driver.abbrev.clone(),
            class_id: r.driver.class_id,
            class_color: r.driver.class_color.clone(),
            is_player: r.is_player,
            class_position: r.class_position,
            lap: r.lap,
            // On-track gap (moves all the time), else iRacing's timing gap (steps at timing lines).
            gap: r.live_gap.or(r.gap),
            laps_down: r.laps_down,
            status: r.status,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CarTrace {
    pub car_idx: i32,
    pub name: String,
    pub abbrev: String,
    pub class_id: i32,
    pub class_color: String,
    pub is_player: bool,
    /// Session time and gap to the class leader (s), as flat pairs: t0, g0, t1, g1...
    pub points: Vec<f32>,
    /// Latest values, drawn as the live end of the line.
    pub t: f64,
    pub gap: f32,
    pub position: i32,
    pub laps_down: i32,
    pub status: CarStatus,
    /// Gaps since the last stored point, averaged into the next one to smooth timing steps.
    sum: f64,
    count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LapMark {
    /// Session time the class leader started this lap.
    pub t: f64,
    pub lap: i32,
}

pub struct RaceHistory {
    pub cars: HashMap<i32, CarTrace>,
    /// Per class, when its leader started each lap.
    pub laps: HashMap<i32, Vec<LapMark>>,
    /// Session time of the first point.
    pub start: Option<f64>,
    pub now: f64,
    /// Bumped on every change.
    pub version: u64,
    session: String,
    every: f64,
    last_sample: f64,
    leader_lap: HashMap<i32, i32>,
}

impl Default for RaceHistory {
    fn default() -> Self {
        Self {
            cars: HashMap::new(),
            laps: HashMap::new(),
            start: None,
            now: 0.0,
            version: 0,
            session: String::new(),
            every: START_EVERY_S,
            last_sample: f64::NEG_INFINITY,
            leader_lap: HashMap::new(),
        }
    }
}

impl RaceHistory {
    pub fn reset(&mut self) {
        self.cars.clear();
        self.laps.clear();
        self.leader_lap.clear();
        self.start = None;
        self.every = START_EVERY_S;
        self.last_sample = f64::NEG_INFINITY;
        self.version += 1;
    }

    /// Takes a field payload (the session key is the track plus the session type).
    pub fn push_field(&mut self, field: &FieldPayload, session_time: f64, session_key: &str) {
        let race = field.clock.session_type.to_lowercase().contains("race");
        let rows: Vec<Sample> = field.standings.iter().filter(|r| r.position > 0).map(Sample::of).collect();
        self.push(&rows, race, session_time, session_key);
    }

    /// `rows` are the cars with a position.
    pub fn push(&mut self, rows: &[Sample], race: bool, session_time: f64, session: &str) {
        if session != self.session || session_time < self.now - 1.0 {
            self.session = session.to_string();
            self.reset();
        }
        self.now = session_time;
        if !race || rows.is_empty() {
            return;
        }
        if self.start.is_none() {
            self.start = Some(session_time);
        }

        // Lap lines: when each class leader starts a new lap.
        for r in rows.iter().filter(|r| r.class_position == 1) {
            let seen = self.leader_lap.get(&r.class_id).copied();
            if seen != Some(r.lap) {
                self.leader_lap.insert(r.class_id, r.lap);
                let marks = self.laps.entry(r.class_id).or_default();
                // The first lap seen is where the chart starts, not a real lap start.
                if seen.is_some() {
                    marks.push(LapMark { t: session_time, lap: r.lap });
                }
            }
        }

        let sample = session_time - self.last_sample >= self.every;
        if sample {
            self.last_sample = session_time;
        }
        for r in rows {
            self.update(r, session_time, sample);
        }
        self.version += 1;
    }

    fn update(&mut self, r: &Sample, t: f64, sample: bool) {
        let gap = if r.class_position == 1 { Some(0.0) } else { r.gap };
        let c = self.cars.entry(r.car_idx).or_insert_with(|| CarTrace {
            car_idx: r.car_idx,
            name: r.name.clone(),
            abbrev: r.abbrev.clone(),
            class_id: r.class_id,
            class_color: r.class_color.clone(),
            is_player: r.is_player,
            points: Vec::new(),
            t,
            gap: gap.unwrap_or(0.0),
            position: r.class_position,
            laps_down: r.laps_down,
            status: r.status,
            sum: 0.0,
            count: 0,
        });
        c.position = r.class_position;
        c.laps_down = r.laps_down;
        c.status = r.status;
        c.is_player = r.is_player;
        c.class_id = r.class_id;
        c.class_color = r.class_color.clone();
        // A car that has left the sim keeps its line up to where it stopped.
        let Some(gap) = gap else { return };
        if r.status == CarStatus::Out {
            return;
        }
        c.t = t;
        c.gap = gap;
        c.sum += gap as f64;
        c.count += 1;
        if sample {
            c.points.push(t as f32);
            c.points.push((c.sum / c.count as f64) as f32);
            c.sum = 0.0;
            c.count = 0;
            if c.points.len() / 2 > MAX_POINTS {
                self.thin();
            }
        }
    }

    /// Halves every car's points and the sample rate, so a long race fits.
    fn thin(&mut self) {
        for c in self.cars.values_mut() {
            let mut kept = Vec::with_capacity(c.points.len() / 2 + 2);
            let mut i = 0;
            while i < c.points.len() {
                kept.push(c.points[i]);
                kept.push(c.points[i + 1]);
                i += 4;
            }
            c.points = kept;
        }
        self.every *= 2.0;
    }

    /// The current sample interval (doubles each time the history is thinned).
    pub fn every(&self) -> f64 {
        self.every
    }
}

/// How much the gap to the leader changed over the leader's last lap
/// (negative = catching), or None with less than a lap of history.
pub fn lap_trend(h: &RaceHistory, c: &CarTrace) -> Option<f32> {
    let marks = h.laps.get(&c.class_id)?;
    if marks.len() < 2 {
        return None;
    }
    let lap_length = marks[marks.len() - 1].t - marks[marks.len() - 2].t;
    let then = c.t - lap_length;
    let pts = &c.points;
    let mut i = pts.len() as isize - 2;
    while i >= 0 {
        if pts[i as usize] as f64 <= then {
            return Some(c.gap - pts[i as usize + 1]);
        }
        i -= 2;
    }
    None
}

/// On the lead lap and still racing: counts towards the gap scale.
pub fn on_scale(c: &CarTrace) -> bool {
    c.laps_down == 0 && c.status != CarStatus::Out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(car: i32, class_pos: i32, lap: i32, gap: Option<f32>) -> Sample {
        Sample {
            car_idx: car,
            name: format!("Car {car}"),
            abbrev: String::new(),
            class_id: 1,
            class_color: "#fff".into(),
            is_player: car == 2,
            class_position: class_pos,
            lap,
            gap,
            laps_down: 0,
            status: CarStatus::Track,
        }
    }

    #[test]
    fn samples_every_two_seconds_and_averages() {
        let mut h = RaceHistory::default();
        for k in 0..=20 {
            let t = k as f64; // 1 Hz
            h.push(&[s(1, 1, 1, Some(0.0)), s(2, 2, 1, Some(k as f32))], true, t, "a");
        }
        let c = &h.cars[&2];
        // Samples at t = 0, 2, 4 ... 20 (the first is taken straight away).
        assert_eq!(c.points.len() / 2, 11);
        assert_eq!(c.points[0], 0.0);
        // t=2 averages the gaps seen at t=1 and t=2.
        assert_eq!(c.points[2], 2.0);
        assert!((c.points[3] - 1.5).abs() < 1e-6);
        assert_eq!(h.cars[&1].points[1], 0.0, "the leader is flat at 0");
        assert_eq!(h.start, Some(0.0));
    }

    #[test]
    fn lap_marks_skip_the_first_lap_seen() {
        let mut h = RaceHistory::default();
        h.push(&[s(1, 1, 3, Some(0.0))], true, 10.0, "a");
        assert!(h.laps[&1].is_empty());
        h.push(&[s(1, 1, 3, Some(0.0))], true, 20.0, "a");
        h.push(&[s(1, 1, 4, Some(0.0))], true, 90.0, "a");
        assert_eq!(h.laps[&1], vec![LapMark { t: 90.0, lap: 4 }]);
    }

    #[test]
    fn resets_in_a_new_session_or_when_time_goes_back() {
        let mut h = RaceHistory::default();
        h.push(&[s(1, 1, 1, Some(0.0)), s(2, 2, 1, Some(3.0))], true, 100.0, "a");
        assert_eq!(h.cars.len(), 2);
        h.push(&[s(1, 1, 1, Some(0.0))], true, 50.0, "a");
        assert_eq!(h.cars.len(), 1, "time went back");
        h.push(&[s(1, 1, 1, Some(0.0)), s(2, 2, 1, Some(3.0))], true, 60.0, "b");
        assert_eq!(h.start, Some(60.0), "new session");
    }

    #[test]
    fn records_nothing_outside_a_race() {
        let mut h = RaceHistory::default();
        h.push(&[s(1, 1, 1, Some(0.0))], false, 5.0, "a");
        assert!(h.cars.is_empty() && h.start.is_none());
        assert_eq!(h.now, 5.0);
    }

    #[test]
    fn a_car_that_left_keeps_its_line() {
        let mut h = RaceHistory::default();
        h.push(&[s(1, 1, 1, Some(0.0)), s(2, 2, 1, Some(4.0))], true, 0.0, "a");
        let mut gone = s(2, 2, 1, Some(9.0));
        gone.status = CarStatus::Out;
        h.push(&[s(1, 1, 1, Some(0.0)), gone], true, 2.0, "a");
        let c = &h.cars[&2];
        assert_eq!(c.status, CarStatus::Out);
        assert_eq!(c.gap, 4.0);
        assert_eq!(c.t, 0.0);
        assert_eq!(c.points.len(), 2);
        // No gap yet: nothing is added either.
        h.push(&[s(1, 1, 1, Some(0.0)), s(2, 2, 1, None)], true, 4.0, "a");
        assert_eq!(h.cars[&2].points.len(), 2);
    }

    #[test]
    fn thins_when_full_and_doubles_the_interval() {
        let mut h = RaceHistory::default();
        let mut t = 0.0;
        while h.every() == START_EVERY_S {
            h.push(&[s(1, 1, 1, Some(0.0)), s(2, 2, 1, Some((t / 2.0) as f32))], true, t, "a");
            t += 2.0;
            assert!(t < 10_000.0, "never thinned");
        }
        assert_eq!(h.every(), 4.0);
        let c = &h.cars[&2];
        assert!(c.points.len() / 2 <= MAX_POINTS / 2 + 1);
        // Every other pair of the old points was kept: spacing is now 4 s.
        assert_eq!(c.points[2] - c.points[0], 4.0);
    }

    #[test]
    fn lap_trend_is_the_change_over_the_leaders_last_lap() {
        let mut h = RaceHistory::default();
        // 100 s laps; car 2 loses 0.5 s a lap to the leader.
        let mut t = 0.0;
        while t <= 300.0 {
            let lap = 1 + (t / 100.0) as i32;
            h.push(&[s(1, 1, lap, Some(0.0)), s(2, 2, lap, Some(2.0 + 0.005 * t as f32))], true, t, "a");
            t += 2.0;
        }
        let tr = lap_trend(&h, &h.cars[&2]).unwrap();
        assert!((tr - 0.5).abs() < 0.02, "{tr}");
        let mut fresh = RaceHistory::default();
        fresh.push(&[s(1, 1, 1, Some(0.0)), s(2, 2, 1, Some(2.0))], true, 0.0, "a");
        assert!(lap_trend(&fresh, &fresh.cars[&2]).is_none());
    }
}
