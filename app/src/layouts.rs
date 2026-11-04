//! Saved layouts (`src-tauri/src/profiles.rs`, `switchProfile` / `cycleProfile`
//! in `src/lib/store.ts`): each is a JSON file in `layouts/` next to
//! config.json, the same files the Tauri app writes, holding every widget's
//! position, size, opacity, options and display.
//!
//! `general.activeProfile` in config.json names the layout the edits go into:
//! every change to a widget is copied into it (`Layouts::sync`), and switching
//! layouts puts its widgets on screen.
//!
//! For the hotkeys and the tray (called on `App` in main.rs):
//!   `App::switch_layout(id)`   put that layout on screen
//!   `App::next_layout(step)`   +1 next, -1 previous, wrapping round
//!   `App::layout_at(index)`    the layout at a position (0 = first), for Ctrl+Shift+1..9

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Store;

/// Marks a file as a Tenths layout, so importing something else fails clearly.
const FORMAT: &str = "tenths-layout";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub format: String,
    pub version: u32,
    pub name: String,
    /// Unix seconds.
    pub updated_at: u64,
    /// Same shape as `widgets` in config.json.
    pub widgets: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub updated_at: u64,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn path(dir: &Path, id: &str) -> Result<PathBuf, String> {
    // Ids are our own slugs; anything else is refused rather than touching other files.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("Not a layout id: {id}"));
    }
    Ok(dir.join(format!("{id}.json")))
}

/// A file-safe id from the name, unique in `dir`.
fn new_id(dir: &Path, name: &str) -> String {
    let mut slug: String = name.to_ascii_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    slug = slug.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
    slug.truncate(40);
    if slug.is_empty() {
        slug = "layout".into();
    }
    let mut id = slug.clone();
    let mut n = 2;
    while dir.join(format!("{id}.json")).exists() {
        id = format!("{slug}-{n}");
        n += 1;
    }
    id
}

fn parse(text: &str) -> Result<Profile, String> {
    let p: Profile = serde_json::from_str(text).map_err(|_| "That isn't a 2Tenths layout file.".to_string())?;
    if p.format != FORMAT {
        return Err("That isn't a 2Tenths layout file.".into());
    }
    if p.version > VERSION {
        return Err("That layout is from a newer version of 2Tenths.".into());
    }
    if !p.widgets.is_object() {
        return Err("That layout file has no widgets in it.".into());
    }
    Ok(p)
}

fn write(dir: &Path, id: &str, p: &Profile) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let target = path(dir, id)?;
    // Not the Tauri app's own temp name, so the two never write the same temp file.
    let tmp = target.with_extension("json.egui-tmp");
    let text = serde_json::to_string_pretty(p).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &target).map_err(|e| e.to_string())
}

pub fn read(dir: &Path, id: &str) -> Result<Profile, String> {
    let text = fs::read_to_string(path(dir, id)?).map_err(|_| "That layout no longer exists.".to_string())?;
    parse(&text)
}

