//! Pit stop (`src/widgets/PitStop.tsx`): what iRacing has ticked for your
//! next stop, read back from its telemetry, with the hotkey for each row.
//!
//! Sending the pit commands (hotkeys, wheel buttons, auto fuel) needs the
//! controls service, which the prototype doesn't have yet; the display is
//! complete, so the key hints, the fuel note and the footer all read as in
//! the web app, but nothing here presses a key or lights one up.

use egui::{Align2, Painter, Pos2, Rect, Vec2};
use telemetry::model::PitPayload;

use super::{Ctx, Widget};
use crate::config::Opts;
use crate::kit::{self, seg};
use crate::logic::format::{fixed, fuel as conv_fuel, fuel_unit, pressure};
use crate::logic::pit::{fuel_plan, full_label, short_label, FuelMode};
use crate::theme::{alpha, Theme};

pub struct PitStop;

const CORNERS: [&str; 4] = ["LF", "RF", "LR", "RR"];
const WIDTH: f32 = 300.0;
/// Barlow's `line-height: normal`.
const LINE: f32 = 1.2;

/// irsdk_PitSvStatus values from 100 up: why iRacing won't service the car.
fn problem(status: i32) -> Option<&'static str> {
    Some(match status {
        100 => "Too far left",
        101 => "Too far right",
        102 => "Too far forward",
        103 => "Too far back",
        104 => "Bad angle",
        105 => "Can't fix that",
        _ => return None,
    })
}

/// A row's hotkey hints, a line per group.
type Hints = Vec<Vec<String>>;

struct Row<'a> {
    label: &'a str,
    on: bool,
    hints: Hints,
}

const KEY_H: f32 = 20.0;
const KEY_GAP: f32 = 3.0;

fn key_width(p: &Painter, th: &Theme, s: &str) -> f32 {
    (kit::width(p, s, th.semi(12.0)) + 10.0).max(18.0)
}

fn keys_size(p: &Painter, th: &Theme, hints: &Hints) -> Vec2 {
    let w = hints
        .iter()
        .map(|line| line.iter().map(|k| key_width(p, th, k)).sum::<f32>() + KEY_GAP * line.len().saturating_sub(1) as f32)
        .fold(0.0, f32::max);
    let h = if hints.is_empty() { 0.0 } else { hints.len() as f32 * KEY_H + KEY_GAP * (hints.len() - 1) as f32 };
    Vec2::new(w, h)
}

/// The hotkey boxes, right-aligned at `right`, centred on `cy`.
fn draw_keys(p: &Painter, th: &Theme, hints: &Hints, right: f32, cy: f32) {
    let size = keys_size(p, th, hints);
    let mut y = cy - size.y / 2.0;
    for line in hints {
        let total: f32 = line.iter().map(|k| key_width(p, th, k)).sum::<f32>() + KEY_GAP * line.len().saturating_sub(1) as f32;
        let mut x = right - total;
        for k in line {
            let w = key_width(p, th, k);
            let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, KEY_H));
            kit::outline(p, r, 3.0, 1.0, th.line_strong);
            kit::text(p, r.center(), Align2::CENTER_CENTER, k, th.semi(12.0), th.ink3);
            x += w + KEY_GAP;
        }
        y += KEY_H + KEY_GAP;
    }
}

/// The tick box: hollow until iRacing reports it ticked.
fn check(p: &Painter, th: &Theme, x: f32, cy: f32, on: bool) {
    let r = Rect::from_center_size(Pos2::new(x + 6.0, cy), Vec2::splat(12.0));
    if on {
        kit::fill(p, r, 3.0, th.gain);
    } else {
        kit::outline(p, r, 3.0, 1.5, th.ink3);
    }
}

impl Widget for PitStop {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let field = cx.live.field.as_deref();
        let pit: Option<&PitPayload> = field.map(|f| &f.pit).filter(|p| p.available);
        let units = cx.units;

        let Some(pit) = pit else {
            let panel = kit::Panel::begin(p, th, WIDTH, Some("Pit stop"), &[]);
            let h = kit::empty(p, th, panel.x(), panel.body_top, "Pit service shows here once you're in the car.");
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        };
        let fuel = field.map(|f| &f.fuel);
        let sets = field.and_then(|f| f.tires.sets_available);
        let margin = Opts::new(cx.cfg, "fuel").n("marginLaps") as f32;
        let mode = FuelMode::parse(o.s("fuelMode"));
        let auto_on = o.b("keepFuelSet");
        // Auto fuel pauses when you change the fuel by hand with the up and
        // down keys; without the controls service nothing ever does.
        let auto_paused = false;
        let show_keys = o.b("showKeys");
        let controls = &cx.general.controls;
        let in_car = cx.live.tick.as_ref().is_some_and(|t| t.is_on_track);

