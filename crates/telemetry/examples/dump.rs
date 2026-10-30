//! Prints engine output as JSON. Handy for checking maths without the UI.
//!   cargo run -p telemetry --example dump            (demo race)
//!   cargo run -p telemetry --example dump -- live    (iRacing, Windows)
use telemetry::{demo::DemoSource, Engine};

fn main() {
    let live = std::env::args().any(|a| a == "live");
    let mut engine = Engine::new();

    if live {
        #[cfg(windows)]
        {
            let mut ir = telemetry::irsdk::Irsdk::open().expect("iRacing is not running");
            let mut frame = telemetry::RawFrame::with_capacity();
            while !ir.wait_for_data(1000) {}
            if let Some(yaml) = ir.session_yaml_if_changed() {
                engine.set_session(telemetry::session::parse(&yaml).expect("yaml"));
            }
            ir.read_frame(&mut frame);
            engine.tick(&frame);
            println!("{}", serde_json::to_string_pretty(&engine.field(&frame)).unwrap());
            return;
        }
        #[cfg(not(windows))]
        panic!("live mode needs Windows");
    }

    let mut demo = DemoSource::new();
    engine.set_session(telemetry::session::parse(&demo.take_session_yaml().unwrap()).unwrap());
    let mut frame = demo.step(1.0 / 60.0).clone();
    for _ in 0..60 * 300 {
        frame = demo.step(1.0 / 60.0).clone();
        engine.tick(&frame);
    }
    println!("{}", serde_json::to_string_pretty(&engine.tick(&frame)).unwrap());
    println!("{}", serde_json::to_string_pretty(&engine.field(&frame)).unwrap());
}
