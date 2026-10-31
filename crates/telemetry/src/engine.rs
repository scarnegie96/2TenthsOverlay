//! Turns raw frames into the compact payloads the widgets render.
//! All the racing logic (relative gaps, standings, fuel maths) lives here.

use std::collections::{HashMap, VecDeque};

use crate::model::*;
use crate::coach::Coach;
use crate::laps::{LapLog, LapStore};
use crate::reference::ReferenceLap;
use crate::session::{DriverInfo, SessionInfo};

const FUEL_WINDOW: usize = 5;
const LAP_TIME_WINDOW: usize = 5;

#[derive(Default)]
pub struct Engine {
    session: Option<SessionInfo>,
    fuel: FuelTracker,
    sectors: SectorTracker,
    laps: LapLog,
    /// Where to save laps; None in the demo.
    lap_store: Option<LapStore>,
    /// Lane each nearby car was given last frame, so cars don't jump sides.
    radar_sides: HashMap<i32, i32>,
    /// Imported comparison lap, if any.
    reference: Option<ReferenceLap>,
    coach: Option<Coach>,
    dynamics: crate::dynamics::DynamicsTracker,
    /// Where each car started the race, for positions gained and lost.
    grid: crate::grid::StartGrid,
    /// Every car's stints, for the Stints widget.
    stints: crate::stints::StintTracker,
    /// Times every pit stop, for the pit rejoin estimate.
    pit_loss: crate::pitloss::PitLossTracker,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_session(&mut self, info: SessionInfo) {
        self.session = Some(info);
        self.configure_laps();
    }

    pub fn set_lap_store(&mut self, store: Option<LapStore>) {
        self.lap_store = store;
        self.configure_laps();
    }

    fn configure_laps(&mut self) {
        let (track, car) = self
            .session
            .as_ref()
            .map(|s| (s.track_key.clone(), s.player().map(|p| p.car_path.clone()).unwrap_or_default()))
            .unwrap_or_default();
        self.laps.configure(self.lap_store.clone(), &track, &car);
    }

    pub fn session(&self) -> Option<&SessionInfo> {
        self.session.as_ref()
    }

    /// Clears everything learned from the current session. Keeps the reference lap and lap store.
    pub fn reset(&mut self) {
        let reference = self.reference.take();
        let coach = reference.as_ref().map(Coach::new);
        let lap_store = self.lap_store.take();
        *self = Self { reference, coach, lap_store, ..Self::default() };
    }

    pub fn set_reference(&mut self, reference: Option<ReferenceLap>) {
        self.coach = reference.as_ref().map(Coach::new);
        self.reference = reference;
    }

    fn reference_matches(&self) -> bool {
        match (&self.reference, &self.session) {
            (Some(r), Some(s)) => r.matches(&s.track_name, &s.track_config, &s.track_key),
            _ => false,
        }
    }

    pub fn reference_payload(&self) -> ReferencePayload {
        match &self.reference {
            None => ReferencePayload::default(),
            Some(r) => ReferencePayload {
                loaded: true,
                track_name: r.track_name.clone(),
                track_config: r.track_config.clone(),
                car_name: r.car_name.clone(),
                driver: r.driver.clone(),
                lap_time: r.lap_time,
                source: r.source.clone(),
                matches_track: self.reference_matches(),
                matches_car: {
                    let ours = self.session.as_ref().and_then(|s| s.player()).map(|p| p.car_path.as_str()).unwrap_or("");
                    r.car_key.is_empty() || ours.is_empty() || r.car_key.eq_ignore_ascii_case(ours)
                },
                kind: r.kind,
                has_steering: !r.steering.is_empty(),
            },
        }
    }

    pub fn session_payload(&self, frame: &RawFrame) -> SessionPayload {
        let Some(s) = &self.session else { return SessionPayload::default() };
        let player = s.player();
        SessionPayload {
            track_name: s.track_name.clone(),
            track_config: s.track_config.clone(),
            track_length_km: s.track_length_km,
            session_type: self.session_type(frame.session_num),
            player_car_idx: s.player_car_idx,
            player_class_id: player.map(|p| p.class_id).unwrap_or(0),
            car_name: player.map(|p| p.car_name.clone()).unwrap_or_default(),
            fuel_max_ltr: s.fuel_max_ltr,
            est_lap_time: s.est_lap_time,
            shift_rpm: s.shift_rpm,
            redline_rpm: s.redline_rpm,
            multiclass: s.is_multiclass(),
            tire_compounds: s.tire_compounds.clone(),
            sectors: s.sectors.clone(),
        }
    }

    fn session_type(&self, num: i32) -> String {
        self.session
            .as_ref()
            .and_then(|s| s.session(num))
            .map(|d| d.session_type.clone())
            .unwrap_or_default()
    }

    fn is_race(&self, num: i32) -> bool {
        self.session_type(num).to_ascii_lowercase().contains("race")
    }

    /// Call on every frame (60 Hz). Updates trackers and returns the fast payload.
    pub fn tick(&mut self, f: &RawFrame) -> TickPayload {
        self.fuel.update(f);
        let starts = self.session.as_ref().map(|s| s.sectors.as_slice()).unwrap_or(&[]);
        self.sectors.update(f, starts);
        self.laps.update(f);
        let race = self.is_race(f.session_num);
        self.grid.update(f, race);
        self.stints.update(f);
        if let Some(s) = &self.session {
            self.pit_loss.update(f, |car| s.driver(car).map(|d| d.class_id));
        }
        let reference_delta = self.reference_delta(f);
        let coach = match (&self.reference, &mut self.coach) {
            (Some(r), Some(c)) if reference_delta.is_some() => {
                let len = if r.length_m > 0.0 {
                    r.length_m
                } else {
                    self.session.as_ref().map(|s| s.track_length_km * 1000.0).unwrap_or(0.0)
                };
                Some(c.update(r, f, reference_delta, len))
            }
            _ => None,
        };
        TickPayload {
            session_time: f.session_time,
            is_on_track: f.is_on_track,
            on_pit_road: f.on_pit_road,
            speed: f.speed,
            rpm: f.rpm,
            shift_rpm: self.shift_rpm(f),
            gear: f.gear,
            throttle: f.throttle,
            brake: f.brake,
            clutch: f.clutch,
            steering: f.steering,
            lap: f.lap,
            lap_dist_pct: f.lap_dist_pct,
            lap_current: f.lap_current,
            lap_last: positive(f.lap_last),
            lap_best: positive(f.lap_best),
            lap_optimal: self.sectors.optimal(),
            deltas: Deltas { reference: reference_delta, ..f.deltas },
            fuel_level: f.fuel_level,
            car_left_right: f.car_left_right,
            incidents: f.incidents,
            nearby: self.nearby(f),
            coach,
            dynamics: self.dynamics.update(f),
            instruments: crate::instruments::build(f, mean(&self.fuel.per_lap)),
            heading: f.yaw_north.map(heading_from_yaw_north),
        }
    }

