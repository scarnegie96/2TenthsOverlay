//! Live coaching against an imported reference lap.
//!
//! On load, the reference is broken into corners. A braking corner runs from
//! where the reference starts braking, through its slowest point (apex), to
//! where it's back on the throttle. A lift corner is the same without the
//! brake: the reference comes off full throttle, slows, and gets back on it.
//! Corners can cross the start/finish line.
//!
//! While you drive, two things happen:
//!
//! - Each frame we compare what you're doing with the reference and pick the
//!   single most useful thing to say. Brake pressure is compared from *your*
//!   brake point, so braking late isn't misread as braking too lightly.
//!   Cues are held briefly so the label doesn't flicker; a more urgent cue can
//!   always interrupt.
//! - Each pass through a corner is recorded: where you braked, how hard, your
//!   slowest speed and gear, when you picked up the throttle, and the time you
//!   gained or lost on entry (up to the reference's apex) and on exit. As you
//!   leave the corner the coach gives a verdict naming the main cause, and it
//!   keeps a running average per corner so it can point at the corner that
//!   costs you most. Corners where you're consistently on pace stop getting
//!   minor cues.
//!
//! Line cues compare how sharply you're turning (yaw rate over speed) with the
//! reference's curvature, which every reference type has. Steering wheel angle
//! (.ibt references only) is the fallback when curvature is missing.

use std::collections::VecDeque;

use serde::Serialize;

use crate::model::{session_flags, track_surface, RawFrame};
use crate::reference::{ReferenceLap, REF_BINS};

/// Bins round the lap. Reference channels have `N + 1` points; the last is the
/// start line again, so bin `N` is bin 0.
const N: usize = REF_BINS;

/// How far before a corner's brake point we start watching it.
const APPROACH_M: f32 = 250.0;
/// How far past the throttle pickup a corner lasts, for timing its exit.
const EXIT_M: f32 = 150.0;
/// Space kept between one corner's exit and the next corner's brake point.
const MIN_GAP_M: f32 = 60.0;

/// Bins forward from `a` to `b`, going round the lap.
fn fwd(a: usize, b: usize) -> usize {
    (b % N + N - a % N) % N
}
/// Shortest signed bin offset from `a` to `b`, positive = `b` is further round.
fn signed(a: usize, b: usize) -> i64 {
    let d = fwd(a, b) as i64;
    if d > (N / 2) as i64 {
        d - N as i64
    } else {
        d
    }
}
fn add(a: usize, k: usize) -> usize {
    (a + k) % N
}
fn sub(a: usize, k: usize) -> usize {
    (a % N + N - k % N) % N
}
fn bins(m: f32, m_per_bin: f32) -> usize {
    (m / m_per_bin.max(1e-3)).ceil().max(1.0) as usize
}
fn bin_of(pct: f32) -> usize {
    (pct.clamp(0.0, 1.0) * N as f32).round() as usize % N
}

/// What the coach is telling you right now.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CueKind {
    /// The reference starts braking soon (countdown).
    BrakeSoon,
    /// The reference lifts soon (countdown).
    LiftSoon,
    /// The reference is already braking and you haven't.
    BrakeNow,
    /// You're braking before the reference does.
    BrakeEarly,
    /// You're braking where the reference doesn't brake at all.
    NoBrakeHere,
    /// Initial pressure, compared from your own brake point.
    BrakeHarder,
    BrakeLighter,
    GoodBraking,
    /// You've let go of the brake where the reference is still trailing it.
    HoldBrake,
    /// You're still braking where the reference has let go.
    ReleaseBrake,
    /// You're lifting more than the reference in a lift corner.
    LiftLess,
    /// Your slowest speed in the corner was below the reference's.
    CarrySpeed,
    /// Your slowest speed in the corner was above the reference's.
    FastApex,
    /// The reference is back on the throttle and you aren't.
    ThrottleNow,
    MoreThrottle,
    /// Gear choice around the apex.
    ShiftDown,
    ShiftUp,
    /// Down on speed on a straight.
    BetterExit,
    /// The reference turns more sharply here.
    TurnMore,
    TurnLess,
    /// Summary as you leave a corner: time gained or lost and the main reason.
    CornerVerdict,
    Gaining,
    Losing,
    OnPace,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Neutral,
    Info,
    Good,
    Warn,
    Bad,
}

/// The main reason a corner gained or lost time.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Issue {
    /// Faster than the reference through the corner.
    Gained,
    BrakedEarly,
    /// Braked late and ended up slower at the apex.
    BrakedLate,
    BrakeHarder,
    /// Slowest speed below the reference's.
    SlowApex,
    /// Too fast in, which cost the exit.
    FastEntry,
    LatePickup,
    LiftedEarly,
    /// Lifted further than the reference, or braked where it only lifts.
    LiftedTooMuch,
    WrongGear,
    /// Lost time on the way in, no single cause.
    Entry,
    /// Lost time on the way out, no single cause.
    Exit,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    pub kind: CueKind,
    pub tone: Tone,
    /// Metres to (positive) or past (negative) the reference's point. For
    /// verdicts: how much later (positive) or earlier you did it.
    pub distance_m: Option<f32>,
    /// Your value and the reference's: pedal 0..1, speed m/s or gear, depending on the cue.
    pub ours: Option<f32>,
    pub reference: Option<f32>,
    pub gear: Option<i32>,
    /// Corner number, counted from the start line.
    pub corner: Option<u32>,
    /// Verdicts: seconds lost in the corner (negative = gained).
    pub time_s: Option<f32>,
    pub issue: Option<Issue>,
    #[serde(skip)]
    priority: u8,
    #[serde(skip)]
    hold_s: f64,
}

impl Cue {
    fn new(kind: CueKind, tone: Tone, priority: u8) -> Self {
        Self {
            kind,
            tone,
            distance_m: None,
            ours: None,
            reference: None,
            gear: None,
            corner: None,
            time_s: None,
            issue: None,
            priority,
            hold_s: HOLD_S,
        }
    }
    fn dist(mut self, d: f32) -> Self {
        self.distance_m = Some(d);
        self
    }
    fn values(mut self, ours: f32, reference: f32) -> Self {
        self.ours = Some(ours);
        self.reference = Some(reference);
        self
    }
    fn corner(mut self, i: usize) -> Self {
        self.corner = Some(i as u32 + 1);
        self
    }
    fn hold(mut self, s: f64) -> Self {
        self.hold_s = s;
        self
    }
    fn same(&self, o: &Cue) -> bool {
        self.kind == o.kind && self.corner == o.corner
    }
}

/// The corner costing you most, averaged over your recent passes.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Focus {
    pub corner: u32,
    /// Average seconds lost per pass.
    pub loss_s: f32,
    pub issue: Option<Issue>,
    pub passes: u32,
}

