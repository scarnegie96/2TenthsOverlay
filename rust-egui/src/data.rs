//! Telemetry in: the same runner and engine as the Tauri app, with a `Sink`
//! that keeps the latest payloads for the UI instead of sending them to
//! WebView windows. Nothing is serialised; the UI reads them in place.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use telemetry::model::{FieldPayload, ReferencePayload, SessionPayload, StatusPayload, TickPayload};
use telemetry::runner::{self, RunnerControl, Sink};

/// The latest of each payload. `Arc` so the UI can take a cheap copy and
/// draw without holding the lock.
#[derive(Default)]
pub struct Latest {
    pub status: Option<Arc<StatusPayload>>,
    pub session: Option<Arc<SessionPayload>>,
    pub tick: Option<Arc<TickPayload>>,
    pub field: Option<Arc<FieldPayload>>,
    /// Throttle and brake over the last few seconds, for the Inputs trace.
    /// Filled here, at the telemetry rate, so it doesn't depend on how often
    /// the UI happens to draw.
    pub trace: VecDeque<TracePoint>,
    /// Frames received, for the stats line.
    pub ticks: u64,
}

#[derive(Clone, Copy)]
pub struct TracePoint {
    pub t: f64,
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
}

/// Seconds of trace kept (the widget shows up to this much).
pub const TRACE_SECONDS: f64 = 10.0;

pub type Shared = Arc<Mutex<Latest>>;

struct UiSink {
    shared: Shared,
    ctx: egui::Context,
}

impl Sink for UiSink {
    fn status(&self, status: &StatusPayload) {
        self.shared.lock().unwrap().status = Some(Arc::new(status.clone()));
        self.ctx.request_repaint();
    }

    fn reference(&self, _reference: &ReferencePayload) {}

    fn session(&self, session: &SessionPayload) {
        let mut l = self.shared.lock().unwrap();
        l.session = Some(Arc::new(session.clone()));
        l.trace.clear();
    }

    fn tick(&self, tick: &TickPayload) {
        {
            let mut l = self.shared.lock().unwrap();
            // A new session (or a replay jump) runs the clock backwards: start the trace again.
            if l.trace.back().is_some_and(|p| tick.session_time < p.t) {
                l.trace.clear();
            }
            if l.trace.back().is_none_or(|p| tick.session_time > p.t) {
                l.trace.push_back(TracePoint {
                    t: tick.session_time,
                    throttle: tick.throttle,
                    brake: tick.brake,
                    clutch: tick.clutch,
                });
            }
            while l.trace.front().is_some_and(|p| tick.session_time - p.t > TRACE_SECONDS) {
                l.trace.pop_front();
            }
            l.tick = Some(Arc::new(tick.clone()));
            l.ticks += 1;
        }
        // One repaint per telemetry frame; egui merges requests, so it never queues up.
        self.ctx.request_repaint();
    }

    fn field(&self, field: &FieldPayload) {
        self.shared.lock().unwrap().field = Some(Arc::new(field.clone()));
    }
}

/// Starts the telemetry thread (iRacing, or the demo race with `demo`).
pub fn start(ctx: egui::Context, demo: bool) -> (Shared, RunnerControl) {
    let shared: Shared = Arc::default();
    let control = RunnerControl::default();
    control.demo.store(demo, std::sync::atomic::Ordering::Relaxed);
    runner::spawn(UiSink { shared: shared.clone(), ctx }, control.clone());
    (shared, control)
}
