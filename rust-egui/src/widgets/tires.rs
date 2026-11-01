//! Tires (`src/widgets/Tires.tsx`): carcass temperatures and tread for each
//! corner. iRacing only updates them in the pit box, so it shows the last
//! reading and says whether it's live.

use egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, Vec2};
use telemetry::model::TireCorner;

use super::{Ctx, Widget};
use crate::config::Units;
use crate::kit::{self, seg};
use crate::logic::format::{js_round, pressure, temp};
use crate::theme::{alpha, Theme};

pub struct Tires;

const NAMES: [&str; 4] = ["LF", "RF", "LR", "RR"];

/// `hsl(h 62% 38%)` as a colour.
fn hsl(h: f32, s: f32, l: f32) -> Color32 {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let q = |v: f32| ((v + m) * 255.0).round() as u8;
    Color32::from_rgb(q(r), q(g), q(b))
}

/// Cold blue, working green, overheating red.
pub fn heat(c: f32) -> Color32 {
    const STOPS: [(f32, f32); 5] = [(40.0, 215.0), (70.0, 175.0), (85.0, 135.0), (100.0, 45.0), (115.0, 0.0)];
    let mut hue = STOPS[0].1;
    if c >= STOPS[4].0 {
        hue = STOPS[4].1;
    }
    for w in STOPS.windows(2) {
        let ((t0, h0), (t1, h1)) = (w[0], w[1]);
        if c >= t0 && c < t1 {
            hue = h0 + (c - t0) / (t1 - t0) * (h1 - h0);
        }
    }
    hsl(hue, 0.62, 0.38)
}

fn wear_color(th: &Theme, w: f32) -> Color32 {
    if w > 0.7 {
        th.gain
    } else if w > 0.4 {
        th.attention
    } else {
        th.loss
    }
}

impl Widget for Tires {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let tires = cx.live.field.as_ref().map(|f| &f.tires);
        let sets = tires.map(|t| match t.sets_available {
            None => format!("{} sets used", t.sets_used),
            Some(a) => format!("{a} of {} sets left", t.sets_used + a),
        });
        let mut parts: Vec<String> = Vec::new();
        if let Some(t) = tires {
            if !t.compound.is_empty() {
                parts.push(t.compound.clone());
            }
        }
        parts.extend(sets);
        let aside = if parts.is_empty() { vec![] } else { vec![seg(parts.join("   "), kit::aside_font(th), th.ink2)] };
        let panel = kit::Panel::begin(p, th, 280.0, Some("Tires"), &aside);
        let x = panel.x();
        let top = panel.body_top;

        let Some(t) = tires.filter(|t| t.has_data) else {
            let h = kit::empty(p, th, x, top, "Readings appear after you stop in your pit box.");
            return panel.finish(p, th, top + h);
        };

        let col_w = (x.span() - 20.0 - 18.0) / 2.0;
        let (cell_h, corner_h) = (23.0, 51.0);
        let mut y = top + 10.0;
        for row in 0..2 {
            for col in 0..2 {
                let k = row * 2 + col;
                let Some(c) = t.corners.get(k) else { continue };
                let left = x.min + 10.0 + col as f32 * (col_w + 18.0);
                corner(p, th, Rect::from_min_size(Pos2::new(left, y), Vec2::new(col_w, corner_h)), NAMES[k], c, cx.units, cx.o.b("showPressure"), cell_h);
            }
            y += corner_h + 10.0;
        }

        // Live or last reading.
        p.hline(x, y + 0.5, Stroke::new(1.0, th.line));
        y += 1.0;
        let (note, color) = if t.live { ("Live while you're in the pit box", th.gain) } else { ("From your last pit stop", th.ink3) };
        kit::text(p, Pos2::new(x.min + 10.0, y + 6.0 + 8.0), Align2::LEFT_CENTER, note, th.medium(13.0), color);
        panel.finish(p, th, y + 6.0 + 16.0 + 6.0)
    }
}

fn corner(p: &Painter, th: &Theme, r: Rect, short: &str, c: &TireCorner, units: Units, show_pressure: bool, cell_h: f32) {
    // Three temperature cells, outer ones rounded on the outside.
    let w = (r.width() - 4.0) / 3.0;
    for (j, &t) in c.temp.iter().enumerate() {
        let cell = Rect::from_min_size(Pos2::new(r.left() + j as f32 * (w + 2.0), r.top()), Vec2::new(w, cell_h));
        let cr = match j {
            0 => CornerRadius { nw: 6, sw: 6, ne: 3, se: 3 },
            2 => CornerRadius { nw: 3, sw: 3, ne: 6, se: 6 },
            _ => CornerRadius::same(3),
        };
        p.rect_filled(cell, cr, heat(t));
        kit::text(p, cell.center(), Align2::CENTER_CENTER, &format!("{}", js_round(temp(t, units) as f64)), th.bold(14.0), Color32::WHITE);
    }
    // Tread remaining.
    let wy = r.top() + cell_h + 4.0;
    for (j, &wear) in c.wear.iter().enumerate() {
        let tread = Rect::from_min_size(Pos2::new(r.left() + j as f32 * (w + 2.0), wy), Vec2::new(w, 4.0));
        kit::fill(p, tread, 2.0, alpha(th.ink, 0.08));
        let f = wear.clamp(0.0, 1.0);
        if f > 0.0 {
            let right = if f > 0.97 { 2 } else { 0 };
            let cr = CornerRadius { nw: 2, sw: 2, ne: right, se: right };
            p.rect_filled(Rect::from_min_size(tread.min, Vec2::new(w * f, 4.0)), cr, wear_color(th, wear));
        }
    }
    // Position, least tread left, cold pressure.
    let my = wy + 4.0 + 4.0 + 8.0;
    let least = c.wear.iter().cloned().fold(f32::INFINITY, f32::min);
    let a = kit::text(p, Pos2::new(r.left(), my), Align2::LEFT_CENTER, short, th.bold(13.0), th.ink3);
    kit::text(p, Pos2::new(a.right() + 6.0, my), Align2::LEFT_CENTER, &format!("{}%", js_round(least as f64 * 100.0)), th.semi(13.0), th.ink);
    if show_pressure && c.cold_pressure > 0.0 {
        kit::text(p, Pos2::new(r.right(), my), Align2::RIGHT_CENTER, &pressure(c.cold_pressure, units), th.medium(13.0), th.ink3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heat_runs_from_blue_through_green_to_red() {
        assert_eq!(heat(20.0), hsl(215.0, 0.62, 0.38));
        assert_eq!(heat(130.0), hsl(0.0, 0.62, 0.38));
        assert_eq!(heat(85.0), hsl(135.0, 0.62, 0.38));
        // Halfway between 70 and 85: hue 155.
        assert_eq!(heat(77.5), hsl(155.0, 0.62, 0.38));
    }

    #[test]
    fn hsl_matches_css() {
        // hsl(0 62% 38%) = rgb(157, 37, 37)
        assert_eq!(hsl(0.0, 0.62, 0.38), Color32::from_rgb(157, 37, 37));
    }
}
