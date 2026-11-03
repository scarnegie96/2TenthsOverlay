//! Radar: your car in the middle, a translucent band beside it for each car alongside (at that
//! car's position along the track, ahead of you at the top to behind you at the bottom; red while
//! it overlaps you well, orange when only partly alongside or just pulling away), and a
//! translucent half-disc glow for the nearest car directly ahead and behind, closer to your car and
//! stronger as the gap shrinks.
//!
//! iRacing only says *that* a car is alongside (`CarLeftRight`), not where it is across the track,
//! so the bands show which side and how far along, not lanes. The side each nearby car is on is
//! the engine's (`nearby`). The top-down radar (`radar.rs`) is kept as it was, and drawn instead
//! when the widget's `style` option is "classic" (set in config.json; not offered in Settings).

use egui::epaint::Mesh;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use super::{Ctx, Widget};
use crate::config::Opts;
use crate::kit;
use crate::schema::{Def, Meta};
use crate::theme::alpha;

pub struct RadarBars;

const SIZE: f32 = 260.0;
const C: f32 = SIZE / 2.0;
/// A car's length and width, metres, and how it's drawn (pixels).
const CAR_LEN_M: f32 = 4.7;
const CAR_H: f32 = 46.0;
const CAR_W: f32 = 20.0;
/// Pixels per metre along the track for cars alongside.
const ALONG: f32 = CAR_H / CAR_LEN_M;
/// The alongside bands: how far out they reach from your car.
const BAND_REACH: f32 = 96.0;
/// The ahead / behind glows: radius, and how near and far their flat edge sits from your car.
const GLOW_R: f32 = 58.0;
const GLOW_NEAR: f32 = CAR_H / 2.0 + 4.0;
const GLOW_FAR: f32 = C - GLOW_R - 2.0;
/// A car alongside counts as well overlapped (red) within this much of level with you, metres.
const OVERLAP_M: f32 = CAR_LEN_M * 0.6;
/// A car ahead or behind is very close (red) within this, metres.
const CLOSE_M: f32 = CAR_LEN_M * 1.5;

/// The radar has no panel, so it keeps light marks on dark outlines in any theme.
const INK: Color32 = Color32::from_rgb(0xe9, 0xee, 0xf3);
const OUTLINE: Color32 = Color32::from_rgba_premultiplied(3, 6, 9, 230);

/// `CarLeftRight` values from the SDK.
const CAR_LEFT: i32 = 2;
const CAR_RIGHT: i32 = 3;
const CAR_LEFT_RIGHT: i32 = 4;
const TWO_CARS_LEFT: i32 = 5;
const TWO_CARS_RIGHT: i32 = 6;

/// The old top-down radar's options, which it still reads when drawn (`style` "classic").
static CLASSIC: Meta = Meta {
    id: "radar",
    name: "Radar",
    x: 842.0,
    y: 596.0,
    enabled: true,
    options: &[("range", Def::N(20.0)), ("alwaysShow", Def::B(true)), ("classColors", Def::B(false))],
};

/// Whether iRacing says a car is alongside on the left and on the right.
fn sides(clr: i32) -> (bool, bool) {
    (matches!(clr, CAR_LEFT | CAR_LEFT_RIGHT | TWO_CARS_LEFT), matches!(clr, CAR_RIGHT | CAR_LEFT_RIGHT | TWO_CARS_RIGHT))
}

/// A band for a car alongside `dist` metres ahead of you (negative behind): its centre's height
/// on the radar, whether it's red (well overlapped) and its strength (0..1).
pub fn band(dist: f32) -> (f32, bool, f32) {
    let y = C - (dist * ALONG).clamp(-(C - CAR_H / 2.0), C - CAR_H / 2.0);
    let d = dist.abs();
    let red = d < OVERLAP_M;
    // Full while overlapping, fading as it pulls away (it can be remembered up to ~14 m off).
    let strength = if d <= CAR_LEN_M { 1.0 } else { (1.0 - (d - CAR_LEN_M) / 10.0).clamp(0.25, 1.0) };
    (y, red, strength)
}

