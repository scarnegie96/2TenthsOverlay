//! Session-info YAML parsing.
//!
//! iRacing writes the YAML as ISO-8859-1 and does not quote free-text fields,
//! so a driver called `Max: The "Rocket"` breaks a strict parser. We decode
//! Latin-1, quote the known free-text keys, then read values leniently from a
//! generic YAML tree (numbers sometimes arrive as strings with units).

use serde_yaml::Value;

#[derive(Debug, Clone, Default)]
pub struct DriverInfo {
    pub car_idx: i32,
    pub user_name: String,
    pub abbrev_name: String,
    pub team_name: String,
    pub car_number: String,
    pub car_name: String,
    /// iRacing's internal car id, e.g. "formulair04".
    pub car_path: String,
    pub class_id: i32,
    pub class_short_name: String,
    /// `#rrggbb`
    pub class_color: String,
    pub class_est_lap_time: f32,
    pub irating: i32,
    pub license: String,
    pub license_color: String,
    pub is_spectator: bool,
    pub is_pace_car: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SessionDef {
    pub num: i32,
    pub session_type: String,
    /// None when unlimited.
    pub laps: Option<i32>,
    /// Seconds; None when unlimited.
    pub time: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionInfo {
    pub track_name: String,
    /// iRacing's internal track id (WeekendInfo.TrackName), e.g. "watkinsglen 2021 fullcourse".
    pub track_key: String,
    pub track_config: String,
    pub track_length_km: f32,
    pub player_car_idx: i32,
    pub fuel_max_ltr: f32,
    pub est_lap_time: f32,
    pub shift_rpm: f32,
    /// Where the shift lights are all lit / start blinking (0 when not given).
    pub last_rpm: f32,
    pub blink_rpm: f32,
    pub redline_rpm: f32,
    pub drivers: Vec<DriverInfo>,
    pub sessions: Vec<SessionDef>,
    /// Compound names by index (DriverInfo.DriverTires).
    pub tire_compounds: Vec<String>,
    /// Sector start points as lap fractions (SplitTimeInfo.Sectors).
    pub sectors: Vec<f32>,
    /// WeekendOptions.IncidentLimit; None when unlimited.
    pub incident_limit: Option<i32>,
    /// WeekendOptions.FastRepairsLimit; None when unlimited.
    pub fast_repairs_limit: Option<i32>,
    pub team_racing: bool,
    /// WeekendInfo.Official: an official session, where iRating changes.
    pub official: bool,
    /// Qualifying order (QualifyResultsInfo.Results): car indices, fastest first.
    pub qualifying: Vec<i32>,
}

impl SessionInfo {
    /// Where the car qualified within its class (1-based), counted from the
    /// qualifying order so it doesn't matter whether iRacing's numbers start
    /// at 0 or 1.
    pub fn qualify_class_position(&self, car_idx: i32) -> Option<i32> {
        let at = self.qualifying.iter().position(|&c| c == car_idx)?;
        let class = self.driver(car_idx).map(|d| d.class_id);
        let ahead = self.qualifying[..at].iter().filter(|&&c| self.driver(c).map(|d| d.class_id) == class).count();
        Some(ahead as i32 + 1)
    }

    pub fn driver(&self, car_idx: i32) -> Option<&DriverInfo> {
        self.drivers.iter().find(|d| d.car_idx == car_idx)
    }

    pub fn player(&self) -> Option<&DriverInfo> {
        self.driver(self.player_car_idx)
    }

    pub fn session(&self, num: i32) -> Option<&SessionDef> {
        self.sessions.iter().find(|s| s.num == num)
    }

    pub fn is_multiclass(&self) -> bool {
        let mut ids = self
            .drivers
            .iter()
            .filter(|d| !d.is_spectator && !d.is_pace_car)
            .map(|d| d.class_id);
        match ids.next() {
            Some(first) => ids.any(|id| id != first),
            None => false,
        }
    }
}

/// Decode the raw bytes from shared memory (Latin-1, NUL terminated).
pub fn decode_latin1(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|&b| b as char).collect()
}

/// Keys whose values are free text typed by users and must be quoted.
const FREE_TEXT_KEYS: &[&str] = &[
    "UserName",
    "TeamName",
    "AbbrevName",
    "Initials",
    "CarScreenName",
    "CarScreenNameShort",
    "CarClassShortName",
    "TrackDisplayName",
    "TrackDisplayShortName",
    "TrackConfigName",
    "TrackCity",
    "TrackCountry",
    "SeriesName",
    "SessionName",
    "ClubName",
    "DivisionName",
];

pub fn sanitize(yaml: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + 1024);
    for line in yaml.lines() {
        out.push_str(&sanitize_line(line));
        out.push('\n');
    }
    out
}

