//! Small pure helpers for reading the iRacing shared memory.
//!
//! The reader itself (`irsdk`) is Windows only. These have no Windows calls,
//! so they can be unit tested on any platform.

/// Size of the fixed header at the start of the view.
pub const HEADER_LEN: usize = 112;

/// Largest single read we'll make (session YAML, telemetry buffer, var
/// headers). Real values are far smaller; anything bigger is garbage.
pub const MAX_READ_LEN: usize = 16 * 1024 * 1024;

/// Checks that `len` bytes at `offset` fit inside a view of `view_len` bytes.
/// Takes the raw signed header values. Returns the start offset, or None if
/// either is negative, the read is too long, or it runs past the end.
pub fn checked_range(offset: i64, len: i64, view_len: usize) -> Option<usize> {
    let offset = usize::try_from(offset).ok()?;
    let len = usize::try_from(len).ok()?;
    range_in_view(offset, len, view_len)
}

/// Same check on unsigned values.
pub fn range_in_view(offset: usize, len: usize, view_len: usize) -> Option<usize> {
    if len > MAX_READ_LEN {
        return None;
    }
    let end = offset.checked_add(len)?;
    (end <= view_len).then_some(offset)
}

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

    #[test]
    fn range_inside() {
        assert_eq!(checked_range(100, 50, 1000), Some(100));
        assert_eq!(checked_range(0, 0, 0), Some(0));
    }

    #[test]
    fn range_ending_exactly_at_end() {
        assert_eq!(checked_range(900, 100, 1000), Some(900));
    }

    #[test]
    fn range_past_end() {
        assert_eq!(checked_range(901, 100, 1000), None);
        assert_eq!(checked_range(1000, 1, 1000), None);
        assert_eq!(checked_range(5000, 0, 1000), None);
    }

    #[test]
    fn range_negative() {
        assert_eq!(checked_range(-1, 4, 1000), None);
        assert_eq!(checked_range(0, -4, 1000), None);
        assert_eq!(checked_range(i32::MIN as i64, 4, usize::MAX), None);
    }

    #[test]
    fn range_overflow() {
        assert_eq!(range_in_view(usize::MAX, 1, usize::MAX), None);
        assert_eq!(range_in_view(usize::MAX - 3, 8, usize::MAX), None);
    }

    #[test]
    fn range_too_long() {
        let big = MAX_READ_LEN + 1;
        assert_eq!(range_in_view(0, big, usize::MAX), None);
        assert_eq!(range_in_view(0, MAX_READ_LEN, usize::MAX), Some(0));
    }
}
