//! Writes a demo-race .ibt with every channel Tenths reads, other cars included,
//! to try the replay mode without a recording of your own.
//!   cargo run -p telemetry --example make_replay_ibt -- demo-race.ibt 3
//!   tenths --replay demo-race.ibt --speed 2
//! (The second number is laps of the demo track, about 92 s each.)
use telemetry::demo::DemoSource;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap_or_else(|| "demo-race.ibt".into());
    let laps: f32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(3.0);
    let bytes = telemetry::replay::record_full_ibt(&mut DemoSource::new(), laps);
    std::fs::write(&path, &bytes).expect("write file");
    let file = telemetry::ibt::IbtFile::from_bytes(bytes).expect("reads back");
    println!("Wrote {path}: {} samples ({:.0} s at {} Hz)", file.row_count(), file.row_count() as f64 / file.tick_rate(), file.tick_rate());
}
