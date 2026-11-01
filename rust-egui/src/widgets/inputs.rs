//! Inputs: gear and speed, a steering wheel, the throttle/brake trace over the
//! last few seconds, and pedal bars. Redrawn every telemetry frame (60 Hz).

use egui::{Align2, Pos2, Rect, Shape, Stroke, Vec2};
use telemetry::model::TickPayload;

use crate::data::TracePoint;
use crate::theme::*;

pub const SIZE: Vec2 = Vec2::new(520.0, 112.0);
/// Seconds of trace shown.
const TRACE_S: f64 = 6.0;

pub fn draw(painter: &egui::Painter, rect: Rect, tick: Option<&TickPayload>, trace: &[TracePoint], imperial: bool) {
    painter.rect_filled(rect, RADIUS, PANEL);
    painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
    let inner = rect.shrink(10.0);

    // Gear and speed.
    let gear_box = Rect::from_min_size(inner.min, Vec2::new(64.0, inner.height()));
    let gear = match tick.map(|t| t.gear) {
        Some(g) if g < 0 => "R".to_string(),
        Some(0) | None => "N".to_string(),
        Some(g) => g.to_string(),
    };
    painter.text(gear_box.center_top() + Vec2::new(0.0, 2.0), Align2::CENTER_TOP, gear, bold(54.0), ACCENT);
    let speed = tick.map(|t| t.speed * if imperial { 2.23694 } else { 3.6 }).unwrap_or(0.0);
    painter.text(gear_box.center_bottom(), Align2::CENTER_BOTTOM, format!("{speed:.0} {}", if imperial { "mph" } else { "km/h" }), semibold(14.0), INK2);

    // Steering wheel, turned like the real one.
    let wheel_c = Pos2::new(gear_box.right() + 44.0, inner.center().y);
    wheel(painter, wheel_c, 34.0, tick.map(|t| t.steering).unwrap_or(0.0));

    // Pedal bars on the right.
    let bars_w = 3.0 * 14.0 + 2.0 * 6.0;
    let bars = Rect::from_min_max(Pos2::new(inner.right() - bars_w, inner.top()), inner.max);
    let pedals = [
        (tick.map(|t| t.clutch).unwrap_or(0.0), CLUTCH),
        (tick.map(|t| t.brake).unwrap_or(0.0), BRAKE),
        (tick.map(|t| t.throttle).unwrap_or(0.0), THROTTLE),
    ];
    for (k, (v, color)) in pedals.iter().enumerate() {
        let x = bars.left() + k as f32 * 20.0;
        let track = Rect::from_min_size(Pos2::new(x, bars.top() + 14.0), Vec2::new(14.0, bars.height() - 14.0));
        painter.rect_filled(track, 2.0, LINE.gamma_multiply(0.6));
        let h = track.height() * v.clamp(0.0, 1.0);
        painter.rect_filled(Rect::from_min_max(Pos2::new(track.left(), track.bottom() - h), track.max), 2.0, *color);
        painter.text(Pos2::new(track.center().x, bars.top()), Align2::CENTER_TOP, format!("{:.0}", v * 100.0), medium(11.0), INK2);
    }

    // The trace between the wheel and the bars.
    let trace_rect = Rect::from_min_max(Pos2::new(wheel_c.x + 46.0, inner.top()), Pos2::new(bars.left() - 12.0, inner.bottom()));
    trace_view(painter, trace_rect, trace);
}

fn wheel(painter: &egui::Painter, c: Pos2, r: f32, steering: f32) {
    // Positive steering is left (anticlockwise); screen angles run clockwise.
    let a = -steering;
    let rot = |v: Vec2| Vec2::new(v.x * a.cos() - v.y * a.sin(), v.x * a.sin() + v.y * a.cos());
    painter.circle_stroke(c, r, Stroke::new(6.0, INK2));
    for spoke in [Vec2::new(-r, 0.0), Vec2::new(r, 0.0), Vec2::new(0.0, r)] {
        painter.line_segment([c, c + rot(spoke * 0.86)], Stroke::new(4.0, INK3));
    }
    // Top-dead-centre marker.
    let top = c + rot(Vec2::new(0.0, -r));
    painter.circle_filled(top, 4.0, ACCENT);
    painter.circle_filled(c, 6.0, INK3);
}

fn trace_view(painter: &egui::Painter, rect: Rect, trace: &[TracePoint]) {
    painter.rect_filled(rect, 3.0, egui::Color32::from_rgba_premultiplied(0, 0, 0, 70));
    for f in [0.25, 0.5, 0.75] {
        painter.hline(rect.x_range(), rect.bottom() - rect.height() * f, Stroke::new(1.0, LINE.gamma_multiply(0.5)));
    }
    let Some(last) = trace.last() else { return };
    let x = |t: f64| rect.right() - ((last.t - t) / TRACE_S) as f32 * rect.width();
    let y = |v: f32| rect.bottom() - 2.0 - v.clamp(0.0, 1.0) * (rect.height() - 4.0);
    let start = trace.partition_point(|p| last.t - p.t > TRACE_S);
    let pts = &trace[start..];
    if pts.len() < 2 {
        return;
    }
    let line = |get: fn(&TracePoint) -> f32, color| {
        Shape::line(pts.iter().map(|p| Pos2::new(x(p.t), y(get(p)))).collect(), Stroke::new(2.0, color))
    };
    painter.add(line(|p| p.clutch, CLUTCH.gamma_multiply(0.6)));
    painter.add(line(|p| p.throttle, THROTTLE));
    painter.add(line(|p| p.brake, BRAKE));
}
