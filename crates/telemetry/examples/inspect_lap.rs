//! Prints what the coach sees in a reference lap: each corner (braking or lift)
//! with its entry point, peak pressure or lowest throttle, slowest speed and
//! gear, and the throttle pickup.
//!   cargo run -p telemetry --example inspect_lap -- path/to/lap.blap   (or .olap / .ibt LAP_NUMBER)
use std::path::Path;
use telemetry::{blap::BlapFile, coach::{find_corners, CornerKind}, ibt::IbtFile, reference::REF_BINS};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("pass a .blap, .olap or .ibt file");
    let p = Path::new(&path);
    let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let r = if ext == "ibt" {
        let lap: i32 = args.next().and_then(|a| a.parse().ok()).expect("pass the lap number for .ibt files");
        IbtFile::open(p).unwrap().reference(lap, &path).unwrap()
    } else {
        BlapFile::open(p).unwrap().reference(&path).unwrap()
    };
    let len = r.length_m;
    let m = |b: usize| b as f32 / REF_BINS as f32 * len;
    println!(
        "{} on {} ({}), {:.3}s by {}, {:.0} m",
        r.car_key, r.track_name, r.track_key, r.lap_time, r.driver, len
    );
    println!(
        "{:>3} {:>5} {:>8} {:>8} {:>6} {:>9} {:>5} {:>9}",
        "#", "kind", "entry", "brake to", "pedal", "apex km/h", "gear", "throttle"
    );
    for (i, c) in find_corners(&r).iter().enumerate() {
        let (kind, pedal) = match c.kind {
            CornerKind::Brake => ("brake", c.peak_brake),
            CornerKind::Lift => ("lift", c.min_throttle),
        };
        println!(
            "{:>3} {:>5} {:>7.0}m {:>7.0}m {:>5.0}% {:>9.0} {:>5} {:>8.0}m",
            i + 1,
            kind,
            m(c.entry),
            m(c.brake_end),
            pedal * 100.0,
            c.min_speed.unwrap_or(0.0) * 3.6,
            c.apex_gear,
            m(c.pickup)
        );
    }
}
