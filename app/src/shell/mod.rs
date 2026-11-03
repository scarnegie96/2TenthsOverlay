//! The app around the panels: where things live on disk, the tray, a single
//! running instance, the corrupt-config notice, saved laps and the reference
//! lap. The Tauri shell is the spec (`src-tauri/src/lib.rs`).
//!
//! Everything the tray, a second launch or the Settings window ask of the
//! overlay window goes through `Request`: `main.rs` handles them in one place.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;
use telemetry::reference::ReferenceLap;

use crate::config;

pub mod instance;
#[cfg(windows)]
pub mod tray;

/// What asks something of the overlay window: the tray, a second launch, Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Open (or focus) the Settings window.
    OpenSettings,
    /// Open Settings on the About page (the tray's update item).
    OpenAbout,
    /// The tray's "Check for updates" / "Update to x.y.z" item.
    UpdateItem,
    ToggleEdit,
    ToggleOverlays,
    NextLayout,
    Quit,
}

/// Where the config, layouts, laps, reports and reference lap live.
#[derive(Debug, Clone)]
pub struct Paths {
    /// `config.json` (Tauri's `app_config_dir()`).
    pub config: Option<PathBuf>,
    /// Tauri's `app_data_dir()`: `laps/`, `reports/`, `reference.json`.
    pub data: Option<PathBuf>,
}

impl Paths {
    /// `--config` and `--data-dir` over the Tauri app's folders.
    pub fn new(config: Option<PathBuf>, data: Option<PathBuf>) -> Self {
        Self { config: config.or_else(config::default_path), data: data.or_else(config::data_dir) }
    }
    pub fn config_dir(&self) -> Option<PathBuf> {
        self.config.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf)
    }
    /// Saved layouts: `layouts/` next to `config.json`.
    pub fn layouts_dir(&self) -> Option<PathBuf> {
        self.config_dir().map(|d| d.join("layouts"))
    }
    pub fn laps_dir(&self) -> Option<PathBuf> {
        self.data.as_ref().map(|d| d.join("laps"))
    }
    pub fn reports_dir(&self) -> Option<PathBuf> {
        self.data.as_ref().map(|d| d.join("reports"))
    }
    pub fn reference_path(&self) -> Option<PathBuf> {
        self.data.as_ref().map(|d| d.join("reference.json"))
    }
}

// --- Corrupt config -------------------------------------------------------------

/// `config.corrupt-20261031-142501.json`, from seconds since 1970 (UTC).
pub fn corrupt_name(secs: u64) -> String {
    let (days, rest) = ((secs / 86_400) as i64, secs % 86_400);
    // Days to a calendar date (Howard Hinnant's civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("config.corrupt-{year:04}{month:02}{day:02}-{:02}{:02}{:02}.json", rest / 3600, rest / 60 % 60, rest % 60)
}

/// A config.json that isn't valid JSON is kept aside (so the next save
/// doesn't lose it for good) and the app starts fresh: returns the name it
/// was kept under, for a notice in Settings (`config_notice`).
pub fn keep_corrupt_config(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let Err(err) = serde_json::from_str::<Value>(&text) else { return None };
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let name = corrupt_name(secs);
    let kept = path.with_file_name(&name);
    match std::fs::rename(path, &kept) {
        Ok(()) => {
            log!("[tenths] config.json couldn't be read ({err}); kept it as {}", kept.display());
            Some(name)
        }
        Err(e) => {
            log!("[tenths] config.json couldn't be read ({err}) or moved aside ({e})");
            None
        }
    }
}

// --- Reference lap ----------------------------------------------------------------

/// The reference lap saved by the last import, if any.
pub fn load_saved_reference(paths: &Paths) -> Option<ReferenceLap> {
    let text = std::fs::read_to_string(paths.reference_path()?).ok()?;
    serde_json::from_str(&text).ok()
}

