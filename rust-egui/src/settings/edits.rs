//! Settings edits `config.json` the way the Tauri app would, without losing
//! anything that isn't Settings' own: every change is a path set or removed
//! in the JSON, applied to the file as it is on disk (not to what was read
//! earlier), written through a temporary file and a rename. Unknown fields
//! and other windows' changes survive. The overlay window picks the file up
//! (and the in-memory `Store` is updated at once).
//!
//! Writes wait until the mouse button is up, so dragging a slider doesn't
//! write the file sixty times a second.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Map, Value};

use crate::config::{self, Store};
use crate::schema::{self, Def, Meta};

#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    Set(Vec<String>, Value),
    Remove(Vec<String>),
}

pub fn path_of(p: &[&str]) -> Vec<String> {
    p.iter().map(|s| (*s).to_string()).collect()
}

/// Sets the value at a JSON path, making objects on the way (replacing anything that isn't one).
fn set_at(root: &mut Value, path: &[String], value: Value) {
    let mut cur = root;
    for key in &path[..path.len() - 1] {
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        cur = cur.as_object_mut().unwrap().entry(key.clone()).or_insert_with(|| Value::Object(Map::new()));
    }
    if !cur.is_object() {
        *cur = Value::Object(Map::new());
    }
    cur.as_object_mut().unwrap().insert(path[path.len() - 1].clone(), value);
}

fn remove_at(root: &mut Value, path: &[String]) {
    let mut cur = root;
    for key in &path[..path.len() - 1] {
        match cur.get_mut(key) {
            Some(next) => cur = next,
            None => return,
        }
    }
    if let Some(o) = cur.as_object_mut() {
        o.remove(&path[path.len() - 1]);
    }
}

pub fn apply(root: &mut Value, edits: &[Edit]) {
    for e in edits {
        match e {
            Edit::Set(p, v) => set_at(root, p, v.clone()),
            Edit::Remove(p) => remove_at(root, p),
        }
    }
}

/// A widget as the Tauri app saves a fresh one: `defaultWidgetConfig`.
pub fn default_widget_json(m: &Meta) -> Value {
    let options: Map<String, Value> = m
        .options
        .iter()
        .map(|(k, d)| {
            (
                (*k).to_string(),
                match d {
                    Def::B(b) => json!(b),
                    Def::N(n) => json!(n),
                    Def::S(s) => json!(s),
                },
            )
        })
        .collect();
    json!({ "enabled": m.enabled, "x": m.x, "y": m.y, "scale": 1, "opacity": 1, "options": options })
}

/// `widgets` with every widget at its defaults.
pub fn default_widgets_json() -> Value {
    Value::Object(schema::WIDGETS.iter().map(|m| (m.id.to_string(), default_widget_json(m))).collect())
}

