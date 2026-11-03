//! Process memory, for the stats line: what the whole app costs.

/// This process's memory in bytes: the working set on Windows (what Task
/// Manager shows), the resident set elsewhere.
#[cfg(windows)]
pub fn memory_bytes() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    (ok != 0).then_some(c.WorkingSetSize as u64)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn memory_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

/// macOS has no /proc: ask `ps` for this process's resident set (kilobytes). Called once a second.
#[cfg(target_os = "macos")]
pub fn memory_bytes() -> Option<u64> {
    let out = std::process::Command::new("/bin/ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().ok()?;
    parse_ps_rss(&String::from_utf8_lossy(&out.stdout))
}

/// `ps -o rss=` prints just the number of kilobytes.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
fn parse_ps_rss(output: &str) -> Option<u64> {
    output.trim().parse::<u64>().ok().filter(|&kb| kb > 0).map(|kb| kb * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ps_output() {
        assert_eq!(parse_ps_rss("  123456\n"), Some(123456 * 1024));
        assert_eq!(parse_ps_rss(""), None);
        assert_eq!(parse_ps_rss("ps: illegal option"), None);
        assert_eq!(parse_ps_rss("0\n"), None);
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn this_process_has_memory() {
        // Linux (and Windows) read it directly.
        assert!(memory_bytes().is_some_and(|b| b > 1_000_000));
    }
}

/// `TENTHS_EGUI_VOTING` (shows the rating thumbs without a finished race), read once: it's asked every frame.
pub fn voting_forced() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var_os("TENTHS_EGUI_VOTING").is_some())
}

/// `TENTHS_EGUI_FAIL=<id>` makes that widget fail, to check the error path; read once.
pub fn fail_widget() -> Option<&'static str> {
    static F: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    F.get_or_init(|| std::env::var("TENTHS_EGUI_FAIL").ok()).as_deref()
}
