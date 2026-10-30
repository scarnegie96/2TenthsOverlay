//! Writes a demo-race .ibt file you can import as a reference lap.
//!   cargo run -p telemetry --example make_ibt -- demo.ibt
use telemetry::{demo::DemoSource, ibt};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "demo.ibt".into());
    let bytes = telemetry::demo::record_ibt(&mut DemoSource::new(), 4.2);
    std::fs::write(&path, &bytes).expect("write file");
    let file = ibt::IbtFile::from_bytes(bytes).unwrap();
    let s = file.summary();
    println!("Wrote {path}: {} {}, {} complete laps", s.track_name, s.track_config, s.laps.len());
    for l in s.laps {
        println!("  lap {:>2}  {:.3}s  {}", l.lap, l.lap_time, if l.clean { "clean" } else { "pit/off-track" });
    }
}
