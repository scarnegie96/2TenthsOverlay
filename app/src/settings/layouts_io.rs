//! Saved layouts: each one is a JSON file in the config folder's `layouts/`
//! holding every widget's position, size, opacity and options, so they can be
//! switched between, backed up or shared. The same files, in the same format,
//! as `src-tauri/src/profiles.rs` (copied from it: both apps read each
//! other's layouts). The multi-display agent may bring a shared module; this
//! one can then go.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
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
    let mut slug: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
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
    let tmp = target.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(p).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &target).map_err(|e| e.to_string())
}

pub fn read(dir: &Path, id: &str) -> Result<Profile, String> {
    let text = fs::read_to_string(path(dir, id)?).map_err(|_| "That layout no longer exists.".to_string())?;
    parse(&text)
}

/// Every layout, by name.
pub fn list(dir: &Path) -> Vec<ProfileSummary> {
    let mut out: Vec<ProfileSummary> = fs::read_dir(dir)
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
            Some(ProfileSummary { id, name: profile.name, updated_at: profile.updated_at })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tenths-profiles-{tag}-{}", std::process::id()));
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
}
