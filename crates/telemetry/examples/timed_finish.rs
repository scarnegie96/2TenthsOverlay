//! Shows how a timed race ended in a recording, to check which rule iRacing
//! uses (see `engine::TimedFinish`): does the chequered flag come out at the
//! leader's first crossing after the clock reaches zero, or the white flag?
//!   cargo run -p telemetry --example timed_finish -- path/to/race.ibt [seconds]
//!
//! Prints one row a second for the last `seconds` (default 180) of the file,
//! plus a row whenever the white or chequered flag changes.
use std::path::Path;

use telemetry::ibt::IbtFile;

const WHITE: u32 = 0x0002;
const CHEQUERED: u32 = 0x0001;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: timed_finish <file.ibt> [seconds]");
        std::process::exit(2);
    };
    let window: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(180.0);
    let ibt = IbtFile::open(Path::new(&path)).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let rows = ibt.row_count();
    let time = |r| ibt.value(r, "SessionTime");
    let Some(end) = (0..rows).rev().find_map(time) else {
        eprintln!("No SessionTime in this file.");
        std::process::exit(1);
    };
    for name in ["SessionTimeRemain", "SessionLapsRemainEx", "SessionFlags", "Lap", "LapDistPct"] {
        if ibt.value(0, name).is_none() {
            eprintln!("note: {name} isn't in this file");
        }
    }

    let num = |v: Option<f64>, digits: usize| v.map(|x| format!("{x:.digits$}")).unwrap_or_else(|| "-".into());
    let decode = |flags: u32| {
        let mut out = Vec::new();
        if flags & WHITE != 0 {
            out.push("white");
        }
        if flags & CHEQUERED != 0 {
            out.push("chequered");
        }
        out.join("+")
    };

    println!("{:>10} {:>10} {:>9} {:>10} {:<16} {:>5} {:>7}", "time", "remain", "lapsRem", "flags", "", "lap", "pct");
    let mut next_print = end - window;
    let mut last_flags: Option<u32> = None;
    for r in 0..rows {
        let Some(t) = time(r) else { continue };
        let flags = ibt.value(r, "SessionFlags").map(|v| v as u32);
        let masked = flags.map(|f| f & (WHITE | CHEQUERED));
        let changed = last_flags.is_some() && masked != last_flags;
        last_flags = masked;
        if t < end - window && !changed {
            continue;
        }
        if t < next_print && !changed {
            continue;
        }
        if t >= next_print {
            next_print = t.floor() + 1.0;
        }
        println!(
            "{:>10.3} {:>10} {:>9} {:>10} {:<16} {:>5} {:>7}{}",
            t,
            num(ibt.value(r, "SessionTimeRemain"), 2),
            num(ibt.value(r, "SessionLapsRemainEx"), 0),
            flags.map(|f| format!("{f:#x}")).unwrap_or_else(|| "-".into()),
            flags.map(decode).unwrap_or_default(),
            num(ibt.value(r, "Lap"), 0),
            num(ibt.value(r, "LapDistPct"), 4),
            if changed { "   <- flags changed" } else { "" },
        );
    }
}
