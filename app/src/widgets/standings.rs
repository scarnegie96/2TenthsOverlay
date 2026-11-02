//! Standings (`src/widgets/Standings.tsx`): the field by class, your class
//! around you, with places gained, iRating and its estimated change, gaps,
//! laps, and where you'd rejoin after a stop. Outside a race it's cut down to
//! laps, gap to the class best, last and best lap, ranked by best lap.

use std::collections::HashMap;

use egui::{Align, Align2, Painter, Pos2, Rect, Stroke, Vec2};
use telemetry::model::{CarStatus, FieldPayload, StandingRow};

use super::relative::session_aside;
use super::{Click, ClickAction, Ctx, Widget};
use crate::kit::{self, seg, Badge, Col, Thumb};
use crate::logic::format::{gap as fmt_gap, irating, lap_time};
use crate::logic::{irating as ir, rejoin};
use crate::theme::{alpha, hex_or, Theme};

pub struct Standings;

#[derive(Debug, Clone, Copy)]
pub enum Slot<'a> {
    Row(&'a StandingRow),
    Break,
}

/// The top of a class plus a window around the player when they're further back.
pub fn pick_rows<'a>(rows: &[&'a StandingRow], max: usize) -> Vec<Slot<'a>> {
    if rows.len() <= max {
        return rows.iter().map(|r| Slot::Row(r)).collect();
    }
    let me = rows.iter().position(|r| r.is_player);
    match me {
        Some(me) if me >= max => {
            let around = 5.min(max / 2);
            let top = max - around;
            let start = (rows.len() - around).min(me - around / 2);
            let mut out: Vec<Slot> = rows[..top].iter().map(|r| Slot::Row(r)).collect();
            out.push(Slot::Break);
            out.extend(rows[start..(start + around).min(rows.len())].iter().map(|r| Slot::Row(r)));
            out
        }
        _ => rows[..max].iter().map(|r| Slot::Row(r)).collect(),
    }
}

pub struct Group<'a> {
    pub class_id: i32,
    pub name: &'a str,
    pub color: &'a str,
    pub rows: Vec<&'a StandingRow>,
}

/// Classes in running order (the class of the overall leader first).
pub fn group_by_class(rows: &[StandingRow]) -> Vec<Group<'_>> {
    let mut groups: Vec<Group> = Vec::new();
    for r in rows {
        match groups.iter_mut().find(|g| g.class_id == r.driver.class_id) {
            Some(g) => g.rows.push(r),
            None => groups.push(Group { class_id: r.driver.class_id, name: &r.driver.class_name, color: &r.driver.class_color, rows: vec![r] }),
        }
    }
    groups
}

/// Each car's estimated iRating change if the race finished now; cars that
/// never got a position count as non-starters.
fn class_irating_changes(rows: &[&StandingRow]) -> HashMap<i32, f64> {
    let mut started: Vec<&&StandingRow> = rows.iter().filter(|r| r.position > 0).collect();
    started.sort_by_key(|r| r.class_position);
    let rest: Vec<&&StandingRow> = rows.iter().filter(|r| r.position <= 0).collect();
    let n = started.len();
    let entries: Vec<ir::Entry> = started
        .iter()
        .enumerate()
        .map(|(k, r)| ir::Entry { car_idx: r.driver.car_idx, irating: r.driver.irating, rank: k as i32 + 1, started: true })
        .chain(rest.iter().enumerate().map(|(k, r)| ir::Entry {
            car_idx: r.driver.car_idx,
            irating: r.driver.irating,
            rank: (n + k) as i32 + 1,
            started: false,
        }))
        .collect();
    ir::changes(&entries)
}

/// A race gap is "6.2"; in practice and qualifying the gap is a lap-time difference: "+0.269".
fn gap_cell(r: &StandingRow, interval: bool, race: bool) -> (String, bool) {
    if r.laps_down > 0 {
        return (format!("+{}L", r.laps_down), false);
    }
    match if interval { r.interval } else { r.gap } {
        Some(v) if race => (fmt_gap(Some(v), 1), false),
        Some(v) => (format!("+{}", fmt_gap(Some(v), 3)), false),
        None if r.class_position == 1 => ("Leader".into(), true),
        None => (String::new(), false),
    }
}

