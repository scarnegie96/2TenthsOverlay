//! The pieces every widget draws: panel chrome per theme, text helpers, table
//! columns, and the timing-table cells from `src/widgets/cells.tsx`
//! (position, class stripe, car number, iRating, licence, tyre and status
//! badges, lap-time colours, rating thumbs) plus a sparkline.
//!
//! Widgets draw at their own origin (0, 0) in CSS pixels; the frame moves and
//! scales the result into place.

use std::sync::Arc;

use egui::epaint::{Mesh, Shadow};
use egui::layers::ShapeIdx;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align, Align2, Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rangef, Rect, Shape, Stroke, StrokeKind, Vec2};

use crate::theme::{alpha, Theme, W};

// --- Text ----------------------------------------------------------------------

/// Lays out one line of text, with extra letter spacing in px.
pub fn galley(p: &Painter, s: &str, font: FontId, color: Color32, spacing: f32) -> Arc<Galley> {
    if spacing == 0.0 {
        return p.layout_no_wrap(s.to_owned(), font, color);
    }
    let mut job = LayoutJob::default();
    job.append(s, 0.0, TextFormat { font_id: font, color, extra_letter_spacing: spacing, valign: Align::Center, ..Default::default() });
    p.layout_job(job)
}

/// Puts a laid-out galley with its `align` corner at `pos`; returns its rect.
pub fn put(p: &Painter, pos: Pos2, align: Align2, g: Arc<Galley>, color: Color32) -> Rect {
    let rect = align.anchor_size(pos, g.size());
    p.galley(rect.min, g, color);
    rect
}

pub fn text(p: &Painter, pos: Pos2, align: Align2, s: &str, font: FontId, color: Color32) -> Rect {
    if s.is_empty() {
        return Rect::from_min_size(pos, Vec2::ZERO);
    }
    let g = p.layout_no_wrap(s.to_owned(), font, color);
    put(p, pos, align, g, color)
}

/// Text with letter spacing (Box Box headings, small capitals labels).
pub fn text_sp(p: &Painter, pos: Pos2, align: Align2, s: &str, font: FontId, color: Color32, spacing: f32) -> Rect {
    if s.is_empty() {
        return Rect::from_min_size(pos, Vec2::ZERO);
    }
    let g = galley(p, s, font, color, spacing);
    put(p, pos, align, g, color)
}

/// One line cut short with "…" when wider than `max_w`.
pub fn text_fit(p: &Painter, pos: Pos2, align: Align2, s: &str, font: FontId, color: Color32, max_w: f32, spacing: f32) -> Rect {
    if s.is_empty() || max_w <= 0.0 {
        return Rect::from_min_size(pos, Vec2::ZERO);
    }
    let mut job = LayoutJob::default();
    job.append(s, 0.0, TextFormat { font_id: font, color, extra_letter_spacing: spacing, ..Default::default() });
    job.wrap = TextWrapping::truncate_at_width(max_w);
    let g = p.layout_job(job);
    put(p, pos, align, g, color)
}

/// Wrapped text in `max_w`; returns its rect.
pub fn text_wrap(p: &Painter, pos: Pos2, s: &str, font: FontId, color: Color32, max_w: f32) -> Rect {
    let g = p.layout(s.to_owned(), font, color, max_w);
    put(p, pos, Align2::LEFT_TOP, g, color)
}

pub fn width(p: &Painter, s: &str, font: FontId) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    p.layout_no_wrap(s.to_owned(), font, Color32::WHITE).size().x
}

pub fn width_sp(p: &Painter, s: &str, font: FontId, spacing: f32) -> f32 {
    if s.is_empty() {
        return 0.0;
    }
    galley(p, s, font, Color32::WHITE, spacing).size().x
}

/// Runs of differently styled text on one line, drawn left to right.
#[derive(Clone)]
pub struct Seg {
    pub text: String,
    pub font: FontId,
    pub color: Color32,
}

pub fn seg(text: impl Into<String>, font: FontId, color: Color32) -> Seg {
    Seg { text: text.into(), font, color }
}

