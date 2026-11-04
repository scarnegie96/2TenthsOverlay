//! Relative (`src/widgets/Relative.tsx`): the cars around you on track,
//! ahead on top, with a fixed number of slots so the panel never changes height.

use egui::{Align, Painter, Pos2, Rect, Vec2};
use telemetry::model::{RelativeRow, SessionClock};

use super::{Ctx, Widget};
use crate::kit::{self, seg, Badge, Col, Seg};
use crate::logic::format::{clock, gap, irating};
use crate::logic::incidents;
use crate::theme::hex_or;

pub struct Relative;

/// Within this many seconds of the car ahead counts as a close fight.
const CLOSE_S: f32 = 1.0;

/// "Lap 10 of 30", or "12:34 left".
pub fn session_aside(c: Option<&SessionClock>) -> Option<String> {
    let c = c?;
    if let (Some(remain), Some(total)) = (c.laps_remain, c.total_laps.filter(|t| *t > 0)) {
        let lap = (total - remain + 1).min(total);
        return Some(format!("Lap {} of {total}", lap.max(1)));
    }
    c.time_remain.map(|t| format!("{} left", clock(t)))
}

impl Widget for Relative {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let field = cx.live.field.as_deref();
        let race = cx.live.race();
        let ahead = o.int("ahead", 0);
        let behind = o.int("behind", 0);
        let (show_ir, show_lic, show_last, show_pit, show_interval, show_marks) =
            (o.b("showIRating"), o.b("showLicense"), o.b("showLastLap"), o.b("showPit"), o.b("showInterval"), o.b("showRatings"));
        let compounds = cx.live.compounds();
        let show_compound = o.s("showCompound");
        let show_tyre = show_compound == "always" || (show_compound == "auto" && compounds.len() > 1);

        let rows: Vec<&RelativeRow> =
            field.map(|f| f.relative.iter().filter(|r| show_pit || !r.on_pit_road || r.is_player).collect()).unwrap_or_default();
        let me = rows.iter().position(|r| r.is_player);

        // Header: session time or laps, SoF, then incidents against the next limit.
        let mut aside: Vec<Seg> = Vec::new();
        let font = kit::aside_font(th);
        let sof = if o.b("showSoF") { field.and_then(|f| f.clock.strength_of_field) } else { None };
        let text = [session_aside(field.map(|f| &f.clock)), sof.filter(|s| *s > 0).map(|s| format!("SoF {}", irating(s)))]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("   ");
        if !text.is_empty() {
            aside.push(seg(text, font.clone(), th.ink2));
        }
        if let (true, Some(f)) = (o.b("showIncidents"), field) {
            let count = f.clock.incidents;
            let t = incidents::target(count, f.clock.incident_limit, o.n("incidentStep").round() as i32);
            let (color, f2) = match t.next {
                Some(next) if next - count <= 4 => (th.loss, th.bold(14.0)),
                Some(next) if t.passed > 0 || count as f32 >= next as f32 * 0.75 => (th.attention, font.clone()),
                _ => (th.ink2, font.clone()),
            };
            let s = match t.next {
                Some(n) => format!("{count}x / {n}x"),
                None => format!("{count}x"),
            };
            if !aside.is_empty() {
                aside.push(seg("    ", font.clone(), th.ink2));
            }
            aside.push(seg(s, f2, color));
        }

        let w = if show_interval { 520.0 } else { 480.0 };
        let title = field.map(|f| f.clock.session_type.as_str()).filter(|s| !s.is_empty()).unwrap_or("Relative");
        let panel = kit::Panel::begin(p, th, w, Some(title), &aside);
        let Some(me) = me else {
            let h = kit::empty(p, th, panel.x(), panel.body_top, "Relative appears once you're in the car.");
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        };

