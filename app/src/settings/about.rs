//! About (version and updates). (The Streaming page is streaming.rs.)
//!
//! The updater is not part of the egui build yet (it waits until this build
//! becomes the release), so the update rows say so. The setting itself,
//! `general.checkUpdates`, is still read and written, so switching builds
//! keeps it.

use egui::Ui;
use serde_json::json;

use crate::theme::Theme;

use super::edits::Editor;
use super::ui::{self, Look};
use super::Env;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn show(ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) {
    ui::title(ui, th, "About", None);
    let auto = ed.raw["general"]["checkUpdates"].as_bool().unwrap_or(true);
    ui::section(ui, th, "Tenths", |ui| {
        ui::field(ui, th, "Version", Some("The native build of Tenths, drawn with egui."), |ui| {
            ui.label(ui::text(th, VERSION, 16.0, th.ink));
        });
        ui::field(ui, th, "Updates", Some("Updates aren't built into this version yet. They arrive when the native build becomes the release; until then, install Tenths from its GitHub releases page."), |ui| {
            ui::button_on(ui, th, "Check now", Look::Normal, false);
        });
        ui::field(ui, th, "Check automatically", Some("A little after Tenths starts, then every six hours. Nothing installs until you say so."), |ui| {
            let mut on = auto;
            if ui::toggle(ui, th, &mut on).changed() {
                ed.set(env.store, &["general", "checkUpdates"], json!(on));
            }
        });
    });
    ui::section(ui, th, "Where things are", |ui| {
        let p = &env.shell.paths;
        let show = |path: Option<std::path::PathBuf>| path.map(|p| p.display().to_string()).unwrap_or_else(|| "unknown".into());
        ui::field(ui, th, "Settings", Some(&show(p.config.clone())), |_| {});
        ui::field(ui, th, "Laps and reports", Some(&show(p.data.clone())), |_| {});
    });
}
