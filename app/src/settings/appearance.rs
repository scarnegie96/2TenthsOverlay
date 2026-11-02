//! Appearance: the theme, and every colour the user can change with a reset
//! and the "follows" rules (`AppearancePane` and `src/lib/theme.ts`).

use std::collections::HashMap;

use egui::Ui;
use serde_json::{json, Value};

use crate::config::ThemeId;
use crate::theme::{default_color, parse_hex, Theme, COLOR_KEYS};

use super::edits::Editor;
use super::ui::{self, Look};
use super::Env;

struct ColorSpec {
    key: &'static str,
    label: &'static str,
    help: Option<&'static str>,
}

const fn c(key: &'static str, label: &'static str, help: Option<&'static str>) -> ColorSpec {
    ColorSpec { key, label, help }
}

/// `COLOR_GROUPS` in theme.ts.
const GROUPS: &[(&str, &[ColorSpec])] = &[
    ("Text", &[c("ink", "Text", None), c("ink-2", "Secondary text", None), c("ink-3", "Muted text", Some("Labels, units and scales"))]),
    (
        "Panels",
        &[
            c("panel-solid", "Panel background", Some("Panels stay slightly see-through; set each widget's opacity in its own settings")),
            c("panel-raised", "Raised surfaces", Some("Buttons and highlighted rows")),
            c("edge", "Borders and dividers", None),
            c("wheel", "Steering wheel and car", Some("In the Inputs and Car dynamics widgets")),
        ],
    ),
    (
        "Timing",
        &[
            c("fastest", "Fastest in class", None),
            c("gain", "Gaining and personal best", None),
            c("loss", "Losing time", None),
            c("attention", "Warnings and highlights", Some("Also the cars alongside on the radar")),
            c("lapped", "Cars you're lapping", None),
            c("lapping", "Cars lapping you", None),
        ],
    ),
    ("Pedals", &[c("throttle", "Throttle", None), c("brake", "Brake", None), c("clutch", "Clutch", None)]),
];

fn label_of(key: &str) -> &'static str {
    GROUPS.iter().flat_map(|(_, cs)| cs.iter()).find(|s| s.key == key).map(|s| s.label).unwrap_or("")
}

pub fn theme_label(t: ThemeId) -> &'static str {
    match t {
        ThemeId::Dark => "Dark",
        ThemeId::Light => "Light",
        ThemeId::F1 => "Box Box",
    }
}

/// The colour in use: the user's own, else the one it follows, else the theme's (`effectiveColor`).
pub fn effective(theme: ThemeId, own: &HashMap<String, String>, key: &str) -> String {
    if let Some(v) = own.get(key).filter(|v| parse_hex(v).is_some()) {
        return v.clone();
    }
    if let Some((_, Some(follows))) = COLOR_KEYS.iter().find(|(k, _)| *k == key) {
        return effective(theme, own, follows);
    }
    default_color(theme, key).to_string()
}

fn rgb(hex: &str) -> [u8; 3] {
    let c = parse_hex(hex).unwrap_or(egui::Color32::GRAY);
    [c.r(), c.g(), c.b()]
}

