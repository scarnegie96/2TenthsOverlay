//! Sending pit commands: `runPitAction`, `pitCommands` and `usePitControl`
//! from `src/lib/pit.ts`, minus the React. The fuel plan, stepping and auto
//! fuel maths are already in `logic/pit.rs`; this is the state machine around
//! them (what each key sends, the 10 s throttle, pausing auto fuel when you
//! set the fuel by hand, setting the fuel as you enter the pit lane) and the
//! feedback the Pit stop panel shows (key lit, "last sent", AUTO paused).

use std::sync::Mutex;

use telemetry::model::{FuelPayload, PitPayload};
use telemetry::pit::PitCommand;

use crate::config::{Config, Opts, Units};
use crate::logic::format::{fixed, fuel as conv_fuel, fuel_unit};
use crate::logic::pit::{auto_fuel_command, fuel_plan, stepped_fuel, AutoFuel, FuelMode};

/// A key's hint lights up for this long after it's pressed (`LIT_MS`).
pub const LIT_S: f64 = 0.3;
/// "Last sent" stays in the panel's footer this long (`SENT_MS`).
pub const SENT_S: f64 = 2.5;
/// Auto fuel re-sends at most this often.
const AUTO_FUEL_EVERY_S: f64 = 10.0;
/// Quick fuel up/down presses step from the last amount set, not iRacing's slow readback.
const STEP_HOLD_S: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitAction {
    Fuel,
    FuelUp,
    FuelDown,
    AutoFuel,
    Tires,
    FastRepair,
    Tearoff,
    Clear,
}

impl PitAction {
    /// From an action id: "pit.fuelUp".
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id.strip_prefix("pit.")? {
            "fuel" => Self::Fuel,
            "fuelUp" => Self::FuelUp,
            "fuelDown" => Self::FuelDown,
            "autoFuel" => Self::AutoFuel,
            "tires" => Self::Tires,
            "fastRepair" => Self::FastRepair,
            "tearoff" => Self::Tearoff,
            "clear" => Self::Clear,
            _ => return None,
        })
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Fuel => "pit.fuel",
            Self::FuelUp => "pit.fuelUp",
            Self::FuelDown => "pit.fuelDown",
            Self::AutoFuel => "pit.autoFuel",
            Self::Tires => "pit.tires",
            Self::FastRepair => "pit.fastRepair",
            Self::Tearoff => "pit.tearoff",
            Self::Clear => "pit.clear",
        }
    }
}

/// What the Pit stop panel shows about the keys. Times are the app's `now` (seconds).
#[derive(Debug, Clone, Default)]
pub struct Feedback {
    /// The action last pressed, and when (`usePitPressed`).
    pub pressed: Option<(PitAction, f64)>,
    /// What was last sent, and when (`usePitSent`).
    pub sent: Option<(String, f64)>,
    /// Auto fuel to finish is paused because you changed the fuel by hand (`useAutoFuel`).
    pub paused: bool,
}

impl Feedback {
    /// The action whose key hint is lit now.
    pub fn lit(&self, now: f64) -> Option<PitAction> {
        self.pressed.filter(|(_, at)| now - at < LIT_S && now >= *at).map(|(a, _)| a)
    }

    /// The "last sent" message, while it's fresh.
    pub fn sent_text(&self, now: f64) -> Option<&str> {
        self.sent.as_ref().filter(|(_, at)| now - at < SENT_S && now >= *at).map(|(t, _)| t.as_str())
    }
}

/// The panel is drawn by a widget that can't reach the controls, so the
/// controls publish their feedback here once a frame.
static FEEDBACK: Mutex<Option<Feedback>> = Mutex::new(None);

pub fn publish(fb: &Feedback) {
    if let Ok(mut g) = FEEDBACK.lock() {
        *g = Some(fb.clone());
    }
}