/// Sent with every tick while a matching reference lap is loaded.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoachPayload {
    pub ref_speed: Option<f32>,
    pub ref_throttle: Option<f32>,
    pub ref_brake: Option<f32>,
    pub ref_gear: Option<i32>,
    /// Steering wheel angle (rad, positive left). Only .ibt references record
    /// steering; .blap/.olap files don't, so this is None for them.
    pub ref_steering: Option<f32>,
    /// Your speed minus the reference's, m/s.
    pub speed_delta: Option<f32>,
    pub cue: Option<Cue>,
    pub focus: Option<Focus>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CornerKind {
    Brake,
    Lift,
}

/// A corner in the reference lap, as bin indices round the lap (0..REF_BINS).
#[derive(Debug, Clone, Copy)]
pub struct Corner {
    pub kind: CornerKind,
    /// Brake point, or where the reference comes off full throttle.
    pub entry: usize,
    /// Last bin of braking (the entry, for lift corners).
    pub brake_end: usize,
    /// Slowest point.
    pub apex: usize,
    /// Back on the throttle: over 50% after braking, over 90% after a lift.
    pub pickup: usize,
    pub peak_brake: f32,
    /// Lowest throttle between entry and pickup.
    pub min_throttle: f32,
    /// Speed at the apex, when the reference has speed.
    pub min_speed: Option<f32>,
    pub apex_gear: i32,
}

/// Finds the corners in a reference lap: braking zones, and lifts that slow
/// the car. Ordered from the start line.
pub fn find_corners(r: &ReferenceLap) -> Vec<Corner> {
    if !r.has_inputs() || r.brake.len() != N + 1 || r.throttle.len() != N + 1 {
        return Vec::new();
    }
    let brake = |o: usize| r.brake[o % N];
    let thr = |o: usize| r.throttle[o % N];
    let has_speed = r.speed.len() == N + 1;
    let has_curv = r.curvature.len() == N + 1;
    let speed = |o: usize| if has_speed { r.speed[o % N] } else { 0.0 };
    let curv = |o: usize| if has_curv { r.curvature[o % N].abs() } else { 0.0 };

    // Scan from somewhere on a straight (the fastest point, or the middle of
    // the longest flat-out run), so no corner is cut in two by where we start.
    let s0 = {
        let full = |o: usize| thr(o) > 0.95 && brake(o) < 0.02;
        let (mut best, mut best_len, mut run) = (None, 0, 0);
        for o in 0..2 * N {
            run = if full(o) { run + 1 } else { 0 };
            if run > best_len && run <= N {
                best_len = run;
                best = Some(o + 1 - run + run / 2);
            }
        }
        match best {
            Some(b) => b % N,
            None if has_speed => (0..N).max_by(|&a, &b| speed(a).total_cmp(&speed(b))).unwrap_or(0),
            None => 0,
        }
    };
    let at = |o: usize| (s0 + o) % N;

    // Braking zones, as offsets from s0: (start, end, peak).
    let mut zones: Vec<(usize, usize, f32)> = Vec::new();
    let mut o = 0;
    while o < N {
        if brake(at(o)) > 0.10 {
            let start = o;
            let (mut end, mut peak, mut quiet) = (o, 0.0f32, 0);
            while o < N && quiet < 3 {
                let v = brake(at(o));
                peak = peak.max(v);
                if v < 0.05 {
                    quiet += 1;
                } else {
                    quiet = 0;
                    end = o;
                }
                o += 1;
            }
            // Merge with the previous zone if they're almost touching (trail braking pulses).
            if let Some(prev) = zones.last_mut() {
                if start <= prev.1 + 8 {
                    prev.1 = end;
                    prev.2 = prev.2.max(peak);
                    continue;
                }
            }
            zones.push((start, end, peak));
        } else {
            o += 1;
        }
    }
    // Light, short brushes of the brake aren't corners worth coaching.
    zones.retain(|z| z.2 > 0.2 && z.1 > z.0 + 1);

    let argmin_speed = |a: usize, b: usize, fallback: usize| {
        if has_speed {
            (a..b.max(a + 1)).min_by(|&x, &y| speed(at(x)).total_cmp(&speed(at(y)))).unwrap_or(fallback)
        } else if has_curv {
            (a..b.max(a + 1)).max_by(|&x, &y| curv(at(x)).total_cmp(&curv(at(y)))).unwrap_or(fallback)
        } else {
            fallback
        }
    };
    let min_thr = |a: usize, b: usize| (a..=b).map(|o| thr(at(o))).fold(1.0f32, f32::min);

    // (offset corner, offset where the reference is flat out again)
    let mut found: Vec<(usize, Corner, usize)> = Vec::new();
    for (k, &(start, end, peak)) in zones.iter().enumerate() {
        let limit = zones.get(k + 1).map(|z| z.0).unwrap_or(N).min(end + 120).min(N);
        let apex = argmin_speed(start, limit, end);
        let pickup = (apex..limit).find(|&o| thr(at(o)) > 0.5).unwrap_or(apex);
        let full = (pickup..N).find(|&o| thr(at(o)) >= 0.95).unwrap_or(limit);
        found.push((
            start,
            Corner {
                kind: CornerKind::Brake,
                entry: at(start),
                brake_end: at(end),
                apex: at(apex),
                pickup: at(pickup),
                peak_brake: peak,
                min_throttle: min_thr(start, pickup),
                min_speed: has_speed.then(|| speed(at(apex))),
                apex_gear: r.gear.get(at(apex)).map(|g| *g as i32).unwrap_or(0),
            },
            full,
        ));
    }

    // Lifts: off full throttle without braking, long enough and slowing the
    // car (or in a real bend), and not part of a braking corner's exit.
    let in_brake_corner = |o: usize| found.iter().any(|(s, _, full)| o >= *s && o <= *full);
    let lifting = |o: usize| brake(at(o)) < 0.05 && thr(at(o)) < 0.85;
    let mut lifts = Vec::new();
    let mut o = 1;
    while o < N {
        if lifting(o) && thr(at(o - 1)) >= 0.9 && !in_brake_corner(o) {
            let a = o;
            while o < N && lifting(o) {
                o += 1;
            }
            let b = o - 1;
            if b < a + 2 {
                continue;
            }
            let next = found.iter().map(|f| f.0).filter(|&s| s > b).min().unwrap_or(N);
            let limit = (b + 60).min(next).min(N);
            let apex = argmin_speed(a, limit, b);
            let lowest = min_thr(a, b);
            let slows = has_speed && speed(at(a)) - speed(at(apex)) >= 1.0;
            let bends = has_curv && (a.saturating_sub(20)..(b + 40).min(N)).any(|x| curv(at(x)) > 0.003);
            let real = if has_speed || has_curv { slows || bends } else { lowest < 0.5 };
            if real {
                let pickup = (apex.max(b)..limit.max(b + 1)).find(|&x| thr(at(x)) >= 0.9).unwrap_or(b + 1);
                lifts.push((
                    a,
                    Corner {
                        kind: CornerKind::Lift,
                        entry: at(a),
                        brake_end: at(a),
                        apex: at(apex),
                        pickup: at(pickup),
                        peak_brake: 0.0,
                        min_throttle: lowest,
                        min_speed: has_speed.then(|| speed(at(apex))),
                        apex_gear: r.gear.get(at(apex)).map(|g| *g as i32).unwrap_or(0),
                    },
                    pickup,
                ));
            }
        } else {
            o += 1;
        }
    }
    found.extend(lifts);

    let mut corners: Vec<Corner> = found.into_iter().map(|f| f.1).collect();
    corners.sort_by_key(|c| c.entry);
    corners
}

