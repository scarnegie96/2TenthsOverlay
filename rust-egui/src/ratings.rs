//! Your thumbs up or down on other drivers, from the Tauri app's
//! `driver-ratings.json` (app data folder; `src-tauri/src/ratings.rs`), for
//! the marks next to their names. Read only, and re-read when it changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use crate::kit::Thumb;

#[derive(Default)]
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
}
