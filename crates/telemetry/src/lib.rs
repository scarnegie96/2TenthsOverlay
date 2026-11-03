//! Telemetry core for the Tenths overlay.
//!
//! Everything here is plain Rust with no Tauri dependency, so it can be unit
//! tested and reused (CLI logger, replay tool, a second front end...).
//!
//! Data flow:
//!
//! ```text
//!  iRacing shared memory ──► irsdk::IrsdkSource ─┐
//!                                                 ├─► RawFrame + session YAML ──► Engine ──► payloads ──► Sink (Tauri)
//!  demo::DemoSource (fake race) ─────────────────┘
//! ```

/// Diagnostics to stderr: debug builds only (see the same macro in the app).
macro_rules! log {
    ($($t:tt)*) => {
        if cfg!(debug_assertions) {
            eprintln!($($t)*);
        }
    };
}

pub mod blap;
pub mod coach;
pub mod demo;
pub mod dynamics;
pub mod engine;
pub mod finish;
pub mod grid;
pub mod ibt;
pub mod laps;
pub mod instruments;
pub mod lapfile;
pub mod model;
pub mod pitloss;
pub mod pit;
pub mod reference;
pub mod replay;
pub mod report;
pub mod runner;
pub mod session;
pub mod shm;
pub mod stints;
pub mod vars;

#[cfg(any(windows, feature = "typecheck-windows"))]
pub mod irsdk;

pub use engine::Engine;
pub use laps::{LapLogPayload, LapRecord, LapStore, PersonalBest};
pub use model::*;
pub use pit::PitCommand;
pub use runner::{spawn, RunnerControl, Sink};
pub use session::SessionInfo;
