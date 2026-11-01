//! Hotkeys, wheel and button-box buttons, and what they run: the egui port of
//! `src-tauri/src/controls.rs` plus `usePitControl` from `src/lib/pit.ts`.
//!
//! The bindings come from `general.controls` in the config (the same file the
//! Tauri app writes), falling back to Ctrl+Shift+<key> per action as
//! `defaultControls()` in `src/lib/controls.ts`. Only the shortcuts that
//! should be live right now are registered with Windows: pit keys only while
//! the Pit stop widget is on and you're in the car, and nothing while the
//! settings window waits for a key (`set_suspended`).
//!
//! Once a frame, `Controls::frame` picks up changed bindings, collects the
//! keys and buttons pressed, runs pit and layout actions itself, and returns
//! the few that belong to the app window (`AppAction`).

pub mod controllers;
pub mod hotkeys;
pub mod layouts;
pub mod pit;

use std::sync::mpsc::{channel, Receiver};

use serde::Serialize;
use serde_json::{json, Value};
use telemetry::runner::RunnerControl;

use crate::config::{Store, Units};
use crate::data::Live;
use controllers::base_id;
use pit::{Inputs, PitAction, PitControl, Settings};

// For the settings window's controls pane (G5): the controllers plugged in now.
#[allow(unused_imports)]
pub use controllers::{list as list_controllers, Controller};

/// Every action, in the order of `ACTIONS` in `controls.ts`, with the key
/// that goes with Ctrl+Shift by default.
pub const ACTIONS: [(&str, &str); 20] = [
    ("overlay.edit", "KeyE"),
    ("overlay.toggle", "KeyH"),
    ("layout.next", "KeyL"),
    ("layout.1", "Digit1"),
    ("layout.2", "Digit2"),
    ("layout.3", "Digit3"),
    ("layout.4", "Digit4"),
    ("layout.5", "Digit5"),
    ("layout.6", "Digit6"),
    ("layout.7", "Digit7"),
    ("layout.8", "Digit8"),
    ("layout.9", "Digit9"),
    ("pit.fuel", "KeyF"),
    ("pit.fuelUp", "ArrowUp"),
    ("pit.fuelDown", "ArrowDown"),
    ("pit.autoFuel", "KeyA"),
    ("pit.tires", "KeyT"),
    ("pit.fastRepair", "KeyR"),
    ("pit.tearoff", "KeyW"),
    ("pit.clear", "KeyX"),
];

/// Not in `ACTIONS` and not configurable: Ctrl+Shift+Q quits (the egui build's
/// own hotkey, from before the controls existed; the tray will have Quit too).
const QUIT: &str = "app.quit";

/// A keyboard shortcut. `code` is the physical key as in `KeyboardEvent.code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub code: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyBinding {
    fn ctrl_shift(code: &str) -> Self {
        Self { code: code.into(), ctrl: true, shift: true, alt: false, meta: false }
    }

    fn parse(v: &Value) -> Option<Self> {
        let flag = |name: &str| v.get(name).and_then(Value::as_bool).unwrap_or(false);
        Some(Self { code: v.get("code")?.as_str()?.to_string(), ctrl: flag("ctrl"), shift: flag("shift"), alt: flag("alt"), meta: flag("meta") })
    }
}

/// A button on a wheel, button box or gamepad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonBinding {
    /// `vid:pid`-style hardware id, see controllers.rs.
    pub device: String,
    /// From 1, or 1001 to 1004 for the hat's four directions.
    pub button: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    pub key: Option<KeyBinding>,
    pub button: Option<ButtonBinding>,
}

/// A button pressed on a controller: matched against the bindings, and handed
/// to the settings window so it can learn a button.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonPress {
    pub device: String,
    pub name: String,
    pub button: u32,
}