pub fn segs_width(p: &Painter, segs: &[Seg], spacing: f32) -> f32 {
    segs.iter().map(|s| width_sp(p, &s.text, s.font.clone(), spacing)).sum()
}

/// Draws segments with their vertical centres on `y`, from `x` (or ending
/// at `x` with RIGHT alignment). Returns the x range used.
pub fn segs(p: &Painter, x: f32, y: f32, right: bool, list: &[Seg], spacing: f32) -> Rangef {
    let total = segs_width(p, list, spacing);
    let mut cx = if right { x - total } else { x };
    let start = cx;
    for s in list {
        let r = text_sp(p, Pos2::new(cx, y), Align2::LEFT_CENTER, &s.text, s.font.clone(), s.color, spacing);
        cx += r.width();
    }
    Rangef::new(start, cx)
}

/// Like `segs`, but on a shared baseline: big numbers with small units after them.
pub fn segs_baseline(p: &Painter, x: f32, baseline: f32, list: &[Seg]) -> Rangef {
    let mut cx = x;
    for s in list {
        if s.text.is_empty() {
            continue;
        }
        let g = p.layout_no_wrap(s.text.clone(), s.font.clone(), s.color);
        let ascent = g.rows.first().map(|r| r.row.glyphs.first().map(|g| g.font_ascent).unwrap_or(r.size.y * 0.8)).unwrap_or(0.0);
        let w = g.size().x;
        p.galley(Pos2::new(cx, baseline - ascent), g, s.color);
        cx += w;
    }
    Rangef::new(x, cx)
}

// --- Shapes --------------------------------------------------------------------------

pub fn radius(r: f32) -> CornerRadius {
    CornerRadius::same(r.round().clamp(0.0, 255.0) as u8)
}

pub fn fill(p: &Painter, rect: Rect, r: f32, color: Color32) {
    p.rect_filled(rect, radius(r), color);
}

pub fn outline(p: &Painter, rect: Rect, r: f32, width: f32, color: Color32) {
    p.rect_stroke(rect, radius(r), Stroke::new(width, color), StrokeKind::Inside);
}

/// A left-to-right gradient.
pub fn hgradient(p: &Painter, rect: Rect, left: Color32, right: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), left);
    mesh.colored_vertex(rect.right_top(), right);
    mesh.colored_vertex(rect.right_bottom(), right);
    mesh.colored_vertex(rect.left_bottom(), left);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// A pill (fully rounded ends) filled with a left-to-right gradient through
