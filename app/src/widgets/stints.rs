//! Stints (`src/widgets/Stints.tsx`): for every driver, how long they've been
//! out since their last stop (live), the tyre they're on, how many stops
//! they've made, and a timeline of the whole session: each stint a bar
//! coloured by compound, with its length in laps, and a tick at each stop.

use std::collections::HashMap;

use egui::{Align, Align2, Painter, Pos2, Rangef, Rect, Stroke, Vec2};
use telemetry::model::{CarStatus, StandingRow};
use telemetry::stints::{CarStints, StintEnd};

use super::standings::{pick_rows, Slot};
use super::{Ctx, Widget};
use crate::kit::{self, compound_kind, seg, tyre_color, Badge, Col};
use crate::theme::{alpha, hex_or, Theme};

pub struct Stints;

const TIMELINE_W: f32 = 240.0;
const TIMELINE_H: f32 = 14.0;

/// A round lap step for the timeline axis, giving about 4 to 8 labels.
fn lap_step(max: f32) -> f32 {
    for s in [1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0] {
        if max / s <= 8.0 {
            return s;
        }
    }
    200.0
}

/// The classes in running order, over just these rows.
struct Group<'a> {
    class_id: i32,
    name: &'a str,
    color: &'a str,
    rows: Vec<&'a StandingRow>,
}

fn group<'a>(rows: &[&'a StandingRow]) -> Vec<Group<'a>> {
    let mut groups: Vec<Group> = Vec::new();
    for r in rows {
        match groups.iter_mut().find(|g| g.class_id == r.driver.class_id) {
            Some(g) => g.rows.push(r),
            None => groups.push(Group { class_id: r.driver.class_id, name: &r.driver.class_name, color: &r.driver.class_color, rows: vec![r] }),
        }
    }
    groups
}

#[derive(Clone, Copy)]
struct Cols {
    tyre: Option<usize>,
    laps: usize,
    stops: usize,
    timeline: Option<usize>,
}

impl Widget for Stints {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let field = cx.live.field.as_deref();
        let compounds = cx.live.compounds();
        let my_rows = o.int("rows", 1);
        let other_rows = o.int("otherClassRows", 1);
        let show_timeline = o.b("showTimeline");
        let show_lengths = o.b("showLengths");
        let show_compound = o.s("showCompound");
        let race = cx.live.race();
        // The compound column only earns its space when there's a choice of tyres.
        let compound_col = show_compound == "always" || (show_compound == "auto" && compounds.len() > 1);

        let standings: Vec<&StandingRow> = field.map(|f| f.standings.iter().filter(|r| r.position > 0 || r.status != CarStatus::Out).collect()).unwrap_or_default();
        let width = if show_timeline { 640.0 } else { 380.0 };
        let panel = kit::Panel::begin(p, th, width, None, &[]);
        if standings.is_empty() {
            let h = kit::empty(p, th, panel.x(), panel.body_top, "Stints appear once cars are on track.");
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        }
        let by_car: HashMap<i32, &CarStints> = field.map(|f| f.stints.iter().map(|c| (c.car_idx, c)).collect()).unwrap_or_default();
        let me = standings.iter().find(|r| r.is_player);
        let mut groups = group(&standings);
        if o.s("classes") == "mine" {
            if let Some(me) = me {
                groups.retain(|g| g.class_id == me.driver.class_id);
            }
        }

        // Timeline scale: laps 0 to the furthest anyone has got.
        let mut max_lap = 1.0f32;
        for c in by_car.values() {
            for s in &c.stints {
                max_lap = max_lap.max(s.end);
            }
        }
        let max_lap = max_lap.ceil();

        let mut cols = vec![Col::Px(24.0), Col::Px(8.0), Col::Px(34.0), Col::Fr];
        let mut push = |on: bool, c: Col| {
            if on {
                cols.push(c);
                Some(cols.len() - 1)
            } else {
                None
            }
        };
        let tyre = push(compound_col, Col::Px(28.0));
        let laps = push(true, Col::Px(48.0)).unwrap();
        let stops = push(true, Col::Px(36.0)).unwrap();
        let timeline = push(show_timeline, Col::Px(TIMELINE_W));
        let c = Cols { tyre, laps, stops, timeline };
        let x = kit::grid(panel.x(), 6.0, 10.0, 7.0, &cols);
        let full = panel.x();