    /// The RPM to shift up at. iRacing's "shift" RPM is where the lights start
    /// filling and is early for most cars, so prefer where they blink (or are
    /// all lit), live for the current gear, then the session's fixed values.
    fn shift_rpm(&self, f: &RawFrame) -> Option<f32> {
        let s = self.session.as_ref();
        f.sl_blink_rpm
            .or(f.sl_last_rpm)
            .or_else(|| s.map(|s| s.blink_rpm).filter(|&r| r > 0.0))
            .or_else(|| s.map(|s| s.last_rpm).filter(|&r| r > 0.0))
            .or_else(|| s.map(|s| s.shift_rpm).filter(|&r| r > 0.0))
    }

    fn reference_delta(&self, f: &RawFrame) -> Option<f32> {
        let r = self.reference.as_ref()?;
        let usable = f.is_on_track && f.lap_current > 0.0 && (0.0..=1.0).contains(&f.lap_dist_pct) && self.reference_matches();
        usable.then(|| r.delta(f.lap_dist_pct, f.lap_current))
    }

    /// Cars within `RADAR_RANGE_M` along the track, for the radar.
    ///
    /// iRacing only tells us *that* a car is alongside (CarLeftRight), not
    /// its lateral position. So distance comes from lap position, and the
    /// overlapping cars nearest to us are assigned to the side(s) iRacing reports.
    fn nearby(&mut self, f: &RawFrame) -> Vec<NearbyCar> {
        let Some(s) = &self.session else { return Vec::new() };
        let track_m = s.track_length_km * 1000.0;
        let p = f.player_car_idx;
        if track_m <= 0.0 || !(0..MAX_CARS as i32).contains(&p) || f.lap_dist_pct < 0.0 {
            return Vec::new();
        }
        let mut cars: Vec<NearbyCar> = s
            .drivers
            .iter()
            .filter(|d| d.car_idx != p && visible_driver(d, f))
            // Ignore cars on pit road unless we're on it too.
            .filter(|d| f.on_pit_road || !f.car_on_pit_road[d.car_idx as usize])
            .filter_map(|d| {
                let mut frac = f.car_lap_dist_pct[d.car_idx as usize] - f.lap_dist_pct;
                frac -= frac.round();
                let dist = frac * track_m;
                (dist.abs() <= RADAR_RANGE_M).then(|| NearbyCar {
                    car_idx: d.car_idx,
                    dist,
                    side: 0,
                    class_color: d.class_color.clone(),
                })
            })
            .collect();
        cars.sort_by(|a, b| a.dist.abs().total_cmp(&b.dist.abs()));

        use car_left_right as clr;
        let sides: &[i32] = match f.car_left_right {
            clr::CAR_LEFT => &[-1],
            clr::CAR_RIGHT => &[1],
            clr::CAR_LEFT_RIGHT => &[-1, 1],
            clr::TWO_CARS_LEFT => &[-1, -1],
            clr::TWO_CARS_RIGHT => &[1, 1],
            _ => &[],
        };
        let mut remaining: Vec<i32> = sides.to_vec();
        let overlaps = |c: &NearbyCar| c.dist.abs() < CAR_LENGTH_M * 1.6;

        // 1. Overlapping cars keep the side they had, if iRacing still reports a car there.
        for c in cars.iter_mut().filter(|c| overlaps(c)) {
            if let Some(&prev) = self.radar_sides.get(&c.car_idx) {
                if let Some(k) = remaining.iter().position(|&s| s == prev) {
                    c.side = remaining.remove(k);
                }
            }
        }
        // 2. Remaining reported sides go to the nearest unassigned overlapping cars.
        for c in cars.iter_mut().filter(|c| overlaps(c) && c.side == 0) {
            if remaining.is_empty() {
                break;
            }
            c.side = remaining.remove(0);
        }
        // 3. A car that was beside us and has just pulled ahead or dropped back
        //    is still in that lane, so keep it there while it's close.
        for c in cars.iter_mut().filter(|c| c.side == 0 && c.dist.abs() < LANE_MEMORY_M) {
            if let Some(&prev) = self.radar_sides.get(&c.car_idx) {
                c.side = prev;
            }
        }

        self.radar_sides = cars.iter().filter(|c| c.side != 0).map(|c| (c.car_idx, c.side)).collect();
        cars
    }

    fn tires(&self, f: &RawFrame, s: &SessionInfo) -> TiresPayload {
        let corners: Vec<TireCorner> = (0..4)
            .map(|k| TireCorner {
                temp: f.tire_temp[k],
                // Reported as a 0..1 fraction; guard against a 0..100 build.
                wear: f.tire_wear[k].map(|w| if w > 1.5 { w / 100.0 } else { w }),
                cold_pressure: f.tire_cold_pressure[k],
            })
            .collect();
        let has_data = corners.iter().any(|c| c.temp.iter().any(|t| *t > 0.0) || c.wear.iter().any(|w| *w > 0.0));
        TiresPayload {
            corners,
            compound: s
                .tire_compounds
                .get(f.tire_compound.max(0) as usize)
                .cloned()
                .unwrap_or_default(),
            sets_used: f.tire_sets_used,
            sets_available: (f.tire_sets_available >= 0 && f.tire_sets_available < 255).then_some(f.tire_sets_available),
            live: f.in_pit_stall,
            has_data,
        }
    }

    fn pit(&self, f: &RawFrame, s: &SessionInfo) -> PitPayload {
        use crate::pit::flags;
        let Some(sv) = f.pit_service else { return PitPayload::default() };
        let on = |bit: u32| sv.flags & bit != 0;
        PitPayload {
            available: true,
            tires: flags::TIRES.map(on),
            pressures: sv.pressure,
            compound: s.tire_compounds.get(sv.compound.max(0) as usize).cloned().unwrap_or_default(),
            fuel: on(flags::FUEL),
            fuel_amount: sv.fuel,
            tearoff: on(flags::TEAROFF),
            fast_repair: on(flags::FAST_REPAIR),
            fast_repairs_left: f.fast_repairs_available.filter(|n| (0..255).contains(n)),
            status: f.pit_sv_status,
            in_pit_stall: f.in_pit_stall,
        }
    }

