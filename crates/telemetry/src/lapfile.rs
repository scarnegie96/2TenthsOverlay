//! Opens any supported lap file by extension: iRacing telemetry (.ibt) or
//! iRacing's saved best/optimal laps (.blap/.olap).

use std::path::Path;

use crate::blap::BlapFile;
use crate::ibt::{IbtFile, IbtSummary};
use crate::reference::ReferenceLap;

fn ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Lists the laps a file offers (a .blap/.olap holds exactly one).
pub fn summary(path: &Path) -> Result<IbtSummary, String> {
    match ext(path).as_str() {
        "blap" | "olap" => Ok(BlapFile::open(path)?.summary()),
        "ibt" => Ok(IbtFile::open(path)?.summary()),
        other => Err(format!("Tenths can't read .{other} files. Choose an .ibt, .blap or .olap file.")),
    }
}

/// Builds a reference lap from a file. `lap` is ignored for .blap/.olap.
pub fn reference(path: &Path, lap: i32) -> Result<ReferenceLap, String> {
    match ext(path).as_str() {
        "blap" | "olap" => BlapFile::open(path)?.reference(&file_name(path)),
        _ => IbtFile::open(path)?.reference(lap, &file_name(path)),
    }
}