        // The header: title and lap on the left, column labels, the lap axis.
        let mut y = panel.body_top;
        let head = Rect::from_min_size(Pos2::new(full.min, y), Vec2::new(full.span(), 30.0));
        p.hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, th.line));
        let hy = head.center().y - 0.5;
        let label_font = if th.broadcast() { th.semi(11.0) } else { th.medium(12.0) };
        let mut title = vec![seg("Stints", if th.broadcast() { th.heavy(14.0) } else { th.bold(14.0) }, th.ink)];
        if max_lap > 1.0 {
            title.push(seg(format!("   Lap {}", max_lap as i32), if th.broadcast() { th.semi(14.0) } else { th.medium(14.0) }, th.ink2));
        }
        let room = Rect::from_min_max(Pos2::new(x[0].min + 4.0, head.top()), Pos2::new(x[3].max, head.bottom()));
        if th.retro() {
            for s in &mut title {
                s.text = th.caps(&s.text);
            }
            kit::fill(p, head, 0.0, crate::theme::alpha(th.panel_raised, 0.7));
            kit::fill(p, Rect::from_min_max(Pos2::new(head.left(), head.bottom() - 2.0), head.max), 0.0, th.accent);
            let tw = kit::width(p, &title[0].text, title[0].font.clone());
            kit::retro_tab(p, th, head.left(), head.top(), head.bottom() - 2.0, room.left() - head.left() + tw + 10.0);
            title[0].color = th.panel_solid;
            // Clear of the tab's slant.
            if let Some(next) = title.get_mut(1) {
                next.text.insert_str(0, "   ");
            }
        }
        kit::segs(&p.with_clip_rect(room), room.left(), hy, false, &title, 0.0);
        let label = |col: Option<usize>, s: &str| {
            if let Some(col) = col {
                kit::text(p, Pos2::new(x[col].center(), hy), Align2::CENTER_CENTER, s, label_font.clone(), th.ink3);
            }
        };
        label(c.tyre, "Tyre");
        label(Some(c.laps), "Stint");
        label(Some(c.stops), "Stops");
        if let Some(t) = c.timeline {
            let step = lap_step(max_lap);
            let mut ticks = Vec::new();
            let mut l = step;
            while l <= max_lap {
                ticks.push(l);
                l += step;
            }
            for (k, l) in ticks.iter().enumerate() {
                let tx = x[t].min + l / max_lap * TIMELINE_W;
                let align = if k + 1 == ticks.len() { Align2::RIGHT_CENTER } else { Align2::CENTER_CENTER };
                kit::text(p, Pos2::new(tx, hy), align, &(*l as i32).to_string(), th.medium(11.0), th.ink3);
            }
        }
        y = head.bottom();

        let rh = th.row_h;
        let show_heads = groups.len() > 1;
        let player_class = me.map(|m| m.driver.class_id);
        for (gi, g) in groups.iter().enumerate() {
            let class = hex_or(g.color, th.ink);
            if gi > 0 && !th.broadcast() {
                y += 4.0;
            }
            if show_heads {
                y = class_head(p, th, full, y, g, class);
            }
            let mine = Some(g.class_id) == player_class;
            for slot in pick_rows(&g.rows, if mine { my_rows } else { other_rows }) {
                match slot {
                    Slot::Break => {
                        let ly = y + 5.0;
                        if th.broadcast() {
                            kit::fill(p, Rect::from_min_max(Pos2::new(full.min, ly - 1.0), Pos2::new(full.max, ly + 1.0)), 0.0, th.accent);
                        } else {
                            kit::dashed_h(p, Rangef::new(full.min + 10.0, full.max - 10.0), ly, th.line_strong, 1.0, 3.0, 3.0);
                        }
                        y += 9.0;
                    }
                    Slot::Row(r) => {
                        let rect = Rect::from_min_size(Pos2::new(full.min, y), Vec2::new(full.span(), rh));
                        y += rh;
                        row(p, th, &x, rect, r, by_car.get(&r.driver.car_idx).copied(), compounds, c, show_lengths, max_lap, race);
                    }
                }
            }
        }
        let bottom = y + if th.broadcast() { 0.0 } else { 3.0 };
        panel.finish(p, th, bottom)
    }
}

