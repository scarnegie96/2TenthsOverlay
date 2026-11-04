//! About (version and updates).
//!
//! The About page drives the updater (`crate::updates`): the version, when it
//! last checked, "Check now", the newer version with its notes and an
//! **Update to x.y.z** button with progress, and the automatic-check switch
//! (`general.checkUpdates`). Problems are shown in words under the Updates row.

use egui::Ui;
use serde_json::json;

use crate::theme::Theme;
use crate::updates::{self, Phase, Status};

use super::edits::Editor;
use super::ui::{self, Look};
use super::Env;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Updates row's help line: what the updater is doing or found.
fn update_line(s: &Status, now: u64) -> String {
    match s.phase {
        Phase::Checking => "Looking for a newer version…".into(),
        Phase::Downloading { done, total } => format!("Downloading the update… {}", updates::progress_text(done, total)),
        Phase::Verifying => "Checking the download…".into(),
        Phase::Launching => "Starting the installer. 2Tenths closes and opens again in a moment.".into(),
        Phase::Idle => match (&s.available, s.last_checked) {
            (Some(r), _) => match &r.published {
                Some(d) => format!("Version {} is available (published {d}).", r.version),
                None => format!("Version {} is available.", r.version),
            },
            (None, Some(t)) => format!("You're up to date. Last checked {}.", updates::ago(t, now)),
            (None, None) => "Not checked yet.".into(),
        },
    }
}

pub fn show(ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) {
    ui::title(ui, th, "About", None);
    let auto = ed.raw["general"]["checkUpdates"].as_bool().unwrap_or(true);
    let status = env.updates.status();
    ui::section(ui, th, "2Tenths", |ui| {
        ui::field(ui, th, "Version", Some("The native build of 2Tenths, drawn with egui."), |ui| {
            ui.label(ui::text(th, VERSION, 16.0, th.ink));
        });
        ui::field(ui, th, "Updates", Some(&update_line(&status, updates::now())), |ui| {
            let ctx = ui.ctx().clone();
            match &status.available {
                Some(r) if !status.busy() => {
                    if ui::button(ui, th, &format!("Update to {}", r.version), Look::Primary).clicked() {
                        env.updates.install(&ctx);
                    }
                    if ui::button(ui, th, "Check again", Look::Quiet).clicked() {
                        env.updates.check_now(&ctx);
                    }
                }
                Some(_) => {}
                None => {
                    if ui::button_on(ui, th, "Check now", Look::Normal, !status.busy()).clicked() {
                        env.updates.check_now(&ctx);
                    }
                }
            }
        });
        if let Phase::Downloading { done, total: Some(total) } = status.phase {
            ui.add_space(8.0);
            ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).desired_width(ui.available_width().min(ui::COLUMN)));
        }
        if let Some(e) = &status.error {
            ui.add_space(8.0);
            ui.add(egui::Label::new(ui::text(th, e, 14.0, th.loss)).wrap());
        }
        ui::field(ui, th, "Check automatically", Some("A little after 2Tenths starts, then every six hours. Nothing installs until you say so."), |ui| {
            let mut on = auto;
            if ui::toggle(ui, th, &mut on).changed() {
                ed.set(env.store, &["general", "checkUpdates"], json!(on));
                env.updates.set_auto(on);
            }
        });
    });
    if let Some(r) = status.available.as_ref().filter(|r| !r.notes.is_empty()) {
        ui::section(ui, th, &format!("What's new in {}", r.version), |ui| {
            ui.add_space(8.0);
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([true, true]).show(ui, |ui| {
                ui.set_max_width(ui::COLUMN);
                for line in r.notes.lines().filter(|l| !l.trim().is_empty()) {
                    ui.add(egui::Label::new(ui::text(th, line.trim_end(), 15.0, th.ink2)).wrap());
                    ui.add_space(4.0);
                }
            });
        });
    }
    ui::section(ui, th, "Where things are", |ui| {
        let p = &env.shell.paths;
        let show = |path: Option<std::path::PathBuf>| path.map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".into());
        ui::field(ui, th, "Settings", Some(&show(p.config.clone())), |_| {});
        ui::field(ui, th, "Laps and reports", Some(&show(p.data.clone())), |_| {});
    });
}