pub fn show(ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) {
    ui::title(ui, th, "Appearance", Some("The theme and colors apply to the overlays and this window."));
    let g = env.store.config.general.clone();
    let own: HashMap<String, String> = g.colors.get(&g.theme).cloned().unwrap_or_default();
    let theme_key = g.theme.key();

    ui::section(ui, th, "Theme", |ui| {
        ui::field(
            ui,
            th,
            "Theme",
            Some("Light panels can be easier to read over bright tracks and daytime skies. Box Box dresses the panels up like a TV timing tower."),
            |ui| {
                let labels: Vec<String> = ThemeId::ALL.iter().map(|t| theme_label(*t).to_string()).collect();
                let on = ThemeId::ALL.iter().position(|t| *t == g.theme);
                if let Some(i) = ui::segmented(ui, th, &labels, on) {
                    ed.set(env.store, &["general", "theme"], json!(ThemeId::ALL[i].key()));
                }
            },
        );
    });

    for (title, colors) in GROUPS {
        ui::section(ui, th, title, |ui| {
            for spec in *colors {
                let custom = own.contains_key(spec.key);
                let value = effective(g.theme, &own, spec.key);
                let follows = COLOR_KEYS.iter().find(|(k, _)| *k == spec.key).and_then(|(_, f)| *f).filter(|_| !custom);
                let mut help: Vec<String> = Vec::new();
                if let Some(h) = spec.help {
                    help.push(h.to_string());
                }
                if let Some(f) = follows {
                    help.push(format!("Matches {} until you change it", label_of(f).to_lowercase()));
                }
                let help = (!help.is_empty()).then(|| help.join(". "));
                ui::field(ui, th, spec.label, help.as_deref(), |ui| {
                    // Right to left: the hex, the swatch, then Default.
                    ui.add_sized([64.0, 22.0], egui::Label::new(ui::text(th, value.to_uppercase(), 13.0, th.ink3)));
                    let mut c3 = rgb(&value);
                    if ui.color_edit_button_srgb(&mut c3).changed() {
                        let hex = format!("#{:02x}{:02x}{:02x}", c3[0], c3[1], c3[2]);
                        set_color(ed, env, theme_key, &own, spec.key, Some(hex), g.theme);
                    }
                    // Hidden, but keeps its place, when there's nothing to reset.
                    if custom {
                        if ui::button(ui, th, "Default", Look::Quiet).clicked() {
                            set_color(ed, env, theme_key, &own, spec.key, None, g.theme);
                        }
                    } else {
                        ui.add_space(70.0);
                    }
                });
            }
        });
    }

    ui::section(ui, th, "Reset", |ui| {
        let changed = own.len();
        let help = if changed > 0 {
            format!("{changed} color{} changed. Other themes keep their own.", if changed == 1 { "" } else { "s" })
        } else {
            "Nothing changed yet.".to_string()
        };
        ui::field(ui, th, &format!("Reset the {} theme's colors", theme_label(g.theme)), Some(&help), |ui| {
            if ui::button_on(ui, th, "Reset colors", Look::Danger, changed > 0).clicked() {
                ed.set(env.store, &["general", "colors", theme_key], json!({}));
            }
        });
    });
}

/// Sets or clears (None, or the theme's own default) one colour of the current theme.
fn set_color(ed: &mut Editor, env: &mut Env<'_>, theme_key: &str, own: &HashMap<String, String>, key: &str, value: Option<String>, theme: ThemeId) {
    let mut next: serde_json::Map<String, Value> = own.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    match value {
        Some(v) if !v.eq_ignore_ascii_case(default_color(theme, key)) => {
            next.insert(key.to_string(), json!(v));
        }
        _ => {
            next.remove(key);
        }
    }
    ed.set(env.store, &["general", "colors", theme_key], Value::Object(next));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_until_changed() {
        let mut own = HashMap::new();
        // Throttle follows gain; brake follows loss.
        assert_eq!(effective(ThemeId::Dark, &own, "throttle"), "#4be08f");
        own.insert("gain".into(), "#00ff00".into());
        assert_eq!(effective(ThemeId::Dark, &own, "throttle"), "#00ff00");
        own.insert("throttle".into(), "#123456".into());
        assert_eq!(effective(ThemeId::Dark, &own, "throttle"), "#123456");
        // Anything that isn't a colour is ignored.
        own.insert("ink".into(), "red".into());
        assert_eq!(effective(ThemeId::Light, &own, "ink"), "#141c25");
    }

    #[test]
    fn every_colour_key_has_a_row() {
        let rows: Vec<&str> = GROUPS.iter().flat_map(|(_, cs)| cs.iter().map(|c| c.key)).collect();
        for (k, _) in COLOR_KEYS {
            assert!(rows.contains(k), "{k}");
        }
        assert_eq!(rows.len(), COLOR_KEYS.len());
    }
}
