//! Updating Tenths itself.
//!
//! Releases are built by GitHub Actions (.github/workflows/release.yml) when a
//! version tag is pushed. Each release carries the installer, its signature
//! and `latest.json`, which says what the newest version is and where to get
//! it. The updater plugin reads `latest.json` from the newest GitHub release,
//! and only installs a download whose signature matches the public key in
//! tauri.conf.json, so nobody else can push an update.
//!
//! Tenths checks a little after starting and then every few hours (unless
//! that's switched off in Settings). It never installs on its own: Settings
//! shows the new version and you choose when. Installing closes Tenths for a
//! few seconds while the installer runs, then opens it again.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::menu::MenuItem;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Wait after starting before the first check, so it doesn't slow startup.
const FIRST_CHECK: Duration = Duration::from_secs(20);
/// Then check this often while Tenths runs.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// A newer version, as Settings shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// The release notes.
    pub notes: Option<String>,
    /// When it was published, in seconds since 1970.
    pub published: Option<i64>,
}

/// What Settings needs to draw the About page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// This version.
    pub current: String,
    /// A newer version, if the last check found one.
    pub available: Option<UpdateInfo>,
    /// Seconds since 1970 of the last check that worked.
    pub last_checked: Option<u64>,
    /// False in builds made without an update key (local builds).
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Default)]
pub struct Updates {
    pending: Mutex<Option<Update>>,
    last_checked: Mutex<Option<u64>>,
    installing: AtomicBool,
    tray_item: Mutex<Option<MenuItem<Wry>>>,
}

impl Updates {
    pub fn set_tray_item(&self, item: MenuItem<Wry>) {
        *self.tray_item.lock().unwrap() = Some(item);
    }
}

/// Builds made without the release config have no public key, so can't verify downloads.
fn enabled(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|c| c.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        published: u.date.map(|d| d.unix_timestamp()),
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn status(app: &AppHandle) -> UpdateStatus {
    let state = app.state::<Updates>();
    let available = state.pending.lock().unwrap().as_ref().map(info);
    let last_checked = *state.last_checked.lock().unwrap();
    UpdateStatus { current: app.package_info().version.to_string(), available, last_checked, enabled: enabled(app) }
}

/// Asks GitHub for the newest release. Remembers it, tells every window and
/// relabels the tray item when there's a newer one.
pub async fn check(app: &AppHandle) -> Result<Option<UpdateInfo>, String> {
    if !enabled(app) {
        return Err("This copy of Tenths was built without updates. Install it from the GitHub releases page to get them.".into());
    }
    let found = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| format!("Couldn't check for updates: {e}"))?;
    let state = app.state::<Updates>();
    *state.last_checked.lock().unwrap() = Some(now());
    let available = found.as_ref().map(info);
    *state.pending.lock().unwrap() = found;
    if let Some(item) = state.tray_item.lock().unwrap().as_ref() {
        let text = match &available {
            Some(u) => format!("Update to {}…", u.version),
            None => "Check for updates".to_string(),
        };
        let _ = item.set_text(text);
    }
    let _ = app.emit("update-status", status(app));
    Ok(available)
}

/// Downloads the update found by the last check, checks its signature and runs
/// the installer. On Windows the app closes here and the installer opens it again.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Updates>();
    let Some(update) = state.pending.lock().unwrap().clone() else {
        return Err("There's no update to install. Check again first.".into());
    };
    if state.installing.swap(true, Ordering::SeqCst) {
        return Err("The update is already being installed.".into());
    }
    let mut downloaded = 0u64;
    let result = update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit("update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await;
    state.installing.store(false, Ordering::SeqCst);
    match result {
        // Windows never gets here: the installer has taken over.
        Ok(()) => app.restart(),
        Err(e) => Err(format!("The update couldn't be installed: {e}")),
    }
}

/// Reads "check for updates automatically" from the saved settings (on unless switched off).
fn auto_check_on(app: &AppHandle) -> bool {
    let Ok(dir) = app.path().app_config_dir() else { return true };
    std::fs::read_to_string(dir.join("config.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["general"]["checkUpdates"].as_bool())
        .unwrap_or(true)
}

/// Checks a little after starting, then every few hours.
pub fn watch(app: AppHandle) {
    if !enabled(&app) {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            if auto_check_on(&app) {
                if let Err(e) = tauri::async_runtime::block_on(check(&app)) {
                    eprintln!("[updates] {e}");
                }
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
}
