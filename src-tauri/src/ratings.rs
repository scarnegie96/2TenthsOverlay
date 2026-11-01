//! Your thumbs up or down on other drivers, given after a race, kept in
//! `driver-ratings.json` in the app data folder and shown next to their name
//! in the Standings and Relative from then on.
//!
//! Drivers are keyed by iRacing customer id, which stays the same when they
//! change their display name; the name is kept for reading the file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriverRating {
    pub name: String,
    pub rating: Rating,
    /// When it was given, in seconds since 1970.
    pub at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default = "version")]
    version: u32,
    /// Customer id (as a string, so the file is plain JSON) -> rating.
    #[serde(default)]
    drivers: BTreeMap<String, DriverRating>,
}

fn version() -> u32 {
    1
}

#[derive(Default)]
pub struct Ratings {
    lock: Mutex<()>,
}

fn path(dir: &Path) -> PathBuf {
    dir.join("driver-ratings.json")
}

fn read(dir: &Path) -> File {
    fs::read_to_string(path(dir)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn write(dir: &Path, file: &File) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let p = path(dir);
    let tmp = p.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

impl Ratings {
    /// Every rating, by customer id.
    pub fn all(&self, dir: &Path) -> BTreeMap<String, DriverRating> {
        let _guard = self.lock.lock().unwrap();
        read(dir).drivers
    }

    /// Sets (or with None, clears) a driver's rating. Returns them all.
    pub fn set(&self, dir: &Path, user_id: i64, name: &str, rating: Option<Rating>) -> Result<BTreeMap<String, DriverRating>, String> {
        if user_id <= 0 {
            return Err("That driver has no iRacing id.".into());
        }
        let _guard = self.lock.lock().unwrap();
        let mut file = read(dir);
        file.version = 1;
        let key = user_id.to_string();
        match rating {
            Some(rating) => {
                let at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                file.drivers.insert(key, DriverRating { name: name.trim().to_string(), rating, at });
            }
            None => {
                file.drivers.remove(&key);
            }
        }
        write(dir, &file)?;
        Ok(file.drivers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_changes_and_clears() {
        let dir = std::env::temp_dir().join(format!("tenths-ratings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let r = Ratings::default();
        assert!(r.all(&dir).is_empty());
        r.set(&dir, 123, "Max Driver", Some(Rating::Up)).unwrap();
        r.set(&dir, 456, "Lap Bully", Some(Rating::Down)).unwrap();
        let all = r.set(&dir, 123, "Max Driver", Some(Rating::Down)).unwrap();
        assert_eq!(all["123"].rating, Rating::Down);
        assert_eq!(all.len(), 2);
        let all = r.set(&dir, 456, "Lap Bully", None).unwrap();
        assert_eq!(all.len(), 1);
        // Read back from disk.
        assert_eq!(r.all(&dir)["123"].name, "Max Driver");
        assert!(r.set(&dir, 0, "Nobody", Some(Rating::Up)).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
