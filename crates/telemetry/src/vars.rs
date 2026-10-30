//! Telemetry variable headers and value decoding, shared by the live
//! shared-memory reader (`irsdk.rs`) and the `.ibt` file reader (`ibt.rs`).
//! Both use the same layout, so this is plain byte parsing with no platform code.

use std::collections::HashMap;

pub const VAR_HEADER_SIZE: usize = 144;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Char,
    Bool,
    Int,
    BitField,
    Float,
    Double,
}

impl VarType {
    pub fn from_raw(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Char,
            1 => Self::Bool,
            2 => Self::Int,
            3 => Self::BitField,
            4 => Self::Float,
            5 => Self::Double,
            _ => return None,
        })
    }

    pub fn to_raw(self) -> i32 {
        match self {
            Self::Char => 0,
            Self::Bool => 1,
            Self::Int => 2,
            Self::BitField => 3,
            Self::Float => 4,
            Self::Double => 5,
        }
    }

    pub fn size(self) -> usize {
        match self {
            Self::Char | Self::Bool => 1,
            Self::Int | Self::BitField | Self::Float => 4,
            Self::Double => 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VarHeader {
    pub name: String,
    pub var_type: VarType,
    pub offset: usize,
    pub count: usize,
    pub desc: String,
    pub unit: String,
}

/// Parses `count` consecutive 144-byte variable headers.
pub fn parse_headers(bytes: &[u8], count: usize) -> HashMap<String, VarHeader> {
    let mut vars = HashMap::with_capacity(count);
    for raw in bytes.chunks_exact(VAR_HEADER_SIZE).take(count) {
        let ty = i32::from_le_bytes(raw[0..4].try_into().unwrap());
        let offset = i32::from_le_bytes(raw[4..8].try_into().unwrap());
        let n = i32::from_le_bytes(raw[8..12].try_into().unwrap());
        let Some(var_type) = VarType::from_raw(ty) else { continue };
        let name = cstr(&raw[16..48]);
        vars.insert(
            name.clone(),
            VarHeader {
                name,
                var_type,
                offset: offset.max(0) as usize,
                count: n.max(1) as usize,
                desc: cstr(&raw[48..112]),
                unit: cstr(&raw[112..144]),
            },
        );
    }
    vars
}

/// Reads element `idx` of a variable from one telemetry row, as f64 whatever its type.
pub fn read_num(row: &[u8], v: &VarHeader, idx: usize) -> Option<f64> {
    if idx >= v.count {
        return None;
    }
    let at = v.offset + idx * v.var_type.size();
    let b = row.get(at..at + v.var_type.size())?;
    Some(match v.var_type {
        VarType::Char | VarType::Bool => b[0] as f64,
        VarType::Int => i32::from_le_bytes(b.try_into().ok()?) as f64,
        VarType::BitField => u32::from_le_bytes(b.try_into().ok()?) as f64,
        VarType::Float => f32::from_le_bytes(b.try_into().ok()?) as f64,
        VarType::Double => f64::from_le_bytes(b.try_into().ok()?),
    })
}

pub fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub fn le_i32(bytes: &[u8], at: usize) -> Option<i32> {
    bytes.get(at..at + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap()))
}
