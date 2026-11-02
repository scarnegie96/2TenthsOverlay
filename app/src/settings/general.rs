//! General: data source, units, when to show, shortcuts, the widgets' on/off
//! switches and sizes, and reset (`GeneralPane` and `WidgetsSection`).

use egui::Ui;
use serde_json::json;

use crate::config::{ShowWhen, Units};
use crate::logic::settings_bindings as bind;
use crate::schema;
use crate::theme::Theme;

use super::edits::Editor;
use super::ui::{self, Look};
use super::{specs, Env};

/// What runs an action now, as a key cap: its key or button, or "None".
pub fn bound(ui: &mut Ui, th: &Theme, controls: &serde_json::Value, id: &str) {
    let all = bind::from_json(controls);
    let label = all.iter().find(|(a, _)| a.id == id).and_then(|(_, b)| bind::full_label(b));
    match label {
        Some(l) => ui::kbd(ui, th, &l),
        None => {
            ui.label(ui::text(th, "None", 15.0, th.ink3));
        }
    }
}

const SCALE_PRESETS: [f32; 5] = [0.75, 1.0, 1.25, 1.5, 2.0];

/// Returns true when "Controls" was clicked.
pub fn show(ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) -> bool {
    let mut to_controls = false;
    ui::title(ui, th, "General", None);
    let g = env.store.config.general.clone();

    ui::section(ui, th, "Data", |ui| {
        ui::field(ui, th, "Use the demo race", Some("A fake multi-class race, for setting up layouts without iRacing running."), |ui| {
            let mut on = g.demo;
            if ui::toggle(ui, th, &mut on).changed() {
                ed.set(env.store, &["general", "demo"], json!(on));
                // The runner switches sources at once.
                env.control.demo.store(on, std::sync::atomic::Ordering::Relaxed);
            }
        });
        ui::field(ui, th, "Units", None, |ui| {
            let cur = if g.units == Units::Imperial { "imperial" } else { "metric" };
            if let Some(v) = ui::select(ui, "units", cur, &[("metric", "Metric (km/h, litres)"), ("imperial", "Imperial (mph, gallons)")]) {
                ed.set(env.store, &["general", "units"], json!(v));
            }
        });
        ui::field(ui, th, "Show overlays", None, |ui| {
            let cur = if g.show_when == ShowWhen::OnTrack { "onTrack" } else { "connected" };
            if let Some(v) = ui::select(ui, "showWhen", cur, &[("connected", "Whenever iRacing is running"), ("onTrack", "Only when I'm in the car")]) {
                ed.set(env.store, &["general", "showWhen"], json!(v));
            }
        });
    });

    ui::section(ui, th, "Shortcuts", |ui| {
        let c = &g.controls;
        ui::field(ui, th, "Edit layout", None, |ui| bound(ui, th, c, "overlay.edit"));
        ui::field(ui, th, "Show or hide overlays", None, |ui| bound(ui, th, c, "overlay.toggle"));
        ui::field(ui, th, "Next saved layout", None, |ui| bound(ui, th, c, "layout.next"));
        ui::field(ui, th, "Saved layout 1 to 9", None, |ui| {
            // Right to left: the last first.
            bound(ui, th, c, "layout.9");
            ui.label(ui::text(th, "…", 15.0, th.ink2));
            bound(ui, th, c, "layout.1");
        });
        ui::field(ui, th, "Change keys and wheel buttons", Some("Every shortcut, the Pit stop keys, and buttons on your wheel or button box."), |ui| {
            if ui::button(ui, th, "Controls", Look::Normal).clicked() {
                to_controls = true;
            }
        });
        ui::note(ui, th, "Run iRacing in borderless windowed mode. Overlays can't draw on top of exclusive fullscreen.");
    });

    // Quick show/hide for every panel, and one size for all of them.
    ui::section(ui, th, "Widgets", |ui| {
        let scales: Vec<f32> = specs::SPECS.iter().map(|s| env.store.config.widget(s.id).scale).collect();
        let common = scales.windows(2).all(|w| w[0] == w[1]).then(|| scales[0]);
        ui::field(ui, th, "Size of every widget", Some("Sets all widgets to the same size. At 1440p, 125% looks about the same as 100% at 1080p."), |ui| {
            let labels: Vec<String> = SCALE_PRESETS.iter().map(|v| format!("{}%", (v * 100.0).round())).collect();
            let on = common.and_then(|c| SCALE_PRESETS.iter().position(|v| (*v - c).abs() < 1e-6));
            if let Some(i) = ui::segmented(ui, th, &labels, on) {
                for s in specs::SPECS {
                    ed.set(env.store, &["widgets", s.id, "scale"], json!(SCALE_PRESETS[i]));
                }
            }
        });
        ui.add_space(12.0);
        let width = ui.available_width().min(ui::COLUMN);
        let cols = ((width + 24.0) / (190.0 + 24.0)).floor().max(1.0) as usize;
        let col_w = (width - 24.0 * (cols as f32 - 1.0)) / cols as f32;
        egui::Grid::new("widget-toggles").num_columns(cols).spacing([24.0, 6.0]).min_col_width(col_w).show(ui, |ui| {
            for (i, s) in specs::SPECS.iter().enumerate() {
                let name = schema::meta(s.id).map(|m| m.name).unwrap_or(s.id);
                let mut on = env.store.config.widget(s.id).enabled;
                ui.horizontal(|ui| {
                    ui.set_min_width(col_w);
                    ui.set_min_height(32.0);
                    ui.label(ui::text(th, name, 16.0, th.ink));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui::toggle(ui, th, &mut on).changed() {
                            ed.set(env.store, &["widgets", s.id, "enabled"], json!(on));
                        }
                    });
                });
                if i % cols == cols - 1 {
                    ui.end_row();
                }
            }
        });
        ui.add_space(4.0);
    });

    ui::section(ui, th, "Layout", |ui| {
        ui::field(ui, th, "Reset every widget", Some("Puts all widgets back to their default size, position and options."), |ui| {
            if ui::button(ui, th, "Reset layout", Look::Danger).clicked() {
                ed.set(env.store, &["widgets"], super::edits::default_widgets_json());
            }
        });
    });
    to_controls
}
