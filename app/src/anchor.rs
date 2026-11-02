//! Where widgets go: anchors to an edge or the centre (`src/overlay/anchor.ts`)
//! and snapping to other widgets while dragging (`src/overlay/snapping.ts`).

use egui::{Pos2, Rect, Vec2};

use crate::config::{AnchorX, AnchorY, WidgetCfg};

/// The screen the default layout (and layouts saved before anchoring) was made on.
pub const REFERENCE: Vec2 = Vec2::new(1920.0, 1080.0);

pub fn anchor_for(left: f32, top: f32, size: Vec2, screen: Vec2) -> (AnchorX, AnchorY) {
    let cx = left + size.x / 2.0;
    let cy = top + size.y / 2.0;
    let ax = if cx < screen.x / 3.0 {
        AnchorX::L
    } else if cx > screen.x * 2.0 / 3.0 {
        AnchorX::R
    } else {
        AnchorX::C
    };
    let ay = if cy < screen.y / 3.0 {
        AnchorY::T
    } else if cy > screen.y * 2.0 / 3.0 {
        AnchorY::B
    } else {
        AnchorY::M
    };
    (ax, ay)
}

/// Offsets from the anchor, rounded to whole pixels.
pub fn to_anchored(left: f32, top: f32, size: Vec2, screen: Vec2) -> (AnchorX, AnchorY, f32, f32) {
    let (ax, ay) = anchor_for(left, top, size, screen);
    let x = match ax {
        AnchorX::L => left,
        AnchorX::R => screen.x - (left + size.x),
        AnchorX::C => left + size.x / 2.0 - screen.x / 2.0,
    };
    let y = match ay {
        AnchorY::T => top,
        AnchorY::B => screen.y - (top + size.y),
        AnchorY::M => top + size.y / 2.0 - screen.y / 2.0,
    };
    (ax, ay, js_round(x), js_round(y))
}

/// `Math.round`: halves go up, also for negatives.
fn js_round(v: f32) -> f32 {
    (v + 0.5).floor()
}

/// Top-left corner on this screen. `unscaled` is the size at 100%: positions
/// without an anchor are for that size on a 1920×1080 screen.
pub fn resolve(c: &WidgetCfg, size: Vec2, screen: Vec2, unscaled: Vec2) -> Pos2 {
    let (ax, ay, x, y) = match c.anchor {
        Some((ax, ay)) => (ax, ay, c.x as f32, c.y as f32),
        None => to_anchored(c.x as f32, c.y as f32, unscaled, REFERENCE),
    };
    let left = match ax {
        AnchorX::L => x,
        AnchorX::R => screen.x - size.x - x,
        AnchorX::C => screen.x / 2.0 + x - size.x / 2.0,
    };
    let top = match ay {
        AnchorY::T => y,
        AnchorY::B => screen.y - size.y - y,
        AnchorY::M => screen.y / 2.0 + y - size.y / 2.0,
    };
    clamp_on_screen(left, top, size, screen)
}

pub fn clamp_on_screen(left: f32, top: f32, size: Vec2, screen: Vec2) -> Pos2 {
    let max_x = (screen.x - size.x).max(0.0);
    let max_y = (screen.y - size.y).max(0.0);
    Pos2::new(js_round(left.clamp(0.0, max_x)), js_round(top.clamp(0.0, max_y)))
}

const THRESHOLD: f32 = 8.0;
const GRID: f32 = 4.0;

/// A guide line to draw while snapping: vertical at x, or horizontal at y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Guide {
    X(f32),
    Y(f32),
}

