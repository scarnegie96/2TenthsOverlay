//! Races the demo for a while, waves the chequered flag and saves the race
//! report, so the report pipeline can be tried without iRacing:
//!   cargo run -p telemetry --example report_demo -- <folder> [laps]
//! The report lands in <folder> as <id>.json.
use telemetry::{demo::DemoSource, report::ReportStore, Engine};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("folder to save the report in");
    let laps: f64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(12.0);

    let mut engine = Engine::new();
    engine.set_report_store(Some(ReportStore::new(dir.clone())));
    let mut demo = DemoSource::new();
    engine.set_session(telemetry::session::parse(&demo.take_session_yaml().unwrap()).unwrap());

    // About 92 s a lap; then the flag, two laps for everyone to finish, and cool down.
    let race_frames = (laps * 92.0 * 60.0) as usize;
    for k in 0..race_frames + 60 * 200 {
        let mut frame = demo.step(1.0 / 60.0).clone();
        if k >= race_frames {
            frame.session_state = if k < race_frames + 60 * 190 { 5 } else { 6 };
        }
        engine.tick(&frame);
    }
    // Leaving saves the final version (the save itself runs in the background).
    engine.reset();
    std::thread::sleep(std::time::Duration::from_millis(500));
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        println!("saved {}", e.path().display());
    }
}
