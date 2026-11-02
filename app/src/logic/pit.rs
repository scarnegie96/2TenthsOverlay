//! The pit service maths from `src/lib/pit.ts`: the fuel plan, the stepped
//! fuel amount and what auto fuel should send, plus the key hints from
//! `src/lib/controls.ts`. Sending the commands themselves is not ported yet
//! (it needs the controls service), but the display uses all of the rest.

use serde_json::Value;
use telemetry::model::{FuelPayload, PitPayload};

/// What the fuel hotkey adds (`pitstop.fuelMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuelMode {
    Finish,
    Full,
    Manual,
}

impl FuelMode {
    pub fn parse(s: &str) -> Self {
        match s {
            "full" => Self::Full,
            "manual" => Self::Manual,
            _ => Self::Finish,
        }
    }
}

/// Litres needed to finish beyond what's in the tank, with the safety margin (negative = spare).
pub fn fuel_to_finish(f: Option<&FuelPayload>, margin_laps: f32) -> Option<f32> {
    let f = f?;
    Some(f.to_finish? + margin_laps * f.per_lap?)
}

/// Whole litres to add for a fuel plan: 0 when there's enough, None when it
/// isn't known yet or the plan is to keep the amount set in iRacing.
pub fn fuel_plan(mode: FuelMode, f: Option<&FuelPayload>, margin_laps: f32) -> Option<i32> {
    if mode == FuelMode::Manual {
        return None;
    }
    let fp = f?;
    let need = if mode == FuelMode::Full { Some(fp.max - fp.level) } else { fuel_to_finish(f, margin_laps) }?;
    Some(need.ceil().max(0.0) as i32)
}

/// Whole litres after one press of the fuel up or down hotkey, kept between empty and a full tank.
#[allow(dead_code)] // for the pit commands, once the controls service is ported
pub fn stepped_fuel(from: f32, delta: f32, tank_max: f32) -> f32 {
    let next = ((from + 0.5).floor() + delta).max(0.0);
    if tank_max > 0.0 {
        next.min(tank_max.ceil())
    } else {
        next
    }
}

/// What auto fuel sends.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)] // for the pit commands, once the controls service is ported
pub enum AutoFuel {
    Fuel(f32),
    ClearFuel,
}

/// Auto fuel only re-sends once the plan is this far (litres) from what's set.
pub const AUTO_FUEL_TOLERANCE: f32 = 1.0;

/// What auto fuel should send now, if anything: the amount to finish, ticked,
/// when it's a litre or more from what iRacing has; untick fuel when nothing
/// more is needed. None to leave it alone.
#[allow(dead_code)] // for the pit commands, once the controls service is ported
pub fn auto_fuel_command(pit: &PitPayload, need: Option<f32>, tank_max: f32) -> Option<AutoFuel> {
    let need = need?;
    if need <= 0.0 {
        return pit.fuel.then_some(AutoFuel::ClearFuel);
    }
    // iRacing won't set more than a tankful, so asking for more (a race that
    // needs another stop after this one) would never match and re-send forever.
    let litres = if tank_max > 0.0 { need.min(tank_max.floor()) } else { need };
    if pit.fuel && (pit.fuel_amount - litres).abs() < AUTO_FUEL_TOLERANCE {
        return None;
    }
    Some(AutoFuel::Fuel(litres))
}

// --- Key hints (controls.ts) -----------------------------------------------

/// The default key for each pit action: Ctrl+Shift and this.
pub fn default_key(action: &str) -> Option<&'static str> {
    Some(match action {
        "pit.fuel" => "KeyF",
        "pit.fuelUp" => "ArrowUp",
        "pit.fuelDown" => "ArrowDown",
        "pit.autoFuel" => "KeyA",
        "pit.tires" => "KeyT",
        "pit.fastRepair" => "KeyR",
        "pit.tearoff" => "KeyW",
        "pit.clear" => "KeyX",
        _ => return None,
    })
}

