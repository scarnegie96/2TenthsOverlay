//! Drives the demo race against a reference recorded from it, and prints
//! how often each coaching cue appears, plus the corner verdicts. A sanity
//! check that cues aren't noise.
//!   cargo run -p telemetry --example coach_demo
use std::collections::BTreeMap;
use telemetry::{demo::{record_ibt, DemoSource}, ibt::IbtFile, Engine};

fn main() {
    let mut demo = DemoSource::new();
    let file = IbtFile::from_bytes(record_ibt(&mut demo, 3.2)).unwrap();
    let lap = file.summary().laps[0].lap;
    let mut engine = Engine::new();
    engine.set_session(telemetry::session::parse(&demo.session_yaml()).unwrap());
    engine.set_reference(Some(file.reference(lap, "demo.ibt").unwrap()));

    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut verdicts: Vec<String> = Vec::new();
    let mut focus = None;
    let mut last = String::new();
    let mut changes = 0;
    for _ in 0..60 * 180 {
        let f = demo.step(1.0 / 60.0).clone();
        let t = engine.tick(&f);
        if let Some(c) = &t.coach {
            focus = c.focus.or(focus);
            if let Some(q) = c.cue.filter(|q| q.kind == telemetry::coach::CueKind::CornerVerdict) {
                let line = format!("T{} {:+.2}s {:?}", q.corner.unwrap_or(0), q.time_s.unwrap_or(0.0), q.issue);
                if verdicts.last() != Some(&line) {
                    verdicts.push(line);
                }
            }
        }
        let kind = t.coach.and_then(|c| c.cue).map(|c| format!("{:?}", c.kind)).unwrap_or("none".into());
        *counts.entry(kind.clone()).or_default() += 1;
        if kind != last {
            changes += 1;
            last = kind;
        }
    }
    println!("cue changes in 3 minutes: {changes}");
    for (k, n) in counts {
        println!("{k:>14}: {:5.1}%", n as f32 / (60.0 * 180.0) * 100.0);
    }
    println!("corner verdicts: {}", verdicts.len());
    for v in &verdicts {
        println!("  {v}");
    }
    println!("focus: {focus:?}");
}
