//! How long the engine takes per frame: `cargo run --release -p telemetry --example bench_engine [file.ibt]`
//! Plays a recording (or the demo race) through the engine as fast as it will go, timing `tick`
//! (the 60 Hz work) and `field` (the 10 Hz work, once every 6 ticks here).
use std::time::{Duration, Instant};
use telemetry::{demo::DemoSource, engine::Engine, model::RawFrame, replay::ReplaySource};

fn main() {
    let mut replay = match std::env::args().nth(1) {
        Some(path) => ReplaySource::open(std::path::Path::new(&path)).expect("open .ibt"),
        None => {
            let bytes = telemetry::replay::record_full_ibt(&mut DemoSource::new(), 3.0);
            ReplaySource::from_file(telemetry::ibt::IbtFile::from_bytes(bytes).unwrap(), "demo".into()).unwrap()
        }
    };
    let mut engine = Engine::new();
    let mut f = RawFrame::with_capacity();
    let (mut tick_t, mut field_t) = (Duration::ZERO, Duration::ZERO);
    let (mut ticks, mut fields) = (0u32, 0u32);
    while replay.read(&mut f) {
        if let Some(y) = replay.take_session_yaml() {
            engine.set_session(telemetry::session::parse(&y).unwrap());
        }
        let t = Instant::now();
        std::hint::black_box(engine.tick(&f));
        tick_t += t.elapsed();
        ticks += 1;
        if ticks % 6 == 0 {
            let t = Instant::now();
            std::hint::black_box(engine.field(&f));
            field_t += t.elapsed();
            fields += 1;
        }
    }
    let us = |d: Duration, n: u32| d.as_secs_f64() * 1e6 / n.max(1) as f64;
    println!("{ticks} frames");
    println!("tick  (60 Hz): {:>7.1} us each  -> {:.2} ms per second of play", us(tick_t, ticks), us(tick_t, ticks) * 60.0 / 1000.0);
    println!("field (10 Hz): {:>7.1} us each  -> {:.2} ms per second of play", us(field_t, fields), us(field_t, fields) * 10.0 / 1000.0);
}
