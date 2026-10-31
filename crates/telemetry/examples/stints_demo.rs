//! Runs the demo race for a while and prints each car's stints, to check the
//! stint tracker against the demo's scheduled pit stops.
//!   cargo run -p telemetry --example stints_demo
use telemetry::{demo::DemoSource, Engine};

fn main() {
    let mut demo = DemoSource::new();
    let mut engine = Engine::new();
    engine.set_session(telemetry::session::parse(&demo.take_session_yaml().unwrap()).unwrap());
    for _ in 0..60 * 60 * 12 {
        let f = demo.step(1.0 / 60.0).clone();
        engine.tick(&f);
    }
    let f = demo.step(1.0 / 60.0).clone();
    let field = engine.field(&f);
    for c in field.stints.iter().take(22) {
        let list: Vec<String> = c
            .stints
            .iter()
            .map(|s| format!("{:.1}-{:.1}{}{}", s.start, s.end, ["H", "W"][s.compound.clamp(0, 1) as usize], if s.partial { "~" } else { "" }))
            .collect();
        println!("car {:>2} stops {} {}{}  {}", c.car_idx, c.stops, if c.in_pits { "PIT " } else { "" }, if c.out { "OUT " } else { "" }, list.join("  "));
    }
}
