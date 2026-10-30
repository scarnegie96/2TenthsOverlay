//! Live coaching against an imported reference lap.
//!
//! On load, the reference is broken into corners: where it starts braking,
//! stops braking, reaches its slowest point (apex) and gets back on the
//! throttle. Each frame we compare what you're doing at the same spot and
//! pick the single most useful thing to say. Cues are held briefly so the
//! label doesn't flicker; a more urgent cue can always interrupt.

use std::collections::VecDeque;

use serde::Serialize;

use crate::model::RawFrame;
use crate::reference::{ReferenceLap, REF_BINS};

/// What the coach is telling you right now.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CueKind {
    /// The reference starts braking soon (countdown).
    BrakeSoon,
    /// The reference is already braking and you aren't.
    BrakeNow,
    /// You're braking before the reference does.
    BrakeEarly,
    /// You're braking where the reference doesn't brake at all.
    NoBrakeHere,
    BrakeHarder,
    BrakeLighter,
    GoodBraking,
    /// Slower than the reference through the slowest part of the corner.
    CarrySpeed,
    /// Faster than the reference through the slowest part of the corner.
    FastApex,
    /// The reference is back on the throttle and you aren't.
    ThrottleNow,
    MoreThrottle,
    ShiftDown,
    ShiftUp,
    /// Down on speed on a straight.
    BetterExit,
    /// The reference turns the car more sharply here.
    TurnMore,
    TurnLess,
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

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    pub kind: CueKind,
    pub tone: Tone,
    /// Metres to (positive) or past (negative) the reference's point, where relevant.
    pub distance_m: Option<f32>,
    /// Your value and the reference's: pedal 0..1 or speed m/s depending on the cue.
    pub ours: Option<f32>,
    pub reference: Option<f32>,
    pub gear: Option<i32>,
    /// Corner number (by braking zone), when the cue is about a corner.
    pub corner: Option<u32>,
    #[serde(skip)]
    priority: u8,
}

impl Cue {
    fn new(kind: CueKind, tone: Tone, priority: u8) -> Self {
        Self { kind, tone, distance_m: None, ours: None, reference: None, gear: None, corner: None, priority }
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
    fn corner(mut self, c: Option<usize>) -> Self {
        self.corner = c.map(|i| i as u32 + 1);
        self
    }
}

/// Sent with every tick while a matching reference lap is loaded.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoachPayload {
    pub ref_speed: Option<f32>,
    pub ref_throttle: Option<f32>,
    pub ref_brake: Option<f32>,
    pub ref_gear: Option<i32>,
    /// Steering wheel angle (rad, positive left). Measured for .ibt references,
    /// estimated from the reference's line for .blap/.olap.
    pub ref_steering: Option<f32>,
    pub steering_estimated: bool,
    /// Your speed minus the reference's, m/s.
    pub speed_delta: Option<f32>,
    pub cue: Option<Cue>,
}

/// A corner in the reference lap, as bin indices round the lap.
#[derive(Debug, Clone, Copy)]
pub struct Zone {
    pub brake_start: usize,
    pub brake_end: usize,
    /// Slowest point.
    pub apex: usize,
    /// Back on the throttle (over 50%).
    pub pickup: usize,
    pub peak_brake: f32,
}

