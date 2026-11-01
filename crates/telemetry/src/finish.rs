//! Whether you've finished the race.
//!
//! The race is over for the leader when the session goes chequered
//! (`SessionState` 5); everyone else finishes the next time they cross the
//! line. So when the chequered flag first shows we note how many laps you've
//! completed, and you've finished once that goes up. Cool down (state 6)
//! means everyone has finished. Leaving the car after the flag counts too.

use crate::model::RawFrame;

/// irsdk_SessionState.
pub mod state {
    pub const CHECKERED: i32 = 5;
    pub const COOL_DOWN: i32 = 6;
}

#[derive(Debug, Default)]
pub struct FinishTracker {
    session_num: Option<i32>,
    /// Your completed laps when the chequered flag came out.
    laps_at_flag: Option<i32>,
    finished: bool,
}

impl FinishTracker {
    pub fn update(&mut self, f: &RawFrame, race: bool) {
        if self.session_num != Some(f.session_num) {
            *self = Self { session_num: Some(f.session_num), ..Self::default() };
        }
        if !race || self.finished {
            return;
        }
        if f.session_state >= state::COOL_DOWN {
            self.finished = true;
            return;
        }
        if f.session_state == state::CHECKERED {
            match self.laps_at_flag {
                None => self.laps_at_flag = Some(f.lap_completed),
                Some(at) if f.lap_completed > at => self.finished = true,
                // Out of the car after the flag: nothing more to race for.
                Some(_) if !f.is_on_track => self.finished = true,
                Some(_) => {}
            }
        }
    }

    pub fn finished(&self) -> bool {
        self.finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(state: i32, laps: i32) -> RawFrame {
        let mut f = RawFrame::with_capacity();
        f.session_num = 2;
        f.session_state = state;
        f.lap_completed = laps;
        f.is_on_track = true;
        f
    }

    #[test]
    fn finishes_at_the_line_after_the_flag() {
        let mut t = FinishTracker::default();
        t.update(&frame(4, 19), true);
        assert!(!t.finished());
        // The leader takes the flag while you're mid-lap.
        t.update(&frame(state::CHECKERED, 19), true);
        assert!(!t.finished());
        t.update(&frame(state::CHECKERED, 20), true);
        assert!(t.finished());
        // Stays finished through cool down.
        t.update(&frame(state::COOL_DOWN, 20), true);
        assert!(t.finished());
    }

    #[test]
    fn cool_down_and_leaving_the_car_count() {
        let mut t = FinishTracker::default();
        t.update(&frame(state::COOL_DOWN, 3), true);
        assert!(t.finished());

        let mut t = FinishTracker::default();
        t.update(&frame(state::CHECKERED, 12), true);
        let mut out = frame(state::CHECKERED, 12);
        out.is_on_track = false;
        t.update(&out, true);
        assert!(t.finished());
    }

    #[test]
    fn not_outside_races_and_resets_with_the_session() {
        let mut t = FinishTracker::default();
        t.update(&frame(state::COOL_DOWN, 3), false);
        assert!(!t.finished());
        t.update(&frame(state::COOL_DOWN, 3), true);
        assert!(t.finished());
        let mut next = frame(4, 0);
        next.session_num = 3;
        t.update(&next, true);
        assert!(!t.finished());
    }
}
