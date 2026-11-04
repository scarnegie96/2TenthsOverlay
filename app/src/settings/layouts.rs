//! Layouts: save where every widget sits as a layout, switch between them,
//! rename, duplicate, export, import and delete (`LayoutsPane`). Changes made
//! while a layout is active are saved into it (`Editor::flush`).

use std::path::PathBuf;
use std::sync::mpsc::Sender;

use egui::{Align, Layout, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use serde_json::{json, Value};

use crate::logic::settings_bindings as bind;
use crate::theme::Theme;

use super::edits::{widgets_over_defaults, Editor};
use super::layouts_io as io;
use super::ui::{self, Look};
use super::{Env, Msg};

#[derive(Default)]
pub struct State {
    list: Option<Vec<io::ProfileSummary>>,
    name: String,
    renaming: Option<(String, String)>,
    confirm_delete: Option<String>,
    error: Option<String>,
}

/// The widgets as they are on screen, all of them, as a layout holds them.
fn on_screen(ed: &Editor) -> Value {
    widgets_over_defaults(&ed.raw["widgets"])
}

impl State {
    fn refresh(&mut self, env: &Env<'_>) {
        self.list = env.shell.paths.layouts_dir().map(|d| io::list(&d));
    }

    fn run(&mut self, r: Result<(), String>) {
        self.error = r.err().map(|e| e.trim_start_matches("Error: ").to_string());
    }

    fn switch(&mut self, id: &str, ed: &mut Editor, env: &mut Env<'_>) -> Result<(), String> {
        let dir = env.shell.paths.layouts_dir().ok_or("No folder for layouts.")?;
        let profile = io::read(&dir, id)?;
        ed.set(env.store, &["widgets"], widgets_over_defaults(&profile.widgets));
        ed.set(env.store, &["general", "activeProfile"], json!(id));
        // The widgets were just saved into this layout: don't rewrite it again.
        Ok(())
    }

    pub fn imported(&mut self, path: Option<PathBuf>, ed: &mut Editor, env: &mut Env<'_>) {
        let Some(path) = path else { return };
        let Some(dir) = env.shell.paths.layouts_dir() else { return };
        let r = io::import(&dir, &path).and_then(|id| self.switch(&id, ed, env));
        self.run(r);
        self.refresh(env);
    }

    pub fn exported(&mut self, id: &str, path: Option<PathBuf>, env: &Env<'_>) {
        let (Some(path), Some(dir)) = (path, env.shell.paths.layouts_dir()) else { return };
        let r = io::export(&dir, id, &path);
        self.run(r);
    }

    pub fn show(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor, tx: &Sender<Msg>) {
        if self.list.is_none() {
            self.refresh(env);
        }
        let list = self.list.clone().unwrap_or_default();
        let active = ed.raw["general"]["activeProfile"].as_str().map(String::from);
        let dir = env.shell.paths.layouts_dir();
        let controls = env.store.config.general.controls.clone();
        let all = bind::from_json(&controls);
        let label_of = |id: &str| all.iter().find(|(a, _)| a.id == id).and_then(|(_, b)| bind::full_label(b));

        ui::title(
            ui,
            th,
            "Layouts",
            Some("Save where every widget sits, its size, opacity and options as a layout, and switch between them, for example one for sprints and one for endurance races. Changes you make while a layout is active are saved into it."),
        );
        let active_name = active.as_ref().and_then(|a| list.iter().find(|p| &p.id == a)).map(|p| p.name.clone());
        ui::section(ui, th, "On screen now", |ui| {
            let label = active_name.clone().unwrap_or_else(|| "Unsaved layout".into());
            let help = if active_name.is_some() { "Moving, resizing or changing widgets updates this layout." } else { "Save it to switch back to it later." };
            ui::field(ui, th, &label, Some(help), |ui| {
                let name = self.name.trim().to_string();
                let save_label = if active_name.is_some() { "Save as new" } else { "Save layout" };
                let mut create = false;
                if ui::button_on(ui, th, save_label, Look::Primary, !name.is_empty()).clicked() {
                    create = true;
                }
                let te = egui::TextEdit::singleline(&mut self.name).hint_text("Name, e.g. Endurance").char_limit(40).desired_width(190.0);
                let r = ui.add_sized([190.0, 32.0], te);
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !name.is_empty() {
                    create = true;
                }
                if create {
                    if let Some(dir) = &dir {
                        let res = io::save(dir, None, Some(&name), on_screen(ed));
                        match res {
                            Ok(id) => {
                                ed.set(env.store, &["general", "activeProfile"], json!(id));
                                self.name.clear();
                                self.error = None;
                            }
                            Err(e) => self.error = Some(e),
                        }
                        self.refresh(env);
                    }
                }
            });
        });

        ui::section(ui, th, "Saved layouts", |ui| {
            if list.is_empty() {
                ui::note(ui, th, "No saved layouts yet.");
            }
            for (i, p) in list.iter().enumerate() {
                self.row(ui, th, env, ed, tx, p, i, active.as_deref() == Some(&p.id), &label_of);
            }
            ui::field(ui, th, "Import a layout", Some("A layout file exported from 2Tenths, on this PC or someone else's."), |ui| {
                if ui::button(ui, th, "Import…", Look::Normal).clicked() {
                    let tx = tx.clone();
                    let ctx = ui.ctx().clone();
                    std::thread::spawn(move || {
                        let p = rfd::FileDialog::new().add_filter("2Tenths layout", &["json"]).pick_file();
                        let _ = tx.send(Msg::ImportLayout(p));
                        ctx.request_repaint();
                    });
                }
            });
        });
        if let Some(e) = &self.error {
            ui::error(ui, th, e);
        }
        ui.add_space(12.0);
        let next = label_of("layout.next").unwrap_or_else(|| "its key".into());
        ui::note(
            ui,
            th,
            &format!("Switch with {next} for the next layout, or each layout's own key from its number (change them under Controls). Layouts are saved as files in the 2Tenths settings folder, under layouts."),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn row(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor, tx: &Sender<Msg>, p: &io::ProfileSummary, index: usize, active: bool, label_of: &dyn Fn(&str) -> Option<String>) {
        let own = (index < 9).then(|| label_of(&format!("layout.{}", index + 1))).flatten();
        let dir = env.shell.paths.layouts_dir();
        let mut act: Option<Act> = None;
        let width = ui.available_width().min(ui::COLUMN);
        let x0 = ui.cursor().min.x;
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.set_max_width(width);
            ui.spacing_mut().item_spacing.x = 14.0;
            // The number key.
            let (rect, r) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
            if index < 9 {
                let color = if active { th.gain } else { th.ink2 };
                let border = if active { th.gain } else { th.line_strong };
                ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, border), StrokeKind::Inside);
                let g = ui.painter().layout_no_wrap((index + 1).to_string(), th.bold(13.0), color);
                ui.painter().galley(rect.center() - g.size() / 2.0, g, color);
                r.on_hover_text(own.unwrap_or_else(|| "Switch to it here".into()));
            }
            let editing = self.renaming.as_ref().is_some_and(|(id, _)| *id == p.id);
            ui.vertical(|ui| {
                ui.set_width((width - 24.0 - 14.0 * 2.0 - 380.0).max(120.0));
                ui.spacing_mut().item_spacing.y = 2.0;
                if editing {
                    let (_, text) = self.renaming.as_mut().unwrap();
                    let r = ui.add(egui::TextEdit::singleline(text).char_limit(40).desired_width(200.0));
                    r.request_focus();
                    let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                    if enter || r.lost_focus() && !esc {
                        act = Some(Act::Rename);
                    } else if esc {
                        self.renaming = None;
                    }
                } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&p.name).font(th.medium(16.0)).color(th.ink));
                        if active {
                            ui.label(RichText::new(" On screen ").font(th.bold(12.0)).color(th.panel_solid).background_color(th.gain));
                        }
                    });
                }
                ui.label(ui::text(th, format!("Saved {}", super::timefmt::date_medium(p.updated_at)), 14.0, th.ink3));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let confirm = self.confirm_delete.as_deref() == Some(&p.id);
                let del = if confirm { "Really delete?" } else { "Delete" };
                if ui::button(ui, th, del, if confirm { Look::Danger } else { Look::Quiet }).clicked() {
                    if confirm {
                        act = Some(Act::Delete);
                    } else {
                        self.confirm_delete = Some(p.id.clone());
                    }
                } else if confirm && ui.input(|i| i.pointer.any_click()) {
                    // Clicking anywhere else cancels, as leaving the button does on the web.
                    self.confirm_delete = None;
                }
                if ui::button(ui, th, "Export", Look::Quiet).clicked() {
                    act = Some(Act::Export);
                }
                if ui::button(ui, th, "Duplicate", Look::Quiet).clicked() {
                    act = Some(Act::Duplicate);
                }
                if ui::button(ui, th, "Rename", Look::Quiet).clicked() {
                    self.renaming = Some((p.id.clone(), p.name.clone()));
                }
                if !active && ui::button(ui, th, "Switch to", Look::Primary).clicked() {
                    act = Some(Act::Switch);
                }
            });
        });
        ui.add_space(8.0);
        let y = ui.cursor().min.y;
        ui.painter().hline(x0..=x0 + width, y, Stroke::new(1.0, th.line));

        let Some(dir) = dir else { return };
        let widgets_of = |ed: &Editor| -> Result<Value, String> { if active { Ok(on_screen(ed)) } else { Ok(io::read(&dir, &p.id)?.widgets) } };
        match act {
            Some(Act::Switch) => {
                let r = self.switch(&p.id, ed, env);
                self.run(r);
            }
            Some(Act::Rename) => {
                if let Some((id, name)) = self.renaming.take() {
                    let name = name.trim().to_string();
                    if !name.is_empty() && name != p.name {
                        let r = widgets_of(ed).and_then(|w| io::save(&dir, Some(&id), Some(&name), w)).map(|_| ());
                        self.run(r);
                        self.refresh(env);
                    }
                }
            }
            Some(Act::Duplicate) => {
                let r = widgets_of(ed).and_then(|w| io::save(&dir, None, Some(&format!("{} copy", p.name)), w)).map(|_| ());
                self.run(r);
                self.refresh(env);
            }
            Some(Act::Export) => {
                let (id, name, tx, ctx) = (p.id.clone(), p.name.clone(), tx.clone(), ui.ctx().clone());
                std::thread::spawn(move || {
                    let file = rfd::FileDialog::new().set_file_name(format!("{name}.layout.json")).add_filter("2Tenths layout", &["json"]).save_file();
                    let _ = tx.send(Msg::ExportLayout(id, file));
                    ctx.request_repaint();
                });
            }
            Some(Act::Delete) => {
                self.confirm_delete = None;
                let r = io::delete(&dir, &p.id);
                self.run(r);
                // The widgets stay as they are, just no longer saved anywhere.
                if active {
                    ed.set(env.store, &["general", "activeProfile"], Value::Null);
                }
                self.refresh(env);
            }
            None => {}
        }
    }
}

enum Act {
    Switch,
    Rename,
    Duplicate,
    Export,
    Delete,
}