/// "1:02.3" from 62.3 s; "9.4" under a minute.
fn fmt_secs(s: f32) -> String {
    if s >= 60.0 {
        format!("{}:{:0>4}", (s / 60.0).floor(), format!("{:.1}", s % 60.0))
    } else {
        format!("{s:.1}")
    }
}

enum Item<'a> {
    Row(&'a StandingRow),
    Break,
    Rejoin,
}

/// Puts the rejoin marker after the car you'd come out behind, or in place
/// of the break when that car is in the rows left out.
fn with_rejoin<'a>(slots: &[Slot<'a>], rejoin: Option<&rejoin::Rejoin>) -> Vec<Item<'a>> {
    let mut out: Vec<Item> = slots
        .iter()
        .map(|s| match s {
            Slot::Row(r) => Item::Row(r),
            Slot::Break => Item::Break,
        })
        .collect();
    let Some(rj) = rejoin else { return out };
    let after = rj.behind.and_then(|b| slots.iter().position(|s| matches!(s, Slot::Row(r) if r.driver.car_idx == b.row.driver.car_idx)));
    if let Some(a) = after {
        out.insert(a + 1, Item::Rejoin);
        return out;
    }
    let brk = slots.iter().position(|s| matches!(s, Slot::Break));
    match (rj.behind, brk) {
        (Some(_), Some(b)) => out[b] = Item::Rejoin,
        (None, _) => out.insert(0, Item::Rejoin),
        _ => out.push(Item::Rejoin),
    }
    out
}

impl Widget for Standings {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let field: Option<&FieldPayload> = cx.live.field.as_deref();
        let session = cx.live.session.as_deref();
        let my_rows = o.int("rows", 1);
        let other_rows = o.int("otherClassRows", 1);
        let interval_mode = o.s("gapMode") == "interval";
        let race = cx.live.race();
        // Practice and qualifying: a cut-down table (laps, gap, last, best) without the race's iRating.
        let (show_last, show_best, show_ir, show_sof, show_marks) = (o.b("showLast"), o.b("showBest"), o.b("showIRating") && race, o.b("showSoF"), o.b("showRatings"));
        let show_laps = !race && o.b("showLaps");
        let show_ir_change = show_ir && o.b("showIRatingChange") && race && session.is_some_and(|s| s.official);
        let all: &[StandingRow] = field.map(|f| f.standings.as_slice()).unwrap_or(&[]);
        let show_change = o.b("showChange") && all.iter().any(|r| r.start_position.is_some());
        let player_class = all.iter().find(|r| r.is_player).map(|r| r.driver.class_id).or(session.map(|s| s.player_class_id));
        let loss = field.and_then(|f| f.pit_loss.iter().find(|l| Some(l.class_id) == player_class));
        let my_row = all.iter().find(|r| r.is_player);
        let finished = field.is_some_and(|f| f.clock.player_finished);
        // Once you've taken the flag: thumbs up or down for the other drivers.
        // TENTHS_EGUI_VOTING=1 shows the thumbs without finishing a race, to check them.
        let voting = race && (finished || std::env::var_os("TENTHS_EGUI_VOTING").is_some()) && o.b("rateDrivers");
        let show_rejoin = o.b("showRejoin") && race && !finished && loss.is_some() && my_row.is_some_and(|r| r.status == CarStatus::Track);
        let mut groups = group_by_class(all);
        if o.b("myClassOnly") {
            groups.retain(|g| Some(g.class_id) == player_class);
        }
        let show_heads = groups.len() > 1;
        let class_sof = |id: i32| field.and_then(|f| f.clock.classes.iter().find(|c| c.class_id == id)).and_then(|c| c.strength_of_field);
        let head_sof = if show_sof && !show_heads && groups.len() == 1 { class_sof(groups[0].class_id) } else { None };