    /// Call at ~10 Hz. Builds relative, standings, fuel and clock.
    pub fn field(&self, f: &RawFrame) -> FieldPayload {
        let Some(session) = &self.session else { return FieldPayload::default() };
        FieldPayload {
            clock: self.clock(f, session),
            relative: self.relative(f, session),
            standings: self.standings(f, session),
            fuel: self.fuel.payload(f, session, self.is_race(f.session_num)),
            tires: self.tires(f, session),
            pit: self.pit(f, session),
            lap_log: self.laps.payload(),
            stints: self.stints.payload(),
            pit_loss: self.pit_loss.payload(),
        }
    }

    fn clock(&self, f: &RawFrame, s: &SessionInfo) -> SessionClock {
        let def = s.session(f.session_num);
        let sof = strength_of_field(s, s.player().map(|p| p.class_id));
        SessionClock {
            classes: class_summaries(s),
            time_of_day: f.time_of_day,
            skies: f.skies,
            track_wetness: f.track_wetness,
            declared_wet: f.declared_wet,
            wind_speed: f.wind_speed,
            wind_dir: f.wind_dir,
            humidity: f.humidity,
            precipitation: f.precipitation,
            fog_level: f.fog_level,
            incidents: if s.team_racing { f.team_incidents } else { f.incidents },
            incident_limit: s.incident_limit,
            fast_repairs_used: f.fast_repairs_used,
            fast_repairs_limit: s.fast_repairs_limit,
            session_type: def.map(|d| d.session_type.clone()).unwrap_or_default(),
            time_remain: (f.session_time_remain > 0.0 && f.session_time_remain < 604_800.0)
                .then_some(f.session_time_remain),
            laps_remain: (f.session_laps_remain >= 0 && f.session_laps_remain < 32_767)
                .then_some(f.session_laps_remain),
            total_laps: def.and_then(|d| d.laps),
            flags: f.session_flags,
            track_temp: f.track_temp,
            air_temp: f.air_temp,
            strength_of_field: sof,
        }
    }

    fn relative(&self, f: &RawFrame, s: &SessionInfo) -> Vec<RelativeRow> {
        let p = f.player_car_idx.clamp(0, MAX_CARS as i32 - 1) as usize;
        let player_class = s.driver(p as i32);
        let lap_time = reference_lap_time(s, player_class, f, p);
        let race = self.is_race(f.session_num);
        let player_progress = f.car_lap[p] as f32 + f.car_lap_dist_pct[p];
        let player_frac = lap_time_fraction(f, p, player_class.map(|d| d.class_est_lap_time).unwrap_or(0.0));
        let fastest = class_fastest(f, s);

        let mut rows: Vec<RelativeRow> = s
            .drivers
            .iter()
            .filter(|d| visible_driver(d, f))
            .map(|d| {
                let i = d.car_idx as usize;
                let gap = if i == p {
                    0.0
                } else {
                    relative_gap(lap_time_fraction(f, i, d.class_est_lap_time), player_frac, lap_time)
                };
                let progress = f.car_lap[i] as f32 + f.car_lap_dist_pct[i];
                let lap_status = if !race || i == p {
                    0
                } else {
                    // Total race progress minus where they sit on track relative to us
                    // leaves whole laps: +1 means they're lapping us, -1 we're lapping them.
                    let mut on_track = f.car_lap_dist_pct[i] - f.car_lap_dist_pct[p];
                    on_track -= on_track.round();
                    ((progress - player_progress) - on_track).round().clamp(-1.0, 1.0) as i32
                };
                RelativeRow {
                    driver: driver_cell(d),
                    position: f.car_position[i],
                    class_position: f.car_class_position[i],
                    gap,
                    lap_status,
                    laps: lap_info(f, d, &fastest),
                    on_pit_road: f.car_on_pit_road[i],
                    status: CarStatus::of(f, i),
                    is_player: i == p,
                }
            })
            .collect();
        rows.sort_by(|a, b| b.gap.total_cmp(&a.gap));
        rows
    }

    fn standings(&self, f: &RawFrame, s: &SessionInfo) -> Vec<StandingRow> {
        let race = self.is_race(f.session_num);
        let p = f.player_car_idx;

        let entrants: Vec<&DriverInfo> = s
            .drivers
            .iter()
            .filter(|d| !d.is_spectator && !d.is_pace_car && (d.car_idx as usize) < MAX_CARS)
            .collect();

        let fastest = class_fastest(f, s);

        let mut rows: Vec<StandingRow> = entrants
            .iter()
            .map(|d| {
                let i = d.car_idx as usize;
                StandingRow {
                    driver: driver_cell(d),
                    position: f.car_position[i],
                    class_position: f.car_class_position[i],
                    lap: f.car_lap_completed[i].max(0),
                    gap: None,
                    laps_down: 0,
                    interval: None,
                    start_position: if race { self.grid.start(d.car_idx, s) } else { None },
                    laps: lap_info(f, d, &fastest),
                    on_pit_road: f.car_on_pit_road[i],
                    status: CarStatus::of(f, i),
                    is_player: d.car_idx == p,
                }
            })
            .collect();

        // Unclassified cars (position 0) go to the back, ordered by best lap.
        rows.sort_by(|a, b| {
            let ka = (a.position == 0, if a.position > 0 { a.position as f32 } else { a.laps.best_lap.unwrap_or(f32::MAX) });
            let kb = (b.position == 0, if b.position > 0 { b.position as f32 } else { b.laps.best_lap.unwrap_or(f32::MAX) });
            ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
        });

        // Gaps are computed within each class.
        let mut class_leader: HashMap<i32, usize> = HashMap::new();
        let mut class_prev: HashMap<i32, usize> = HashMap::new();
        for idx in 0..rows.len() {
            let class = rows[idx].driver.class_id;
            let i = rows[idx].driver.car_idx as usize;
            if rows[idx].position == 0 {
                continue;
            }
            let leader = *class_leader.entry(class).or_insert(idx);
            let li = rows[leader].driver.car_idx as usize;

            if race {
                let progress = |c: usize| f.car_lap_completed[c] as f32 + f.car_lap_dist_pct[c].max(0.0);
                let laps_down = (progress(li) - progress(i)).floor().max(0.0) as i32;
                let f2 = f.car_f2_time[i] - f.car_f2_time[li];
                rows[idx].laps_down = laps_down;
                rows[idx].gap = (leader != idx).then_some(f2.max(0.0));
                if let Some(&prev) = class_prev.get(&class) {
                    let pi = rows[prev].driver.car_idx as usize;
                    rows[idx].interval = Some((f.car_f2_time[i] - f.car_f2_time[pi]).max(0.0));
                }
            } else if let (Some(best), Some(leader_best)) = (rows[idx].laps.best_lap, rows[leader].laps.best_lap) {
                rows[idx].gap = (leader != idx).then_some(best - leader_best);
                if let Some(&prev) = class_prev.get(&class) {
                    if let Some(prev_best) = rows[prev].laps.best_lap {
                        rows[idx].interval = Some(best - prev_best);
                    }
                }
            }
            class_prev.insert(class, idx);
        }
        rows
    }
}

