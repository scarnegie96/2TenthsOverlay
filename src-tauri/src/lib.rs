//! Tauri shell: windows, tray, hotkeys, config storage, and the bridge from
//! the telemetry thread to the web views.

mod controllers;
mod controls;
mod displays;
mod profiles;
mod ratings;
mod reports;
mod stream;
mod updates;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

use displays::{Display, Displays};
use tauri_plugin_global_shortcut::ShortcutState;
use telemetry::ibt::IbtSummary;
use telemetry::lapfile;
use telemetry::reference::ReferenceLap;
use telemetry::{FieldPayload, PitCommand, ReferencePayload, RunnerControl, SessionPayload, Sink, StatusPayload, TickPayload};

/// The overlay window on the primary display; others are `overlay-2`… (displays.rs).
/// App-wide jobs (layout and pit shortcuts) go to this one only.
pub(crate) const OVERLAY: &str = "overlay";
const SETTINGS: &str = "settings";

/// What a freshly opened window needs to render immediately.
#[derive(Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    status: Option<StatusPayload>,
    session: Option<SessionPayload>,
    reference: Option<ReferencePayload>,
    edit_mode: bool,
    overlay_visible: bool,
    demo: bool,
}

pub(crate) struct AppState {
    control: RunnerControl,
    pub(crate) edit_mode: AtomicBool,
    pub(crate) overlay_visible: AtomicBool,
    latest: Arc<Mutex<Snapshot>>,
    config_lock: Mutex<()>,
    /// Set when config.json couldn't be read and was moved aside.
    config_notice: Mutex<Option<String>>,
}

/// Forwards telemetry to the web views, and to any OBS pages (stream.rs).
struct TauriSink {
    app: AppHandle,
    latest: Arc<Mutex<Snapshot>>,
}

impl TauriSink {
    fn stream(&self) -> tauri::State<'_, stream::StreamHub> {
        self.app.state::<stream::StreamHub>()
    }
}

impl Sink for TauriSink {
    fn status(&self, status: &StatusPayload) {
        self.latest.lock().unwrap().status = Some(status.clone());
        let _ = self.app.emit("status", status);
        self.stream().publish("status", status);
    }
    fn reference(&self, reference: &ReferencePayload) {
        self.latest.lock().unwrap().reference = Some(reference.clone());
        let _ = self.app.emit("reference", reference);
        self.stream().publish("reference", reference);
    }
    fn session(&self, session: &SessionPayload) {
        self.latest.lock().unwrap().session = Some(session.clone());
        let _ = self.app.emit("session", session);
        self.stream().publish("session", session);
    }
    fn tick(&self, tick: &TickPayload) {
        // 60 Hz data only goes to the overlays.
        displays::emit_overlays(&self.app, "tick", tick);
        self.stream().publish("tick", tick);
    }
    fn field(&self, field: &FieldPayload) {
        let _ = self.app.emit("field", field);
        self.stream().publish("field", field);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

#[tauri::command]
fn load_config(app: AppHandle, state: State<AppState>) -> Result<Option<Value>, String> {
    let _guard = state.config_lock.lock().unwrap();
    let path = config_path(&app)?;
    let Ok(text) = fs::read_to_string(&path) else { return Ok(None) };
    match serde_json::from_str(&text) {
        Ok(config) => Ok(Some(config)),
        Err(err) => {
            // Keep the broken file, so the next save doesn't lose it for good.
            let name = corrupt_name(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()));
            let kept = path.with_file_name(&name);
            match fs::rename(&path, &kept) {
                Ok(()) => {
                    eprintln!("[tenths] config.json couldn't be read ({err}); kept it as {}", kept.display());
                    *state.config_notice.lock().unwrap() = Some(name);
                }
                Err(e) => eprintln!("[tenths] config.json couldn't be read ({err}) or moved aside ({e})"),
            }
            Ok(None)
        }
    }
}

/// The name a broken config.json was kept under this run, for a notice in Settings.
#[tauri::command]
fn config_notice(state: State<AppState>) -> Option<String> {
    state.config_notice.lock().unwrap().clone()
}

/// `config.corrupt-20261031-142501.json`, from seconds since 1970 (UTC).
fn corrupt_name(secs: u64) -> String {
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
    format!(
        "config.corrupt-{year:04}{month:02}{day:02}-{:02}{:02}{:02}.json",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

#[tauri::command]
fn save_config(app: AppHandle, state: State<AppState>, config: Value) -> Result<(), String> {
    let _guard = state.config_lock.lock().unwrap();
    let path = config_path(&app)?;
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    let _ = app.emit("config-changed", &config);
    let hub = app.state::<stream::StreamHub>();
    hub.publish("config-changed", &config);
    hub.apply(&app, &config);
    Ok(())
}

/// The saved config as it is on disk (None if missing or unreadable), for the OBS pages.
pub(crate) fn read_config(app: &AppHandle) -> Option<Value> {
    let text = fs::read_to_string(config_path(app).ok()?).ok()?;
    serde_json::from_str(&text).ok()
}

// ---------------------------------------------------------------------------
// Race reports (reports.rs)
// ---------------------------------------------------------------------------

fn reports_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("reports"))
}

#[tauri::command]
fn list_reports(app: AppHandle) -> Result<Vec<reports::ReportSummary>, String> {
    Ok(reports::list(&reports_dir(&app)?))
}

#[tauri::command]
fn get_report(app: AppHandle, id: String) -> Result<Value, String> {
    reports::read(&reports_dir(&app)?, &id)
}

#[tauri::command]
fn delete_report(app: AppHandle, id: String) -> Result<(), String> {
    reports::delete(&reports_dir(&app)?, &id)
}

/// The streaming server's port while it's running.
#[tauri::command]
fn stream_port(state: State<stream::StreamHub>) -> Option<u16> {
    state.port()
}

// ---------------------------------------------------------------------------
// Saved layouts
// ---------------------------------------------------------------------------

pub(crate) fn profiles_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("layouts"))
}