/// The latest published feedback (nothing pressed yet, until the controls run).
pub fn feedback() -> Feedback {
    FEEDBACK.lock().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// Where commands go: the runner (iRacing or the demo race).
pub trait PitOut {
    fn send(&mut self, cmd: PitCommand);
}

impl PitOut for telemetry::runner::RunnerControl {
    fn send(&mut self, cmd: PitCommand) {
        self.pit_command(cmd);
    }
}

#[cfg(test)]
impl PitOut for Vec<PitCommand> {
    fn send(&mut self, cmd: PitCommand) {
        self.push(cmd);
    }
}

/// The settings pit control reads: the Pit stop widget's options, the Fuel
/// widget's margin, and the units.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub mode: FuelMode,
    pub step: f32,
    /// `keepFuelSet`: auto fuel to finish.
    pub keep: bool,
    /// `autoFuel`: set the planned fuel as you enter the pit lane.
    pub at_entry: bool,
    pub margin: f32,
    pub units: Units,
}

impl Settings {
    pub fn from_config(cfg: &Config, units: Units) -> Self {
        let o = Opts::new(cfg, "pitstop");
        Self {
            mode: FuelMode::parse(o.s("fuelMode")),
            step: o.n("fuelStep") as f32,
            keep: o.b("keepFuelSet"),
            at_entry: o.b("autoFuel"),
            margin: Opts::new(cfg, "fuel").n("marginLaps") as f32,
            units,
        }
    }
}

/// What pit control looks at each frame.
pub struct Inputs<'a> {
    pub settings: Settings,
    pub pit: Option<&'a PitPayload>,
    pub fuel: Option<&'a FuelPayload>,
    pub on_pit_road: Option<bool>,
    /// Field payloads received so far (changes when a new one arrives).
    pub fields: u64,
    pub now: f64,
}

impl Inputs<'_> {
    /// Pit service is published (you're in the car, not spectating).
    fn pit(&self) -> Option<&PitPayload> {
        self.pit.filter(|p| p.available)
    }

    fn plan(&self) -> Option<i32> {
        fuel_plan(self.settings.mode, self.fuel, self.settings.margin)
    }

    fn fuel_text(&self, litres: f32) -> String {
        let u = self.settings.units;
        format!("{} {}", fixed(conv_fuel(litres, u) as f64, if u == Units::Imperial { 1 } else { 0 }), fuel_unit(u))
    }
}

/// The commands for a key, apart from fuel up, down and auto. Each one
/// toggles: fuel sets the plan, or unticks fuel when that amount is already
/// set; tires tick all four, or untick them when any are ticked.
pub fn pit_commands(action: PitAction, pit: &PitPayload, plan: Option<i32>, mode: FuelMode) -> Vec<PitCommand> {
    match action {
        PitAction::Fuel => {
            if mode == FuelMode::Manual {
                return vec![if pit.fuel { PitCommand::ClearFuel } else { PitCommand::Fuel { litres: 0 } }];
            }
            let Some(plan) = plan else { return Vec::new() };
            if plan <= 0 || (pit.fuel && (pit.fuel_amount - plan as f32).abs() < 1.0) {
                return vec![PitCommand::ClearFuel];
            }
            vec![PitCommand::Fuel { litres: plan as u32 }]
        }
        PitAction::Tires => {
            if pit.tires.iter().any(|t| *t) {
                return vec![PitCommand::ClearTires];
            }
            (0..4).map(|corner| PitCommand::Tire { corner, kpa: 0 }).collect()
        }
        PitAction::FastRepair => vec![if pit.fast_repair { PitCommand::ClearFastRepair } else { PitCommand::FastRepair }],
        PitAction::Tearoff => vec![if pit.tearoff { PitCommand::ClearTearoff } else { PitCommand::Tearoff }],
        PitAction::Clear => vec![PitCommand::Clear],
        // Handled by `run_action`.
        PitAction::FuelUp | PitAction::FuelDown | PitAction::AutoFuel => Vec::new(),
    }
}