/// Car length used to decide when another car overlaps ours.
const CAR_LENGTH_M: f32 = 4.7;
const RADAR_RANGE_M: f32 = 60.0;
/// How far a car can move away along the track and still be drawn in its last lane.
const LANE_MEMORY_M: f32 = 14.0;
/// Lap times within this many seconds count as the same lap.
const LAP_EPS: f32 = 0.0005;

/// Fastest lap per class: class id -> (car idx, time).
fn class_fastest(f: &RawFrame, s: &SessionInfo) -> HashMap<i32, (i32, f32)> {
    let mut fastest: HashMap<i32, (i32, f32)> = HashMap::new();
    for d in s.drivers.iter().filter(|d| !d.is_spectator && !d.is_pace_car && (0..MAX_CARS as i32).contains(&d.car_idx)) {
        let best = f.car_best_lap[d.car_idx as usize];
        if best > 0.0 {
            let e = fastest.entry(d.class_id).or_insert((d.car_idx, best));
            if best < e.1 {
                *e = (d.car_idx, best);
            }
        }
    }
    fastest
}

/// Last / best lap with purple (class fastest) and green (personal best) flags.
fn lap_info(f: &RawFrame, d: &DriverInfo, fastest: &HashMap<i32, (i32, f32)>) -> LapInfo {
    let i = d.car_idx as usize;
    let last = positive(f.car_last_lap[i]);
    let best = positive(f.car_best_lap[i]);
    let class_best = fastest.get(&d.class_id).copied();
    let last_flag = match (last, best) {
        (Some(l), Some(b)) if (l - b).abs() < LAP_EPS => {
            if class_best.is_some_and(|(_, t)| (l - t).abs() < LAP_EPS) {
                LapFlag::Fastest
            } else {
                LapFlag::Personal
            }
        }
        _ => LapFlag::None,
    };
    LapInfo {
        last_lap: last,
        best_lap: best,
        last_flag,
        class_fastest: class_best.map(|(c, _)| c) == Some(d.car_idx),
    }
}

fn positive(v: f32) -> Option<f32> {
    (v > 0.0).then_some(v)
}

fn visible_driver(d: &DriverInfo, f: &RawFrame) -> bool {
    let i = d.car_idx;
    !d.is_spectator
        && !d.is_pace_car
        && (0..MAX_CARS as i32).contains(&i)
        && f.car_track_surface[i as usize] != track_surface::NOT_IN_WORLD
        && f.car_lap_dist_pct[i as usize] >= 0.0
}

fn driver_cell(d: &DriverInfo) -> DriverCell {
    DriverCell {
        car_idx: d.car_idx,
        name: d.user_name.clone(),
        abbrev: d.abbrev_name.clone(),
        number: d.car_number.clone(),
        class_id: d.class_id,
        class_name: d.class_short_name.clone(),
        class_color: d.class_color.clone(),
        irating: d.irating,
        license: d.license.clone(),
        license_color: d.license_color.clone(),
    }
}

fn reference_lap_time(s: &SessionInfo, player: Option<&DriverInfo>, f: &RawFrame, p: usize) -> f32 {
    [
        player.map(|d| d.class_est_lap_time).unwrap_or(0.0),
        s.est_lap_time,
        f.car_best_lap[p],
        f.lap_best,
    ]
    .into_iter()
    .find(|t| *t > 1.0)
    .unwrap_or(90.0)
}

/// How far round the lap a car is, as a fraction of a lap in *time*.
/// `CarIdxEstTime` is measured against each car's own class pace, so we
/// normalise by that class's estimated lap time to compare cars across classes.
/// Falls back to distance when the estimate isn't available.
pub fn lap_time_fraction(f: &RawFrame, car: usize, class_est_lap: f32) -> f32 {
    let est = f.car_est_time[car];
    if est > 0.0 && class_est_lap > 1.0 {
        (est / class_est_lap).clamp(0.0, 1.0)
    } else {
        f.car_lap_dist_pct[car].clamp(0.0, 1.0)
    }
}

/// Seconds on track between two cars, positive when the car is ahead of the player.
/// Takes each car's lap fraction (see `lap_time_fraction`) and wraps to ±half a lap.
pub fn relative_gap(car_frac: f32, player_frac: f32, lap_time: f32) -> f32 {
    let mut d = car_frac - player_frac;
    d -= d.round();
    d * lap_time
}

fn is_entry(d: &DriverInfo) -> bool {
    !d.is_spectator && !d.is_pace_car && d.car_idx >= 0
}

/// Each class with its entry count and SoF, fastest class (shortest estimated lap) first.
fn class_summaries(s: &SessionInfo) -> Vec<ClassSummary> {
    let mut out: Vec<(f32, ClassSummary)> = Vec::new();
    for d in s.drivers.iter().filter(|d| is_entry(d)) {
        match out.iter_mut().find(|(_, c)| c.class_id == d.class_id) {
            Some((_, c)) => c.cars += 1,
            None => out.push((
                d.class_est_lap_time,
                ClassSummary {
                    class_id: d.class_id,
                    name: d.class_short_name.clone(),
                    color: d.class_color.clone(),
                    cars: 1,
                    strength_of_field: strength_of_field(s, Some(d.class_id)),
                },
            )),
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, c)| c).collect()
}

/// iRacing's published SoF formula.
fn strength_of_field(s: &SessionInfo, class: Option<i32>) -> Option<i32> {
    let irs: Vec<f64> = s
        .drivers
        .iter()
        .filter(|d| is_entry(d) && d.irating > 0)
        .filter(|d| class.is_none_or(|c| d.class_id == c))
        .map(|d| d.irating as f64)
        .collect();
    if irs.is_empty() {
        return None;
    }
    let br = 1600.0 / std::f64::consts::LN_2;
    let sum: f64 = irs.iter().map(|ir| (-ir / br).exp()).sum();
    let n = irs.len() as f64;
    Some((br * (n / sum).ln()).round() as i32)
}