/// `stops` (position 0..1, colour).
pub fn pill_gradient(p: &Painter, rect: Rect, stops: &[(f32, Color32)]) {
    let color_at = |f: f32| {
        let f = f.clamp(0.0, 1.0);
        for w in stops.windows(2) {
            let ((a, ca), (b, cb)) = (w[0], w[1]);
            if f <= b {
                let t = if b > a { (f - a) / (b - a) } else { 0.0 };
                return crate::theme::mix(cb, ca, t);
            }
        }
        stops.last().map(|s| s.1).unwrap_or(Color32::TRANSPARENT)
    };
    let r = rect.height() / 2.0;
    let n = 64;
    let mut mesh = Mesh::default();
    for i in 0..=n {
        let x = rect.left() + rect.width() * i as f32 / n as f32;
        // Half height at x: round within r of either end.
        let d = (x - rect.left()).min(rect.right() - x);
        let h = if d < r { (r * r - (r - d) * (r - d)).max(0.0).sqrt() } else { r };
        let c = color_at((x - rect.left()) / rect.width());
        mesh.colored_vertex(Pos2::new(x, rect.center().y - h), c);
        mesh.colored_vertex(Pos2::new(x, rect.center().y + h), c);
        if i > 0 {
            let b = (i * 2) as u32;
            mesh.add_triangle(b - 2, b - 1, b);
            mesh.add_triangle(b - 1, b + 1, b);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// A top-to-bottom gradient.
pub fn vgradient(p: &Painter, rect: Rect, top: Color32, bottom: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
}

/// A filled polygon of any shape (triangulated as a fan around its centre,
/// which suits the star-shaped outlines drawn here).
pub fn fan(p: &Painter, pts: &[Pos2], color: Color32) {
    if pts.len() < 3 {
        return;
    }
    let c = pts.iter().fold(Vec2::ZERO, |a, q| a + q.to_vec2()) / pts.len() as f32;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c.to_pos2(), color);
    for q in pts {
        mesh.colored_vertex(*q, color);
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

pub fn dashed_h(p: &Painter, x: Rangef, y: f32, color: Color32, width: f32, dash: f32, gap: f32) {
    p.extend(Shape::dashed_line(&[Pos2::new(x.min, y), Pos2::new(x.max, y)], Stroke::new(width, color), dash, gap));
}

pub fn dashed_rect(p: &Painter, rect: Rect, color: Color32, width: f32, dash: f32, gap: f32) {
    let pts = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
    p.extend(Shape::dashed_line(&pts, Stroke::new(width, color), dash, gap));
}

// --- Panel -----------------------------------------------------------------------------

pub const HEAD_H: f32 = 28.0;

/// The shared widget chrome (`Panel.tsx`): a glass panel with an optional
/// header line. Begin it, draw the body from `body_top`, then `finish` with
/// the body's height: the background is slotted in under everything drawn.
pub struct Panel {
    pub w: f32,
    /// Inside the border.
    pub inset: f32,
    pub body_top: f32,
    shadow: ShapeIdx,
    bg: ShapeIdx,
}

impl Panel {
    pub fn begin(p: &Painter, th: &Theme, w: f32, title: Option<&str>, aside: &[Seg]) -> Self {
        let shadow = p.add(Shape::Noop);
        let bg = p.add(Shape::Noop);
        let inset = if th.f1() { 0.0 } else { 1.0 };
        let mut panel = Self { w, inset, body_top: inset, shadow, bg };
        if title.is_some() || !aside.is_empty() {
            panel.body_top = inset + head(p, th, Rect::from_min_size(Pos2::new(inset, inset), Vec2::new(w - 2.0 * inset, HEAD_H)), title.unwrap_or(""), aside);
        }
        panel
    }

    /// The inner left and right edges.
    pub fn x(&self) -> Rangef {
        Rangef::new(self.inset, self.w - self.inset)
    }

    /// Closes the panel at `bottom` (y of the body's end) and returns its size.
    pub fn finish(self, p: &Painter, th: &Theme, bottom: f32) -> Vec2 {
        let size = Vec2::new(self.w, bottom + self.inset);
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        if th.f1() {
            let shadow = Shadow { offset: [0, 2], blur: 12, spread: 0, color: Color32::from_black_alpha(89) };
            p.set(self.shadow, shadow.as_shape(rect, CornerRadius::ZERO));
            p.set(self.bg, Shape::rect_filled(rect, CornerRadius::ZERO, th.panel));
        } else {
            p.set(self.bg, Shape::rect_filled(rect, radius(th.radius), th.panel));
            // The inset highlight along the top, then the border.
            p.hline(Rangef::new(th.radius, size.x - th.radius), 1.5, Stroke::new(1.0, Color32::from_white_alpha(10)));
            p.rect_stroke(rect, radius(th.radius), Stroke::new(1.0, th.line), StrokeKind::Inside);
        }
        size
    }
}

/// The header line inside `rect` (its top `HEAD_H` px). Returns its height.
pub fn head(p: &Painter, th: &Theme, rect: Rect, title: &str, aside: &[Seg]) -> f32 {
    let h = HEAD_H;
    let r = Rect::from_min_size(rect.min, Vec2::new(rect.width(), h));
    let y = r.top() + (h - 1.0) / 2.0;
    let spacing = if th.f1() { 14.0 * 0.04 } else { 0.0 };
    if th.f1() {
        fill(p, r, 0.0, alpha(th.panel_raised, 0.7));
        p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 2.0), r.max), 0.0, th.f1_red);
    } else {
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, th.line));
    }
    let caps: Vec<Seg>;
    let aside = if th.f1() {
        caps = aside.iter().map(|s| Seg { text: s.text.to_uppercase(), ..s.clone() }).collect();
        &caps[..]
    } else {
        aside
    };
    let aside_w = if aside.is_empty() { 0.0 } else { segs_width(p, aside, spacing) };
    if !aside.is_empty() {
        segs(p, r.right() - 10.0, y, true, aside, spacing);
    }
    let font = if th.f1() { th.heavy(14.0) } else { th.bold(14.0) };
    let max = r.width() - 20.0 - if aside_w > 0.0 { aside_w + 12.0 } else { 0.0 };
    text_fit(p, Pos2::new(r.left() + 10.0, y), Align2::LEFT_CENTER, &th.caps(title), font, th.ink, max, spacing);
    h
}

