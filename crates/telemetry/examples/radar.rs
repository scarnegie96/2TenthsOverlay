//! Prints what the radar sees in the demo race, once a second.
use telemetry::{demo::DemoSource, Engine};

fn main() {
    let mut demo = DemoSource::new();
    let mut engine = Engine::new();
    engine.set_session(telemetry::session::parse(&demo.take_session_yaml().unwrap()).unwrap());
    for s in 0..40 {
        let mut tick = None;
        for _ in 0..60 {
            let f = demo.step(1.0 / 60.0).clone();
            tick = Some(engine.tick(&f));
        }
        let t = tick.unwrap();
        let cars: Vec<String> = t.nearby.iter().map(|c| format!("#{} {:+.1}m side {}", c.car_idx, c.dist, c.side)).collect();
        println!("{s:>2}s clr={} {}", t.car_left_right, cars.join(" | "));
    }
}
