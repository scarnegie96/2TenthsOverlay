//! The telemetry thread. Owns the source and engine, and pushes payloads to a
//! `Sink` (implemented by the Tauri app) at the right rates:
//!
//! * `tick`    – every frame (60 Hz)
//! * `field`   – 10 Hz
//! * `session` – when the session info changes
//! * `status`  – when the connection changes

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::demo::DemoSource;
use crate::engine::Engine;
use crate::laps::LapStore;
use crate::model::*;
use crate::reference::ReferenceLap;

pub trait Sink: Send + 'static {
    fn status(&self, status: &StatusPayload);
    fn reference(&self, reference: &ReferencePayload);
    fn session(&self, session: &SessionPayload);
    fn tick(&self, tick: &TickPayload);
    fn field(&self, field: &FieldPayload);
}

#[derive(Clone, Default)]
pub struct RunnerControl {
    pub demo: Arc<AtomicBool>,
    pub stop: Arc<AtomicBool>,
    /// Imported comparison lap. Bump `reference_rev` after changing it.
    pub reference: Arc<Mutex<Option<ReferenceLap>>>,
    pub reference_rev: Arc<AtomicU64>,
    /// Folder for saved laps from iRacing (never the demo). None to not save.
    pub laps_dir: Option<PathBuf>,
}

impl RunnerControl {
    pub fn set_reference(&self, reference: Option<ReferenceLap>) {
        *self.reference.lock().unwrap() = reference;
        self.reference_rev.fetch_add(1, Ordering::Relaxed);
    }
}

enum Source {
    Demo(Box<DemoSource>),
    #[cfg(windows)]
    Iracing(crate::irsdk::Irsdk),
}

const FIELD_INTERVAL: Duration = Duration::from_millis(100);
const RECONNECT_INTERVAL: Duration = Duration::from_millis(1000);

pub fn spawn<S: Sink>(sink: S, control: RunnerControl) -> JoinHandle<()> {
    thread::Builder::new()
        .name("telemetry".into())
        .spawn(move || run(sink, control))
        .expect("spawn telemetry thread")
}

fn run<S: Sink>(sink: S, control: RunnerControl) {
    let mut source: Option<Source> = None;
    let mut engine = Engine::new();
    let mut frame = RawFrame::with_capacity();
    let mut last_field = Instant::now() - FIELD_INTERVAL;
    let mut last_status: Option<(SourceKind, bool)> = None;
    let mut last_session_num = i32::MIN;

    let mut report = |kind: SourceKind, connected: bool, message: &str| {
        if last_status != Some((kind, connected)) {
            last_status = Some((kind, connected));
            sink.status(&StatusPayload { source: kind, connected, message: message.to_string() });
        }
    };

    let mut reference_rev = u64::MAX;

    while !control.stop.load(Ordering::Relaxed) {
        let rev = control.reference_rev.load(Ordering::Relaxed);
        if rev != reference_rev {
            reference_rev = rev;
            engine.set_reference(control.reference.lock().unwrap().clone());
            sink.reference(&engine.reference_payload());
        }

        let want_demo = control.demo.load(Ordering::Relaxed);

        // Switch sources when the demo toggle changes.
        let mismatch = match &source {
            Some(Source::Demo(_)) => !want_demo,
            #[cfg(windows)]
            Some(Source::Iracing(_)) => want_demo,
            None => false,
        };
        if mismatch {
            source = None;
            engine.reset();
            last_session_num = i32::MIN;
        }

        if source.is_none() {
            if want_demo {
                source = Some(Source::Demo(Box::default()));
                engine.set_lap_store(None);
            } else {
                #[cfg(windows)]
                {
                    source = crate::irsdk::Irsdk::open().map(Source::Iracing);
                }
                engine.set_lap_store(control.laps_dir.clone().filter(|_| source.is_some()).map(|dir| LapStore { dir }));
                if source.is_none() {
                    report(SourceKind::None, false, "Waiting for iRacing");
                    thread::sleep(RECONNECT_INTERVAL);
                    continue;
                }
            }
        }

        // Pull the next frame.
        let (kind, yaml) = match source.as_mut().expect("source set above") {
            Source::Demo(demo) => {
                thread::sleep(Duration::from_micros(16_667));
                frame.clone_from(demo.step(1.0 / 60.0));
                (SourceKind::Demo, demo.take_session_yaml())
            }
            #[cfg(windows)]
            Source::Iracing(ir) => {
                if !ir.wait_for_data(200) {
                    if !ir.is_connected() {
                        report(SourceKind::Iracing, false, "iRacing is open, waiting for a session");
                        engine.reset();
                        last_session_num = i32::MIN;
                        thread::sleep(Duration::from_millis(250));
                    }
                    continue;
                }
                ir.read_frame(&mut frame);
                (SourceKind::Iracing, ir.session_yaml_if_changed())
            }
        };
        report(kind, true, if kind == SourceKind::Demo { "Demo race" } else { "Connected to iRacing" });

        let session_changed = match yaml {
            Some(text) => match crate::session::parse(&text) {
                Ok(info) => {
                    engine.set_session(info);
                    true
                }
                Err(err) => {
                    eprintln!("[telemetry] session YAML parse failed: {err}");
                    false
                }
            },
            None => false,
        };
        if session_changed || frame.session_num != last_session_num {
            last_session_num = frame.session_num;
            if engine.session().is_some() {
                sink.session(&engine.session_payload(&frame));
                // Whether the reference matches depends on the track.
                sink.reference(&engine.reference_payload());
            }
        }

        sink.tick(&engine.tick(&frame));
        if last_field.elapsed() >= FIELD_INTERVAL {
            last_field = Instant::now();
            sink.field(&engine.field(&frame));
        }
    }
}