/// For each corner, the stretch of lap we watch it over: (start, end) bins,
/// from the approach to the end of the exit. Windows don't overlap.
fn windows(corners: &[Corner], m_per_bin: f32) -> Vec<(usize, usize)> {
    let n = corners.len();
    let approach = bins(APPROACH_M, m_per_bin).min(N / 3);
    let exits: Vec<usize> = (0..n)
        .map(|i| {
            let c = corners[i];
            let to_pickup = fwd(c.entry, c.pickup);
            let want = to_pickup + bins(EXIT_M, m_per_bin);
            let off = if n == 1 {
                want.min(N - approach - 1)
            } else {
                let to_next = fwd(c.entry, corners[(i + 1) % n].entry);
                let gap = to_next.saturating_sub(bins(MIN_GAP_M, m_per_bin));
                if gap > to_pickup {
                    want.min(gap)
                } else {
                    want.min(to_next.saturating_sub(1))
                }
            };
            add(c.entry, off.max(1))
        })
        .collect();
    (0..n)
        .map(|i| {
            let c = corners[i];
            let start = if n == 1 {
                sub(c.entry, approach)
            } else {
                let prev_exit = exits[(i + n - 1) % n];
                if fwd(prev_exit, c.entry) <= approach {
                    add(prev_exit, 1)
                } else {
                    sub(c.entry, approach)
                }
            };
            (start, exits[i])
        })
        .collect()
}

/// "Has this been true for long enough?"
#[derive(Default)]
struct Sustain {
    since: Option<f64>,
}

impl Sustain {
    fn check(&mut self, cond: bool, now: f64, min: f64) -> bool {
        if !cond {
            self.since = None;
            return false;
        }
        let since = *self.since.get_or_insert(now);
        now - since >= min
    }
}

/// Your recent record in one corner.
#[derive(Debug, Default, Clone, Copy)]
struct History {
    passes: u32,
    /// Seconds lost per pass, averaged with more weight on recent passes.
    loss: f32,
    last_issue: Option<Issue>,
}

/// One pass through a corner, recorded as it happens.
#[derive(Debug, Clone, Copy)]
struct Pass {
    corner: usize,
    /// Started at the beginning of the corner's window with a delta, and nothing went wrong since.
    valid: bool,
    last_bin: usize,
    delta_start: f32,
    delta_apex: Option<f32>,
    delta_last: f32,
    /// Where you first braked (lifted, in a lift corner), once you'd kept at it for a moment.
    brake_bin: Option<usize>,
    /// Where and when the current press started, before it counts.
    brake_try: Option<(usize, f64)>,
    peak_brake: f32,
    min_speed: f32,
    min_bin: usize,
    min_gear: i32,
    min_throttle: f32,
    pickup_bin: Option<usize>,
    apex_done: bool,
}

/// Offsets of a corner's points from the start of its window.
#[derive(Clone, Copy)]
struct Offsets {
    entry: usize,
    brake_end: usize,
    apex: usize,
    pickup: usize,
    exit: usize,
}

const HOLD_S: f64 = 0.9;
const VERDICT_HOLD_S: f64 = 2.5;

pub struct Coach {
    corners: Vec<Corner>,
    windows: Vec<(usize, usize)>,
    windows_len: f32,
    history: Vec<History>,
    pass: Option<Pass>,
    current: Option<(Cue, f64)>,
    /// A one-off cue (verdict, apex speed) waiting for its turn, with an expiry time.
    pending: Option<(Cue, f64)>,
    deltas: VecDeque<(f64, f32)>,
    // The engine's delta restarts at the line; this keeps it continuous.
    last_pct: Option<f32>,
    last_delta: Option<f32>,
    delta_offset: f32,
    press_hard: Sustain,
    press_light: Sustain,
    hold_brake: Sustain,
    release_brake: Sustain,
    lift_less: Sustain,
    gear_wrong: Sustain,
    more_throttle: Sustain,
    exit_slow: Sustain,
    turn: Sustain,
}

impl Coach {
    pub fn new(reference: &ReferenceLap) -> Self {
        let corners = find_corners(reference);
        Self {
            history: vec![History::default(); corners.len()],
            corners,
            windows: Vec::new(),
            windows_len: 0.0,
            pass: None,
            current: None,
            pending: None,
            deltas: VecDeque::new(),
            last_pct: None,
            last_delta: None,
            delta_offset: 0.0,
            press_hard: Sustain::default(),
            press_light: Sustain::default(),
            hold_brake: Sustain::default(),
            release_brake: Sustain::default(),
            lift_less: Sustain::default(),
            gear_wrong: Sustain::default(),
            more_throttle: Sustain::default(),
            exit_slow: Sustain::default(),
            turn: Sustain::default(),
        }
    }

    pub fn corners(&self) -> &[Corner] {
        &self.corners
    }

    /// `delta`: time versus the reference (positive = slower). `length_m`: lap length.
    pub fn update(&mut self, r: &ReferenceLap, f: &RawFrame, delta: Option<f32>, length_m: f32) -> CoachPayload {
        let p = f.lap_dist_pct.clamp(0.0, 1.0);
        let len = length_m.max(1.0);
        if (len - self.windows_len).abs() > 0.5 {
            self.windows = windows(&self.corners, len / N as f32);
            self.windows_len = len;
        }
        let ref_speed = r.speed_at(p);
        let delta = delta.map(|d| self.continuous(d, p));

        let off_track = usize::try_from(f.player_car_idx)
            .ok()
            .and_then(|i| f.car_track_surface.get(i))
            .is_some_and(|s| *s == track_surface::OFF_TRACK);
        let yellow = f.session_flags & (session_flags::YELLOW | session_flags::YELLOW_WAVING) != 0;
        let driving = f.is_on_track && !f.on_pit_road && f.speed > 3.0;

        let cue = if driving && !off_track && !yellow {
            self.track(f, p, delta, len);
            let candidate = self.pick(r, f, p, delta, len);
            Some(self.hold(candidate, f.session_time))
        } else {
            // Nothing to coach, and this pass can't be compared fairly.
            self.pass = None;
            self.current = None;
            None
        };

        CoachPayload {
            ref_speed,
            ref_throttle: r.throttle_at(p),
            ref_brake: r.brake_at(p),
            ref_gear: r.gear_at(p).filter(|g| *g > 0),
            ref_steering: r.steering_at(p),
            speed_delta: ref_speed.map(|s| f.speed - s),
            cue,
            focus: self.focus(),
        }
    }

    /// Delta that carries on through the start line instead of restarting.
    fn continuous(&mut self, d: f32, p: f32) -> f32 {
        if let (Some(lp), Some(ld)) = (self.last_pct, self.last_delta) {
            if lp > 0.9 && p < 0.1 {
                self.delta_offset += ld - d;
            }
        }
        self.last_pct = Some(p);
        self.last_delta = Some(d);
        d + self.delta_offset
    }