type Bindings = Vec<(&'static str, Binding)>;

/// The bindings in `general.controls`: an action's saved entry when it is
/// well formed (as `normalize` in `config.ts` decides), else its default.
pub fn parse_bindings(controls: &Value) -> Bindings {
    ACTIONS
        .iter()
        .map(|(id, code)| {
            let default = Binding { key: Some(KeyBinding::ctrl_shift(code)), button: None };
            let saved = controls.get(*id).filter(|c| {
                c.is_object() && c.get("key").is_some_and(|k| k.is_null() || k.is_object()) && c.get("button").is_some_and(|b| b.is_null() || b.is_object())
            });
            let binding = match saved {
                Some(c) => Binding {
                    key: c.get("key").and_then(KeyBinding::parse),
                    button: c.get("button").and_then(|b| {
                        Some(ButtonBinding { device: b.get("device")?.as_str()?.to_string(), button: u32::try_from(b.get("button")?.as_u64()?).ok()? })
                    }),
                },
                None => default,
            };
            (*id, binding)
        })
        .collect()
}

/// The actions bound to a button. Bindings to exactly this device win.
/// Otherwise a binding to the same model counts, so bindings made with one
/// box keep working when a second identical one is plugged in (its id then
/// gains an instance part, see controllers.rs), and the other way round.
fn button_actions(bindings: &Bindings, device: &str, button: u32) -> Vec<&'static str> {
    let bound = |same: &dyn Fn(&str) -> bool| -> Vec<&'static str> {
        bindings.iter().filter(|(_, b)| b.button.as_ref().is_some_and(|b| b.button == button && same(&b.device))).map(|(action, _)| *action).collect()
    };
    let exact = bound(&|d| d == device);
    if !exact.is_empty() {
        return exact;
    }
    // Two different boxes of one model (both with an instance part) never match.
    let plain = |id: &str| !id.contains('#');
    bound(&|d| base_id(d) == base_id(device) && (plain(d) || plain(device)))
}

/// What the app window should do (everything else `Controls` runs itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    /// `overlay.edit`
    ToggleEdit,
    /// `overlay.toggle`: show or hide every overlay.
    ToggleVisible,
    Quit,
}

/// What one frame of controls produced.
#[derive(Default)]
pub struct Output {
    pub app: Vec<AppAction>,
    /// Every controller button pressed (even while suspended), for the
    /// settings window to learn from.
    pub presses: Vec<ButtonPress>,
}

pub struct Controls {
    /// `general.controls` as last parsed.
    source: Value,
    bindings: Bindings,
    hotkeys: hotkeys::Hotkeys,
    buttons: Receiver<ButtonPress>,
    /// The Pit stop widget is on and you're in the car.
    pit_active: bool,
    /// The settings window is waiting for a key or button.
    suspended: bool,
    /// What's registered: each shortcut and the actions it runs.
    registered: Vec<(KeyBinding, Vec<&'static str>)>,
    /// Bindings, `pit_active` or `suspended` changed: register again.
    dirty: bool,
    /// Actions asked for by `inject` (`--press`), run next frame.
    queued: Vec<&'static str>,
    pit: PitControl,
    runner: RunnerControl,
}

impl Controls {
    pub fn new(ctx: egui::Context, runner: RunnerControl) -> Self {
        let (tx, buttons) = channel();
        let repaint = ctx.clone();
        controllers::spawn(move |press| {
            let _ = tx.send(press);
            repaint.request_repaint();
        });
        Self {
            source: Value::Null,
            bindings: parse_bindings(&Value::Null),
            hotkeys: hotkeys::Hotkeys::new(ctx),
            buttons,
            pit_active: false,
            suspended: false,
            registered: Vec::new(),
            dirty: true,
            queued: Vec::new(),
            pit: PitControl::default(),
            runner,
        }
    }

    /// Stops every hotkey and button while the settings window waits for a
    /// new one (`suspend_controls`).
    pub fn set_suspended(&mut self, suspended: bool) {
        if self.suspended != suspended {
            self.suspended = suspended;
            self.dirty = true;
        }
    }

    /// Runs an action as if its key had been pressed (`--press`).
    pub fn inject(&mut self, action: &str) {
        match ACTIONS.iter().map(|(id, _)| *id).chain([QUIT]).find(|id| *id == action) {
            Some(id) => self.queued.push(id),
            None => eprintln!("[tenths] unknown action {action:?}"),
        }
    }

    fn live(&self, action: &str) -> bool {
        !self.suspended && (!action.starts_with("pit.") || self.pit_active)
    }

