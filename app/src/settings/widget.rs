//! One pane per widget, generated from the option specs: show, size,
//! opacity, screen, position, then each option as a switch, slider, list or
//! text box (`WidgetPane` and `OptionField`). The Delta pane adds the
//! reference lap import (`ReferenceSection`).

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use egui::Ui;
use serde_json::{json, Value};
use telemetry::ibt::IbtSummary;

use crate::config::{Opts, OPACITY_MAX, OPACITY_MIN, SCALE_MAX, SCALE_MIN};
use crate::logic::format::lap_time;
use crate::schema;
use crate::theme::Theme;

use super::edits::{default_widget_json, path_of, Edit, Editor};
use super::specs::{self, Kind, OptionSpec};
use super::ui::{self, Look};
use super::{number_json, Env, Msg};

const ANCHOR_WORDS: [(&str, &str); 6] = [("l", "left"), ("c", "centre"), ("r", "right"), ("t", "top"), ("m", "middle"), ("b", "bottom")];

fn word(k: &str) -> &'static str {
    ANCHOR_WORDS.iter().find(|(a, _)| *a == k).map(|(_, w)| *w).unwrap_or("")
}

fn position_help(ax: Option<&str>, ay: Option<&str>) -> String {
    let (Some(ax), Some(ay)) = (ax, ay) else {
        return "Default spot, placed for your screen size. Drag it in edit mode to move it.".into();
    };
    let wh = if ax == "c" && ay == "m" { "centre".to_string() } else { format!("{} {}", word(ay), word(ax)) };
    format!("Pinned to the {wh} of the screen, so it stays in place at any resolution. Drag it in edit mode to move it.")
}

pub fn show(ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor, reference: &mut RefState, tx: &Sender<Msg>, id: &'static str) {
    let (Some(meta), Some(spec)) = (schema::meta(id), specs::spec(id)) else { return };
    let w = env.store.config.widget(id).clone();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(meta.name).font(th.bold(30.0)).color(th.ink));
        if spec.experimental {
            ui.label(egui::RichText::new(" EXPERIMENTAL ").font(th.bold(12.0)).color(th.panel_solid).background_color(th.attention));
        }
    });
    ui.add_space(6.0);
    ui.scope(|ui| {
        ui.set_max_width(560.0);
        ui.add(egui::Label::new(ui::text(th, spec.description, 16.0, th.ink2)).wrap());
    });
    if spec.experimental {
        ui::note(ui, th, "Experimental: this is new and may change or have rough edges.");
    }

    ui::section(ui, th, "Display", |ui| {
        ui::field(ui, th, "Show this widget", None, |ui| {
            let mut on = w.enabled;
            if ui::toggle(ui, th, &mut on).changed() {
                ed.set(env.store, &["widgets", id, "enabled"], json!(on));
            }
        });
        let pct = |v: f64| format!("{}%", (v * 100.0).round());
        ui::field(ui, th, "Size", None, |ui| {
            if let Some(v) = ui::slider(ui, th, w.scale as f64, SCALE_MIN as f64, SCALE_MAX as f64, 0.05, &pct) {
                ed.set(env.store, &["widgets", id, "scale"], number_json((v * 100.0).round() / 100.0));
            }
        });
        ui::field(ui, th, "Opacity", None, |ui| {
            if let Some(v) = ui::slider(ui, th, w.opacity as f64, OPACITY_MIN as f64, OPACITY_MAX as f64, 0.05, &pct) {
                ed.set(env.store, &["widgets", id, "opacity"], number_json((v * 100.0).round() / 100.0));
            }
        });
        if env.displays.len() > 1 {
            let current = ed.raw["widgets"][id]["display"].as_str().map(String::from);
            let away = current.as_ref().is_some_and(|d| !env.displays.iter().any(|(k, _)| k == d));
            let help = if away {
                "Its screen isn't connected, so it's on the main one for now. It goes back when that screen is."
            } else {
                "Which display it shows on. You can also drag it across in edit mode."
            };
            ui::field(ui, th, "Screen", Some(help), |ui| {
                let key = current.filter(|d| env.displays.iter().any(|(k, _)| k == d)).unwrap_or_else(|| env.displays[0].0.clone());
                let choices: Vec<(&str, &str)> = env.displays.iter().map(|(k, n)| (k.as_str(), n.as_str())).collect();
                if let Some(v) = ui::select(ui, ("screen", id), &key, &choices) {
                    ed.set(env.store, &["widgets", id, "display"], json!(v));
                }
            });
        }
        let raw = &ed.raw["widgets"][id];
        let help = position_help(raw["ax"].as_str(), raw["ay"].as_str());
        ui::field(ui, th, "Position", Some(&help), |ui| {
            if ui::button(ui, th, "Reset position", Look::Normal).clicked() {
                for e in [
                    Edit::Set(path_of(&["widgets", id, "x"]), json!(meta.x)),
                    Edit::Set(path_of(&["widgets", id, "y"]), json!(meta.y)),
                    Edit::Set(path_of(&["widgets", id, "scale"]), json!(1)),
                    Edit::Remove(path_of(&["widgets", id, "ax"])),
                    Edit::Remove(path_of(&["widgets", id, "ay"])),
                    Edit::Remove(path_of(&["widgets", id, "display"])),
                ] {
                    ed.edit(env.store, e);
                }
            }
        });
    });

    if !spec.options.is_empty() {
        ui::section(ui, th, "Options", |ui| {
            for o in spec.options {
                option_field(ui, th, env, ed, id, o);
            }
            ui::field(ui, th, "Restore defaults", None, |ui| {
                if ui::button(ui, th, "Reset options", Look::Normal).clicked() {
                    ed.set(env.store, &["widgets", id, "options"], default_widget_json(meta)["options"].clone());
                }
            });
        });
    }
    if id == "delta" {
        reference.show(ui, th, env, tx);
    }
}

