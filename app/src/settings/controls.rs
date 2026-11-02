//! Controls: every action with a key and a controller button each. Click a
//! box, then press the key or the button on your wheel. Esc cancels
//! (`ControlsPane` in Settings.tsx).
//!
//! Key capture is done here from egui's key events. Wheel buttons come from
//! the controls service through `SettingsWindow::feed_button`.

use std::time::{Duration, Instant};

use egui::{Sense, Stroke, StrokeKind, Ui, Vec2};
use serde_json::Value;

use crate::logic::settings_bindings::{self as bind, Binding, ButtonBinding, KeyBinding};
use crate::theme::Theme;

use super::edits::Editor;
use super::ui::{self, Look};
use super::{ControllerInfo, Env};

/// How long a box waits for a key or button before giving up.
const CAPTURE: Duration = Duration::from_secs(15);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Key,
    Button,
}

struct Capture {
    action: usize,
    kind: Kind,
    since: Instant,
}

#[derive(Default)]
pub struct State {
    capture: Option<Capture>,
    pub controllers: Option<Vec<ControllerInfo>>,
    pub pending_button: Option<ButtonBinding>,
}

impl State {
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }
    pub fn cancel(&mut self) {
        self.capture = None;
    }

    pub fn show(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) {
        ui::title(
            ui,
            th,
            "Controls",
            Some("A key and a controller button for every action. Click a box, then press the key or the button on your wheel. Esc cancels."),
        );
        let all = bind::from_json(&env.store.config.general.controls);

        // Waiting for something: stop after a while, or when the window loses focus,
        // so the hotkeys are never left off.
        if let Some(c) = &self.capture {
            if c.since.elapsed() > CAPTURE || !ui.ctx().input(|i| i.focused) {
                self.capture = None;
            }
        }
        if let Some(c) = &self.capture {
            let (action, kind) = (c.action, c.kind);
            let keys = ui.ctx().input(|i| i.events.clone());
            for e in keys {
                let egui::Event::Key { key, physical_key, pressed: true, modifiers, repeat: false } = e else { continue };
                if key == egui::Key::Escape {
                    self.capture = None;
                    break;
                }
                if kind != Kind::Key {
                    continue;
                }
                let Some(code) = bind::key_code(physical_key.unwrap_or(key)) else { continue };
                let k = KeyBinding { code, ctrl: modifiers.ctrl, shift: modifiers.shift, alt: modifiers.alt, meta: modifiers.mac_cmd };
                set(ed, env, &all, action, |b| b.key = Some(k.clone()));
                self.capture = None;
                break;
            }
        }
        // A wheel button pressed while waiting for one.
        if let Some(press) = self.pending_button.take() {
            if let Some(Capture { action, kind: Kind::Button, .. }) = self.capture {
                set(ed, env, &all, action, |b| b.button = Some(press.clone()));
                self.capture = None;
            }
        }

        ui::section(ui, th, "Controllers", |ui| {
            let help = if self.controllers.is_some() {
                "Buttons work while iRacing has the focus. Plug a controller in and come back here to see it."
            } else {
                "Wheel and button-box buttons are read by the controls service, which this window isn't connected to here. Keys work."
            };
            ui::field(ui, th, "Wheels and button boxes", Some(help), |ui| {
                let text = match &self.controllers {
                    None => "Not available".to_string(),
                    Some(l) if l.is_empty() => "None found".to_string(),
                    Some(l) => l.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", "),
                };
                ui.label(ui::text(th, text, 16.0, th.ink2));
            });
        });

        let connected: Option<Vec<String>> = self.controllers.as_ref().map(|l| l.iter().map(|c| c.device.clone()).collect());
        for group in bind::GROUPS {
            ui::section(ui, th, group.title(), |ui| {
                if group == bind::Group::PitStop {
                    ui::note(ui, th, "These work while the Pit stop widget is on and you're in the car.");
                    ui.add_space(8.0);
                }
                for (i, (a, b)) in all.iter().enumerate().filter(|(_, (a, _))| a.group == group) {
                    let warn = bind::warning(a.id, &all, connected.as_deref());
                    let waiting = |c: &Option<Capture>, kind| {
                        c.as_ref().filter(|c| c.action == i && c.kind == kind).map(|c| CAPTURE.as_secs().saturating_sub(c.since.elapsed().as_secs()))
                    };
                    let (wk, wb) = (waiting(&self.capture, Kind::Key), waiting(&self.capture, Kind::Button));
                    ui::field(ui, th, a.label, warn.as_deref(), |ui| {
                        // Right to left: the button box, then the key.
                        let r = bind_box(ui, th, b.button.as_ref().map(bind::button_label).as_deref(), wb, "No button", b.button.as_ref().map(|x| x.name.as_str()));
                        if r.main {
                            self.capture = if wb.is_some() { None } else { Some(Capture { action: i, kind: Kind::Button, since: Instant::now() }) };
                        }
                        if r.clear {
                            set(ed, env, &all, i, |b| b.button = None);
                        }
                        let r = bind_box(ui, th, b.key.as_ref().map(bind::key_label).as_deref(), wk, "No key", None);
                        if r.main {
                            self.capture = if wk.is_some() { None } else { Some(Capture { action: i, kind: Kind::Key, since: Instant::now() }) };
                        }
                        if r.clear {
                            set(ed, env, &all, i, |b| b.key = None);
                        }
                    });
                }
            });
        }

        ui::section(ui, th, "Reset", |ui| {
            ui::field(ui, th, "Restore the default keys", Some("Ctrl+Shift with a letter for each action. Clears every controller button."), |ui| {
                if ui::button(ui, th, "Restore defaults", Look::Normal).clicked() {
                    ed.set(env.store, &["general", "controls"], bind::defaults_json());
                    self.capture = None;
                }
            });
        });
    }
}

