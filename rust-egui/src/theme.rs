//! Colours, fonts and the small pieces every widget draws (panel, title bar,
//! class stripe, iRating badge), matching the web app's dark theme.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};

pub const PANEL: Color32 = Color32::from_rgba_premultiplied(14, 17, 23, 222);
pub const PANEL_HEAD: Color32 = Color32::from_rgba_premultiplied(22, 26, 34, 235);
pub const LINE: Color32 = Color32::from_rgba_premultiplied(48, 55, 68, 255);
pub const INK: Color32 = Color32::from_rgb(236, 239, 244);
pub const INK2: Color32 = Color32::from_rgb(170, 178, 192);
pub const INK3: Color32 = Color32::from_rgb(112, 121, 138);
pub const GAIN: Color32 = Color32::from_rgb(70, 214, 120);
pub const ACCENT: Color32 = Color32::from_rgb(255, 176, 46);
pub const PLAYER_ROW: Color32 = Color32::from_rgba_premultiplied(70, 58, 22, 200);
pub const THROTTLE: Color32 = Color32::from_rgb(70, 214, 120);
pub const BRAKE: Color32 = Color32::from_rgb(255, 72, 72);
pub const CLUTCH: Color32 = Color32::from_rgb(80, 150, 255);
pub const PURPLE: Color32 = Color32::from_rgb(196, 120, 255);
/// Cars a lap up on you (red) or down (blue), as in the web Relative.
pub const LAP_UP: Color32 = Color32::from_rgb(255, 130, 120);
pub const LAP_DOWN: Color32 = Color32::from_rgb(120, 170, 255);

pub const RADIUS: f32 = 6.0;

const MEDIUM: &str = "barlow-medium";
const SEMIBOLD: &str = "barlow-semibold";
const BOLD: &str = "barlow-bold";

/// The web app's font (Barlow Semi Condensed), bundled as TTF (OFL licence).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(FontData::from_static(bytes)));
        // Each weight is its own family; egui's default fonts stay as fallbacks (symbols).
        let mut chain = vec![name.to_owned()];
        chain.extend(fonts.families[&FontFamily::Proportional].iter().cloned());
        fonts.families.insert(FontFamily::Name(name.into()), chain);
    };
    add(&mut fonts, MEDIUM, include_bytes!("../assets/fonts/BarlowSemiCondensed-Medium.ttf"));
    add(&mut fonts, SEMIBOLD, include_bytes!("../assets/fonts/BarlowSemiCondensed-SemiBold.ttf"));
    add(&mut fonts, BOLD, include_bytes!("../assets/fonts/BarlowSemiCondensed-Bold.ttf"));
    // Plain egui text (the edit bar) uses the medium weight too.
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, MEDIUM.to_owned());
    ctx.set_fonts(fonts);
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// `#ffda59` (or `ffda59`) -> colour; grey when it isn't one.
pub fn hex(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    if s.len() == 6 {
        if let Ok(v) = u32::from_str_radix(s, 16) {
            return Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
        }
    }
    INK3
}

/// Panel background and title bar. Returns the area under the title.
pub fn panel(painter: &egui::Painter, rect: Rect, title: &str, aside: &str) -> Rect {
    painter.rect_filled(rect, RADIUS, PANEL);
    painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, LINE), StrokeKind::Inside);
    let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), TITLE_H));
    painter.rect_filled(head, egui::CornerRadius { nw: RADIUS as u8, ne: RADIUS as u8, sw: 0, se: 0 }, PANEL_HEAD);
    painter.hline(rect.x_range(), head.bottom(), Stroke::new(1.0, LINE));
    painter.text(head.left_center() + Vec2::new(10.0, 0.0), egui::Align2::LEFT_CENTER, title, bold(14.0), INK);
    if !aside.is_empty() {
        painter.text(head.right_center() - Vec2::new(10.0, 0.0), egui::Align2::RIGHT_CENTER, aside, medium(13.0), INK2);
    }
    Rect::from_min_max(Pos2::new(rect.left(), head.bottom()), rect.max)
}

pub const TITLE_H: f32 = 26.0;

/// A short centred note in an empty panel.
pub fn empty(painter: &egui::Painter, body: Rect, text: &str) {
    painter.text(body.left_center() + Vec2::new(10.0, 0.0), egui::Align2::LEFT_CENTER, text, medium(13.0), INK3);
}

/// The slanted class-colour stripe next to a position.
pub fn class_stripe(painter: &egui::Painter, center: Pos2, h: f32, color: Color32) {
    let w = 3.0;
    let skew = h * 0.22;
    let pts = vec![
        Pos2::new(center.x - w / 2.0 + skew / 2.0, center.y - h / 2.0),
        Pos2::new(center.x + w / 2.0 + skew / 2.0, center.y - h / 2.0),
        Pos2::new(center.x + w / 2.0 - skew / 2.0, center.y + h / 2.0),
        Pos2::new(center.x - w / 2.0 - skew / 2.0, center.y + h / 2.0),
    ];
    painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// iRating as a light pill ("2.4k"), right-aligned at `right`.
pub fn irating_badge(painter: &egui::Painter, right: Pos2, ir: i32) {
    if ir <= 0 {
        return;
    }
    let text = if ir >= 1000 { format!("{:.1}k", ir as f32 / 1000.0) } else { ir.to_string() };
    let galley = painter.layout_no_wrap(text, semibold(12.0), Color32::from_rgb(20, 22, 26));
    let size = galley.size() + Vec2::new(8.0, 2.0);
    let rect = Rect::from_min_size(Pos2::new(right.x - size.x, right.y - size.y / 2.0), size);
    painter.rect_filled(rect, 3.0, Color32::from_rgb(214, 218, 226));
    painter.galley(rect.min + Vec2::new(4.0, 1.0), galley, INK);
}

/// "1:32.456"; "–" when unknown.
pub fn lap_time(t: Option<f32>) -> String {
    match t {
        Some(t) if t > 0.0 => {
            let m = (t / 60.0).floor() as i32;
            let s = t - m as f32 * 60.0;
            if m > 0 { format!("{m}:{s:06.3}") } else { format!("{s:.3}") }
        }
        _ => "–".into(),
    }
}
