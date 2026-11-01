//! Checks the Weather widget's compass maths against a real iRacing .ibt file.
//!
//!   cargo run -p telemetry --example compass_check -- "path/to/file.ibt"
//!   cargo run -p telemetry --example compass_check -- --selftest
//!
//! The car's heading: an .ibt has the car's GPS position (Lat, Lon) as well as
//! YawNorth, so the direction the car actually travelled (from one position to
//! the next) can be compared with the heading Tenths works out from YawNorth.
//! If Tenths' sign is right, they agree to a few degrees; if it's backwards,
//! they disagree everywhere except heading due north or south.
//!
//! The wind: prints WindDir (and the session's TrackWindDir) as compass
//! bearings, to compare with the wind iRacing shows in the sim or with the
//! direction set for a hosted session. Tenths reads WindDir as where the wind
//! blows FROM.
use std::f64::consts::{PI, TAU};
use telemetry::engine::heading_from_yaw_north;
use telemetry::ibt::{self, IbtFile};
use telemetry::vars::VarType;

fn main() {
    let arg = std::env::args().nth(1).expect("usage: compass_check <file.ibt> | --selftest");
    let file = if arg == "--selftest" {
        IbtFile::from_bytes(selftest_file()).unwrap()
    } else {
        IbtFile::open(std::path::Path::new(&arg)).unwrap_or_else(|e| panic!("{arg}: {e}"))
    };
    check(&file);
}

fn bearing(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    // Initial great-circle bearing, radians clockwise from north.
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    y.atan2(x).rem_euclid(TAU)
}

fn diff(a: f64, b: f64) -> f64 {
    ((a - b + PI).rem_euclid(TAU) - PI).abs()
}

fn compass(r: f64) -> String {
    const NAMES: [&str; 16] = ["N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"];
    let deg = r.to_degrees().rem_euclid(360.0);
    format!("{deg:5.1}° ({})", NAMES[((deg / 22.5).round() as usize) % 16])
}

fn check(f: &IbtFile) {
    let n = f.row_count();
    let has = |name: &str| f.value(0, name).is_some();
    for v in ["Lat", "Lon", "YawNorth", "Speed", "WindDir", "WindVel"] {
        println!("{v:>9}: {}", if has(v) { "recorded" } else { "MISSING" });
    }

    // Heading: compare over ~0.5 s steps (30 rows at 60 Hz) while moving.
    if has("Lat") && has("Lon") && has("YawNorth") {
        let step = 30;
        let (mut ok, mut flipped, mut count) = (0.0, 0.0, 0u32);
        let mut worst = 0.0f64;
        for r in (0..n.saturating_sub(step)).step_by(step) {
            let v = |row: usize, name: &str| f.value(row, name).unwrap_or(0.0);
            if v(r, "Speed") < 15.0 || v(r + step, "Speed") < 15.0 {
                continue; // slow or stopped: the GPS direction is noise
            }
            let (la1, lo1, la2, lo2) = (v(r, "Lat"), v(r, "Lon"), v(r + step, "Lat"), v(r + step, "Lon"));
            if la1 == 0.0 || la2 == 0.0 {
                continue;
            }
            let travelled = bearing(la1, lo1, la2, lo2);
            // Heading half way through the step (the travel direction is an average).
            let yaw = v(r + step / 2, "YawNorth") as f32;
            let tenths = heading_from_yaw_north(yaw) as f64;
            let other = heading_from_yaw_north(-yaw) as f64;
            let d = diff(travelled, tenths);
            ok += d;
            flipped += diff(travelled, other);
            worst = worst.max(d);
            count += 1;
        }
        if count == 0 {
            println!("\nHeading: no stretches at speed to compare.");
        } else {
            let (ok, flipped) = (ok / count as f64, flipped / count as f64);
            println!("\nHeading, over {count} half-second stretches at speed:");
            println!("  Tenths' heading vs the way the car travelled: {:.1}° apart on average", ok.to_degrees());
            println!("  The opposite sign would be:                   {:.1}° apart on average", flipped.to_degrees());
            println!(
                "  => {}",
                if ok < 10f64.to_radians() && ok < flipped {
                    "Tenths' heading is RIGHT"
                } else if flipped < ok {
                    "Tenths' heading is BACKWARDS (YawNorth sign)"
                } else {
                    "unclear: check the file has a few laps at speed"
                }
            );
        }
    } else {
        println!("\nHeading: this file has no GPS position or YawNorth to check against.");
    }

    // Wind: as compass bearings, read as where it blows from.
    if has("WindDir") {
        let dirs: Vec<f64> = (0..n).step_by(60).filter_map(|r| f.value(r, "WindDir")).collect();
        let speed = f.value(n / 2, "WindVel").unwrap_or(0.0);
        if let (Some(first), Some(last)) = (dirs.first(), dirs.last()) {
            println!("\nWind (WindDir, read by Tenths as where it blows FROM):");
            println!("  start {}   end {}   speed mid-file {:.1} m/s ({:.0} km/h, {:.0} mph)",
                compass(*first), compass(*last), speed, speed * 3.6, speed * 2.23694);
            println!("  If iRacing shows the opposite direction, Tenths has the wind backwards.");
        }
    }
}

/// A made-up file: a car going round a circle clockwise (seen from above),
/// with YawNorth set the way Tenths assumes, so the check should say RIGHT.
fn selftest_file() -> Vec<u8> {
    let cols = [
        ("Lat", VarType::Double),
        ("Lon", VarType::Double),
        ("YawNorth", VarType::Float),
        ("Speed", VarType::Float),
        ("WindDir", VarType::Float),
        ("WindVel", VarType::Float),
    ];
    let (lat0, lon0, radius_m, speed) = (52.0f64, -1.0f64, 400.0, 40.0);
    let m_per_deg_lat = 111_320.0;
    let m_per_deg_lon = m_per_deg_lat * lat0.to_radians().cos();
    let rows: Vec<Vec<f64>> = (0..60 * 120)
        .map(|i| {
            let t = i as f64 / 60.0;
            let a = speed * t / radius_m; // angle round the circle, clockwise from north
            let (north, east) = (radius_m * a.cos(), radius_m * a.sin());
            let heading = (a + PI / 2.0).rem_euclid(TAU); // clockwise travel
            vec![lat0 + north / m_per_deg_lat, lon0 + east / m_per_deg_lon, heading, speed, 5.5, 4.0]
        })
        .collect();
    ibt::write("WeekendInfo:\n TrackName: selftest\n", &cols, &rows)
}
