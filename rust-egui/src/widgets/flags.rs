//! Flags (`src/widgets/Flags.tsx`): a trackside LED flag board (28x28 dots
//! with a halo) and a caption in the flag's colour under it. The dot meshes
//! are built once per pattern and reused every frame.

use std::sync::Arc;

use egui::epaint::{Mesh, Shadow};
use egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, TextureHandle, TextureId, Vec2};

use super::{Ctx, Widget};
use crate::kit;
use crate::logic::flags::{board_for, lit_dots, sc_text, Board, GreenWindow, Pattern, DOTS};

const PITCH: f32 = 5.5;
const SIZE: f32 = DOTS as f32 * PITCH;
const PAD: f32 = 7.0;
const UNLIT: Color32 = Color32::from_rgb(0x1b, 0x1f, 0x25);
const HOUSING: Color32 = Color32::from_rgb(0x0a, 0x0c, 0x0f);
/// Seconds per half of a flash.
const FLASH: f64 = 0.3;

#[derive(Default)]
pub struct Flags {
    green: GreenWindow,
    dot: Option<TextureHandle>,
    /// The lit board for the current pattern, and every dot dark (flash off).
    lit: Option<(Pattern, Arc<Mesh>)>,
    dark: Option<Arc<Mesh>>,
}

/// A soft-edged white disc, tinted per dot.
fn disc(ctx: &egui::Context) -> TextureHandle {
    const N: usize = 64;
    let mut px = Vec::with_capacity(N * N);
    for y in 0..N {
        for x in 0..N {
            let d = (x as f32 + 0.5 - N as f32 / 2.0).hypot(y as f32 + 0.5 - N as f32 / 2.0);
            px.push(Color32::from_white_alpha(((N as f32 / 2.0 - 0.5 - d).clamp(0.0, 1.0) * 255.0) as u8));
        }
    }
    ctx.load_texture("flag-dot", egui::ColorImage::new([N, N], px), egui::TextureOptions::LINEAR)
}

fn dot(mesh: &mut Mesh, c: Pos2, r: f32, color: Color32) {
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    mesh.add_rect_with_uv(Rect::from_center_size(c, Vec2::splat(r * 2.0)), uv, color);
}

/// Every dot: a soft halo under each lit one, then the core.
fn build(tex: TextureId, dots: &[Option<Color32>]) -> Arc<Mesh> {
    let mut m = Mesh::with_texture(tex);
    for (i, d) in dots.iter().enumerate() {
        let c = Pos2::new(PAD + ((i % DOTS) as f32 + 0.5) * PITCH, PAD + ((i / DOTS) as f32 + 0.5) * PITCH);
        if let Some(color) = d {
            let [r, g, b, _] = color.to_srgba_unmultiplied();
            dot(&mut m, c, PITCH * 0.62, Color32::from_rgba_unmultiplied(r, g, b, (0.28f32 * 255.0) as u8));
        }
    }
    for (i, d) in dots.iter().enumerate() {
        let c = Pos2::new(PAD + ((i % DOTS) as f32 + 0.5) * PITCH, PAD + ((i / DOTS) as f32 + 0.5) * PITCH);
        dot(&mut m, c, PITCH * 0.36, d.unwrap_or(UNLIT));
    }
    Arc::new(m)
}

/// Greedy word wrap to `width`.
fn wrap(p: &Painter, s: &str, font: &egui::FontId, width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(line) if kit::width(p, &format!("{line} {word}"), font.clone()) <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// `text-wrap: balance`: the narrowest width that keeps the same line count.
fn balanced(p: &Painter, s: &str, font: &egui::FontId, width: f32) -> Vec<String> {
    let full = wrap(p, s, font, width);
    let (mut lo, mut hi) = (0.0f32, width);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if wrap(p, s, font, mid).len() <= full.len() {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    wrap(p, s, font, hi)
}

impl Widget for Flags {
    fn observe(&mut self, live: &crate::data::Live, now: f64) {
        self.green.observe(live.field.as_ref().map(|f| f.clock.flags), now);
    }

    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let flags = cx.live.field.as_ref().map(|f| f.clock.flags).unwrap_or(0);
        let race = cx.live.race();
        let show_green = race && self.green.open(cx.now, o.n("greenSeconds"));
        let board: Option<Board> = board_for(flags, show_green, &sc_text(o.s("scBoard")), race);
        if board.is_none() && !cx.editing && !o.b("alwaysShow") {
            return Vec2::ZERO;
        }
        let caption = match &board {
            Some(b) => match cx.o.w.options.get(b.text_key).and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => cx.o.s(b.text_key).to_string(),
            },
            None if cx.editing => "No flag".into(),
            None => String::new(),
        };
        let caption = if o.b("showCaption") { caption } else { String::new() };
        let flag_color = board.as_ref().map(|b| b.color).unwrap_or(th.ink3);

        // The meshes, rebuilt only when the pattern changes.
        let tex = self.dot.get_or_insert_with(|| disc(cx.ctx)).id();
        let pattern = board.as_ref().map(|b| b.pattern.clone()).unwrap_or(Pattern::Off);
        if self.lit.as_ref().is_none_or(|(k, _)| *k != pattern) {
            self.lit = Some((pattern.clone(), build(tex, &lit_dots(&pattern))));
        }
        let dark = self.dark.get_or_insert_with(|| build(tex, &lit_dots(&Pattern::Off))).clone();

        // The housing: board, then the caption bar across the bottom.
        let w = SIZE + 2.0 * PAD;
        let font = th.bold(16.0);
        let lines = if caption.is_empty() { vec![] } else { balanced(p, &caption, &font, w - 20.0) };
        let cap_h = if lines.is_empty() { 0.0 } else { lines.len() as f32 * 19.2 + 12.0 };
        let size = Vec2::new(w, w + cap_h);
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        let r = kit::radius(8.0);
        p.add(Shadow { offset: [0, 2], blur: 10, spread: 0, color: Color32::from_black_alpha(115) }.as_shape(rect, r));
        p.rect_filled(rect, r, HOUSING);
        p.rect_stroke(rect.expand(0.5), kit::radius(8.5), Stroke::new(1.0, Color32::from_white_alpha(15)), StrokeKind::Middle);

        let on = !board.as_ref().is_some_and(|b| b.flash) || ((cx.now / FLASH) as u64).is_multiple_of(2);
        let mesh = if on { self.lit.as_ref().unwrap().1.clone() } else { dark };
        p.add(Shape::mesh(mesh));

        if cap_h > 0.0 {
            let bar = Rect::from_min_max(Pos2::new(0.0, w), size.to_pos2());
            p.rect_filled(bar, CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 }, flag_color);
            for (i, line) in lines.iter().enumerate() {
                kit::text(p, Pos2::new(w / 2.0, bar.top() + 6.0 + i as f32 * 19.2 + 9.6), Align2::CENTER_CENTER, line, font.clone(), Color32::BLACK);
            }
        }
        size
    }
}