/// The class heading: name and cars over a wash of the class colour.
fn class_head(p: &Painter, th: &Theme, full: Rangef, y: f32, g: &Group, class: egui::Color32) -> f32 {
    let (h, margin_top, margin_bottom) = if th.broadcast() { (24.0, 0.0, 0.0) } else { (22.0, 2.0, 1.0) };
    let r = Rect::from_min_size(Pos2::new(full.min, y + margin_top), Vec2::new(full.span(), h));
    let cy = r.center().y;
    let name = if g.name.is_empty() { "Class" } else { g.name };
    let rest = format!("{} cars", g.rows.len());
    if th.broadcast() {
        kit::fill(p, r, 0.0, alpha(th.panel_raised, 0.7));
        kit::fill(p, Rect::from_min_size(r.min, Vec2::new(r.width(), 2.0)), 0.0, th.accent);
        kit::fill(p, Rect::from_min_size(Pos2::new(r.left() + 9.0, r.top() + 2.0), Vec2::new(3.0, h - 2.0)), 0.0, class);
        let n = kit::text_sp(p, Pos2::new(r.left() + 9.0 + 8.0, cy + 1.0), Align2::LEFT_CENTER, &name.to_uppercase(), th.heavy(12.0), th.ink, 0.5);
        kit::text_sp(p, Pos2::new(n.right() + 12.0, cy + 1.0), Align2::LEFT_CENTER, &rest.to_uppercase(), th.semi(12.0), th.ink3, 0.5);
    } else {
        let mut mesh = egui::epaint::Mesh::default();
        let mid = r.left() + r.width() * 0.7;
        let c0 = alpha(class, 0.16);
        let c1 = alpha(class, 0.0);
        for (xx, c) in [(r.left(), c0), (mid, c1), (r.right(), c1)] {
            mesh.colored_vertex(Pos2::new(xx, r.top()), c);
            mesh.colored_vertex(Pos2::new(xx, r.bottom()), c);
        }
        for i in 0..2u32 {
            mesh.add_triangle(i * 2, i * 2 + 1, i * 2 + 2);
            mesh.add_triangle(i * 2 + 1, i * 2 + 3, i * 2 + 2);
        }
        p.add(egui::Shape::mesh(mesh));
        p.hline(r.x_range(), r.top() + 0.5, Stroke::new(1.0, alpha(class, 0.4)));
        let n = kit::text(p, Pos2::new(r.left() + 9.0, cy), Align2::LEFT_CENTER, name, th.bold(13.0), class);
        kit::text(p, Pos2::new(n.right() + 12.0, cy), Align2::LEFT_CENTER, &rest, th.medium(13.0), th.ink3);
    }
    y + margin_top + h + margin_bottom
}