    fn corner_at(&self, bin: usize) -> Option<usize> {
        self.windows.iter().position(|&(s, e)| fwd(s, bin) <= fwd(s, e))
    }

    fn offsets(&self, i: usize) -> Offsets {
        let c = self.corners[i];
        let (w0, w1) = self.windows[i];
        Offsets {
            entry: fwd(w0, c.entry),
            brake_end: fwd(w0, c.brake_end),
            apex: fwd(w0, c.apex),
            pickup: fwd(w0, c.pickup),
            exit: fwd(w0, w1),
        }
    }

    /// Records the current pass through a corner, and closes it (with a verdict) on the way out.
    fn track(&mut self, f: &RawFrame, p: f32, delta: Option<f32>, len: f32) {
        let bin = bin_of(p);
        let m_per_bin = len / N as f32;
        let here = self.corner_at(bin);

        if let Some(ps) = &mut self.pass {
            let step = fwd(ps.last_bin, bin);
            // Backwards, or further than the car could go in a frame: a reset or a tow.
            if step > N / 2 || step as f32 * m_per_bin > (f.speed * 0.5).max(40.0) || delta.is_none() {
                ps.valid = false;
            }
        }
        if let Some(ps) = self.pass {
            if Some(ps.corner) != here {
                self.pass = None;
                let o = self.offsets(ps.corner);
                let (w0, _) = self.windows[ps.corner];
                if ps.valid && fwd(w0, ps.last_bin) + 2 >= o.exit {
                    self.finish(ps, m_per_bin, f.session_time);
                }
            }
        }
        let Some(i) = here else { return };
        let o = self.offsets(i);
        let (w0, _) = self.windows[i];
        let at = fwd(w0, bin);
        let ps = self.pass.get_or_insert(Pass {
            corner: i,
            valid: at <= 3 && delta.is_some(),
            last_bin: bin,
            delta_start: delta.unwrap_or(0.0),
            delta_apex: None,
            delta_last: delta.unwrap_or(0.0),
            brake_bin: None,
            brake_try: None,
            peak_brake: 0.0,
            min_speed: f32::MAX,
            min_bin: bin,
            min_gear: f.gear,
            min_throttle: 1.0,
            pickup_bin: None,
            apex_done: false,
        });
        ps.last_bin = bin;
        if let Some(d) = delta {
            ps.delta_last = d;
            if ps.delta_apex.is_none() && at >= o.apex {
                ps.delta_apex = Some(d);
            }
        }
        let c = self.corners[i];
        // Your brake (or lift) point: where a press that lasts at least 0.1 s began.
        let pressing = match c.kind {
            CornerKind::Brake => f.brake > 0.1,
            CornerKind::Lift => f.throttle < 0.85 || f.brake > 0.1,
        };
        if ps.brake_bin.is_none() && at <= o.apex && pressing {
            let (b, t0) = *ps.brake_try.get_or_insert((bin, f.session_time));
            if f.session_time - t0 >= 0.1 {
                ps.brake_bin = Some(b);
            }
        } else if !pressing {
            ps.brake_try = None;
        }
        ps.peak_brake = ps.peak_brake.max(f.brake);
        if at + bins(30.0, m_per_bin) >= o.entry && at <= o.pickup.max(o.apex) {
            ps.min_throttle = ps.min_throttle.min(f.throttle);
        }
        if f.speed < ps.min_speed && at + bins(30.0, m_per_bin) >= o.entry {
            ps.min_speed = f.speed;
            ps.min_bin = bin;
            ps.min_gear = f.gear;
            ps.pickup_bin = None;
        }
        let past_min = fwd(w0, bin) > fwd(w0, ps.min_bin);
        let pickup_level = if c.kind == CornerKind::Lift { 0.9 } else { 0.5 };
        if ps.pickup_bin.is_none() && past_min && f.throttle > pickup_level && f.brake < 0.05 && ps.min_speed < f32::MAX {
            ps.pickup_bin = Some(bin);
        }

        // Once you're clearly past your slowest point, say how it compared.
        if !ps.apex_done && ps.min_speed < f32::MAX && at + bins(40.0, m_per_bin) >= o.apex && f.speed > ps.min_speed + 1.0 {
            ps.apex_done = true;
            let quiet = self.history[i].passes >= 2 && self.history[i].loss < 0.03;
            if let (Some(rs), false) = (c.min_speed, quiet) {
                let diff = ps.min_speed - rs;
                let cue = if diff < -1.5 {
                    Some(Cue::new(CueKind::CarrySpeed, Tone::Warn, 5))
                } else if diff > 2.0 {
                    Some(Cue::new(CueKind::FastApex, Tone::Good, 3))
                } else {
                    None
                };
                if let Some(cue) = cue {
                    let cue = cue.values(ps.min_speed, rs).corner(i).hold(1.2);
                    self.pending = Some((cue, f.session_time + 1.5));
                }
            }
        }
    }

    /// Scores a finished pass, remembers it, and queues the verdict.
    fn finish(&mut self, ps: Pass, m_per_bin: f32, now: f64) {
        let i = ps.corner;
        let c = self.corners[i];
        let Some(d_apex) = ps.delta_apex else { return };
        if ps.min_speed == f32::MAX {
            return;
        }
        let entry_loss = d_apex - ps.delta_start;
        let exit_loss = ps.delta_last - d_apex;
        let total = entry_loss + exit_loss;
        let m = |b: i64| b as f32 * m_per_bin;
        let brake_off = ps.brake_bin.map(|b| m(signed(c.entry, b)));
        let pickup_off = ps.pickup_bin.map(|b| m(signed(c.pickup, b)));
        let speed_diff = c.min_speed.map(|s| ps.min_speed - s);
        let slow = speed_diff.is_some_and(|d| d < -1.5);
        let fast = speed_diff.is_some_and(|d| d > 2.0);

        let issue = if total <= -0.03 {
            Issue::Gained
        } else if entry_loss >= exit_loss {
            match c.kind {
                CornerKind::Brake => {
                    if brake_off.is_some_and(|d| d < -8.0) {
                        Issue::BrakedEarly
                    } else if brake_off.is_some_and(|d| d > 8.0) && slow {
                        Issue::BrakedLate
                    } else if slow {
                        Issue::SlowApex
                    } else if ps.peak_brake < c.peak_brake - 0.15 {
                        Issue::BrakeHarder
                    } else {
                        Issue::Entry
                    }
                }
                CornerKind::Lift => {
                    if ps.peak_brake > 0.2 || ps.min_throttle < c.min_throttle - 0.25 {
                        Issue::LiftedTooMuch
                    } else if brake_off.is_some_and(|d| d < -10.0) {
                        Issue::LiftedEarly
                    } else if slow {
                        Issue::SlowApex
                    } else {
                        Issue::Entry
                    }
                }
            }
        } else if pickup_off.is_some_and(|d| d > 10.0) {
            Issue::LatePickup
        } else if slow {
            Issue::SlowApex
        } else if fast {
            Issue::FastEntry
        } else if c.apex_gear >= 1 && ps.min_gear >= 1 && ps.min_gear != c.apex_gear {
            Issue::WrongGear
        } else {
            Issue::Exit
        };

        let h = &mut self.history[i];
        h.loss = if h.passes == 0 { total } else { 0.6 * h.loss + 0.4 * total };
        h.passes += 1;
        if issue != Issue::Gained {
            h.last_issue = Some(issue);
        }

        if total.abs() < 0.03 {
            return;
        }
        let tone = if issue == Issue::Gained { Tone::Good } else { Tone::Warn };
        let mut cue = Cue::new(CueKind::CornerVerdict, tone, 3).corner(i).hold(VERDICT_HOLD_S);
        cue.time_s = Some(total);
        cue.issue = Some(issue);
        match issue {
            Issue::BrakedEarly | Issue::BrakedLate | Issue::LiftedEarly => cue.distance_m = brake_off,
            Issue::LatePickup => cue.distance_m = pickup_off,
            Issue::SlowApex | Issue::FastEntry => {
                cue.ours = Some(ps.min_speed);
                cue.reference = c.min_speed;
            }
            Issue::BrakeHarder => cue = cue.values(ps.peak_brake, c.peak_brake),
            Issue::LiftedTooMuch => cue = cue.values(ps.min_throttle, c.min_throttle),
            Issue::WrongGear => {
                cue.ours = Some(ps.min_gear as f32);
                cue.gear = Some(c.apex_gear);
            }
            _ => {}
        }
        if issue == Issue::BrakedLate {
            cue.ours = Some(ps.min_speed);
            cue.reference = c.min_speed;
        }
        self.pending = Some((cue, now + 4.0));
    }