fn profiles_changed(app: &AppHandle) {
    if let Ok(dir) = profiles_dir(app) {
        let _ = app.emit("profiles-changed", profiles::list(&dir));
    }
}

#[tauri::command]
fn list_profiles(app: AppHandle) -> Result<Vec<profiles::ProfileSummary>, String> {
    Ok(profiles::list(&profiles_dir(&app)?))
}

#[tauri::command]
fn get_profile(app: AppHandle, id: String) -> Result<profiles::Profile, String> {
    profiles::read(&profiles_dir(&app)?, &id)
}

#[tauri::command]
fn save_profile(app: AppHandle, id: Option<String>, name: Option<String>, widgets: Value) -> Result<String, String> {
    let id = profiles::save(&profiles_dir(&app)?, id.as_deref(), name.as_deref(), widgets)?;
    profiles_changed(&app);
    Ok(id)
}

#[tauri::command]
fn delete_profile(app: AppHandle, id: String) -> Result<(), String> {
    profiles::delete(&profiles_dir(&app)?, &id)?;
    profiles_changed(&app);
    Ok(())
}

#[tauri::command]
fn export_profile(app: AppHandle, id: String, path: String) -> Result<(), String> {
    profiles::export(&profiles_dir(&app)?, &id, std::path::Path::new(&path))
}

#[tauri::command]
fn import_profile(app: AppHandle, path: String) -> Result<String, String> {
    let id = profiles::import(&profiles_dir(&app)?, std::path::Path::new(&path))?;
    profiles_changed(&app);
    Ok(id)
}

/// What a layout shortcut asks for; the overlay window does the switching.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LayoutShortcut {
    /// +1 for the next layout.
    step: Option<i32>,
    /// 0-based position in the list.
    index: Option<usize>,
}