/// The aside text style of panel headers (Box Box: semibold).
pub fn aside_font(th: &Theme) -> FontId {
    if th.f1() {
        th.semi(14.0)
    } else {
        th.medium(14.0)
    }
}

/// `Empty`: a muted note in an empty panel. Returns the height used.
pub fn empty(p: &Painter, th: &Theme, x: Rangef, top: f32, msg: &str) -> f32 {
    let r = text_wrap(p, Pos2::new(x.min + 12.0, top + 18.0), msg, th.medium(14.0), th.ink3, x.span() - 24.0);
    r.height() + 36.0
}

/// A small error note for a widget that failed to draw (`WidgetBoundary`).
pub fn error_note(p: &Painter, th: &Theme, name: &str, msg: &str) -> Vec2 {
    let w = 320.0;
    let panel = Panel::begin(p, th, w, None, &[]);
    let r = text_wrap(p, Pos2::new(12.0, 12.0), &format!("{name} couldn't show: {msg}"), th.medium(14.0), th.loss, w - 24.0);
    panel.finish(p, th, r.bottom() + 12.0)
}

// --- Table columns -----------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum Col {
    Px(f32),
    /// Takes what's left (`minmax(0, 1fr)`).
    Fr,
}

/// CSS grid columns across `x` with padding and a column gap: each column's x range.
pub fn grid(x: Rangef, pad_l: f32, pad_r: f32, gap: f32, cols: &[Col]) -> Vec<Rangef> {
    let fixed: f32 = cols.iter().map(|c| if let Col::Px(w) = c { *w } else { 0.0 }).sum();
    let frs = cols.iter().filter(|c| matches!(c, Col::Fr)).count().max(1) as f32;
    let gaps = gap * (cols.len().saturating_sub(1)) as f32;
    let fr = ((x.span() - pad_l - pad_r - fixed - gaps) / frs).max(0.0);
    let mut out = Vec::with_capacity(cols.len());
    let mut cx = x.min + pad_l;
    for c in cols {
        let w = match c {
            Col::Px(w) => *w,
            Col::Fr => fr,
        };
        out.push(Rangef::new(cx, cx + w));
        cx += w + gap;
    }
    out
}

/// Text in a column: left, centred or right.
pub fn cell(p: &Painter, col: Rangef, y: f32, align: Align, s: &str, font: FontId, color: Color32) -> Rect {
    let (x, a) = match align {
        Align::Min => (col.min, Align2::LEFT_CENTER),
        Align::Center => (col.center(), Align2::CENTER_CENTER),
        Align::Max => (col.max, Align2::RIGHT_CENTER),
    };
    text(p, Pos2::new(x, y), a, s, font, color)
}

// --- Timing-table cells ---------------------------------------------------------------------

/// Your row: a wash of your class colour fading to the right with a bar on
/// the left (Box Box: grey with a red bar).
pub fn player_row(p: &Painter, th: &Theme, rect: Rect, class: Color32) {
    if th.f1() {
        fill(p, rect, 0.0, alpha(th.ink, 0.16));
        fill(p, Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())), 0.0, th.f1_red);
    } else {
        hgradient(p, rect, alpha(class, 0.34), alpha(class, 0.12));
        fill(p, Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())), 0.0, class);
    }
}

