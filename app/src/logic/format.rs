//! Text formatting, as `src/lib/format.ts`.

use crate::config::Units;

pub const MINUS: &str = "−";

/// JavaScript's `toFixed`: rounds half away from zero on the decimal string
/// (close enough; JS rounds the binary value, which rarely differs).
pub fn fixed(v: f64, decimals: usize) -> String {
    format!("{:.*}", decimals, v)
}

/// 92.123 -> "1:32.123"; "–" when unknown.
pub fn lap_time(t: Option<f32>) -> String {
    lap_time_d(t, 3)
}

pub fn lap_time_d(t: Option<f32>, decimals: usize) -> String {
    let Some(t) = t.filter(|t| *t > 0.0) else { return "–".into() };
    let t = t as f64;
    let m = (t / 60.0).floor();
    let s = t - m * 60.0;
    let sec = format!("{:0>width$}", fixed(s, decimals), width = decimals + 3);
    if m > 0.0 {
        format!("{m}:{sec}")
    } else {
        fixed(s, decimals)
    }
}

/// Signed seconds with a real minus sign: "+0.21", "−1.40".
pub fn signed(v: f64, decimals: usize) -> String {
    let s = fixed(v.abs(), decimals);
    if s.parse::<f64>().unwrap_or(0.0) == 0.0 {
        return fixed(0.0, decimals);
    }
    format!("{}{s}", if v < 0.0 { MINUS } else { "+" })
}

pub fn gap(v: Option<f32>, decimals: usize) -> String {
    v.map(|v| fixed(v.abs() as f64, decimals)).unwrap_or_default()
}

/// 1:02:03 / 12:34
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as i64;
    let (h, m, ss) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{ss:02}")
    } else {
        format!("{m}:{ss:02}")
    }
}

pub fn speed(ms: f32, units: Units) -> i64 {
    js_round(ms.abs() as f64 * if units == Units::Imperial { 2.23694 } else { 3.6 }) as i64
}

pub fn speed_unit(units: Units) -> &'static str {
    if units == Units::Imperial {
        "mph"
    } else {
        "km/h"
    }
}

pub fn fuel(litres: f32, units: Units) -> f32 {
    if units == Units::Imperial {
        litres * 0.264172
    } else {
        litres
    }
}

pub fn fuel_unit(units: Units) -> &'static str {
    if units == Units::Imperial {
        "gal"
    } else {
        "L"
    }
}

/// Tyre pressure from kPa: "165 kPa" or "23.9 psi".
pub fn pressure(kpa: f32, units: Units) -> String {
    if units == Units::Imperial {
        format!("{} psi", fixed(kpa as f64 * 0.145038, 1))
    } else {
        format!("{} kPa", js_round(kpa as f64))
    }
}

/// Temperature in the user's units, no unit: °C or °F.
pub fn temp(c: f32, units: Units) -> f32 {
    if units == Units::Imperial {
        c * 9.0 / 5.0 + 32.0
    } else {
        c
    }
}

pub fn temp_unit(units: Units) -> &'static str {
    if units == Units::Imperial {
        "°F"
    } else {
        "°C"
    }
}

pub fn gear(g: i32) -> String {
    match g {
        g if g < 0 => "R".into(),
        0 => "N".into(),
        g => g.to_string(),
    }
}

/// 2431 -> "2.4k"
pub fn irating(ir: i32) -> String {
    if ir == 0 {
        return String::new();
    }
    if ir >= 1000 {
        format!("{}k", fixed(ir as f64 / 1000.0, 1))
    } else {
        ir.to_string()
    }
}

/// "A 3.45" -> ("A", "3.45")
pub fn license(lic: &str) -> (&str, &str) {
    let mut it = lic.split(' ');
    (it.next().unwrap_or(""), it.next().unwrap_or(""))
}

/// `Math.round`.
pub fn js_round(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// Readable text colour on top of a hex background (true = dark text).
pub fn dark_text_on(hex: &str) -> bool {
    match crate::theme::parse_hex(hex) {
        Some(c) => 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32 > 150.0,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lap_times() {
        assert_eq!(lap_time(Some(92.123)), "1:32.123");
        assert_eq!(lap_time(Some(59.5)), "59.500");
        assert_eq!(lap_time(Some(61.05)), "1:01.050");
        assert_eq!(lap_time(None), "–");
        assert_eq!(lap_time(Some(0.0)), "–");
        assert_eq!(lap_time_d(Some(92.16), 1), "1:32.2");
    }

    #[test]
    fn signs_and_clock() {
        assert_eq!(signed(0.214, 2), "+0.21");
        assert_eq!(signed(-1.4, 2), "−1.40");
        assert_eq!(signed(-0.001, 2), "0.00");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(clock(754.9), "12:34");
        assert_eq!(clock(-5.0), "0:00");
        assert_eq!(gap(Some(-4.23), 1), "4.2");
        assert_eq!(gap(None, 1), "");
    }

    #[test]
    fn units() {
        assert_eq!(speed(50.0, Units::Metric), 180);
        assert_eq!(speed(50.0, Units::Imperial), 112);
        assert_eq!(pressure(165.0, Units::Metric), "165 kPa");
        assert_eq!(pressure(165.0, Units::Imperial), "23.9 psi");
        assert!((fuel(10.0, Units::Imperial) - 2.64172).abs() < 1e-4);
        assert_eq!(temp(30.0, Units::Imperial), 86.0);
        assert_eq!(irating(2431), "2.4k");
        assert_eq!(irating(950), "950");
        assert_eq!(irating(0), "");
        assert_eq!(license("A 3.45"), ("A", "3.45"));
        assert_eq!(gear(-1), "R");
        assert!(dark_text_on("#ffda59"));
        assert!(!dark_text_on("#0d131b"));
    }
}