/// Changes one action's binding and saves it, with the other half left as it was.
fn set(ed: &mut Editor, env: &mut Env<'_>, all: &[(&'static bind::Action, Binding)], i: usize, change: impl FnOnce(&mut Binding)) {
    let mut b = all[i].1.clone();
    change(&mut b);
    let v: Value = b.to_json();
    ed.set(env.store, &["general", "controls", all[i].0.id], v);
}

struct BoxClicks {
    main: bool,
    clear: bool,
}

/// A binding box (`.bind`): its label, or "No key"; a × to clear it; "Waiting… 12s" while capturing.
fn bind_box(ui: &mut Ui, th: &Theme, label: Option<&str>, waiting: Option<u64>, empty: &str, tip: Option<&str>) -> BoxClicks {
    let mut clicks = BoxClicks { main: false, clear: false };
    let has = label.is_some();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(150.0, 32.0), Sense::hover());
    let clear_w = if has && waiting.is_none() { 26.0 } else { 0.0 };
    let main_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right() - clear_w, rect.bottom()));
    let main = ui.interact(main_rect, ui.scope_id().with(("bind-main", rect.min.x as i32, rect.min.y as i32)), Sense::click());
    let border = if waiting.is_some() { th.attention } else { th.line_strong };
    let p = ui.painter();
    p.rect(rect, 5.0, th.panel_raised, Stroke::new(1.0, border), StrokeKind::Inside);
    let (text, color, font) = match (waiting, label) {
        (Some(s), _) => (format!("Waiting… {s}s"), th.attention, th.semi(16.0)),
        (None, Some(l)) => (l.to_string(), th.ink, th.semi(16.0)),
        (None, None) => (empty.to_string(), th.ink3, th.medium(16.0)),
    };
    let g = p.layout_no_wrap(text, font, color);
    p.with_clip_rect(main_rect.shrink2(Vec2::new(0.0, 0.0))).galley(egui::pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g, color);
    if main.clicked() {
        clicks.main = true;
    }
    let main = match (waiting.is_some(), tip) {
        (true, _) => main.on_hover_text("Esc cancels"),
        (false, Some(t)) => main.on_hover_text(t),
        _ => main,
    };
    let _ = main.on_hover_cursor(egui::CursorIcon::PointingHand);
    if clear_w > 0.0 {
        let clear_rect = egui::Rect::from_min_max(egui::pos2(rect.right() - clear_w, rect.top()), rect.max);
        let r = ui.interact(clear_rect, ui.scope_id().with(("bind-clear", rect.min.x as i32, rect.min.y as i32)), Sense::click());
        let p = ui.painter();
        p.vline(clear_rect.left(), clear_rect.y_range(), Stroke::new(1.0, th.line));
        let c = if r.hovered() { th.ink } else { th.ink3 };
        let g = p.layout_no_wrap("×".into(), th.medium(16.0), c);
        p.galley(clear_rect.center() - g.size() / 2.0, g, c);
        if r.on_hover_text("Clear").on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            clicks.clear = true;
        }
    }
    clicks
}