/// Finds braking zones and what follows them.
pub fn find_zones(r: &ReferenceLap) -> Vec<Zone> {
    if !r.has_inputs() || r.speed.is_empty() {
        return Vec::new();
    }
    let n = REF_BINS + 1;
    let mut zones: Vec<Zone> = Vec::new();
    let mut i = 0;
    while i < n {
        if r.brake[i] > 0.10 {
            let start = i;
            let mut peak = 0.0f32;
            let mut quiet = 0;
            let mut end = i;
            while i < n && quiet < 3 {
                peak = peak.max(r.brake[i]);
                if r.brake[i] < 0.05 {
                    quiet += 1;
                } else {
                    quiet = 0;
                    end = i;
                }
                i += 1;
            }
            // Merge with the previous zone if they're almost touching (trail braking pulses).
            if let Some(prev) = zones.last_mut() {
                if start <= prev.brake_end + 8 {
                    prev.brake_end = end;
                    prev.peak_brake = prev.peak_brake.max(peak);
                    continue;
                }
            }
            zones.push(Zone { brake_start: start, brake_end: end, apex: end, pickup: end, peak_brake: peak });
        } else {
            i += 1;
        }
    }
    // Light, short brushes of the brake aren't corners worth coaching.
    zones.retain(|z| z.peak_brake > 0.2 && z.brake_end > z.brake_start + 1);

    let starts: Vec<usize> = zones.iter().map(|z| z.brake_start).collect();
    for (k, z) in zones.iter_mut().enumerate() {
        let limit = starts.get(k + 1).copied().unwrap_or(n).min(z.brake_end + 120);
        let (mut apex, mut min) = (z.brake_end, f32::MAX);
        for b in z.brake_start..limit {
            if r.speed[b] < min {
                min = r.speed[b];
                apex = b;
            }
        }
        z.apex = apex;
        z.pickup = (apex..limit).find(|&b| r.throttle[b] > 0.5).unwrap_or(apex);
    }
    zones
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

/// Learns how far your wheel turns for a given cornering curvature, so an
/// estimated ghost wheel can be drawn for references without steering data.
#[derive(Default)]
struct SteerCalibration {
    sxx: f64,
    sxy: f64,
    n: u32,
    /// Agreement between our curvature and the reference's (sign convention check).
    sign_sum: f64,
}

impl SteerCalibration {
    fn learn(&mut self, curvature: f32, steering: f32) {
        // Forget slowly so a car change doesn't stick forever.
        self.sxx = self.sxx * 0.999 + (curvature * curvature) as f64;
        self.sxy = self.sxy * 0.999 + (curvature * steering) as f64;
        self.n = self.n.saturating_add(1);
    }
    fn ratio(&self) -> Option<f32> {
        (self.n >= 120 && self.sxx > 1e-9).then(|| (self.sxy / self.sxx) as f32)
    }
    fn sign(&self) -> f32 {
        if self.sign_sum < 0.0 { -1.0 } else { 1.0 }
    }
}

const HOLD_S: f64 = 0.9;

pub struct Coach {
    zones: Vec<Zone>,
    current: Option<(Cue, f64)>,
    calib: SteerCalibration,
    deltas: VecDeque<(f64, f32)>,
    gear_wrong: Sustain,
    more_throttle: Sustain,
    exit_slow: Sustain,
    turn: Sustain,
}

impl Coach {
    pub fn new(reference: &ReferenceLap) -> Self {
        Self {
            zones: find_zones(reference),
            current: None,
            calib: SteerCalibration::default(),
            deltas: VecDeque::new(),
            gear_wrong: Sustain::default(),
            more_throttle: Sustain::default(),
            exit_slow: Sustain::default(),
            turn: Sustain::default(),
        }
    }

    pub fn zones(&self) -> &[Zone] {
        &self.zones
    }

    /// `delta`: time versus the reference (positive = slower). `length_m`: lap length.
    pub fn update(&mut self, r: &ReferenceLap, f: &RawFrame, delta: Option<f32>, length_m: f32) -> CoachPayload {
        let p = f.lap_dist_pct.clamp(0.0, 1.0);
        let ref_speed = r.speed_at(p);
        let ref_curv = r.curvature_at(p);

        // Our own turning rate, for the steering estimate and turn cues.
        let our_curv = (f.speed > 12.0).then(|| f.yaw_rate / f.speed);
        if let (Some(c), Some(rc)) = (our_curv, ref_curv) {
            if c.abs() > 0.002 && f.steering.abs() < 6.0 {
                self.calib.learn(c, f.steering);
            }
            if c.abs() > 0.004 && rc.abs() > 0.004 {
                self.calib.sign_sum = (self.calib.sign_sum * 0.999 + (c * rc).signum() as f64).clamp(-500.0, 500.0);
            }
        }
        let (ref_steering, estimated) = match r.steering_at(p) {
            Some(s) => (Some(s), false),
            None => (self.calib.ratio().zip(ref_curv).map(|(k, rc)| k * rc * self.calib.sign()), true),
        };

        let cue = if f.is_on_track && !f.on_pit_road && f.speed > 3.0 {
            let candidate = self.pick(r, f, p, delta, length_m.max(1.0), our_curv);
            Some(self.hold(candidate, f.session_time))
        } else {
            self.current = None;
            None
        };

        CoachPayload {
            ref_speed,
            ref_throttle: r.throttle_at(p),
            ref_brake: r.brake_at(p),
            ref_gear: r.gear_at(p).filter(|g| *g > 0),
            ref_steering,
            steering_estimated: estimated,
            speed_delta: ref_speed.map(|s| f.speed - s),
            cue,
        }
    }

    /// Keeps the current cue on screen for a moment unless something more urgent comes up.
    fn hold(&mut self, candidate: Cue, now: f64) -> Cue {
        match self.current {
            Some((cur, since)) if cur.kind == candidate.kind => {
                self.current = Some((candidate, since));
                candidate
            }
            Some((cur, since)) if candidate.priority <= cur.priority && now - since < HOLD_S => cur,
            _ => {
                self.current = Some((candidate, now));
                candidate
            }
        }
    }

    fn pick(&mut self, r: &ReferenceLap, f: &RawFrame, p: f32, delta: Option<f32>, len: f32, our_curv: Option<f32>) -> Cue {
        let now = f.session_time;
        let bin = (p * REF_BINS as f32).round() as usize;
        let m_per_bin = len / REF_BINS as f32;
        let ahead_m = |b: usize| ((b as f32 / REF_BINS as f32 - p).rem_euclid(1.0)) * len;
        let behind_m = |b: usize| ((p - b as f32 / REF_BINS as f32).rem_euclid(1.0)) * len;

        let ours_brake = f.brake;
        let ours_thr = f.throttle;
        let ref_brake = r.brake_at(p).unwrap_or(0.0);
        let ref_thr = r.throttle_at(p).unwrap_or(0.0);
        let ref_speed = r.speed_at(p);
        let sd = ref_speed.map(|s| f.speed - s);
        let braking = ours_brake > 0.1;

        // Which corner are we in (from brake point to throttle pickup), and which is next?
        // A corner runs from the brake point to 150 m after the throttle pickup.
        let exit_bins = (150.0 / m_per_bin).ceil() as usize;
        let in_zone = self
            .zones
            .iter()
            .position(|z| bin + 1 >= z.brake_start && bin <= z.pickup.max(z.brake_end) + exit_bins);
        let next = self
            .zones
            .iter()
            .enumerate()
            .min_by(|a, b| ahead_m(a.1.brake_start).total_cmp(&ahead_m(b.1.brake_start)));

        let mut best = Cue::new(CueKind::OnPace, Tone::Neutral, 0);
        let mut offer = |c: Cue| {
            if c.priority > best.priority {
                best = c;
            }
        };

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
                        offer(Cue::new(CueKind::Gaining, Tone::Good, 1).values(-change, 0.0));
                    } else if change > 0.04 {
                        offer(Cue::new(CueKind::Losing, Tone::Warn, 1).values(change, 0.0));
                    }
                }
            }
        }

        if let Some(z) = in_zone.map(|i| (i, self.zones[i])) {
            let (zi, z) = z;
            let in_braking = bin + 1 >= z.brake_start && bin <= z.brake_end;
            let past = behind_m(z.brake_start);
            if in_braking && !braking && past > 5.0 && past < (z.brake_end - z.brake_start) as f32 * m_per_bin {
                offer(Cue::new(CueKind::BrakeNow, Tone::Bad, 9).dist(-past).corner(Some(zi)));
            }
            if in_braking && braking && ref_brake > 0.1 {
                let diff = ours_brake - ref_brake;
                let c = if diff < -0.15 {
                    Cue::new(CueKind::BrakeHarder, Tone::Warn, 6)
                } else if diff > 0.15 {
                    Cue::new(CueKind::BrakeLighter, Tone::Warn, 6)
                } else {
                    Cue::new(CueKind::GoodBraking, Tone::Good, 2)
                };
                offer(c.values(ours_brake, ref_brake).corner(Some(zi)));
            }
            // Around the slowest point.
            let near_apex = (bin as i64 - z.apex as i64).unsigned_abs() as f32 * m_per_bin < 35.0;
            if let (true, Some(sd), Some(rs)) = (near_apex, sd, ref_speed) {
                if sd < -1.5 {
                    offer(Cue::new(CueKind::CarrySpeed, Tone::Warn, 5).values(f.speed, rs).corner(Some(zi)));
                } else if sd > 2.5 {
                    offer(Cue::new(CueKind::FastApex, Tone::Good, 3).values(f.speed, rs).corner(Some(zi)));
                }
            }
            // Getting back on the power.
            let after_pickup = bin >= z.pickup && (bin - z.pickup) as f32 * m_per_bin < 150.0;
            if after_pickup && ref_thr > 0.6 && ours_thr < 0.3 && !braking {
                offer(Cue::new(CueKind::ThrottleNow, Tone::Bad, 7).values(ours_thr, ref_thr).corner(Some(zi)));
            }
        } else if braking {
            // Braking outside any of the reference's corners.
            match next {
                Some((ni, z)) if ahead_m(z.brake_start) < 250.0 => {
                    offer(Cue::new(CueKind::BrakeEarly, Tone::Warn, 8).dist(ahead_m(z.brake_start)).corner(Some(ni)))
                }
                _ => offer(Cue::new(CueKind::NoBrakeHere, Tone::Warn, 7)),
            }
        }

        // Countdown to the next braking point.
        if let Some((ni, z)) = next {
            let d = ahead_m(z.brake_start);
            let lookahead = (f.speed * 1.6).max(60.0);
            if !braking && d > 0.0 && d <= lookahead && in_zone != Some(ni) {
                offer(Cue::new(CueKind::BrakeSoon, Tone::Info, 4).dist(d).corner(Some(ni)));
            }
        }

        // Generally less throttle than the reference where it's flat out.
        if self.more_throttle.check(ref_thr > 0.9 && ours_thr < ref_thr - 0.35 && !braking, now, 0.4) {
            offer(Cue::new(CueKind::MoreThrottle, Tone::Warn, 5).values(ours_thr, ref_thr));
        }

        // Gear choice.
        let ref_gear = r.gear_at(p).unwrap_or(0);
        let wrong = ref_gear >= 2 && f.gear >= 1 && f.gear != ref_gear;
        if self.gear_wrong.check(wrong, now, 0.6) {
            let kind = if f.gear > ref_gear { CueKind::ShiftDown } else { CueKind::ShiftUp };
            let mut c = Cue::new(kind, Tone::Info, 4);
            c.gear = Some(ref_gear);
            offer(c);
        }

        // Down on speed on a straight: the exit cost you.
        let straight = ref_thr > 0.95 && ref_brake < 0.02;
        if let (true, Some(rs)) = (self.exit_slow.check(straight && sd.is_some_and(|d| d < -2.5), now, 0.5), ref_speed) {
            offer(Cue::new(CueKind::BetterExit, Tone::Warn, 3).values(f.speed, rs));
        }

        // How sharply we turn versus the reference, in proper corners.
        if let (Some(c), Some(rc)) = (our_curv, r.curvature_at(p)) {
            let rc = rc * self.calib.sign();
            let ratio = if rc.abs() > 0.008 { c / rc } else { 1.0 };
            let off = rc.abs() > 0.008 && !(0.75..=1.3).contains(&ratio);
            if self.turn.check(off, now, 0.4) {
                let kind = if ratio < 0.75 { CueKind::TurnMore } else { CueKind::TurnLess };
                offer(Cue::new(kind, Tone::Info, 2).values(c.abs(), rc.abs()));
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::{RefMeta, RefSample};

    /// 1 km "lap" at 50 m/s with one corner: brake 30-36%, slowest at 40%, throttle back at 44%.
    fn reference() -> ReferenceLap {
        let samples: Vec<RefSample> = (0..=1000)
            .map(|i| {
                let pct = i as f32 / 1000.0;
                let braking = (0.30..0.36).contains(&pct);
                let coasting = (0.36..0.44).contains(&pct);
                let speed = if (0.30..0.44).contains(&pct) { 50.0 - 25.0 * (1.0 - ((pct - 0.40) / 0.08).abs()).max(0.0) } else { 50.0 };
                RefSample {
                    pct,
                    time: pct * 20.0,
                    speed: Some(speed),
                    throttle: if braking || coasting { 0.0 } else { 1.0 },
                    brake: if braking { 0.8 } else { 0.0 },
                    gear: if (0.33..0.45).contains(&pct) { 3 } else { 5 },
                    ..Default::default()
                }
            })
            .collect();
        let meta = RefMeta { track_name: "T".into(), length_m: 1000.0, lap_time: 20.0, ..Default::default() };
        ReferenceLap::build(meta, &samples).unwrap()
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

    #[test]
    fn finds_the_corner() {
        let z = find_zones(&reference());
        assert_eq!(z.len(), 1);
        assert!((299..=301).contains(&z[0].brake_start), "{:?}", z[0]);
        assert!((399..=401).contains(&z[0].apex), "{:?}", z[0]);
        assert!((439..=441).contains(&z[0].pickup), "{:?}", z[0]);
    }

    #[test]
    fn cues_follow_the_corner() {
        let r = reference();
        let mut c = Coach::new(&r);
        let kind = |c: &mut Coach, f: RawFrame| c.update(&r, &f, None, 1000.0).cue.map(|q| q.kind);
        // 40 m before the brake point, flat out: countdown.
        assert_eq!(kind(&mut c, frame(0.26, 0.0, 1.0, 0.0, 50.0, 5)), Some(CueKind::BrakeSoon));
        // 20 m past it and still flat out: brake now (urgent, so it interrupts).
        assert_eq!(kind(&mut c, frame(0.32, 0.2, 1.0, 0.0, 50.0, 5)), Some(CueKind::BrakeNow));
        // Braking much lighter than the reference.
        let mut c = Coach::new(&r);
        assert_eq!(kind(&mut c, frame(0.33, 5.0, 0.0, 0.3, 45.0, 5)), Some(CueKind::BrakeHarder));
        // Braking 60 m early.
        let mut c = Coach::new(&r);
        assert_eq!(kind(&mut c, frame(0.24, 9.0, 0.0, 0.6, 50.0, 5)), Some(CueKind::BrakeEarly));
        // Too slow at the apex.
        let mut c = Coach::new(&r);
        assert_eq!(kind(&mut c, frame(0.40, 12.0, 0.2, 0.0, 20.0, 3)), Some(CueKind::CarrySpeed));
        // Reference is back on the power and we're not.
        let mut c = Coach::new(&r);
        assert_eq!(kind(&mut c, frame(0.46, 15.0, 0.0, 0.0, 30.0, 3)), Some(CueKind::ThrottleNow));
    }

    #[test]
    fn cue_is_held_then_replaced() {
        let r = reference();
        let mut c = Coach::new(&r);
        let mut cue = |f: RawFrame| c.update(&r, &f, None, 1000.0).cue.unwrap().kind;
        // Wrong gear on the straight for long enough to trigger.
        for i in 0..50 {
            cue(frame(0.1, i as f64 * 0.02, 1.0, 0.0, 50.0, 4));
        }
        assert_eq!(cue(frame(0.1, 1.0, 1.0, 0.0, 50.0, 4)), CueKind::ShiftUp);
        // Now everything's fine, but the shift cue stays up briefly...
        assert_eq!(cue(frame(0.1, 1.2, 1.0, 0.0, 50.0, 5)), CueKind::ShiftUp);
        // ...then clears.
        assert_eq!(cue(frame(0.1, 2.2, 1.0, 0.0, 50.0, 5)), CueKind::OnPace);
    }
}
