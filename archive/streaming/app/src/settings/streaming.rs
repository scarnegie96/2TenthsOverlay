//! Settings → Streaming: the stream window for OBS (stream.rs).

use egui::Ui;
use serde_json::json;

use crate::config::StreamBackground;
use crate::schema::WIDGETS;
use crate::stream;
use crate::theme::Theme;

use super::edits::Editor;
use super::ui;
use super::Env;

const SIZES: [(f32, &str); 3] = [(0.5, "50%"), (0.75, "75%"), (1.0, "100%")];

pub fn show(ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) {
    ui::title(
        ui,
        th,
        "Streaming",
        Some("Show the widgets on stream without capturing your screen: Tenths opens a window of them on a plain background, and OBS captures that window."),
    );
    let st = env.store.config.general.streaming.clone();

    ui::section(ui, th, "Stream window", |ui| {
        ui::field(ui, th, "Stream window", Some("Opens a window called \"Tenths stream\". Closing it turns this off. The tray menu has the same switch."), |ui| {
            let mut on = st.window;
            if ui::toggle(ui, th, &mut on).changed() {
                ed.set(env.store, &["general", "streaming", "window"], json!(on));
            }
        });
        ui::field(ui, th, "Background", Some("A colour to key out in OBS."), |ui| {
            let choices: Vec<(&str, &str)> = StreamBackground::ALL.iter().map(|b| (b.key(), stream::background_name(*b))).collect();
            if let Some(v) = ui::select(ui, "stream-background", st.background.key(), &choices) {
                ed.set(env.store, &["general", "streaming", "background"], json!(v));
            }
        });
        if env.displays.len() > 1 {
            ui::field(ui, th, "Screen", Some("Follows this screen's layout and size."), |ui| {
                let main = env.displays[0].0.as_str();
                let current = st.display.as_deref().filter(|k| env.displays.iter().any(|(d, _)| d == k)).unwrap_or(main);
                let choices: Vec<(&str, &str)> = env.displays.iter().map(|(k, n)| (k.as_str(), n.as_str())).collect();
                if let Some(v) = ui::select(ui, "stream-display", current, &choices) {
                    if v == main {
                        ed.remove(env.store, &["general", "streaming", "display"]);
                    } else {
                        ed.set(env.store, &["general", "streaming", "display"], json!(v));
                    }
                }
            });
        }
        ui::field(ui, th, "Size", Some("Of that screen. Smaller is lighter for OBS; 100% is its full resolution."), |ui| {
            let labels: Vec<String> = SIZES.iter().map(|(_, l)| (*l).to_string()).collect();
            let on = SIZES.iter().position(|(s, _)| (s - st.scale).abs() < 0.01);
            if let Some(i) = ui::segmented(ui, th, &labels, on) {
                ed.set(env.store, &["general", "streaming", "scale"], json!(SIZES[i].0));
            }
        });
    });

    ui::section(ui, th, "Shows", |ui| {
        // "Only these" with nothing ticked yet is still "only these".
        let key = ui.scope_id().with("stream-pick");
        let mut pick = ui.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) || !st.widgets.is_empty();
        ui::field(ui, th, "Widgets", Some("Everything the layout has on that screen, where it is there, or only the ones you pick, wherever the layout puts them (even a widget you have off on screen)."), |ui| {
            let labels = vec!["Everything on the screen".to_string(), "Only these".to_string()];
            if let Some(i) = ui::segmented(ui, th, &labels, Some(usize::from(pick))) {
                pick = i == 1;
                ui.data_mut(|d| d.insert_temp(key, pick));
                if !pick && !st.widgets.is_empty() {
                    ed.remove(env.store, &["general", "streaming", "widgets"]);
                }
            }
        });
        if pick {
            ui.add_space(8.0);
            let mut chosen = st.widgets.clone();
            let mut changed = false;
            egui::Grid::new("stream-widgets").num_columns(3).spacing([24.0, 6.0]).show(ui, |ui| {
                for (i, m) in WIDGETS.iter().enumerate() {
                    let mut on = chosen.iter().any(|id| id == m.id);
                    if ui.checkbox(&mut on, ui::text(th, m.name, 16.0, th.ink)).changed() {
                        changed = true;
                        if on {
                            chosen.push(m.id.to_string());
                        } else {
                            chosen.retain(|id| id != m.id);
                        }
                    }
                    if i % 3 == 2 {
                        ui.end_row();
                    }
                }
            });
            if changed {
                // In the widgets' own order.
                chosen.sort_by_key(|id| WIDGETS.iter().position(|m| m.id == id));
                if chosen.is_empty() {
                    ed.remove(env.store, &["general", "streaming", "widgets"]);
                } else {
                    ed.set(env.store, &["general", "streaming", "widgets"], json!(chosen));
                }
            }
        }
    });

    ui::section(ui, th, "In OBS", |ui| {
        ui.add_space(8.0);
        let steps = [
            "Sources → + → Window Capture, then pick \"Tenths stream\" as the Window.",
            "Set Capture Method to \"Windows 10 (1903 and up)\". The window can then be behind iRacing and still be captured.",
            "Right-click the source → Filters → + → Chroma Key, and choose the background colour above (Green, or Magenta if your overlay has green in it). With a black or grey background, use a Color Key filter instead.",
            "Untick \"Capture Cursor\", and resize the source in the scene to taste.",
        ];
        for (i, s) in steps.iter().enumerate() {
            ui.add(egui::Label::new(ui::text(th, format!("{}.  {s}", i + 1), 15.0, th.ink2)).wrap());
            ui.add_space(4.0);
        }
        ui::note(ui, th, "The widgets look as they do on screen, with the same theme, options and opacity, and run only while the window is open. The Tauri version's OBS web pages are not part of this build.");
    });
}