/// A glow for the nearest car in line `dist` metres away (either way) with the radar showing
/// `range` metres: how far its flat edge sits from your car's centre, whether it's red (very close)
/// and its strength (0..1). None out of range.
pub fn glow(dist: f32, range: f32) -> Option<(f32, bool, f32)> {
    let d = dist.abs();
    if d > range {
        return None;
    }
    let far = ((d - CAR_LEN_M) / (range - CAR_LEN_M).max(1.0)).clamp(0.0, 1.0);
    Some((GLOW_NEAR + far * (GLOW_FAR - GLOW_NEAR), d < CLOSE_M, 1.0 - 0.75 * far))
}

impl Widget for RadarBars {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        if cx.o.s("style") == "classic" {
            let o = Opts { w: cx.o.w, meta: &CLASSIC };
            let classic = Ctx {
                th: cx.th,
                o,
                general: cx.general,
                cfg: cx.cfg,
                units: cx.units,
                live: cx.live,
                ratings: cx.ratings,
                editing: cx.editing,
                windowed: cx.windowed,
                now: cx.now,
                ctx: cx.ctx,
                clicks: cx.clicks,
                pointer: cx.pointer,
            };
            return super::radar::Radar.draw(p, &classic);
        }

        let th = cx.th;
        let tick = cx.live.tick.as_ref();
        let range = (cx.o.n("range") as f32).max(CAR_LEN_M * 2.0);
        let nearby = tick.map(|t| t.nearby.as_slice()).unwrap_or(&[]);
        let (left, right) = sides(tick.map(|t| t.car_left_right).unwrap_or(0));

        // Cars alongside, by side: the engine's, or level with you where iRacing says there's a
        // car but the engine has none on that side.
        let mut bands: Vec<(f32, f32)> = nearby.iter().filter(|c| c.side != 0).map(|c| (c.side as f32, c.dist)).collect();
        for (lit, side) in [(left, -1.0), (right, 1.0)] {
            if lit && !bands.iter().any(|(s, _)| *s == side) {
                bands.push((side, 0.0));
            }
        }
        // The nearest car in line ahead and behind.
        let in_line = nearby.iter().filter(|c| c.side == 0);
        let ahead = in_line.clone().filter(|c| c.dist >= 0.0).map(|c| c.dist).reduce(f32::min).and_then(|d| glow(d, range));
        let behind = in_line.filter(|c| c.dist < 0.0).map(|c| c.dist).reduce(f32::max).and_then(|d| glow(d, range));

        if !(cx.editing || cx.o.b("alwaysShow") || !bands.is_empty() || ahead.is_some() || behind.is_some()) {
            return Vec2::ZERO;
        }

        // Faint guides through your car, as a sense of the space around it.
        let guide = Stroke::new(1.0, alpha(INK, 0.35));
        p.line_segment([Pos2::new(C, C - GLOW_FAR), Pos2::new(C, C + GLOW_FAR)], guide);
        for dy in [-CAR_H / 2.0 + 4.0, 0.0, CAR_H / 2.0 - 4.0] {
            let reach = if dy == 0.0 { BAND_REACH * 0.9 } else { BAND_REACH * 0.55 };
            p.line_segment([Pos2::new(C - reach, C + dy), Pos2::new(C + reach, C + dy)], guide);
        }

        for (dir, g) in [(-1.0, ahead), (1.0, behind)] {
            if let Some((edge, red, strength)) = g {
                let color = if red { th.loss } else { th.attention };
                half_disc(p, Pos2::new(C, C + dir * edge), dir, alpha(color, 0.55 * strength));
            }
        }
        for (side, dist) in &bands {
            let (y, red, strength) = band(*dist);
            let color = if red { th.loss } else { th.attention };
            side_band(p, *side, y, alpha(color, 0.8 * strength));
        }

