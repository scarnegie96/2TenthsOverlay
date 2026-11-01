//! Words for the coaching cues (`src/lib/coach.ts`). The engine decides what
//! to say and sends the numbers; this says it, in the user's units.

use telemetry::coach::{Cue, CueKind, Issue};

use super::format::{fixed, js_round, signed, speed, speed_unit};
use crate::config::Units;

fn pct(v: Option<f32>) -> String {
    format!("{}%", js_round(v.unwrap_or(0.0) as f64 * 100.0))
}

fn metres(v: Option<f32>) -> String {
    let m = v.unwrap_or(0.0).abs() as f64;
    let m = if m >= 30.0 { js_round(m / 10.0) * 10.0 } else { js_round(m) };
    format!("{m} m")
}

fn ordinal(n: Option<f32>) -> String {
    let g = n.unwrap_or(0.0) as i64;
    let s = match g {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    };
    format!("{g}{s}")
}

fn gear_n(g: Option<i32>) -> Option<f32> {
    g.map(|g| g as f32)
}

/// A few words for why a corner cost time, as in the focus line.
pub fn issue_text(issue: Option<Issue>) -> &'static str {
    match issue {
        Some(Issue::Gained) => "faster than the reference",
        Some(Issue::BrakedEarly) => "braking early",
        Some(Issue::BrakedLate) => "braking late, slow apex",
        Some(Issue::BrakeHarder) => "braking too lightly",
        Some(Issue::SlowApex) => "slow at the apex",
        Some(Issue::FastEntry) => "too fast in, slow out",
        Some(Issue::LatePickup) => "late on the throttle",
        Some(Issue::LiftedEarly) => "lifting early",
        Some(Issue::LiftedTooMuch) => "lifting too much",
        Some(Issue::WrongGear) => "gear at the apex",
        Some(Issue::Entry) => "entry",
        Some(Issue::Exit) => "exit",
        None => "",
    }
}

pub struct Text {
    pub label: String,
    pub detail: String,
}

fn t(label: impl Into<String>, detail: impl Into<String>) -> Text {
    Text { label: label.into(), detail: detail.into() }
}

/// The corner summary given on the way out: the fix first, then the cost.
fn verdict_text(cue: &Cue, units: Units) -> Text {
    let unit = speed_unit(units);
    let spd = |ms: Option<f32>| speed(ms.unwrap_or(0.0), units).to_string();
    let time = cue.time_s.unwrap_or(0.0) as f64;
    let where_ = format!("Corner {}", cue.corner.map(|c| c.to_string()).unwrap_or("?".into()));
    let cost = format!("{where_}: {} {}s", if time >= 0.0 { "lost" } else { "gained" }, fixed(time.abs(), 2));
    let late = cue.distance_m.unwrap_or(0.0) >= 0.0;
    match cue.issue {
        Some(Issue::Gained) => t(format!("Gained {}s", fixed(time.abs(), 2)), format!("{where_}, faster than the reference")),
        Some(Issue::BrakedEarly) => t(format!("Braked {} early", metres(cue.distance_m)), cost),
        Some(Issue::BrakedLate) => t(format!("Braked {} late", metres(cue.distance_m)), format!("{cost}, {} vs {} {unit} at the apex", spd(cue.ours), spd(cue.reference))),
        Some(Issue::BrakeHarder) => t("Brake harder", format!("{cost}, peak {} vs {}", pct(cue.ours), pct(cue.reference))),
        Some(Issue::SlowApex) => t("Carry more speed", format!("{cost}, {} vs {} {unit} at the apex", spd(cue.ours), spd(cue.reference))),
        Some(Issue::FastEntry) => t("Slower in, faster out", format!("{cost}, {} vs {} {unit} at the apex", spd(cue.ours), spd(cue.reference))),
        Some(Issue::LatePickup) => t("Throttle earlier", format!("{cost}, {} {} than the reference", metres(cue.distance_m), if late { "later" } else { "earlier" })),
        Some(Issue::LiftedEarly) => t(format!("Lifted {} early", metres(cue.distance_m)), cost),
        Some(Issue::LiftedTooMuch) => t("Lift less", format!("{cost}, down to {} vs {} throttle", pct(cue.ours), pct(cue.reference))),
        Some(Issue::WrongGear) => t(format!("Use {} at the apex", ordinal(gear_n(cue.gear))), format!("{cost}, you were in {}", ordinal(cue.ours))),
        Some(Issue::Entry) => t(format!("{}s on entry", signed(time, 2)), format!("{where_}, up to the apex")),
        Some(Issue::Exit) => t(format!("{}s on exit", signed(time, 2)), format!("{where_}, from the apex on")),
        None => t(format!("{}s", signed(time, 2)), where_),
    }
}