/// Snaps a moving rect to the edges and centres of the others and of the
/// screen, else to a 4 px grid, and keeps it on screen.
pub fn snap(moving: Rect, others: &[Rect], screen: Vec2) -> (Pos2, Vec<Guide>) {
    let mut xs = vec![0.0, screen.x / 2.0, screen.x];
    let mut ys = vec![0.0, screen.y / 2.0, screen.y];
    for o in others {
        xs.extend([o.left(), o.right(), o.center().x]);
        ys.extend([o.top(), o.bottom(), o.center().y]);
    }
    let best = |start: f32, size: f32, lines: &[f32]| {
        let mut hit: Option<(f32, f32)> = None;
        for edge in [start, start + size / 2.0, start + size] {
            for &line in lines {
                let d = line - edge;
                if d.abs() <= THRESHOLD && hit.is_none_or(|(hd, _)| d.abs() < hd.abs()) {
                    hit = Some((d, line));
                }
            }
        }
        hit
    };
    let mut guides = Vec::new();
    let (mut x, mut y) = (moving.left(), moving.top());
    match best(x, moving.width(), &xs) {
        Some((d, line)) => {
            x += d;
            guides.push(Guide::X(line));
        }
        None => x = (x / GRID).round() * GRID,
    }
    match best(y, moving.height(), &ys) {
        Some((d, line)) => {
            y += d;
            guides.push(Guide::Y(line));
        }
        None => y = (y / GRID).round() * GRID,
    }
    x = x.min((screen.x - moving.width()).max(0.0)).max(0.0);
    y = y.min((screen.y - moving.height()).max(0.0)).max(0.0);
    (Pos2::new(x, y), guides)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{normalize, Config};

    fn cfg(x: f64, y: f64, anchor: Option<(AnchorX, AnchorY)>) -> WidgetCfg {
        let mut w = Config::default().widget("relative").clone();
        w.x = x;
        w.y = y;
        w.anchor = anchor;
        w
    }

    #[test]
    fn unanchored_layouts_keep_their_edge_on_bigger_screens() {
        // Default Relative (1396, 600), 520 wide: right-hand third, middle.
        let size = Vec2::new(520.0, 240.0);
        let c = cfg(1396.0, 600.0, None);
        assert_eq!(resolve(&c, size, REFERENCE, size), Pos2::new(1396.0, 600.0));
        let p = resolve(&c, size, Vec2::new(2560.0, 1440.0), size);
        // Same distance from the right edge (1920 - 1396 - 520 = 4).
        assert_eq!(p.x, 2560.0 - 520.0 - 4.0);
        // Same offset of its centre from the screen's centre.
        assert_eq!(p.y, 720.0 + (600.0 + 120.0 - 540.0) - 120.0);
    }

    #[test]
    fn anchored_round_trip() {
        let screen = Vec2::new(2560.0, 1440.0);
        let size = Vec2::new(300.0, 200.0);
        for (left, top) in [(10.0, 10.0), (1200.0, 700.0), (2200.0, 1200.0), (100.0, 1230.0)] {
            let (ax, ay, x, y) = to_anchored(left, top, size, screen);
            let c = cfg(x as f64, y as f64, Some((ax, ay)));
            assert_eq!(resolve(&c, size, screen, size), Pos2::new(left, top));
        }
    }

    #[test]
    fn pulled_back_on_screen() {
        let c = cfg(5000.0, -40.0, Some((AnchorX::L, AnchorY::T)));
        let p = resolve(&c, Vec2::new(100.0, 50.0), Vec2::new(1920.0, 1080.0), Vec2::new(100.0, 50.0));
        assert_eq!(p, Pos2::new(1820.0, 0.0));
    }

    #[test]
    fn snaps_to_neighbours_and_grid() {
        let screen = Vec2::new(1920.0, 1080.0);
        let other = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(200.0, 100.0));
        // Left edge 5 px right of the other's right edge: snaps onto it.
        let (p, g) = snap(Rect::from_min_size(Pos2::new(305.0, 503.0), Vec2::new(50.0, 50.0)), &[other], screen);
        assert_eq!(p, Pos2::new(300.0, 504.0));
        assert_eq!(g, vec![Guide::X(300.0)]);
        // Centre lines up with the screen's centre.
        let (p, _) = snap(Rect::from_min_size(Pos2::new(913.0, 700.0), Vec2::new(100.0, 40.0)), &[], screen);
        assert_eq!(p.x, 910.0);
        let _ = normalize(&serde_json::Value::Null);
    }
}