/// Every layout, by name.
pub fn list(dir: &Path) -> Vec<Summary> {
    let mut out: Vec<Summary> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let p = entry.path();
            if p.extension()? != "json" {
                return None;
            }
            let id = p.file_stem()?.to_str()?.to_string();
            let profile = parse(&fs::read_to_string(&p).ok()?).ok()?;
            Some(Summary { id, name: profile.name, updated_at: profile.updated_at })
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// Creates a layout (no id) or updates one. A missing name keeps the current one.
pub fn save(dir: &Path, id: Option<&str>, name: Option<&str>, widgets: Value) -> Result<String, String> {
    if !widgets.is_object() {
        return Err("No widgets to save.".into());
    }
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    let (id, name) = match id {
        Some(id) => {
            let existing = read(dir, id)?;
            (id.to_string(), name.map(String::from).unwrap_or(existing.name))
        }
        None => {
            let name = name.ok_or("Give the layout a name.")?.to_string();
            (new_id(dir, &name), name)
        }
    };
    let profile = Profile { format: FORMAT.into(), version: VERSION, name, updated_at: now(), widgets };
    write(dir, &id, &profile)?;
    Ok(id)
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    fs::remove_file(path(dir, id)?).map_err(|e| e.to_string())
}

pub fn export(dir: &Path, id: &str, to: &Path) -> Result<(), String> {
    let p = read(dir, id)?;
    fs::write(to, serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Copies a layout file in under a new id, so it never overwrites one of ours.
pub fn import(dir: &Path, from: &Path) -> Result<String, String> {
    let text = fs::read_to_string(from).map_err(|e| e.to_string())?;
    let p = parse(&text)?;
    let id = new_id(dir, &p.name);
    write(dir, &id, &Profile { updated_at: now(), ..p })?;
    Ok(id)
}

/// Which layout a shortcut asks for (`cycleProfile`): at a position, or
/// `step` places from the active one (wrapping; no active layout counts as
/// before the first). None when there's nothing to switch to or it's the
/// active one already.
pub fn cycle_target(ids: &[&str], active: Option<&str>, step: Option<i32>, index: Option<usize>) -> Option<usize> {
    if ids.is_empty() {
        return None;
    }
    let i = match index {
        Some(i) if i >= ids.len() => return None,
        Some(i) => i,
        None => {
            let at = active.and_then(|a| ids.iter().position(|id| *id == a)).map(|a| a as i64).unwrap_or(-1);
            (at + step.unwrap_or(1) as i64).rem_euclid(ids.len() as i64) as usize
        }
    };
    (Some(ids[i]) != active).then_some(i)
}

/// The saved layouts and which one is active.
pub struct Layouts {
    dir: Option<PathBuf>,
    pub list: Vec<Summary>,
    dir_mtime: Option<SystemTime>,
    last_poll: Option<Instant>,
    /// Widgets as last written to (or read from) each layout, so unchanged ones aren't rewritten.
    saved: HashMap<String, String>,
    last_edits: u64,
}

impl Layouts {
    /// `layouts/` next to config.json (the Tauri app's `app_config_dir()/layouts`).
    pub fn open(config_path: Option<&Path>, store: &Store) -> Self {
        let dir = config_path.and_then(|p| p.parent()).map(|d| d.join("layouts"));
        let mut l = Self { dir, list: Vec::new(), dir_mtime: None, last_poll: None, saved: HashMap::new(), last_edits: store.edits };
        l.relist();
        l
    }

    fn relist(&mut self) {
        let Some(dir) = &self.dir else { return };
        self.dir_mtime = fs::metadata(dir).and_then(|m| m.modified()).ok();
        self.list = list(dir);
    }

    /// Re-reads the list when the folder has changed (at most once a second).
    pub fn poll(&mut self) {
        let now = Instant::now();
        if self.last_poll.is_some_and(|t| now - t < Duration::from_secs(1)) {
            return;
        }
        self.last_poll = Some(now);
        let Some(dir) = &self.dir else { return };
        if fs::metadata(dir).and_then(|m| m.modified()).ok() != self.dir_mtime {
            self.relist();
        }
    }

    pub fn name_of(&self, id: &str) -> Option<&str> {
        self.list.iter().find(|p| p.id == id).map(|p| p.name.as_str())
    }

    fn dir(&self) -> Result<&Path, String> {
        self.dir.as_deref().ok_or_else(|| "There's no folder for layouts.".to_string())
    }

    /// Puts a saved layout on screen and makes it the active one.
    pub fn switch(&mut self, store: &mut Store, id: &str) -> Result<(), String> {
        let profile = read(self.dir()?, id)?;
        store.set_layout(&profile.widgets, Some(id));
        // What the layout holds, normalised, so the switch itself isn't "an edit".
        self.saved.insert(id.to_string(), store.widgets_json().to_string());
        self.last_edits = store.edits;
        Ok(())
    }

    /// The layout `step` places on (or at a position); returns its id when it switched.
    pub fn cycle(&mut self, store: &mut Store, step: Option<i32>, index: Option<usize>) -> Option<String> {
        self.relist();
        let ids: Vec<&str> = self.list.iter().map(|p| p.id.as_str()).collect();
        let i = cycle_target(&ids, store.config.general.active_profile.as_deref(), step, index)?;
        let id = self.list[i].id.clone();
        self.switch(store, &id).ok()?;
        Some(id)
    }

    /// Copies edits made since the last call into the active layout. If it was
    /// deleted elsewhere, carries on with an unsaved layout.
    pub fn sync(&mut self, store: &mut Store) {
        if store.edits == self.last_edits {
            return;
        }
        self.last_edits = store.edits;
        let Some(id) = store.config.general.active_profile.clone() else { return };
        let widgets = store.widgets_json();
        let text = widgets.to_string();
        if self.saved.get(&id) == Some(&text) {
            return;
        }
        let Ok(dir) = self.dir() else { return };
        match save(dir, Some(&id), None, widgets) {
            Ok(_) => {
                self.saved.insert(id, text);
                self.relist();
            }
            Err(e) => {
                log!("[tenths] couldn't save layout {id}: {e}");
                self.saved.remove(&id);
                if !self.dir.as_deref().is_some_and(|d| path(d, &id).is_ok_and(|p| p.exists())) {
                    store.set_active_profile(None);
                }
            }
        }
    }

    /// Saves what's on screen as a new layout and makes it the active one.
    pub fn save_new(&mut self, store: &mut Store, name: &str) -> Result<String, String> {
        let widgets = store.widgets_json();
        let id = save(self.dir()?, None, Some(name), widgets.clone())?;
        self.saved.insert(id.clone(), widgets.to_string());
        store.set_active_profile(Some(&id));
        self.relist();
        Ok(id)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), String> {
        let widgets = read(self.dir()?, id)?.widgets;
        save(self.dir()?, Some(id), Some(name), widgets)?;
        self.relist();
        Ok(())
    }

    /// The widgets stay as they are, just no longer saved anywhere if it was active.
    pub fn delete(&mut self, store: &mut Store, id: &str) -> Result<(), String> {
        delete(self.dir()?, id)?;
        self.saved.remove(id);
        if store.config.general.active_profile.as_deref() == Some(id) {
            store.set_active_profile(None);
        }
        self.relist();
        Ok(())
    }

    pub fn export(&self, id: &str, to: &Path) -> Result<(), String> {
        export(self.dir()?, id, to)
    }

    pub fn import(&mut self, from: &Path) -> Result<String, String> {
        let id = import(self.dir()?, from)?;
        self.relist();
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tenths-egui-layouts-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn save_list_rename_and_delete() {
        let dir = temp_dir("crud");
        let a = save(&dir, None, Some("Sprint race"), json!({ "relative": { "x": 1 } })).unwrap();
        let b = save(&dir, None, Some("Sprint race"), json!({})).unwrap();
        assert_eq!(a, "sprint-race");
        assert_eq!(b, "sprint-race-2");
        save(&dir, Some(&b), Some("Endurance"), json!({ "fuel": {} })).unwrap();
        save(&dir, Some(&a), None, json!({ "relative": { "x": 2 } })).unwrap();

        let names: Vec<_> = list(&dir).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Endurance", "Sprint race"]);
        assert_eq!(read(&dir, &a).unwrap().widgets["relative"]["x"], 2);

        delete(&dir, &a).unwrap();
        assert_eq!(list(&dir).len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn import_checks_the_file_and_never_overwrites() {
        let dir = temp_dir("import");
        let id = save(&dir, None, Some("Night"), json!({ "radar": {} })).unwrap();
        let out = dir.join("shared.layout.json");
        export(&dir, &id, &out).unwrap();
        let copy = import(&dir, &out).unwrap();
        assert_eq!(copy, "night-2");

        let junk = dir.join("junk.json");
        fs::write(&junk, r#"{ "hello": 1 }"#).unwrap();
        assert!(import(&dir, &junk).is_err());
        assert!(read(&dir, "../config").is_err());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cycles_like_the_overlay_did() {
        let ids = ["a", "b", "c"];
        // From the active one, wrapping both ways.
        assert_eq!(cycle_target(&ids, Some("a"), Some(1), None), Some(1));
        assert_eq!(cycle_target(&ids, Some("c"), Some(1), None), Some(0));
        assert_eq!(cycle_target(&ids, Some("a"), Some(-1), None), Some(2));
        assert_eq!(cycle_target(&ids, Some("a"), None, None), Some(1));
        // No active layout counts as before the first.
        assert_eq!(cycle_target(&ids, None, Some(1), None), Some(0));
        assert_eq!(cycle_target(&ids, Some("gone"), Some(1), None), Some(0));
        // By position: out of range or already active is nothing.
        assert_eq!(cycle_target(&ids, Some("a"), None, Some(2)), Some(2));
        assert_eq!(cycle_target(&ids, Some("a"), None, Some(0)), None);
        assert_eq!(cycle_target(&ids, Some("a"), None, Some(3)), None);
        assert_eq!(cycle_target(&[], None, Some(1), None), None);
        assert_eq!(cycle_target(&["only"], Some("only"), Some(1), None), None);
    }

    #[test]
    fn switching_applies_widgets_and_edits_save_into_the_active_layout() {
        let dir = temp_dir("switch");
        let cfg = dir.join("config.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &cfg,
            serde_json::to_string(&json!({
                "version": 1,
                "general": { "theme": "light", "future": 1 },
                "widgets": { "relative": { "enabled": true, "x": 1, "y": 2, "scale": 1, "opacity": 1, "options": {} } },
                "other": [1]
            }))
            .unwrap(),
        )
        .unwrap();
        let mut store = Store::open(Some(cfg.clone()));
        let mut layouts = Layouts::open(Some(&cfg), &store);
        assert!(layouts.list.is_empty());

        // Save what's on screen as a layout; it becomes the active one.
        let day = layouts.save_new(&mut store, "Day").unwrap();
        assert_eq!(store.config.general.active_profile.as_deref(), Some("day"));
        // A second layout with the radar on, saved from a hand-made widgets value.
        let mut night_widgets = store.widgets_json();
        night_widgets["radar"]["enabled"] = json!(false);
        night_widgets["relative"]["enabled"] = json!(false);
        night_widgets["relative"]["display"] = json!("\\\\.\\DISPLAY2");
        let night = save(&dir.join("layouts"), None, Some("Night"), night_widgets).unwrap();
        layouts.relist();
        assert_eq!(layouts.list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Day", "Night"]);

        // Switch: widgets come from the layout, config.json keeps everything else.
        layouts.switch(&mut store, &night).unwrap();
        assert!(!store.config.widget("relative").enabled);
        assert_eq!(store.config.widget("relative").display.as_deref(), Some("\\\\.\\DISPLAY2"));
        let saved: Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
        assert_eq!(saved["general"]["activeProfile"], "night");
        assert_eq!(saved["general"]["theme"], "light");
        assert_eq!(saved["general"]["future"], 1);
        assert_eq!(saved["other"], json!([1]));
        assert_eq!(saved["widgets"]["relative"]["enabled"], false);
        assert_eq!(saved["widgets"]["relative"]["display"], "\\\\.\\DISPLAY2");
        // Switching alone doesn't rewrite the layout file.
        let before = read(&dir.join("layouts"), &night).unwrap().updated_at;
        layouts.sync(&mut store);
        assert_eq!(read(&dir.join("layouts"), &night).unwrap().updated_at, before);

        // An edit goes into the active layout, not the other one.
        store.save_widget("tires", &[("enabled", json!(true)), ("x", json!(77))]);
        layouts.sync(&mut store);
        let night_file = read(&dir.join("layouts"), &night).unwrap();
        assert_eq!(night_file.widgets["tires"]["x"].as_f64(), Some(77.0));
        assert_eq!(night_file.name, "Night");
        assert_ne!(read(&dir.join("layouts"), &day).unwrap().widgets["tires"]["x"].as_f64(), Some(77.0));

        // cycle() goes to the next by name, and by position.
        assert_eq!(layouts.cycle(&mut store, Some(1), None).as_deref(), Some("day"));
        assert_eq!(store.config.general.active_profile.as_deref(), Some("day"));
        assert_eq!(layouts.cycle(&mut store, None, Some(1)).as_deref(), Some("night"));
        assert_eq!(layouts.cycle(&mut store, None, Some(1)), None);

        // Deleting the active layout leaves the widgets and clears the active one.
        layouts.delete(&mut store, &night).unwrap();
        assert_eq!(store.config.general.active_profile, None);
        assert_eq!(layouts.list.len(), 1);
        let saved: Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
        assert_eq!(saved["general"]["activeProfile"], Value::Null);

        // An active layout deleted behind our back: the next edit drops it.
        layouts.switch(&mut store, &day).unwrap();
        fs::remove_file(dir.join("layouts/day.json")).unwrap();
        store.save_widget("tires", &[("x", json!(5))]);
        layouts.sync(&mut store);
        assert_eq!(store.config.general.active_profile, None);
        let _ = fs::remove_dir_all(dir);
    }
}
