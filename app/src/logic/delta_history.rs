//! How the delta to each comparison lap evolves around the lap, and what
//! each sector gained or lost (`src/lib/deltaHistory.ts`). Fed from every
//! tick whether the Delta widget is shown or not; starts afresh in a new
//! session or when session time goes backwards.

use telemetry::model::{Deltas, TickPayload};

/// Points per lap.
pub const BINS: usize = 320;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Best,
    Optimal,
    SessionBest,
    SessionOptimal,
    Last,
    Reference,
}

pub const MODES: [Mode; 6] = [Mode::Best, Mode::Optimal, Mode::SessionBest, Mode::SessionOptimal, Mode::Last, Mode::Reference];

impl Mode {
    pub fn parse(s: &str) -> Self {
        match s {
            "optimal" => Self::Optimal,
            "sessionBest" => Self::SessionBest,
            "sessionOptimal" => Self::SessionOptimal,
            "last" => Self::Last,
            "reference" => Self::Reference,
            _ => Self::Best,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Best => "Best lap",
            Self::Optimal => "Optimal lap",
            Self::SessionBest => "Session best",
            Self::SessionOptimal => "Session optimal",
            Self::Last => "Last lap",
            Self::Reference => "Reference lap",
        }
    }
    pub fn of(self, d: &Deltas) -> Option<f32> {
        match self {
            Self::Best => d.best,
            Self::Optimal => d.optimal,
            Self::SessionBest => d.session_best,
            Self::SessionOptimal => d.session_optimal,
            Self::Last => d.last,
            Self::Reference => d.reference,
        }
    }
    fn index(self) -> usize {
        MODES.iter().position(|m| *m == self).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct ModeHistory {
    /// Delta at each point of the current lap (None where not driven yet).
    pub current: Vec<Option<f32>>,
    pub previous: Vec<Option<f32>>,
    /// Time gained (−) or lost (+) per sector this lap; None if not finished.
    pub sectors: Vec<Option<f32>>,
    pub previous_sectors: Vec<Option<f32>>,
    /// Delta when the current sector began.
    pub sector_start: Option<f32>,
    pub now: Option<f32>,
}

impl ModeHistory {
    fn new(sectors: usize) -> Self {
        Self {
            current: vec![None; BINS],
            previous: vec![None; BINS],
            sectors: vec![None; sectors],
            previous_sectors: vec![None; sectors],
            sector_start: None,
            now: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DeltaHistory {
    /// Sector start points; the first is always 0.
    pub sectors: Vec<f32>,
    pub modes: Vec<ModeHistory>,
    pub current_sector: usize,
    pub pct: f32,
    lap: Option<i32>,
    session: Option<String>,
    session_time: f64,
}

impl Default for DeltaHistory {
    fn default() -> Self {
        let mut h = Self { sectors: vec![0.0], modes: Vec::new(), current_sector: 0, pct: 0.0, lap: None, session: None, session_time: 0.0 };
        h.reset();
        h
    }
}

impl DeltaHistory {
    pub fn reset(&mut self) {
        self.modes = MODES.iter().map(|_| ModeHistory::new(self.sectors.len())).collect();
        self.current_sector = 0;
        self.pct = 0.0;
        self.lap = None;
    }

    pub fn mode(&self, m: Mode) -> &ModeHistory {
        &self.modes[m.index()]
    }

    /// Changing the sector layout clears the sector splits (the graph is kept).
    pub fn set_sectors(&mut self, starts: &[f32]) {
        let mut next: Vec<f32> = std::iter::once(0.0).chain(starts.iter().copied().filter(|s| *s > 0.001 && *s < 0.999)).collect();
        next.sort_by(|a, b| a.total_cmp(b));
        if next.len() == self.sectors.len() && next.iter().zip(&self.sectors).all(|(a, b)| (a - b).abs() < 1e-6) {
            return;
        }
        self.sectors = next;
        for h in &mut self.modes {
            h.sectors = vec![None; self.sectors.len()];
            h.previous_sectors = vec![None; self.sectors.len()];
            h.sector_start = None;
        }
        self.current_sector = self.sector_at(self.pct);
    }

    pub fn sector_at(&self, pct: f32) -> usize {
        let mut k = 0;
        while k + 1 < self.sectors.len() && pct >= self.sectors[k + 1] {
            k += 1;
        }
        k
    }

    /// `session` names the session (None while unknown).
    pub fn push(&mut self, t: &TickPayload, session: Option<&str>) {
        let changed = session.is_some() && self.session.is_some() && session != self.session.as_deref();
        if changed || t.session_time < self.session_time - 1.0 {
            self.reset();
        }
        if let Some(s) = session {
            self.session = Some(s.to_string());
        }
        self.session_time = t.session_time;
        if !t.is_on_track || t.lap_dist_pct < 0.0 {
            return;
        }
        let pct = t.lap_dist_pct.clamp(0.0, 0.99999);
        if self.lap.is_some() {
            if pct < self.pct - 0.5 {
                self.new_lap();
            } else if pct < self.pct - 0.02 || self.lap.is_some_and(|l| t.lap < l) {
                self.clear_lap();
            }
        }
        self.lap = Some(t.lap);

        let sector = self.sector_at(pct);
        if sector != self.current_sector {
            for m in MODES {
                let h = &mut self.modes[m.index()];
                let d = m.of(&t.deltas);
                if sector > self.current_sector {
                    if let (Some(start), Some(d)) = (h.sector_start, d) {
                        h.sectors[self.current_sector] = Some(d - start);
                    }
                }
                h.sector_start = d;
            }
            self.current_sector = sector;
        }

        let bin = ((pct * BINS as f32) as usize).min(BINS - 1);
        for m in MODES {
            let d = m.of(&t.deltas);
            let h = &mut self.modes[m.index()];
            h.now = d;
            let Some(d) = d else { continue };
            if h.sector_start.is_none() && sector == 0 && pct < 0.01 {
                h.sector_start = Some(0.0);
            }
            h.current[bin] = Some(d);
        }
        self.pct = pct;
    }

    fn new_lap(&mut self) {
        let last = self.sectors.len() - 1;
        let n = self.sectors.len();
        for h in &mut self.modes {
            if self.current_sector == last {
                if let (Some(start), Some(now)) = (h.sector_start, h.now) {
                    h.sectors[last] = Some(now - start);
                }
            }
            h.previous = std::mem::replace(&mut h.current, vec![None; BINS]);
            h.previous_sectors = std::mem::replace(&mut h.sectors, vec![None; n]);
            h.sector_start = Some(0.0);
        }
        self.current_sector = 0;
        self.pct = 0.0;
    }

    fn clear_lap(&mut self) {
        for h in &mut self.modes {
            h.current.iter_mut().for_each(|v| *v = None);
            h.sectors.iter_mut().for_each(|v| *v = None);
            h.sector_start = None;
        }
    }

    /// Live gain or loss in the sector being driven now.
    pub fn live_sector(&self, m: Mode) -> Option<f32> {
        let h = self.mode(m);
        Some(h.now? - h.sector_start?)
    }

    /// The most recently completed sector (wrapping to last lap's last).
    pub fn last_sector(&self, m: Mode) -> (usize, Option<f32>) {
        let h = self.mode(m);
        if self.current_sector > 0 {
            let i = self.current_sector - 1;
            (i, h.sectors[i])
        } else {
            let i = self.sectors.len() - 1;
            (i, h.previous_sectors[i])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(time: f64, lap: i32, pct: f32, best: f32) -> TickPayload {
        TickPayload {
            session_time: time,
            is_on_track: true,
            lap,
            lap_dist_pct: pct,
            deltas: Deltas { best: Some(best), ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn records_a_lap_and_its_sectors() {
        let mut h = DeltaHistory::default();
        h.set_sectors(&[0.0, 0.5]);
        assert_eq!(h.sectors, vec![0.0, 0.5]);
        // A lap from the line: delta grows 0 -> 0.4 linearly.
        let mut t = 0.0;
        for i in 0..1000 {
            let pct = i as f32 / 1000.0;
            h.push(&tick(t, 1, pct, 0.4 * pct), Some("s"));
            t += 0.1;
        }
        let best = h.mode(Mode::Best);
        assert!(best.current[0].is_some() && best.current[BINS - 1].is_some());
        // Sector 1 ended at 0.5: about +0.2.
        assert!((best.sectors[0].unwrap() - 0.2).abs() < 0.01);
        assert!((h.live_sector(Mode::Best).unwrap() - 0.2).abs() < 0.01);
        assert_eq!(h.last_sector(Mode::Best).0, 0);
        // Over the line: the lap moves to previous, sector 2 is closed.
        h.push(&tick(t, 2, 0.001, 0.0), Some("s"));
        let best = h.mode(Mode::Best);
        assert!(best.previous[BINS - 1].is_some());
        assert!(best.current[BINS - 1].is_none());
        let (i, v) = h.last_sector(Mode::Best);
        assert_eq!(i, 1);
        assert!((v.unwrap() - 0.2).abs() < 0.01);
        assert_eq!(h.current_sector, 0);
        // Other modes had no data.
        assert!(h.mode(Mode::Last).now.is_none());
    }

    #[test]
    fn jumps_back_clear_and_new_sessions_reset() {
        let mut h = DeltaHistory::default();
        h.push(&tick(10.0, 3, 0.4, 0.1), Some("a"));
        h.push(&tick(10.1, 3, 0.3, 0.1), Some("a"));
        // A jump back of more than 2 %: this lap is cleared.
        assert!(h.mode(Mode::Best).current[(0.4 * BINS as f32) as usize].is_none());
        h.push(&tick(11.0, 3, 0.31, 0.1), Some("b"));
        assert!(h.mode(Mode::Best).current[(0.3 * BINS as f32) as usize].is_none());
        assert!(h.mode(Mode::Best).current[(0.31 * BINS as f32) as usize].is_some());
        assert_eq!(Mode::parse("sessionOptimal"), Mode::SessionOptimal);
        assert_eq!(Mode::parse("nonsense"), Mode::Best);
    }
}