        let hints = |groups: &[&[&str]]| -> Hints {
            if !show_keys {
                return Vec::new();
            }
            groups
                .iter()
                .map(|ids| ids.iter().filter_map(|id| short_label(controls, id)).collect::<Vec<_>>())
                .filter(|line| !line.is_empty())
                .collect()
        };
        let clear = full_label(controls, "pit.clear");

        let u = fuel_unit(units);
        let v = |l: f32| fixed(conv_fuel(l, units) as f64, if units == crate::config::Units::Imperial { 1 } else { 0 });
        let plan = fuel_plan(mode, fuel, margin);

        let prob = problem(pit.status);
        let aside_text = prob.unwrap_or(match pit.status {
            1 => "Servicing",
            2 => "Service done",
            _ => "Next stop",
        });
        let aside = match prob {
            Some(t) => seg(t, th.bold(14.0), th.attention),
            None => seg(aside_text, kit::aside_font(th), th.ink2),
        };
        let any_tire = pit.tires.iter().any(|t| *t);

        // The fuel row's note.
        let finish = fuel_plan(FuelMode::Finish, fuel, margin);
        let amount_off = |want: i32| !pit.fuel || (pit.fuel_amount - want as f32).abs() >= 1.0;
        let (fuel_note, fuel_warn) = if auto_on {
            // Auto fuel to finish keeps iRacing's amount right, so the note says what it's doing.
            if auto_paused {
                match finish {
                    None => ("Auto paused: set by hand".to_string(), false),
                    Some(f) => (format!("Auto paused · needs {} {u}", v(f as f32)), f > 0 && amount_off(f)),
                }
            } else {
                match finish {
                    None => ("Auto: plan after a green lap".to_string(), false),
                    Some(0) => ("Auto: enough to finish".to_string(), false),
                    Some(f) => (format!("Auto: {} {u} to finish", v(f as f32)), false),
                }
            }
        } else if mode == FuelMode::Manual {
            ("Hotkey keeps iRacing's amount".to_string(), false)
        } else {
            match plan {
                None => ("Plan after a green lap".to_string(), false),
                Some(0) => ((if mode == FuelMode::Full { "Tank is full" } else { "Enough to finish" }).to_string(), pit.fuel),
                Some(n) => (
                    if mode == FuelMode::Full { format!("{} {u} fills the tank", v(n as f32)) } else { format!("Needs {} {u} to finish", v(n as f32)) },
                    amount_off(n),
                ),
            }
        };

        let panel = kit::Panel::begin(p, th, WIDTH, Some("Pit stop"), &[aside]);
        let x = panel.x();
        let mut y = panel.body_top;
        let note_font = th.medium(13.0);
        let note_h = 13.0 * LINE;
        let note = |color| if color { th.attention } else { th.ink3 };

        let rows = [
            Row { label: "Fuel", on: pit.fuel, hints: hints(&[&["pit.fuel", "pit.autoFuel"], &["pit.fuelUp", "pit.fuelDown"]]) },
            Row { label: "Tires", on: any_tire, hints: hints(&[&["pit.tires"]]) },
            Row { label: "Fast repair", on: pit.fast_repair, hints: hints(&[&["pit.fastRepair"]]) },
            Row { label: "Tear-off", on: pit.tearoff, hints: hints(&[&["pit.tearoff"]]) },
        ];
        let tire_note: String = [Some(pit.compound.clone()).filter(|c| !c.is_empty()), sets.map(|s| format!("{s} sets left"))]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");

