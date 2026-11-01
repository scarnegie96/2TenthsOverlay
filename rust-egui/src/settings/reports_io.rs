//! Saved race reports (telemetry/src/report.rs writes them): listing,
//! reading and deleting for Settings → Races. Copied from
//! `src-tauri/src/reports.rs`: the same files.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// What the list shows for each race, read from its file.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportSummary {
    pub id: String,
    pub started_at: u64,
    pub track: String,
    pub track_config: String,
    pub car: String,
    pub session_type: String,
    pub official: bool,
    pub finished: bool,
    /// Your class position at the end, and how many were in your class.
    pub class_position: i64,
    pub class_cars: usize,
    pub laps: i64,
}

/// Report ids are the file names the telemetry writes: letters, digits, `-` and `_`.
fn path(dir: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.len() > 120 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Not a race report.".into());
    }
    Ok(dir.join(format!("{id}.json")))
}

fn summary(id: String, v: &Value) -> ReportSummary {
    let player = v["playerCarIdx"].as_i64().unwrap_or(-1);
    let class = v["playerClassId"].as_i64().unwrap_or(-1);
    let cars = v["cars"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let me = cars.iter().find(|c| c["carIdx"].as_i64() == Some(player));
    ReportSummary {
        id,
        started_at: v["startedAt"].as_u64().unwrap_or(0),
        track: v["track"].as_str().unwrap_or("").to_string(),
        track_config: v["trackConfig"].as_str().unwrap_or("").to_string(),
        car: v["car"].as_str().unwrap_or("").to_string(),
        session_type: v["sessionType"].as_str().unwrap_or("").to_string(),
        official: v["official"].as_bool().unwrap_or(false),
        finished: v["finished"].as_bool().unwrap_or(false),
        class_position: me.and_then(|c| c["finishClassPosition"].as_i64()).unwrap_or(0),
        class_cars: cars.iter().filter(|c| c["classId"].as_i64() == Some(class)).count(),
        laps: me.and_then(|c| c["lapsCompleted"].as_i64()).unwrap_or(0),
    }
}

/// Every saved race, newest first. Files that can't be read are skipped.
pub fn list(dir: &Path) -> Vec<ReportSummary> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<ReportSummary> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let id = p.file_name()?.to_str()?.strip_suffix(".json")?.to_string();
            let v: Value = serde_json::from_str(&fs::read_to_string(&p).ok()?).ok()?;
            Some(summary(id, &v))
        })
        .collect();
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at).then_with(|| b.id.cmp(&a.id)));
    out
}

pub fn read(dir: &Path, id: &str) -> Result<Value, String> {
    let text = fs::read_to_string(path(dir, id)?).map_err(|_| "That race report is gone.".to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("That race report can't be read: {e}"))
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    fs::remove_file(path(dir, id)?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_reads_and_deletes() {
        let dir = std::env::temp_dir().join(format!("tenths-reports-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let report = serde_json::json!({
            "startedAt": 100, "track": "Spa", "trackConfig": "GP", "car": "GT3", "sessionType": "Race",
            "official": true, "finished": true, "playerCarIdx": 3, "playerClassId": 2,
            "cars": [
                {"carIdx": 3, "classId": 2, "finishClassPosition": 4, "lapsCompleted": 20},
                {"carIdx": 5, "classId": 2}, {"carIdx": 7, "classId": 9}
            ]
        });
        fs::write(dir.join("b.json"), report.to_string()).unwrap();
        fs::write(dir.join("a.json"), serde_json::json!({"startedAt": 50}).to_string()).unwrap();
        fs::write(dir.join("broken.json"), "{").unwrap();
        let all = list(&dir);
        assert_eq!(all.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
        assert_eq!((all[0].class_position, all[0].class_cars, all[0].laps), (4, 2, 20));
        assert_eq!(read(&dir, "b").unwrap()["track"], "Spa");
        assert!(read(&dir, "../b").is_err() && read(&dir, "nope").is_err());
        delete(&dir, "b").unwrap();
        assert_eq!(list(&dir).len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }
}
