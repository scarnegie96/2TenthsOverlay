//! Fuel (`src/widgets/Fuel.tsx`): what's in the tank, the average burn, and
//! how much to add to finish (`fuelToFinish` in `src/lib/pit.ts`).

use egui::{Align2, Painter, Pos2, Rect, Stroke, Vec2};
use telemetry::model::FuelPayload;

use super::{Ctx, Widget};
use crate::kit::{self, seg};
use crate::logic::format::{fixed, fuel, fuel_unit};
use crate::theme::alpha;

pub struct Fuel;

/// Litres needed to finish beyond what's in the tank, with the safety margin
/// (negative = spare). None until a green lap has been measured.
pub fn fuel_to_finish(f: Option<&FuelPayload>, margin_laps: f32) -> Option<f32> {
    let f = f?;
    Some(f.to_finish? + margin_laps * f.per_lap?)
}

impl Widget for Fuel {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let units = cx.units;
        let f = cx.live.field.as_ref().map(|f| &f.fuel);
        let u = fuel_unit(units);
        let v = |l: Option<f32>, d: usize| l.map(|l| fixed(fuel(l, units) as f64, d)).unwrap_or("–".into());

        let level = f.map(|f| f.level).unwrap_or(0.0);
        let tank_pct = f.filter(|f| f.max > 0.0).map(|f| (level / f.max).min(1.0)).unwrap_or(0.0);
        let need = fuel_to_finish(f, cx.o.n("marginLaps") as f32);
        let enough = need.is_some_and(|n| n <= 0.0);

        let aside_text = match f {
            Some(f) if f.samples > 0 => format!("{} lap avg", f.samples),
            _ => "Measuring".into(),
        };
        let panel = kit::Panel::begin(p, th, 280.0, Some("Fuel"), &[seg(aside_text, kit::aside_font(th), th.ink2)]);
        let x = panel.x();
        let top = panel.body_top;

        // The level and the tank bar.
        let y0 = top + 8.0;
        let base = y0 + 27.0;
        let r = kit::segs_baseline(p, x.min + 10.0, base, &[seg(v(Some(level), 1), th.bold(30.0), th.ink)]);
        let r2 = kit::segs_baseline(p, r.max + 3.0, base, &[seg(u, th.medium(14.0), th.ink3)]);
        let tank = Rect::from_min_max(Pos2::new(r2.max + 12.0, y0 + 11.0), Pos2::new(x.max - 10.0, y0 + 19.0));
        kit::fill(p, tank, 4.0, alpha(th.ink, 0.08));
        if tank_pct > 0.0 {
            // A scaled bar clipped by the track: round on the left, square on the right until full.
            let w = tank.width() * tank_pct;
            let right = if tank_pct > 0.97 { 4 } else { 0 };
            let cr = egui::CornerRadius { nw: 4, sw: 4, ne: right, se: right };
            p.rect_filled(Rect::from_min_size(tank.min, Vec2::new(w, tank.height())), cr, th.ink2);
        }

        // Per lap, laps in tank, laps to go.
        let mut y = y0 + 30.0 + 4.0 + 4.0;
        let laps = |l: Option<f32>| l.map(|l| fixed(l as f64, 1)).unwrap_or("–".into());
        let rows = [
            ("Per lap", v(f.and_then(|f| f.per_lap), 2)),
            ("Laps in tank", laps(f.and_then(|f| f.laps_of_fuel))),
            ("Laps to go", laps(f.and_then(|f| f.laps_to_go))),
        ];
        // Least and most used on a clean lap this session, when there are two to tell apart.
        let mut rows = rows.to_vec();
        if let (Some(lo), Some(hi)) = (f.and_then(|f| f.min_per_lap), f.and_then(|f| f.max_per_lap)) {
            rows.insert(1, ("Min / max lap", format!("{} / {}", v(Some(lo), 2), v(Some(hi), 2))));
        }
        for (label, value) in rows {
            let cy = y + 9.0;
            kit::text(p, Pos2::new(x.min + 10.0, cy), Align2::LEFT_CENTER, label, th.medium(15.0), th.ink2);
            kit::text(p, Pos2::new(x.max - 10.0, cy), Align2::RIGHT_CENTER, &value, th.semi(15.0), th.ink);
            y += 18.0 + 2.0;
        }
        y = y - 2.0 + 8.0;

        // The call: add, or enough.
        p.hline(x, y + 0.5, Stroke::new(1.0, th.line));
        y += 1.0;
        let (msg, color) = match need {
            None => ("Complete a green lap to start the calculation".to_string(), th.ink3),
            Some(n) if enough => (format!("Enough to finish, {} {u} spare", v(Some(-n), 1)), th.gain),
            Some(n) => (format!("Add {} {u} to finish", v(Some(n), 1)), th.attention),
        };
        let r = kit::text_wrap(p, Pos2::new(x.min + 10.0, y + 6.0), &msg, th.semi(15.0), color, x.span() - 20.0);
        panel.finish(p, th, r.bottom() + 6.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(to_finish: Option<f32>, per_lap: Option<f32>) -> FuelPayload {
        FuelPayload { level: 30.0, max: 60.0, per_lap, last_lap_used: None, min_per_lap: None, max_per_lap: None, laps_of_fuel: None, laps_to_go: None, to_finish, samples: 3 }
    }

    #[test]
    fn to_finish_adds_the_margin() {
        assert_eq!(fuel_to_finish(Some(&f(Some(10.0), Some(2.0))), 0.5), Some(11.0));
        assert_eq!(fuel_to_finish(Some(&f(Some(-4.0), Some(2.0))), 0.5), Some(-3.0));
    }

    #[test]
    fn unknown_until_measured() {
        assert_eq!(fuel_to_finish(None, 0.5), None);
        assert_eq!(fuel_to_finish(Some(&f(None, Some(2.0))), 0.5), None);
        assert_eq!(fuel_to_finish(Some(&f(Some(1.0), None)), 0.5), None);
    }
}
