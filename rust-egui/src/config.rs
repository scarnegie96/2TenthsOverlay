//! The Tauri app's `config.json`, read as-is and normalised like
//! `normalizeConfig` in `src/lib/config.ts`. The file stays the Tauri app's:
//! this only ever writes a widget's place (or scale, or enabled) back into
//! it, leaving every other field as it was.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Map, Value};

use crate::schema::{self, Def, Meta, WIDGETS};

pub const SCALE_MIN: f32 = 0.5;
pub const SCALE_MAX: f32 = 3.0;
pub const OPACITY_MIN: f32 = 0.2;
pub const OPACITY_MAX: f32 = 1.0;

const APP_ID: &str = "dev.tenths.overlay";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Units {
    #[default]
    Metric,
    Imperial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShowWhen {
    #[default]
    Connected,
    OnTrack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeId {
    #[default]
    Dark,
    Light,
    F1,
}

impl ThemeId {
    pub const ALL: [ThemeId; 3] = [ThemeId::Dark, ThemeId::Light, ThemeId::F1];
    pub fn key(self) -> &'static str {
        match self {
            ThemeId::Dark => "dark",
            ThemeId::Light => "light",
            ThemeId::F1 => "f1",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.key() == s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorX {
    L,
    C,
    R,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorY {
    T,
    M,
    B,
}

impl AnchorX {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "l" => Some(Self::L),
            "c" => Some(Self::C),
            "r" => Some(Self::R),
            _ => None,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::L => "l",
            Self::C => "c",
            Self::R => "r",
        }
    }
}
impl AnchorY {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "t" => Some(Self::T),
            "m" => Some(Self::M),
            "b" => Some(Self::B),
            _ => None,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::T => "t",
            Self::M => "m",
            Self::B => "b",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WidgetCfg {
    pub enabled: bool,
    pub x: f64,
    pub y: f64,
    pub anchor: Option<(AnchorX, AnchorY)>,
    pub scale: f32,
    pub opacity: f32,
    pub options: Map<String, Value>,
    /// The display it is on (`displays::Display::key`); None = the primary.
    pub display: Option<String>,
}

impl WidgetCfg {
    pub fn default_for(m: &Meta) -> Self {
        Self { enabled: m.enabled, x: m.x, y: m.y, anchor: None, scale: 1.0, opacity: 1.0, options: Map::new(), display: None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct General {
    pub demo: bool,
    pub units: Units,
    pub show_when: ShowWhen,
    pub theme: ThemeId,
    /// Colours changed from each theme's defaults (`general.colors[theme][key]`).
    pub colors: HashMap<ThemeId, HashMap<String, String>>,
    /// `general.controls`, as saved (for the Pit stop's key hints).
    pub controls: Value,
    /// `general.activeProfile`: the saved layout that edits are saved into.
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub general: General,
    /// In `WIDGETS` order.
    pub widgets: Vec<WidgetCfg>,
}

impl Default for Config {
    fn default() -> Self {
        normalize(&Value::Null)
    }
}

impl Config {
    pub fn widget(&self, id: &str) -> &WidgetCfg {
        let i = WIDGETS.iter().position(|m| m.id == id).expect("known widget");
        &self.widgets[i]
    }
    pub fn widget_mut(&mut self, id: &str) -> &mut WidgetCfg {
        let i = WIDGETS.iter().position(|m| m.id == id).expect("known widget");
        &mut self.widgets[i]
    }
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().filter(|n| n.is_finite())
}

fn normalize_widget(raw: &Value, m: &Meta) -> WidgetCfg {
    let base = WidgetCfg::default_for(m);
    let Some(o) = raw.as_object() else { return base };
    let mut w = WidgetCfg {
        enabled: o.get("enabled").and_then(Value::as_bool).unwrap_or(base.enabled),
        x: o.get("x").and_then(num).unwrap_or(base.x),
        y: o.get("y").and_then(num).unwrap_or(base.y),
        anchor: None,
        scale: o.get("scale").and_then(num).map(|s| (s as f32).clamp(SCALE_MIN, SCALE_MAX)).unwrap_or(base.scale),
        opacity: o.get("opacity").and_then(num).map(|s| (s as f32).clamp(OPACITY_MIN, OPACITY_MAX)).unwrap_or(base.opacity),
        options: Map::new(),
        display: o.get("display").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from),
    };
    let ax = o.get("ax").and_then(Value::as_str).and_then(AnchorX::parse);
    let ay = o.get("ay").and_then(Value::as_str).and_then(AnchorY::parse);
    match (ax, ay) {
        (Some(ax), Some(ay)) => w.anchor = Some((ax, ay)),
        _ if o.contains_key("ax") || o.contains_key("ay") => {
            // A broken anchor makes the offsets meaningless too.
            w.x = base.x;
            w.y = base.y;
        }
        _ => {}
    }
    if let Some(opts) = o.get("options").and_then(Value::as_object) {
        for (k, v) in opts {
            if v.is_boolean() || v.is_string() || num(v).is_some() {
                w.options.insert(k.clone(), v.clone());
            }
        }
    }
    w
}

/// A saved config over the defaults; anything of the wrong type falls back.
pub fn normalize(raw: &Value) -> Config {
    let widgets = WIDGETS
        .iter()
        .map(|m| match raw.get("widgets").and_then(|w| w.get(m.id)) {
            Some(v) => normalize_widget(v, m),
            None => WidgetCfg::default_for(m),
        })
        .collect();
    let g = raw.get("general").cloned().unwrap_or(Value::Null);
    let mut colors: HashMap<ThemeId, HashMap<String, String>> = HashMap::new();
    for t in ThemeId::ALL {
        let mut own = HashMap::new();
        if let Some(saved) = g.get("colors").and_then(|c| c.get(t.key())).and_then(Value::as_object) {
            for (k, v) in saved {
                if let Some(s) = v.as_str() {
                    own.insert(k.clone(), s.to_string());
                }
            }
        }
        colors.insert(t, own);
    }
    let general = General {
        demo: g.get("demo").and_then(Value::as_bool).unwrap_or(false),
        units: match g.get("units").and_then(Value::as_str) {
            Some("imperial") => Units::Imperial,
            _ => Units::Metric,
        },
        show_when: match g.get("showWhen").and_then(Value::as_str) {
            Some("onTrack") => ShowWhen::OnTrack,
            _ => ShowWhen::Connected,
        },
        theme: g.get("theme").and_then(Value::as_str).and_then(ThemeId::parse).unwrap_or_default(),
        colors,
        controls: g.get("controls").cloned().unwrap_or(Value::Null),
        active_profile: g.get("activeProfile").and_then(Value::as_str).map(String::from),
    };
    Config { general, widgets }
}

// --- Option lookup ----------------------------------------------------------

/// One widget's options, with the defaults from the schema.
#[derive(Clone, Copy)]
pub struct Opts<'a> {
    pub w: &'a WidgetCfg,
    pub meta: &'static Meta,
}

impl<'a> Opts<'a> {
    pub fn new(cfg: &'a Config, id: &str) -> Self {
        Self { w: cfg.widget(id), meta: schema::meta(id).expect("known widget") }
    }
    fn raw(&self, key: &str) -> Option<&'a Value> {
        self.w.options.get(key)
    }
    pub fn b(&self, key: &str) -> bool {
        self.raw(key).and_then(Value::as_bool).unwrap_or(match self.meta.default_of(key) {
            Some(Def::B(b)) => b,
            _ => false,
        })
    }
    pub fn n(&self, key: &str) -> f64 {
        self.raw(key).and_then(num).unwrap_or(match self.meta.default_of(key) {
            Some(Def::N(n)) => n,
            _ => 0.0,
        })
    }
    /// A whole-number option (row counts and the like), at least `min`.
    pub fn int(&self, key: &str, min: i64) -> usize {
        (self.n(key).round() as i64).max(min) as usize
    }
    pub fn s(&self, key: &str) -> &'a str {
        self.raw(key).and_then(Value::as_str).unwrap_or(match self.meta.default_of(key) {
            Some(Def::S(s)) => s,
            _ => "",
        })
    }
}

// --- Where the file lives -----------------------------------------------------

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// Tauri's `app_config_dir()` for this app.
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = env_dir("APPDATA");
    #[cfg(target_os = "macos")]
    let base = env_dir("HOME").map(|h| h.join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = env_dir("XDG_CONFIG_HOME").or_else(|| env_dir("HOME").map(|h| h.join(".config")));
    base.map(|b| b.join(APP_ID))
}

/// Tauri's `app_data_dir()`: the same Roaming folder on Windows.
pub fn data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = env_dir("APPDATA");
    #[cfg(target_os = "macos")]
    let base = env_dir("HOME").map(|h| h.join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = env_dir("XDG_DATA_HOME").or_else(|| env_dir("HOME").map(|h| h.join(".local/share")));
    base.map(|b| b.join(APP_ID))
}

pub fn default_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.json"))
}

// --- Reading, watching and writing back --------------------------------------------

/// The config file and what was last read from it.
pub struct Store {
    pub path: Option<PathBuf>,
    pub config: Config,
    /// Modified time when last read or written; None when there's no file.
    mtime: Option<SystemTime>,
    last_poll: Option<std::time::Instant>,
    /// What went wrong last, for the edit bar.
    pub error: Option<String>,
    /// Counts `save_widget` calls (not reloads), so edits can be copied into
    /// the active layout.
    pub edits: u64,
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Reads and parses the file: Ok(None) when it isn't there.
fn read_value(path: &Path) -> Result<Option<Value>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map(Some).map_err(|e| format!("{} couldn't be read: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

impl Store {
    pub fn open(path: Option<PathBuf>) -> Self {
        let mut s = Self { path, config: Config::default(), mtime: None, last_poll: None, error: None, edits: 0 };
        s.reload();
        s
    }

    fn reload(&mut self) {
        let Some(path) = self.path.clone() else { return };
        self.mtime = mtime(&path);
        match read_value(&path) {
            Ok(Some(v)) => {
                self.config = normalize(&v);
                self.error = None;
            }
            Ok(None) => {
                self.config = Config::default();
                self.error = None;
            }
            // Keep what we had: the Tauri app may be half-way through a write.
            Err(e) => {
                eprintln!("[tenths-egui] {e}");
                self.error = Some(e);
            }
        }
    }

    /// Picks up changes saved by the Tauri app's settings. Checks the file's
    /// modified time at most once a second; returns true when it reloaded.
    pub fn poll(&mut self) -> bool {
        let now = std::time::Instant::now();
        if self.last_poll.is_some_and(|t| now - t < std::time::Duration::from_secs(1)) {
            return false;
        }
        self.last_poll = Some(now);
        let Some(path) = &self.path else { return false };
        if mtime(path) != self.mtime {
            self.reload();
            return true;
        }
        false
    }

    /// Sets fields of one widget (`x`, `y`, `ax`, `ay`, `scale`, `enabled`) in
    /// the file, keeping everything else in it, and in memory.
    pub fn save_widget(&mut self, id: &str, patch: &[(&str, Value)]) {
        // In memory first, so the widget stays where it was dropped even if the write fails.
        let mut raw = serde_json::to_value(self.widget_value(id)).unwrap_or(Value::Null);
        for (k, v) in patch {
            raw[*k] = v.clone();
        }
        let meta = schema::meta(id).expect("known widget");
        *self.config.widget_mut(id) = normalize_widget(&raw, meta);
        self.edits += 1;

        let Some(path) = self.path.clone() else { return };
        match write_widget(&path, id, patch) {
            Ok(()) => {
                self.mtime = mtime(&path);
                self.error = None;
            }
            Err(e) => {
                eprintln!("[tenths-egui] couldn't save {}: {e}", path.display());
                self.error = Some(format!("Couldn't save: {e}"));
            }
        }
    }

    /// Every widget's settings, as `widgets` in config.json and in a layout file.
    pub fn widgets_json(&self) -> Value {
        Value::Object(WIDGETS.iter().map(|m| (m.id.to_string(), self.widget_value(m.id))).collect())
    }

    /// Puts a layout's widgets on screen and (with `Some`) makes it the active
    /// one, in memory and in the file (`switchProfile`).
    pub fn set_layout(&mut self, widgets: &Value, active: Option<&str>) {
        let fresh = normalize(&serde_json::json!({ "widgets": widgets }));
        self.config.widgets = fresh.widgets;
        self.config.general.active_profile = active.map(String::from);
        let json = self.widgets_json();
        let Some(path) = self.path.clone() else { return };
        let result = write_root(&path, |root| {
            root.insert("widgets".into(), json);
            set_active(root, active);
        });
        self.finish_write(&path, result);
    }

    /// Sets (or clears) `general.activeProfile`.
    pub fn set_active_profile(&mut self, active: Option<&str>) {
        self.config.general.active_profile = active.map(String::from);
        let Some(path) = self.path.clone() else { return };
        let result = write_root(&path, |root| set_active(root, active));
        self.finish_write(&path, result);
    }

    fn finish_write(&mut self, path: &Path, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.mtime = mtime(path);
                self.error = None;
            }
            Err(e) => {
                eprintln!("[tenths-egui] couldn't save {}: {e}", path.display());
                self.error = Some(format!("Couldn't save: {e}"));
            }
        }
    }

    /// The widget's settings as the JSON the Tauri app saves.
    fn widget_value(&self, id: &str) -> Value {
        let w = self.config.widget(id);
        let mut o = Map::new();
        o.insert("enabled".into(), w.enabled.into());
        o.insert("x".into(), w.x.into());
        o.insert("y".into(), w.y.into());
        if let Some(d) = &w.display {
            o.insert("display".into(), d.clone().into());
        }
        if let Some((ax, ay)) = w.anchor {
            o.insert("ax".into(), ax.key().into());
            o.insert("ay".into(), ay.key().into());
        }
        o.insert("scale".into(), (w.scale as f64).into());
        o.insert("opacity".into(), (w.opacity as f64).into());
        o.insert("options".into(), Value::Object(w.options.clone()));
        Value::Object(o)
    }
}

fn set_active(root: &mut Map<String, Value>, active: Option<&str>) {
    let general = root.entry("general").or_insert_with(|| Value::Object(Map::new()));
    if !general.is_object() {
        *general = Value::Object(Map::new());
    }
    general.as_object_mut().unwrap().insert("activeProfile".into(), active.map(Value::from).unwrap_or(Value::Null));
}

/// Reads the file fresh, lets `change` edit its top level, and writes it back
/// the safe way. Refuses to touch a file that's there but can't be read.
pub fn write_root(path: &Path, change: impl FnOnce(&mut Map<String, Value>)) -> Result<(), String> {
    let mut root = match read_value(path)? {
        Some(v @ Value::Object(_)) => v,
        Some(_) => return Err("config.json isn't a JSON object".into()),
        None => serde_json::json!({ "version": 1 }),
    };
    change(root.as_object_mut().expect("object"));
    write_atomic(path, &root)
}

fn write_atomic(path: &Path, root: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(root).map_err(|e| e.to_string())?;
    // Not the Tauri app's own temp name, so the two never write the same temp file.
    let tmp = path.with_extension("json.egui-tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Reads the file fresh, changes the one widget, and writes it back through
/// a temporary file and a rename, so a crash never leaves half a config.
/// Refuses to touch a file that's there but can't be read.
pub fn write_widget(path: &Path, id: &str, patch: &[(&str, Value)]) -> Result<(), String> {
    let mut root = match read_value(path)? {
        Some(v @ Value::Object(_)) => v,
        Some(_) => return Err("config.json isn't a JSON object".into()),
        None => serde_json::json!({ "version": 1 }),
    };
    let obj = root.as_object_mut().expect("object");
    let widgets = obj.entry("widgets").or_insert_with(|| Value::Object(Map::new()));
    if !widgets.is_object() {
        *widgets = Value::Object(Map::new());
    }
    let meta = schema::meta(id).ok_or("unknown widget")?;
    let w = widgets.as_object_mut().unwrap().entry(id).or_insert_with(|| {
        serde_json::json!({ "enabled": meta.enabled, "x": meta.x, "y": meta.y, "scale": 1, "opacity": 1, "options": {} })
    });
    if !w.is_object() {
        *w = serde_json::json!({ "enabled": meta.enabled, "x": meta.x, "y": meta.y, "scale": 1, "opacity": 1, "options": {} });
    }
    let w = w.as_object_mut().unwrap();
    for (k, v) in patch {
        w.insert((*k).to_string(), v.clone());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    // Not the Tauri app's own temp name, so the two never write the same temp file.
    let tmp = path.with_extension("json.egui-tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_without_a_file() {
        let c = Config::default();
        assert!(c.widget("relative").enabled);
        assert!(!c.widget("session").enabled);
        assert_eq!(c.widget("standings").x, 24.0);
        assert_eq!(Opts::new(&c, "relative").n("ahead"), 3.0);
        assert_eq!(Opts::new(&c, "standings").s("gapMode"), "leader");
        assert!(Opts::new(&c, "relative").b("showSoF"));
        assert_eq!(c.general.theme, ThemeId::Dark);
    }

    #[test]
    fn normalizes_like_config_ts() {
        let c = normalize(&json!({
            "general": { "units": "imperial", "theme": "f1", "showWhen": "onTrack", "colors": { "f1": { "ink": "#ff0000", "edge": 3 } } },
            "widgets": {
                "relative": { "enabled": false, "x": 10, "y": 20, "ax": "r", "ay": "b", "scale": 9, "opacity": 0.1, "options": { "ahead": 5, "bad": [1] } },
                "standings": { "x": 1, "y": 2, "ax": "q", "ay": "t" },
                "inputs": "nonsense"
            }
        }));
        assert_eq!(c.general.units, Units::Imperial);
        assert_eq!(c.general.theme, ThemeId::F1);
        assert_eq!(c.general.show_when, ShowWhen::OnTrack);
        assert_eq!(c.general.colors[&ThemeId::F1].get("ink").map(String::as_str), Some("#ff0000"));
        assert!(!c.general.colors[&ThemeId::F1].contains_key("edge"));
        let r = c.widget("relative");
        assert!(!r.enabled);
        assert_eq!(r.anchor, Some((AnchorX::R, AnchorY::B)));
        assert_eq!(r.scale, SCALE_MAX);
        assert_eq!(r.opacity, OPACITY_MIN);
        assert_eq!(Opts::new(&c, "relative").n("ahead"), 5.0);
        assert!(!r.options.contains_key("bad"));
        // A broken anchor resets the offsets.
        let s = c.widget("standings");
        assert_eq!((s.x, s.y, s.anchor), (24.0, 160.0, None));
        assert!(c.widget("inputs").enabled);
    }

    #[test]
    fn writing_a_position_keeps_everything_else() {
        let dir = std::env::temp_dir().join(format!("tenths-egui-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let original = json!({
            "version": 1,
            "general": { "theme": "light", "controls": { "pitFuel": { "key": { "code": "KeyF" }, "button": null } }, "future": [1, 2] },
            "widgets": { "relative": { "enabled": true, "x": 1, "y": 2, "scale": 1.5, "opacity": 0.9, "options": { "ahead": 4 }, "display": "\\\\.\\DISPLAY2" } },
            "somethingNew": { "keep": true }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&original).unwrap()).unwrap();
        let mut store = Store::open(Some(path.clone()));
        assert_eq!(store.config.general.theme, ThemeId::Light);
        store.save_widget("relative", &[("x", json!(30)), ("y", json!(40)), ("ax", json!("r")), ("ay", json!("t"))]);
        store.save_widget("radar", &[("x", json!(5)), ("y", json!(6)), ("ax", json!("c")), ("ay", json!("m"))]);
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["somethingNew"], original["somethingNew"]);
        assert_eq!(saved["general"], original["general"]);
        let r = &saved["widgets"]["relative"];
        assert_eq!((r["x"].as_f64(), r["y"].as_f64(), r["ax"].as_str(), r["ay"].as_str()), (Some(30.0), Some(40.0), Some("r"), Some("t")));
        assert_eq!(r["scale"], json!(1.5));
        assert_eq!(r["options"], json!({ "ahead": 4 }));
        assert_eq!(r["display"], original["widgets"]["relative"]["display"]);
        assert_eq!(saved["widgets"]["radar"]["ax"], json!("c"));
        assert_eq!(store.config.widget("relative").anchor, Some((AnchorX::R, AnchorY::T)));
        assert!(!dir.join("config.json.egui-tmp").exists());

        // A file that can't be read is never written over.
        std::fs::write(&path, "{ broken").unwrap();
        assert!(write_widget(&path, "relative", &[("x", json!(1))]).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
        std::fs::remove_dir_all(&dir).ok();
    }
}