        let panel = kit::Panel::begin(p, th, if voting { 576.0 } else { 520.0 }, None, &[]);
        if all.is_empty() {
            let h = kit::empty(p, th, panel.x(), panel.body_top, "Standings appear when a session is running.");
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        }

        let mut cols = vec![Col::Px(24.0), Col::Px(8.0), Col::Px(34.0), Col::Fr];
        let mut push = |on: bool, c: Col| {
            if on {
                cols.push(c);
                Some(cols.len() - 1)
            } else {
                None
            }
        };
        let c_change = push(show_change, Col::Px(34.0));
        let c_ir = push(show_ir, Col::Px(if show_ir_change { 74.0 } else { 40.0 }));
        let c_laps = push(show_laps, Col::Px(40.0));
        let c_gap = push(true, Col::Px(if race { 54.0 } else { 66.0 })).unwrap();
        let c_last = push(show_last, Col::Px(62.0));
        let c_best = push(show_best, Col::Px(62.0));
        let c_rate = push(voting, Col::Px(52.0));
        let x = kit::grid(panel.x(), 6.0, 10.0, 7.0, &cols);
        let full = panel.x();

        // One header row: track and lap on the left, column labels on the right.
        let mut y = panel.body_top;
        let head = Rect::from_min_size(Pos2::new(full.min, y), Vec2::new(full.span(), 30.0));
        let spacing = if th.f1() { 0.5 } else { 0.0 };
        if th.f1() {
            kit::fill(p, head, 0.0, alpha(th.panel_raised, 0.7));
            kit::fill(p, Rect::from_min_max(Pos2::new(head.left(), head.bottom() - 2.0), head.max), 0.0, th.f1_red);
        } else {
            p.hline(head.x_range(), head.bottom() - 0.5, Stroke::new(1.0, th.line));
        }
        let hy = head.center().y - 0.5;
        let label_font = if th.f1() { th.semi(11.0) } else { th.medium(12.0) };
        let label = |c: Option<usize>, s: &str| {
            if let Some(c) = c {
                kit::text_sp(p, Pos2::new(x[c].center(), hy), Align2::CENTER_CENTER, &th.caps(s), label_font.clone(), th.ink3, spacing);
            }
        };
        label(c_change, "+/−");
        label(c_ir, "iR");
        label(c_laps, "Laps");
        label(Some(c_gap), if interval_mode { "Int" } else { "Gap" });
        label(c_last, "Last");
        label(c_best, "Best");
        label(c_rate, "Rate");
        let mut title = vec![seg(session.map(|s| s.track_name.as_str()).filter(|s| !s.is_empty()).unwrap_or("Standings"), if th.f1() { th.heavy(14.0) } else { th.bold(14.0) }, th.ink)];
        let aside_font = if th.f1() { th.semi(14.0) } else { th.medium(14.0) };
        if let Some(a) = session_aside(field.map(|f| &f.clock)) {
            title.push(seg(format!("   {a}"), aside_font.clone(), th.ink2));
        }
        if let Some(s) = head_sof.filter(|s| *s > 0) {
            title.push(seg(format!("   SoF {}", irating(s)), aside_font, th.ink2));
        }
        for s in &mut title {
            s.text = th.caps(&s.text);
        }
        let title_room = Rect::from_min_max(Pos2::new(x[0].min + 4.0, head.top()), Pos2::new(x[3].max, head.bottom()));
        let clip = p.with_clip_rect(title_room);
        kit::segs(&clip, title_room.left(), hy, false, &title, spacing);
        y = head.bottom();

