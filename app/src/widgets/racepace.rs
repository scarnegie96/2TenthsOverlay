//! Race pace (`src/widgets/RacePace.tsx`): everyone in your class against the
//! class leader over the race. The leader is the flat line along the top;
//! every other car is a line below it at its gap to the leader, so a line
//! sloping down is a car slower than the leader and one rising is catching.
//! Lap lines mark when the leader started each lap.

use egui::{Align2, Color32, Painter, Pos2, Shape, Stroke, Vec2};
use telemetry::model::CarStatus;

use super::{Ctx, Widget};
use crate::data::Live;
use crate::kit::{self, seg};
use crate::logic::race_history::{lap_trend, on_scale, CarTrace, RaceHistory};
use crate::theme::{alpha, Theme};

const W: f32 = 520.0;
const H: f32 = 250.0;
/// Plot area inside the canvas: room for gap labels left, lap labels below, names right.
const PAD_LEFT: f32 = 40.0;
const PAD_RIGHT: f32 = 132.0;
const PAD_TOP: f32 = 12.0;
const PAD_BOTTOM: f32 = 20.0;
const LABEL_H: f32 = 13.0;

#[derive(Default)]
pub struct RacePace {
    history: RaceHistory,
    seen: u64,
}

/// A round step for the gap gridlines, giving 3 to 6 lines.
fn nice_step(max: f32) -> f32 {
    for s in [1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0] {
        if max / s <= 6.0 {
            return s;
        }
    }
    600.0
}

fn fmt_gap(s: f32) -> String {
    if s >= 60.0 {
        format!("{}:{:02}", (s / 60.0).floor(), crate::logic::format::js_round((s % 60.0) as f64))
    } else {
        format!("{s:.1}")
    }
}

impl Widget for RacePace {
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
        let rows = field.map(|f| f.standings.as_slice()).unwrap_or(&[]);
        let has_data = rows.iter().any(|r| r.position > 0);
        let leader = {
            let me = rows.iter().find(|r| r.is_player);
            let class = me.map(|r| r.driver.class_id).or_else(|| rows.first().map(|r| r.driver.class_id));
            rows.iter().find(|r| Some(r.driver.class_id) == class && r.class_position == 1).map(|r| r.driver.name.clone())
        };
        let aside = match leader {
            Some(name) => format!("Gap to {name}"),
            None => "Gap to class leader".to_string(),
        };
        let aside = [seg(aside, kit::aside_font(th), th.ink2)];
        let panel = kit::Panel::begin(p, th, W + 20.0, Some("Race pace"), &aside);
        let msg = if !cx.live.race() {
            Some("The pace chart runs in races.")
        } else if !has_data {
            Some("Lines appear once the race is under way.")
        } else {
            None
        };
        if let Some(msg) = msg {
            let h = kit::empty(p, th, panel.x(), panel.body_top, msg);
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        }
        let top = panel.body_top + 6.0;
        let o = cx.o;
        let opts = Opts {
            window: o.s("window").to_string(),
            show: o.s("show").to_string(),
            around: o.int("around", 1) as i32,
            max_gap: o.n("maxGap") as f32,
            show_trend: o.b("showTrend"),
        };
        chart(p, th, &self.history, &opts, Pos2::new(10.0, top));
        panel.finish(p, th, top + H + 8.0)
    }
}

struct Opts {
    window: String,
    show: String,
    around: i32,
    max_gap: f32,
    show_trend: bool,
}

