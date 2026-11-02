//! Every action a hotkey or controller button can run, and how bindings are
//! saved and shown: `src/lib/controls.ts`. The Settings window edits these
//! and stores them in `general.controls` exactly as the Tauri app does
//! (`{ key: { code, ctrl, shift, alt, meta } | null, button: { device, name, button } | null }`),
//! so both apps read each other's bindings.
//!
//! The service that registers the hotkeys and reads wheel buttons is not
//! here: it takes the same JSON.

use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Overlay,
    Layouts,
    PitStop,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Overlay => "Overlay",
            Group::Layouts => "Layouts",
            Group::PitStop => "Pit stop",
        }
    }
}

pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub group: Group,
    /// Ctrl+Shift + this key by default.
    pub key: &'static str,
}

const fn a(id: &'static str, label: &'static str, group: Group, key: &'static str) -> Action {
    Action { id, label, group, key }
}

pub const ACTIONS: &[Action] = &[
    a("overlay.edit", "Edit layout", Group::Overlay, "KeyE"),
    a("overlay.toggle", "Show or hide overlays", Group::Overlay, "KeyH"),
    a("layout.next", "Next layout", Group::Layouts, "KeyL"),
    a("layout.1", "Layout 1", Group::Layouts, "Digit1"),
    a("layout.2", "Layout 2", Group::Layouts, "Digit2"),
    a("layout.3", "Layout 3", Group::Layouts, "Digit3"),
    a("layout.4", "Layout 4", Group::Layouts, "Digit4"),
    a("layout.5", "Layout 5", Group::Layouts, "Digit5"),
    a("layout.6", "Layout 6", Group::Layouts, "Digit6"),
    a("layout.7", "Layout 7", Group::Layouts, "Digit7"),
    a("layout.8", "Layout 8", Group::Layouts, "Digit8"),
    a("layout.9", "Layout 9", Group::Layouts, "Digit9"),
    a("pit.fuel", "Fuel: set the plan, or untick", Group::PitStop, "KeyF"),
    a("pit.fuelUp", "More fuel", Group::PitStop, "ArrowUp"),
    a("pit.fuelDown", "Less fuel", Group::PitStop, "ArrowDown"),
    a("pit.autoFuel", "Auto fuel to finish: on or off", Group::PitStop, "KeyA"),
    a("pit.tires", "Four tires, or none", Group::PitStop, "KeyT"),
    a("pit.fastRepair", "Fast repair", Group::PitStop, "KeyR"),
    a("pit.tearoff", "Tear-off", Group::PitStop, "KeyW"),
    a("pit.clear", "Untick everything", Group::PitStop, "KeyX"),
];

pub const GROUPS: [Group; 3] = [Group::Overlay, Group::Layouts, Group::PitStop];

/// A keyboard shortcut. `code` is the physical key, as in `KeyboardEvent.code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub code: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

/// A button on a wheel, button box or gamepad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonBinding {
    pub device: String,
    pub name: String,
    pub button: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub key: Option<KeyBinding>,
    pub button: Option<ButtonBinding>,
}

impl KeyBinding {
    pub fn to_json(&self) -> Value {
        json!({ "code": self.code, "ctrl": self.ctrl, "shift": self.shift, "alt": self.alt, "meta": self.meta })
    }
    fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            code: v.get("code")?.as_str()?.to_string(),
            ctrl: v.get("ctrl").and_then(Value::as_bool).unwrap_or(false),
            shift: v.get("shift").and_then(Value::as_bool).unwrap_or(false),
            alt: v.get("alt").and_then(Value::as_bool).unwrap_or(false),
            meta: v.get("meta").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}

impl ButtonBinding {
    pub fn to_json(&self) -> Value {
        json!({ "device": self.device, "name": self.name, "button": self.button })
    }
    fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            device: v.get("device")?.as_str()?.to_string(),
            name: v.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            button: v.get("button")?.as_i64()?,
        })
    }
}

impl Binding {
    pub fn to_json(&self) -> Value {
        json!({ "key": self.key.as_ref().map(KeyBinding::to_json), "button": self.button.as_ref().map(ButtonBinding::to_json) })
    }

    pub fn default_for(action: &Action) -> Self {
        Self { key: Some(KeyBinding { code: action.key.into(), ctrl: true, shift: true, alt: false, meta: false }), button: None }
    }
}

/// `general.controls` as saved: a binding per action, the default for any that's missing or broken.
pub fn from_json(controls: &Value) -> Vec<(&'static Action, Binding)> {
    ACTIONS
        .iter()
        .map(|a| {
            let b = controls
                .get(a.id)
                .filter(|c| c.is_object())
                .map(|c| Binding {
                    key: c.get("key").and_then(KeyBinding::from_json),
                    button: c.get("button").and_then(ButtonBinding::from_json),
                })
                .unwrap_or_else(|| Binding::default_for(a));
            (a, b)
        })
        .collect()
}

/// The default bindings as `general.controls` JSON.
pub fn defaults_json() -> Value {
    Value::Object(ACTIONS.iter().map(|a| (a.id.to_string(), Binding::default_for(a).to_json())).collect())
}

