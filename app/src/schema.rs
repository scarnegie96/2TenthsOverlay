//! Every widget's id, name, default place and option defaults: the Rust copy
//! of `WIDGETS` in `src/lib/config.ts`. Keep the two in step.

/// An option's default value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Def {
    B(bool),
    N(f64),
    S(&'static str),
}

pub struct Meta {
    pub id: &'static str,
    pub name: &'static str,
    /// Default placement on a 1920×1080 screen.
    pub x: f64,
    pub y: f64,
    pub enabled: bool,
    pub options: &'static [(&'static str, Def)],
}

use Def::*;

pub const WIDGETS: &[Meta] = &[
    Meta {
        id: "relative",
        name: "Relative",
        x: 1396.0,
        y: 600.0,
        enabled: true,
        options: &[
            ("ahead", N(3.0)),
            ("behind", N(3.0)),
            ("showIRating", B(true)),
            ("showLicense", B(true)),
            ("showLastLap", B(true)),
            ("showPit", B(true)),
            ("showInterval", B(true)),
            ("showCompound", S("auto")),
            ("showSoF", B(true)),
            ("showRatings", B(true)),
            ("showIncidents", B(true)),
            ("incidentStep", N(4.0)),
        ],
    },
    Meta {
        id: "standings",
        name: "Standings",
        x: 24.0,
        y: 160.0,
        enabled: true,
        options: &[
            ("rows", N(10.0)),
            ("otherClassRows", N(3.0)),
            ("myClassOnly", B(false)),
            ("gapMode", S("leader")),
            ("showChange", B(true)),
            ("showRejoin", B(true)),
            ("showIRating", B(true)),
            ("showIRatingChange", B(true)),
            ("showLast", B(true)),
            ("showBest", B(true)),
            ("showLaps", B(true)),
            ("showSoF", B(true)),
            ("rateDrivers", B(true)),
            ("showRatings", B(true)),
        ],
    },
    Meta {
        id: "session",
        name: "Session",
        x: 572.0,
        y: 250.0,
        enabled: false,
        options: &[("showClasses", B(true)), ("showConditions", B(true)), ("showLimits", B(true))],
    },
    Meta {
        id: "inputs",
        name: "Inputs",
        x: 600.0,
        y: 940.0,
        enabled: true,
        options: &[
            ("traceSeconds", N(6.0)),
            ("showTrace", B(true)),
            ("showSteering", B(true)),
            ("wheel", S("auto")),
            ("showClutch", B(true)),
            ("shiftLight", B(true)),
            ("showGhost", B(true)),
        ],
    },
    Meta {
        id: "delta",
        name: "Delta",
        x: 572.0,
        y: 24.0,
        enabled: true,
        options: &[
            ("compareTo", S("best")),
            ("sectors", S("track")),
            ("miniSectors", N(12.0)),
            ("showPrevious", B(true)),
            ("showLapTimes", B(true)),
            ("simple", B(false)),
        ],
    },
    Meta {
        id: "laplog",
        name: "Lap log",
        x: 1090.0,
        y: 560.0,
        enabled: true,
        options: &[
            ("rows", N(8.0)),
            ("compareTo", S("session")),
            ("showDelta", B(true)),
            ("showTemp", B(true)),
            ("showFuel", B(false)),
            ("showPersonalBest", B(true)),
        ],
    },
    Meta {
        id: "coach",
        name: "Coach",
        x: 760.0,
        y: 270.0,
        enabled: true,
        options: &[("showDetail", B(true)), ("showStats", B(true)), ("showFocus", B(true))],
    },
    Meta {
        id: "dynamics",
        name: "Car dynamics",
        x: 24.0,
        y: 716.0,
        enabled: true,
        options: &[
            ("maxG", N(3.0)),
            ("envelopeHold", N(10.0)),
            ("envelopeReturn", N(10.0)),
            ("direction", S("felt")),
            ("showAttitude", B(true)),
            ("exaggerate", N(4.0)),
            ("showBalance", B(true)),
        ],
    },
    Meta {
        id: "instruments",
        name: "Instruments",
        x: 1396.0,
        y: 846.0,
        enabled: true,
        options: &[("showGauges", B(true)), ("showBias", B(true)), ("testLights", B(false))],
    },
    Meta { id: "fuel", name: "Fuel", x: 1580.0, y: 340.0, enabled: true, options: &[("marginLaps", N(0.5))] },
    Meta {
        id: "radar",
        name: "Radar",
        x: 842.0,
        y: 596.0,
        enabled: true,
        options: &[("range", N(20.0)), ("alwaysShow", B(true)), ("classColors", B(false))],
    },
    Meta { id: "tires", name: "Tires", x: 1580.0, y: 120.0, enabled: true, options: &[("showPressure", B(true))] },
    Meta {
        id: "pitstop",
        name: "Pit stop",
        x: 572.0,
        y: 250.0,
        enabled: false,
        options: &[
            ("fuelMode", S("finish")),
            ("fuelStep", N(5.0)),
            ("keepFuelSet", B(false)),
            ("autoFuel", B(false)),
            ("showKeys", B(true)),
        ],
    },
    Meta {
        id: "flags",
        name: "Flags",
        x: 1240.0,
        y: 270.0,
        enabled: true,
        options: &[
            ("showCaption", B(true)),
            ("greenSeconds", N(5.0)),
            ("scBoard", S("SC")),
            ("textYellow", S("Accident ahead")),
            ("textCaution", S("Safety car")),
            ("textDebris", S("Debris on track")),
            ("textBlue", S("Faster car behind")),
            ("textWhite", S("Last lap")),
            ("textCheckered", S("Finish")),
            ("textGreen", S("Green flag")),
            ("textOneToGreen", S("Green next lap")),
            ("textRed", S("Session stopped")),
            ("textBlack", S("Penalty: pit now")),
            ("textRepair", S("Damage: pit for repairs")),
            ("textWarning", S("Warning")),
            ("textDisqualified", S("Disqualified")),
            ("alwaysShow", B(false)),
        ],
    },
    Meta {
        id: "weather",
        name: "Weather",
        x: 1396.0,
        y: 24.0,
        enabled: false,
        options: &[("showCompass", B(true)), ("compass", S("car")), ("showTrends", B(true))],
    },
    Meta {
        id: "racepace",
        name: "Race pace",
        x: 572.0,
        y: 700.0,
        enabled: false,
        options: &[
            ("window", S("race")),
            ("show", S("all")),
            ("around", N(3.0)),
            ("maxGap", N(0.0)),
            ("showTrend", B(true)),
        ],
    },
    Meta {
        id: "stints",
        name: "Stints",
        x: 572.0,
        y: 330.0,
        enabled: false,
        options: &[
            ("classes", S("all")),
            ("rows", N(10.0)),
            ("otherClassRows", N(5.0)),
            ("showTimeline", B(true)),
            ("showLengths", B(true)),
            ("showCompound", S("auto")),
        ],
    },
    Meta {
        id: "battle",
        name: "Battle",
        x: 1060.0,
        y: 330.0,
        enabled: false,
        options: &[
            ("trendLaps", N(3.0)),
            ("showSparkline", B(true)),
            ("sparkLaps", N(8.0)),
            ("showLaps", B(true)),
            ("showTyres", B(true)),
        ],
    },
    Meta {
        id: "twitch",
        name: "Twitch chat",
        x: 620.0,
        y: 300.0,
        enabled: false,
        options: &[("channel", S("")), ("rows", N(10.0)), ("fadeAfter", N(60.0)), ("showEmotes", B(true))],
    },
];

