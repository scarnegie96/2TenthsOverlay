//! Estimated iRating change for each driver if the race finished as it
//! stands (`src/lib/irating.ts`): the community formula from the "iRacing
//! SOF iRating Calculator" spreadsheet, worked out per class.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    pub car_idx: i32,
    pub irating: i32,
    /// Finishing position in the class, from 1.
    pub rank: i32,
    /// False for drivers who registered but never took the start.
    pub started: bool,
}

const BR1: f64 = 1600.0 / std::f64::consts::LN_2;

/// The chance a driver rated `a` finishes ahead of one rated `b`.
fn chance(a: f64, b: f64) -> f64 {
    let ea = (-a / BR1).exp();
    let eb = (-b / BR1).exp();
    ((1.0 - ea) * eb) / ((1.0 - eb) * ea + (1.0 - ea) * eb)
}

/// car index -> estimated change (unrounded). Empty when there's no one to race.
pub fn changes(entries: &[Entry]) -> HashMap<i32, f64> {
    let mut out = HashMap::new();
    let registrations = entries.len() as f64;
    let starters = entries.iter().filter(|e| e.started).count() as f64;
    let non_starters = registrations - starters;
    if registrations < 2.0 || starters == 0.0 {
        return out;
    }
    let expected: Vec<f64> =
        entries.iter().map(|a| entries.iter().map(|b| chance(a.irating as f64, b.irating as f64)).sum::<f64>() - 0.5).collect();
    let mut sum_starters = 0.0;
    for (i, e) in entries.iter().enumerate() {
        if !e.started {
            continue;
        }
        let rank = e.rank as f64;
        let fudge = ((registrations - non_starters / 2.0) / 2.0 - rank) / 100.0;
        let change = ((registrations - rank - expected[i] - fudge) * 200.0) / starters;
        out.insert(e.car_idx, change);
        sum_starters += change;
    }
    if non_starters > 0.0 {
        let sum_expected_non: f64 = entries.iter().zip(&expected).filter(|(e, _)| !e.started).map(|(_, x)| x).sum();
        for (i, e) in entries.iter().enumerate() {
            if e.started {
                continue;
            }
            out.insert(e.car_idx, ((-sum_starters / non_starters) * expected[i]) / (sum_expected_non / non_starters));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// scripts/check-irating.ts: the `irating` crate's 28-driver race with one non-starter.
    #[test]
    fn check_irating() {
        let field: [(i32, i32, bool, f64); 28] = [
            (1, 7526, true, 17.63672),
            (2, 5982, true, 25.833923),
            (3, 5463, true, 25.432884),
            (4, 4279, true, 37.92791),
            (5, 4137, true, 33.394478),
            (6, 4044, true, 27.948332),
            (7, 3891, true, 23.814116),
            (8, 3612, true, 22.626814),
            (9, 3147, true, 26.485985),
            (10, 2823, true, 27.702335),
            (11, 2715, true, 23.36419),
            (12, 2603, true, 19.21653),
            (13, 2512, true, 14.53251),
            (14, 2352, false, 10.437519),
            (15, 2227, true, 8.5288105),
            (16, 2195, true, 2.2037997),
            (17, 2166, true, -4.2093577),
            (18, 2089, true, -9.06982),
            (19, 1773, true, -5.7882223),
            (20, 1772, true, -13.086736),
            (21, 1752, true, -19.722021),
            (22, 1748, true, -26.915356),
            (23, 1705, true, -32.73568),
            (24, 1662, true, -38.54108),
            (25, 1622, true, -44.439545),
            (26, 1537, true, -48.679874),
            (27, 1464, true, -53.308353),
            (28, 1203, true, -50.590836),
        ];
        let entries: Vec<Entry> = field.iter().map(|&(rank, irating, started, _)| Entry { car_idx: rank, irating, rank, started }).collect();
        let got = changes(&entries);
        for (rank, _, _, want) in field {
            let g = got[&rank];
            assert!((g - want).abs() <= 0.01, "P{rank}: got {g}, want {want}");
        }
        // Nobody to race: no estimate.
        assert!(changes(&[Entry { car_idx: 0, irating: 2000, rank: 1, started: true }]).is_empty());
    }
}