pub fn cue_text(cue: &Cue, units: Units) -> Text {
    let spd = |ms: Option<f32>| speed(ms.unwrap_or(0.0), units).to_string();
    let diff = || format!("{} {}", speed((cue.ours.unwrap_or(0.0) - cue.reference.unwrap_or(0.0)).abs(), units), speed_unit(units));
    let gear = cue.gear.map(|g| g.to_string()).unwrap_or_default();
    match cue.kind {
        CueKind::BrakeSoon => t(format!("Brake in {}", metres(cue.distance_m)), "Reference brake point ahead"),
        CueKind::LiftSoon => t(format!("Lift in {}", metres(cue.distance_m)), format!("Reference lifts to {}, no brake", pct(cue.reference))),
        CueKind::BrakeNow => t("Brake now", format!("Reference braked {} ago", metres(cue.distance_m))),
        CueKind::BrakeEarly => t("Braking early", format!("Reference brakes {} later", metres(cue.distance_m))),
        CueKind::NoBrakeHere => t("No braking here", "Reference stays off the brakes"),
        CueKind::BrakeHarder => t("Brake harder", format!("You {}, reference {} from its brake point", pct(cue.ours), pct(cue.reference))),
        CueKind::BrakeLighter => t("Brake lighter", format!("You {}, reference {} from its brake point", pct(cue.ours), pct(cue.reference))),
        CueKind::GoodBraking => t("Good braking", format!("Matching the reference at {}", pct(cue.reference))),
        CueKind::HoldBrake => t("Trail the brake", format!("Reference still at {} here", pct(cue.reference))),
        CueKind::ReleaseBrake => t("Release the brake", "Reference is off the brake here"),
        CueKind::LiftLess => t("Lift less", format!("You {}, reference {}", pct(cue.ours), pct(cue.reference))),
        CueKind::CarrySpeed => t("Carry more speed", format!("{} vs {} {} at the apex", spd(cue.ours), spd(cue.reference), speed_unit(units))),
        CueKind::FastApex => t("Faster at the apex", format!("{} up on the reference", diff())),
        CueKind::ThrottleNow => t("Throttle now", format!("Reference is already at {}", pct(cue.reference))),
        CueKind::MoreThrottle => t("More throttle", format!("You {}, reference {}", pct(cue.ours), pct(cue.reference))),
        CueKind::ShiftDown => t(format!("Shift down to {gear}"), format!("Reference uses {} here", ordinal(gear_n(cue.gear)))),
        CueKind::ShiftUp => t(format!("Shift up to {gear}"), format!("Reference uses {} here", ordinal(gear_n(cue.gear)))),
        CueKind::BetterExit => t("Better exit needed", format!("{} down on the straight", diff())),
        CueKind::TurnMore => t("Turn in more", "Reference turns tighter here"),
        CueKind::TurnLess => t("Open the wheel", "You're turning tighter than the reference"),
        CueKind::CornerVerdict => verdict_text(cue, units),
        CueKind::Gaining => t("Gaining", format!("{}s in the last 2 seconds", fixed(cue.ours.unwrap_or(0.0) as f64, 2))),
        CueKind::Losing => t("Losing time", format!("{}s in the last 2 seconds", fixed(cue.ours.unwrap_or(0.0) as f64, 2))),
        CueKind::OnPace => t("On reference pace", "Matching the reference"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_words() {
        assert_eq!(metres(Some(-34.0)), "30 m");
        assert_eq!(metres(Some(12.4)), "12 m");
        assert_eq!(metres(Some(46.0)), "50 m");
        assert_eq!(pct(Some(0.856)), "86%");
        assert_eq!(ordinal(Some(1.0)), "1st");
        assert_eq!(ordinal(Some(2.0)), "2nd");
        assert_eq!(ordinal(Some(3.0)), "3rd");
        assert_eq!(ordinal(Some(5.0)), "5th");
        assert_eq!(issue_text(Some(Issue::LatePickup)), "late on the throttle");
        assert_eq!(issue_text(None), "");
    }
}