        for (i, row) in rows.iter().enumerate() {
            // The detail column's height decides the row's.
            let value_h = 17.0 * 1.1;
            let pill_font = th.semi(12.0);
            let pill_h = 12.0 * LINE + 2.0;
            let detail_h = match i {
                0 => value_h + 2.0 + note_h,
                1 => pill_h * 2.0 + 3.0 + 2.0 + note_h,
                2 if pit.fast_repairs_left.is_some() => note_h,
                _ => 0.0,
            };
            let keys = keys_size(p, th, &row.hints);
            let content = detail_h.max(keys.y);
            let top = y;
            let h = content + 12.0 + if i > 0 { 1.0 } else { 0.0 };
            if i > 0 {
                p.hline(x, top + 0.5, egui::Stroke::new(1.0, th.line));
            }
            let body_top = top + if i > 0 { 1.0 } else { 0.0 } + 6.0;
            let cy = body_top + content / 2.0;
            check(p, th, x.min + 10.0, cy, row.on);
            let label_x = x.min + 10.0 + 12.0 + 8.0;
            let (lf, lc) = if row.on { (th.semi(15.0), th.ink) } else { (th.medium(15.0), th.ink3) };
            kit::text(p, Pos2::new(label_x, cy), Align2::LEFT_CENTER, row.label, lf, lc);
            let right = x.max - 10.0;
            if !row.hints.is_empty() {
                draw_keys(p, th, &row.hints, right, cy);
            }
            let dx = label_x + 76.0 + 8.0;
            let dtop = cy - detail_h / 2.0;
            // The detail column ends before the keys; long notes are cut short.
            let room = (right - if keys.x > 0.0 { keys.x + 8.0 } else { 0.0 } - dx).max(40.0);
            match i {
                0 => {
                    let vy = dtop + value_h / 2.0;
                    let text = if pit.fuel { format!("+{} {u}", v(pit.fuel_amount)) } else { "No fuel".to_string() };
                    let r = kit::text(p, Pos2::new(dx, vy), Align2::LEFT_CENTER, &text, th.bold(17.0), if pit.fuel { th.ink } else { th.ink3 });
                    if auto_on {
                        let tag_font = th.heavy(10.0);
                        let tw = kit::width_sp(p, "AUTO", tag_font.clone(), 0.4);
                        let tr = Rect::from_center_size(Pos2::new(r.right() + 6.0 + (tw + 8.0) / 2.0, vy - 1.0), Vec2::new(tw + 8.0, 10.0 * LINE));
                        kit::fill(p, tr, 3.0, if auto_paused { th.ink3 } else { th.gain });
                        kit::text_sp(p, tr.center(), Align2::CENTER_CENTER, "AUTO", tag_font, th.panel_solid, 0.4);
                    }
                    kit::text_fit(p, Pos2::new(dx, dtop + value_h + 2.0 + note_h / 2.0), Align2::LEFT_CENTER, &fuel_note, note_font.clone(), note(fuel_warn), room, 0.0);
                }
                1 => {
                    // Two by two, LF RF over LR RR.
                    let keys_w = keys.x + if keys.x > 0.0 { 8.0 } else { 0.0 };
                    let avail = right - keys_w - dx;
                    let texts: Vec<String> = (0..4).map(|k| if pit.pressures[k] > 0.0 { pressure(pit.pressures[k], units) } else { String::new() }).collect();
                    let need = (0..4)
                        .map(|k| kit::width(p, CORNERS[k], pill_font.clone()) + if texts[k].is_empty() { 0.0 } else { 4.0 + kit::width(p, &texts[k], th.medium(12.0)) } + 10.0)
                        .fold(0.0, f32::max);
                    let col_w = ((avail - 3.0) / 2.0).max(need);
                    for k in 0..4 {
                        let (col, line) = (k % 2, k / 2);
                        let r = Rect::from_min_size(Pos2::new(dx + col as f32 * (col_w + 3.0), dtop + line as f32 * (pill_h + 3.0)), Vec2::new(col_w, pill_h));
                        let on = pit.tires[k];
                        kit::fill(p, r, 3.0, if on { alpha(th.gain, 0.24) } else { alpha(th.ink, 0.06) });
                        let c = if on { th.ink } else { th.ink3 };
                        kit::text(p, Pos2::new(r.left() + 5.0, r.center().y), Align2::LEFT_CENTER, CORNERS[k], pill_font.clone(), c);
                        if !texts[k].is_empty() {
                            kit::text(p, Pos2::new(r.right() - 5.0, r.center().y), Align2::RIGHT_CENTER, &texts[k], th.medium(12.0), c);
                        }
                    }
                    kit::text_fit(p, Pos2::new(dx, dtop + pill_h * 2.0 + 3.0 + 2.0 + note_h / 2.0), Align2::LEFT_CENTER, &tire_note, note_font.clone(), th.ink3, room, 0.0);
                }
                2 => {
                    if let Some(left) = pit.fast_repairs_left {
                        let warn = pit.fast_repair && left == 0;
                        let s = if left == 0 { "None left".to_string() } else { format!("{left} left") };
                        kit::text(p, Pos2::new(dx, cy), Align2::LEFT_CENTER, &s, note_font.clone(), note(warn));
                    }
                }
                _ => {}
            }
            y += h;
        }

        // The foot: what the keys do, or why they don't work yet.
        if show_keys {
            p.hline(x, y + 0.5, egui::Stroke::new(1.0, th.line));
            let msg = if !in_car {
                "Hotkeys work while you're in the car".to_string()
            } else if let Some(c) = clear {
                format!("{c} unticks everything")
            } else {
                "Set keys in Settings → Controls".to_string()
            };
            let fy = y + 1.0 + 6.0;
            kit::text(p, Pos2::new(x.min + 10.0, fy + note_h / 2.0), Align2::LEFT_CENTER, &msg, th.medium(13.0), th.ink3);
            y += 1.0 + note_h + 12.0;
        }
        panel.finish(p, th, y)
    }
}