        // Your car, over everything.
        let car = Rect::from_center_size(Pos2::new(C, C), Vec2::new(CAR_W, CAR_H));
        p.rect_filled(car.expand(3.0), kit::radius(7.0), Color32::from_black_alpha(36));
        p.rect_filled(car.expand(1.5), kit::radius(6.0), Color32::from_black_alpha(60));
        p.rect_filled(car, kit::radius(4.0), INK);
        p.rect_stroke(car, kit::radius(4.0), Stroke::new(1.5, OUTLINE), egui::StrokeKind::Middle);
        Vec2::splat(SIZE)
    }
}

/// A band out from your car's `side` (-1 left, 1 right), a car's length tall, centred at height
/// `y`: strongest next to your car, fading to nothing at its far end, with soft top and bottom.
fn side_band(p: &Painter, side: f32, y: f32, color: Color32) {
    let inner = C + side * (CAR_W / 2.0 + 3.0);
    let outer = C + side * (CAR_W / 2.0 + 3.0 + BAND_REACH);
    let clear = Color32::TRANSPARENT;
    // Three layers, each a little shorter and stronger, for soft top and bottom edges.
    for (h, a) in [(CAR_H * 1.25, 0.35), (CAR_H, 0.6), (CAR_H * 0.75, 1.0)] {
        let c = alpha(color, a);
        let (top, bottom) = (y - h / 2.0, y + h / 2.0);
        let mut mesh = Mesh::default();
        mesh.colored_vertex(Pos2::new(inner, top), c);
        mesh.colored_vertex(Pos2::new(inner, bottom), c);
        mesh.colored_vertex(Pos2::new(outer, top), clear);
        mesh.colored_vertex(Pos2::new(outer, bottom), clear);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 3, 2);
        p.add(Shape::mesh(mesh));
    }
}

/// A half disc glowing out from `at` (the middle of its flat edge), towards the top (`dir` -1)
/// or the bottom (1): `color` at the edge, fading to nothing at its rim.
fn half_disc(p: &Painter, at: Pos2, dir: f32, color: Color32) {
    let n = 32;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(at, color);
    for k in 0..=n {
        let a = std::f32::consts::PI * k as f32 / n as f32;
        mesh.colored_vertex(Pos2::new(at.x + GLOW_R * a.cos(), at.y + dir * GLOW_R * a.sin()), Color32::TRANSPARENT);
    }
    // The flat edge: a little colour along it too, so it reads as a line where the car is.
    for k in 0..n {
        mesh.add_triangle(0, k + 1, k + 2);
    }
    p.add(Shape::mesh(mesh));
    p.line_segment([Pos2::new(at.x - GLOW_R * 0.8, at.y), Pos2::new(at.x + GLOW_R * 0.8, at.y)], Stroke::new(2.0, alpha(color, 0.9)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_sit_where_the_car_alongside_is() {
        // Level with you: across your car's middle, red, full strength.
        assert_eq!(band(0.0), (C, true, 1.0));
        // Half a length ahead: higher up, still overlapping but not by much (orange).
        let (y, red, s) = band(CAR_LEN_M * 0.7);
        assert!(y < C && !red && s == 1.0);
        // Behind: lower down. Far off: fainter, but never gone while it's shown.
        assert!(band(-2.0).0 > C);
        assert!((band(-14.0).2 - 0.25).abs() < 0.01);
    }

    #[test]
    fn glows_come_closer_and_stronger() {
        let near = glow(5.0, 20.0).unwrap();
        let far = glow(18.0, 20.0).unwrap();
        assert!(near.0 < far.0 && near.2 > far.2);
        assert!(near.1 && !far.1, "red only when very close");
        assert!(glow(25.0, 20.0).is_none());
        // The furthest glow still fits inside the radar.
        assert!(GLOW_FAR + GLOW_R <= C);
    }

    #[test]
    fn sides_from_car_left_right() {
        assert_eq!(sides(0), (false, false));
        assert_eq!(sides(CAR_LEFT), (true, false));
        assert_eq!(sides(CAR_RIGHT), (false, true));
        assert_eq!(sides(CAR_LEFT_RIGHT), (true, true));
        assert_eq!(sides(TWO_CARS_LEFT), (true, false));
        assert_eq!(sides(TWO_CARS_RIGHT), (false, true));
    }
}