        let rh = th.row_h;
        for (gi, g) in groups.iter().enumerate() {
            let class = hex_or(g.color, th.ink);
            if gi > 0 && !th.f1() {
                y += 4.0;
            }
            let mine = Some(g.class_id) == player_class;
            if show_heads {
                y = class_head(p, th, full, y, g, class, if show_sof { class_sof(g.class_id) } else { None });
            }
            let changes = if show_ir_change { class_irating_changes(&g.rows) } else { HashMap::new() };
            let slots = pick_rows(&g.rows, if mine { my_rows } else { other_rows });
            let rj = if show_rejoin && mine { loss.and_then(|l| rejoin::after_stop(&g.rows, l.loss_s)) } else { None };
            for item in with_rejoin(&slots, rj.as_ref()) {
                match item {
                    Item::Break => {
                        let ly = y + 5.0;
                        if th.f1() {
                            kit::fill(p, Rect::from_min_max(Pos2::new(full.min, ly - 1.0), Pos2::new(full.max, ly + 1.0)), 0.0, th.f1_red);
                        } else {
                            kit::dashed_h(p, egui::Rangef::new(full.min + 10.0, full.max - 10.0), ly, th.line_strong, 1.0, 3.0, 3.0);
                        }
                        y += 9.0;
                    }
                    Item::Rejoin => {
                        let Some(rj) = rj.as_ref() else { continue };
                        let (pos, loss) = (rj.position, rj.loss);
                        let r = Rect::from_min_size(Pos2::new(full.min + 6.0, y + 1.0), Vec2::new(full.span() - 12.0, 24.0));
                        kit::fill(p, r, 4.0, alpha(th.attention, 0.10));
                        kit::dashed_rect(p, r, th.attention, 1.0, 3.0, 3.0);
                        let cy = r.center().y;
                        let tag = kit::text_sp(p, Pos2::new(r.left() + 8.0, cy), Align2::LEFT_CENTER, "REJOIN AFTER PIT", th.heavy(11.0), th.attention, 0.55);
                        let pr = kit::text(p, Pos2::new(tag.right() + 10.0, cy), Align2::LEFT_CENTER, &format!("P{pos}"), th.heavy(13.0), th.ink);
                        let loss_r = kit::text(p, Pos2::new(r.right() - 8.0, cy), Align2::RIGHT_CENTER, &format!("stop ≈{loss:.0}s"), th.medium(13.0), th.ink3);
                        let mut parts = Vec::new();
                        if let Some(b) = rj.behind {
                            parts.push(seg(fmt_secs(b.by), th.bold(13.0), th.ink));
                            parts.push(seg(format!(" behind #{}", b.row.driver.number), th.medium(13.0), th.ink2));
                        }
                        if rj.behind.is_some() && rj.ahead.is_some() {
                            parts.push(seg("  ·  ", th.medium(13.0), th.ink3));
                        }
                        if let Some(a) = rj.ahead {
                            parts.push(seg(fmt_secs(a.by), th.bold(13.0), th.ink));
                            parts.push(seg(format!(" ahead of #{}", a.row.driver.number), th.medium(13.0), th.ink2));
                        }
                        let room = Rect::from_min_max(Pos2::new(pr.right() + 10.0, r.top()), Pos2::new(loss_r.left() - 10.0, r.bottom()));
                        kit::segs(&p.with_clip_rect(room), room.left(), cy, false, &parts, 0.0);
                        y += 26.0;
                    }
                    Item::Row(r) => {
                        let rect = Rect::from_min_size(Pos2::new(full.min, y), Vec2::new(full.span(), rh));
                        y += rh;
                        row(p, th, cx, &x, rect, r, RowCols { c_change, c_ir, c_laps, c_gap, c_last, c_best, c_rate, interval_mode, race, show_marks }, changes.get(&r.driver.car_idx).copied());
                    }
                }
            }
        }
        let bottom = y + if th.f1() { 0.0 } else { 3.0 };
        panel.finish(p, th, bottom)
    }
}

#[derive(Clone, Copy)]
struct RowCols {
    c_change: Option<usize>,
    c_ir: Option<usize>,
    c_laps: Option<usize>,
    c_gap: usize,
    c_last: Option<usize>,
    c_best: Option<usize>,
    c_rate: Option<usize>,
    interval_mode: bool,
    race: bool,
    show_marks: bool,
}