/// Box Box's thin rule under every row.
pub fn row_rule(p: &Painter, th: &Theme, rect: Rect) {
    if th.f1() {
        p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, alpha(th.ink, 0.06)));
    }
}

/// The position: right-aligned bold, or Box Box's white box (red outside the
/// top ten, grey in the pits).
pub fn pos_cell(p: &Painter, th: &Theme, col: Rangef, y: f32, label: &str, pos: i32, in_pits: bool) {
    if th.f1() {
        let r = Rect::from_center_size(Pos2::new(col.center(), y), Vec2::new(col.span(), 20.0));
        let (bg, fg) = if in_pits {
            (Color32::from_rgb(0x8b, 0x8b, 0x96), Color32::from_rgb(0x15, 0x15, 0x1e))
        } else if pos > 10 {
            (Color32::from_rgb(0x9e, 0x0b, 0x17), Color32::WHITE)
        } else {
            (Color32::from_rgb(0xf4, 0xf4, 0xf6), Color32::from_rgb(0x15, 0x15, 0x1e))
        };
        fill(p, r, 2.0, bg);
        text(p, r.center(), Align2::CENTER_CENTER, label, th.bold(14.0), fg);
    } else {
        cell(p, col, y, Align::Max, label, th.bold(15.0), th.ink2);
    }
}

/// The slanted class-colour bar (`ClassStripe`), centred in its column.
pub fn class_bar(p: &Painter, th: &Theme, cx: f32, y: f32, color: Color32) {
    if th.f1() {
        fill(p, Rect::from_center_size(Pos2::new(cx, y), Vec2::new(3.0, 16.0)), 0.0, color);
        return;
    }
    let (w, h) = (4.0, 18.0);
    // skewX(-16deg): the top leans right.
    let skew = (16f32).to_radians().tan() * h / 2.0;
    let pts = vec![
        Pos2::new(cx - w / 2.0 + skew, y - h / 2.0),
        Pos2::new(cx + w / 2.0 + skew, y - h / 2.0),
        Pos2::new(cx + w / 2.0 - skew, y + h / 2.0),
        Pos2::new(cx - w / 2.0 - skew, y + h / 2.0),
    ];
    p.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// "#45": a small muted hash and the number, right-aligned at `right`.
pub fn num_cell(p: &Painter, th: &Theme, right: f32, y: f32, number: &str, size: f32) {
    let (font, color) = if th.f1() { (th.semi(size), th.ink3) } else { (th.bold(size), th.ink2) };
    let list = [seg("#", th.medium(12.0), th.ink3), seg(number, font, color)];
    let w = segs_width(p, &list, 0.0) + 1.0;
    let r = text(p, Pos2::new(right - w, y + 0.5), Align2::LEFT_CENTER, "#", th.medium(12.0), th.ink3);
    text(p, Pos2::new(r.right() + 1.0, y), Align2::LEFT_CENTER, number, list[1].font.clone(), color);
}

/// A thumbs up or down rating you gave a driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thumb {
    Up,
    Down,
}

/// Pit and out symbols (`StatusBadge`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    PitLane,
    PitStall,
    Out,
    Garage,
}

impl Badge {
    pub fn of(status: telemetry::model::CarStatus, race: bool) -> Option<Self> {
        use telemetry::model::CarStatus::*;
        match status {
            PitLane => Some(Self::PitLane),
            PitStall => Some(Self::PitStall),
            Out if race => Some(Self::Out),
            Out => Some(Self::Garage),
            Track => None,
        }
    }
}

pub fn badge_width(p: &Painter, th: &Theme, b: Badge) -> f32 {
    match b {
        Badge::PitLane | Badge::PitStall => 17.0,
        Badge::Out | Badge::Garage => width_sp(p, if b == Badge::Out { "OUT" } else { "GAR" }, th.heavy(10.0), 0.4) + 6.0,
    }
}