/// "KeyF" -> "F", "Digit1" -> "1", "Numpad4" -> "Num 4", "ArrowUp" -> "↑".
pub fn key_name(code: &str) -> String {
    let named = match code {
        "ArrowUp" => "↑",
        "ArrowDown" => "↓",
        "ArrowLeft" => "←",
        "ArrowRight" => "→",
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
    if let Some(k) = code.strip_prefix("Key") {
        return k.into();
    }
    if let Some(k) = code.strip_prefix("Digit") {
        return k.into();
    }
    if let Some(k) = code.strip_prefix("Numpad") {
        return format!("Num {k}");
    }
    code.into()
}

fn hat(button: i64) -> Option<(&'static str, &'static str)> {
    Some(match button {
        1001 => ("Hat up", "Hat ↑"),
        1002 => ("Hat right", "Hat →"),
        1003 => ("Hat down", "Hat ↓"),
        1004 => ("Hat left", "Hat ←"),
        _ => return None,
    })
}

/// The binding for an action: what's saved in `general.controls`, else the default key.
struct Binding<'a> {
    key: Option<&'a Value>,
    button: Option<i64>,
    default_code: Option<&'static str>,
}

fn binding<'a>(controls: &'a Value, action: &str) -> Binding<'a> {
    let saved = controls.get(action).filter(|c| c.is_object());
    match saved {
        Some(c) => Binding {
            key: c.get("key").filter(|k| k.is_object()),
            button: c.get("button").filter(|b| b.is_object()).and_then(|b| b.get("button")).and_then(Value::as_i64),
            default_code: None,
        },
        None => Binding { key: None, button: None, default_code: default_key(action) },
    }
}

/// A short hint for a widget: the controller button if there is one (that's
/// what you'd press while driving), otherwise the key without its modifiers.
pub fn short_label(controls: &Value, action: &str) -> Option<String> {
    let b = binding(controls, action);
    if let Some(n) = b.button {
        return Some(hat(n).map(|h| h.1.to_string()).unwrap_or_else(|| format!("B{n}")));
    }
    if let Some(code) = b.key.and_then(|k| k.get("code")).and_then(Value::as_str) {
        return Some(key_name(code));
    }
    b.default_code.map(key_name)
}

