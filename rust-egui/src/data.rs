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
    pub reference: Option<Arc<ReferencePayload>>,
    pub tick: Option<Arc<TickPayload>>,
    pub field: Option<Arc<FieldPayload>>,
    /// Pedals and the reference's over the last seconds, for the Inputs
    /// trace. Filled here, at the telemetry rate, so it doesn't depend on how
    /// often the UI happens to draw.
    pub trace: VecDeque<TracePoint>,
    /// Frames received, for the stats line.
    pub ticks: u64,
    /// Field payloads received, so the UI's histories take each one once.
    pub fields: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TracePoint {
    pub t: f64,
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
    /// The reference lap's pedals at the same point, when one is loaded.
    pub ref_throttle: Option<f32>,
    pub ref_brake: Option<f32>,
}

/// Seconds of trace kept (the Inputs widget shows up to this much).
pub const TRACE_SECONDS: f64 = 20.0;

pub type Shared = Arc<Mutex<Latest>>;

/// Longest the overlay waits between redraws while telemetry flows, in ms
/// (`--fps`; 30 fps by default). iRacing sends 60 frames a second; drawing
/// every one of them costs the sim more than it gains on screen.
pub static FRAME_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(33);

struct UiSink {
    shared: Shared,
    ctx: egui::Context,
}

impl Sink for UiSink {
    fn status(&self, status: &StatusPayload) {
        self.shared.lock().unwrap().status = Some(Arc::new(status.clone()));
        self.ctx.request_repaint();
    }

    fn reference(&self, reference: &ReferencePayload) {
        self.shared.lock().unwrap().reference = Some(Arc::new(reference.clone()));
    }

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
                    ref_throttle: tick.coach.as_ref().and_then(|c| c.ref_throttle),
                    ref_brake: tick.coach.as_ref().and_then(|c| c.ref_brake),
                });
            }
            while l.trace.front().is_some_and(|p| tick.session_time - p.t > TRACE_SECONDS) {
                l.trace.pop_front();
            }
            l.tick = Some(Arc::new(tick.clone()));
            l.ticks += 1;
        }
        // A redraw at most every FRAME_MS: egui keeps the earliest pending
        // request, so frames arriving in between don't add any.
        let ms = FRAME_MS.load(std::sync::atomic::Ordering::Relaxed);
        self.ctx.request_repaint_after(std::time::Duration::from_millis(ms));
    }

    fn field(&self, field: &FieldPayload) {
        let mut l = self.shared.lock().unwrap();
        l.field = Some(Arc::new(field.clone()));
        l.fields += 1;
    }
}

/// Starts the telemetry thread (iRacing, or the demo race with `demo`).
/// `offline` starts nothing: no data at all, for checking empty states.
pub fn start(ctx: egui::Context, demo: bool, offline: bool) -> (Shared, RunnerControl) {
    start_with(ctx, demo, offline, |_| {})
}

/// `start`, with a chance to set up the runner first: where laps and race
/// reports are saved, whether to save reports, the saved reference lap
/// (`shell` and the Settings window own those).
pub fn start_with(ctx: egui::Context, demo: bool, offline: bool, setup: impl FnOnce(&mut RunnerControl)) -> (Shared, RunnerControl) {
    let shared: Shared = Arc::default();
    let mut control = RunnerControl::default();
    if offline {
        shared.lock().unwrap().status = Some(Arc::new(StatusPayload {
            source: telemetry::model::SourceKind::None,
            connected: false,
            message: "Offline".into(),
        }));
        return (shared, control);
    }
    control.demo.store(demo, std::sync::atomic::Ordering::Relaxed);
    if demo {
        // The web preview has a reference lap loaded; so does this: a demo lap
        // recorded as an .ibt and imported, for the Coach, Delta and ghost inputs.
        if let Some(lap) = demo_reference() {
            control.set_reference(Some(lap));
        }
    }
    setup(&mut control);
    runner::spawn(UiSink { shared: shared.clone(), ctx }, control.clone());
    (shared, control)
}

fn demo_reference() -> Option<telemetry::reference::ReferenceLap> {
    let mut demo = telemetry::demo::DemoSource::new();
    let bytes = telemetry::demo::record_ibt(&mut demo, 3.2);
    let file = telemetry::ibt::IbtFile::from_bytes(bytes).ok()?;
    let lap = file.summary().laps.first()?.lap;
    file.reference(lap, "Demo reference.ibt").ok()
}

/// What the widgets draw from this frame: a cheap copy of `Latest`.
#[derive(Default, Clone)]
pub struct Live {
    pub status: Option<Arc<StatusPayload>>,
    pub session: Option<Arc<SessionPayload>>,
    pub reference: Option<Arc<ReferencePayload>>,
    pub tick: Option<Arc<TickPayload>>,
    pub field: Option<Arc<FieldPayload>>,
    pub trace: Vec<TracePoint>,
    pub ticks: u64,
    pub fields: u64,
}

impl Live {
    pub fn take(shared: &Shared) -> Self {
        let l = shared.lock().unwrap();
        Self {
            status: l.status.clone(),
            session: l.session.clone(),
            reference: l.reference.clone(),
            tick: l.tick.clone(),
            field: l.field.clone(),
            trace: l.trace.iter().copied().collect(),
            ticks: l.ticks,
            fields: l.fields,
        }
    }

    pub fn connected(&self) -> bool {
        self.status.as_ref().is_some_and(|s| s.connected)
    }

    pub fn race(&self) -> bool {
        self.field.as_ref().is_some_and(|f| f.clock.session_type.to_lowercase().contains("race"))
    }

    pub fn compounds(&self) -> &[String] {
        self.session.as_ref().map(|s| s.tire_compounds.as_slice()).unwrap_or(&[])
    }
}