fn option_field(ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor, id: &str, o: &OptionSpec) {
    let opts = Opts::new(&env.store.config, id);
    let path = ["widgets", id, "options", o.key];
    match o.kind {
        Kind::Toggle => {
            let mut on = opts.b(o.key);
            ui::field(ui, th, o.label, o.help, |ui| {
                if ui::toggle(ui, th, &mut on).changed() {
                    ed.set(env.store, &path, json!(on));
                }
            });
        }
        Kind::Number { min, max, step, suffix } => {
            let v = opts.n(o.key).clamp(min, max);
            ui::field(ui, th, o.label, o.help, |ui| {
                let fmt = |v: f64| ui::number_text(v, step, suffix);
                if let Some(nv) = ui::slider(ui, th, v, min, max, step, &fmt) {
                    // Whole steps stay exact; the slider's float noise doesn't reach the file.
                    let nv = (nv / step).round() * step;
                    ed.set(env.store, &path, number_json((nv * 1e6).round() / 1e6));
                }
            });
        }
        Kind::Select(choices) => {
            let cur = opts.s(o.key).to_string();
            let help = choices.iter().find(|c| c.value == cur).and_then(|c| c.help).or(o.help);
            ui::field(ui, th, o.label, help, |ui| {
                let list: Vec<(&str, &str)> = choices.iter().map(|c| (c.value, c.label)).collect();
                if let Some(v) = ui::select(ui, ("opt", id, o.key), &cur, &list) {
                    ed.set(env.store, &path, json!(v));
                }
            });
        }
        Kind::Text { max_length } => {
            let mut text = opts.s(o.key).to_string();
            let default = schema::meta(id).and_then(|m| m.default_of(o.key)).map(|d| match d {
                schema::Def::S(s) => s.to_string(),
                _ => String::new(),
            });
            ui::field(ui, th, o.label, o.help, |ui| {
                let mut te = egui::TextEdit::singleline(&mut text).desired_width(228.0).hint_text(default.clone().unwrap_or_default());
                if let Some(n) = max_length {
                    te = te.char_limit(n);
                }
                if ui.add_sized([240.0, 32.0], te).changed() {
                    ed.set(env.store, &path, Value::String(text.clone()));
                }
            });
        }
    }
}

// --- Reference lap -------------------------------------------------------------------------

#[derive(Default)]
pub struct RefState {
    file: Option<(PathBuf, IbtSummary)>,
    chosen: Option<i32>,
    busy: bool,
    error: Option<String>,
}

impl RefState {
    /// The file dialog closed: read what laps it holds, off the UI thread (an .ibt can be hundreds of MB).
    pub fn picked(&mut self, path: Option<PathBuf>, tx: &Sender<Msg>) {
        let Some(path) = path else {
            self.busy = false;
            return;
        };
        let tx = tx.clone();
        std::thread::spawn(move || {
            let r = telemetry::lapfile::summary(&path).and_then(|s| {
                if s.laps.is_empty() {
                    Err("That file has no complete laps. Record at least one full lap from line to line.".to_string())
                } else {
                    Ok((path, s))
                }
            });
            let _ = tx.send(Msg::LapSummary(r));
        });
    }