fn describe(cmds: &[PitCommand], fuel: &str) -> String {
    match cmds.first() {
        None => String::new(),
        Some(PitCommand::Fuel { litres }) if *litres > 0 => format!("Fuel set to {fuel}"),
        Some(PitCommand::Fuel { .. }) => "Fuel ticked".into(),
        Some(PitCommand::ClearFuel) => "Fuel unticked".into(),
        Some(PitCommand::Tire { .. }) => "Four tires ticked".into(),
        Some(PitCommand::ClearTires) => "Tires unticked".into(),
        Some(PitCommand::FastRepair) => "Fast repair ticked".into(),
        Some(PitCommand::ClearFastRepair) => "Fast repair unticked".into(),
        Some(PitCommand::Tearoff) => "Tear-off ticked".into(),
        Some(PitCommand::ClearTearoff) => "Tear-off unticked".into(),
        Some(PitCommand::Clear) => "Everything unticked".into(),
        Some(PitCommand::Compound { .. }) => "Compound changed".into(),
    }
}

fn litres_u32(l: f32) -> u32 {
    l.round().max(0.0) as u32
}

pub struct PitControl {
    pub fb: Feedback,
    /// When auto fuel last sent; reset when it's turned on or resumed, so it acts at once.
    auto_last_sent: f64,
    /// The amount the last fuel up or down press set, and when.
    stepped: Option<(f32, f64)>,
    /// Fuel was set by hand with up or down, so auto fuel leaves the next stop alone.
    fuel_by_hand: bool,
    prev_on_pit_road: Option<bool>,
    last_fields: u64,
    keep_active: bool,
}

impl Default for PitControl {
    fn default() -> Self {
        Self {
            fb: Feedback::default(),
            auto_last_sent: f64::NEG_INFINITY,
            stepped: None,
            fuel_by_hand: false,
            prev_on_pit_road: None,
            last_fields: 0,
            keep_active: false,
        }
    }
}

impl PitControl {
    fn resume_auto_fuel(&mut self) {
        self.auto_last_sent = f64::NEG_INFINITY;
        self.fb.paused = false;
    }

    fn send(&mut self, cmds: &[PitCommand], fuel_text: &str, now: f64, out: &mut dyn PitOut) {
        if cmds.is_empty() {
            return;
        }
        for c in cmds {
            out.send(*c);
        }
        self.fb.sent = Some((describe(cmds, fuel_text), now));
    }

    /// Runs one pit action (`runPitAction`). Returns the new `keepFuelSet`
    /// when the action changed it, for the caller to save in the config.
    pub fn run_action(&mut self, action: PitAction, inp: &Inputs, out: &mut dyn PitOut) -> Option<bool> {
        let now = inp.now;
        self.fb.pressed = Some((action, now));
        let mode = inp.settings.mode;
        let plan = inp.plan();
        let text = plan.map(|l| inp.fuel_text(l as f32)).unwrap_or_default();
        if action == PitAction::AutoFuel {
            // On or off, kept in the settings; turning it on (or back on) resumes it.
            let on = !inp.settings.keep || self.fb.paused;
            self.resume_auto_fuel();
            self.fb.sent = Some((if on { "Auto fuel to finish on" } else { "Auto fuel off" }.into(), now));
            return Some(on);
        }
        let pit = inp.pit()?;
        if matches!(action, PitAction::FuelUp | PitAction::FuelDown) {
            let from = match self.stepped {
                Some((litres, at)) if now - at < STEP_HOLD_S => litres,
                _ if pit.fuel => pit.fuel_amount,
                _ => 0.0,
            };
            let step = inp.settings.step;
            let max = inp.fuel.map_or(0.0, |f| f.max);
            let next = stepped_fuel(from, if action == PitAction::FuelUp { step } else { -step }, max);
            self.stepped = Some((next, now));
            self.fuel_by_hand = true;
            // Set by hand: auto fuel steps back until you ask for it again.
            self.fb.paused = true;
            let cmd = if next > 0.0 { PitCommand::Fuel { litres: litres_u32(next) } } else { PitCommand::ClearFuel };
            self.send(&[cmd], &inp.fuel_text(next), now, out);
            return None;
        }
        self.stepped = None;
        if matches!(action, PitAction::Fuel | PitAction::Clear) {
            self.fuel_by_hand = false;
        }
        if action == PitAction::Fuel {
            self.resume_auto_fuel();
        }
        if action == PitAction::Clear {
            self.fb.paused = true;
        }
        self.send(&pit_commands(action, pit, plan, mode), &text, now, out);
        None
    }

