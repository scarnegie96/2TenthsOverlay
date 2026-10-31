//! Pit service: the commands iRacing accepts from other programs, and what it
//! reports back as ticked for the next stop.
//!
//! Commands go out as a Windows broadcast message (`irsdk::send_pit_command`).
//! iRacing never answers, so the overlay shows what the `PitSv*` telemetry
//! says is ticked rather than what it asked for. The numbers come from the
//! SDK's irsdk_defines.h (irsdk_PitCommandMode, irsdk_PitSvFlags).

use serde::Deserialize;

/// irsdk_BroadcastMsg: pit commands are message 9.
pub const BROADCAST_PIT_COMMAND: u16 = 9;

/// `PitSvFlags` bits (irsdk_PitSvFlags).
pub mod flags {
    pub const LF_TIRE: u32 = 0x01;
    pub const RF_TIRE: u32 = 0x02;
    pub const LR_TIRE: u32 = 0x04;
    pub const RR_TIRE: u32 = 0x08;
    pub const FUEL: u32 = 0x10;
    pub const TEAROFF: u32 = 0x20;
    pub const FAST_REPAIR: u32 = 0x40;
    /// LF, RF, LR, RR
    pub const TIRES: [u32; 4] = [LF_TIRE, RF_TIRE, LR_TIRE, RR_TIRE];
}

/// One pit command, as the overlay sends it (`{ "kind": "fuel", "litres": 30 }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PitCommand {
    /// Untick everything.
    Clear,
    /// Add fuel, in whole litres. 0 keeps the amount already set in iRacing.
    Fuel { litres: u32 },
    ClearFuel,
    /// Change one tire (0 LF, 1 RF, 2 LR, 3 RR) at a pressure in kPa. 0 keeps the current pressure.
    Tire { corner: u8, kpa: u32 },
    ClearTires,
    /// Use a windscreen tear-off.
    Tearoff,
    ClearTearoff,
    FastRepair,
    ClearFastRepair,
    /// Fit this compound, by its number in the session's list of compounds.
    Compound { index: u32 },
}

impl PitCommand {
    /// irsdk_PitCommandMode and its parameter.
    pub fn encode(self) -> (u16, u32) {
        match self {
            Self::Clear => (0, 0),
            Self::Tearoff => (1, 0),
            Self::Fuel { litres } => (2, litres),
            Self::Tire { corner, kpa } => (3 + corner.min(3) as u16, kpa),
            Self::ClearTires => (7, 0),
            Self::FastRepair => (8, 0),
            Self::ClearTearoff => (9, 0),
            Self::ClearFastRepair => (10, 0),
            Self::ClearFuel => (11, 0),
            Self::Compound { index } => (12, index),
        }
    }
}

/// What's ticked for the next stop: the `PitSv*` variables.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PitService {
    /// `PitSvFlags`
    pub flags: u32,
    /// Litres to add.
    pub fuel: f32,
    /// kPa each tire will be set to, LF, RF, LR, RR.
    pub pressure: [f32; 4],
    /// Compound to fit, by its number in the session's list.
    pub compound: i32,
}

impl PitService {
    /// Does what iRacing does with a command. Used by the demo race.
    pub fn apply(&mut self, cmd: PitCommand) {
        match cmd {
            PitCommand::Clear => self.flags = 0,
            PitCommand::Fuel { litres } => {
                self.flags |= flags::FUEL;
                if litres > 0 {
                    self.fuel = litres as f32;
                }
            }
            PitCommand::ClearFuel => self.flags &= !flags::FUEL,
            PitCommand::Tire { corner, kpa } => {
                let k = corner.min(3) as usize;
                self.flags |= flags::TIRES[k];
                if kpa > 0 {
                    self.pressure[k] = kpa as f32;
                }
            }
            PitCommand::ClearTires => self.flags &= !flags::TIRES.iter().fold(0, |a, b| a | b),
            PitCommand::Tearoff => self.flags |= flags::TEAROFF,
            PitCommand::ClearTearoff => self.flags &= !flags::TEAROFF,
            PitCommand::FastRepair => self.flags |= flags::FAST_REPAIR,
            PitCommand::ClearFastRepair => self.flags &= !flags::FAST_REPAIR,
            PitCommand::Compound { index } => self.compound = index as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_encode_as_the_sdk_numbers_them() {
        assert_eq!(PitCommand::Clear.encode(), (0, 0));
        assert_eq!(PitCommand::Tearoff.encode(), (1, 0));
        assert_eq!(PitCommand::Fuel { litres: 33 }.encode(), (2, 33));
        assert_eq!(PitCommand::Tire { corner: 0, kpa: 165 }.encode(), (3, 165));
        assert_eq!(PitCommand::Tire { corner: 3, kpa: 0 }.encode(), (6, 0));
        assert_eq!(PitCommand::ClearTires.encode(), (7, 0));
        assert_eq!(PitCommand::FastRepair.encode(), (8, 0));
        assert_eq!(PitCommand::ClearTearoff.encode(), (9, 0));
        assert_eq!(PitCommand::ClearFastRepair.encode(), (10, 0));
        assert_eq!(PitCommand::ClearFuel.encode(), (11, 0));
        assert_eq!(PitCommand::Compound { index: 1 }.encode(), (12, 1));
    }

    #[test]
    fn commands_read_from_the_overlay_json() {
        let cmd: PitCommand = serde_json::from_str(r#"{"kind":"fuel","litres":30}"#).unwrap();
        assert_eq!(cmd, PitCommand::Fuel { litres: 30 });
        let cmd: PitCommand = serde_json::from_str(r#"{"kind":"clearFastRepair"}"#).unwrap();
        assert_eq!(cmd, PitCommand::ClearFastRepair);
        let cmd: PitCommand = serde_json::from_str(r#"{"kind":"tire","corner":2,"kpa":0}"#).unwrap();
        assert_eq!(cmd, PitCommand::Tire { corner: 2, kpa: 0 });
    }

    #[test]
    fn service_ticks_and_unticks() {
        let mut sv = PitService { fuel: 12.0, pressure: [165.0, 165.0, 162.0, 162.0], ..Default::default() };
        sv.apply(PitCommand::Fuel { litres: 0 });
        assert_eq!((sv.flags, sv.fuel), (flags::FUEL, 12.0), "0 keeps the amount");
        sv.apply(PitCommand::Fuel { litres: 30 });
        assert_eq!(sv.fuel, 30.0);
        sv.apply(PitCommand::Tire { corner: 1, kpa: 0 });
        sv.apply(PitCommand::Tire { corner: 2, kpa: 170 });
        assert_eq!(sv.flags, flags::FUEL | flags::RF_TIRE | flags::LR_TIRE);
        assert_eq!(sv.pressure, [165.0, 165.0, 170.0, 162.0]);
        sv.apply(PitCommand::Tearoff);
        sv.apply(PitCommand::ClearTires);
        assert_eq!(sv.flags, flags::FUEL | flags::TEAROFF);
        sv.apply(PitCommand::Clear);
        assert_eq!(sv.flags, 0);
        assert_eq!(sv.fuel, 30.0, "clearing unticks but keeps the amount");
    }
}
