//! Battle (`src/widgets/Battle.tsx`): the cars directly ahead of and behind you
//! in your class (ahead on top). For each, the gap now, how much it changes per
//! lap, their times through the last three sectors and how much the gap has
//! changed so far this lap, their last and best laps against yours, their tyres
//! and stops. Only what the timing shows: no predictions.

use std::collections::HashMap;

use egui::{Align, Align2, Painter, Pos2, Rangef, Rect, Stroke, Vec2};
use telemetry::model::StandingRow;
use telemetry::stints::CarStints;

use super::{Ctx, Widget};
use crate::data::Live;
use crate::kit::{self, seg};
use crate::logic::battle::{laps_between, trend, Place, Trend, STEADY};
use crate::logic::format::{lap_time, signed};
use crate::logic::race_history::RaceHistory;
use crate::theme::{hex_or, Theme};

const W: f32 = 360.0;
/// A gap of more than this isn't a battle yet, so it's shown quieter.
const NEAR_S: f32 = 2.0;
/// The figures column's width; the sector times use the rest of the row.
const FIGURES_W: f32 = 112.0;
const SECTOR_ROW_H: f32 = 34.0;
/// How many of the latest sectors are shown.
const SECTORS_SHOWN: usize = 3;
/// A change in gap this big within one lap is a lap counted differently, not driving.
const LAP_CHANGE_MAX: f32 = 30.0;

#[derive(Default)]
pub struct Battle {
    history: RaceHistory,
    seen: u64,
    /// Your lap the baselines below are from.
    my_lap: Option<i32>,
    /// Each car's gap to you (seconds, positive when behind you) as your lap began.
    lap_start: HashMap<i32, f32>,
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Ahead,
    Behind,
}

struct Shared<'a> {
    me: &'a StandingRow,
    mine: Option<&'a CarStints>,
    stints: &'a [CarStints],
    compounds: &'a [String],
    lap: f64,
    trend_laps: u32,
    show_sectors: bool,
    show_laps: bool,
    show_tyres: bool,
    /// Where each sector starts, as a fraction of the lap.
    sector_starts: &'a [f32],
}

impl Widget for Battle {
    fn observe(&mut self, live: &Live, _now: f64) {
        if live.fields == self.seen {
            return;
        }
        self.seen = live.fields;
        let (Some(field), Some(tick)) = (live.field.as_deref(), live.tick.as_deref()) else { return };
        let track = live.session.as_ref().map(|s| s.track_name.as_str()).unwrap_or("");
        self.history.push_field(field, tick.session_time, &format!("{track}|{}", field.clock.session_type));
        if let Some(me) = field.standings.iter().find(|r| r.is_player) {
            self.lap_mark(me, &field.standings);
        }
    }

    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let field = cx.live.field.as_deref();
        let race = cx.live.race();
        let rows = field.map(|f| f.standings.as_slice()).unwrap_or(&[]);
        let me = rows.iter().find(|r| r.is_player);

        let (Some(field), true, Some(me)) = (field, race, me) else {
            let panel = kit::Panel::begin(p, th, W, Some("Battle"), &[]);
            let msg = if !race { "Battles show in races." } else { "Battles show once you're in the race." };
            let h = kit::empty(p, th, panel.x(), panel.body_top, msg);
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        };

        let mut cls: Vec<&StandingRow> = rows.iter().filter(|r| r.driver.class_id == me.driver.class_id && r.position > 0).collect();
        cls.sort_by_key(|r| r.class_position);
        let at = cls.iter().position(|r| r.is_player);
        let ahead = at.filter(|a| *a > 0).map(|a| cls[a - 1]);
        let behind = at.filter(|a| a + 1 < cls.len()).map(|a| cls[a + 1]);

        let o = cx.o;
        let shared = Shared {
            me,
            mine: field.stints.iter().find(|c| c.car_idx == me.driver.car_idx),
            stints: &field.stints,
            compounds: cx.live.compounds(),
            // A lap's time for the trend: your recent lap, else your best.
            lap: me.laps.last_lap.or(me.laps.best_lap).unwrap_or(0.0) as f64,
            trend_laps: o.int("trendLaps", 1) as u32,
            show_sectors: o.b("showSectors"),
            show_laps: o.b("showLaps"),
            show_tyres: o.b("showTyres"),
            sector_starts: cx.live.session.as_ref().map(|s| s.sectors.as_slice()).unwrap_or(&[]),
        };

