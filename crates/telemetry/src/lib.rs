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

pub mod demo;
pub mod engine;
pub mod model;
pub mod runner;
pub mod session;

#[cfg(any(windows, feature = "typecheck-windows"))]
pub mod irsdk;

pub use engine::Engine;
pub use model::*;
pub use runner::{spawn, RunnerControl, Sink};
pub use session::SessionInfo;
