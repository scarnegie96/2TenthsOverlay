//! The layout shortcuts (`layout.next`, `layout.1` to `layout.9`) call this.
//!
//! MERGE POINT: saved layouts (G3, `src-tauri/src/profiles.rs`) are built on
//! another branch. Until that lands, `switch_layout` only logs. When merging,
//! replace its body with the real switch (load the layout's widgets into the
//! store and set `general.activeProfile`); nothing else in `controls` needs to
//! change, `main.rs` already calls it with the store.

use crate::config::Store;

/// Which layout a shortcut asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutSwitch {
    /// The one after the active layout, wrapping round (`layout.next`).
    Next,
    /// The nth saved layout, counted from 0 (`layout.1` is `Index(0)`).
    Index(usize),
}

/// The layout shortcut for an action id, if it is one.
pub fn from_action(action: &str) -> Option<LayoutSwitch> {
    let n = action.strip_prefix("layout.")?;
    if n == "next" {
        return Some(LayoutSwitch::Next);
    }
    n.parse::<usize>().ok().filter(|n| (1..=9).contains(n)).map(|n| LayoutSwitch::Index(n - 1))
}

/// Switches layout. No saved layouts yet: says so and does nothing.
pub fn switch_layout(_store: &mut Store, switch: LayoutSwitch) {
    eprintln!("[tenths] layout shortcut {switch:?}: saved layouts aren't available in this build yet");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions() {
        assert_eq!(from_action("layout.next"), Some(LayoutSwitch::Next));
        assert_eq!(from_action("layout.1"), Some(LayoutSwitch::Index(0)));
        assert_eq!(from_action("layout.9"), Some(LayoutSwitch::Index(8)));
        assert_eq!(from_action("layout.0"), None);
        assert_eq!(from_action("layout.10"), None);
        assert_eq!(from_action("pit.fuel"), None);
    }
}
