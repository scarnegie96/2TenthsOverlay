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
use crate::pit::PitCommand;
use crate::reference::ReferenceLap;
use crate::replay::ReplaySource;

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
    /// Where race reports are saved (report.rs); None to not save them.
    pub reports_dir: Option<PathBuf>,
    /// Whether to save race reports (Settings → Races). Can change at any time.
    pub save_reports: Arc<AtomicBool>,
    /// Pit commands waiting to go to the sim (or the demo).
    pub pit: Arc<Mutex<Vec<PitCommand>>>,
    /// Runs the demo race this many times faster than real time (0 or 1:
    /// normal). Every simulated frame still goes through the engine; the sink
    /// gets the last of each batch. For screenshots that need a race well under way.
    pub demo_speed: Arc<std::sync::atomic::AtomicU32>,
    /// Play this .ibt file instead of iRacing or the demo (`--replay`). Set before `spawn`.
    pub replay: Option<PathBuf>,
    /// Replay speed in thousandths of real time (1000 = real time, 0 = paused). Can change at any time.
    pub replay_speed: Arc<std::sync::atomic::AtomicU32>,
    /// Start the replay again when it ends, instead of holding the last frame.
    pub replay_loop: Arc<AtomicBool>,
}

impl RunnerControl {
    pub fn set_reference(&self, reference: Option<ReferenceLap>) {
        *self.reference.lock().unwrap() = reference;
        self.reference_rev.fetch_add(1, Ordering::Relaxed);
    }

    /// Queues a pit command for whichever sim is running. Dropped when none is.
    pub fn pit_command(&self, cmd: PitCommand) {
        self.pit.lock().unwrap().push(cmd);
    }
}

enum Source {
    Demo(Box<DemoSource>),
    Replay(Box<ReplaySource>),
    #[cfg(windows)]
    Iracing(crate::irsdk::Irsdk),
}

const FIELD_INTERVAL: Duration = Duration::from_millis(100);
const RECONNECT_INTERVAL: Duration = Duration::from_millis(1000);
const DEMO_FRAME: Duration = Duration::from_micros(16_667);

/// Paces the demo by the real clock, so time spent working each frame
/// doesn't make it run slow.
struct FrameClock {
    period: Duration,
    deadline: Instant,
    last: Instant,
}

impl FrameClock {
    /// Longest step the demo takes, so a stall doesn't make it jump.
    const MAX_STEP: f64 = 0.1;
    /// Further behind than this and we give up catching up.
    const MAX_BEHIND: Duration = Duration::from_millis(100);

    fn new(period: Duration) -> Self {
        let now = Instant::now();
        Self { period, deadline: now + period, last: now }
    }

