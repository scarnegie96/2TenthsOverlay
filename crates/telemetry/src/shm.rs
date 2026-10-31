//! Small pure helpers for reading the iRacing shared memory.
//!
//! The reader itself (`irsdk`) is Windows only. These have no Windows calls,
//! so they can be unit tested on any platform.

/// What the variable list was read from. If any of these change, the
/// var headers we hold may point at the wrong bytes and must be read again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VarLayout {
    pub num_vars: i32,
    pub var_header_offset: i32,
    pub buf_len: i32,
}

/// True when the var headers must be (re)read. `loaded` is the layout they
/// were last read from, or None if we have none (start, or after a disconnect).
pub fn needs_var_reload(loaded: Option<VarLayout>, now: VarLayout) -> bool {
    loaded != Some(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: VarLayout = VarLayout { num_vars: 300, var_header_offset: 1000, buf_len: 6000 };

    #[test]
    fn reload_when_nothing_loaded() {
        // Fresh start or after a disconnect.
        assert!(needs_var_reload(None, L));
    }

    #[test]
    fn no_reload_when_unchanged() {
        assert!(!needs_var_reload(Some(L), L));
    }

    #[test]
    fn reload_when_count_same_but_offset_moved() {
        let now = VarLayout { var_header_offset: 2000, ..L };
        assert!(needs_var_reload(Some(L), now));
    }

    #[test]
    fn reload_when_buf_len_changes() {
        let now = VarLayout { buf_len: 6400, ..L };
        assert!(needs_var_reload(Some(L), now));
    }

    #[test]
    fn reload_when_count_changes() {
        let now = VarLayout { num_vars: 301, ..L };
        assert!(needs_var_reload(Some(L), now));
    }
}
