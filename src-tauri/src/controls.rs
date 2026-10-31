//! Hotkeys and controller buttons: which key or button runs which action.
//!
//! The bindings live in the front end's config (`src/lib/controls.ts`); the
//! overlay sends them here whenever they change. Only the shortcuts that
//! should be live right now are registered with Windows, so pit keys don't
//! take Ctrl+Shift+T and friends from other programs while you're not in the
//! car, and nothing fires while the settings window is waiting for a key.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

#[derive(Debug, Clone, Deserialize)]
pub struct KeyBinding {
    /// KeyboardEvent.code, like "KeyF" or "ArrowUp".
    pub code: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub meta: bool,
}

impl KeyBinding {
    fn shortcut(&self) -> Option<Shortcut> {
        let code = Code::from_str(&self.code).ok()?;
        let mut mods = Modifiers::empty();
        for (on, m) in [(self.ctrl, Modifiers::CONTROL), (self.shift, Modifiers::SHIFT), (self.alt, Modifiers::ALT), (self.meta, Modifiers::SUPER)] {
            if on {
                mods |= m;
            }
        }
        Some(Shortcut::new((!mods.is_empty()).then_some(mods), code))
    }

    /// "Ctrl+Shift+E", for the tray menu. Matches `keyLabel` in the front end for letters and digits.
    fn label(&self) -> String {
        let key = self
            .code
            .strip_prefix("Key")
            .or_else(|| self.code.strip_prefix("Digit"))
            .unwrap_or(&self.code);
        [(self.ctrl, "Ctrl"), (self.alt, "Alt"), (self.shift, "Shift"), (self.meta, "Win")]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, name)| *name)
            .chain(std::iter::once(key))
            .collect::<Vec<_>>()
            .join("+")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ButtonBinding {
    pub device: String,
    pub button: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Binding {
    pub key: Option<KeyBinding>,
    pub button: Option<ButtonBinding>,
}

/// A button pressed on a controller. Sent to the settings window so it can
/// learn a button, and matched against the bindings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonPress {
    pub device: String,
    pub name: String,
    pub button: u32,
}

#[derive(Default)]
pub struct Controls {
    /// Action id ("overlay.edit", "pit.fuel"...) to its key and button.
    bindings: Mutex<HashMap<String, Binding>>,
    /// The shortcuts registered with Windows, and the actions each one runs.
    registered: Mutex<Vec<(Shortcut, Vec<String>)>>,
    /// The Pit stop widget is on and you're in the car.
    pit_active: AtomicBool,
    /// The settings window is waiting for a key or button.
    suspended: AtomicBool,
    /// Tray menu items that show their shortcut: (action, item, label without the shortcut).
    tray: Mutex<Vec<(&'static str, MenuItem<Wry>, &'static str)>>,
}

impl Controls {
    fn live(&self, action: &str) -> bool {
        !self.suspended.load(Ordering::Relaxed) && (!action.starts_with("pit.") || self.pit_active.load(Ordering::Relaxed))
    }

    pub fn set_tray(&self, items: Vec<(&'static str, MenuItem<Wry>, &'static str)>) {
        *self.tray.lock().unwrap() = items;
    }
}

pub fn set_bindings(app: &AppHandle, bindings: HashMap<String, Binding>) {
    *app.state::<Controls>().bindings.lock().unwrap() = bindings;
    sync(app);
    update_tray(app);
}

pub fn set_pit_active(app: &AppHandle, active: bool) {
    if app.state::<Controls>().pit_active.swap(active, Ordering::Relaxed) != active {
        sync(app);
    }
}

pub fn set_suspended(app: &AppHandle, suspended: bool) {
    if app.state::<Controls>().suspended.swap(suspended, Ordering::Relaxed) != suspended {
        sync(app);
    }
}

/// Registers exactly the shortcuts that should be live now.
fn sync(app: &AppHandle) {
    let controls = app.state::<Controls>();
    let mut wanted: Vec<(Shortcut, Vec<String>)> = Vec::new();
    for (action, binding) in controls.bindings.lock().unwrap().iter() {
        let Some(key) = binding.key.as_ref().filter(|_| controls.live(action)) else { continue };
        let Some(shortcut) = key.shortcut() else {
            eprintln!("[tenths] unknown key {:?} for {action}", key.code);
            continue;
        };
        match wanted.iter_mut().find(|(s, _)| *s == shortcut) {
            Some((_, actions)) => actions.push(action.clone()),
            None => wanted.push((shortcut, vec![action.clone()])),
        }
    }
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    for (shortcut, _) in &wanted {
        if let Err(err) = shortcuts.register(*shortcut) {
            eprintln!("[tenths] hotkey {shortcut}: {err}");
        }
    }
    *controls.registered.lock().unwrap() = wanted;
}

fn update_tray(app: &AppHandle) {
    let controls = app.state::<Controls>();
    let bindings = controls.bindings.lock().unwrap();
    for (action, item, label) in controls.tray.lock().unwrap().iter() {
        let key = bindings.get(*action).and_then(|b| b.key.as_ref()).map(KeyBinding::label);
        let _ = item.set_text(match key {
            Some(key) => format!("{label}   {key}"),
            None => label.to_string(),
        });
    }
}

/// A registered shortcut was pressed.
pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    let actions = app
        .state::<Controls>()
        .registered
        .lock()
        .unwrap()
        .iter()
        .find(|(s, _)| s == shortcut)
        .map(|(_, actions)| actions.clone())
        .unwrap_or_default();
    for action in actions {
        run(app, &action);
    }
}

/// A controller button was pressed (from the controller thread).
pub fn on_button(app: &AppHandle, press: ButtonPress) {
    let _ = app.emit("controller-button", &press);
    let controls = app.state::<Controls>();
    let actions: Vec<String> = controls
        .bindings
        .lock()
        .unwrap()
        .iter()
        .filter(|(action, b)| {
            controls.live(action)
                && b.button.as_ref().is_some_and(|b| b.device == press.device && b.button == press.button)
        })
        .map(|(action, _)| action.clone())
        .collect();
    for action in actions {
        run(app, &action);
    }
}

fn run(app: &AppHandle, action: &str) {
    match action {
        "overlay.edit" => crate::toggle_edit(app),
        "overlay.toggle" => crate::toggle_visible(app),
        "layout.next" => crate::switch_layout(app, Some(1), None),
        _ => {
            if let Some(n) = action.strip_prefix("layout.").and_then(|n| n.parse::<usize>().ok()) {
                crate::switch_layout(app, None, Some(n.saturating_sub(1)));
            } else if let Some(pit) = action.strip_prefix("pit.") {
                let _ = app.emit_to(crate::OVERLAY, "pit-shortcut", pit);
            }
        }
    }
}