/// The full name of whatever runs an action: "Ctrl+Shift+X", "Button 12" or "Hat up".
pub fn full_label(controls: &Value, action: &str) -> Option<String> {
    let b = binding(controls, action);
    if let Some(n) = b.button {
        return Some(hat(n).map(|h| h.0.to_string()).unwrap_or_else(|| format!("Button {n}")));
    }
    if let Some(k) = b.key {
        let flag = |name: &str| k.get(name).and_then(Value::as_bool).unwrap_or(false);
        let code = k.get("code").and_then(Value::as_str)?;
        let mut parts: Vec<String> = Vec::new();
        for (name, label) in [("ctrl", "Ctrl"), ("alt", "Alt"), ("shift", "Shift"), ("meta", "Win")] {
            if flag(name) {
                parts.push(label.into());
            }
        }
        parts.push(key_name(code));
        return Some(parts.join("+"));
    }
    b.default_code.map(|c| format!("Ctrl+Shift+{}", key_name(c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn menu(fuel: bool, amount: f32) -> PitPayload {
        PitPayload { fuel, fuel_amount: amount, ..Default::default() }
    }

    /// The cases of `scripts/check-pit.ts`.
    #[test]
    fn auto_fuel_to_finish() {
        use AutoFuel::{ClearFuel, Fuel};
        let cases: [(PitPayload, Option<f32>, Option<AutoFuel>); 7] = [
            (menu(false, 0.0), None, None),                   // no plan yet: leave it
            (menu(false, 0.0), Some(28.0), Some(Fuel(28.0))), // tick it with the amount
            (menu(true, 27.6), Some(28.0), None),             // within a litre: leave it
            (menu(true, 20.0), Some(28.0), Some(Fuel(28.0))), // a litre or more off: update
            (menu(true, 35.0), Some(28.0), Some(Fuel(28.0))), // too much: bring it down
            (menu(true, 10.0), Some(0.0), Some(ClearFuel)),   // enough already: untick
            (menu(false, 10.0), Some(0.0), None),             // already unticked
        ];
        for (pit, need, want) in cases {
            assert_eq!(auto_fuel_command(&pit, need, 0.0), want, "fuel {} {}, need {need:?}", pit.fuel, pit.fuel_amount);
        }
    }

    #[test]
    fn auto_fuel_caps_at_a_tankful() {
        // More than a tankful needed (another stop to come): set a tankful, then leave it.
        assert_eq!(auto_fuel_command(&menu(false, 0.0), Some(140.0), 110.4), Some(AutoFuel::Fuel(110.0)));
        assert_eq!(auto_fuel_command(&menu(true, 110.0), Some(140.0), 110.4), None);
    }

    fn fuel(level: f32, max: f32, per_lap: Option<f32>, to_finish: Option<f32>) -> FuelPayload {
        FuelPayload { level, max, per_lap, to_finish, ..Default::default() }
    }

    #[test]
    fn plans() {
        let f = fuel(20.0, 100.0, Some(2.0), Some(10.0));
        // 10 to finish + 0.5 laps of margin (1 L) = 11, rounded up.
        assert_eq!(fuel_to_finish(Some(&f), 0.5), Some(11.0));
        assert_eq!(fuel_plan(FuelMode::Finish, Some(&f), 0.5), Some(11));
        assert_eq!(fuel_plan(FuelMode::Finish, Some(&f), 0.3), Some(11)); // 10.6 -> 11
        assert_eq!(fuel_plan(FuelMode::Full, Some(&f), 0.5), Some(80));
        assert_eq!(fuel_plan(FuelMode::Manual, Some(&f), 0.5), None);
        // Spare fuel: nothing to add.
        let spare = fuel(20.0, 100.0, Some(2.0), Some(-30.0));
        assert_eq!(fuel_plan(FuelMode::Finish, Some(&spare), 0.5), Some(0));
        // Not known yet.
        assert_eq!(fuel_plan(FuelMode::Finish, Some(&fuel(20.0, 100.0, None, None)), 0.5), None);
        assert_eq!(fuel_plan(FuelMode::Finish, None, 0.5), None);
    }

    #[test]
    fn stepping() {
        assert_eq!(stepped_fuel(27.6, 5.0, 100.0), 33.0);
        assert_eq!(stepped_fuel(3.0, -5.0, 100.0), 0.0);
        assert_eq!(stepped_fuel(98.0, 5.0, 100.4), 101.0);
        assert_eq!(stepped_fuel(98.0, 5.0, 0.0), 103.0);
    }

    #[test]
    fn labels() {
        assert_eq!(key_name("KeyF"), "F");
        assert_eq!(key_name("ArrowUp"), "↑");
        assert_eq!(key_name("Numpad4"), "Num 4");
        assert_eq!(key_name("F13"), "F13");
        // Defaults when nothing is saved.
        let none = Value::Null;
        assert_eq!(short_label(&none, "pit.fuelUp").as_deref(), Some("↑"));
        assert_eq!(full_label(&none, "pit.clear").as_deref(), Some("Ctrl+Shift+X"));
        let c = json!({
            "pit.fuel": { "key": { "code": "KeyG", "ctrl": true, "shift": false, "alt": true, "meta": false }, "button": null },
            "pit.tires": { "key": { "code": "KeyT", "ctrl": true, "shift": true }, "button": { "device": "d", "name": "n", "button": 12 } },
            "pit.tearoff": { "key": null, "button": { "device": "d", "name": "n", "button": 1001 } },
            "pit.clear": { "key": null, "button": null },
        });
        assert_eq!(short_label(&c, "pit.fuel").as_deref(), Some("G"));
        assert_eq!(full_label(&c, "pit.fuel").as_deref(), Some("Ctrl+Alt+G"));
        assert_eq!(short_label(&c, "pit.tires").as_deref(), Some("B12"));
        assert_eq!(full_label(&c, "pit.tires").as_deref(), Some("Button 12"));
        assert_eq!(short_label(&c, "pit.tearoff").as_deref(), Some("Hat ↑"));
        assert_eq!(full_label(&c, "pit.tearoff").as_deref(), Some("Hat up"));
        // Unbound.
        assert_eq!(short_label(&c, "pit.clear"), None);
        // Not saved: the default.
        assert_eq!(short_label(&c, "pit.fuelDown").as_deref(), Some("↓"));
    }
}