/// Draws a status badge with its left edge at `x`; returns its right edge.
pub fn status_badge(p: &Painter, th: &Theme, x: f32, y: f32, b: Badge) -> f32 {
    let w = badge_width(p, th, b);
    let r = Rect::from_min_size(Pos2::new(x, y - 8.5), Vec2::new(w, 17.0));
    let rad = if th.f1() { 2.0 } else { 3.0 };
    let accent = if th.f1() { th.f1_red } else { th.attention };
    match b {
        Badge::PitLane => {
            outline(p, r, rad, 1.5, accent);
            text(p, r.center(), Align2::CENTER_CENTER, "P", th.heavy(12.0), accent);
        }
        Badge::PitStall => {
            fill(p, r, rad, accent);
            let fg = if th.f1() { Color32::WHITE } else { th.panel_solid };
            text(p, r.center(), Align2::CENTER_CENTER, "P", th.heavy(12.0), fg);
        }
        Badge::Out | Badge::Garage => {
            fill(p, r, rad, th.ink3);
            text_sp(p, r.center(), Align2::CENTER_CENTER, if b == Badge::Out { "OUT" } else { "GAR" }, th.heavy(10.0), th.panel_solid, 0.4);
        }
    }
    r.right()
}

/// The name, cut short to make room for a rating thumb and a status badge
/// after it, which never shorten.
#[allow(clippy::too_many_arguments)]
pub fn name_cell(p: &Painter, th: &Theme, col: Rangef, y: f32, name: &str, color: Color32, bold: bool, mark: Option<Thumb>, badge: Option<Badge>, dim: f32) {
    let mark_w = if mark.is_some() { 4.0 + 11.0 } else { 0.0 };
    let badge_w = badge.map(|b| 6.0 + badge_width(p, th, b)).unwrap_or(0.0);
    let (font, spacing) = if th.f1() { (th.bold(15.0), 0.45) } else if bold { (th.bold(15.0), 0.0) } else { (th.medium(15.0), 0.0) };
    let r = text_fit(p, Pos2::new(col.min, y), Align2::LEFT_CENTER, &th.caps(name), font, color.gamma_multiply(dim), col.span() - mark_w - badge_w, spacing);
    let mut x = r.right();
    if let Some(m) = mark {
        let c = if m == Thumb::Up { th.gain } else { th.loss };
        thumb(p, Pos2::new(x + 4.0 + 5.5, y), 11.0, m == Thumb::Down, c.gamma_multiply(0.85));
        x += mark_w;
    }
    if let Some(b) = badge {
        status_badge(p, th, x + 6.0, y, b);
    }
}

/// The thumb icon (24×24 SVG path from cells.tsx) at `size`, centred on `c`.
pub fn thumb(p: &Painter, c: Pos2, size: f32, down: bool, color: Color32) {
    let k = size / 24.0;
    let at = |x: f32, y: f32| {
        let y = if down { 24.0 - y } else { y };
        Pos2::new(c.x + (x - 12.0) * k, c.y + (y - 12.0) * k)
    };
    // The cuff, then the hand in two convex halves.
    let cuff = [at(2.0, 21.0), at(6.0, 21.0), at(6.0, 9.0), at(2.0, 9.0)];
    let thumb_tip = [at(6.0, 9.0), at(6.6, 7.6), at(13.2, 1.0), at(14.3, 2.0), at(14.7, 3.1), at(14.7, 3.4), at(13.7, 8.0)];
    let palm = [
        at(6.0, 9.0),
        at(13.7, 8.0),
        at(20.0, 8.0),
        at(21.4, 8.6),
        at(22.0, 10.0),
        at(22.0, 12.0),
        at(21.8, 12.7),
        at(18.8, 19.8),
        at(17.0, 21.0),
        at(8.0, 21.0),
        at(6.6, 20.4),
        at(6.0, 19.0),
    ];
    for poly in [&cuff[..], &thumb_tip[..], &palm[..]] {
        p.add(Shape::convex_polygon(poly.to_vec(), color, Stroke::NONE));
    }
}

