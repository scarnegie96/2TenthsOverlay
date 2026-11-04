//! Battle (`src/widgets/Battle.tsx`): the cars directly ahead of and behind you
//! in your class (ahead on top). For each, the gap now, how much it changes per
//! lap, their latest sector times and last and best laps against yours, their
//! tyres and stops. Only what the timing shows: no predictions.

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
/// Each sector's cell is at least this wide; more sectors than fit wrap onto another row.
const SECTOR_MIN_W: f32 = 56.0;
const SECTOR_ROW_H: f32 = 34.0;

#[derive(Default)]
pub struct Battle {
    history: RaceHistory,
    seen: u64,
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

impl Battle {
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
        // Sector times beside the figures, in as many rows as they need.
        let n_sectors = if s.show_sectors { car.sectors.len().max(me.sectors.len()) } else { 0 };
        let sectors_x = Rangef::new(left + FIGURES_W, right);
        let per_row = ((sectors_x.span() / SECTOR_MIN_W).floor() as usize).max(1);
        let sector_rows = n_sectors.div_ceil(per_row);
        let main_h = 46f32.max(sector_rows as f32 * SECTOR_ROW_H);
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
        if n_sectors > 0 {
            sectors(p, th, sectors_x, main_top, per_row, &car.sectors, &me.sectors, n_sectors);
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

/// Their latest time through each sector, and below it the difference to yours: red where they
/// were faster than you, green where slower. `per_row` cells to a row across `x`, from `top`.
#[allow(clippy::too_many_arguments)]
fn sectors(p: &Painter, th: &Theme, x: Rangef, top: f32, per_row: usize, theirs: &[Option<f32>], mine: &[Option<f32>], n: usize) {
    let cols = per_row.min(n).max(1);
    let cell_w = x.span() / cols as f32;
    for k in 0..n {
        let (row, col) = (k / cols, k % cols);
        let cx = x.min + cell_w * (col as f32 + 0.5);
        let y = top + row as f32 * SECTOR_ROW_H;
        let t = theirs.get(k).copied().flatten();
        let m = mine.get(k).copied().flatten();
        kit::text(p, Pos2::new(cx, y + 6.0), Align2::CENTER_CENTER, &format!("S{}", k + 1), th.medium(10.0), th.ink3);
        let time = t.map(|t| sector_time(t as f64)).unwrap_or_else(|| "–".into());
        kit::text(p, Pos2::new(cx, y + 18.0), Align2::CENTER_CENTER, &time, th.semi(12.5), th.ink);
        if let (Some(t), Some(m)) = (t, m) {
            let d = (t - m) as f64;
            let color = if d < -0.0005 {
                th.loss
            } else if d > 0.0005 {
                th.gain
            } else {
                th.ink2
            };
            kit::text(p, Pos2::new(cx, y + 29.0), Align2::CENTER_CENTER, &signed(d, 2), th.semi(11.0), color);
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
    fn sector_times_read_like_lap_times() {
        assert_eq!(sector_time(31.234), "31.23");
        assert_eq!(sector_time(75.5), "1:15.50");
    }
}