    /// The background part of `usePitControl`, once a frame while `live` (the
    /// Pit stop widget is on and you're in the car): auto fuel to finish, and
    /// setting the fuel as you enter the pit lane.
    pub fn update(&mut self, live: bool, inp: &Inputs, out: &mut dyn PitOut) {
        let now = inp.now;
        let new_field = inp.fields != self.last_fields;
        self.last_fields = inp.fields;
        let entered = inp.on_pit_road == Some(true) && self.prev_on_pit_road == Some(false);
        if inp.on_pit_road.is_some() {
            self.prev_on_pit_road = inp.on_pit_road;
        }

        // Auto fuel to finish: keep iRacing's amount at what you need as the
        // estimate changes. Not in the pit lane (the stop is under way), and
        // not while paused because you set it by hand.
        let active = live && inp.settings.keep;
        if active != self.keep_active {
            self.keep_active = active;
            self.auto_last_sent = f64::NEG_INFINITY;
        }
        if active && new_field && !self.fb.paused && inp.on_pit_road != Some(true) && now - self.auto_last_sent >= AUTO_FUEL_EVERY_S {
            if let Some(pit) = inp.pit() {
                let need = fuel_plan(FuelMode::Finish, inp.fuel, inp.settings.margin).map(|n| n as f32);
                let tank = inp.fuel.map_or(0.0, |f| f.max);
                if let Some(cmd) = auto_fuel_command(pit, need, tank) {
                    self.auto_last_sent = now;
                    out.send(match cmd {
                        AutoFuel::Fuel(l) => PitCommand::Fuel { litres: litres_u32(l) },
                        AutoFuel::ClearFuel => PitCommand::ClearFuel,
                    });
                }
            }
        }

        // Auto fuel: set the planned amount as you cross into the pit lane.
        if live && entered && inp.settings.at_entry {
            if self.fuel_by_hand {
                self.fuel_by_hand = false;
                return;
            }
            let (Some(litres), Some(pit)) = (inp.plan(), inp.pit()) else { return };
            if inp.settings.mode == FuelMode::Manual || (pit.fuel && (pit.fuel_amount - litres as f32).abs() < 1.0) {
                return;
            }
            let cmd = if litres > 0 { PitCommand::Fuel { litres: litres as u32 } } else { PitCommand::ClearFuel };
            self.send(&[cmd], &inp.fuel_text(litres as f32), now, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(fuel: bool, amount: f32) -> PitPayload {
        PitPayload { available: true, fuel, fuel_amount: amount, ..Default::default() }
    }

    fn fuel_state(level: f32, max: f32, per_lap: f32, to_finish: f32) -> FuelPayload {
        FuelPayload { level, max, per_lap: Some(per_lap), to_finish: Some(to_finish), ..Default::default() }
    }

    fn settings() -> Settings {
        Settings { mode: FuelMode::Finish, step: 5.0, keep: false, at_entry: false, margin: 0.0, units: Units::Metric }
    }

    struct Rig {
        c: PitControl,
        out: Vec<PitCommand>,
        pit: PitPayload,
        fuel: FuelPayload,
        s: Settings,
        fields: u64,
        pit_road: Option<bool>,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                c: PitControl::default(),
                out: Vec::new(),
                pit: menu(false, 0.0),
                fuel: fuel_state(10.0, 100.0, 2.0, 18.0),
                s: settings(),
                fields: 0,
                pit_road: Some(false),
            }
        }

        fn press(&mut self, a: PitAction, now: f64) -> Option<bool> {
            let inp = Inputs { settings: self.s, pit: Some(&self.pit), fuel: Some(&self.fuel), on_pit_road: self.pit_road, fields: self.fields, now };
            self.c.run_action(a, &inp, &mut self.out)
        }

        fn tick(&mut self, now: f64) {
            self.fields += 1;
            let inp = Inputs { settings: self.s, pit: Some(&self.pit), fuel: Some(&self.fuel), on_pit_road: self.pit_road, fields: self.fields, now };
            self.c.update(true, &inp, &mut self.out);
        }
    }

    #[test]
    fn ids_round_trip() {
        for a in [
            PitAction::Fuel,
            PitAction::FuelUp,
            PitAction::FuelDown,
            PitAction::AutoFuel,
            PitAction::Tires,
            PitAction::FastRepair,
            PitAction::Tearoff,
            PitAction::Clear,
        ] {
            assert_eq!(PitAction::from_id(a.id()), Some(a));
        }
        assert_eq!(PitAction::from_id("overlay.edit"), None);
        assert_eq!(PitAction::from_id("pit.nope"), None);
    }

    #[test]
    fn fuel_key_sets_the_plan_then_unticks() {
        let mut r = Rig::new();
        r.press(PitAction::Fuel, 1.0);
        assert_eq!(r.out, [PitCommand::Fuel { litres: 18 }]);
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Fuel set to 18 L");
        // iRacing now shows that amount: pressing again unticks.
        r.pit = menu(true, 18.0);
        r.press(PitAction::Fuel, 2.0);
        assert_eq!(r.out[1], PitCommand::ClearFuel);
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Fuel unticked");
    }

    #[test]
    fn fuel_modes() {
        let mut r = Rig::new();
        r.s.mode = FuelMode::Full;
        r.press(PitAction::Fuel, 1.0);
        assert_eq!(r.out, [PitCommand::Fuel { litres: 90 }]);
        let mut r = Rig::new();
        r.s.mode = FuelMode::Manual;
        r.press(PitAction::Fuel, 1.0);
        assert_eq!(r.out, [PitCommand::Fuel { litres: 0 }]);
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Fuel ticked");
        r.pit = menu(true, 12.0);
        r.press(PitAction::Fuel, 2.0);
        assert_eq!(r.out[1], PitCommand::ClearFuel);
        // Enough fuel: the plan is 0, so the key unticks.
        let mut r = Rig::new();
        r.fuel = fuel_state(50.0, 100.0, 2.0, -5.0);
        r.press(PitAction::Fuel, 1.0);
        assert_eq!(r.out, [PitCommand::ClearFuel]);
        // No plan yet and not manual: nothing to send, nothing to say.
        let mut r = Rig::new();
        r.fuel = FuelPayload { max: 100.0, ..Default::default() };
        r.press(PitAction::Fuel, 1.0);
        assert!(r.out.is_empty());
        assert!(r.c.fb.sent.is_none());
    }

    #[test]
    fn imperial_text() {
        let mut r = Rig::new();
        r.s.units = Units::Imperial;
        r.press(PitAction::Fuel, 1.0);
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Fuel set to 4.8 gal");
    }

    #[test]
    fn tires_toggle() {
        let mut r = Rig::new();
        r.press(PitAction::Tires, 1.0);
        assert_eq!(r.out.len(), 4);
        assert_eq!(r.out[3], PitCommand::Tire { corner: 3, kpa: 0 });
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Four tires ticked");
        r.out.clear();
        r.pit.tires = [false, true, false, false];
        r.press(PitAction::Tires, 2.0);
        assert_eq!(r.out, [PitCommand::ClearTires]);
    }

    #[test]
    fn repair_tearoff_clear() {
        let mut r = Rig::new();
        r.press(PitAction::FastRepair, 1.0);
        r.press(PitAction::Tearoff, 1.0);
        r.pit.fast_repair = true;
        r.pit.tearoff = true;
        r.press(PitAction::FastRepair, 2.0);
        r.press(PitAction::Tearoff, 2.0);
        r.press(PitAction::Clear, 3.0);
        assert_eq!(
            r.out,
            [
                PitCommand::FastRepair,
                PitCommand::Tearoff,
                PitCommand::ClearFastRepair,
                PitCommand::ClearTearoff,
                PitCommand::Clear
            ]
        );
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Everything unticked");
        assert!(r.c.fb.paused, "clear pauses auto fuel");
    }

    #[test]
    fn up_and_down_step_from_the_last_press() {
        let mut r = Rig::new();
        r.pit = menu(true, 20.0);
        r.press(PitAction::FuelUp, 1.0);
        // iRacing's readback hasn't caught up: step from what we set.
        r.press(PitAction::FuelUp, 1.2);
        r.press(PitAction::FuelDown, 1.4);
        assert_eq!(r.out, [PitCommand::Fuel { litres: 25 }, PitCommand::Fuel { litres: 30 }, PitCommand::Fuel { litres: 25 }]);
        // Later, step from what iRacing shows.
        r.pit = menu(true, 40.0);
        r.press(PitAction::FuelUp, 5.0);
        assert_eq!(r.out[3], PitCommand::Fuel { litres: 45 });
        assert!(r.c.fb.paused);
        // Down to nothing unticks; never past empty or a full tank.
        r.pit = menu(true, 3.0);
        r.press(PitAction::FuelDown, 10.0);
        assert_eq!(r.out[4], PitCommand::ClearFuel);
        r.pit = menu(true, 98.0);
        r.press(PitAction::FuelUp, 20.0);
        assert_eq!(r.out[5], PitCommand::Fuel { litres: 100 });
    }

    #[test]
    fn nothing_is_sent_without_pit_service() {
        let mut r = Rig::new();
        r.pit.available = false;
        r.press(PitAction::Fuel, 1.0);
        r.press(PitAction::FuelUp, 1.0);
        assert!(r.out.is_empty());
        // The key still lights up.
        assert_eq!(r.c.fb.lit(1.1), Some(PitAction::FuelUp));
        assert_eq!(r.c.fb.lit(1.4), None);
    }

    #[test]
    fn auto_fuel_key_toggles_and_resumes() {
        let mut r = Rig::new();
        assert_eq!(r.press(PitAction::AutoFuel, 1.0), Some(true));
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Auto fuel to finish on");
        r.s.keep = true;
        assert_eq!(r.press(PitAction::AutoFuel, 2.0), Some(false));
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Auto fuel off");
        // Paused by hand: the key turns it back on rather than off.
        r.c.fb.paused = true;
        assert_eq!(r.press(PitAction::AutoFuel, 3.0), Some(true));
        assert!(!r.c.fb.paused);
        assert!(r.out.is_empty());
    }

    #[test]
    fn sent_and_lit_expire() {
        let mut r = Rig::new();
        r.press(PitAction::Fuel, 10.0);
        assert_eq!(r.c.fb.lit(10.2), Some(PitAction::Fuel));
        assert_eq!(r.c.fb.lit(10.31), None);
        assert_eq!(r.c.fb.sent_text(12.0), Some("Fuel set to 18 L"));
        assert_eq!(r.c.fb.sent_text(12.6), None);
    }

    #[test]
    fn auto_fuel_to_finish_is_throttled_and_pauses() {
        let mut r = Rig::new();
        r.s.keep = true;
        r.tick(1.0);
        assert_eq!(r.out, [PitCommand::Fuel { litres: 18 }]);
        // iRacing now has it: nothing more. A moving estimate within a litre: nothing.
        r.pit = menu(true, 18.0);
        r.fuel.to_finish = Some(17.6);
        r.tick(5.0);
        assert_eq!(r.out.len(), 1);
        // A real change, but too soon after the last send.
        r.fuel.to_finish = Some(30.0);
        r.tick(6.0);
        assert_eq!(r.out.len(), 1);
        r.tick(12.0);
        assert_eq!(r.out, [PitCommand::Fuel { litres: 18 }, PitCommand::Fuel { litres: 30 }]);
        // In the pit lane: left alone.
        r.fuel.to_finish = Some(40.0);
        r.pit_road = Some(true);
        r.tick(40.0);
        assert_eq!(r.out.len(), 2);
        r.pit_road = Some(false);
        // Set by hand: paused until the fuel key or auto key resumes it.
        r.press(PitAction::FuelUp, 41.0);
        let sent = r.out.len();
        r.tick(60.0);
        assert_eq!(r.out.len(), sent);
        r.press(PitAction::Fuel, 61.0);
        let sent = r.out.len();
        r.pit = menu(true, 5.0);
        r.tick(62.0);
        assert_eq!(r.out.len(), sent + 1, "resumed, and acts at once");
    }

    #[test]
    fn auto_fuel_waits_for_a_new_field_payload() {
        let mut r = Rig::new();
        r.s.keep = true;
        r.tick(1.0);
        assert_eq!(r.out.len(), 1);
        // Same payload as last frame, ten seconds later: nothing new to react to.
        r.pit = menu(false, 0.0);
        let inp = Inputs { settings: r.s, pit: Some(&r.pit), fuel: Some(&r.fuel), on_pit_road: r.pit_road, fields: r.fields, now: 30.0 };
        r.c.update(true, &inp, &mut r.out);
        assert_eq!(r.out.len(), 1);
    }

    #[test]
    fn auto_fuel_stays_off_when_not_live() {
        let mut r = Rig::new();
        r.s.keep = true;
        r.fields += 1;
        let inp = Inputs { settings: r.s, pit: Some(&r.pit), fuel: Some(&r.fuel), on_pit_road: r.pit_road, fields: r.fields, now: 1.0 };
        r.c.update(false, &inp, &mut r.out);
        assert!(r.out.is_empty());
    }

    #[test]
    fn set_fuel_on_pit_entry() {
        let mut r = Rig::new();
        r.s.at_entry = true;
        r.tick(1.0);
        assert!(r.out.is_empty(), "not in the pit lane yet");
        r.pit_road = Some(true);
        r.tick(2.0);
        assert_eq!(r.out, [PitCommand::Fuel { litres: 18 }]);
        assert_eq!(r.c.fb.sent.as_ref().unwrap().0, "Fuel set to 18 L");
        // Still in the lane: once only.
        r.tick(3.0);
        assert_eq!(r.out.len(), 1);
        // Next entry, with fuel already right: nothing.
        r.pit_road = Some(false);
        r.tick(4.0);
        r.pit = menu(true, 18.0);
        r.pit_road = Some(true);
        r.tick(5.0);
        assert_eq!(r.out.len(), 1);
    }

    #[test]
    fn pit_entry_skips_one_stop_after_setting_by_hand() {
        let mut r = Rig::new();
        r.s.at_entry = true;
        r.tick(1.0);
        r.press(PitAction::FuelUp, 1.5);
        let sent = r.out.len();
        r.pit_road = Some(true);
        r.tick(2.0);
        assert_eq!(r.out.len(), sent, "your amount stays");
        r.pit_road = Some(false);
        r.tick(3.0);
        r.pit_road = Some(true);
        r.tick(4.0);
        assert_eq!(r.out.len(), sent + 1, "the next stop is planned again");
    }

    #[test]
    fn pit_entry_in_manual_mode_does_nothing() {
        let mut r = Rig::new();
        r.s.at_entry = true;
        r.s.mode = FuelMode::Manual;
        r.tick(1.0);
        r.pit_road = Some(true);
        r.tick(2.0);
        assert!(r.out.is_empty());
    }

    #[test]
    fn pit_entry_unticks_when_there_is_enough() {
        let mut r = Rig::new();
        r.s.at_entry = true;
        r.fuel = fuel_state(80.0, 100.0, 2.0, -10.0);
        r.pit = menu(true, 10.0);
        r.tick(1.0);
        r.pit_road = Some(true);
        r.tick(2.0);
        assert_eq!(r.out, [PitCommand::ClearFuel]);
    }
}