    /// Registers exactly the shortcuts that should be live now.
    fn sync(&mut self) {
        let mut wanted: Vec<(KeyBinding, Vec<&'static str>)> = Vec::new();
        let quit = Binding { key: Some(KeyBinding::ctrl_shift("KeyQ")), button: None };
        let all = self.bindings.iter().map(|(a, b)| (*a, b)).chain([(QUIT, &quit)]);
        for (action, binding) in all {
            let Some(key) = binding.key.as_ref().filter(|_| self.live(action)) else { continue };
            match wanted.iter_mut().find(|(k, _)| k == key) {
                Some((_, actions)) => actions.push(action),
                None => wanted.push((key.clone(), vec![action])),
            }
        }
        let keys: Vec<KeyBinding> = wanted.iter().map(|(k, _)| k.clone()).collect();
        self.hotkeys.set(&keys);
        self.registered = wanted;
        self.dirty = false;
    }

    /// Once a frame: pick up changed bindings, take the keys and buttons
    /// pressed, run what they're bound to, and do the pit service's
    /// background work (auto fuel). `now` is the app's clock in seconds.
    pub fn frame(&mut self, store: &mut Store, live: &Live, units: Units, now: f64) -> Output {
        if store.config.general.controls != self.source {
            self.source = store.config.general.controls.clone();
            self.bindings = parse_bindings(&self.source);
            self.dirty = true;
        }
        // The pit keys work while the Pit stop widget is on and you're in the car.
        let on_track = live.tick.as_ref().is_some_and(|t| t.is_on_track);
        let pit_live = store.config.widget("pitstop").enabled && live.connected() && on_track;
        if pit_live != self.pit_active {
            self.pit_active = pit_live;
            self.dirty = true;
        }
        if self.dirty {
            self.sync();
        }

        let mut actions: Vec<&'static str> = std::mem::take(&mut self.queued);
        for key in self.hotkeys.poll() {
            if let Some((_, a)) = self.registered.iter().find(|(k, _)| *k == key) {
                actions.extend(a.iter().copied());
            }
        }
        let mut out = Output::default();
        while let Ok(press) = self.buttons.try_recv() {
            let bound = button_actions(&self.bindings, &press.device, press.button);
            actions.extend(bound.into_iter().filter(|a| self.live(a)));
            out.presses.push(press);
        }

        for action in actions {
            self.run(action, store, live, units, now, &mut out);
        }
        // Pit commands go to the runner, which is where iRacing's broadcast
        // message or the demo race takes them from.
        let settings = Settings::from_config(&store.config, units);
        let inputs = pit_inputs(settings, live, now);
        self.pit.update(pit_live, &inputs, &mut self.runner);
        pit::publish(&self.pit.fb);
        out
    }

    fn run(&mut self, action: &str, store: &mut Store, live: &Live, units: Units, now: f64, out: &mut Output) {
        match action {
            "overlay.edit" => out.app.push(AppAction::ToggleEdit),
            "overlay.toggle" => out.app.push(AppAction::ToggleVisible),
            QUIT => out.app.push(AppAction::Quit),
            _ => {
                if let Some(switch) = layouts::from_action(action) {
                    layouts::switch_layout(store, switch);
                } else if let Some(pit_action) = PitAction::from_id(action).filter(|_| self.live(action)) {
                    let inputs = pit_inputs(Settings::from_config(&store.config, units), live, now);
                    if let Some(on) = self.pit.run_action(pit_action, &inputs, &mut self.runner) {
                        save_keep_fuel_set(store, on);
                    }
                }
            }
        }
    }
}

fn pit_inputs(settings: Settings, live: &Live, now: f64) -> Inputs<'_> {
    Inputs {
        settings,
        pit: live.field.as_deref().map(|f| &f.pit),
        fuel: live.field.as_deref().map(|f| &f.fuel),
        on_pit_road: live.tick.as_deref().map(|t| t.on_pit_road),
        fields: live.fields,
        now,
    }
}

/// Auto fuel on or off is kept in the Pit stop widget's options, like any other.
fn save_keep_fuel_set(store: &mut Store, on: bool) {
    let mut options = store.config.widget("pitstop").options.clone();
    options.insert("keepFuelSet".into(), json!(on));
    store.save_widget("pitstop", &[("options", Value::Object(options))]);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use telemetry::model::{FieldPayload, SourceKind, StatusPayload, TickPayload};
    use telemetry::pit::PitCommand;

    use super::*;

    const PLAIN: &str = "VID_0EB7&PID_0E04&MI_00";
    const BOX_A: &str = "VID_0EB7&PID_0E04&MI_00#7&2B1C3E&0&0000";
    const BOX_B: &str = "VID_0EB7&PID_0E04&MI_00#7&91AD07&0&0000";

    fn with_buttons(list: &[(&'static str, &str, u32)]) -> Bindings {
        list.iter()
            .map(|(action, device, button)| (*action, Binding { key: None, button: Some(ButtonBinding { device: device.to_string(), button: *button }) }))
            .collect()
    }

    // The cases of controls.rs in the Tauri app.
    #[test]
    fn exact_device_first() {
        let b = with_buttons(&[("a", BOX_A, 5), ("b", BOX_B, 5), ("old", PLAIN, 5)]);
        assert_eq!(button_actions(&b, BOX_A, 5), ["a"]);
        assert_eq!(button_actions(&b, BOX_B, 5), ["b"]);
        assert_eq!(button_actions(&b, PLAIN, 5), ["old"]);
        assert!(button_actions(&b, BOX_A, 6).is_empty());
    }

    #[test]
    fn plain_bindings_work_on_either_twin() {
        let b = with_buttons(&[("old", PLAIN, 5), ("other", "VID_346E&PID_0004", 5)]);
        assert_eq!(button_actions(&b, BOX_A, 5), ["old"]);
        assert_eq!(button_actions(&b, BOX_B, 5), ["old"]);
    }

    #[test]
    fn twin_bindings_work_once_the_other_is_unplugged() {
        let b = with_buttons(&[("a", BOX_A, 5), ("a2", BOX_A, 6)]);
        assert_eq!(button_actions(&b, PLAIN, 5), ["a"]);
        // But a different box of the same model doesn't run box A's actions.
        assert!(button_actions(&b, BOX_B, 5).is_empty());
    }

    #[test]
    fn several_actions_on_one_button() {
        let b = with_buttons(&[("x", PLAIN, 1), ("y", PLAIN, 1)]);
        assert_eq!(button_actions(&b, BOX_A, 1), ["x", "y"]);
    }

    #[test]
    fn defaults_are_ctrl_shift_keys() {
        for controls in [Value::Null, json!({}), json!([1]), json!({ "pit.fuel": "nonsense" })] {
            let b = parse_bindings(&controls);
            assert_eq!(b.len(), 20);
            let edit = &b[0].1;
            assert_eq!(edit.key, Some(KeyBinding::ctrl_shift("KeyE")));
            assert_eq!(edit.button, None);
            let up = b.iter().find(|(a, _)| *a == "pit.fuelUp").unwrap();
            assert_eq!(up.1.key, Some(KeyBinding::ctrl_shift("ArrowUp")));
        }
    }

    #[test]
    fn saved_bindings_replace_the_default_whole() {
        let b = parse_bindings(&json!({
            "overlay.edit": { "key": { "code": "F13", "ctrl": false, "shift": false, "alt": true, "meta": false }, "button": null },
            "overlay.toggle": { "key": null, "button": { "device": PLAIN, "name": "Box", "button": 12 } },
            "pit.fuel": { "key": null, "button": null },
            // Missing `button`: not well formed, so the default stays.
            "pit.tires": { "key": null },
        }));
        let get = |id: &str| b.iter().find(|(a, _)| *a == id).unwrap().1.clone();
        let edit = get("overlay.edit");
        assert_eq!(edit.key, Some(KeyBinding { code: "F13".into(), ctrl: false, shift: false, alt: true, meta: false }));
        // A button replaces the key: it is unbound from the keyboard.
        let toggle = get("overlay.toggle");
        assert_eq!(toggle.key, None);
        assert_eq!(toggle.button, Some(ButtonBinding { device: PLAIN.into(), button: 12 }));
        // Explicitly unbound.
        assert_eq!(get("pit.fuel"), Binding::default());
        assert_eq!(get("pit.tires").key, Some(KeyBinding::ctrl_shift("KeyT")));
    }

    fn live_in_car(on_track: bool) -> Live {
        let field = FieldPayload {
            fuel: telemetry::model::FuelPayload { level: 10.0, max: 100.0, per_lap: Some(2.0), to_finish: Some(18.0), ..Default::default() },
            pit: telemetry::model::PitPayload { available: true, ..Default::default() },
            ..Default::default()
        };
        Live {
            status: Some(Arc::new(StatusPayload { source: SourceKind::None, connected: true, message: String::new() })),
            tick: Some(Arc::new(TickPayload { is_on_track: on_track, ..Default::default() })),
            field: Some(Arc::new(field)),
            fields: 1,
            ..Default::default()
        }
    }

    fn controls() -> (Controls, Store) {
        let c = Controls::new(egui::Context::default(), RunnerControl::default());
        (c, Store::open(None))
    }

    fn queued(c: &Controls) -> Vec<PitCommand> {
        c.runner.pit.lock().unwrap().clone()
    }

    #[test]
    fn pit_keys_only_while_the_widget_is_on_and_you_are_in_the_car() {
        let (mut c, mut store) = controls();
        // Widget off: not live, and the pit key is dropped.
        c.inject("pit.fuel");
        c.frame(&mut store, &live_in_car(true), Units::Metric, 1.0);
        assert!(queued(&c).is_empty());
        assert!(!c.registered.iter().any(|(_, a)| a.contains(&"pit.fuel")));
        assert!(c.registered.iter().any(|(_, a)| a.contains(&"overlay.edit")));

        store.save_widget("pitstop", &[("enabled", json!(true))]);
        // In the garage: still not live.
        c.inject("pit.fuel");
        c.frame(&mut store, &live_in_car(false), Units::Metric, 2.0);
        assert!(queued(&c).is_empty());

        c.frame(&mut store, &live_in_car(true), Units::Metric, 3.0);
        assert!(c.registered.iter().any(|(_, a)| a.contains(&"pit.fuel")), "registered once you're in the car");
        c.inject("pit.fuel");
        c.frame(&mut store, &live_in_car(true), Units::Metric, 4.0);
        // 18 L to finish plus the Fuel widget's default margin of half a lap (1 L).
        assert_eq!(queued(&c), [PitCommand::Fuel { litres: 19 }]);
        assert_eq!(c.pit.fb.sent_text(4.5), Some("Fuel set to 19 L"));
        assert_eq!(c.pit.fb.lit(4.1), Some(PitAction::Fuel));

        // Leaving the car unregisters them again.
        c.frame(&mut store, &live_in_car(false), Units::Metric, 5.0);
        assert!(!c.registered.iter().any(|(_, a)| a.contains(&"pit.fuel")));
    }

    #[test]
    fn suspended_runs_nothing() {
        let (mut c, mut store) = controls();
        c.set_suspended(true);
        c.frame(&mut store, &live_in_car(true), Units::Metric, 1.0);
        assert!(c.registered.is_empty(), "not even the quit key");
        c.set_suspended(false);
        c.frame(&mut store, &live_in_car(true), Units::Metric, 2.0);
        assert!(!c.registered.is_empty());
    }

    #[test]
    fn app_actions() {
        let (mut c, mut store) = controls();
        for a in ["overlay.edit", "overlay.toggle", "app.quit", "layout.next", "layout.3"] {
            c.inject(a);
        }
        c.inject("nonsense");
        let out = c.frame(&mut store, &live_in_car(true), Units::Metric, 1.0);
        // Layout actions go to the layouts hook, not the window.
        assert_eq!(out.app, [AppAction::ToggleEdit, AppAction::ToggleVisible, AppAction::Quit]);
    }

    #[test]
    fn shared_keys_run_every_action_and_changes_are_picked_up() {
        let (mut c, mut store) = controls();
        store.config.general.controls = json!({
            "overlay.edit": { "key": { "code": "KeyZ", "ctrl": true, "shift": false, "alt": false, "meta": false }, "button": null },
            "overlay.toggle": { "key": { "code": "KeyZ", "ctrl": true, "shift": false, "alt": false, "meta": false }, "button": null },
        });
        c.frame(&mut store, &live_in_car(true), Units::Metric, 1.0);
        let z = KeyBinding { code: "KeyZ".into(), ctrl: true, shift: false, alt: false, meta: false };
        let (_, actions) = c.registered.iter().find(|(k, _)| *k == z).unwrap();
        assert_eq!(actions, &["overlay.edit", "overlay.toggle"]);
        // Ctrl+Shift+E no longer does anything.
        assert!(!c.registered.iter().any(|(k, _)| *k == KeyBinding::ctrl_shift("KeyE")));
        // Quit is always there.
        assert!(c.registered.iter().any(|(k, a)| *k == KeyBinding::ctrl_shift("KeyQ") && a == &["app.quit"]));
    }

    #[test]
    fn auto_fuel_key_is_saved_in_the_widget_options() {
        let (mut c, mut store) = controls();
        store.save_widget("pitstop", &[("enabled", json!(true))]);
        c.frame(&mut store, &live_in_car(true), Units::Metric, 1.0);
        c.inject("pit.autoFuel");
        c.frame(&mut store, &live_in_car(true), Units::Metric, 2.0);
        assert!(store.config.widget("pitstop").options.get("keepFuelSet").and_then(Value::as_bool).unwrap());
        // Other options survive.
        assert!(store.config.widget("pitstop").enabled);
        c.inject("pit.autoFuel");
        c.frame(&mut store, &live_in_car(true), Units::Metric, 3.0);
        assert!(!store.config.widget("pitstop").options.get("keepFuelSet").and_then(Value::as_bool).unwrap());
    }
}