/// iRating as a light badge, with the estimated change as a coloured second
/// half when given. Placed in `col` by `align`.
pub fn ir_badge(p: &Painter, th: &Theme, col: Rangef, y: f32, align: Align, value: i32, change: Option<f32>) {
    if value == 0 {
        return;
    }
    let font = th.bold(12.0);
    let label = crate::logic::format::irating(value);
    let vw = (width(p, &label, font.clone()) + 8.0).max(if change.is_some() { 32.0 } else { 34.0 });
    let delta = change.map(|c| {
        let v = crate::logic::format::js_round(c as f64) as i64;
        let (s, bg) = match v.cmp(&0) {
            std::cmp::Ordering::Greater => (format!("+{v}"), th.gain),
            std::cmp::Ordering::Less => (format!("−{}", -v), th.loss),
            std::cmp::Ordering::Equal => ("0".to_string(), th.ink3),
        };
        let w = (width(p, &s, font.clone()) + 8.0).max(30.0);
        (s, bg, w)
    });
    let total = vw + delta.as_ref().map(|d| d.2).unwrap_or(0.0);
    let left = match align {
        Align::Min => col.min,
        Align::Center => col.center() - total / 2.0,
        Align::Max => col.max - total,
    };
    let rad = if th.f1() { 2.0 } else { 3.0 };
    let r = Rect::from_min_size(Pos2::new(left, y - 8.0), Vec2::new(total, 16.0));
    fill(p, r, rad, alpha(th.ink, 0.88));
    let vr = Rect::from_min_size(r.min, Vec2::new(vw, 16.0));
    text(p, vr.center(), Align2::CENTER_CENTER, &label, font.clone(), th.panel_solid);
    if let Some((s, bg, w)) = delta {
        let dr = Rect::from_min_size(Pos2::new(vr.right(), r.top()), Vec2::new(w, 16.0));
        p.rect_filled(dr, CornerRadius { nw: 0, sw: 0, ne: rad as u8, se: rad as u8 }, bg);
        text(p, dr.center(), Align2::CENTER_CENTER, &s, font, th.panel_solid);
    }
}

/// Width of the licence chip ("B 3.42").
pub fn lic_width(p: &Painter, th: &Theme, lic: &str) -> f32 {
    let (letter, sr) = crate::logic::format::license(lic);
    3.0 + 4.0 + width(p, letter, th.bold(12.0)) + 2.0 + width(p, sr, th.medium(11.0)) + 4.0 + 1.0
}

/// The licence chip: letter and safety rating, with the licence colour down its left edge.
pub fn lic_chip(p: &Painter, th: &Theme, x: f32, y: f32, lic: &str, color: Color32) {
    let (letter, sr) = crate::logic::format::license(lic);
    let w = lic_width(p, th, lic);
    let r = Rect::from_min_size(Pos2::new(x, y - 9.0), Vec2::new(w, 18.0));
    let rad = if th.f1() { 2.0 } else { 3.0 };
    outline(p, r, rad, 1.0, th.line_strong);
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0, 18.0)), CornerRadius { nw: rad as u8, sw: rad as u8, ne: 0, se: 0 }, color);
    let lr = text(p, Pos2::new(r.left() + 7.0, y), Align2::LEFT_CENTER, letter, th.bold(12.0), th.ink2);
    text(p, Pos2::new(lr.right() + 2.0, y + 0.5), Align2::LEFT_CENTER, sr, th.medium(11.0), th.ink3);
}

/// What kind of tyre a compound name is (`compoundKind` in Stints.tsx).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tyre {
    Wet,
    Soft,
    Medium,
    Hard,
    Other,
}

pub fn compound_kind(name: &str) -> Tyre {
    let n = name.to_lowercase();
    if ["wet", "rain", "inter"].iter().any(|k| n.contains(k)) {
        Tyre::Wet
    } else if n.contains("soft") {
        Tyre::Soft
    } else if n.contains("medium") {
        Tyre::Medium
    } else if ["hard", "dry", "primary", "prime"].iter().any(|k| n.contains(k)) {
        Tyre::Hard
    } else {
        Tyre::Other
    }
}

