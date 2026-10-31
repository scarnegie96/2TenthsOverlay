//! One transparent overlay window per connected display.
//!
//! The overlay used to be a single window over the primary monitor. Now the
//! window labelled `overlay` (from tauri.conf.json) covers the primary display
//! and every other display gets its own window, `overlay-2`, `overlay-3`…,
//! created here. Each window shows the widgets placed on its display, so
//! widgets can sit on any screen, and each window has its display's own scale
//! factor (a single window stretched across screens can't, so widgets would
//! land in the wrong place on mixed-DPI setups).
//!
//! Displays are polled every couple of seconds, so plugging in, unplugging or
//! rearranging monitors adds, removes or moves windows without a restart.

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget, Manager, Monitor, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{AppState, OVERLAY};

/// A display as the front end sees it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    /// Stable id: Windows' device name (e.g. `\\.\DISPLAY2`), else its position.
    pub key: String,
    /// 1 for the primary, then left to right, top to bottom.
    pub number: u32,
    pub primary: bool,
    /// Physical pixels on the virtual desktop.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// Windows display scaling: 1.0 = 100%, 1.5 = 150%.
    pub scale: f64,
}

/// Which overlay window covers which display.
#[derive(Default)]
pub struct Displays {
    pub list: Mutex<Vec<Display>>,
    /// Window label -> display key.
    pub windows: Mutex<HashMap<String, String>>,
}

fn key_of(m: &Monitor) -> String {
    match m.name() {
        Some(n) if !n.trim().is_empty() => n.clone(),
        _ => format!("{},{}", m.position().x, m.position().y),
    }
}

/// The connected displays: primary first, then left to right, top to bottom.
pub fn current(app: &AppHandle) -> Vec<Display> {
    let monitors = app.available_monitors().unwrap_or_default();
    let primary = app.primary_monitor().ok().flatten().map(|m| key_of(&m));
    let mut list: Vec<Display> = monitors
        .iter()
        .map(|m| Display {
            key: key_of(m),
            number: 0,
            primary: Some(key_of(m)) == primary,
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
        })
        .collect();
    // No primary reported (rare): treat the first as primary.
    if !list.iter().any(|d| d.primary) {
        if let Some(d) = list.first_mut() {
            d.primary = true;
        }
    }
    list.sort_by_key(|d| (!d.primary, d.x, d.y));
    let mut seen = std::collections::HashSet::new();
    list.retain(|d| seen.insert(d.key.clone()));
    for (i, d) in list.iter_mut().enumerate() {
        d.number = i as u32 + 1;
    }
    list
}

/// Is this window one of the overlays?
pub fn is_overlay(label: &str) -> bool {
    label == OVERLAY || label.starts_with("overlay-")
}

/// Every overlay window.
pub fn overlays(app: &AppHandle) -> Vec<WebviewWindow> {
    app.webview_windows().into_iter().filter(|(l, _)| is_overlay(l)).map(|(_, w)| w).collect()
}

/// Sends an event to every overlay window (and nothing else).
pub fn emit_overlays<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    let _ = app.emit_filter(event, payload, |t| match t {
        EventTarget::WebviewWindow { label } | EventTarget::Window { label } | EventTarget::Webview { label } => {
            is_overlay(label)
        }
        _ => false,
    });
}

fn place(w: &WebviewWindow, d: &Display) {
    let _ = w.set_position(tauri::PhysicalPosition::new(d.x, d.y));
    let _ = w.set_size(tauri::PhysicalSize::new(d.width, d.height));
}

/// Makes the overlay windows match the displays: the primary window over the
/// primary display, one more window per other display, none left over.
/// Returns true when anything changed.
pub fn sync(app: &AppHandle) -> bool {
    let list = current(app);
    let state = app.state::<Displays>();
    if *state.list.lock().unwrap() == list && !list.is_empty() {
        return false;
    }
    let app_state = app.state::<AppState>();
    let editing = app_state.edit_mode.load(Ordering::Relaxed);
    let visible = app_state.overlay_visible.load(Ordering::Relaxed);

    let mut windows = HashMap::new();
    for d in &list {
        let label = if d.primary { OVERLAY.to_string() } else { format!("overlay-{}", d.number) };
        let w = match app.get_webview_window(&label) {
            Some(w) => w,
            None => match build(app, &label) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("couldn't open an overlay for display {}: {e}", d.number);
                    continue;
                }
            },
        };
        place(&w, d);
        let _ = w.set_ignore_cursor_events(!editing);
        if visible {
            let _ = w.show();
        }
        windows.insert(label, d.key.clone());
    }
    // Displays that have gone: close their windows (never the primary one).
    for w in overlays(app) {
        if w.label() != OVERLAY && !windows.contains_key(w.label()) {
            let _ = w.destroy();
        }
    }
    *state.windows.lock().unwrap() = windows;
    *state.list.lock().unwrap() = list.clone();
    let _ = app.emit("displays-changed", &list);
    true
}

/// A secondary overlay window, set up like the primary one in tauri.conf.json.
fn build(app: &AppHandle, label: &str) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App("overlay.html".into()))
        .title("Tenths overlay")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()
}

/// Watches for displays being added, removed or rearranged.
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(2));
        sync(&app);
    });
}
