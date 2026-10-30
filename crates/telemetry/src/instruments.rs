//! Dashboard data for the Instruments widget: warning lights, engine gauges
//! and brake bias.

use serde::Serialize;

use crate::model::RawFrame;

/// `EngineWarnings` bits (irsdk_EngineWarnings).
pub mod engine_warning {
    pub const WATER_TEMP: u32 = 0x0001;
    pub const FUEL_PRESSURE: u32 = 0x0002;
    pub const OIL_PRESSURE: u32 = 0x0004;
    pub const STALLED: u32 = 0x0008;
    pub const PIT_LIMITER: u32 = 0x0010;
    pub const REV_LIMITER: u32 = 0x0020;
    /// Newer SDK versions.
    pub const OIL_TEMP: u32 = 0x0040;
    pub const REPAIR_REQUIRED: u32 = 0x0080;
    pub const REPAIR_OPTIONAL: u32 = 0x0100;
}

/// Under this many laps of fuel left, the low fuel light comes on.
pub const LOW_FUEL_LAPS: f32 = 2.0;
/// Fallback when there's no per-lap figure yet: fraction of the tank.
pub const LOW_FUEL_PCT: f32 = 0.08;

#[derive(Debug, Clone, Copy, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Warnings {
    pub water_temp: bool,
    pub fuel_pressure: bool,
    pub oil_pressure: bool,
    pub oil_temp: bool,
    pub stalled: bool,
    pub pit_limiter: bool,
    pub rev_limiter: bool,
    pub repair_required: bool,
    pub repair_optional: bool,
    /// Worked out by us, not an iRacing flag.
    pub low_fuel: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentsPayload {
    pub warnings: Warnings,
    /// °C
    pub oil_temp: Option<f32>,
    /// bar
    pub oil_press: Option<f32>,
    /// °C
    pub water_temp: Option<f32>,
    /// bar
    pub fuel_press: Option<f32>,
    /// volts
    pub voltage: Option<f32>,
    /// Percent to the front, when the car has adjustable bias.
    pub brake_bias: Option<f32>,
    /// Extra bias controls some cars have (fine adjust, peak bias).
    pub brake_bias_fine: Option<f32>,
    pub peak_brake_bias: Option<f32>,
}

/// Some cars report bias as 0..1, most as a percentage.
fn as_percent(v: f32) -> f32 {
    if v > 0.0 && v <= 1.0 { v * 100.0 } else { v }
}

/// Gauges read 0 when a car doesn't model them; treat that as missing.
fn present(v: Option<f32>) -> Option<f32> {
    v.filter(|x| x.is_finite() && *x != 0.0)
}

pub fn build(f: &RawFrame, fuel_per_lap: Option<f32>) -> InstrumentsPayload {
    use engine_warning as w;
    let bits = f.engine_warnings;
    let low_fuel = f.is_on_track
        && match fuel_per_lap.filter(|p| *p > 0.0) {
            Some(per_lap) => f.fuel_level / per_lap < LOW_FUEL_LAPS,
            None => f.fuel_level_pct > 0.0 && f.fuel_level_pct < LOW_FUEL_PCT,
        };
    InstrumentsPayload {
        warnings: Warnings {
            water_temp: bits & w::WATER_TEMP != 0,
            fuel_pressure: bits & w::FUEL_PRESSURE != 0,
            oil_pressure: bits & w::OIL_PRESSURE != 0,
            oil_temp: bits & w::OIL_TEMP != 0,
            stalled: bits & w::STALLED != 0,
            pit_limiter: bits & w::PIT_LIMITER != 0,
            rev_limiter: bits & w::REV_LIMITER != 0,
            repair_required: bits & w::REPAIR_REQUIRED != 0,
            repair_optional: bits & w::REPAIR_OPTIONAL != 0,
            low_fuel,
        },
        oil_temp: present(f.oil_temp),
        oil_press: present(f.oil_press),
        water_temp: present(f.water_temp),
        fuel_press: present(f.fuel_press),
        voltage: present(f.voltage),
        brake_bias: f.brake_bias.map(as_percent),
        brake_bias_fine: f.brake_bias_fine,
        peak_brake_bias: f.peak_brake_bias.map(as_percent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_warning_bits() {
        let mut f = RawFrame::with_capacity();
        f.engine_warnings = engine_warning::OIL_PRESSURE | engine_warning::PIT_LIMITER | engine_warning::REPAIR_REQUIRED;
        let i = build(&f, None);
        assert!(i.warnings.oil_pressure && i.warnings.pit_limiter && i.warnings.repair_required);
        assert!(!i.warnings.water_temp && !i.warnings.rev_limiter && !i.warnings.low_fuel);
    }

    #[test]
    fn low_fuel_from_laps_or_tank() {
        let mut f = RawFrame::with_capacity();
        f.is_on_track = true;
        f.fuel_level = 5.0;
        f.fuel_level_pct = 0.5;
        assert!(build(&f, Some(3.0)).warnings.low_fuel); // 1.7 laps left
        assert!(!build(&f, Some(2.0)).warnings.low_fuel); // 2.5 laps left
        f.fuel_level_pct = 0.05;
        assert!(build(&f, None).warnings.low_fuel); // no per-lap figure yet: tank under 8%
    }

    #[test]
    fn brake_bias_and_missing_gauges() {
        let mut f = RawFrame::with_capacity();
        f.brake_bias = Some(0.545);
        f.oil_temp = Some(0.0);
        f.water_temp = Some(91.0);
        let i = build(&f, None);
        assert!((i.brake_bias.unwrap() - 54.5).abs() < 1e-3);
        assert_eq!(i.oil_temp, None);
        assert_eq!(i.water_temp, Some(91.0));
    }
}