#[tauri::command]
fn get_snapshot(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

pub(crate) fn snapshot(app: &AppHandle) -> Snapshot {
    let state = app.state::<AppState>();
    let mut snap = state.latest.lock().unwrap().clone();
    snap.edit_mode = state.edit_mode.load(Ordering::Relaxed);
    snap.overlay_visible = state.overlay_visible.load(Ordering::Relaxed);
    snap.demo = state.control.demo.load(Ordering::Relaxed);
    snap
}

#[tauri::command]
fn set_edit_mode(app: AppHandle, enabled: bool) {
    apply_edit_mode(&app, enabled);
}

#[tauri::command]
fn set_overlay_visible(app: AppHandle, visible: bool) {
    apply_overlay_visible(&app, visible);
}

/// The connected displays, primary first.
#[tauri::command]
fn get_displays(state: State<Displays>) -> Vec<Display> {
    state.list.lock().unwrap().clone()
}

/// Which display the calling overlay window covers (None for other windows).
#[tauri::command]
fn my_display(window: WebviewWindow, state: State<Displays>) -> Option<String> {
    state.windows.lock().unwrap().get(window.label()).cloned()
}

// ---------------------------------------------------------------------------
// Driver ratings (ratings.rs)
// ---------------------------------------------------------------------------

pub(crate) fn ratings_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_driver_ratings(app: AppHandle, state: State<ratings::Ratings>) -> Result<std::collections::BTreeMap<String, ratings::DriverRating>, String> {
    Ok(state.all(&ratings_dir(&app)?))
}

/// Thumbs up or down for a driver (None clears it). Every window hears about it.
#[tauri::command]
fn set_driver_rating(
    app: AppHandle,
    state: State<ratings::Ratings>,
    user_id: i64,
    name: String,
    rating: Option<ratings::Rating>,
) -> Result<(), String> {
    let all = state.set(&ratings_dir(&app)?, user_id, &name, rating)?;
    app.state::<stream::StreamHub>().publish("driver-ratings", &all);
    let _ = app.emit("driver-ratings", all);
    Ok(())
}

/// Lets the calling overlay take the mouse (true) or pass every click through
/// to iRacing (false), for the parts of a widget you can click outside edit
/// mode. The overlay turns it on only while the pointer is over them.
#[tauri::command]
fn set_clickable(window: WebviewWindow, state: State<AppState>, clickable: bool) {
    if state.edit_mode.load(Ordering::Relaxed) {
        return;
    }
    // Clicking shouldn't take the keyboard away from iRacing.
    let _ = window.set_focusable(!clickable);
    let _ = window.set_ignore_cursor_events(!clickable);
}

/// The mouse on the desktop and whether its button is held, for dragging widgets between displays.
#[tauri::command]
fn pointer(app: AppHandle) -> Option<displays::Pointer> {
    displays::pointer(&app)
}

#[tauri::command]
fn set_demo(app: AppHandle, state: State<AppState>, enabled: bool) {
    state.control.demo.store(enabled, Ordering::Relaxed);
    let _ = app.emit("demo-changed", enabled);
}

// ---------------------------------------------------------------------------
// Reference laps (.ibt import)
// ---------------------------------------------------------------------------

fn reference_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("reference.json"))
}

/// Lists the laps in an .ibt, .blap or .olap so the user can pick one.
#[tauri::command]
async fn inspect_lap_file(path: String) -> Result<IbtSummary, String> {
    // .ibt files can be hundreds of MB; read off the main thread.
    tauri::async_runtime::spawn_blocking(move || lapfile::summary(std::path::Path::new(&path)))
        .await
        .map_err(|e| e.to_string())?
}

/// Builds a reference from one lap of a file, saves it and starts comparing against it.
#[tauri::command]
async fn load_reference(app: AppHandle, path: String, lap: i32) -> Result<(), String> {
    let reference = tauri::async_runtime::spawn_blocking(move || lapfile::reference(std::path::Path::new(&path), lap))
    .await
    .map_err(|e| e.to_string())??;
    let text = serde_json::to_string(&reference).map_err(|e| e.to_string())?;
    // Write a copy first, so a crash can't leave half a file behind.
    let path = reference_path(&app)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    app.state::<AppState>().control.set_reference(Some(reference));
    Ok(())
}

