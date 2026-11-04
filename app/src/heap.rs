//! Heap profiling, to see where the memory goes (`--features heap-profile`).
//!
//! ```text
//! cargo build --profile heap --features heap-profile
//! target/heap/tenths --replay race.ibt --bench 60 --heap-profile
//! python3 tools/heap_top.py tenths-heap.json
//! ```
//!
//! Every allocation is recorded with the call stack that made it ([dhat]). The `--bench` line
//! gains the live heap (bytes and blocks now, and the peak) every ten seconds, which shows
//! whether it settles or keeps climbing. When the app quits (`--bench N` quits after N seconds)
//! the profile is written to the file: `tools/heap_top.py` prints the biggest sites, and
//! <https://nnethercote.github.io/dh_view/dh_view.html> opens it as a tree (in the browser; the
//! file isn't uploaded) with three views: at the peak (t-gmax), at the end (t-end), and total.
//!
//! dhat writes its file only when its profiler is dropped, and eframe exits the process
//! without returning from `main`, so the profiler is dropped from `App::on_exit` (`finish`).
//!
//! Without the feature none of this exists: the system allocator, no cost, and `--heap-profile`
//! just says so.

#[cfg(feature = "heap-profile")]
mod on {
    use std::sync::Mutex;

    #[global_allocator]
    static ALLOC: dhat::Alloc = dhat::Alloc;

    struct Holder(#[allow(dead_code)] dhat::Profiler);
    // The profiler is only ever created and dropped under this lock.
    unsafe impl Send for Holder {}
    static PROFILER: Mutex<Option<Holder>> = Mutex::new(None);

    pub fn start(file: &str) {
        let profiler = dhat::Profiler::builder().file_name(file).build();
        *PROFILER.lock().unwrap() = Some(Holder(profiler));
        eprintln!("[heap] profiling every allocation; {file} is written when 2Tenths quits (--bench N quits after N seconds)");
    }

    /// The live heap: "now 52.3 MB in 41,210 blocks, peak 61.0 MB". None unless profiling.
    pub fn stats() -> Option<String> {
        PROFILER.lock().ok()?.as_ref()?;
        let s = dhat::HeapStats::get();
        Some(format!("heap now {:.1} MB in {} blocks, peak {:.1} MB", mb(s.curr_bytes), s.curr_blocks, mb(s.max_bytes)))
    }

    /// Writes the profile. Call once, on the way out.
    pub fn finish() {
        if let Some(Holder(profiler)) = PROFILER.lock().ok().and_then(|mut p| p.take()) {
            drop(profiler);
            eprintln!("[heap] profile written");
        }
    }

    fn mb(bytes: usize) -> f64 {
        bytes as f64 / 1_048_576.0
    }
}

#[cfg(not(feature = "heap-profile"))]
mod on {
    pub fn start(_file: &str) {
        eprintln!("[heap] this build has no heap profiler: build with `--profile heap --features heap-profile`");
    }
    pub fn stats() -> Option<String> {
        None
    }
    pub fn finish() {}
}

pub use on::{finish, start, stats};

/// What `--heap-profile` writes when it's given no file name.
pub const DEFAULT_FILE: &str = "tenths-heap.json";

/// The file named after `--heap-profile`, if there is one (the next word isn't another flag).
pub fn file_arg(next: Option<&str>) -> String {
    next.filter(|n| !n.starts_with("--")).unwrap_or(DEFAULT_FILE).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_name_is_optional() {
        assert_eq!(file_arg(None), DEFAULT_FILE);
        assert_eq!(file_arg(Some("--replay")), DEFAULT_FILE);
        assert_eq!(file_arg(Some("mine.json")), "mine.json");
    }

    #[test]
    fn no_stats_until_profiling() {
        assert_eq!(stats(), None);
    }
}
