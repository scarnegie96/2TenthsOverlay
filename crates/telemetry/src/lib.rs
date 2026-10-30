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

pub mod blap;
pub mod coach;
pub mod demo;
pub mod dynamics;
pub mod engine;
pub mod ibt;
pub mod laps;
pub mod instruments;
pub mod lapfile;
pub mod model;
pub mod reference;
pub mod runner;
pub mod session;
pub mod vars;

#[cfg(any(windows, feature = "typecheck-windows"))]
pub mod irsdk;

pub use engine::Engine;
pub use laps::{LapLogPayload, LapRecord, LapStore, PersonalBest};
pub use model::*;
pub use runner::{spawn, RunnerControl, Sink};
pub use session::SessionInfo;