/// A saved layout's `widgets` over the defaults, so a layout from an older
/// version gets the options added since. Unknown fields (`display`) stay.
pub fn widgets_over_defaults(saved: &Value) -> Value {
    let mut out = Map::new();
    for m in schema::WIDGETS {
        let mut w = default_widget_json(m);
        if let Some(s) = saved.get(m.id).and_then(Value::as_object) {
            for (k, v) in s {
                if k == "options" {
                    if let (Some(o), Some(d)) = (v.as_object(), w["options"].as_object_mut()) {
                        d.extend(o.clone());
                    }
                } else {
                    w[k] = v.clone();
                }
            }
        }
        out.insert(m.id.to_string(), w);
    }
    Value::Object(out)
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// What Settings has of config.json, and what it still has to write.
pub struct Editor {
    path: Option<PathBuf>,
    layouts_dir: Option<PathBuf>,
    /// The whole file, as last read or written, plus edits not yet on disk.
    pub raw: Value,
    pending: Vec<Edit>,
    mtime: Option<SystemTime>,
    last_check: Option<Instant>,
    pub error: Option<String>,
}

fn read_file(path: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(v @ Value::Object(_)) => Ok(v),
            Ok(_) => Err(format!("{} isn't a JSON object", path.display())),
            Err(e) => Err(format!("{} couldn't be read: {e}", path.display())),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({ "version": 1 })),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

impl Editor {
    pub fn new(path: Option<PathBuf>, layouts_dir: Option<PathBuf>) -> Self {
        let mut e = Self { path, layouts_dir, raw: json!({ "version": 1 }), pending: Vec::new(), mtime: None, last_check: None, error: None };
        e.reload();
        e
    }

    fn reload(&mut self) {
        let Some(path) = &self.path else { return };
        self.mtime = mtime(path);
        match read_file(path) {
            Ok(v) => {
                self.raw = v;
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Makes an edit: at once in memory (and in the overlay's `Store`), on disk when the mouse is up.
    pub fn edit(&mut self, store: &mut Store, e: Edit) {
        apply(&mut self.raw, std::slice::from_ref(&e));
        self.pending.push(e);
        store.config = config::normalize(&self.raw);
    }

    pub fn set(&mut self, store: &mut Store, path: &[&str], value: Value) {
        self.edit(store, Edit::Set(path_of(path), value));
    }

    pub fn remove(&mut self, store: &mut Store, path: &[&str]) {
        self.edit(store, Edit::Remove(path_of(path)));
    }

    /// Picks up another window's saves while there's nothing of ours waiting.
    pub fn refresh(&mut self, store: &mut Store) {
        if !self.pending.is_empty() {
            return;
        }
        let now = Instant::now();
        if self.last_check.is_some_and(|t| now - t < Duration::from_millis(500)) {
            return;
        }
        self.last_check = Some(now);
        if let Some(path) = &self.path {
            if mtime(path) != self.mtime {
                self.reload();
                store.config = config::normalize(&self.raw);
            }
        }
    }

    /// Writes what's pending, onto the file as it is now. Not while the mouse is down.
    pub fn flush(&mut self, mouse_down: bool) {
        if self.pending.is_empty() || mouse_down {
            return;
        }
        let edits = std::mem::take(&mut self.pending);
        let Some(path) = self.path.clone() else { return };
        match write_edits(&path, &edits) {
            Ok(root) => {
                self.mtime = mtime(&path);
                self.error = None;
                self.raw = root;
                // Changes to the widgets go into the layout they belong to.
                if edits.iter().any(|e| matches!(e, Edit::Set(p, _) | Edit::Remove(p) if p.first().map(String::as_str) == Some("widgets"))) {
                    self.save_into_active_layout();
                }
            }
            Err(e) => {
                eprintln!("[tenths-egui] couldn't save settings: {e}");
                self.error = Some(format!("Couldn't save: {e}"));
            }
        }
    }

    fn save_into_active_layout(&self) {
        let (Some(dir), Some(id)) = (&self.layouts_dir, self.raw["general"]["activeProfile"].as_str()) else { return };
        if let Err(e) = super::layouts_io::save(dir, Some(id), None, self.raw["widgets"].clone()) {
            eprintln!("[tenths-egui] couldn't update layout {id}: {e}");
        }
    }
}

/// Applies edits to the file on disk and writes it back (temporary file, then rename).
/// Refuses to touch a file that's there but can't be read.
pub fn write_edits(path: &Path, edits: &[Edit]) -> Result<Value, String> {
    let mut root = read_file(path)?;
    apply(&mut root, edits);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    // Not the Tauri app's own temp name, so the two never write the same temp file.
    let tmp = path.with_extension("json.egui-tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tenths-egui-edits-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn paths_set_and_remove() {
        let mut v = json!({ "general": { "theme": "dark", "future": [1] } });
        apply(
            &mut v,
            &[
                Edit::Set(path_of(&["general", "units"]), json!("imperial")),
                Edit::Set(path_of(&["widgets", "relative", "options", "ahead"]), json!(5)),
                Edit::Remove(path_of(&["general", "theme"])),
                Edit::Remove(path_of(&["nothing", "here"])),
            ],
        );
        assert_eq!(v, json!({ "general": { "units": "imperial", "future": [1] }, "widgets": { "relative": { "options": { "ahead": 5 } } } }));
        // A value of the wrong type on the way is replaced.
        apply(&mut v, &[Edit::Set(path_of(&["general", "units", "x"]), json!(1))]);
        assert_eq!(v["general"]["units"], json!({ "x": 1 }));
    }

    #[test]
    fn edits_keep_everything_else_and_other_writers_changes() {
        let dir = temp("keep");
        let path = dir.join("config.json");
        let original = json!({
            "version": 1,
            "general": { "theme": "light", "controls": { "pitFuel": { "key": null } }, "future": [1, 2] },
            "widgets": { "relative": { "enabled": true, "x": 1, "y": 2, "display": "D2", "options": { "ahead": 4 } } },
            "somethingNew": { "keep": true }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&original).unwrap()).unwrap();
        let mut store = Store::open(Some(path.clone()));
        let mut ed = Editor::new(Some(path.clone()), None);
        ed.set(&mut store, &["general", "units"], json!("imperial"));
        ed.set(&mut store, &["widgets", "relative", "options", "behind"], json!(2));
        // In memory at once, before it's written.
        assert_eq!(store.config.general.units, config::Units::Imperial);
        assert!(ed.has_pending());
        // Another window saves in between: the write goes onto the file as it is now.
        let mut other = original.clone();
        other["widgets"]["relative"]["x"] = json!(99);
        std::fs::write(&path, serde_json::to_string_pretty(&other).unwrap()).unwrap();
        // Not while the mouse is down.
        ed.flush(true);
        assert!(ed.has_pending());
        ed.flush(false);
        assert!(!ed.has_pending());
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["somethingNew"], original["somethingNew"]);
        assert_eq!(saved["general"]["controls"], original["general"]["controls"]);
        assert_eq!(saved["general"]["future"], original["general"]["future"]);
        assert_eq!(saved["general"]["units"], json!("imperial"));
        assert_eq!(saved["widgets"]["relative"]["x"], json!(99));
        assert_eq!(saved["widgets"]["relative"]["display"], json!("D2"));
        assert_eq!(saved["widgets"]["relative"]["options"], json!({ "ahead": 4, "behind": 2 }));
        assert!(!dir.join("config.json.egui-tmp").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unreadable_file_is_never_written_over() {
        let dir = temp("broken");
        let path = dir.join("config.json");
        std::fs::write(&path, "{ broken").unwrap();
        let mut store = Store::open(Some(path.clone()));
        let mut ed = Editor::new(Some(path.clone()), None);
        assert!(ed.error.is_some());
        ed.set(&mut store, &["general", "units"], json!("imperial"));
        ed.flush(false);
        assert!(ed.error.as_deref().unwrap().starts_with("Couldn't save"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_file_starts_as_version_1_and_edits_save_into_the_active_layout() {
        let dir = temp("layout");
        let path = dir.join("config.json");
        let layouts = dir.join("layouts");
        let id = super::super::layouts_io::save(&layouts, None, Some("Sprint"), json!({})).unwrap();
        let mut store = Store::open(Some(path.clone()));
        let mut ed = Editor::new(Some(path.clone()), Some(layouts.clone()));
        ed.set(&mut store, &["general", "activeProfile"], json!(id));
        ed.set(&mut store, &["widgets", "radar", "enabled"], json!(true));
        ed.flush(false);
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["version"], json!(1));
        let layout = super::super::layouts_io::read(&layouts, &id).unwrap();
        assert_eq!(layout.widgets["radar"]["enabled"], json!(true));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn layouts_get_the_defaults_for_what_they_lack() {
        let w = widgets_over_defaults(&json!({ "relative": { "x": 5, "display": "D2", "options": { "ahead": 7 } } }));
        assert_eq!(w["relative"]["x"], json!(5));
        assert_eq!(w["relative"]["display"], json!("D2"));
        assert_eq!(w["relative"]["options"]["ahead"], json!(7));
        assert_eq!(w["relative"]["options"]["behind"], json!(3.0));
        assert_eq!(w["session"]["enabled"], json!(false));
        assert_eq!(w.as_object().unwrap().len(), schema::WIDGETS.len());
    }
}