#[tauri::command]
fn clear_reference(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let path = reference_path(&app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    state.control.set_reference(None);
    Ok(())
}

fn load_saved_reference(app: &AppHandle) -> Option<ReferenceLap> {
    let text = fs::read_to_string(reference_path(app).ok()?).ok()?;
    serde_json::from_str(&text).ok()
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings(&app);
}

// ---------------------------------------------------------------------------
// Updates (updates.rs)
// ---------------------------------------------------------------------------

#[tauri::command]
fn update_status(app: AppHandle) -> updates::UpdateStatus {
    updates::status(&app)
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<updates::UpdateInfo>, String> {
    updates::check(&app).await
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    updates::install(&app).await
}

/// The tray's update item: opens Settings on the About page.
fn show_updates(app: &AppHandle) {
    show_settings(app);
    let _ = app.emit_to(SETTINGS, "show-updates", ());
}

// ---------------------------------------------------------------------------
// Pit service
// ---------------------------------------------------------------------------

/// Sends a pit command to iRacing (or the demo race).
#[tauri::command]
fn pit_command(state: State<AppState>, action: PitCommand) {
    state.control.pit_command(action);
}

/// The pit hotkeys and buttons only work while the Pit stop widget is on and
/// you're in the car, so they don't take keys from other programs otherwise.
#[tauri::command]
fn set_pit_active(app: AppHandle, active: bool) {
    controls::set_pit_active(&app, active);
}

/// The key and controller button for each action, from the config.
#[tauri::command]
fn set_controls(app: AppHandle, controls: std::collections::HashMap<String, controls::Binding>) {
    controls::set_bindings(&app, controls);
}

/// Wheels, button boxes and gamepads plugged in now.
#[tauri::command]
fn list_controllers() -> Vec<controllers::Controller> {
    controllers::list()
}

/// Stops every hotkey and button while the settings window waits for a new one.
#[tauri::command]
fn suspend_controls(app: AppHandle, suspended: bool) {
    controls::set_suspended(&app, suspended);
}

// ---------------------------------------------------------------------------
// Window behaviour
// ---------------------------------------------------------------------------

/// Edit mode makes the overlays take the mouse so widgets can be dragged.
/// Outside edit mode every click goes straight through to iRacing.
fn apply_edit_mode(app: &AppHandle, enabled: bool) {
    let state = app.state::<AppState>();
    state.edit_mode.store(enabled, Ordering::Relaxed);
    if enabled && !state.overlay_visible.load(Ordering::Relaxed) {
        apply_overlay_visible(app, true);
    }
    for w in displays::overlays(app) {
        let _ = w.set_focusable(true);
        let _ = w.set_ignore_cursor_events(!enabled);
    }
    if enabled {
        // Keys (Escape to finish) go to the primary display's overlay.
        if let Some(w) = app.get_webview_window(OVERLAY) {
            let _ = w.set_focus();
        }
    }
    let _ = app.emit("edit-mode", enabled);
}

fn apply_overlay_visible(app: &AppHandle, visible: bool) {
    let state = app.state::<AppState>();
    state.overlay_visible.store(visible, Ordering::Relaxed);
    for w in displays::overlays(app) {
        let _ = if visible { w.show() } else { w.hide() };
    }
    let _ = app.emit("overlay-visible", visible);
}

fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open settings", true, None::<&str>)?;
    // Their shortcuts are added to the text once the overlay sends the bindings.
    let edit = MenuItem::with_id(app, "edit", "Edit layout", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "toggle", "Show / hide overlays", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next-layout", "Next layout", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    // Relabelled "Update to 0.x.y…" when a check finds one (updates.rs).
    let update = MenuItem::with_id(app, "update", "Check for updates", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Tenths", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &edit, &hide, &next, &sep, &update, &quit])?;
    app.state::<updates::Updates>().set_tray_item(update.clone());
    app.state::<controls::Controls>().set_tray(vec![
        ("overlay.edit", edit.clone(), "Edit layout"),
        ("overlay.toggle", hide.clone(), "Show / hide overlays"),
        ("layout.next", next.clone(), "Next layout"),
    ]);

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Tenths")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_settings(app),
            "edit" => toggle_edit(app),
            "toggle" => toggle_visible(app),
            "next-layout" => switch_layout(app, Some(1), None),
            "update" => show_updates(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_settings(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Asks the overlay to switch layouts: `step` +1 for the next, or `index` from 0.
fn switch_layout(app: &AppHandle, step: Option<i32>, index: Option<usize>) {
    let _ = app.emit_to(OVERLAY, "layout-shortcut", LayoutShortcut { step, index });
}

fn toggle_edit(app: &AppHandle) {
    let on = app.state::<AppState>().edit_mode.load(Ordering::Relaxed);
    apply_edit_mode(app, !on);
}

fn toggle_visible(app: &AppHandle) {
    let on = app.state::<AppState>().overlay_visible.load(Ordering::Relaxed);
    apply_overlay_visible(app, !on);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let control = RunnerControl::default();
    let latest = Arc::new(Mutex::new(Snapshot::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            // Shortcuts are registered from the bindings the overlay sends (controls.rs).
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        controls::on_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .manage(controls::Controls::default())
        .manage(Displays::default())
        .manage(updates::Updates::default())
        .manage(ratings::Ratings::default())
        .manage(stream::StreamHub::default())
        .manage(AppState {
            control: control.clone(),
            edit_mode: AtomicBool::new(false),
            overlay_visible: AtomicBool::new(true),
            latest: latest.clone(),
            config_lock: Mutex::new(()),
            config_notice: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            config_notice,
            save_config,
            get_snapshot,
            set_edit_mode,
            set_overlay_visible,
            set_demo,
            get_displays,
            my_display,
            pointer,
            set_clickable,
            get_driver_ratings,
            set_driver_rating,
            stream_port,
            list_reports,
            get_report,
            delete_report,
            open_settings,
            pit_command,
            set_pit_active,
            set_controls,
            suspend_controls,
            list_controllers,
            inspect_lap_file,
            load_reference,
            clear_reference,
            list_profiles,
            get_profile,
            save_profile,
            delete_profile,
            export_profile,
            import_profile,
            update_status,
            check_update,
            install_update,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // An overlay window over every display, kept in step as monitors come and go.
            displays::sync(&handle);
            displays::watch(handle.clone());
            build_tray(&handle)?;
            updates::watch(handle.clone());
            // The OBS pages, if they were left on.
            if let Some(config) = read_config(&handle) {
                handle.state::<stream::StreamHub>().apply(&handle, &config);
            }
            let buttons = handle.clone();
            controllers::spawn(move |press| controls::on_button(&buttons, press));
            if let Some(reference) = load_saved_reference(&handle) {
                control.set_reference(Some(reference));
            }

            // Lap log and personal bests, one file per car and track.
            let mut runner = control.clone();
            runner.laps_dir = handle.path().app_data_dir().ok().map(|d| d.join("laps"));
            runner.reports_dir = handle.path().app_data_dir().ok().map(|d| d.join("reports"));
            telemetry::spawn(TauriSink { app: handle, latest: latest.clone() }, runner);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing settings hides it; the app lives in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == SETTINGS {
                    api.prevent_close();
                    let _ = window.hide();
                    // The page stays loaded, so a key or button it was waiting
                    // for would keep every hotkey off. Turn them back on and tell it.
                    controls::set_suspended(window.app_handle(), false);
                    let _ = window.emit_to(SETTINGS, "settings-hidden", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Tenths");
}

#[cfg(test)]
mod tests {
    use super::corrupt_name;

    #[test]
    fn corrupt_names_carry_the_date_and_time() {
        assert_eq!(corrupt_name(0), "config.corrupt-19700101-000000.json");
        // 2026-10-31 14:25:01 UTC
        assert_eq!(corrupt_name(1_793_456_701), "config.corrupt-20261031-142501.json");
        // 2024-02-29 23:59:59 UTC, a leap day.
        assert_eq!(corrupt_name(1_709_251_199), "config.corrupt-20240229-235959.json");
    }
}
