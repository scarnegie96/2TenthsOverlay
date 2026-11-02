//! The tray icon and its menu (`build_tray` in `src-tauri/src/lib.rs`): Open
//! settings, Edit layout, Show / hide overlays, Next layout, Stream window, Check for
//! updates, Quit. A left click opens Settings.
//!
//! Windows only: the `tray-icon` crate needs the thread's message loop,
//! which eframe's event loop provides. Not compiled on other platforms, where
//! `Shell` has no tray.

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use super::Request;

const ICON_PNG: &[u8] = include_bytes!("../../assets/icons/64x64.png");

pub struct Tray {
    _icon: TrayIcon,
    open: MenuId,
    edit: MenuId,
    toggle: MenuId,
    next: MenuId,
    /// "Stream window", ticked while the OBS window is open.
    stream: CheckMenuItem,
    update: MenuId,
    quit: MenuId,
    /// Items whose text gets the action's shortcut: `(action id, item, plain text)`.
    shortcut_items: Vec<(&'static str, MenuItem, &'static str)>,
}

fn icon() -> Result<Icon, String> {
    let img = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = img.dimensions();
    Icon::from_rgba(img.into_raw(), w, h).map_err(|e| e.to_string())
}

impl Tray {
    pub fn new() -> Result<Self, String> {
        let open = MenuItem::new("Open settings", true, None);
        // Their shortcuts are added to the text once the bindings are known.
        let edit = MenuItem::new("Edit layout", true, None);
        let toggle = MenuItem::new("Show / hide overlays", true, None);
        let next = MenuItem::new("Next layout", true, None);
        let stream = CheckMenuItem::new("Stream window", true, false, None);
        // Relabelled "Update to 0.x.y…" when a check finds one (the updater is not ported yet).
        let update = MenuItem::new("Check for updates", true, None);
        let quit = MenuItem::new("Quit Tenths", true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &edit, &toggle, &next, &stream, &PredefinedMenuItem::separator(), &update, &quit]).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("Tenths")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_icon(icon()?)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            _icon: tray,
            open: open.id().clone(),
            edit: edit.id().clone(),
            toggle: toggle.id().clone(),
            next: next.id().clone(),
            stream,
            update: update.id().clone(),
            quit: quit.id().clone(),
            shortcut_items: vec![
                ("overlay.edit", edit, "Edit layout"),
                ("overlay.toggle", toggle, "Show / hide overlays"),
                ("layout.next", next, "Next layout"),
            ],
        })
    }

    pub fn poll(&self) -> Vec<Request> {
        let mut out = Vec::new();
        while let Ok(e) = MenuEvent::receiver().try_recv() {
            let id = &e.id;
            if *id == self.open {
                out.push(Request::OpenSettings);
            } else if *id == self.edit {
                out.push(Request::ToggleEdit);
            } else if *id == self.toggle {
                out.push(Request::ToggleOverlays);
            } else if *id == self.next {
                out.push(Request::NextLayout);
            } else if id == self.stream.id() {
                out.push(Request::ToggleStream);
            } else if *id == self.update {
                out.push(Request::OpenAbout);
            } else if *id == self.quit {
                out.push(Request::Quit);
            }
        }
        while let Ok(e) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                out.push(Request::OpenSettings);
            }
        }
        out
    }

    /// Ticks or clears "Stream window" to match the setting (the click toggled it already; the setting has the last word).
    pub fn set_stream(&self, on: bool) {
        self.stream.set_checked(on);
    }

    pub fn set_shortcuts(&self, shortcuts: &[(&str, Option<String>)]) {
        for (action, item, plain) in &self.shortcut_items {
            let key = shortcuts.iter().find(|(a, _)| a == action).and_then(|(_, k)| k.as_deref());
            item.set_text(match key {
                Some(k) => format!("{plain}   {k}"),
                None => (*plain).to_string(),
            });
        }
    }
}
