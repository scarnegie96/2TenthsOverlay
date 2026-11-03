//! Every car's latest time through each track sector (`SplitTimeInfo.Sectors` in the session
//! YAML), for comparing sector by sector (the Battle widget). A sector's time is from crossing
//! its start to crossing its end, each crossing timed between frames from where the car was on
//! either side of it. A sector driven partly on pit road, or not seen from its start (joined
//! mid-sector, a tow or a reset), has no time; the car's previous time for it is cleared too, so a
//! stale one isn't shown as current.

use crate::model::{RawFrame, MAX_CARS};

#[derive(Debug, Default)]
pub struct CarSectors {
    session_num: Option<i32>,
    starts: Vec<f32>,
    cars: Vec<Car>,
}

#[derive(Debug, Default, Clone)]
struct Car {
    /// The sector being timed, when it started (session time), and whether it's still clean.
    sector: Option<usize>,
    start: f64,
    clean: bool,
    /// Lap fraction and session time last frame.
    prev: Option<(f32, f64)>,
    /// Latest time through each sector.
    last: Vec<Option<f32>>,
}

impl CarSectors {
    /// Call every frame with the sector start points (lap fractions, the first 0).
    pub fn update(&mut self, f: &RawFrame, starts: &[f32]) {
        let starts = if starts.is_empty() { &[0.0][..] } else { starts };
        if self.session_num != Some(f.session_num) || self.starts != starts {
            self.session_num = Some(f.session_num);
            self.starts = starts.to_vec();
            self.cars = vec![Car { last: vec![None; starts.len()], ..Car::default() }; MAX_CARS];
        }
        let t = f.session_time;
        let n = self.starts.len();
        for (i, car) in self.cars.iter_mut().enumerate() {
            let pct = f.car_lap_dist_pct.get(i).copied().unwrap_or(-1.0);
            if pct < 0.0 {
                // Not in the world: nothing to time until it's back.
                car.sector = None;
                car.prev = None;
                continue;
            }
            let pits = f.car_on_pit_road.get(i).copied().unwrap_or(false);
            let idx = self.starts.iter().rposition(|&s| s <= pct).unwrap_or(0);
            match (car.sector, car.prev) {
                (Some(cur), Some((p0, t0))) => {
                    let wrapped = pct < p0 - 0.5;
                    if idx != cur || wrapped {
                        let next = (cur + 1) % n;
                        if idx == next && (next != 0 || wrapped) {
                            let boundary = if next == 0 { 1.0 } else { self.starts[next] };
                            let p1 = if wrapped { pct + 1.0 } else { pct };
                            let frac = if p1 > p0 { ((boundary - p0) / (p1 - p0)).clamp(0.0, 1.0) } else { 1.0 };
                            let crossed = t0 + frac as f64 * (t - t0);
                            let time = (crossed - car.start) as f32;
                            car.last[cur] = (car.clean && time > 0.5).then_some(time);
                            car.sector = Some(next);
                            car.start = crossed;
                            car.clean = !pits;
                        } else {
                            // Skipped a sector or went backwards: start over mid-sector.
                            car.last[cur] = None;
                            car.sector = Some(idx);
                            car.start = t;
                            car.clean = false;
                        }
                    }
                }
                _ => {
                    car.sector = Some(idx);
                    car.start = t;
                    car.clean = false;
                }
            }
            if pits {
                car.clean = false;
            }
            car.prev = Some((pct, t));
        }
    }

    /// Car `car`'s latest time through each sector (empty before the first update).
    pub fn last(&self, car: usize) -> Vec<Option<f32>> {
        self.cars.get(car).map(|c| c.last.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_each_sector_between_frames() {
        let mut s = CarSectors::default();
        let mut f = RawFrame::with_capacity();
        let starts = [0.0, 0.3, 0.6];
        // Car 0 laps in 100 s at a steady pace, 10 frames a second, from half way round.
        for k in 0..=1500 {
            let t = k as f64 / 10.0;
            f.session_time = t;
            f.car_lap_dist_pct[0] = ((0.5 + t / 100.0) % 1.0) as f32;
            s.update(&f, &starts);
        }
        let last = s.last(0);
        assert!((last[0].unwrap() - 30.0).abs() < 0.05, "{last:?}");
        assert!((last[1].unwrap() - 30.0).abs() < 0.05, "{last:?}");
        assert!((last[2].unwrap() - 40.0).abs() < 0.05, "{last:?}");
    }

    #[test]
    fn a_sector_through_the_pits_has_no_time() {
        let mut s = CarSectors::default();
        let mut f = RawFrame::with_capacity();
        let starts = [0.0, 0.5];
        for k in 0..=2000 {
            let t = k as f64 / 10.0;
            f.session_time = t;
            f.car_lap_dist_pct[0] = ((t / 100.0) % 1.0) as f32;
            // On pit road through the start of the second lap's first sector.
            f.car_on_pit_road[0] = (100.0..105.0).contains(&t);
            s.update(&f, &starts);
        }
        let last = s.last(0);
        assert_eq!(last[0], None, "{last:?}");
        assert!((last[1].unwrap() - 50.0).abs() < 0.05, "{last:?}");
    }
}