#[allow(clippy::too_many_arguments)]
fn row(p: &Painter, th: &Theme, x: &[Rangef], rect: Rect, r: &StandingRow, c: Option<&CarStints>, compounds: &[String], cols: Cols, show_lengths: bool, max_lap: f32, race: bool) {
    let cy = rect.center().y;
    let class = hex_or(&r.driver.class_color, th.ink);
    if r.is_player {
        kit::player_row(p, th, rect, class);
    }
    kit::row_rule(p, th, rect);
    let out = c.is_some_and(|c| c.out) || r.status == CarStatus::Out;
    let pos = if r.class_position > 0 { r.class_position } else { r.position };
    let label = if pos > 0 { pos.to_string() } else { "–".into() };
    let in_pits = matches!(r.status, CarStatus::PitLane | CarStatus::PitStall);
    kit::pos_cell(p, th, x[0], cy, &label, pos, in_pits);
    // A car that's out fades back, all but its place and name.
    let mut faded = p.clone();
    if out {
        faded.multiply_opacity(0.5);
    }
    kit::class_bar(&faded, th, x[1].center(), cy, class);
    kit::num_cell(&faded, th, x[2].max, cy, &r.driver.number, 15.0);
    kit::name_cell(p, th, x[3], cy, &r.driver.name, th.ink, r.is_player, None, Badge::of(r.status, race), if out { 0.5 } else { 1.0 });

    let p = &faded;
    let current = c.and_then(|c| c.stints.last());
    let live = current.filter(|s| s.open);
    let name = |s: Option<&telemetry::stints::Stint>| s.filter(|s| s.compound >= 0).and_then(|s| compounds.get(s.compound as usize)).map(String::as_str).unwrap_or("");
    if let Some(col) = cols.tyre {
        let tyre = name(live.or(current));
        if !tyre.is_empty() {
            kit::tyre_badge(p, th, Pos2::new(x[col].center(), cy), 18.0, tyre);
        }
    }
    let laps = match (live, c) {
        (Some(s), _) => format!("{}{}", if s.partial { "≥" } else { "" }, crate::logic::format::fixed((s.end - s.start) as f64, 1)),
        (None, Some(c)) if c.in_pits => "–".into(),
        _ => String::new(),
    };
    kit::cell(p, x[cols.laps], cy, Align::Center, &laps, th.bold(15.0), th.ink);
    kit::cell(p, x[cols.stops], cy, Align::Center, &c.map(|c| c.stops.to_string()).unwrap_or_else(|| "–".into()), th.medium(15.0), th.ink2);
    if let Some(t) = cols.timeline {
        timeline(p, th, x[t].min, cy - TIMELINE_H / 2.0, c, compounds, max_lap, show_lengths);
    }
}

/// Each stint as a bar, coloured by compound, with a tick at each stop.
#[allow(clippy::too_many_arguments)]
fn timeline(p: &Painter, th: &Theme, left: f32, top: f32, c: Option<&CarStints>, compounds: &[String], max_lap: f32, show_lengths: bool) {
    let x = |lap: f32| left + lap / max_lap * TIMELINE_W;
    kit::fill(p, Rect::from_min_size(Pos2::new(left, top + TIMELINE_H / 2.0 - 1.0), Vec2::new(TIMELINE_W, 2.0)), 0.0, th.grid);
    let Some(c) = c else { return };
    for s in &c.stints {
        let x0 = x(s.start);
        let w = (x(s.end) - x0).max(1.5);
        let kind = if s.compound >= 0 { compound_kind(compounds.get(s.compound as usize).map(String::as_str).unwrap_or("")) } else { kit::Tyre::Other };
        let opacity = if s.partial {
            0.4
        } else if s.open {
            1.0
        } else {
            0.75
        };
        let bar = Rect::from_min_size(Pos2::new(x0, top + 2.0), Vec2::new(w, TIMELINE_H - 4.0));
        kit::fill(p, bar, 2.0, tyre_color(th, kind).gamma_multiply(opacity));
        let laps = s.end - s.start;
        if show_lengths && w >= 16.0 {
            kit::text(p, Pos2::new(x0 + w / 2.0, top + TIMELINE_H / 2.0 + 0.5), Align2::CENTER_CENTER, &(laps.floor() as i32).to_string(), th.bold(9.0), th.panel_solid);
        }
        match s.ended_by {
            Some(StintEnd::Pit) => kit::fill(p, Rect::from_min_size(Pos2::new(x(s.end) - 0.5, top), Vec2::new(1.5, TIMELINE_H)), 0.0, th.ink),
            Some(StintEnd::Out) => {
                // A cross where the car left the sim.
                let c = Pos2::new(x(s.end) + 3.0 + 3.0, top + TIMELINE_H / 2.0);
                let st = Stroke::new(1.2, th.loss);
                p.line_segment([c + Vec2::new(-2.5, -2.5), c + Vec2::new(2.5, 2.5)], st);
                p.line_segment([c + Vec2::new(-2.5, 2.5), c + Vec2::new(2.5, -2.5)], st);
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_steps() {
        assert_eq!(lap_step(1.0), 1.0);
        assert_eq!(lap_step(8.0), 1.0);
        assert_eq!(lap_step(15.0), 2.0);
        assert_eq!(lap_step(40.0), 5.0);
        assert_eq!(lap_step(100.0), 20.0);
        assert_eq!(lap_step(5000.0), 200.0);
    }
}