pub fn meta(id: &str) -> Option<&'static Meta> {
    WIDGETS.iter().find(|m| m.id == id)
}

impl Meta {
    pub fn default_of(&self, key: &str) -> Option<Def> {
        self.options.iter().find(|(k, _)| *k == key).map(|(_, d)| *d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_widget_once() {
        assert_eq!(WIDGETS.len(), 19);
        for (i, a) in WIDGETS.iter().enumerate() {
            assert!(WIDGETS[i + 1..].iter().all(|b| b.id != a.id), "{} twice", a.id);
        }
    }

    /// The defaults here are a copy of config.ts; check the copy hasn't drifted
    /// when the web source is next to it (it always is in this repo).
    #[test]
    fn matches_config_ts() {
        let Ok(ts) = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../archive/tauri/src/lib/config.ts")) else { return };
        for m in WIDGETS {
            assert!(ts.contains(&format!("id: \"{}\"", m.id)), "{} missing in config.ts", m.id);
            for (key, def) in m.options {
                let at = ts.find(&format!("key: \"{key}\"")).unwrap_or_else(|| panic!("{}.{key} missing in config.ts", m.id));
                let _ = at;
                let want = match def {
                    Def::B(b) => format!("default: {b}"),
                    Def::N(n) => format!("default: {n}"),
                    Def::S(s) => format!("default: \"{s}\""),
                };
                // The option's spec line (from the widget's id onwards) holds its default.
                let start = ts.find(&format!("id: \"{}\"", m.id)).unwrap();
                let spec = &ts[start..];
                let line = spec.find(&format!("key: \"{key}\"")).unwrap();
                let tail = &spec[line..];
                let end = tail.find("\n      {").or_else(|| tail.find("\n    ],")).unwrap_or(tail.len());
                assert!(tail[..end].contains(&want), "{}.{key}: want {want}", m.id);
            }
        }
    }
}