/// The class heading: name, cars and SoF over a wash of the class colour.
fn class_head(p: &Painter, th: &Theme, full: egui::Rangef, y: f32, g: &Group, class: egui::Color32, sof: Option<i32>) -> f32 {
    let (h, margin_top, margin_bottom) = if th.f1() { (24.0, 0.0, 0.0) } else { (22.0, 2.0, 1.0) };
    let r = Rect::from_min_size(Pos2::new(full.min, y + margin_top), Vec2::new(full.span(), h));
    let cy = r.center().y;
    let name = if g.name.is_empty() { "Class" } else { g.name };
    let rest = format!("{} cars{}", g.rows.len(), sof.filter(|s| *s > 0).map(|s| format!("   SoF {}", irating(s))).unwrap_or_default());
    if th.f1() {
        kit::fill(p, r, 0.0, alpha(th.panel_raised, 0.7));
        kit::fill(p, Rect::from_min_size(r.min, Vec2::new(r.width(), 2.0)), 0.0, th.f1_red);
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
fn row(p: &Painter, th: &Theme, cx: &Ctx, x: &[egui::Rangef], rect: Rect, r: &StandingRow, c: RowCols, change: Option<f64>) {
    let cy = rect.center().y;
    let class = hex_or(&r.driver.class_color, th.ink);
    if r.is_player {
        kit::player_row(p, th, rect, class);
    }
    kit::row_rule(p, th, rect);
    // A car that's out of the race fades back, all but its place and name.
    let out = r.status == CarStatus::Out;
    let fade = |col: egui::Color32| if out { col.gamma_multiply(0.5) } else { col };
    let pos = if r.class_position > 0 { r.class_position } else { r.position };
    let label = if pos > 0 { pos.to_string() } else { "–".into() };
    let in_pits = matches!(r.status, CarStatus::PitLane | CarStatus::PitStall);
    kit::pos_cell(p, th, x[0], cy, &label, pos, in_pits);
    kit::class_bar(p, th, x[1].center(), cy, fade(class));
    {
        let mut faded = p.clone();
        if out {
            faded.multiply_opacity(0.5);
        }
        kit::num_cell(&faded, th, x[2].max, cy, &r.driver.number, 15.0);
    }
    let mark = if c.show_marks && !r.is_player { cx.ratings.get(r.driver.user_id) } else { None };
    kit::name_cell(p, th, x[3], cy, &r.driver.name, th.ink, r.is_player, mark, Badge::of(r.status, c.race), if out { 0.5 } else { 1.0 });
    let mut faded = p.clone();
    if out {
        faded.multiply_opacity(0.5);
    }
    let p = &faded;
    if let Some(col) = c.c_change {
        if let (Some(start), true) = (r.start_position, r.class_position > 0) {
            let d = start - r.class_position;
            let (s, color) = match d.cmp(&0) {
                std::cmp::Ordering::Equal => ("–".to_string(), th.ink3),
                std::cmp::Ordering::Greater => (format!("▲{d}"), th.gain),
                std::cmp::Ordering::Less => (format!("▼{}", -d), th.loss),
            };
            kit::cell(p, x[col], cy, Align::Center, &s, th.bold(14.0), color);
        }
    }
    if let Some(col) = c.c_ir {
        kit::ir_badge(p, th, x[col], cy, Align::Center, r.driver.irating, change.map(|v| v as f32));
    }
    if let Some(col) = c.c_laps {
        let laps = if r.lap > 0 { r.lap.to_string() } else { "–".into() };
        kit::cell(p, x[col], cy, Align::Center, &laps, th.semi(15.0), th.ink2);
    }
    let (g, leader) = gap_cell(r, c.interval_mode, c.race);
    let (font, color) = if leader { (th.medium(15.0), th.ink3) } else { (th.semi(15.0), th.ink) };
    kit::cell(p, x[c.c_gap], cy, Align::Center, &g, font, color);
    if let Some(col) = c.c_last {
        let (font, color) = kit::last_lap_style(th, r.laps.last_flag);
        kit::cell(p, x[col], cy, Align::Center, &lap_time(r.laps.last_lap), font, color);
    }
    if let Some(col) = c.c_best {
        let (font, color) = kit::best_lap_style(th, r.laps.class_fastest);
        kit::cell(p, x[col], cy, Align::Center, &lap_time(r.laps.best_lap), font, color);
    }
    if let (Some(col), false, true) = (c.c_rate, r.is_player, r.driver.user_id > 0) {
        rate_buttons(p, th, cx, x[col].center(), cy, r);
    }
}

/// Thumbs up and down for a driver (`RateButtons`): white until picked, then
/// green or red; picking the same one again clears it. They take clicks even
/// while the overlay is click-through.
fn rate_buttons(p: &Painter, th: &Theme, cx: &Ctx, cx_mid: f32, cy: f32, r: &StandingRow) {
    let current = cx.ratings.get(r.driver.user_id);
    let (w, h, gap) = (22.0, 20.0, 4.0);
    for (rating, left) in [(Thumb::Up, cx_mid - gap / 2.0 - w), (Thumb::Down, cx_mid + gap / 2.0)] {
        let rect = Rect::from_min_size(Pos2::new(left, cy - h / 2.0), Vec2::new(w, h));
        let on = current == Some(rating);
        let hover = cx.pointer.is_some_and(|m| rect.contains(m));
        if hover {
            kit::fill(p, rect, 4.0, alpha(th.ink, 0.12));
        }
        // White (the light theme: grey) until picked.
        let idle = if th.light() { th.ink2 } else { egui::Color32::WHITE };
        let color = match (on, rating) {
            (true, Thumb::Up) => th.gain,
            (true, Thumb::Down) => th.loss,
            _ => idle.gamma_multiply(0.85),
        };
        kit::thumb(p, rect.center(), 14.0, rating == Thumb::Down, color);
        cx.clicks.borrow_mut().push(Click {
            rect,
            action: ClickAction::Rate { user_id: r.driver.user_id, name: r.driver.name.clone(), rating: if on { None } else { Some(rating) } },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::rejoin::tests::row as mk;

    #[test]
    fn picks_top_and_window_around_player() {
        let rows: Vec<StandingRow> = (1..=20).map(|i| mk(i, i, Some(i as f32), i == 15)).collect();
        let refs: Vec<&StandingRow> = rows.iter().collect();
        let slots = pick_rows(&refs, 10);
        // 5 at the top, a break, then 5 around P15.
        assert_eq!(slots.len(), 11);
        assert!(matches!(slots[5], Slot::Break));
        let shown: Vec<i32> = slots.iter().filter_map(|s| if let Slot::Row(r) = s { Some(r.class_position) } else { None }).collect();
        assert_eq!(shown, vec![1, 2, 3, 4, 5, 13, 14, 15, 16, 17]);
        // Near the top: just the top rows.
        let rows: Vec<StandingRow> = (1..=20).map(|i| mk(i, i, Some(i as f32), i == 3)).collect();
        let refs: Vec<&StandingRow> = rows.iter().collect();
        assert_eq!(pick_rows(&refs, 10).len(), 10);
    }

    #[test]
    fn gaps() {
        let mut r = mk(1, 1, None, false);
        assert_eq!(gap_cell(&r, false, true), ("Leader".into(), true));
        r.class_position = 2;
        r.gap = Some(6.25);
        assert_eq!(gap_cell(&r, false, true).0, "6.2");
        // Practice: the difference in best laps, to the thousandth.
        r.gap = Some(0.2689);
        assert_eq!(gap_cell(&r, false, false).0, "+0.269");
        r.laps_down = 11;
        assert_eq!(gap_cell(&r, false, true).0, "+11L");
        assert_eq!(fmt_secs(62.34), "1:02.3");
        assert_eq!(fmt_secs(9.44), "9.4");
    }
}