// ---------------------------------------------------------------------------
// Optimal lap
// ---------------------------------------------------------------------------

/// Times each track sector and keeps the best clean one, so the optimal lap is
/// their sum. A sector counts only when driven start to finish on track, off pit
/// road and without going off; boundary times are interpolated between frames.
#[derive(Default)]
struct SectorTracker {
    session_num: i32,
    starts: Vec<f32>,
    best: Vec<Option<f32>>,
    /// Sector being timed and when it began.
    sector: Option<usize>,
    sector_start: f64,
    clean: bool,
    prev: Option<(f32, f64)>,
}

impl SectorTracker {
    fn update(&mut self, f: &RawFrame, starts: &[f32]) {
        let starts = if starts.is_empty() { &[0.0][..] } else { starts };
        if f.session_num != self.session_num || self.starts != starts {
            *self = Self {
                session_num: f.session_num,
                starts: starts.to_vec(),
                best: vec![None; starts.len()],
                ..Self::default()
            };
        }
        if !f.is_on_track || f.lap_dist_pct < 0.0 {
            self.sector = None;
            self.prev = None;
            return;
        }

        let (pct, t) = (f.lap_dist_pct, f.session_time);
        let off = f.on_pit_road
            || f.car_track_surface.get(f.player_car_idx.max(0) as usize) == Some(&track_surface::OFF_TRACK);
        let idx = self.starts.iter().rposition(|&s| s <= pct).unwrap_or(0);

        match (self.sector, self.prev) {
            (Some(cur), Some((p0, t0))) => {
                let n = self.starts.len();
                let wrapped = pct < p0 - 0.5;
                if idx != cur || wrapped {
                    let next = (cur + 1) % n;
                    if idx == next && (next != 0 || wrapped) {
                        // Crossed into the next sector: finish this one.
                        let boundary = if next == 0 { 1.0 } else { self.starts[next] };
                        let p1 = if wrapped { pct + 1.0 } else { pct };
                        let frac = if p1 > p0 { ((boundary - p0) / (p1 - p0)).clamp(0.0, 1.0) } else { 1.0 };
                        let crossed_at = t0 + frac as f64 * (t - t0);
                        let time = (crossed_at - self.sector_start) as f32;
                        if self.clean && time > 1.0 && self.best[cur].is_none_or(|b| time < b) {
                            self.best[cur] = Some(time);
                        }
                        self.sector = Some(next);
                        self.sector_start = crossed_at;
                        self.clean = !off;
                    } else {
                        // Skipped a sector or went backwards (tow, reset): start over mid-sector.
                        self.sector = Some(idx);
                        self.sector_start = t;
                        self.clean = false;
                    }
                }
            }
            _ => {
                // Joined mid-sector, so this one can't be timed.
                self.sector = Some(idx);
                self.sector_start = t;
                self.clean = false;
            }
        }
        if off {
            self.clean = false;
        }
        self.prev = Some((pct, t));
    }

    /// Sum of the best sectors, once every sector has a clean time.
    fn optimal(&self) -> Option<f32> {
        if self.best.is_empty() {
            return None;
        }
        self.best.iter().copied().sum()
    }
}

// ---------------------------------------------------------------------------
// Fuel
// ---------------------------------------------------------------------------

#[derive(Default)]
struct FuelTracker {
    session_num: i32,
    lap_completed: Option<i32>,
    lap_start_fuel: Option<f32>,
    lap_start_time: f64,
    last_fuel: f32,
    lap_tainted: bool,
    per_lap: VecDeque<f32>,
    lap_times: VecDeque<f32>,
}

impl FuelTracker {
    fn update(&mut self, f: &RawFrame) {
        if f.session_num != self.session_num {
            *self = Self { session_num: f.session_num, ..Self::default() };
        }
        if !f.is_on_track {
            self.lap_completed = None;
            self.last_fuel = f.fuel_level;
            return;
        }
        // Refuelling or a pit visit spoils this lap's sample.
        if f.on_pit_road || f.fuel_level > self.last_fuel + 0.05 {
            self.lap_tainted = true;
        }
        self.last_fuel = f.fuel_level;

        match self.lap_completed {
            None => {
                // First frame on track: we joined mid-lap, so skip this lap's sample.
                self.lap_completed = Some(f.lap_completed);
                self.lap_start_fuel = Some(f.fuel_level);
                self.lap_start_time = f.session_time;
                self.lap_tainted = true;
            }
            Some(prev) if prev != f.lap_completed => {
                let completed_one = f.lap_completed == prev + 1;
                if let (Some(start), true, false) = (self.lap_start_fuel, completed_one, self.lap_tainted) {
                    let used = start - f.fuel_level;
                    if used > 0.01 {
                        push_capped(&mut self.per_lap, used, FUEL_WINDOW);
                    }
                    let lap_time = (f.session_time - self.lap_start_time) as f32;
                    if lap_time > 5.0 {
                        push_capped(&mut self.lap_times, lap_time, LAP_TIME_WINDOW);
                    }
                }
                self.lap_completed = Some(f.lap_completed);
                self.lap_start_fuel = Some(f.fuel_level);
                self.lap_start_time = f.session_time;
                self.lap_tainted = f.on_pit_road;
            }
            Some(_) => {}
        }
    }

    fn payload(&self, f: &RawFrame, s: &SessionInfo, race: bool) -> FuelPayload {
        let per_lap = mean(&self.per_lap);
        let lap_time = mean(&self.lap_times)
            .or_else(|| (f.lap_best > 0.0).then_some(f.lap_best))
            .or_else(|| (s.est_lap_time > 0.0).then_some(s.est_lap_time));

        let rest_of_lap = 1.0 - f.lap_dist_pct.clamp(0.0, 1.0);
        let laps_to_go = laps_to_go(&LapsToGo {
            race,
            laps_remain: f.session_laps_remain,
            time_remain: f.session_time_remain,
            rest_of_lap,
            lap_time,
            leader: if race { race_leader(f, s) } else { None },
            rule: TIMED_FINISH,
        });

        FuelPayload {
            level: f.fuel_level,
            max: s.fuel_max_ltr,
            per_lap,
            last_lap_used: self.per_lap.back().copied(),
            laps_of_fuel: per_lap.filter(|p| *p > 0.0).map(|p| f.fuel_level / p),
            laps_to_go,
            to_finish: match (per_lap, laps_to_go) {
                (Some(p), Some(l)) => Some(p * l - f.fuel_level),
                _ => None,
            },
            samples: self.per_lap.len() as u32,
        }
    }
}

