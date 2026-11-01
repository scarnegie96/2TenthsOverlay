//! System-wide keyboard shortcuts (`RegisterHotKey`, through `global-hotkey`).
//!
//! `Hotkeys::set` registers exactly the shortcuts that should be live now (see
//! `Controls::sync`), as `controls.rs` does in the Tauri app, so pit keys
//! don't take Ctrl+Shift+T and friends from other programs while you're not
//! in the car. Only Windows has a real backend; elsewhere this does nothing.

#[cfg(windows)]
pub use win::Hotkeys;

#[cfg(not(windows))]
pub use stub::Hotkeys;

#[cfg(not(windows))]
mod stub {
    use super::super::KeyBinding;

    /// No global shortcuts off Windows.
    pub struct Hotkeys;

    impl Hotkeys {
        pub fn new(_ctx: egui::Context) -> Self {
            Self
        }
        pub fn set(&mut self, _wanted: &[KeyBinding]) {}
        pub fn poll(&mut self) -> Vec<KeyBinding> {
            Vec::new()
        }
    }
}

#[cfg(windows)]
mod win {
    use std::str::FromStr;
    use std::sync::mpsc::{channel, Receiver};

    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

    use super::super::KeyBinding;

    pub struct Hotkeys {
        manager: Option<GlobalHotKeyManager>,
        /// What's registered with Windows now, and the binding each came from.
        registered: Vec<(HotKey, KeyBinding)>,
        pressed: Receiver<u32>,
    }

    fn hotkey(k: &KeyBinding) -> Option<HotKey> {
        let code = Code::from_str(&k.code).ok()?;
        let mut mods = Modifiers::empty();
        for (on, m) in [(k.ctrl, Modifiers::CONTROL), (k.shift, Modifiers::SHIFT), (k.alt, Modifiers::ALT), (k.meta, Modifiers::SUPER)] {
            if on {
                mods |= m;
            }
        }
        Some(HotKey::new((!mods.is_empty()).then_some(mods), code))
    }

    impl Hotkeys {
        pub fn new(ctx: egui::Context) -> Self {
            let manager = match GlobalHotKeyManager::new() {
                Ok(m) => Some(m),
                Err(e) => {
                    eprintln!("[tenths] hotkeys unavailable: {e}");
                    None
                }
            };
            // Presses go to our own queue (and wake the UI) instead of the
            // crate's channel, so a key is acted on without waiting for the
            // next timed repaint.
            let (tx, pressed) = channel();
            GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
                if e.state == HotKeyState::Pressed {
                    let _ = tx.send(e.id);
                    ctx.request_repaint();
                }
            }));
            Self { manager, registered: Vec::new(), pressed }
        }

        pub fn set(&mut self, wanted: &[KeyBinding]) {
            let Some(manager) = &self.manager else { return };
            for (hk, _) in self.registered.drain(..) {
                let _ = manager.unregister(hk);
            }
            for k in wanted {
                let Some(hk) = hotkey(k) else {
                    eprintln!("[tenths] unknown key {:?}", k.code);
                    continue;
                };
                match manager.register(hk) {
                    Ok(()) => self.registered.push((hk, k.clone())),
                    Err(e) => eprintln!("[tenths] hotkey {}: {e}", k.code),
                }
            }
        }

        /// The shortcuts pressed since the last call.
        pub fn poll(&mut self) -> Vec<KeyBinding> {
            let mut out = Vec::new();
            while let Ok(id) = self.pressed.try_recv() {
                if let Some((_, k)) = self.registered.iter().find(|(hk, _)| hk.id() == id) {
                    out.push(k.clone());
                }
            }
            out
        }
    }
}
