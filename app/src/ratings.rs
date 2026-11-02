//! Your thumbs up or down on other drivers, from the Tauri app's
//! `driver-ratings.json` (app data folder; `src-tauri/src/ratings.rs`), for
//! the marks next to their names. Re-read when it changes, and written when
//! you rate someone (`Ratings::set`, a port of `Ratings::set` there).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::kit::Thumb;

#[derive(Default, Clone)]
pub struct Ratings {
    path: Option<PathBuf>,
    mtime: Option<SystemTime>,
    last_poll: Option<Instant>,
    by_user: HashMap<i64, Thumb>,
}

/// Customer id -> rating, from the file's text.
pub fn parse(text: &str) -> HashMap<i64, Thumb> {
    let mut out = HashMap::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return out };
    let Some(drivers) = v.get("drivers").and_then(|d| d.as_object()) else { return out };
    for (id, r) in drivers {
        let thumb = match r.get("rating").and_then(|x| x.as_str()) {
            Some("up") => Thumb::Up,
            Some("down") => Thumb::Down,
            _ => continue,
        };
        if let Ok(id) = id.parse::<i64>() {
            out.insert(id, thumb);
        }
    }
    out
}

impl Ratings {
    pub fn open(dir: Option<PathBuf>) -> Self {
        let mut r = Self { path: dir.map(|d| d.join("driver-ratings.json")), ..Default::default() };
        r.poll();
        r
    }

    /// Re-reads the file when it has changed, at most every two seconds.
    pub fn poll(&mut self) {
        let now = Instant::now();
        if self.last_poll.is_some_and(|t| now - t < Duration::from_secs(2)) {
            return;
        }
        self.last_poll = Some(now);
        let Some(path) = &self.path else { return };
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        if mtime != self.mtime {
            self.mtime = mtime;
            self.by_user = std::fs::read_to_string(path).map(|t| parse(&t)).unwrap_or_default();
        }
    }

    pub fn get(&self, user_id: i64) -> Option<Thumb> {
        if user_id <= 0 {
            return None;
        }
        self.by_user.get(&user_id).copied()
    }

    /// Sets (or with None, clears) your rating of a driver and saves it.
    pub fn set(&mut self, user_id: i64, name: &str, rating: Option<Thumb>) -> Result<(), String> {
        let path = self.path.clone().ok_or("There's nowhere to save ratings.")?;
        set_in(&path, user_id, name, rating)?;
        // Show it right away; the poll then finds the file as we left it.
        match rating {
            Some(r) => self.by_user.insert(user_id, r),
            None => self.by_user.remove(&user_id),
        };
        self.mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        Ok(())
    }
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Before writing over a file that's there but can't be read (hand-edited,
/// half-written), keep it aside, so the ratings in it aren't lost.
fn keep_unreadable(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    if serde_json::from_str::<serde_json::Value>(&text).is_ok_and(|v| v.is_object()) {
        return;
    }
    let kept = path.with_file_name(format!("driver-ratings.unreadable-{}.json", now_secs()));
    match std::fs::rename(path, &kept) {
        Ok(()) => eprintln!("[tenths] driver-ratings.json couldn't be read; kept it as {}", kept.display()),
        Err(e) => eprintln!("[tenths] driver-ratings.json couldn't be read or moved aside: {e}"),
    }
}

/// Changes one driver in the file, keeping the others (and anything else in it).
fn set_in(path: &Path, user_id: i64, name: &str, rating: Option<Thumb>) -> Result<(), String> {
    use serde_json::{json, Map, Value};
    if user_id <= 0 {
        return Err("That driver has no iRacing id.".into());
    }
    keep_unreadable(path);
    let mut root = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}));
    root["version"] = json!(1);
    if !root["drivers"].is_object() {
        root["drivers"] = Value::Object(Map::new());
    }
    let drivers = root["drivers"].as_object_mut().unwrap();
    match rating {
        Some(r) => {
            let r = if r == Thumb::Up { "up" } else { "down" };
            drivers.insert(user_id.to_string(), json!({ "name": name.trim(), "rating": r, "at": now_secs() }));
        }
        None => {
            drivers.remove(&user_id.to_string());
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.egui-tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_tauri_file() {
        let r = parse(r#"{"version":1,"drivers":{"123":{"name":"A","rating":"up","at":1},"456":{"name":"B","rating":"down","at":2},"x":{"rating":"up"},"7":{"rating":"meh"}}}"#);
        assert_eq!(r.get(&123), Some(&Thumb::Up));
        assert_eq!(r.get(&456), Some(&Thumb::Down));
        assert_eq!(r.len(), 2);
        assert!(parse("not json").is_empty());
    }

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("tenths-egui-ratings-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn sets_changes_and_clears_in_the_tauri_file() {
        let d = dir("set");
        let mut r = Ratings::open(Some(d.clone()));
        assert!(r.get(123).is_none());
        r.set(123, " Max Driver ", Some(Thumb::Up)).unwrap();
        r.set(456, "Lap Bully", Some(Thumb::Down)).unwrap();
        r.set(123, "Max Driver", Some(Thumb::Down)).unwrap();
        assert_eq!((r.get(123), r.get(456)), (Some(Thumb::Down), Some(Thumb::Down)));
        // The same shape the Tauri app reads: a fresh reader sees it.
        let text = std::fs::read_to_string(d.join("driver-ratings.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["drivers"]["123"]["name"], "Max Driver");
        assert_eq!(v["drivers"]["123"]["rating"], "down");
        assert!(v["drivers"]["123"]["at"].as_u64().unwrap() > 0);
        assert_eq!(Ratings::open(Some(d.clone())).get(456), Some(Thumb::Down));
        r.set(456, "Lap Bully", None).unwrap();
        assert!(r.get(456).is_none());
        assert_eq!(parse(&std::fs::read_to_string(d.join("driver-ratings.json")).unwrap()).len(), 1);
        assert!(r.set(0, "Nobody", Some(Thumb::Up)).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn unreadable_file_is_kept_not_overwritten() {
        let d = dir("bad");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("driver-ratings.json"), "{ not json").unwrap();
        let mut r = Ratings::open(Some(d.clone()));
        r.set(123, "Max Driver", Some(Thumb::Up)).unwrap();
        let kept: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("driver-ratings.unreadable-"))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(std::fs::read_to_string(kept[0].path()).unwrap(), "{ not json");
        assert_eq!(r.get(123), Some(Thumb::Up));
        let _ = std::fs::remove_dir_all(d);
    }
}