fn chart(p: &Painter, th: &Theme, h: &RaceHistory, o: &Opts, origin: Pos2) {
    let pt = |x: f32, y: f32| Pos2::new(origin.x + x, origin.y + y);
    let (x0, x1, y0, y1) = (PAD_LEFT, W - PAD_RIGHT, PAD_TOP, H - PAD_BOTTOM);
    let mut all: Vec<&CarTrace> = h.cars.values().collect();
    all.sort_by_key(|c| c.car_idx);
    let me = all.iter().find(|c| c.is_player).copied();
    let Some(class_id) = me.map(|c| c.class_id).or_else(|| all.first().map(|c| c.class_id)) else { return };
    let mut cars: Vec<&CarTrace> = all.iter().filter(|c| c.class_id == class_id).copied().collect();
    if let (true, Some(me)) = (o.show == "around", me) {
        cars.retain(|c| c.position == 1 || (c.position - me.position).abs() <= o.around);
    }
    let Some(start) = h.start.filter(|_| !cars.is_empty()) else { return };

    // Time window: the whole race, or the leader's last few laps.
    let marks = h.laps.get(&class_id).map(|m| m.as_slice()).unwrap_or(&[]);
    let t_end = h.now;
    let mut t_start = start;
    if o.window != "race" {
        if let Ok(n) = o.window.parse::<usize>() {
            if n > 0 && marks.len() >= n {
                t_start = marks[marks.len() - n].t;
            }
        }
    }
    let span = (t_end - t_start).max(1.0);
    let x_at = |t: f64| x0 + ((t - t_start) / span) as f32 * (x1 - x0);

    // Gap scale: fit the cars still on the lead lap, unless capped. Lapped
    // and retired cars would squash everyone else, so they run along the
    // bottom edge instead.
    let mut most = 0f32;
    for c in cars.iter().filter(|c| on_scale(c)) {
        most = most.max(c.gap);
        for pair in c.points.chunks_exact(2) {
            if pair[0] as f64 >= t_start {
                most = most.max(pair[1]);
            }
        }
    }
    let max = if o.max_gap > 0.0 { o.max_gap } else { 5f32.max(most * 1.08) };
    let y_at = |g: f32| y0 + (g.min(max) / max) * (y1 - y0);

    // Grid: gap lines with labels on the left.
    let step = nice_step(max);
    let mut g = step;
    while g <= max {
        let y = y_at(g).round() + 0.5;
        p.line_segment([pt(x0, y), pt(x1, y)], Stroke::new(1.0, th.grid));
        let label = if g >= 60.0 { format!("+{}", fmt_gap(g)) } else { format!("+{g}s") };
        kit::text(p, pt(x0 - 6.0, y), Align2::RIGHT_CENTER, &label, th.medium(10.0), th.ink3);
        g += step;
    }
    // Lap lines, labelled along the bottom (thinned so labels don't crowd).
    let visible: Vec<_> = marks.iter().filter(|m| m.t >= t_start).collect();
    let every = ((visible.len() as f32 / 10.0).ceil() as usize).max(1);
    for (k, m) in visible.iter().enumerate() {
        let x = x_at(m.t).round() + 0.5;
        p.add(Shape::dashed_line(&[pt(x, y0), pt(x, y1)], Stroke::new(1.0, th.grid), 2.0, 3.0));
        if (visible.len() - 1 - k) % every == 0 {
            kit::text(p, pt(x, H - 5.0), Align2::CENTER_BOTTOM, &format!("L{}", m.lap), th.medium(10.0), th.ink3);
        }
    }

    // Each car's line, you last so you're on top.
    let mut order = cars.clone();
    order.sort_by(|a, b| a.is_player.cmp(&b.is_player).then(b.position.cmp(&a.position)));
    for c in &order {
        if c.position == 1 {
            continue;
        }
        let out = c.status == CarStatus::Out;
        let (color, width) = if c.is_player { (th.attention, 2.5) } else { (alpha(th.ink2, if out { 0.25 } else { 0.55 }), 1.4) };
        let mut pts: Vec<Pos2> = Vec::new();
        let n = c.points.len() / 2;
        for i in 0..n {
            let t = c.points[i * 2] as f64;
            // Keep the point just before the window so the line enters from the edge.
            if t < t_start && !(i + 1 < n && c.points[(i + 1) * 2] as f64 >= t_start) {
                continue;
            }
            pts.push(pt(x_at(t).max(x0), y_at(c.points[i * 2 + 1])));
        }
        if c.t >= t_start {
            pts.push(pt(x_at(c.t), y_at(c.gap)));
        }
        if pts.len() < 2 {
            continue;
        }
        let stroke = Stroke::new(width, color);
        if out {
            p.add(Shape::dashed_line(&pts, stroke, 3.0, 3.0));
        } else {
            p.add(Shape::line(pts, stroke));
        }
    }

    // The leader: a flat line along the top.
    let lead = cars.iter().find(|c| c.position == 1);
    let lead_player = lead.is_some_and(|c| c.is_player);
    p.line_segment([pt(x0, y0), pt(x1, y0)], Stroke::new(if lead_player { 2.5 } else { 2.0 }, if lead_player { th.attention } else { th.ink }));

    // Names on the right at each car's current gap, nudged apart so they don't overlap.
    let mut labels: Vec<(&CarTrace, f32)> = cars
        .iter()
        .filter(|c| c.status != CarStatus::Out || c.t >= t_start)
        .map(|c| (*c, if c.position == 1 { y0 } else { y_at(c.gap) }))
        .collect();
    labels.sort_by_key(|(c, _)| c.position);
    for i in 1..labels.len() {
        labels[i].1 = labels[i].1.max(labels[i - 1].1 + LABEL_H);
    }
    if let Some(last) = labels.last() {
        let overflow = last.1 - (H - 6.0);
        if overflow > 0.0 {
            let n = labels.len();
            labels[n - 1].1 -= overflow;
            for i in (0..n - 1).rev() {
                labels[i].1 = labels[i].1.min(labels[i + 1].1 - LABEL_H);
            }
        }
    }
    for (c, y) in &labels {
        let ly = y.max(6.0);
        let dot = if c.position == 1 { y0 } else { y_at(c.gap) };
        let color = if c.is_player {
            th.attention
        } else if c.position == 1 {
            th.ink
        } else {
            th.ink2
        };
        // Current position dot and a leader line to the label.
        p.circle_filled(pt(x1, dot), if c.is_player { 3.5 } else { 2.5 }, color);
        p.line_segment([pt(x1 + 4.0, dot), pt(x1 + 10.0, ly)], Stroke::new(1.0, alpha(color, 0.4)));

        let name = format!("{} {}", c.position, if c.abbrev.is_empty() { &c.name } else { &c.abbrev });
        let name = if name.chars().count() > 15 { format!("{}…", name.chars().take(14).collect::<String>()) } else { name };
        kit::text(p, pt(x1 + 13.0, ly), Align2::LEFT_CENTER, &name, if c.is_player { th.bold(11.0) } else { th.semi(11.0) }, color);
        let right = pt(W - 2.0, ly);
        let font = th.medium(11.0);
        if c.position == 1 {
            kit::text(p, right, Align2::RIGHT_CENTER, "Leader", font, th.ink3);
            continue;
        }
        let trend = if o.show_trend && c.status != CarStatus::Out { lap_trend(h, c) } else { None };
        let gap_text = if c.status == CarStatus::Out {
            "Out".to_string()
        } else if c.laps_down > 0 {
            format!("+{}L", c.laps_down)
        } else {
            format!("+{}", fmt_gap(c.gap))
        };
        match trend {
            Some(t) if t.abs() >= 0.05 => {
                // Gaining on the leader (gap shrinking) is green, losing is red.
                let col: Color32 = if t < 0.0 { th.gain } else { th.loss };
                kit::text(p, right, Align2::RIGHT_CENTER, &format!("{}{:.1}", if t < 0.0 { "▲" } else { "▼" }, t.abs()), font.clone(), col);
                kit::text(p, pt(W - 34.0, ly), Align2::RIGHT_CENTER, &gap_text, font, th.ink3);
            }
            _ => {
                kit::text(p, right, Align2::RIGHT_CENTER, &gap_text, font, th.ink3);
            }
        }
    }
}
