//! Where you'd rejoin if you pitted now (`src/lib/rejoin.ts`): your gap to
//! the class leader plus the session's average stop loss for your class,
//! against everyone else's gap.

use telemetry::model::{CarStatus, StandingRow};

#[derive(Debug, Clone, Copy)]
pub struct Near<'a> {
    pub row: &'a StandingRow,
    pub by: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Rejoin<'a> {
    /// Class position you'd come out in.
    pub position: usize,
    /// The car you'd come out behind, and by how much.
    pub behind: Option<Near<'a>>,
    pub ahead: Option<Near<'a>>,
    pub loss: f32,
}

fn gap(r: &StandingRow) -> Option<f32> {
    if r.class_position == 1 {
        Some(0.0)
    } else {
        r.gap
    }
}

/// `rows` are your class in race order, you included.
pub fn after_stop<'a>(rows: &[&'a StandingRow], loss: f32) -> Option<Rejoin<'a>> {
    let me = rows.iter().find(|r| r.is_player)?;
    if me.status == CarStatus::Out {
        return None;
    }
    let at = gap(me)? + loss;
    let mut behind: Option<Near> = None;
    let mut ahead: Option<Near> = None;
    let mut before = 0;
    for &r in rows {
        let Some(g) = gap(r) else { continue };
        if r.is_player || r.status == CarStatus::Out {
            continue;
        }
        if g <= at {
            before += 1;
            if behind.is_none_or(|b| g > gap(b.row).unwrap_or(0.0)) {
                behind = Some(Near { row: r, by: at - g });
            }
        } else if ahead.is_none_or(|a| g < gap(a.row).unwrap_or(0.0)) {
            ahead = Some(Near { row: r, by: g - at });
        }
    }
    Some(Rejoin { position: before + 1, behind, ahead, loss })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use telemetry::model::{DriverCell, LapInfo};

    pub fn row(car: i32, class_pos: i32, gap: Option<f32>, player: bool) -> StandingRow {
        StandingRow {
            driver: DriverCell {
                car_idx: car,
                name: format!("Car {car}"),
                abbrev: String::new(),
                number: car.to_string(),
                class_id: 1,
                class_name: "GT3".into(),
                class_color: "#ffda59".into(),
                irating: 2000,
                user_id: car as i64,
                license: "A 3.00".into(),
                license_color: "#0153db".into(),
            },
            position: class_pos,
            class_position: class_pos,
            lap: 10,
            lap_pct: 0.5,
            gap,
            laps_down: 0,
            interval: None,
            start_position: None,
            laps: LapInfo::default(),
            on_pit_road: false,
            status: CarStatus::Track,
            is_player: player,
        }
    }

    #[test]
    fn rejoins_between_the_right_cars() {
        let rows = vec![
            row(1, 1, Some(0.0), false),
            row(2, 2, Some(3.0), true),
            row(3, 3, Some(10.0), false),
            row(4, 4, Some(40.0), false),
        ];
        let r = after_stop(&rows.iter().collect::<Vec<_>>(), 25.0).unwrap();
        // 3 + 25 = 28: behind car 3 by 18, ahead of car 4 by 12, P3.
        assert_eq!(r.position, 3);
        assert_eq!(r.behind.unwrap().row.driver.car_idx, 3);
        assert!((r.behind.unwrap().by - 18.0).abs() < 1e-4);
        assert_eq!(r.ahead.unwrap().row.driver.car_idx, 4);
        assert!((r.ahead.unwrap().by - 12.0).abs() < 1e-4);
    }

    #[test]
    fn out_cars_and_unknown_gaps_dont_count() {
        let mut rows = vec![row(1, 1, Some(0.0), true), row(2, 2, Some(5.0), false), row(3, 3, None, false)];
        rows[1].status = CarStatus::Out;
        let r = after_stop(&rows.iter().collect::<Vec<_>>(), 20.0).unwrap();
        assert_eq!(r.position, 1);
        assert!(r.behind.is_none() && r.ahead.is_none());
        rows[0].status = CarStatus::Out;
        assert!(after_stop(&rows.iter().collect::<Vec<_>>(), 20.0).is_none());
    }
}