    /// The corner costing you most, once you've been through it.
    fn focus(&self) -> Option<Focus> {
        self.history
            .iter()
            .enumerate()
            .filter(|(_, h)| h.passes > 0 && h.loss > 0.05)
            .max_by(|a, b| a.1.loss.total_cmp(&b.1.loss))
            .map(|(i, h)| Focus { corner: i as u32 + 1, loss_s: h.loss, issue: h.last_issue, passes: h.passes })
    }

    /// Keeps the current cue on screen for a moment unless something more urgent comes up.
    fn hold(&mut self, candidate: Cue, now: f64) -> Cue {
        let shown = match self.current {
            Some((cur, since)) if cur.same(&candidate) => {
                self.current = Some((candidate, since));
                candidate
            }
            Some((cur, since)) if candidate.priority <= cur.priority && now - since < cur.hold_s => cur,
            _ => {
                self.current = Some((candidate, now));
                candidate
            }
        };
        if self.pending.is_some_and(|(p, _)| p.same(&shown)) {
            self.pending = None;
        }
        shown
    }

    fn pick(&mut self, r: &ReferenceLap, f: &RawFrame, p: f32, delta: Option<f32>, len: f32) -> Cue {
        let now = f.session_time;
        let bin = bin_of(p);
        let m_per_bin = len / N as f32;
        let m = |b: usize| b as f32 * m_per_bin;

        let ref_brake = r.brake_at(p).unwrap_or(0.0);
        let ref_thr = r.throttle_at(p).unwrap_or(0.0);
        let ref_speed = r.speed_at(p);
        let sd = ref_speed.map(|s| f.speed - s);
        let braking = f.brake > 0.1;

        let mut cands: Vec<Cue> = vec![Cue::new(CueKind::OnPace, Tone::Neutral, 0)];

        // Trend: time gained or lost over the last 2 seconds.
        if let Some(d) = delta {
            self.deltas.push_back((now, d));
            while self.deltas.front().is_some_and(|(t, _)| now - t > 2.0 || *t > now) {
                self.deltas.pop_front();
            }
            if let Some(&(t0, d0)) = self.deltas.front() {
                if now - t0 > 1.0 {
                    let change = d - d0;
                    if change < -0.04 {
                        cands.push(Cue::new(CueKind::Gaining, Tone::Good, 1).values(-change, 0.0));
                    } else if change > 0.04 {
                        cands.push(Cue::new(CueKind::Losing, Tone::Warn, 1).values(change, 0.0));
                    }
                }
            }
        }

        // A verdict or apex note waiting for a gap.
        match self.pending {
            Some((c, until)) if now <= until => cands.push(c),
            Some(_) => self.pending = None,
            None => {}
        }

        // Conditions that need to hold for a moment, worked out every frame so
        // their timers reset properly.
        let mut want_hard = false;
        let mut want_light = false;
        let mut want_hold = false;
        let mut want_release = false;
        let mut want_lift_less = false;
        let mut want_gear: Option<i32> = None;
        let mut turn_ratio: Option<f32> = None;
        // Your pressure and the reference's, from your brake point.
        let mut pressure: Option<(f32, f32)> = None;
        let mut corner_cues: Vec<Cue> = Vec::new();
        let mut minor_ok = true;
        let mut corner_idx = None;

        match self.pass {
            Some(ps) => {
                let i = ps.corner;
                corner_idx = Some(i);
                let c = self.corners[i];
                let o = self.offsets(i);
                let (w0, _) = self.windows[i];
                let at = fwd(w0, bin);
                let h = self.history[i];
                minor_ok = !(h.passes >= 2 && h.loss < 0.03);
                let lookahead = (f.speed * 1.6).max(60.0);

                match c.kind {
                    CornerKind::Brake => {
                        if at < o.entry {
                            let to_go = m(o.entry - at);
                            if braking {
                                if to_go > 8.0 {
                                    corner_cues.push(Cue::new(CueKind::BrakeEarly, Tone::Warn, 8).dist(to_go));
                                }
                            } else if to_go <= lookahead {
                                corner_cues.push(Cue::new(CueKind::BrakeSoon, Tone::Info, 4).dist(to_go));
                            }
                        } else if at <= o.brake_end && ps.brake_bin.is_none() && !braking {
                            let past = m(at - o.entry);
                            if past > 5.0 {
                                corner_cues.push(Cue::new(CueKind::BrakeNow, Tone::Bad, 9).dist(-past));
                            }
                        }
                        if let (Some(sb), true) = (ps.brake_bin, at <= o.apex) {
                            // The reference's pressure the same distance after its own brake point.
                            let since = fwd(sb, bin);
                            let ref_len = o.brake_end.saturating_sub(o.entry);
                            let aligned = if since <= ref_len { r.brake[(c.entry + since) % N] } else { 0.0 };
                            if braking && since <= ref_len / 2 + 1 && aligned > 0.3 {
                                let diff = f.brake - aligned;
                                want_hard = diff < -0.15;
                                want_light = diff > 0.15;
                                pressure = Some((f.brake, aligned));
                                if diff.abs() < 0.12 {
                                    corner_cues.push(Cue::new(CueKind::GoodBraking, Tone::Good, 2).values(f.brake, aligned));
                                }
                            }
                            // Trailing towards the apex is about position, not timing.
                            let in_ref_braking = at >= o.entry && at <= o.brake_end;
                            want_hold = f.brake < 0.03 && in_ref_braking && ref_brake > 0.25;
                            want_release = f.brake > 0.2 && ref_brake < 0.05 && aligned < 0.05 && at > o.brake_end;
                        }
                    }
                    CornerKind::Lift => {
                        if at < o.entry && !braking && f.throttle > 0.85 {
                            let to_go = m(o.entry - at);
                            if to_go <= lookahead {
                                corner_cues.push(Cue::new(CueKind::LiftSoon, Tone::Info, 4).dist(to_go).values(f.throttle, c.min_throttle));
                            }
                        }
                        if at + bins(10.0, m_per_bin) >= o.entry && at <= o.pickup {
                            if braking {
                                corner_cues.push(Cue::new(CueKind::NoBrakeHere, Tone::Warn, 7));
                            }
                            want_lift_less = !braking && f.throttle < ref_thr - 0.3;
                        }
                    }
                }

                // Back on the power.
                if at >= o.pickup && m(at - o.pickup) < 150.0 && ref_thr > 0.6 && f.throttle < 0.3 && !braking {
                    corner_cues.push(Cue::new(CueKind::ThrottleNow, Tone::Bad, 7).values(f.throttle, ref_thr));
                }

                // Gear around the apex only; shift points on straights vary too much to nag about.
                let near_apex = (at as i64 - o.apex as i64).unsigned_abs() as f32 * m_per_bin < 40.0;
                let ref_gear = r.gear_at(p).unwrap_or(0);
                if near_apex && ref_gear >= 1 && f.gear >= 1 && f.gear != ref_gear {
                    want_gear = Some(ref_gear);
                }

                // Line: how sharply you're turning against the reference, mid-corner.
                let from = if c.kind == CornerKind::Brake { o.brake_end } else { o.entry };
                if at >= from && at <= o.pickup.max(o.apex) && f.speed > 12.0 {
                    turn_ratio = line_ratio(r, f, p);
                }
            }
            None => {
                if braking {
                    cands.push(Cue::new(CueKind::NoBrakeHere, Tone::Warn, 7));
                }
            }
        }

        let hard = self.press_hard.check(want_hard, now, 0.15);
        let light = self.press_light.check(want_light, now, 0.15);
        let hold = self.hold_brake.check(want_hold, now, 0.15);
        let release = self.release_brake.check(want_release, now, 0.2);
        let lift_less = self.lift_less.check(want_lift_less, now, 0.3);
        let gear = self.gear_wrong.check(want_gear.is_some(), now, 0.4);
        let turn_off = turn_ratio.is_some_and(|q| !(0.75..=1.3).contains(&q));
        let turn = self.turn.check(turn_off, now, 0.4);

        if let (true, Some((ours, aligned))) = (hard || light, pressure) {
            let kind = if hard { CueKind::BrakeHarder } else { CueKind::BrakeLighter };
            corner_cues.push(Cue::new(kind, Tone::Warn, 6).values(ours, aligned));
        }
        if hold {
            corner_cues.push(Cue::new(CueKind::HoldBrake, Tone::Warn, 5).values(f.brake, ref_brake));
        }
        if release {
            corner_cues.push(Cue::new(CueKind::ReleaseBrake, Tone::Warn, 5).values(f.brake, ref_brake));
        }
        if lift_less {
            corner_cues.push(Cue::new(CueKind::LiftLess, Tone::Warn, 5).values(f.throttle, ref_thr));
        }
        if let (true, Some(g)) = (gear, want_gear) {
            let kind = if f.gear > g { CueKind::ShiftDown } else { CueKind::ShiftUp };
            let mut c = Cue::new(kind, Tone::Info, 4);
            c.gear = Some(g);
            corner_cues.push(c);
        }
        if let (true, Some(q)) = (turn, turn_ratio) {
            let kind = if q < 0.75 { CueKind::TurnMore } else { CueKind::TurnLess };
            corner_cues.push(Cue::new(kind, Tone::Info, 2).values(q, 1.0));
        }

        for c in corner_cues {
            // In corners where you're already on pace, only the urgent stuff.
            let minor = matches!(
                c.kind,
                CueKind::BrakeHarder
                    | CueKind::BrakeLighter
                    | CueKind::GoodBraking
                    | CueKind::HoldBrake
                    | CueKind::ReleaseBrake
                    | CueKind::LiftLess
                    | CueKind::ShiftDown
                    | CueKind::ShiftUp
                    | CueKind::TurnMore
                    | CueKind::TurnLess
            );
            if minor && !minor_ok {
                continue;
            }
            cands.push(match corner_idx {
                Some(i) => c.corner(i),
                None => c,
            });
        }

        // Generally less throttle than the reference where it's flat out.
        if self.more_throttle.check(ref_thr > 0.9 && f.throttle < ref_thr - 0.35 && !braking, now, 0.4) {
            cands.push(Cue::new(CueKind::MoreThrottle, Tone::Warn, 5).values(f.throttle, ref_thr));
        }

        // Down on speed on a straight: the exit cost you.
        let straight = ref_thr > 0.95 && ref_brake < 0.02;
        if let (true, Some(rs)) = (self.exit_slow.check(straight && sd.is_some_and(|d| d < -2.5), now, 0.5), ref_speed) {
            cands.push(Cue::new(CueKind::BetterExit, Tone::Warn, 3).values(f.speed, rs));
        }

        // Highest priority wins; on a tie, the first offered.
        let mut best = cands[0];
        for c in cands {
            if c.priority > best.priority {
                best = c;
            }
        }
        best
    }
}