        let aside: Vec<_> = if me.class_position > 0 { vec![seg(format!("P{}", me.class_position), kit::aside_font(th), th.ink2)] } else { Vec::new() };
        let panel = kit::Panel::begin(p, th, W, Some("Battle"), &aside);
        let mut y = panel.body_top;
        for (k, (side, car)) in [(Side::Ahead, ahead), (Side::Behind, behind)].into_iter().enumerate() {
            if k > 0 {
                p.hline(panel.x(), y + 0.5, Stroke::new(1.0, th.line));
                y += 1.0;
            }
            y += self.opponent(p, th, &panel, y, side, car, &shared);
        }
        panel.finish(p, th, y)
    }
}

/// A car's gap to you on track: seconds, positive when it's behind you.
fn gap_to_me(car: &StandingRow, me: &StandingRow) -> Option<f32> {
    let live = |r: &StandingRow| if r.class_position == 1 { Some(0.0) } else { r.live_gap };
    Some(live(car)? - live(me)?)
}

/// The sector a lap fraction is in, given where each sector starts.
fn sector_at(starts: &[f32], pct: f32) -> usize {
    starts.iter().rposition(|s| *s <= pct).unwrap_or(0)
}

/// The last `SECTORS_SHOWN` sectors finished before sector `current`, oldest first (all of
/// them, in order, when the track has no more than that).
fn latest_sectors(n: usize, current: usize) -> Vec<usize> {
    if n <= SECTORS_SHOWN {
        return (0..n).collect();
    }
    (1..=SECTORS_SHOWN).rev().map(|back| (current + n - back) % n).collect()
}

impl Battle {
    /// When your lap changes, remembers every car's gap to you, to measure this lap's change from.
    fn lap_mark(&mut self, me: &StandingRow, rows: &[StandingRow]) {
        if me.lap <= 0 || self.my_lap == Some(me.lap) {
            return;
        }
        self.my_lap = Some(me.lap);
        self.lap_start = rows.iter().filter(|r| !r.is_player).filter_map(|r| Some((r.driver.car_idx, gap_to_me(r, me)?))).collect();
    }

    /// How much the gap to `car` has changed since your lap began: seconds, positive when you
    /// gained on it (like a sector difference: their time minus yours).
    fn lap_change(&self, car: &StandingRow, me: &StandingRow) -> Option<f32> {
        let start = self.lap_start.get(&car.driver.car_idx)?;
        let d = gap_to_me(car, me)? - start;
        (d.abs() < LAP_CHANGE_MAX).then_some(d)
    }

