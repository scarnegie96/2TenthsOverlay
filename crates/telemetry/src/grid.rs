//! Where each car started a race, for positions gained and lost.
//!
//! iRacing doesn't publish a starting grid as such. While the race hasn't
//! started (getting in the car, warm-up, parade laps) the live class
//! positions are the grid, so we keep the latest of those. Cars still
//! missing when the green flag drops are filled in from their first
//! classified position on the opening lap, and the grid is fixed once the
//! leader finishes lap 1. If we join a race already under way there's
//! nothing to watch, so the qualifying order from the session info stands in.

use std::collections::HashMap;

use crate::model::{RawFrame, MAX_CARS};
use crate::session::SessionInfo;

/// irsdk_SessionState: racing.
const STATE_RACING: i32 = 4;

#[derive(Debug, Default)]
pub struct StartGrid {
    session_num: Option<i32>,
    /// Car index -> class position at the start (1-based).
    class_pos: HashMap<i32, i32>,
    locked: bool,
}

impl StartGrid {
    /// Call every frame with whether the current session is a race.
    pub fn update(&mut self, f: &RawFrame, race: bool) {
        if self.session_num != Some(f.session_num) {
            *self = Self { session_num: Some(f.session_num), ..Self::default() };
        }
        if !race || self.locked {
            return;
        }
        let cars = f.car_class_position.len().min(MAX_CARS);
        let leader_laps = (0..cars).map(|i| f.car_lap_completed[i]).max().unwrap_or(0);
        if f.session_state < STATE_RACING {
            // Before the start: the live order is the grid.
            for i in 0..cars {
                if f.car_class_position[i] > 0 {
                    self.class_pos.insert(i as i32, f.car_class_position[i]);
                }
            }
        } else if leader_laps <= 0 {
            // Green flag, still on the opening lap: fill in anyone we haven't seen.
            for i in 0..cars {
                if f.car_class_position[i] > 0 {
                    self.class_pos.entry(i as i32).or_insert(f.car_class_position[i]);
                }
            }
        } else {
            // Under way. Whatever we have is the grid (empty if we joined late).
            self.locked = true;
        }
    }

    /// Class position the car started from, if known.
    pub fn start(&self, car_idx: i32, session: &SessionInfo) -> Option<i32> {
        self.class_pos.get(&car_idx).copied().or_else(|| session.qualify_class_position(car_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(state: i32, laps: i32, positions: &[(usize, i32)]) -> RawFrame {
        let mut f = RawFrame::with_capacity();
        f.session_num = 2;
        f.session_state = state;
        f.car_class_position = vec![0; MAX_CARS];
        f.car_lap_completed = vec![laps; MAX_CARS];
        for &(i, p) in positions {
            f.car_class_position[i] = p;
        }
        f
    }

    #[test]
    fn grid_is_the_order_before_the_start() {
        let s = SessionInfo::default();
        let mut g = StartGrid::default();
        g.update(&frame(3, 0, &[(4, 1), (7, 2)]), true);
        // Green flag, cars swap on the opening lap: the grid doesn't change.
        g.update(&frame(4, 0, &[(4, 2), (7, 1), (9, 3)]), true);
        // A car only classified after the start gets its first position.
        assert_eq!((g.start(4, &s), g.start(7, &s), g.start(9, &s)), (Some(1), Some(2), Some(3)));
        // Later changes are ignored once the leader is on lap 2.
        g.update(&frame(4, 1, &[(12, 4)]), true);
        g.update(&frame(4, 1, &[(4, 3)]), true);
        assert_eq!((g.start(4, &s), g.start(12, &s)), (Some(1), None));
    }

    #[test]
    fn joining_late_uses_qualifying() {
        let s = crate::session::parse(
            "QualifyResultsInfo:\n Results:\n - Position: 0\n   ClassPosition: 0\n   CarIdx: 7\n - Position: 1\n   ClassPosition: 1\n   CarIdx: 4\n",
        )
        .unwrap();
        let mut g = StartGrid::default();
        g.update(&frame(4, 5, &[(4, 1), (7, 2)]), true);
        assert_eq!((g.start(7, &s), g.start(4, &s)), (Some(1), Some(2)));
    }

    #[test]
    fn new_session_starts_afresh() {
        let s = SessionInfo::default();
        let mut g = StartGrid::default();
        g.update(&frame(3, 0, &[(4, 1)]), true);
        let mut f = frame(3, 0, &[(4, 5)]);
        f.session_num = 3;
        g.update(&f, true);
        assert_eq!(g.start(4, &s), Some(5));
    }

    #[test]
    fn car_status_from_surface_and_pit_road() {
        use crate::model::{track_surface, CarStatus};
        let mut f = RawFrame::with_capacity();
        f.car_track_surface[1] = track_surface::ON_TRACK;
        f.car_track_surface[2] = track_surface::ON_TRACK;
        f.car_on_pit_road[2] = true;
        f.car_track_surface[3] = track_surface::IN_PIT_STALL;
        f.car_on_pit_road[3] = true;
        // Car 4 left the session: not in the world.
        assert_eq!(CarStatus::of(&f, 1), CarStatus::Track);
        assert_eq!(CarStatus::of(&f, 2), CarStatus::PitLane);
        assert_eq!(CarStatus::of(&f, 3), CarStatus::PitStall);
        assert_eq!(CarStatus::of(&f, 4), CarStatus::Out);
    }
}
