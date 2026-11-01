//! Lap log (`src/widgets/LapLog.tsx`): every lap this session, newest first,
//! with the session and all-time bests on top. The newest `rows` laps are
//! shown; older ones fade out below (the web list scrolls in edit mode).

use egui::{Align, Align2, Painter, Pos2, Rect, Stroke, Vec2};
use telemetry::LapRecord;

use super::{Ctx, Widget};
use crate::config::Units;
use crate::kit::{self, seg, Col};
use crate::logic::format::{fixed, fuel, fuel_unit, js_round, lap_time, signed};
use crate::theme::{alpha, Theme};

pub struct LapLog;

/// "today", "yesterday", "12 days ago"
pub fn age(unix: u64, now: u64) -> String {
    let days = (now as i64 - unix as i64).div_euclid(86_400);
    match days {
        d if d <= 0 => "today".into(),
        1 => "yesterday".into(),
        d => format!("{d} days ago"),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn temp(c: f32, u: Units) -> String {
    format!("{}°", js_round(crate::logic::format::temp(c, u) as f64))
}

impl Widget for LapLog {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let units = cx.units;
        let log = cx.live.field.as_ref().map(|f| &f.lap_log);
        let rows = o.int("rows", 1);
        let personal = o.s("compareTo") == "personal";
        let (show_delta, show_temp, show_fuel, show_pb) = (o.b("showDelta"), o.b("showTemp"), o.b("showFuel"), o.b("showPersonalBest"));
        let session_best = log.and_then(|l| l.session_best);
        let pb = log.and_then(|l| l.personal_best.as_ref());
        let best = if personal { pb.map(|p| p.time).or(session_best) } else { session_best };
        let count = log.map(|l| l.laps.len()).unwrap_or(0);
        let aside = if count > 0 { vec![seg(format!("{count} laps"), kit::aside_font(th), th.ink2)] } else { vec![] };
        let panel = kit::Panel::begin(p, th, 300.0, Some("Lap log"), &aside);
        let x = panel.x();
        let mut y = panel.body_top;

        // Session and personal bests.
        let bests_h = 6.0 + 15.0 + 1.0 + 22.0 + 7.0;
        let half = if show_pb { x.span() / 2.0 } else { x.span() };
        let r = Rect::from_min_size(Pos2::new(x.min, y), Vec2::new(half, bests_h));
        p.rect_filled(Rect::from_min_size(r.min, Vec2::new(3.0, r.height())), 0.0, th.gain);
        best_box(p, th, r, "Session best", &lap_time(session_best), th.gain);
        if show_pb {
            let r2 = Rect::from_min_size(Pos2::new(x.min + half, y), Vec2::new(half, bests_h));
            p.vline(r2.left() + 0.5, r2.y_range(), Stroke::new(1.0, th.line));
            let label = match pb {
                Some(pb) => format!("Personal best · {}", age(pb.at, unix_now())),
                None => "Personal best".into(),
            };
            best_box(p, th, r2, &label, &lap_time(pb.map(|p| p.time)), th.ink);
        }
        y += bests_h;
        p.hline(x, y - 0.5, Stroke::new(1.0, th.line));

        let mut cols = vec![Col::Px(34.0), Col::Fr];
        let mut push = |on: bool, c: Col| {
            if on {
                cols.push(c);
                Some(cols.len() - 1)
            } else {
                None
            }
        };
        let c_delta = push(show_delta, Col::Px(64.0));
        let c_temp = push(show_temp, Col::Px(40.0));
        let c_fuel = push(show_fuel, Col::Px(62.0));
        let g = kit::grid(x, 10.0, 10.0, 8.0, &cols);

        // Column labels.
        let hy = y + 11.0;
        kit::cell(p, g[0], hy, Align::Min, "Lap", th.medium(12.0), th.ink3);
        kit::cell(p, g[1], hy, Align::Min, "Time", th.medium(12.0), th.ink3);
        if let Some(c) = c_delta {
            kit::cell(p, g[c], hy, Align::Max, if personal { "To PB" } else { "To best" }, th.medium(12.0), th.ink3);
        }
        if let Some(c) = c_temp {
            kit::cell(p, g[c], hy, Align::Max, "Track", th.medium(12.0), th.ink3);
        }
        if let Some(c) = c_fuel {
            kit::cell(p, g[c], hy, Align::Max, "Fuel", th.medium(12.0), th.ink3);
        }
        y += 22.0;
        p.hline(x, y - 0.5, Stroke::new(1.0, th.line));

        let laps: Vec<&LapRecord> = log.map(|l| l.laps.iter().rev().collect()).unwrap_or_default();
        if laps.is_empty() {
            let h = kit::empty(p, th, x, y, "Laps appear here as you complete them.");
            return panel.finish(p, th, y + h);
        }
        let rh = th.row_h;
        let more = laps.len() > rows;
        let list_top = y;
        for (k, l) in laps.iter().take(rows).enumerate() {
            let rect = Rect::from_min_size(Pos2::new(x.min, y), Vec2::new(x.span(), rh));
            if k % 2 == 1 {
                kit::fill(p, rect, 0.0, alpha(th.ink, 0.03));
            }
            row(p, th, &g, rect, l, best, session_best, units, (c_delta, c_temp, c_fuel));
            y += rh;
        }
        // A fade at the bottom says there's more below.
        if more {
            let fade = Rect::from_min_max(Pos2::new(x.min, y - 14.0), Pos2::new(x.max, y));
            kit::vgradient(p, fade, alpha(th.panel_solid, 0.0), alpha(th.panel_solid, 0.8));
        }
        let _ = list_top;
        panel.finish(p, th, y)
    }
}

fn best_box(p: &Painter, th: &Theme, r: Rect, label: &str, value: &str, color: egui::Color32) {
    kit::text_fit(p, Pos2::new(r.left() + 10.0, r.top() + 6.0), Align2::LEFT_TOP, label, th.medium(12.0), th.ink3, r.width() - 20.0, 0.0);
    kit::text(p, Pos2::new(r.left() + 10.0, r.top() + 6.0 + 15.0 + 1.0), Align2::LEFT_TOP, value, th.bold(20.0), color);
}

#[allow(clippy::too_many_arguments)]
fn row(p: &Painter, th: &Theme, g: &[egui::Rangef], rect: Rect, l: &LapRecord, best: Option<f32>, session_best: Option<f32>, units: Units, cols: (Option<usize>, Option<usize>, Option<usize>)) {
    let cy = rect.center().y;
    let is_best = l.clean && l.time.is_some() && l.time == session_best;
    kit::cell(p, g[0], cy, Align::Min, &l.lap.to_string(), th.medium(15.0), th.ink3);
    let time_c = if is_best {
        th.gain
    } else if !l.clean {
        th.ink3
    } else {
        th.ink
    };
    let r = kit::cell(p, g[1], cy, Align::Min, &lap_time(l.time), th.semi(15.0), time_c);
    let mut tx = r.right() + 6.0;
    if !l.clean && !l.pit {
        p.circle_filled(Pos2::new(tx + 3.5, cy), 3.5, th.loss);
        tx += 7.0 + 6.0;
    }
    let mut tag = |text: &str, bg: egui::Color32, fg: egui::Color32| {
        let w = kit::width(p, text, th.bold(11.0)) + 8.0;
        let tr = Rect::from_min_size(Pos2::new(tx, cy - 7.5), Vec2::new(w, 15.0));
        kit::fill(p, tr, 3.0, bg);
        kit::text(p, tr.center(), Align2::CENTER_CENTER, text, th.bold(11.0), fg);
        tx += w + 6.0;
    };
    if l.pit {
        tag("Pit", alpha(th.ink, 0.10), th.ink2);
    }
    if l.personal_best {
        tag("PB", th.gain, th.panel_solid);
    }
    let (c_delta, c_temp, c_fuel) = cols;
    if let Some(c) = c_delta {
        let delta = match (l.time, best) {
            (Some(t), Some(b)) => Some(t - b),
            _ => None,
        };
        let color = match delta {
            Some(d) if l.clean => {
                if d <= 0.0 {
                    th.gain
                } else {
                    th.loss
                }
            }
            _ => th.ink,
        };
        kit::cell(p, g[c], cy, Align::Max, &delta.map(|d| signed(d as f64, 3)).unwrap_or("–".into()), th.medium(15.0), color);
    }
    if let Some(c) = c_temp {
        kit::cell(p, g[c], cy, Align::Max, &temp(l.track_temp, units), th.medium(15.0), th.ink2);
    }
    if let Some(c) = c_fuel {
        let v = l.fuel_used.map(|f| format!("{} {}", fixed(fuel(f, units) as f64, 2), fuel_unit(units))).unwrap_or("–".into());
        kit::cell(p, g[c], cy, Align::Max, &v, th.medium(15.0), th.ink2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages() {
        let now = 1_700_000_000;
        assert_eq!(age(now - 3600, now), "today");
        assert_eq!(age(now - 86_400 - 5, now), "yesterday");
        assert_eq!(age(now - 12 * 86_400, now), "12 days ago");
        assert_eq!(temp(31.4, Units::Metric), "31°");
    }
}