pub fn tyre_color(th: &Theme, kind: Tyre) -> Color32 {
    match kind {
        Tyre::Wet => th.lapped,
        Tyre::Soft => th.loss,
        Tyre::Medium => th.attention,
        Tyre::Hard | Tyre::Other => th.ink2,
    }
}

/// The round tyre badge with the compound's first letter.
pub fn tyre_badge(p: &Painter, th: &Theme, c: Pos2, size: f32, name: &str) {
    let Some(first) = name.chars().next() else { return };
    let color = tyre_color(th, compound_kind(name));
    p.circle_stroke(c, size / 2.0 - 1.0, Stroke::new(2.0, color));
    let font = th.heavy(if size < 18.0 { 10.0 } else { 11.0 });
    text(p, c, Align2::CENTER_CENTER, &first.to_uppercase().to_string(), font, color);
}

/// A lap time coloured like a timing screen; returns (font, colour).
pub fn last_lap_style(th: &Theme, flag: telemetry::model::LapFlag) -> (FontId, Color32) {
    use telemetry::model::LapFlag;
    match flag {
        LapFlag::Fastest => (th.bold(14.0), th.fastest),
        LapFlag::Personal => (th.semi(14.0), th.gain),
        LapFlag::None => (th.font(W::Medium, 14.0), if th.f1() { th.ink } else { th.ink2 }),
    }
}

pub fn best_lap_style(th: &Theme, fastest: bool) -> (FontId, Color32) {
    if fastest {
        (th.bold(14.0), th.fastest)
    } else {
        (th.medium(14.0), if th.f1() { th.ink2 } else { th.ink3 })
    }
}

// --- Charts --------------------------------------------------------------------------------

/// A small line over `rect` for `values` (oldest first), scaled to `lo..hi`,
/// with an area fill down to the bottom.
pub fn sparkline(p: &Painter, rect: Rect, values: &[f32], lo: f32, hi: f32, color: Color32, area: Option<Color32>) {
    if values.len() < 2 {
        return;
    }
    let span = (hi - lo).max(1e-6);
    let n = values.len() as f32 - 1.0;
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| Pos2::new(rect.left() + rect.width() * i as f32 / n, rect.bottom() - (v - lo) / span * rect.height()))
        .collect();
    if let Some(fill_color) = area {
        let mut mesh = Mesh::default();
        for (i, q) in pts.iter().enumerate() {
            mesh.colored_vertex(*q, fill_color);
            mesh.colored_vertex(Pos2::new(q.x, rect.bottom()), alpha(fill_color, 0.0));
            if i > 0 {
                let b = (i as u32) * 2;
                mesh.add_triangle(b - 2, b - 1, b);
                mesh.add_triangle(b - 1, b + 1, b);
            }
        }
        p.add(Shape::mesh(mesh));
    }
    p.add(Shape::line(pts, Stroke::new(1.5, color)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_kinds() {
        assert_eq!(compound_kind("Hard"), Tyre::Hard);
        assert_eq!(compound_kind("Wet"), Tyre::Wet);
        assert_eq!(compound_kind("Intermediate"), Tyre::Wet);
        assert_eq!(compound_kind("Soft"), Tyre::Soft);
        assert_eq!(compound_kind("Medium"), Tyre::Medium);
        assert_eq!(compound_kind("Dry"), Tyre::Hard);
        assert_eq!(compound_kind("Alternate"), Tyre::Other);
    }

    #[test]
    fn grid_columns() {
        let cols = grid(Rangef::new(0.0, 480.0), 6.0, 10.0, 7.0, &[Col::Px(24.0), Col::Px(8.0), Col::Fr, Col::Px(40.0)]);
        assert_eq!(cols[0], Rangef::new(6.0, 30.0));
        assert_eq!(cols[1], Rangef::new(37.0, 45.0));
        assert_eq!(cols[3].max, 470.0);
        assert!((cols[2].span() - (480.0 - 16.0 - 72.0 - 21.0)).abs() < 1e-3);
    }
}