fn sanitize_line(line: &str) -> String {
    let trimmed = line.trim_start();
    let body = trimmed.strip_prefix("- ").unwrap_or(trimmed);
    if let Some((key, value)) = body.split_once(':') {
        if FREE_TEXT_KEYS.contains(&key) {
            let value = value.trim();
            if value.is_empty() || value.starts_with('"') {
                return line.to_string();
            }
            let prefix_len = line.len() - body.len();
            let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
            return format!("{}{}: \"{}\"", &line[..prefix_len], key, escaped);
        }
    }
    line.to_string()
}

pub fn parse(yaml: &str) -> Result<SessionInfo, serde_yaml::Error> {
    let root: Value = serde_yaml::from_str(&sanitize(yaml))?;
    let weekend = &root["WeekendInfo"];
    let driver_info = &root["DriverInfo"];

    let drivers = driver_info["Drivers"]
        .as_sequence()
        .map(|seq| seq.iter().map(parse_driver).collect())
        .unwrap_or_default();

    let sessions = root["SessionInfo"]["Sessions"]
        .as_sequence()
        .map(|seq| {
            seq.iter()
                .map(|s| SessionDef {
                    num: int(&s["SessionNum"]).unwrap_or(0),
                    session_type: text(&s["SessionType"]),
                    laps: int(&s["SessionLaps"]),
                    time: float(&s["SessionTime"]),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(SessionInfo {
        track_name: text(&weekend["TrackDisplayName"]),
        track_key: text(&weekend["TrackName"]),
        track_config: text(&weekend["TrackConfigName"]),
        track_length_km: float(&weekend["TrackLength"]).unwrap_or(0.0) as f32,
        player_car_idx: int(&driver_info["DriverCarIdx"]).unwrap_or(0),
        fuel_max_ltr: float(&driver_info["DriverCarFuelMaxLtr"]).unwrap_or(0.0) as f32,
        est_lap_time: float(&driver_info["DriverCarEstLapTime"]).unwrap_or(0.0) as f32,
        shift_rpm: float(&driver_info["DriverCarSLShiftRPM"]).unwrap_or(0.0) as f32,
        last_rpm: float(&driver_info["DriverCarSLLastRPM"]).unwrap_or(0.0) as f32,
        blink_rpm: float(&driver_info["DriverCarSLBlinkRPM"]).unwrap_or(0.0) as f32,
        redline_rpm: float(&driver_info["DriverCarRedLine"]).unwrap_or(0.0) as f32,
        drivers,
        sessions,
        // "unlimited" doesn't parse as a number, so it comes out as None.
        incident_limit: int(&weekend["WeekendOptions"]["IncidentLimit"]),
        fast_repairs_limit: int(&weekend["WeekendOptions"]["FastRepairsLimit"]),
        team_racing: int(&weekend["TeamRacing"]).unwrap_or(0) != 0,
        official: int(&weekend["Official"]).unwrap_or(0) != 0,
        qualifying: {
            let mut q: Vec<(i64, i32)> = root["QualifyResultsInfo"]["Results"]
                .as_sequence()
                .map(|seq| {
                    seq.iter()
                        .filter_map(|r| Some((int(&r["Position"])? as i64, int(&r["CarIdx"])?)))
                        .collect()
                })
                .unwrap_or_default();
            q.sort_by_key(|x| x.0);
            q.into_iter().map(|x| x.1).collect()
        },
        sectors: {
            let mut v: Vec<f32> = root["SplitTimeInfo"]["Sectors"]
                .as_sequence()
                .map(|seq| seq.iter().filter_map(|x| float(&x["SectorStartPct"]).map(|f| f as f32)).collect())
                .unwrap_or_default();
            v.sort_by(|a, b| a.total_cmp(b));
            v.dedup();
            v
        },
        tire_compounds: driver_info["DriverTires"]
            .as_sequence()
            .map(|seq| {
                let mut v: Vec<(i32, String)> = seq
                    .iter()
                    .map(|t| (int(&t["TireIndex"]).unwrap_or(0), text(&t["TireCompoundType"])))
                    .collect();
                v.sort_by_key(|(i, _)| *i);
                v.into_iter().map(|(_, name)| name).collect()
            })
            .unwrap_or_default(),
    })
}

fn parse_driver(d: &Value) -> DriverInfo {
    DriverInfo {
        car_idx: int(&d["CarIdx"]).unwrap_or(-1),
        user_name: text(&d["UserName"]),
        abbrev_name: text(&d["AbbrevName"]),
        team_name: text(&d["TeamName"]),
        car_number: text(&d["CarNumber"]),
        car_name: text(&d["CarScreenNameShort"]),
        car_path: text(&d["CarPath"]),
        class_id: int(&d["CarClassID"]).unwrap_or(0),
        class_short_name: text(&d["CarClassShortName"]),
        class_color: color(&d["CarClassColor"]),
        class_est_lap_time: float(&d["CarClassEstLapTime"]).unwrap_or(0.0) as f32,
        irating: int(&d["IRating"]).unwrap_or(0),
        license: text(&d["LicString"]),
        license_color: color(&d["LicColor"]),
        is_spectator: int(&d["IsSpectator"]).unwrap_or(0) != 0,
        is_pace_car: int(&d["CarIsPaceCar"]).unwrap_or(0) != 0,
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

/// Reads numbers that may be plain (`88.12`) or carry units (`7.00 km`, `1800.0000 sec`).
fn float(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let token = s.split_whitespace().next()?;
            token.parse::<f64>().ok()
        }
        _ => None,
    }
}

fn int(v: &Value) -> Option<i32> {
    match v {
        Value::Number(n) => n.as_i64().map(|i| i as i32).or_else(|| n.as_f64().map(|f| f as i32)),
        Value::String(s) => {
            let token = s.split_whitespace().next()?;
            if let Some(hex) = token.strip_prefix("0x") {
                i64::from_str_radix(hex, 16).ok().map(|i| i as i32)
            } else {
                token.parse::<f64>().ok().map(|f| f as i32)
            }
        }
        _ => None,
    }
}

/// iRacing colors look like `0xffda59` (parsed by YAML as an int) -> `#ffda59`.
fn color(v: &Value) -> String {
    let n = match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => {
            let s = s.trim().trim_start_matches("0x").trim_start_matches('#');
            i64::from_str_radix(s, 16).ok()
        }
        _ => None,
    };
    match n {
        Some(n) => format!("#{:06x}", n & 0xff_ffff),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"---
WeekendInfo:
 TrackName: spa 2024 gp
 TrackDisplayName: Circuit de Spa-Francorchamps
 TrackConfigName: Grand Prix Pits
 TrackLength: 6.93 km
 TeamRacing: 0
 WeekendOptions:
  IncidentLimit: 17
  FastRepairsLimit: unlimited
SplitTimeInfo:
 Sectors:
 - SectorNum: 0
   SectorStartPct: 0.000000
 - SectorNum: 1
   SectorStartPct: 0.312400
 - SectorNum: 2
   SectorStartPct: 0.689900
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: unlimited
   SessionTime: 1800.0000 sec
   SessionType: Practice
 - SessionNum: 2
   SessionLaps: 25
   SessionTime: unlimited
   SessionType: Race
DriverInfo:
 DriverCarIdx: 3
 DriverCarFuelMaxLtr: 120.000
 DriverCarEstLapTime: 137.4500
 DriverCarSLShiftRPM: 7900.000
 DriverCarRedLine: 8500.000
 DriverTires:
 - TireIndex: 1
   TireCompoundType: "Wet"
 - TireIndex: 0
   TireCompoundType: "Hard"
 Drivers:
 - CarIdx: 0
   UserName: Pace Car
   CarNumber: "0"
   CarIsPaceCar: 1
   CarClassID: 11
 - CarIdx: 3
   UserName: Max: The "Rocket" O\Driver
   AbbrevName: Driver, M
   CarNumber: "12"
   CarScreenNameShort: BMW M4 GT3
   CarClassID: 4029
   CarClassShortName: GT3 Class
   CarClassColor: 0xffda59
   CarClassEstLapTime: 137.4500
   IRating: 2431
   LicString: A 3.45
   LicColor: 0x0153db
   IsSpectator: 0
 - CarIdx: 4
   UserName: Ana Lopes
   CarNumber: "7"
   CarClassID: 4048
   CarClassColor: 0x33ceff
   IRating: 1876
   LicString: B 2.10
"#;

    #[test]
    fn parses_quirky_yaml() {
        let info = parse(SAMPLE).expect("parse");
        assert_eq!(info.track_name, "Circuit de Spa-Francorchamps");
        assert!((info.track_length_km - 6.93).abs() < 1e-3);
        assert_eq!(info.player_car_idx, 3);
        assert_eq!(info.fuel_max_ltr, 120.0);

        let me = info.player().unwrap();
        assert_eq!(me.user_name, r#"Max: The "Rocket" O\Driver"#);
        assert_eq!(me.car_number, "12");
        assert_eq!(me.class_color, "#ffda59");
        assert_eq!(me.license_color, "#0153db");
        assert_eq!(me.irating, 2431);

        assert!(info.driver(0).unwrap().is_pace_car);
        assert!(info.is_multiclass());
        assert_eq!(info.tire_compounds, vec!["Hard", "Wet"]);
        assert_eq!(info.sectors.len(), 3);
        assert!((info.sectors[1] - 0.3124).abs() < 1e-4);
        assert_eq!(info.incident_limit, Some(17));
        assert_eq!(info.fast_repairs_limit, None);
        assert!(!info.team_racing);

        let race = info.session(2).unwrap();
        assert_eq!(race.laps, Some(25));
        assert_eq!(race.time, None);
        assert_eq!(info.session(0).unwrap().time, Some(1800.0));
    }

    #[test]
    fn decodes_latin1() {
        let bytes = [b'J', 0xF6, b'r', b'g', 0, b'x'];
        assert_eq!(decode_latin1(&bytes), "Jörg");
    }
}