/// "KeyF" to "F", "Digit1" to "1", "Numpad4" to "Num 4", "ArrowUp" to an arrow.
pub fn key_name(code: &str) -> String {
    let named = match code {
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
        "Space" => "Space",
        "Enter" => "Enter",
        "Backspace" => "Backspace",
        "Tab" => "Tab",
        "Escape" => "Esc",
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Semicolon" => ";",
        "Quote" => "'",
        "Backquote" => "`",
        "Backslash" => "\\",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "Insert" => "Insert",
        "Delete" => "Delete",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "Page up",
        "PageDown" => "Page down",
        "NumpadAdd" => "Num +",
        "NumpadSubtract" => "Num -",
        "NumpadMultiply" => "Num *",
        "NumpadDivide" => "Num /",
        "NumpadEnter" => "Num Enter",
        "NumpadDecimal" => "Num .",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    if let Some(rest) = code.strip_prefix("Key") {
        return rest.into();
    }
    if let Some(rest) = code.strip_prefix("Digit") {
        return rest.into();
    }
    if let Some(rest) = code.strip_prefix("Numpad") {
        return format!("Num {rest}");
    }
    code.into()
}

/// "Ctrl+Shift+F"
pub fn key_label(k: &KeyBinding) -> String {
    let mut parts: Vec<String> = Vec::new();
    if k.ctrl {
        parts.push("Ctrl".into());
    }
    if k.alt {
        parts.push("Alt".into());
    }
    if k.shift {
        parts.push("Shift".into());
    }
    if k.meta {
        parts.push("Win".into());
    }
    parts.push(key_name(&k.code));
    parts.join("+")
}

/// Button numbers past real buttons stand for the four directions of a hat.
pub fn hat_name(button: i64) -> Option<&'static str> {
    match button {
        1001 => Some("Hat up"),
        1002 => Some("Hat right"),
        1003 => Some("Hat down"),
        1004 => Some("Hat left"),
        _ => None,
    }
}

/// "Button 12" or "Hat up"
pub fn button_label(b: &ButtonBinding) -> String {
    hat_name(b.button).map(String::from).unwrap_or_else(|| format!("Button {}", b.button))
}

/// The full name of whatever runs an action: its controller button, else its key.
pub fn full_label(b: &Binding) -> Option<String> {
    b.button.as_ref().map(button_label).or_else(|| b.key.as_ref().map(key_label))
}

/// A key with no modifier, other than F13 to F24 (which keyboards don't have, so button boxes use them).
pub fn bare_key(k: &KeyBinding) -> bool {
    let f_high = k.code.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()).is_some_and(|n| (13..=24).contains(&n));
    !k.ctrl && !k.alt && !k.shift && !k.meta && !f_high
}