/// Saves the reference through a temporary file, so a crash can't leave half a file behind.
pub fn save_reference(paths: &Paths, reference: &ReferenceLap) -> Result<(), String> {
    let path = paths.reference_path().ok_or("No folder to save the reference lap in.")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string(reference).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn clear_saved_reference(paths: &Paths) -> Result<(), String> {
    if let Some(path) = paths.reference_path() {
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// --- The runner ----------------------------------------------------------------------

/// `general.saveReports`: on unless switched off (`apply_save_reports`).
pub fn save_reports_setting(config: Option<&Path>) -> bool {
    let value: Option<Value> = config.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok());
    value.and_then(|v| v["general"]["saveReports"].as_bool()).unwrap_or(true)
}

/// Where the telemetry thread saves laps (personal bests across sessions) and
/// race reports, and the reference lap saved by the last import
/// (`setup` in `lib.rs`).
pub fn setup_runner(paths: &Paths, control: &mut telemetry::runner::RunnerControl) {
    control.laps_dir = paths.laps_dir();
    control.reports_dir = paths.reports_dir();
    control.save_reports.store(save_reports_setting(paths.config.as_deref()), std::sync::atomic::Ordering::Relaxed);
    if let Some(reference) = load_saved_reference(paths) {
        control.set_reference(Some(reference));
    }
}

/// The tray's update item text.
#[cfg_attr(not(windows), allow(dead_code))]
fn update_item_text(available: Option<&str>) -> String {
    match available {
        Some(v) => format!("Update to {v}…"),
        None => "Check for updates".to_string(),
    }
}

// --- The shell itself --------------------------------------------------------------

pub struct Shell {
    pub paths: Paths,
    /// Name the broken config.json was kept under this run.
    pub config_notice: Option<String>,
    tx: Sender<Request>,
    rx: Receiver<Request>,
    #[cfg(windows)]
    tray: Option<tray::Tray>,
}

impl Shell {
    pub fn new(paths: Paths, config_notice: Option<String>) -> Self {
        let (tx, rx) = channel();
        Self {
            paths,
            config_notice,
            tx,
            rx,
            #[cfg(windows)]
            tray: tray::Tray::new().map_err(|e| log!("[tenths] no tray icon: {e}")).ok(),
        }
    }

    /// Whether there's a tray icon to come back from: Settings goes there when the overlay goes
    /// live in iRacing. Without one (macOS, Linux, or no tray available) it minimises instead.
    pub fn has_tray(&self) -> bool {
        #[cfg(windows)]
        {
            self.tray.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// A sender for threads that ask the app to do something (a second launch).
    pub fn sender(&self) -> Sender<Request> {
        self.tx.clone()
    }

    /// Everything asked since the last call: from the tray, a second launch, other threads.
    pub fn poll(&mut self) -> Vec<Request> {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut out: Vec<Request> = self.rx.try_iter().collect();
        #[cfg(windows)]
        if let Some(t) = &self.tray {
            out.extend(t.poll());
        }
        out
    }

    /// The tray's update item says "Update to <version>…" when a newer version is known, else "Check for updates".
    /// Cheap to call every frame. No tray on this platform: no-op.
    pub fn set_update_item(&mut self, available: Option<&str>) {
        #[cfg(windows)]
        if let Some(t) = &self.tray {
            t.set_update_text(&update_item_text(available));
        }
        #[cfg(not(windows))]
        let _ = available;
    }

    /// Adds each action's shortcut to the tray's item, as the Tauri app does
    /// once the bindings are known: `(action id, shortcut)`. Call it from the
    /// controls service when bindings change. No tray on this platform: no-op.
    pub fn set_tray_shortcuts(&mut self, shortcuts: &[(&str, Option<String>)]) {
        #[cfg(windows)]
        if let Some(t) = &self.tray {
            t.set_shortcuts(shortcuts);
        }
        #[cfg(not(windows))]
        let _ = shortcuts;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_names_carry_the_date_and_time() {
        assert_eq!(corrupt_name(0), "config.corrupt-19700101-000000.json");
        // 2026-10-31 14:25:01 UTC
        assert_eq!(corrupt_name(1_793_456_701), "config.corrupt-20261031-142501.json");
        // 2024-02-29 23:59:59 UTC, a leap day.
        assert_eq!(corrupt_name(1_709_251_199), "config.corrupt-20240229-235959.json");
    }

    #[test]
    fn the_tray_update_item_names_the_version() {
        assert_eq!(update_item_text(None), "Check for updates");
        assert_eq!(update_item_text(Some("0.5.1")), "Update to 0.5.1…");
    }

    #[test]
    fn a_broken_config_is_kept_aside() {
        let dir = std::env::temp_dir().join(format!("tenths-egui-shell-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        // Nothing there, or fine: nothing to do.
        assert_eq!(keep_corrupt_config(&path), None);
        std::fs::write(&path, r#"{ "version": 1 }"#).unwrap();
        assert_eq!(keep_corrupt_config(&path), None);
        assert!(path.exists());
        // Broken: moved aside under a dated name, with its contents.
        std::fs::write(&path, "{ broken").unwrap();
        let name = keep_corrupt_config(&path).expect("kept");
        assert!(name.starts_with("config.corrupt-") && name.ends_with(".json"), "{name}");
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(dir.join(&name)).unwrap(), "{ broken");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reports_are_saved_unless_switched_off() {
        let dir = std::env::temp_dir().join(format!("tenths-egui-sr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        assert!(save_reports_setting(Some(&path)));
        std::fs::write(&path, r#"{ "general": { "saveReports": false } }"#).unwrap();
        assert!(!save_reports_setting(Some(&path)));
        std::fs::write(&path, r#"{ "general": {} }"#).unwrap();
        assert!(save_reports_setting(Some(&path)));
        let paths = Paths::new(Some(path), Some(dir.clone()));
        let mut control = telemetry::runner::RunnerControl::default();
        setup_runner(&paths, &mut control);
        assert_eq!(control.laps_dir, Some(dir.join("laps")));
        assert_eq!(control.reports_dir, Some(dir.join("reports")));
        assert!(control.save_reports.load(std::sync::atomic::Ordering::Relaxed));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn folders_follow_the_config_and_data_paths() {
        let p = Paths::new(Some(PathBuf::from("/a/b/config.json")), Some(PathBuf::from("/d")));
        assert_eq!(p.layouts_dir(), Some(PathBuf::from("/a/b/layouts")));
        assert_eq!(p.laps_dir(), Some(PathBuf::from("/d/laps")));
        assert_eq!(p.reports_dir(), Some(PathBuf::from("/d/reports")));
        assert_eq!(p.reference_path(), Some(PathBuf::from("/d/reference.json")));
    }

    #[test]
    fn reference_save_load_clear() {
        let dir = std::env::temp_dir().join(format!("tenths-egui-ref-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = Paths::new(Some(dir.join("config.json")), Some(dir.clone()));
        assert!(load_saved_reference(&paths).is_none());
        let mut demo = telemetry::demo::DemoSource::new();
        let bytes = telemetry::demo::record_ibt(&mut demo, 3.2);
        let file = telemetry::ibt::IbtFile::from_bytes(bytes).unwrap();
        let lap = file.summary().laps.first().unwrap().lap;
        let reference = file.reference(lap, "Demo.ibt").unwrap();
        save_reference(&paths, &reference).unwrap();
        assert!(!dir.join("reference.json.tmp").exists());
        let back = load_saved_reference(&paths).expect("loads");
        assert_eq!(back.track_name, reference.track_name);
        clear_saved_reference(&paths).unwrap();
        assert!(load_saved_reference(&paths).is_none());
        // Clearing twice is fine.
        clear_saved_reference(&paths).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
