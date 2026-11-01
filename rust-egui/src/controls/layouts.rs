//! The layout shortcuts (`layout.next`, `layout.1` to `layout.9`): which
//! layout a shortcut asks for. The app switches it (`App::next_layout`,
//! `App::layout_at` in main.rs, on `layouts::Layouts`).


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