/// Clashes and other things worth knowing about a binding.
pub fn warning(id: &str, all: &[(&'static Action, Binding)], connected: Option<&[String]>) -> Option<String> {
    let b = &all.iter().find(|(a, _)| a.id == id)?.1;
    let others = || all.iter().filter(|(a, _)| a.id != id);
    let key_clash = b.key.as_ref().and_then(|k| others().find(|(_, o)| o.key.as_ref() == Some(k)));
    let button_clash = b
        .button
        .as_ref()
        .and_then(|k| others().find(|(_, o)| o.button.as_ref().is_some_and(|ob| ob.device == k.device && ob.button == k.button)));
    // Two boxes of one model get "<model>#<instance>" ids; a binding to the model works on either.
    let base = |d: &str| d.split('#').next().unwrap_or(d).to_string();
    let unplugged = b.button.as_ref().zip(connected).is_some_and(|(bb, list)| {
        !list.iter().any(|c| *c == bb.device || (base(c) == base(&bb.device) && (!c.contains('#') || !bb.device.contains('#'))))
    });
    let mut notes: Vec<String> = Vec::new();
    if let Some((a, _)) = key_clash {
        notes.push(format!("Same key as {}", a.label.to_lowercase()));
    }
    if let Some((a, _)) = button_clash {
        notes.push(format!("Same button as {}", a.label.to_lowercase()));
    }
    if b.key.as_ref().is_some_and(bare_key) {
        notes.push("Without Ctrl, Alt or Shift, this key stops working in other programs while Tenths runs".into());
    }
    if let Some(bb) = &b.button {
        notes.push(format!("On {}{}", bb.name, if unplugged { ", not plugged in" } else { "" }));
    }
    (!notes.is_empty()).then(|| notes.join(". "))
}

/// The `KeyboardEvent.code` of an egui key, for the keys a shortcut can use.
pub fn key_code(key: egui::Key) -> Option<String> {
    use egui::Key as K;
    let s = match key {
        K::ArrowDown => "ArrowDown",
        K::ArrowLeft => "ArrowLeft",
        K::ArrowRight => "ArrowRight",
        K::ArrowUp => "ArrowUp",
        K::Tab => "Tab",
        K::Backspace => "Backspace",
        K::Enter => "Enter",
        K::Space => "Space",
        K::Insert => "Insert",
        K::Delete => "Delete",
        K::Home => "Home",
        K::End => "End",
        K::PageUp => "PageUp",
        K::PageDown => "PageDown",
        K::Comma => "Comma",
        K::Backslash => "Backslash",
        K::Slash => "Slash",
        K::OpenBracket => "BracketLeft",
        K::CloseBracket => "BracketRight",
        K::Backtick => "Backquote",
        K::Minus => "Minus",
        K::Period => "Period",
        K::Equals => "Equal",
        K::Semicolon => "Semicolon",
        K::Quote => "Quote",
        K::Num0 => "Digit0",
        K::Num1 => "Digit1",
        K::Num2 => "Digit2",
        K::Num3 => "Digit3",
        K::Num4 => "Digit4",
        K::Num5 => "Digit5",
        K::Num6 => "Digit6",
        K::Num7 => "Digit7",
        K::Num8 => "Digit8",
        K::Num9 => "Digit9",
        // Letters and function keys have their own names: "KeyA", "F5".
        other => {
            let n = other.name();
            if n.len() == 1 && n.as_bytes()[0].is_ascii_uppercase() {
                return Some(format!("Key{n}"));
            }
            if n.starts_with('F') && n[1..].parse::<u32>().is_ok_and(|v| (1..=24).contains(&v)) {
                return Some(n.to_string());
            }
            return None;
        }
    };
    Some(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_ctrl_shift_and_round_trip() {
        let json = defaults_json();
        let all = from_json(&json);
        assert_eq!(all.len(), 20);
        let edit = &all[0].1;
        assert_eq!(key_label(edit.key.as_ref().unwrap()), "Ctrl+Shift+E");
        assert_eq!(json["overlay.edit"]["key"]["code"], "KeyE");
        assert_eq!(json["overlay.edit"]["button"], Value::Null);
        assert_eq!(all[3].1.key.as_ref().unwrap().code, "Digit1");
    }

    #[test]
    fn names_and_labels() {
        assert_eq!(key_name("KeyF"), "F");
        assert_eq!(key_name("Digit1"), "1");
        assert_eq!(key_name("Numpad4"), "Num 4");
        assert_eq!(key_name("ArrowUp"), "↑");
        assert_eq!(key_name("F13"), "F13");
        let k = KeyBinding { code: "KeyX".into(), ctrl: true, shift: true, alt: true, meta: true };
        assert_eq!(key_label(&k), "Ctrl+Alt+Shift+Win+X");
        let b = ButtonBinding { device: "VID_1&PID_2".into(), name: "Wheel".into(), button: 1001 };
        assert_eq!(button_label(&b), "Hat up");
        assert_eq!(button_label(&ButtonBinding { button: 12, ..b.clone() }), "Button 12");
        // A controller button is what you'd press while driving, so it's named first.
        assert_eq!(full_label(&Binding { key: Some(k), button: Some(b) }).as_deref(), Some("Hat up"));
        assert_eq!(full_label(&Binding::default()), None);
    }

    #[test]
    fn broken_entries_fall_back_to_the_default() {
        let all = from_json(&json!({ "overlay.edit": "nonsense", "overlay.toggle": { "key": null, "button": null } }));
        assert_eq!(all[0].1.key.as_ref().unwrap().code, "KeyE");
        assert_eq!(all[1].1, Binding::default());
    }

    #[test]
    fn bare_keys_and_warnings() {
        let bare = |code: &str| KeyBinding { code: code.into(), ctrl: false, shift: false, alt: false, meta: false };
        assert!(bare_key(&bare("KeyF")));
        assert!(bare_key(&bare("F12")));
        assert!(!bare_key(&bare("F13")));
        assert!(!bare_key(&bare("F24")));
        let mut all = from_json(&defaults_json());
        assert_eq!(warning("overlay.edit", &all, None), None);
        // Give the toggle the edit key and a button.
        all[1].1 = all[0].1.clone();
        let w = warning("overlay.toggle", &all, None).unwrap();
        assert!(w.starts_with("Same key as edit layout"), "{w}");
        all[0].1.key = Some(bare("KeyQ"));
        all[0].1.button = Some(ButtonBinding { device: "VID_A".into(), name: "Box".into(), button: 3 });
        let w = warning("overlay.edit", &all, Some(&[])).unwrap();
        assert!(w.contains("Without Ctrl") && w.ends_with("On Box, not plugged in"), "{w}");
        let w = warning("overlay.edit", &all, Some(&["VID_A#2".to_string()])).unwrap();
        assert!(w.ends_with("On Box"), "{w}");
    }

    #[test]
    fn egui_keys_become_codes() {
        assert_eq!(key_code(egui::Key::A).as_deref(), Some("KeyA"));
        assert_eq!(key_code(egui::Key::Num7).as_deref(), Some("Digit7"));
        assert_eq!(key_code(egui::Key::F13).as_deref(), Some("F13"));
        assert_eq!(key_code(egui::Key::ArrowUp).as_deref(), Some("ArrowUp"));
        assert_eq!(key_code(egui::Key::Equals).as_deref(), Some("Equal"));
        assert_eq!(key_code(egui::Key::Copy), None);
    }
}