    pub fn summary(&mut self, r: Result<(PathBuf, IbtSummary), String>) {
        self.busy = false;
        match r {
            Ok((path, summary)) => {
                // The fastest clean lap to start with.
                let mut laps: Vec<_> = summary.laps.iter().collect();
                laps.sort_by(|a, b| b.clean.cmp(&a.clean).then(a.lap_time.total_cmp(&b.lap_time)));
                self.chosen = laps.first().map(|l| l.lap);
                self.file = Some((path, summary));
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn loaded(&mut self, r: Result<(), String>) {
        self.busy = false;
        match r {
            Ok(()) => self.file = None,
            Err(e) => self.error = Some(e),
        }
    }

    fn show(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, tx: &Sender<Msg>) {
        ui::section(ui, th, "Reference lap and coaching", |ui| {
            let reference = env.live.reference.as_deref().filter(|r| r.loaded);
            let (label, help) = match reference {
                Some(r) => {
                    let mut parts = vec![
                        format!("{} by {} in the {}", lap_time(Some(r.lap_time)), if r.driver.is_empty() { "unknown driver" } else { &r.driver }, if r.car_name.is_empty() { "car" } else { &r.car_name }),
                        format!("{}{}", r.track_name, if r.track_config.is_empty() { String::new() } else { format!(", {}", r.track_config) }),
                    ];
                    if !r.matches_track {
                        parts.push("Not this track, so it's paused until you drive there".into());
                    }
                    if !r.matches_car {
                        parts.push("It's a different car from yours, so coaching will be less accurate".into());
                    }
                    if !r.has_steering {
                        parts.push("Steering isn't saved in .blap/.olap files, so there's no reference wheel or steering coaching. Use an .ibt lap for those".into());
                    }
                    ("Comparing against", parts.join(". "))
                }
                None => ("No reference lap", "Import iRacing's saved best or optimal lap (.blap or .olap, in Documents\\iRacing\\lapfiles) or a lap from a telemetry file (.ibt, recorded with Alt+L). It powers the Reference lap comparison, the ghost inputs and the Coach widget.".to_string()),
            };
            ui::field(ui, th, label, Some(&help), |ui| {
                let import_label = if reference.is_some() { "Import another lap" } else { "Import a lap" };
                if ui::button_on(ui, th, import_label, Look::Primary, !self.busy).clicked() {
                    self.busy = true;
                    self.error = None;
                    let (tx, ctx) = (tx.clone(), ui.ctx().clone());
                    std::thread::spawn(move || {
                        let p = rfd::FileDialog::new().add_filter("Lap files", &["ibt", "blap", "olap"]).pick_file();
                        let _ = tx.send(Msg::PickedLapFile(p));
                        ctx.request_repaint();
                    });
                }
                if reference.is_some() && ui::button_on(ui, th, "Remove", Look::Normal, !self.busy).clicked() {
                    let r = crate::shell::clear_saved_reference(&env_paths(env));
                    match r {
                        Ok(()) => env.control.set_reference(None),
                        Err(e) => self.error = Some(e),
                    }
                }
            });
            if let Some((path, summary)) = self.file.clone() {
                self.picker(ui, th, env, tx, path, &summary);
            }
            if let Some(e) = &self.error {
                ui::error(ui, th, e);
            }
        });
    }

    fn picker(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, tx: &Sender<Msg>, path: PathBuf, summary: &IbtSummary) {
        ui.add_space(12.0);
        let blap = summary.kind == telemetry::reference::RefKind::Blap;
        let track = format!("{}{}", summary.track_name, if summary.track_config.is_empty() { String::new() } else { format!(", {}", summary.track_config) });
        let driver = if summary.driver.is_empty() { "unknown" } else { &summary.driver };
        ui.add(egui::Label::new(ui::text(th, format!("{track}. {}, driven by {driver}. {}", summary.car_name, if blap { "" } else { "Pick a lap:" }), 14.0, th.ink3)).wrap());
        ui.add_space(8.0);
        let fastest = summary.laps.iter().map(|l| l.lap_time).fold(f32::INFINITY, f32::min);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for l in &summary.laps {
                let chosen = self.chosen == Some(l.lap);
                let (rect, r) = ui.allocate_exact_size(egui::vec2(150.0, 54.0), egui::Sense::click());
                let p = ui.painter();
                p.rect(rect, 5.0, th.panel_raised, egui::Stroke::new(1.0, if chosen { th.ink } else { th.line_strong }), egui::StrokeKind::Inside);
                if chosen {
                    p.rect_stroke(rect.shrink(1.0), 4.0, egui::Stroke::new(1.0, th.ink), egui::StrokeKind::Inside);
                }
                let name = if blap { "Saved lap".to_string() } else { format!("Lap {}", l.lap) };
                let g = p.layout_no_wrap(name, th.medium(16.0), th.ink);
                p.galley(rect.min + egui::vec2(10.0, 8.0), g, th.ink);
                let tcolor = if l.lap_time == fastest { th.fastest } else { th.ink };
                let g = p.layout_no_wrap(lap_time(Some(l.lap_time)), th.bold(16.0), tcolor);
                p.galley(egui::pos2(rect.right() - 10.0 - g.size().x, rect.top() + 8.0), g, tcolor);
                let g = p.layout_no_wrap(if l.clean { "Clean".into() } else { "Pit or off track".into() }, th.medium(13.0), th.ink3);
                p.galley(rect.min + egui::vec2(10.0, 32.0), g, th.ink3);
                if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.chosen = Some(l.lap);
                }
            }
        });
        ui.add_space(12.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let lap = self.chosen;
            let label = if blap { "Use this lap".to_string() } else { format!("Use lap {}", lap.unwrap_or(0)) };
            if ui::button_on(ui, th, &label, Look::Primary, !self.busy && lap.is_some()).clicked() {
                self.busy = true;
                self.error = None;
                let (tx, ctx, control, paths) = (tx.clone(), ui.ctx().clone(), env.control.clone(), env_paths(env));
                std::thread::spawn(move || {
                    let r = telemetry::lapfile::reference(&path, lap.unwrap_or(0)).and_then(|reference| {
                        crate::shell::save_reference(&paths, &reference)?;
                        control.set_reference(Some(reference));
                        Ok(())
                    });
                    let _ = tx.send(Msg::ReferenceLoaded(r));
                    ctx.request_repaint();
                });
            }
            if ui::button(ui, th, "Cancel", Look::Normal).clicked() {
                self.file = None;
            }
        });
    }
}