/// How sharply you're turning compared with the reference here (1 = the same),
/// from curvature when the reference has it, else steering wheel angle.
fn line_ratio(r: &ReferenceLap, f: &RawFrame, p: f32) -> Option<f32> {
    if let Some(k) = r.curvature_at(p) {
        // Signs aren't compared: heading-based and yaw-based curvature can differ in convention.
        let ours = (f.yaw_rate / f.speed.max(1.0)).abs();
        return (k.abs() > 0.004).then(|| ours / k.abs());
    }
    let rs = r.steering_at(p)?;
    let corner = rs.abs() > 0.15;
    (corner && f.steering.signum() == rs.signum()).then(|| f.steering.abs() / rs.abs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{RefMeta, RefSample};

    const LEN: f32 = 1000.0;

    /// 1 km "lap" at 50 m/s with one corner: brake 30-36% (hard at first, trailing
    /// off), slowest at 40%, throttle back at 44%.
    fn samples() -> Vec<RefSample> {
        (0..=1000)
            .map(|i| {
                let pct = i as f32 / 1000.0;
                let braking = (0.30..0.36).contains(&pct);
                let coasting = (0.36..0.44).contains(&pct);
                let speed = if (0.30..0.48).contains(&pct) { 50.0 - 25.0 * (1.0 - ((pct - 0.40) / 0.08).abs()).max(0.0) } else { 50.0 };
                RefSample {
                    pct,
                    time: 0.0,
                    speed: Some(speed),
                    throttle: if braking || coasting { 0.0 } else { 1.0 },
                    brake: if braking { 1.0 - (pct - 0.30) / 0.06 * 0.6 } else { 0.0 },
                    gear: if (0.33..0.45).contains(&pct) { 3 } else { 5 },
                    ..Default::default()
                }
            })
            .collect()
    }

    /// Fills in time from speed so the reference is self-consistent.
    fn build(mut s: Vec<RefSample>) -> ReferenceLap {
        let mut t = 0.0;
        for k in 0..s.len() {
            if k > 0 {
                let v = (s[k].speed.unwrap() + s[k - 1].speed.unwrap()) / 2.0;
                t += (s[k].pct - s[k - 1].pct) * LEN / v;
            }
            s[k].time = t;
        }
        let meta = RefMeta { track_name: "T".into(), length_m: LEN, lap_time: t, ..Default::default() };
        ReferenceLap::build(meta, &s).unwrap()
    }

    fn reference() -> ReferenceLap {
        build(samples())
    }

    fn frame(pct: f32, t: f64, throttle: f32, brake: f32, speed: f32, gear: i32) -> RawFrame {
        let mut f = RawFrame::with_capacity();
        f.is_on_track = true;
        f.lap_dist_pct = pct;
        f.session_time = t;
        f.throttle = throttle;
        f.brake = brake;
        f.speed = speed;
        f.gear = gear;
        f
    }

    /// What a driver does at each point: (throttle, brake, speed, gear).
    type Driver<'a> = &'a dyn Fn(f32) -> (f32, f32, f32, i32);

    /// Drives from `from` for `laps` laps at 60 Hz, with a live delta like the engine's.
    fn drive(c: &mut Coach, r: &ReferenceLap, from: f32, laps: f32, driver: Driver) -> Vec<(f32, CoachPayload)> {
        let dt = 1.0 / 60.0;
        let mut pos = from as f64;
        let mut lap_time = r.time_at(from);
        let mut t = 0.0;
        let mut out = Vec::new();
        while pos < (from + laps) as f64 {
            let p = pos.fract() as f32;
            let (thr, brk, v, g) = driver(p);
            let f = frame(p, t, thr, brk, v, g);
            let d = r.delta(p, lap_time);
            out.push((p, c.update(r, &f, Some(d), LEN)));
            let next = pos + (v * dt / LEN) as f64;
            if next.floor() > pos.floor() {
                lap_time = 0.0;
            }
            pos = next;
            lap_time += dt;
            t += dt as f64;
        }
        out
    }

    /// Exactly the reference.
    fn copy(r: &ReferenceLap) -> impl Fn(f32) -> (f32, f32, f32, i32) + '_ {
        move |p| (r.throttle_at(p).unwrap(), r.brake_at(p).unwrap(), r.speed_at(p).unwrap(), r.gear_at(p).unwrap())
    }

    fn kinds(out: &[(f32, CoachPayload)]) -> Vec<CueKind> {
        out.iter().filter_map(|(_, c)| c.cue.map(|q| q.kind)).collect()
    }

    fn verdicts(out: &[(f32, CoachPayload)]) -> Vec<Cue> {
        let mut v: Vec<Cue> = Vec::new();
        for c in out.iter().filter_map(|(_, c)| c.cue).filter(|c| c.kind == CueKind::CornerVerdict) {
            if v.last().is_none_or(|l| !l.same(&c)) {
                v.push(c);
            }
        }
        v
    }

    #[test]
    fn finds_the_corner() {
        let z = find_corners(&reference());
        assert_eq!(z.len(), 1);
        assert_eq!(z[0].kind, CornerKind::Brake);
        assert!((299..=301).contains(&z[0].entry), "{:?}", z[0]);
        assert!((399..=401).contains(&z[0].apex), "{:?}", z[0]);
        assert!((439..=441).contains(&z[0].pickup), "{:?}", z[0]);
        assert_eq!(z[0].apex_gear, 3);
    }

    #[test]
    fn finds_lift_corners() {
        // Add a lift to 50% throttle at 70-74%, dipping to 44 m/s at 74%.
        let mut s = samples();
        for x in s.iter_mut() {
            if (0.70..0.74).contains(&x.pct) {
                x.throttle = 0.5;
            }
            if (0.68..0.80).contains(&x.pct) {
                x.speed = Some(50.0 - 6.0 * (1.0 - ((x.pct - 0.74) / 0.06).abs()).max(0.0));
            }
        }
        let z = find_corners(&build(s));
        assert_eq!(z.len(), 2, "{z:?}");
        assert_eq!(z[1].kind, CornerKind::Lift);
        assert!((699..=701).contains(&z[1].entry), "{:?}", z[1]);
        assert!((739..=741).contains(&z[1].apex), "{:?}", z[1]);
        assert!((z[1].min_throttle - 0.5).abs() < 0.01);
    }

    #[test]
    fn corner_across_the_line_is_one_corner() {
        // Shift everything round so braking starts at 97% and the apex is at 7%.
        let s: Vec<RefSample> = samples();
        let shifted: Vec<RefSample> = (0..=1000)
            .map(|i| {
                let src = s[(i + 330) % 1000];
                RefSample { pct: i as f32 / 1000.0, ..src }
            })
            .collect();
        let r = build(shifted);
        let z = find_corners(&r);
        assert_eq!(z.len(), 1, "{z:?}");
        assert!((969..=971).contains(&z[0].entry), "{:?}", z[0]);
        assert!((69..=71).contains(&z[0].apex), "{:?}", z[0]);
        assert!((109..=111).contains(&z[0].pickup), "{:?}", z[0]);
    }

    #[test]
    fn copying_the_reference_is_quiet() {
        let r = reference();
        let mut c = Coach::new(&r);
        let out = drive(&mut c, &r, 0.0, 2.0, &copy(&r));
        for k in kinds(&out) {
            assert!(
                matches!(k, CueKind::OnPace | CueKind::BrakeSoon | CueKind::GoodBraking),
                "unexpected {k:?}"
            );
        }
        assert!(kinds(&out).contains(&CueKind::BrakeSoon));
        assert!(verdicts(&out).is_empty());
        assert!(out.last().unwrap().1.focus.is_none());
    }

    #[test]
    fn braking_late_is_not_called_light_braking() {
        // Same pressure trace as the reference, 20 m later.
        let r = reference();
        let mut c = Coach::new(&r);
        let late = |p: f32| {
            let q = if (0.30..0.40).contains(&p) { p - 0.02 } else { p };
            let (t, b, _, g) = copy(&r)(q);
            (t, b, r.speed_at(p).unwrap(), g)
        };
        let out = drive(&mut c, &r, 0.2, 0.3, &late);
        let mut k = kinds(&out);
        k.dedup();
        assert!(k.contains(&CueKind::BrakeNow), "{k:?}");
        assert!(!k.contains(&CueKind::BrakeLighter), "{k:?}");
        assert!(!k.contains(&CueKind::BrakeHarder), "{k:?}");
    }

    #[test]
    fn light_braking_is_caught() {
        let r = reference();
        let mut c = Coach::new(&r);
        let light = |p: f32| {
            let (t, b, v, g) = copy(&r)(p);
            (t, b * 0.5, v, g)
        };
        let out = drive(&mut c, &r, 0.2, 0.3, &light);
        assert!(kinds(&out).contains(&CueKind::BrakeHarder));
    }

    #[test]
    fn early_braking_gets_a_verdict_and_becomes_the_focus() {
        // Brakes 40 m early with the same trace, and is 15% slower until the apex.
        let r = reference();
        let mut c = Coach::new(&r);
        let early = |p: f32| {
            let q = if (0.26..0.40).contains(&p) { (p + 0.04).min(0.40) } else { p };
            let (t, b, _, g) = copy(&r)(q);
            let v = r.speed_at(p).unwrap() * if (0.26..0.40).contains(&p) { 0.85 } else { 1.0 };
            (t, b, v, g)
        };
        let out = drive(&mut c, &r, 0.0, 0.9, &early);
        assert!(kinds(&out).contains(&CueKind::BrakeEarly));
        let v = verdicts(&out);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].issue, Some(Issue::BrakedEarly));
        assert_eq!(v[0].corner, Some(1));
        assert!(v[0].time_s.unwrap() > 0.1, "{v:?}");
        assert!((v[0].distance_m.unwrap() + 40.0).abs() < 3.0, "{v:?}");
        let focus = out.last().unwrap().1.focus.unwrap();
        assert_eq!(focus.corner, 1);
        assert_eq!(focus.issue, Some(Issue::BrakedEarly));
    }

    #[test]
    fn late_throttle_gets_an_exit_verdict() {
        // Picks up the throttle 30 m late and is 15% slower until the straight.
        let r = reference();
        let mut c = Coach::new(&r);
        let lazy = |p: f32| {
            let (mut t, b, mut v, g) = copy(&r)(p);
            if (0.40..0.47).contains(&p) {
                t = 0.0;
            }
            if (0.42..0.56).contains(&p) {
                v *= 0.85;
            }
            (t, b, v, g)
        };
        let out = drive(&mut c, &r, 0.0, 0.9, &lazy);
        assert!(kinds(&out).contains(&CueKind::ThrottleNow));
        let v = verdicts(&out);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].issue, Some(Issue::LatePickup), "{v:?}");
        assert!((v[0].distance_m.unwrap() - 30.0).abs() < 3.0, "{v:?}");
    }

    #[test]
    fn verdict_works_across_the_line() {
        // The corner straddles the line; brake 30 m early and lose time.
        let s = samples();
        let shifted: Vec<RefSample> = (0..=1000).map(|i| RefSample { pct: i as f32 / 1000.0, ..s[(i + 330) % 1000] }).collect();
        let r = build(shifted);
        let mut c = Coach::new(&r);
        let early = |p: f32| {
            let in_entry = !(0.07..0.94).contains(&p);
            let q = if in_entry { (p + 0.03).fract() } else { p };
            let q = if p < 0.07 && q > 0.07 { 0.07 } else { q };
            let (t, b, _, g) = copy(&r)(q);
            let v = r.speed_at(p).unwrap() * if in_entry { 0.85 } else { 1.0 };
            (t, b, v, g)
        };
        let out = drive(&mut c, &r, 0.7, 0.6, &early);
        let v = verdicts(&out);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].issue, Some(Issue::BrakedEarly), "{v:?}");
        assert!(v[0].time_s.unwrap() > 0.05 && v[0].time_s.unwrap() < 2.0, "{v:?}");
    }

    #[test]
    fn steering_cues_only_with_recorded_steering() {
        // No steering or curvature in the reference (like an old .blap): never a turn cue.
        let r = reference();
        let mut c = Coach::new(&r);
        for i in 0..60 {
            let mut f = frame(0.41, i as f64 / 60.0, 0.2, 0.0, 25.0, 3);
            f.steering = 0.1;
            let k = c.update(&r, &f, None, LEN).cue.unwrap().kind;
            assert!(k != CueKind::TurnMore && k != CueKind::TurnLess);
        }
        // Same reference with 0.6 rad of lock recorded through the corner (like an .ibt).
        let mut r = reference();
        r.steering = (0..=REF_BINS).map(|k| if (360..=440).contains(&k) { 0.6 } else { 0.0 }).collect();
        let mut c = Coach::new(&r);
        let mut seen = false;
        for i in 0..60 {
            let mut f = frame(0.41, i as f64 / 60.0, 0.0, 0.0, r.speed_at(0.41).unwrap(), 3);
            f.steering = 0.2;
            seen |= c.update(&r, &f, None, LEN).cue.unwrap().kind == CueKind::TurnMore;
        }
        assert!(seen);
    }

    #[test]
    fn curvature_drives_line_cues() {
        // Reference turning on a 50 m radius through the corner; you on a 100 m one.
        let mut r = reference();
        r.curvature = (0..=REF_BINS).map(|k| if (360..=440).contains(&k) { 0.02 } else { 0.0 }).collect();
        let mut c = Coach::new(&r);
        let mut seen = false;
        for i in 0..60 {
            let v = r.speed_at(0.41).unwrap();
            let mut f = frame(0.41, i as f64 / 60.0, 0.0, 0.0, v, 3);
            f.yaw_rate = -v * 0.01; // opposite sign convention on purpose
            seen |= c.update(&r, &f, None, LEN).cue.unwrap().kind == CueKind::TurnMore;
        }
        assert!(seen);
    }

    #[test]
    fn verdict_is_held_then_cleared() {
        let r = reference();
        let mut c = Coach::new(&r);
        let mut v = Cue::new(CueKind::CornerVerdict, Tone::Warn, 3).corner(0).hold(VERDICT_HOLD_S);
        v.time_s = Some(0.2);
        c.pending = Some((v, 4.0));
        // On the straight, nothing else to say: the verdict shows and stays up.
        let k = |c: &mut Coach, t: f64| c.update(&r, &frame(0.6, t, 1.0, 0.0, 50.0, 5), Some(0.0), LEN).cue.unwrap().kind;
        assert_eq!(k(&mut c, 0.0), CueKind::CornerVerdict);
        assert_eq!(k(&mut c, 2.0), CueKind::CornerVerdict);
        assert_eq!(k(&mut c, 3.0), CueKind::OnPace);
    }

    #[test]
    fn no_cues_under_yellow_or_off_track() {
        let r = reference();
        let mut c = Coach::new(&r);
        let mut f = frame(0.33, 0.0, 1.0, 0.0, 50.0, 5);
        f.session_flags = session_flags::YELLOW;
        assert!(c.update(&r, &f, Some(0.0), LEN).cue.is_none());
        let mut f = frame(0.33, 0.1, 1.0, 0.0, 50.0, 5);
        f.car_track_surface[0] = track_surface::OFF_TRACK;
        assert!(c.update(&r, &f, Some(0.0), LEN).cue.is_none());
    }
}