    /// One section from `top`; returns its height.
    #[allow(clippy::too_many_arguments)]
    fn opponent(&self, p: &Painter, th: &Theme, panel: &kit::Panel, top: f32, side: Side, car: Option<&StandingRow>, s: &Shared) -> f32 {
        let Some(car) = car else { return 30.0 };
        let me = s.me;

        // The gap now: on the same lap, from where the cars are on track (live, like the Relative);
        // otherwise, or when either isn't on track, from iRacing's timing.
        let g = |r: &StandingRow| if r.class_position == 1 { Some(0.0) } else { r.gap };
        let live = |r: &StandingRow| if r.class_position == 1 { Some(0.0) } else { r.live_gap };
        // Laps apart: from the live gaps when there are both (the lap counters tick a moment after
        // the line, which made a car crossing it look a lap away), else from the lap counters.
        // (At your best lap's pace: your last lap may have been a stop.)
        let pace = me.laps.best_lap.or(me.laps.last_lap).unwrap_or(0.0);
        let laps_apart = match (live(car), live(me)) {
            (Some(a), Some(b)) if pace > 1.0 => ((a - b).abs() / pace).floor() as i32,
            _ => laps_between(Place::from(car), Place::from(me)),
        };
        let gap_now = match (live(car), live(me), g(car), g(me)) {
            (Some(a), Some(b), _, _) if laps_apart == 0 => Some((a - b).abs()),
            (_, _, Some(a), Some(b)) => Some((a - b).abs()),
            _ => None,
        };

        // How the gap changes per lap, from the race history.
        let empty: Vec<f32> = Vec::new();
        let trace = |idx: i32| self.history.cars.get(&idx).map(|c| &c.points).unwrap_or(&empty);
        let (mine_pts, their_pts) = (trace(me.driver.car_idx), trace(car.driver.car_idx));
        let now = self.history.now;
        let t: Option<Trend> = if laps_apart == 0 { trend(mine_pts, their_pts, now, s.lap, s.trend_laps) } else { None };

        // The gap shrinking is good when chasing, bad when defending.
        let closing = t.map(|t| t.closing_per_lap).unwrap_or(0.0);
        let (good, bad) = match side {
            Side::Ahead => (closing > STEADY, closing < -STEADY),
            Side::Behind => (closing < -STEADY, closing > STEADY),
        };
        let tone = if good {
            Some(th.gain)
        } else if bad {
            Some(th.loss)
        } else {
            None
        };

        let facts = s.show_laps || s.show_tyres;
        let left = panel.x().min + 3.0 + 8.0;
        let right = panel.x().max - 10.0;
        // The last three sectors beside the figures, then this lap so far. "Last" is as seen by
        // whichever of you is behind on track, so both of you have been through them.
        let n_all = car.sectors.len().max(me.sectors.len());
        let shown: Vec<usize> = if s.show_sectors && n_all > 0 {
            let trailing = match side {
                Side::Ahead => me,
                Side::Behind => car,
            };
            latest_sectors(n_all, sector_at(s.sector_starts, trailing.lap_pct))
        } else {
            Vec::new()
        };
        let sectors_x = Rangef::new(left + FIGURES_W, right);
        let main_h = 46f32.max(if s.show_sectors { SECTOR_ROW_H } else { 0.0 });
        let height = 6.0 + 20.0 + 3.0 + main_h + if facts { 3.0 + 16.0 } else { 0.0 } + 6.0;
        let near = gap_now.is_some_and(|g| g < NEAR_S) && laps_apart == 0;
        if near {
            p.rect_filled(Rect::from_min_size(Pos2::new(panel.x().min, top), Vec2::new(3.0, height)), 0.0, th.attention);
        }
        let class = hex_or(&car.driver.class_color, th.ink);

        // Who: position, class stripe, number, name, iRating.
        let cy = top + 6.0 + 10.0;
        let pos = kit::text(p, Pos2::new(left, cy), Align2::LEFT_CENTER, &format!("P{}", car.class_position), th.bold(14.0), th.ink);
        let stripe_x = pos.right() + 6.0 + 2.0;
        kit::class_bar(p, th, stripe_x, cy, class);
        let num_x = stripe_x + 2.0 + 6.0;
        let (num_font, num_color) = if th.broadcast() { (th.semi(14.0), th.ink3) } else { (th.bold(14.0), th.ink2) };
        let list = [seg("#", th.medium(12.0), th.ink3), seg(car.driver.number.clone(), num_font, num_color)];
        let num_w = kit::segs_width(p, &list, 0.0) + 1.0;
        kit::num_cell(p, th, num_x + num_w, cy, &car.driver.number, 14.0);
        let name_x = num_x + num_w + 6.0;
        let ir_w = if car.driver.irating > 0 { (kit::width(p, &crate::logic::format::irating(car.driver.irating), th.bold(12.0)) + 8.0).max(34.0) } else { 0.0 };
        let name_max = right - ir_w - if ir_w > 0.0 { 6.0 } else { 0.0 } - name_x;
        kit::text_fit(p, Pos2::new(name_x, cy), Align2::LEFT_CENTER, &car.driver.name, th.semi(14.0), th.ink, name_max, 0.0);
        kit::ir_badge(p, th, Rangef::new(left, right), cy, Align::Max, car.driver.irating, None);

        // Figures: the gap, and its change per lap.
        let main_top = top + 6.0 + 20.0 + 3.0;
        let gap_text = if laps_apart > 0 {
            format!("+{laps_apart}L")
        } else if let Some(g) = gap_now {
            format!("{:.*}", if g < 10.0 { 2 } else { 1 }, g)
        } else {
            "–".to_string()
        };
        kit::text(p, Pos2::new(left, main_top + 14.5), Align2::LEFT_CENTER, &gap_text, th.bold(28.0), th.ink);
        let rate_y = main_top + 37.0;
        match t {
            Some(t) => {
                let c = t.closing_per_lap;
                let sign = if c.abs() < 0.005 {
                    "±"
                } else if c > 0.0 {
                    "−"
                } else {
                    "+"
                };
                let r = kit::text(p, Pos2::new(left, rate_y), Align2::LEFT_CENTER, &format!("{sign}{:.2}", c.abs()), th.bold(14.0), tone.unwrap_or(th.ink2));
                kit::text(p, Pos2::new(r.right() + 2.0, rate_y), Align2::LEFT_CENTER, "/lap", th.medium(11.0), th.ink3);
            }
            None => {
                let r = kit::text(p, Pos2::new(left, rate_y), Align2::LEFT_CENTER, "–", th.bold(14.0), th.ink2);
                kit::text(p, Pos2::new(r.right() + 2.0, rate_y), Align2::LEFT_CENTER, "/lap", th.medium(11.0), th.ink3);
            }
        }
        if s.show_sectors {
            let lap = if laps_apart == 0 { self.lap_change(car, me) } else { None };
            sectors(p, th, sectors_x, main_top, &shown, &car.sectors, &me.sectors, lap);
        }

        // Facts: last and best laps against yours, tyre, stops.
        if facts {
            let cy = main_top + main_h + 3.0 + 8.0;
            let mut x = left;
            let font = th.medium(12.5);
            let lap_vs = |x: &mut f32, label: &str, theirs: Option<f32>, mine: Option<f32>| {
                let k = kit::text(p, Pos2::new(*x, cy), Align2::LEFT_CENTER, label, font.clone(), th.ink3);
                let v = kit::text(p, Pos2::new(k.right() + 4.0, cy), Align2::LEFT_CENTER, &lap_time(theirs), font.clone(), th.ink);
                let mut end = v.right();
                let d = match (theirs, mine) {
                    (Some(a), Some(b)) if a > 0.0 && b > 0.0 => Some((a - b) as f64),
                    _ => None,
                };
                if let Some(d) = d {
                    let color = if d > 0.0005 {
                        th.gain
                    } else if d < -0.0005 {
                        th.loss
                    } else {
                        th.ink
                    };
                    end = kit::text(p, Pos2::new(end + 4.0, cy), Align2::LEFT_CENTER, &signed(d, 3), font.clone(), color).right();
                }
                *x = end + 9.0;
            };
            if s.show_laps {
                lap_vs(&mut x, "L", car.laps.last_lap, me.laps.last_lap);
                lap_vs(&mut x, "B", car.laps.best_lap, me.laps.best_lap);
            }
            if s.show_tyres {
                let theirs = s.stints.iter().find(|c| c.car_idx == car.driver.car_idx);
                let stint = theirs.and_then(|c| c.stints.last());
                if let Some(stint) = stint {
                    let tyre = if stint.compound >= 0 { s.compounds.get(stint.compound as usize).map(String::as_str).unwrap_or("") } else { "" };
                    let tyre_laps = (stint.end - stint.start).floor().max(0.0) as i32;
                    let mut fx = x;
                    if !tyre.is_empty() {
                        kit::tyre_badge(p, th, Pos2::new(fx + 9.0, cy), 18.0, tyre);
                        fx += 18.0 + 4.0 + 1.0;
                    }
                    let r = kit::text(p, Pos2::new(fx, cy), Align2::LEFT_CENTER, &format!("{tyre_laps}{}L", if stint.partial { "+" } else { "" }), font.clone(), th.ink);
                    x = r.right() + 9.0;
                }
                if let Some(theirs) = theirs {
                    let k = kit::text(p, Pos2::new(x, cy), Align2::LEFT_CENTER, "Stops", font.clone(), th.ink3);
                    let v = kit::text(p, Pos2::new(k.right() + 4.0, cy), Align2::LEFT_CENTER, &theirs.stops.to_string(), font.clone(), th.ink);
                    let mine = s.mine.map(|m| m.stops).unwrap_or(0);
                    kit::text(p, Pos2::new(v.right() + 4.0, cy), Align2::LEFT_CENTER, &format!("({mine})"), font, th.ink3);
                }
            }
        }
        height
    }
}