fn env_paths(env: &Env<'_>) -> crate::shell::Paths {
    env.shell.paths.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_help_words() {
        assert!(position_help(None, None).starts_with("Default spot"));
        assert!(position_help(Some("r"), Some("t")).starts_with("Pinned to the top right of the screen"));
        assert!(position_help(Some("c"), Some("m")).starts_with("Pinned to the centre of the screen"));
    }

    #[test]
    fn every_widget_has_specs_that_match_schema_defaults() {
        for m in schema::WIDGETS {
            let spec = specs::spec(m.id).unwrap_or_else(|| panic!("no spec for {}", m.id));
            assert_eq!(spec.options.len(), m.options.len(), "{} option count", m.id);
            for (key, def) in m.options {
                let o = spec.options.iter().find(|o| o.key == *key).unwrap_or_else(|| panic!("{}.{key} has no control", m.id));
                // The generated copy of config.ts agrees with schema.rs.
                let (_, _, ts) = specs::TS_DEFAULTS.iter().find(|(w, k, _)| *w == m.id && k == key).unwrap();
                let want = match def {
                    schema::Def::B(b) => json!(b),
                    schema::Def::N(n) => json!(n),
                    schema::Def::S(s) => json!(s),
                };
                let got: Value = serde_json::from_str(ts).unwrap();
                match (got.as_f64(), want.as_f64()) {
                    (Some(a), Some(b)) => assert_eq!(a, b, "{}.{key} default", m.id),
                    _ => assert_eq!(got, want, "{}.{key} default", m.id),
                }
                match (o.kind, def) {
                    (Kind::Toggle, schema::Def::B(_)) | (Kind::Text { .. }, schema::Def::S(_)) => {}
                    (Kind::Number { min, max, .. }, schema::Def::N(n)) => assert!(min <= *n && *n <= max, "{}.{key} range", m.id),
                    (Kind::Select(c), schema::Def::S(s)) => assert!(c.iter().any(|c| c.value == *s), "{}.{key} choice", m.id),
                    _ => panic!("{}.{key}: control and default disagree", m.id),
                }
            }
        }
    }
}
