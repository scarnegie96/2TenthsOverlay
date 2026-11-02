//! Radar (`src/widgets/Radar.tsx`): a top-down view of the cars around you,
//! with no panel behind it. Distance along the track is exact; cars alongside
//! are placed in the lane iRacing says they're in (`CarLeftRight`).

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use super::{Ctx, Widget};
use crate::kit;
use crate::theme::{alpha, hex_or};

pub struct Radar;

const SIZE: f32 = 236.0;
const C: f32 = SIZE / 2.0;
const RADIUS: f32 = 100.0;
const CAR_LEN: f32 = 4.7;
const CAR_WID: f32 = 2.0;
/// Lateral distance between lane centres, metres.
const LANE: f32 = 3.8;

/// The radar has no panel, so it keeps light marks on dark outlines in any theme.
const INK: Color32 = Color32::from_rgb(0xe9, 0xee, 0xf3);
const FAR: Color32 = Color32::from_rgb(0xb9, 0xc4, 0xd0);
const OUTLINE: Color32 = Color32::from_rgba_premultiplied(3, 6, 9, 230);

/// `CarLeftRight` values from the SDK.
const CAR_LEFT: i32 = 2;
const CAR_RIGHT: i32 = 3;
const CAR_LEFT_RIGHT: i32 = 4;
const TWO_CARS_LEFT: i32 = 5;
const TWO_CARS_RIGHT: i32 = 6;

fn sides(clr: i32) -> (bool, bool) {
    (matches!(clr, CAR_LEFT | CAR_LEFT_RIGHT | TWO_CARS_LEFT), matches!(clr, CAR_RIGHT | CAR_LEFT_RIGHT | TWO_CARS_RIGHT))
}

impl Widget for Radar {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let tick = cx.live.tick.as_ref();
        let range = (cx.o.n("range") as f32).max(1.0);
        let scale = RADIUS / range;
        // Keep whole cars inside the radius so they never collide with the distance labels.
        let cars: Vec<_> = tick.map(|t| t.nearby.iter().filter(|c| c.dist.abs() <= range - CAR_LEN / 2.0).collect()).unwrap_or_default();
        let (left, right) = sides(tick.map(|t| t.car_left_right).unwrap_or(0));
        let visible = cx.editing || cx.o.b("alwaysShow") || !cars.is_empty();
        if !visible {
            return Vec2::ZERO;
        }
        let class_colors = cx.o.b("classColors");

        side_warning(p, th.attention, false, left);
        side_warning(p, th.attention, true, right);

        let car_rect = |side: f32, dist: f32| {
            let size = Vec2::new(CAR_WID * scale, CAR_LEN * scale);
            Rect::from_center_size(Pos2::new(C + side * LANE * scale, C - dist * scale), size)
        };
        let rounding = kit::radius((CAR_WID * scale * 0.3).min(4.0));
        let mark = |r: Rect, fill: Color32| {
            // A dark halo for readability over bright sky or kerbs, then the outline.
            p.rect_filled(r.expand(3.0), kit::radius(7.0), Color32::from_black_alpha(36));
            p.rect_filled(r.expand(1.5), kit::radius(5.0), Color32::from_black_alpha(60));
            p.rect_filled(r, rounding, fill);
            p.rect_stroke(r, rounding, Stroke::new(1.5, OUTLINE), egui::StrokeKind::Middle);
        };
        for c in &cars {
            let d = c.dist.abs();
            let fill = if d < CAR_LEN {
                th.loss
            } else if d < CAR_LEN * 2.2 {
                th.attention
            } else if class_colors {
                hex_or(&c.class_color, FAR)
            } else {
                FAR
            };
            mark(car_rect(c.side as f32, c.dist), fill);
        }
        mark(car_rect(0.0, 0.0), INK);

        // Distance readouts are for the cars in line with us, not the ones alongside.
        let in_line = cars.iter().filter(|c| c.side == 0);
        let ahead = in_line.clone().filter(|c| c.dist >= 0.0).map(|c| c.dist).fold(None, |a: Option<f32>, d| Some(a.map_or(d, |a| a.min(d))));
        let behind = in_line.filter(|c| c.dist < 0.0).map(|c| c.dist).fold(None, |a: Option<f32>, d| Some(a.map_or(d, |a| a.max(d))));
        if let Some(d) = ahead {
            dist_label(p, th, C - RADIUS - 5.0, d);
        }
        if let Some(d) = behind {
            dist_label(p, th, C + RADIUS + 13.0, d);
        }
        Vec2::splat(SIZE)
    }
}

/// An arc hugging the left or right edge of the radar, +-35 degrees from horizontal.
fn side_warning(p: &Painter, attention: Color32, right: bool, on: bool) {
    let r = RADIUS - 6.0;
    let sx = if right { 1.0 } else { -1.0 };
    let n = 24;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = (-35.0 + 70.0 * i as f32 / n as f32).to_radians();
            Pos2::new(C + sx * r * a.cos(), C + r * a.sin())
        })
        .collect();
    let line = |width: f32, color: Color32| {
        p.add(Shape::line(pts.clone(), Stroke::new(width, color)));
        for e in [pts[0], pts[n]] {
            p.circle_filled(e, width / 2.0, color);
        }
    };
    // The dark halo under every mark, and a glow when lit.
    line(10.0, Color32::from_black_alpha(46));
    line(8.0, Color32::from_black_alpha(70));
    if on {
        line(14.0, alpha(attention, 0.18));
        line(10.0, alpha(attention, 0.30));
        line(6.0, attention);
    } else {
        line(6.0, Color32::from_rgba_unmultiplied(0xe9, 0xee, 0xf3, 82));
    }
}

fn dist_label(p: &Painter, th: &crate::theme::Theme, baseline: f32, dist: f32) {
    let s = format!("{} m", crate::logic::format::fixed(dist.abs() as f64, 1));
    let font = th.semi(12.0);
    let w = kit::width(p, &s, font.clone());
    let x = C - w / 2.0;
    // The dark outline (paint-order: stroke), then the text.
    for (dx, dy) in [(-1.5, 0.0), (1.5, 0.0), (0.0, -1.5), (0.0, 1.5), (-1.1, -1.1), (1.1, -1.1), (-1.1, 1.1), (1.1, 1.1)] {
        let shade = Color32::from_rgba_unmultiplied(4, 7, 11, 217);
        kit::segs_baseline(p, x + dx, baseline + dy, &[kit::seg(s.clone(), font.clone(), shade)]);
    }
    kit::segs_baseline(p, x, baseline, &[kit::seg(s, font, INK)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_cars_light_the_right_arcs() {
        assert_eq!(sides(0), (false, false));
        assert_eq!(sides(1), (false, false));
        assert_eq!(sides(CAR_LEFT), (true, false));
        assert_eq!(sides(CAR_RIGHT), (false, true));
        assert_eq!(sides(CAR_LEFT_RIGHT), (true, true));
        assert_eq!(sides(TWO_CARS_LEFT), (true, false));
        assert_eq!(sides(TWO_CARS_RIGHT), (false, true));
    }
}