        // The licence column fits its widest chip ("auto").
        let lic_w = rows.iter().map(|r| kit::lic_width(p, th, &r.driver.license)).fold(0.0, f32::max);
        let mut cols = vec![Col::Px(24.0), Col::Px(8.0), Col::Px(34.0), Col::Fr];
        let mut at = |on: bool, c: Col| {
            if on {
                cols.push(c);
                Some(cols.len() - 1)
            } else {
                None
            }
        };
        let c_tyre = at(show_tyre, Col::Px(22.0));
        let c_lic = at(show_lic, Col::Px(lic_w));
        let c_ir = at(show_ir, Col::Px(40.0));
        let c_last = at(show_last, Col::Px(62.0));
        let c_gap = at(true, Col::Px(40.0)).unwrap();
        let c_int = at(show_interval, Col::Px(40.0));
        let x = kit::grid(panel.x(), 6.0, 10.0, 7.0, &cols);

        let pad = if th.broadcast() { 0.0 } else { 3.0 };
        let mut y = panel.body_top + pad;
        let rh = th.row_h;
        for k in -(ahead as isize)..=(behind as isize) {
            let i = me as isize + k;
            let rect = Rect::from_min_size(Pos2::new(panel.x().min, y), Vec2::new(panel.x().span(), rh));
            y += rh;
            if i < 0 || i as usize >= rows.len() {
                continue;
            }
            let i = i as usize;
            let r = rows[i];
            // Gap to the car directly in front on track (the row above).
            let interval = if i > 0 { Some(rows[i - 1].gap - r.gap) } else { None };
            let cy = rect.center().y;
            let class = hex_or(&r.driver.class_color, th.ink);
            if r.is_player {
                kit::player_row(p, th, rect, class);
            }
            kit::row_rule(p, th, rect);
            let label = if r.class_position > 0 { r.class_position.to_string() } else { "–".into() };
            kit::pos_cell(p, th, x[0], cy, &label, r.class_position, r.on_pit_road);
            kit::class_bar(p, th, x[1].center(), cy, class);
            kit::num_cell(p, th, x[2].max, cy, &r.driver.number, 15.0);
            let ink = if r.is_player {
                th.ink
            } else if r.lap_status > 0 {
                th.lapping
            } else if r.lap_status < 0 {
                th.lapped
            } else {
                th.ink
            };
            let mark = if show_marks && !r.is_player { cx.ratings.get(r.driver.user_id) } else { None };
            kit::name_cell(p, th, x[3], cy, &r.driver.name, ink, r.is_player, mark, Badge::of(r.status, race), 1.0);
            if let Some(c) = c_tyre {
                if let Some(name) = usize::try_from(r.compound).ok().and_then(|k| compounds.get(k)).filter(|n| !n.is_empty()) {
                    kit::tyre_badge(p, th, Pos2::new(x[c].center(), cy), 16.0, name);
                }
            }
            if let Some(c) = c_lic {
                kit::lic_chip(p, th, x[c].min, cy, &r.driver.license, hex_or(&r.driver.license_color, th.ink3));
            }
            if let Some(c) = c_ir {
                kit::ir_badge(p, th, x[c], cy, Align::Max, r.driver.irating, None);
            }
            if let Some(c) = c_last {
                let (font, color) = kit::last_lap_style(th, r.laps.last_flag);
                kit::cell(p, x[c], cy, Align::Max, &crate::logic::format::lap_time(r.laps.last_lap), font, color);
            }
            if !r.is_player {
                kit::cell(p, x[c_gap], cy, Align::Max, &gap(Some(r.gap), 1), th.semi(15.0), ink);
            }
            if let (Some(c), Some(v)) = (c_int, interval) {
                let color = if v < CLOSE_S { th.attention } else { th.ink3 };
                kit::cell(p, x[c], cy, Align::Max, &gap(Some(v), 1), th.medium(14.0), color);
            }
        }
        panel.finish(p, th, y + pad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aside_text() {
        let mut c = SessionClock { laps_remain: Some(21), total_laps: Some(30), ..Default::default() };
        assert_eq!(session_aside(Some(&c)).as_deref(), Some("Lap 10 of 30"));
        c.laps_remain = Some(40);
        assert_eq!(session_aside(Some(&c)).as_deref(), Some("Lap 1 of 30"));
        let c = SessionClock { time_remain: Some(754.0), ..Default::default() };
        assert_eq!(session_aside(Some(&c)).as_deref(), Some("12:34 left"));
        assert_eq!(session_aside(None), None);
    }
}
