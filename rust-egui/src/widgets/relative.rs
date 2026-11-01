//! Relative: the cars around you on track, ahead on top, with the gap in
//! seconds. A fixed number of rows, so the panel never changes height.

use egui::{Align2, Pos2, Rect, Vec2};
use telemetry::model::{CarStatus, FieldPayload, RelativeRow};

use crate::theme::{self, *};

const AHEAD: usize = 4;
const BEHIND: usize = 4;
const ROW_H: f32 = 24.0;
pub const SIZE: Vec2 = Vec2::new(380.0, TITLE_H + ROW_H * (AHEAD + BEHIND + 1) as f32 + 4.0);

pub fn draw(painter: &egui::Painter, rect: Rect, field: Option<&FieldPayload>) {
    let title = field.map(|f| f.clock.session_type.as_str()).filter(|s| !s.is_empty()).unwrap_or("Relative");
    let aside = field
        .map(|f| match f.clock.incident_limit {
            Some(l) => format!("{}x / {}x", f.clock.incidents, l),
            None => format!("{}x", f.clock.incidents),
        })
        .unwrap_or_default();
    let body = theme::panel(painter, rect, title, &aside);
    let rows: &[RelativeRow] = field.map(|f| f.relative.as_slice()).unwrap_or(&[]);
    let Some(me) = rows.iter().position(|r| r.is_player) else {
        theme::empty(painter, body, "Relative appears once you're in the car.");
        return;
    };

    for slot in 0..(AHEAD + BEHIND + 1) {
        let i = me as isize + slot as isize - AHEAD as isize;
        let top = body.top() + 2.0 + slot as f32 * ROW_H;
        let row_rect = Rect::from_min_size(Pos2::new(body.left(), top), Vec2::new(body.width(), ROW_H));
        if i < 0 || i as usize >= rows.len() {
            continue;
        }
        row(painter, row_rect, &rows[i as usize]);
    }
}

fn row(painter: &egui::Painter, r: Rect, row: &RelativeRow) {
    let y = r.center().y;
    if row.is_player {
        painter.rect_filled(r.shrink2(Vec2::new(3.0, 1.0)), 3.0, PLAYER_ROW);
    }
    // Lapping you (a lap up) in red, a lap down in blue; your class in plain ink.
    let ink = match row.lap_status {
        s if s > 0 => LAP_UP,
        s if s < 0 => LAP_DOWN,
        _ => INK,
    };
    let dim = matches!(row.status, CarStatus::PitLane | CarStatus::PitStall | CarStatus::Out);
    let ink = if dim { ink.gamma_multiply(0.5) } else { ink };

    let mut x = r.left() + 8.0;
    let pos = if row.class_position > 0 { row.class_position.to_string() } else { "–".into() };
    painter.text(Pos2::new(x + 20.0, y), Align2::RIGHT_CENTER, pos, bold(15.0), ink);
    x += 26.0;
    theme::class_stripe(painter, Pos2::new(x + 3.0, y), 15.0, hex(&row.driver.class_color));
    x += 10.0;
    painter.text(Pos2::new(x, y), Align2::LEFT_CENTER, format!("#{}", row.driver.number), medium(13.0), INK3);
    x += 36.0;

    // Right-hand columns first, so the name gets what's left.
    let right = r.right() - 10.0;
    let gap = if row.is_player { "0.0".to_string() } else { format!("{:.1}", row.gap.abs()) };
    painter.text(Pos2::new(right, y), Align2::RIGHT_CENTER, gap, semibold(15.0), ink);
    theme::irating_badge(painter, Pos2::new(right - 52.0, y), row.driver.irating);
    let status = match row.status {
        CarStatus::PitLane | CarStatus::PitStall => "PIT",
        CarStatus::Out => "OUT",
        CarStatus::Track => "",
    };
    let name_right = right - 52.0 - 46.0;
    if !status.is_empty() {
        painter.text(Pos2::new(name_right, y), Align2::RIGHT_CENTER, status, bold(11.0), ACCENT);
    }
    let width = name_right - x - if status.is_empty() { 4.0 } else { 30.0 };
    clipped(painter, Pos2::new(x, y), &row.driver.name, semibold(15.0), ink, width);
}

/// Draws text left-aligned at `pos`, cut short with "…" if it's wider than `width`.
pub fn clipped(painter: &egui::Painter, pos: Pos2, text: &str, font: egui::FontId, color: egui::Color32, width: f32) {
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), egui::TextFormat::simple(font, color));
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(0.0));
    let galley = painter.layout_job(job);
    painter.galley(Pos2::new(pos.x, pos.y - galley.size().y / 2.0), galley, color);
}