/// How a timed race ends once the clock reaches zero.
///
/// We haven't confirmed which one iRacing uses. Run the `timed_finish`
/// example on a recording of a timed race to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimedFinish {
    /// The leader takes the chequered flag the first time they cross the line
    /// after the clock reaches zero.
    ChequeredAtLine,
    /// The leader takes the white flag at that crossing, then does one more lap.
    WhiteThenOneLap,
}

/// The rule the fuel maths uses. `WhiteThenOneLap` is never less than the
/// older figure (whole laps of time left, rounded up, plus the rest of this
/// lap), and `ChequeredAtLine` is never more, so until the rule is confirmed
/// this one can't leave the car short.
pub const TIMED_FINISH: TimedFinish = TimedFinish::WhiteThenOneLap;

/// Where the race leader is, for working out when a timed race ends.
#[derive(Debug, Clone, Copy)]
pub struct Leader {
    /// Fraction of a lap left before the leader next crosses the line.
    pub rest_of_lap: f32,
    /// The leader's lap time in seconds.
    pub lap_time: f32,
}

/// Inputs for `laps_to_go`.
#[derive(Debug, Clone, Copy)]
pub struct LapsToGo {
    pub race: bool,
    /// iRacing's `SessionLapsRemainEx`. Counts the lap we're on; 32767 when unknown.
    pub laps_remain: i32,
    /// iRacing's `SessionTimeRemain`, seconds.
    pub time_remain: f64,
    /// Fraction of a lap left before we next cross the line.
    pub rest_of_lap: f32,
    /// Our lap time in seconds.
    pub lap_time: Option<f32>,
    /// The overall leader, when it's another car. iRacing ends the race for
    /// everyone when the overall leader finishes.
    pub leader: Option<Leader>,
    pub rule: TimedFinish,
}

/// Laps (as a fraction, including the rest of this one) until we take the
/// chequered flag, or None when it can't be worked out.
///
/// * A race with a real laps-remaining figure from iRacing uses that.
/// * Otherwise, with time on the clock: if another car leads the race, work
///   out when the leader finishes under `rule`, then how many of our laps it
///   takes to reach the line after that. If we lead (or the leader isn't
///   known), apply `rule` to our own laps.
/// * Practice and qualifying end at our first crossing after the clock runs
///   out, whatever `rule` says.
pub fn laps_to_go(x: &LapsToGo) -> Option<f32> {
    if x.race && (0..32_767).contains(&x.laps_remain) {
        // Laps remaining counts the current lap; we still need the rest of it.
        return Some((x.laps_remain as f32 - 1.0).max(0.0) + x.rest_of_lap);
    }
    if !(x.time_remain > 0.0 && x.time_remain < 604_800.0) {
        return None;
    }
    let lt = x.lap_time.filter(|t| *t > 1.0)? as f64;
    let rest = x.rest_of_lap.clamp(0.0, 1.0) as f64;
    let rule = if x.race { x.rule } else { TimedFinish::ChequeredAtLine };

    let laps = match x.leader.filter(|l| l.lap_time > 1.0) {
        Some(l) => {
            // Seconds until the leader finishes, then our first crossing after that.
            let l_lt = l.lap_time as f64;
            let finish = finish_laps(x.time_remain, l.rest_of_lap.clamp(0.0, 1.0) as f64, l_lt, rule) * l_lt;
            rest + crossings_until(finish, rest, lt)
        }
        None => finish_laps(x.time_remain, rest, lt, rule),
    };
    Some(laps as f32)
}

/// Small slack so a crossing that lands exactly on a time isn't pushed a lap later by rounding.
const CROSSING_EPS: f64 = 1e-6;

/// Whole laps after the next crossing (which is `rest` laps away) until the
/// first crossing at or after `t` seconds, at `lt` seconds a lap.
fn crossings_until(t: f64, rest: f64, lt: f64) -> f64 {
    (t / lt - rest - CROSSING_EPS).ceil().max(0.0)
}

/// Laps until a car `rest` laps from the line, lapping in `lt`, finishes when
/// the clock (`t` seconds left) ends the race under `rule`.
fn finish_laps(t: f64, rest: f64, lt: f64, rule: TimedFinish) -> f64 {
    let at_line = rest + crossings_until(t, rest, lt);
    match rule {
        TimedFinish::ChequeredAtLine => at_line,
        TimedFinish::WhiteThenOneLap => at_line + 1.0,
    }
}

/// The overall race leader, when it's another car with a known lap time.
fn race_leader(f: &RawFrame, s: &SessionInfo) -> Option<Leader> {
    let d = s
        .drivers
        .iter()
        .filter(|d| !d.is_spectator && !d.is_pace_car && (0..MAX_CARS as i32).contains(&d.car_idx))
        .find(|d| f.car_position[d.car_idx as usize] == 1)?;
    let i = d.car_idx as usize;
    if d.car_idx == f.player_car_idx || f.car_lap_dist_pct[i] < 0.0 {
        return None;
    }
    let lap_time = [f.car_last_lap[i], f.car_best_lap[i], d.class_est_lap_time].into_iter().find(|t| *t > 1.0)?;
    Some(Leader { rest_of_lap: 1.0 - f.car_lap_dist_pct[i].clamp(0.0, 1.0), lap_time })
}

fn push_capped(q: &mut VecDeque<f32>, v: f32, cap: usize) {
    q.push_back(v);
    while q.len() > cap {
        q.pop_front();
    }
}

fn mean(q: &VecDeque<f32>) -> Option<f32> {
    (!q.is_empty()).then(|| q.iter().sum::<f32>() / q.len() as f32)
}

