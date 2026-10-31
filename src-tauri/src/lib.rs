//! Tauri shell: windows, tray, hotkeys, config storage, and the bridge from
//! the telemetry thread to the web views.

mod controllers;
mod controls;
mod profiles;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;
use telemetry::ibt::IbtSummary;
use telemetry::lapfile;
use telemetry::reference::ReferenceLap;
use telemetry::{FieldPayload, PitCommand, ReferencePayload, RunnerControl, SessionPayload, Sink, StatusPayload, TickPayload};

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

struct AppState {
    control: RunnerControl,
    edit_mode: AtomicBool,
    overlay_visible: AtomicBool,
    latest: Arc<Mutex<Snapshot>>,
    config_lock: Mutex<()>,
}

/// Forwards telemetry to the web views.
struct TauriSink {
    app: AppHandle,
    latest: Arc<Mutex<Snapshot>>,
}

impl Sink for TauriSink {
    fn status(&self, status: &StatusPayload) {
        self.latest.lock().unwrap().status = Some(status.clone());
        let _ = self.app.emit("status", status);
    }
    fn reference(&self, reference: &ReferencePayload) {
        self.latest.lock().unwrap().reference = Some(reference.clone());
        let _ = self.app.emit("reference", reference);
    }
    fn session(&self, session: &SessionPayload) {
        self.latest.lock().unwrap().session = Some(session.clone());
        let _ = self.app.emit("session", session);
    }
    fn tick(&self, tick: &TickPayload) {
        // 60 Hz data only goes to the overlay.
        let _ = self.app.emit_to(OVERLAY, "tick", tick);
    }
    fn field(&self, field: &FieldPayload) {
        let _ = self.app.emit("field", field);
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
fn load_config(app: AppHandle) -> Result<Option<Value>, String> {
    let path = config_path(&app)?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text).ok()),
        Err(_) => Ok(None),
    }
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
    Ok(())
}

// ---------------------------------------------------------------------------
// Saved layouts
// ---------------------------------------------------------------------------

fn profiles_dir(app: &AppHandle) -> Result<PathBuf, String> {
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
fn get_snapshot(state: State<AppState>) -> Snapshot {
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
    fs::write(reference_path(&app)?, text).map_err(|e| e.to_string())?;
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

fn overlay(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(OVERLAY)
}

/// Edit mode makes the overlay take the mouse so widgets can be dragged.
/// Outside edit mode every click goes straight through to iRacing.
fn apply_edit_mode(app: &AppHandle, enabled: bool) {
    let state = app.state::<AppState>();
    state.edit_mode.store(enabled, Ordering::Relaxed);
    if let Some(w) = overlay(app) {
        let _ = w.set_ignore_cursor_events(!enabled);
        if enabled {
            if !state.overlay_visible.load(Ordering::Relaxed) {
                apply_overlay_visible(app, true);
            }
            let _ = w.set_focus();
        }
    }
    let _ = app.emit("edit-mode", enabled);
}

fn apply_overlay_visible(app: &AppHandle, visible: bool) {
    let state = app.state::<AppState>();
    state.overlay_visible.store(visible, Ordering::Relaxed);
    if let Some(w) = overlay(app) {
        let _ = if visible { w.show() } else { w.hide() };
    }
    let _ = app.emit("overlay-visible", visible);
}

/// Cover the primary monitor with the transparent overlay window.
fn fit_overlay_to_monitor(w: &WebviewWindow) {
    let monitor = w.primary_monitor().ok().flatten().or_else(|| w.current_monitor().ok().flatten());
    if let Some(m) = monitor {
        let _ = w.set_position(*m.position());
        let _ = w.set_size(*m.size());
    }
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
    let quit = MenuItem::with_id(app, "quit", "Quit Tenths", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &edit, &hide, &next, &sep, &quit])?;
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
        .manage(AppState {
            control: control.clone(),
            edit_mode: AtomicBool::new(false),
            overlay_visible: AtomicBool::new(true),
            latest: latest.clone(),
            config_lock: Mutex::new(()),
        })
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            get_snapshot,
            set_edit_mode,
            set_overlay_visible,
            set_demo,
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
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            if let Some(w) = overlay(&handle) {
                fit_overlay_to_monitor(&w);
                w.set_ignore_cursor_events(true)?;
                w.show()?;
            }
            build_tray(&handle)?;
            let buttons = handle.clone();
            controllers::spawn(move |press| controls::on_button(&buttons, press));
            if let Some(reference) = load_saved_reference(&handle) {
                control.set_reference(Some(reference));
            }

            // Lap log and personal bests, one file per car and track.
            let mut runner = control.clone();
            runner.laps_dir = handle.path().app_data_dir().ok().map(|d| d.join("laps"));
            telemetry::spawn(TauriSink { app: handle, latest: latest.clone() }, runner);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing settings hides it; the app lives in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == SETTINGS {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Tenths");
}
