//! Standings: the field by class. Your class shows the rows around you, other
//! classes their leaders. Gap to the class leader, last and best laps.

use egui::{Align2, Pos2, Rect, Vec2};
use telemetry::model::{CarStatus, FieldPayload, LapFlag, StandingRow};

use super::relative::clipped;
use crate::theme::{self, *};

const ROW_H: f32 = 22.0;
const CLASS_H: f32 = 20.0;
/// Rows of your class (around you) and of each other class (leaders).
const MY_CLASS_ROWS: usize = 10;
const OTHER_CLASS_ROWS: usize = 3;
pub const WIDTH: f32 = 470.0;

/// What's shown: (class header, rows) in class order.
fn layout(field: &FieldPayload) -> Vec<(String, Vec<&StandingRow>)> {
    let me = field.standings.iter().find(|r| r.is_player);
    let mut out = Vec::new();
    for class in &field.clock.classes {
        let mut rows: Vec<&StandingRow> =
            field.standings.iter().filter(|r| r.driver.class_id == class.class_id && r.position > 0).collect();
        rows.sort_by_key(|r| r.class_position);
        let mine = me.is_some_and(|m| m.driver.class_id == class.class_id);
        let keep = if mine { MY_CLASS_ROWS } else { OTHER_CLASS_ROWS };
        let start = match (mine, me) {
            (true, Some(m)) => {
                let at = rows.iter().position(|r| r.driver.car_idx == m.driver.car_idx).unwrap_or(0);
                at.saturating_sub(keep / 2).min(rows.len().saturating_sub(keep))
            }
            _ => 0,
        };
        let shown: Vec<&StandingRow> = rows.into_iter().skip(start).take(keep).collect();
        if !shown.is_empty() {
            let sof = class.strength_of_field.map(|s| format!("   SoF {:.1}k", s as f32 / 1000.0)).unwrap_or_default();
            out.push((format!("{}   {} cars{}", class.name, class.cars, sof), shown));
        }
    }
    out
}

pub fn size(field: Option<&FieldPayload>) -> Vec2 {
    let body = match field {
        Some(f) => layout(f).iter().map(|(_, rows)| CLASS_H + rows.len() as f32 * ROW_H).sum::<f32>(),
        None => 40.0,
    };
    Vec2::new(WIDTH, TITLE_H + body.max(40.0) + 4.0)
}

pub fn draw(painter: &egui::Painter, rect: Rect, field: Option<&FieldPayload>) {
    let aside = field
        .map(|f| match (f.clock.laps_remain, f.clock.total_laps) {
            (_, Some(total)) if total > 0 => {
                let lap = f.standings.iter().map(|r| r.lap).max().unwrap_or(0);
                format!("Lap {} of {}", (lap + 1).min(total), total)
            }
            _ => f.clock.time_remain.map(clock).unwrap_or_default(),
        })
        .unwrap_or_default();
    let body = theme::panel(painter, rect, "Standings", &aside);
    let Some(field) = field.filter(|f| !f.standings.is_empty()) else {
        theme::empty(painter, body, "Standings appear when a session is running.");
        return;
    };
    let race = field.clock.session_type.to_ascii_lowercase().contains("race");
    let mut top = body.top() + 2.0;
    for (header, rows) in layout(field) {
        let class_color = hex(&rows[0].driver.class_color);
        let h = Rect::from_min_size(Pos2::new(body.left(), top), Vec2::new(body.width(), CLASS_H));
        painter.rect_filled(h.shrink2(Vec2::new(4.0, 2.0)), 2.0, class_color.gamma_multiply(0.14));
        painter.text(h.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, header, bold(12.0), class_color);
        top += CLASS_H;
        for r in rows {
            row(painter, Rect::from_min_size(Pos2::new(body.left(), top), Vec2::new(body.width(), ROW_H)), r, race);
            top += ROW_H;
        }
    }
}

fn row(painter: &egui::Painter, r: Rect, row: &StandingRow, race: bool) {
    let y = r.center().y;
    if row.is_player {
        painter.rect_filled(r.shrink2(Vec2::new(3.0, 1.0)), 3.0, PLAYER_ROW);
    }
    let dim = matches!(row.status, CarStatus::Out);
    let ink = if dim { INK.gamma_multiply(0.45) } else { INK };

    let mut x = r.left() + 8.0;
    painter.text(Pos2::new(x + 20.0, y), Align2::RIGHT_CENTER, row.class_position.to_string(), bold(14.0), ink);
    x += 26.0;
    theme::class_stripe(painter, Pos2::new(x + 3.0, y), 14.0, hex(&row.driver.class_color));
    x += 10.0;
    painter.text(Pos2::new(x, y), Align2::LEFT_CENTER, format!("#{}", row.driver.number), medium(12.0), INK3);
    x += 34.0;

    // Columns from the right: best, last, gap, iRating; the name gets the rest.
    let best_x = r.right() - 10.0;
    let last_x = best_x - 66.0;
    let gap_x = last_x - 66.0;
    let ir_x = gap_x - 50.0;
    let best_color = if row.laps.class_fastest { PURPLE } else { INK2 };
    painter.text(Pos2::new(best_x, y), Align2::RIGHT_CENTER, theme::lap_time(row.laps.best_lap), medium(13.0), best_color);
    let last_color = match row.laps.last_flag {
        LapFlag::Fastest => PURPLE,
        LapFlag::Personal => GAIN,
        LapFlag::None => INK2,
    };
    painter.text(Pos2::new(last_x, y), Align2::RIGHT_CENTER, theme::lap_time(row.laps.last_lap), medium(13.0), last_color);
    let gap = if row.class_position == 1 {
        "Leader".to_string()
    } else if race && row.laps_down > 0 {
        format!("+{}L", row.laps_down)
    } else {
        row.gap.map(|g| format!("{g:.1}")).unwrap_or_else(|| "–".into())
    };
    painter.text(Pos2::new(gap_x, y), Align2::RIGHT_CENTER, gap, semibold(13.0), ink);
    theme::irating_badge(painter, Pos2::new(ir_x, y), row.driver.irating);
    let pit = matches!(row.status, CarStatus::PitLane | CarStatus::PitStall);
    let name_right = ir_x - 46.0;
    if pit {
        painter.text(Pos2::new(name_right, y), Align2::RIGHT_CENTER, "PIT", bold(11.0), ACCENT);
    }
    clipped(painter, Pos2::new(x, y), &row.driver.name, semibold(14.0), ink, name_right - x - if pit { 28.0 } else { 4.0 });
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}