/// iRacing's `YawNorth` turns the same way as its yaw rate (positive = left,
/// anticlockwise seen from above). A compass heading runs clockwise from
/// north, so it's the negative, wrapped to 0..2π.
pub fn heading_from_yaw_north(yaw_north: f32) -> f32 {
    (-yaw_north).rem_euclid(std::f32::consts::TAU)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_wraps_across_start_finish() {
        let mut f = RawFrame::with_capacity();
        f.car_est_time[0] = 1.0; // player just crossed the line
        f.car_est_time[1] = 89.0; // car about to cross: 2 s behind
        f.car_est_time[2] = 3.5; // car 2.5 s ahead
        f.car_est_time[3] = 49.5; // slower class (99 s lap), half a lap round = 45 s at our pace
        let frac = |c| lap_time_fraction(&f, c, if c == 3 { 99.0 } else { 90.0 });
        assert!((relative_gap(frac(1), frac(0), 90.0) + 2.0).abs() < 1e-3);
        assert!((relative_gap(frac(2), frac(0), 90.0) - 2.5).abs() < 1e-3);
        assert!((relative_gap(frac(3), frac(0), 90.0).abs() - 44.0).abs() < 1e-3);
    }

    #[test]
    fn fuel_average_ignores_pit_laps() {
        let mut t = FuelTracker::default();
        let mut f = RawFrame::with_capacity();
        f.is_on_track = true;
        f.fuel_level = 50.0;
        f.lap_completed = 1;
        t.update(&f); // joins mid lap -> tainted
        for lap in 2..=4 {
            f.fuel_level -= 2.5;
            f.lap_completed = lap;
            f.session_time += 90.0;
            t.update(&f);
        }
        // first crossing (lap 2) was a partial lap, so 2 samples
        assert_eq!(t.per_lap.len(), 2);
        assert!((mean(&t.per_lap).unwrap() - 2.5).abs() < 1e-4);

        // pit lap with refuel is ignored
        f.on_pit_road = true;
        f.fuel_level = 80.0;
        t.update(&f);
        f.on_pit_road = false;
        f.lap_completed = 5;
        t.update(&f);
        assert_eq!(t.per_lap.len(), 2);
    }

    fn two_car_session() -> SessionInfo {
        SessionInfo {
            track_length_km: 5.0,
            player_car_idx: 0,
            drivers: (0..3)
                .map(|i| DriverInfo { car_idx: i, class_id: 1, class_color: "#fff".into(), ..Default::default() })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn radar_places_overlapping_car_on_reported_side() {
        let mut e = Engine::new();
        e.set_session(two_car_session());
        let mut f = RawFrame::with_capacity();
        f.player_car_idx = 0;
        f.lap_dist_pct = 0.5;
        for i in 0..3 {
            f.car_track_surface[i] = track_surface::ON_TRACK;
        }
        f.car_lap_dist_pct[0] = 0.5;
        f.car_lap_dist_pct[1] = 0.5004; // 2 m ahead, alongside
        f.car_lap_dist_pct[2] = 0.4960; // 20 m behind
        f.car_left_right = car_left_right::CAR_RIGHT;
        let near = e.nearby(&f);
        assert_eq!(near.len(), 2);
        let alongside = near.iter().find(|c| c.car_idx == 1).unwrap();
        assert_eq!(alongside.side, 1);
        assert!((alongside.dist - 2.0).abs() < 0.01);
        let behind = near.iter().find(|c| c.car_idx == 2).unwrap();
        assert_eq!(behind.side, 0);
        assert!((behind.dist + 20.0).abs() < 0.01);
    }

    #[test]
    fn last_lap_flags() {
        let s = two_car_session();
        let mut f = RawFrame::with_capacity();
        f.car_best_lap[0] = 90.0;
        f.car_last_lap[0] = 90.0; // fastest in class
        f.car_best_lap[1] = 91.0;
        f.car_last_lap[1] = 91.0; // personal best only
        f.car_best_lap[2] = 92.0;
        f.car_last_lap[2] = 93.0; // nothing
        let fastest = class_fastest(&f, &s);
        let flag = |i: usize| lap_info(&f, &s.drivers[i], &fastest).last_flag;
        assert_eq!(flag(0), LapFlag::Fastest);
        assert_eq!(flag(1), LapFlag::Personal);
        assert_eq!(flag(2), LapFlag::None);
    }

    #[test]
    fn sof_matches_equal_field() {
        let s = SessionInfo {
            drivers: (0..10)
                .map(|i| DriverInfo { car_idx: i, irating: 2000, ..Default::default() })
                .collect(),
            ..Default::default()
        };
        assert_eq!(strength_of_field(&s, None), Some(2000));
    }

    /// Drives one lap at 60 Hz taking `sector_secs[k]` seconds over sector k,
    /// optionally putting the car off track halfway through one sector.
    fn drive(t: &mut SectorTracker, f: &mut RawFrame, starts: &[f32], sector_secs: &[f32], off_in: Option<usize>) {
        let mut pct = f.lap_dist_pct;
        for (k, &secs) in sector_secs.iter().enumerate() {
            let end = starts.get(k + 1).copied().unwrap_or(1.0);
            let width = end - starts[k];
            let steps = (secs * 60.0).round() as usize;
            for step in 0..steps {
                f.session_time += 1.0 / 60.0;
                pct += width / steps as f32;
                f.lap_dist_pct = pct.fract();
                let off = off_in == Some(k) && step == steps / 2;
                f.car_track_surface[0] = if off { track_surface::OFF_TRACK } else { track_surface::ON_TRACK };
                t.update(f, starts);
            }
        }
        pct = pct.round();
        f.lap_dist_pct = pct.fract();
    }

    #[test]
    fn optimal_lap_from_best_clean_sectors() {
        let starts = [0.0, 0.3, 0.7];
        let mut t = SectorTracker::default();
        let mut f = RawFrame::with_capacity();
        f.is_on_track = true;
        f.lap_dist_pct = 0.0;

        // First lap: timing only starts on the first frame, so sector 1 isn't timed.
        drive(&mut t, &mut f, &starts, &[30.0, 40.0, 30.0], None);
        assert_eq!(t.optimal(), None, "the first sector started mid-way");
        drive(&mut t, &mut f, &starts, &[29.0, 41.0, 30.0], None);
        let o = t.optimal().unwrap();
        assert!((o - 99.0).abs() < 0.05, "29 + 40 + 30, got {o}");
        // A faster sector that went off doesn't count.
        drive(&mut t, &mut f, &starts, &[28.0, 39.0, 31.0], Some(0));
        let o = t.optimal().unwrap();
        assert!((o - 98.0).abs() < 0.05, "29 + 39 + 30, got {o}");
    }

    #[test]
    fn shift_rpm_prefers_live_blink_then_session_values() {
        let mut e = Engine::new();
        e.set_session(SessionInfo { shift_rpm: 7000.0, last_rpm: 7600.0, ..Default::default() });
        let mut f = RawFrame::with_capacity();
        assert_eq!(e.shift_rpm(&f), Some(7600.0));
        f.sl_last_rpm = Some(7700.0);
        assert_eq!(e.shift_rpm(&f), Some(7700.0));
        f.sl_blink_rpm = Some(7800.0);
        assert_eq!(e.shift_rpm(&f), Some(7800.0));
    }

    #[test]
    fn class_summaries_split_the_field() {
        let driver = |i: i32, class_id: i32, irating: i32, est: f32| DriverInfo {
            car_idx: i, class_id, irating, class_est_lap_time: est, ..Default::default()
        };
        let mut pace = driver(0, 9, 0, 0.0);
        pace.is_pace_car = true;
        let s = SessionInfo {
            drivers: vec![pace, driver(1, 2, 1500, 100.0), driver(2, 1, 3000, 90.0), driver(3, 2, 1500, 100.0)],
            ..Default::default()
        };
        let c = class_summaries(&s);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].class_id, c[0].cars, c[0].strength_of_field), (1, 1, Some(3000)));
        assert_eq!((c[1].class_id, c[1].cars, c[1].strength_of_field), (2, 2, Some(1500)));
    }

    /// Timed race, 100 s laps, no laps-remaining figure from iRacing.
    fn timed(time_remain: f64, rest_of_lap: f32, rule: TimedFinish) -> LapsToGo {
        LapsToGo {
            race: true,
            laps_remain: 32_767,
            time_remain,
            rest_of_lap,
            lap_time: Some(100.0),
            leader: None,
            rule,
        }
    }

    fn close(a: Option<f32>, b: f32) -> bool {
        a.is_some_and(|a| (a - b).abs() < 1e-4)
    }

    #[test]
    fn timed_race_clock_runs_out_early_in_a_lap() {
        // Half a lap to the line (50 s), clock ends at 60 s: 10 s into the next lap.
        use TimedFinish::*;
        assert!(close(laps_to_go(&timed(60.0, 0.5, ChequeredAtLine)), 1.5));
        assert!(close(laps_to_go(&timed(60.0, 0.5, WhiteThenOneLap)), 2.5));
    }

    #[test]
    fn timed_race_clock_runs_out_late_in_a_lap() {
        // Clock ends at 140 s, 10 s before our crossing at 150 s.
        use TimedFinish::*;
        assert!(close(laps_to_go(&timed(140.0, 0.5, ChequeredAtLine)), 1.5));
        assert!(close(laps_to_go(&timed(140.0, 0.5, WhiteThenOneLap)), 2.5));
    }

    #[test]
    fn timed_race_clock_runs_out_on_the_line() {
        // Clock ends exactly as we cross at 50 s: that crossing counts.
        use TimedFinish::*;
        assert!(close(laps_to_go(&timed(50.0, 0.5, ChequeredAtLine)), 0.5));
        assert!(close(laps_to_go(&timed(50.0, 0.5, WhiteThenOneLap)), 1.5));
        assert!(close(laps_to_go(&timed(300.0, 1.0, ChequeredAtLine)), 3.0));
    }

    #[test]
    fn default_rule_never_below_old_figure() {
        for t in [1.0, 37.0, 50.0, 99.9, 100.0, 140.0, 1234.5] {
            for rest in [0.0, 0.01, 0.3, 0.5, 0.99, 1.0] {
                let old = (t as f32 / 100.0).ceil() + rest;
                let new = laps_to_go(&timed(t, rest, TIMED_FINISH)).unwrap();
                let a = laps_to_go(&timed(t, rest, TimedFinish::ChequeredAtLine)).unwrap();
                assert!(new >= old - 1e-4, "t {t} rest {rest}: {new} < {old}");
                assert!(a <= old + 1e-4, "t {t} rest {rest}: A {a} > {old}");
            }
        }
    }

    #[test]
    fn timed_race_lapped_car_finishes_after_the_leader() {
        // The leader has just crossed (crossings at 99 s, 199 s); the clock ends at 100 s.
        // We're nearly at the line (crossings at 5, 105, 205, 305 s), a lap down.
        let mut x = timed(100.0, 0.05, TimedFinish::ChequeredAtLine);
        x.leader = Some(Leader { rest_of_lap: 0.99, lap_time: 100.0 });
        // Leader finishes at 199 s, so we finish at 205 s.
        assert!(close(laps_to_go(&x), 2.05));
        // White flag at 199 s, leader finishes at 299 s, we finish at 305 s.
        x.rule = TimedFinish::WhiteThenOneLap;
        assert!(close(laps_to_go(&x), 3.05));
        // On our own clock it would have been a lap less.
        x.leader = None;
        assert!(close(laps_to_go(&x), 2.05));
    }

    #[test]
    fn timed_race_slower_class_uses_leader_pace() {
        // Leader laps in 90 s, 0.9 of a lap to go; clock ends at 200 s.
        // Leader crosses at 81, 171, 261 s: white at 261, finishes at 351 s.
        // We lap in 120 s and cross at 114, 234, 354 s.
        let mut x = timed(200.0, 0.95, TimedFinish::WhiteThenOneLap);
        x.lap_time = Some(120.0);
        x.leader = Some(Leader { rest_of_lap: 0.9, lap_time: 90.0 });
        assert!(close(laps_to_go(&x), 2.95));
    }

    #[test]
    fn laps_race_uses_iracing_laps_remaining() {
        let mut x = timed(500.0, 0.25, TIMED_FINISH);
        x.laps_remain = 4;
        x.leader = Some(Leader { rest_of_lap: 0.5, lap_time: 80.0 });
        assert!(close(laps_to_go(&x), 3.25));
        x.laps_remain = 0;
        assert!(close(laps_to_go(&x), 0.25));
        // Not a race: the laps figure is ignored and the clock ends it at our next crossing after.
        x.race = false;
        assert!(close(laps_to_go(&x), 5.25));
        x.time_remain = -1.0;
        x.laps_remain = 32_767;
        x.race = true;
        assert_eq!(laps_to_go(&x), None);
    }

    #[test]
    fn heading_runs_clockwise_from_north() {
        use std::f32::consts::{FRAC_PI_2, PI};
        let close = |a: f32, b: f32| (a - b).abs() < 1e-5;
        assert!(close(heading_from_yaw_north(0.0), 0.0));
        // Turned left (anticlockwise) a quarter: facing west.
        assert!(close(heading_from_yaw_north(FRAC_PI_2), 3.0 * FRAC_PI_2));
        // Turned right a quarter: facing east.
        assert!(close(heading_from_yaw_north(-FRAC_PI_2), FRAC_PI_2));
        assert!(close(heading_from_yaw_north(PI), PI));
    }
}