/// Red where they were faster than you, green where slower (`d` is their time minus yours).
fn diff_color(th: &Theme, d: f64) -> egui::Color32 {
    if d < -0.0005 {
        th.loss
    } else if d > 0.0005 {
        th.gain
    } else {
        th.ink2
    }
}

/// Their time through each of the `shown` sectors with the difference to yours under it, then a
/// last cell, "Lap": how much the gap has changed since your lap began (live, back to 0 each
/// lap). One row across `x` from `top`.
#[allow(clippy::too_many_arguments)]
fn sectors(p: &Painter, th: &Theme, x: Rangef, top: f32, shown: &[usize], theirs: &[Option<f32>], mine: &[Option<f32>], lap: Option<f32>) {
    let cell_w = x.span() / (shown.len() + 1) as f32;
    let y = top;
    for (col, &k) in shown.iter().enumerate() {
        let cx = x.min + cell_w * (col as f32 + 0.5);
        let t = theirs.get(k).copied().flatten();
        let m = mine.get(k).copied().flatten();
        kit::text(p, Pos2::new(cx, y + 6.0), Align2::CENTER_CENTER, &format!("S{}", k + 1), th.medium(10.0), th.ink3);
        let time = t.map(|t| sector_time(t as f64)).unwrap_or_else(|| "–".into());
        kit::text(p, Pos2::new(cx, y + 18.0), Align2::CENTER_CENTER, &time, th.semi(12.5), th.ink);
        if let (Some(t), Some(m)) = (t, m) {
            let d = (t - m) as f64;
            kit::text(p, Pos2::new(cx, y + 29.0), Align2::CENTER_CENTER, &signed(d, 2), th.semi(11.0), diff_color(th, d));
        }
    }
    // This lap so far, set apart by a thin rule.
    let left = x.min + cell_w * shown.len() as f32;
    let cx = left + cell_w / 2.0;
    if !shown.is_empty() {
        p.vline(left, Rangef::new(y + 2.0, y + 32.0), Stroke::new(1.0, th.line));
    }
    kit::text(p, Pos2::new(cx, y + 6.0), Align2::CENTER_CENTER, "Lap", th.medium(10.0), th.ink3);
    match lap {
        Some(d) => {
            let d = d as f64;
            kit::text(p, Pos2::new(cx, y + 22.0), Align2::CENTER_CENTER, &signed(d, 2), th.bold(15.0), diff_color(th, d));
        }
        None => {
            kit::text(p, Pos2::new(cx, y + 22.0), Align2::CENTER_CENTER, "–", th.bold(15.0), th.ink2);
        }
    }
}

/// A sector time: seconds with two decimals, or m:ss.ss past a minute.
fn sector_time(t: f64) -> String {
    if t >= 60.0 {
        let m = (t / 60.0).floor();
        format!("{}:{:05.2}", m as i64, t - m * 60.0)
    } else {
        format!("{t:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_sectors_before_this_one() {
        assert_eq!(latest_sectors(7, 0), vec![4, 5, 6]);
        assert_eq!(latest_sectors(7, 2), vec![6, 0, 1]);
        assert_eq!(latest_sectors(7, 5), vec![2, 3, 4]);
        assert_eq!(latest_sectors(3, 1), vec![0, 1, 2]);
        assert_eq!(latest_sectors(2, 1), vec![0, 1]);
        let starts = [0.0, 0.3, 0.7];
        assert_eq!(sector_at(&starts, 0.1), 0);
        assert_eq!(sector_at(&starts, 0.3), 1);
        assert_eq!(sector_at(&starts, 0.95), 2);
        assert_eq!(sector_at(&[], 0.5), 0);
    }

    #[test]
    fn sector_times_read_like_lap_times() {
        assert_eq!(sector_time(31.234), "31.23");
        assert_eq!(sector_time(75.5), "1:15.50");
    }
}