    /// Sleeps until the next frame is due. Returns the seconds since the last one.
    fn wait(&mut self) -> f32 {
        let now = Instant::now();
        if let Some(left) = self.deadline.checked_duration_since(now) {
            thread::sleep(left);
        }
        let now = Instant::now();
        self.deadline += self.period;
        if now.saturating_duration_since(self.deadline) > Self::MAX_BEHIND {
            self.deadline = now + self.period;
        }
        let dt = now.duration_since(self.last).as_secs_f64().min(Self::MAX_STEP);
        self.last = now;
        dt as f32
    }
}

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
    let mut last_status: Option<(SourceKind, bool, String)> = None;
    let mut last_session_num = i32::MIN;

    let mut report = |kind: SourceKind, connected: bool, message: &str| {
        // The message counts too: a replay changes it when it ends.
        if last_status.as_ref().is_none_or(|(k, c, m)| (*k, *c, m.as_str()) != (kind, connected, message)) {
            last_status = Some((kind, connected, message.to_string()));
            sink.status(&StatusPayload { source: kind, connected, message: message.to_string() });
        }
    };

    let mut reference_rev = u64::MAX;
    let mut demo_clock = FrameClock::new(DEMO_FRAME);
    let mut replay_label = String::new();
    let mut replay_failed = false;

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
            // A replay is chosen at launch and stays.
            Some(Source::Replay(_)) => false,
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
            if let Some(path) = control.replay.as_ref() {
                if replay_failed {
                    thread::sleep(RECONNECT_INTERVAL);
                    continue;
                }
                match ReplaySource::open(path) {
                    Ok(replay) => {
                        replay_label = format!("Replay: {}", replay.name());
                        source = Some(Source::Replay(Box::new(replay)));
                        demo_clock = FrameClock::new(DEMO_FRAME);
                        engine.reset();
                        last_session_num = i32::MIN;
                        // A replay must not write laps or race reports into the driver's own folders.
                        engine.set_lap_store(None);
                        engine.set_report_store(None);
                    }
                    Err(err) => {
                        eprintln!("[telemetry] replay: {err}");
                        replay_failed = true;
                        report(SourceKind::Replay, false, &err);
                        continue;
                    }
                }
            } else if want_demo {
                source = Some(Source::Demo(Box::default()));
                demo_clock = FrameClock::new(DEMO_FRAME);
                engine.set_lap_store(None);
                engine.set_report_store(None);
            } else {
                #[cfg(windows)]
                {
                    source = crate::irsdk::Irsdk::open().map(Source::Iracing);
                }
                engine.set_lap_store(control.laps_dir.clone().filter(|_| source.is_some()).map(|dir| LapStore { dir }));
                engine.set_report_store(
                    control.reports_dir.clone().filter(|_| source.is_some()).map(|dir| crate::report::ReportStore { dir, enabled: control.save_reports.clone() }),
                );
                if source.is_none() {
                    report(SourceKind::None, false, "Waiting for iRacing");
                    thread::sleep(RECONNECT_INTERVAL);
                    continue;
                }
            }
        }

        let commands = std::mem::take(&mut *control.pit.lock().unwrap());
        for cmd in commands {
            match source.as_mut().expect("source set above") {
                Source::Demo(demo) => demo.pit_command(cmd),
                // Nothing in a recording to send them to.
                Source::Replay(_) => {}
                #[cfg(windows)]
                Source::Iracing(_) => {
                    if !crate::irsdk::send_pit_command(cmd) {
                        eprintln!("[telemetry] pit command {cmd:?} not sent");
                    }
                }
            }
        }

        // Pull the next frame.
        let (kind, yaml) = match source.as_mut().expect("source set above") {
            Source::Demo(demo) => {
                let dt = demo_clock.wait();
                // Fast-forward: the extra frames go through the engine too, so laps,
                // stints and pit stops are all recorded; only the last reaches the sink.
                let speed = control.demo_speed.load(Ordering::Relaxed).max(1);
                let mut yaml = None;
                for _ in 1..speed {
                    frame.clone_from(demo.step(dt));
                    if let Some(text) = demo.take_session_yaml() {
                        if let Ok(info) = crate::session::parse(&text) {
                            engine.set_session(info);
                        }
                        yaml = Some(text);
                    }
                    engine.tick(&frame);
                }
                frame.clone_from(demo.step(dt));
                (SourceKind::Demo, demo.take_session_yaml().or(yaml))
            }
            Source::Replay(replay) => {
                let dt = demo_clock.wait();
                let speed = control.replay_speed.load(Ordering::Relaxed) as f64 / 1000.0;
                let due = replay.advance(dt, speed);
                if due == 0 {
                    if replay.finished() {
                        if control.replay_loop.load(Ordering::Relaxed) {
                            replay.rewind();
                            engine.reset();
                            last_session_num = i32::MIN;
                        } else {
                            report(SourceKind::Replay, true, &format!("{replay_label} (finished)"));
                        }
                    }
                    continue;
                }
                // Every row goes through the engine so laps, stints and stops are
                // recorded; only the last reaches the sink.
                let mut yaml = None;
                for i in 0..due {
                    if !replay.read(&mut frame) {
                        break;
                    }
                    if let Some(text) = replay.take_session_yaml() {
                        if let Ok(info) = crate::session::parse(&text) {
                            engine.set_session(info);
                        }
                        yaml = Some(text);
                    }
                    if i + 1 < due {
                        engine.tick(&frame);
                    }
                }
                (SourceKind::Replay, yaml)
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
        match kind {
            SourceKind::Demo => report(kind, true, "Demo race"),
            SourceKind::Replay => report(kind, true, &replay_label),
            _ => report(kind, true, "Connected to iRacing"),
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_clock_keeps_real_time_despite_work() {
        let mut clock = FrameClock::new(DEMO_FRAME);
        let start = Instant::now();
        let mut stepped = 0.0f64;
        for _ in 0..30 {
            stepped += clock.wait() as f64;
            // Pretend each frame takes 5 ms of work.
            thread::sleep(Duration::from_millis(5));
        }
        let real = start.elapsed().as_secs_f64();
        // 30 frames at 60 Hz is 0.5 s; the old fixed sleep would take 0.65 s.
        assert!(real < 0.6, "took {real:.3} s");
        assert!((stepped - real).abs() < 0.03, "stepped {stepped:.3} s in {real:.3} s");
    }

    #[test]
    fn frame_clock_caps_a_stall() {
        let mut clock = FrameClock::new(DEMO_FRAME);
        thread::sleep(Duration::from_millis(250));
        assert!(clock.wait() <= 0.1 + 1e-6);
        // After a stall it starts again from now, not trying to catch up.
        let t = Instant::now();
        clock.wait();
        assert!(t.elapsed() >= Duration::from_millis(10));
    }

    /// Collects what the runner publishes, for the replay test below.
    #[derive(Default)]
    struct Collected {
        statuses: Vec<(SourceKind, bool, String)>,
        sessions: usize,
        ticks: Vec<f64>,
        fields: Vec<FieldPayload>,
    }
    struct Collector(Arc<Mutex<Collected>>);
    impl Sink for Collector {
        fn status(&self, s: &StatusPayload) {
            self.0.lock().unwrap().statuses.push((s.source, s.connected, s.message.clone()));
        }
        fn reference(&self, _: &ReferencePayload) {}
        fn session(&self, _: &SessionPayload) {
            self.0.lock().unwrap().sessions += 1;
        }
        fn tick(&self, t: &TickPayload) {
            self.0.lock().unwrap().ticks.push(t.session_time);
        }
        fn field(&self, f: &FieldPayload) {
            self.0.lock().unwrap().fields.push(f.clone());
        }
    }

    fn run_replay(path: PathBuf, speed: u32, wait: Duration) -> Collected {
        let got = Arc::new(Mutex::new(Collected::default()));
        let control = RunnerControl { replay: Some(path), ..Default::default() };
        control.replay_speed.store(speed, Ordering::Relaxed);
        let handle = spawn(Collector(got.clone()), control.clone());
        thread::sleep(wait);
        control.stop.store(true, Ordering::Relaxed);
        handle.join().unwrap();
        Arc::try_unwrap(got).ok().unwrap().into_inner().unwrap()
    }

    #[test]
    fn replays_an_ibt_through_the_engine() {
        let mut demo = DemoSource::new();
        let bytes = crate::replay::record_full_ibt(&mut demo, 0.6);
        let path = std::env::temp_dir().join(format!("tenths-replay-test-{}.ibt", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let got = run_replay(path.clone(), 60_000, Duration::from_millis(2500));
        std::fs::remove_file(&path).ok();

        assert!(got.statuses.iter().any(|(k, c, m)| *k == SourceKind::Replay && *c && m.starts_with("Replay: tenths-replay-test")), "{:?}", got.statuses);
        assert!(got.sessions >= 1, "no session payload");
        // At 60x each published tick stands for a batch of rows, all of which went through the engine.
        assert!(got.ticks.len() > 5, "{} ticks", got.ticks.len());
        let (first, last) = (got.ticks[0], *got.ticks.last().unwrap());
        assert!(last - first > 45.0, "only played {:.1} s of the 55 s recorded", last - first);
        assert!(got.ticks.windows(2).all(|w| w[1] >= w[0]), "time went backwards");
        let field = got.fields.last().expect("a field payload");
        assert!(field.standings.len() > 5, "{} standings rows: other cars should come through", field.standings.len());
        assert!(!field.relative.is_empty(), "relative should have cars around the player");
        // It reached the end and said so.
        assert!(got.statuses.last().is_some_and(|(_, _, m)| m.ends_with("(finished)")), "{:?}", got.statuses.last());
    }

    #[test]
    fn a_bad_replay_file_reports_instead_of_hanging() {
        let path = std::env::temp_dir().join(format!("tenths-not-an-ibt-{}.ibt", std::process::id()));
        std::fs::write(&path, b"definitely not telemetry").unwrap();
        let got = run_replay(path.clone(), 1000, Duration::from_millis(300));
        std::fs::remove_file(&path).ok();
        assert!(got.statuses.iter().any(|(k, c, m)| *k == SourceKind::Replay && !*c && m.contains(".ibt")), "{:?}", got.statuses);
        assert!(got.ticks.is_empty());
    }

    #[test]
    fn a_paused_replay_plays_nothing() {
        let mut demo = DemoSource::new();
        let path = std::env::temp_dir().join(format!("tenths-replay-paused-{}.ibt", std::process::id()));
        std::fs::write(&path, crate::replay::record_full_ibt(&mut demo, 0.2)).unwrap();
        let got = run_replay(path.clone(), 0, Duration::from_millis(400));
        std::fs::remove_file(&path).ok();
        assert!(got.ticks.is_empty());
    }
}
