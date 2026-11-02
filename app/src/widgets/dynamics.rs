//! Car dynamics (`src/widgets/Dynamics.tsx`): a g-meter with the grip you've
//! used recently as an outline that withdraws to the centre, the car seen
//! from above (slip), the side (pitch) and behind (roll), and an
//! understeer / oversteer balance bar.

use std::collections::VecDeque;

use egui::epaint::Mesh;
use egui::{Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use super::{Ctx, Widget};
use crate::kit::{self, seg};
use crate::logic::format::fixed;
use crate::svg::{self, Path, Xf};
use crate::theme::{alpha, Theme};

const METER: f32 = 176.0;
const TRAIL_S: f64 = 1.2;
const BUCKETS: usize = 48;

/// How far a point of the grip outline reaches at `now`: its peak for
/// `hold` seconds, then easing back to the centre over `ret` seconds.
pub fn envelope_reach(peak: f32, set_at: f64, now: f64, hold: f64, ret: f64) -> f32 {
    let age = now - set_at;
    if age <= hold {
        return peak;
    }
    if ret <= 0.0 {
        return 0.0;
    }
    let f = ((age - hold) / ret).min(1.0) as f32;
    peak * (1.0 - f * f * (3.0 - 2.0 * f))
}

struct Cars {
    top_wheels: Vec<Path>,
    top_body: Path,
    top_trim: Path,
    top_detail: Path,
    top_cabin: Path,
    top_wing: Path,
    side_body: Path,
    side_detail: Path,
    side_trim: Path,
    side_wing: Path,
    rear_wheels: Vec<Path>,
    rear_body: Path,
    rear_detail: Path,
    rear_plate: Path,
    rear_trim: Path,
    rear_lights: Vec<Path>,
    rear_wing: Vec<Path>,
}

impl Cars {
    fn new() -> Self {
        Self {
            top_wheels: vec![svg::rect(-14.5, -20.0, 5.0, 10.0, 1.5), svg::rect(9.5, -20.0, 5.0, 10.0, 1.5), svg::rect(-15.5, 11.0, 6.0, 11.0, 1.5), svg::rect(9.5, 11.0, 6.0, 11.0, 1.5)],
            top_body: Path::parse(
                "M 0 -32 C 6 -32 10 -31 11.5 -28 L 12.5 -21 C 13 -17 13 -13 12.5 -9 C 12 -6 11.5 -3 11.5 0 C 11.5 4 13.5 7 13.5 11 \
                 L 13.5 22 C 13.5 25.5 12 27.5 9 27.5 L -9 27.5 C -12 27.5 -13.5 25.5 -13.5 22 L -13.5 11 C -13.5 7 -11.5 4 -11.5 0 \
                 C -11.5 -3 -12 -6 -12.5 -9 C -13 -13 -13 -17 -12.5 -21 L -11.5 -28 C -10 -31 -6 -32 0 -32 Z",
            ),
            top_trim: Path::parse("M -8 -29.5 Q 0 -31.5 8 -29.5 M -4.5 -24 L -4.5 -17 M 4.5 -24 L 4.5 -17"),
            top_detail: Path::parse("M -7 -10 C -5 -12 5 -12 7 -10 L 8 7 C 6 9 -6 9 -8 7 Z"),
            top_cabin: svg::rect(-6.0, -4.0, 12.0, 8.0, 2.0),
            top_wing: svg::rect(-14.5, 23.5, 29.0, 5.0, 1.0),
            side_body: Path::parse(
                "M -33 8.5 L -34 2 C -34 -1 -33 -3 -31 -4 L -22 -5 C -16 -6 -12 -12 -6 -14 C -2 -15.5 4 -15.5 7 -14 \
                 C 11 -12 14 -8 18 -6.5 C 24 -5 30 -3 33 0 L 35 6 L 36 8.5 L 29 8.5 A 8 8 0 0 0 13 8.5 \
                 L -12 8.5 A 8 8 0 0 0 -28 8.5 Z",
            ),
            side_detail: Path::parse("M -13 -7 C -10 -10 -7 -12 -3 -12.5 L 5 -12.5 C 8 -11.5 11 -9 13 -7 Z"),
            side_trim: Path::parse("M -6 6 L 9 6"),
            side_wing: Path::parse("M -36 -12 L -24 -12.5 L -24 -10 L -35 -9.5 Z M -31 -9.5 L -29.5 -9.5 L -29.5 -4.5 L -31 -4.5 Z"),
            rear_wheels: vec![svg::rect(-29.0, 0.0, 10.0, 14.0, 2.5), svg::rect(19.0, 0.0, 10.0, 14.0, 2.5)],
            rear_body: Path::parse(
                "M -27.5 4 L -27.5 0 C -27.5 -3 -25.5 -5 -22 -5.5 L -14 -6.5 C -12 -12 -10 -16 -6 -16.5 L 6 -16.5 \
                 C 10 -16 12 -12 14 -6.5 L 22 -5.5 C 25.5 -5 27.5 -3 27.5 0 L 27.5 4 L 19 4 L 17 10 L -17 10 L -19 4 Z",
            ),
            rear_detail: Path::parse("M -8.5 -14.5 L 8.5 -14.5 L 11 -8.5 L -11 -8.5 Z"),
            rear_plate: svg::rect(-15.0, 4.0, 30.0, 5.0, 1.0),
            rear_trim: Path::parse("M -7.5 4.5 L -7.5 9 M 0 4.5 L 0 9 M 7.5 4.5 L 7.5 9"),
            rear_lights: vec![svg::rect(-24.5, -3.5, 8.0, 2.0, 1.0), svg::rect(16.5, -3.5, 8.0, 2.0, 1.0)],
            rear_wing: vec![
                Path::parse("M -8.5 -17 L -7 -17 L -7 -9 L -8.5 -9 Z M 7 -17 L 8.5 -17 L 8.5 -9 L 7 -9 Z"),
                svg::rect(-25.0, -20.0, 50.0, 3.0, 1.0),
                svg::rect(-26.5, -22.0, 2.5, 8.0, 0.8),
                svg::rect(24.0, -22.0, 2.5, 8.0, 0.8),
            ],
        }
    }
}

pub struct Dynamics {
    /// Recent g (felt direction: x = lateral, y = longitudinal), for the trail.
    trail: VecDeque<(f64, f32, f32)>,
    peak: [f32; BUCKETS],
    set_at: [f64; BUCKETS],
    clock: f64,
    last_time: f64,
    hold: f64,
    ret: f64,
    /// The last balance reading, held through straights.
    last_balance: Option<f32>,
    cars: Cars,
}

impl Default for Dynamics {
    fn default() -> Self {
        Self {
            trail: VecDeque::new(),
            peak: [0.0; BUCKETS],
            set_at: [0.0; BUCKETS],
            clock: 0.0,
            last_time: f64::NAN,
            hold: 10.0,
            ret: 10.0,
            last_balance: None,
            cars: Cars::new(),
        }
    }
}

impl Widget for Dynamics {
    fn observe(&mut self, live: &crate::data::Live, _now: f64) {
        let Some(t) = live.tick.as_deref() else { return };
        if t.session_time == self.last_time {
            return;
        }
        self.last_time = t.session_time;
        let d = &t.dynamics;
        if let Some(b) = d.balance {
            self.last_balance = Some(b);
        }
        let (x, y) = (d.lat_g, d.long_g);
        let now = t.session_time;
        self.trail.push_back((now, x, y));
        while self.trail.front().is_some_and(|p| p.0 < now - TRAIL_S || p.0 > now) {
            self.trail.pop_front();
        }
        // Time went backwards (new session, replay): start the outline afresh.
        if now < self.clock {
            self.peak = [0.0; BUCKETS];
            self.set_at = [0.0; BUCKETS];
        }
        self.clock = now;
        let mag = x.hypot(y);
        let a = y.atan2(x);
        let b = (((a / std::f32::consts::TAU * BUCKETS as f32).round() as i64).rem_euclid(BUCKETS as i64)) as usize;
        if mag >= envelope_reach(self.peak[b], self.set_at[b], now, self.hold, self.ret) {
            self.peak[b] = mag;
            self.set_at[b] = now;
        }
    }

    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        self.hold = o.n("envelopeHold");
        self.ret = o.n("envelopeReturn");
        let max_g = o.n("maxG").max(1.0).round() as i32;
        let felt = o.s("direction") == "felt";
        let exaggerate = o.n("exaggerate") as f32;
        let (show_att, show_bal) = (o.b("showAttitude"), o.b("showBalance"));
        let d = cx.live.tick.as_ref().map(|t| t.dynamics).unwrap_or_default();
        let meter_only = !show_att && !show_bal;
        let total = d.lat_g.hypot(d.long_g);
        let w = if meter_only { 2.0 + 24.0 + METER } else { 470.0 };
        let aside = [seg(format!("{} g", fixed(total as f64, 2)), kit::aside_font(th), th.ink2)];
        let panel = kit::Panel::begin(p, th, w, Some(if meter_only { "G-meter" } else { "Car dynamics" }), &aside);
        let left = panel.inset + 12.0;
        let top = panel.body_top + 10.0;

        let m = Rect::from_min_size(Pos2::new(left, top), Vec2::splat(METER));
        self.gmeter(p, th, m, max_g, felt);
        // Readouts.
        let ry = m.bottom() + 6.0;
        let cw = (METER - 8.0) / 3.0;
        let long_label = if d.long_g < 0.0 { "Braking" } else { "Accel" };
        for (i, (label, v)) in [("Lateral", d.lat_g.abs()), (long_label, d.long_g.abs()), ("Vertical", if cx.live.tick.is_some() { d.vert_g } else { 1.0 })].into_iter().enumerate() {
            let c = left + i as f32 * (cw + 4.0) + cw / 2.0;
            kit::text(p, Pos2::new(c, ry), Align2::CENTER_TOP, label, th.medium(11.0), th.ink3);
            let vs = [seg(fixed(v as f64, 2), th.bold(17.0), th.ink), seg(" g", th.medium(11.0), th.ink3)];
            let vw = kit::segs_width(p, &vs, 0.0);
            kit::segs_baseline(p, c - vw / 2.0, ry + 13.0 + 15.0, &vs);
        }
        let body_bottom = ry + 13.0 + 19.0 + 12.0;

        if !meter_only {
            let side = Rect::from_min_max(Pos2::new(m.right() + 14.0, top), Pos2::new(w - panel.inset - 12.0, body_bottom - 12.0));
            if show_att {
                let vw = (side.width() - 12.0) / 3.0;
                let views = [("Slip", d.slip_deg), ("Pitch", d.pitch_deg), ("Roll", d.roll_deg)];
                for (i, (label, v)) in views.into_iter().enumerate() {
                    let r = Rect::from_min_size(Pos2::new(side.left() + i as f32 * (vw + 6.0), side.top()), Vec2::new(vw, 4.0 + 84.0 + 2.0 + 15.0 + 5.0));
                    kit::fill(p, r, 5.0, alpha(th.well, 0.20));
                    let c = Pos2::new(r.center().x, r.top() + 4.0 + 42.0);
                    match i {
                        0 => self.top_view(p, th, c, d.slip_deg),
                        1 => self.side_view(p, th, c, d.pitch_deg * exaggerate),
                        _ => self.rear_view(p, th, c, d.roll_deg * exaggerate),
                    }
                    let vs = [
                        seg(format!("{label} "), th.medium(12.0), th.ink3),
                        seg(format!("{}{}°", if v >= 0.0 { "" } else { "−" }, fixed(v.abs() as f64, 1)), th.bold(12.0), th.ink),
                    ];
                    let tw = kit::segs_width(p, &vs, 0.0);
                    kit::segs(p, r.center().x - tw / 2.0, r.bottom() - 5.0 - 7.5, false, &vs, 0.0);
                }
            }
            if show_bal {
                self.balance(p, th, side, d.balance);
            }
        }
        panel.finish(p, th, body_bottom)
    }
}

impl Dynamics {
    fn gmeter(&self, p: &Painter, th: &Theme, m: Rect, max_g: i32, felt: bool) {
        let c = m.center();
        let r = METER / 2.0 - 8.0;
        let scale = r / max_g as f32;
        let line = Stroke::new(1.0, th.line_strong);
        for g in 1..=max_g {
            if g == max_g {
                p.circle_stroke(c, g as f32 * scale, line);
            } else {
                let n = 96;
                let pts: Vec<Pos2> = (0..=n).map(|i| c + Vec2::angled(i as f32 / n as f32 * std::f32::consts::TAU) * g as f32 * scale).collect();
                p.extend(Shape::dashed_line(&pts, line, 2.0, 4.0));
            }
        }
        p.hline(egui::Rangef::new(c.x - r, c.x + r), c.y, line);
        p.vline(c.x, egui::Rangef::new(c.y - r, c.y + r), line);
        for g in 1..=max_g {
            kit::text(p, Pos2::new(c.x + g as f32 * scale - 3.0, c.y + 2.0), Align2::RIGHT_TOP, &format!("{g}g"), th.medium(10.0), th.ink3);
        }
        kit::text(p, Pos2::new(c.x, m.top()), Align2::CENTER_TOP, if felt { "Brake" } else { "Accel" }, th.medium(10.0), th.ink3);
        kit::text(p, Pos2::new(c.x, m.bottom() + 1.0), Align2::CENTER_BOTTOM, if felt { "Accel" } else { "Brake" }, th.medium(10.0), th.ink3);

        // Felt: braking pushes the dot up, a left turn right. The car's acceleration is the opposite.
        let sign = if felt { 1.0 } else { -1.0 };
        let env: Vec<f32> = (0..BUCKETS).map(|k| envelope_reach(self.peak[k], self.set_at[k], self.clock, self.hold, self.ret)).collect();
        let mut pts = Vec::with_capacity(BUCKETS);
        for k in 0..BUCKETS {
            let mx = env[k].max(0.92 * env[(k + 1) % BUCKETS].max(env[(k + BUCKETS - 1) % BUCKETS]));
            let a = k as f32 / BUCKETS as f32 * std::f32::consts::TAU;
            let rr = mx.min(max_g as f32) * scale;
            pts.push(c + Vec2::new(a.cos() * rr * sign, a.sin() * rr * sign));
        }
        let mut mesh = Mesh::default();
        mesh.colored_vertex(c, alpha(th.ink, 0.07));
        for q in &pts {
            mesh.colored_vertex(*q, alpha(th.ink, 0.07));
        }
        for i in 0..BUCKETS as u32 {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % BUCKETS as u32);
        }
        p.add(Shape::mesh(mesh));
        p.add(Shape::closed_line(pts, Stroke::new(1.0, alpha(th.ink, 0.25))));

        // The fading trail and the dot.
        let at = |x: f32, y: f32| c + Vec2::new(x * sign, y * sign) * scale;
        if let Some(&(now, _, _)) = self.trail.back() {
            let v: Vec<_> = self.trail.iter().collect();
            for w in v.windows(2) {
                let age = ((now - w[1].0) / TRAIL_S) as f32;
                p.line_segment([at(w[0].1, w[0].2), at(w[1].1, w[1].2)], Stroke::new(2.0, alpha(th.attention, 0.65 * (1.0 - age))));
            }
            let (_, x, y) = **v.last().unwrap();
            let mg = max_g as f32;
            let dot = at(x.clamp(-mg, mg), y.clamp(-mg, mg));
            p.circle_filled(dot, 7.0, th.attention);
            p.circle_filled(dot, 3.0, th.ink);
        }
    }

    fn top_view(&self, p: &Painter, th: &Theme, c: Pos2, slip: f32) {
        let k = 84.0 / 80.0;
        let xf = Xf::new(c, [0.0, 0.0], k);
        p.extend(Shape::dashed_line(&[xf.map([0.0, 34.0]), xf.map([0.0, -34.0])], Stroke::new(1.5 * k, th.ink3), 3.0 * k, 3.0 * k));
        p.add(Shape::convex_polygon(vec![xf.map([0.0, -36.0]), xf.map([4.0, -29.0]), xf.map([-4.0, -29.0])], th.ink3, Stroke::NONE));
        let car = &self.cars;
        let xf = xf.turned(slip.to_radians());
        let body = if slip.abs() > 3.0 { th.attention } else { th.wheel };
        for wheel in &car.top_wheels {
            wheel_shape(p, th, wheel, xf);
        }
        car.top_body.fill(p, xf, body);
        car.top_trim.stroke(p, xf, 1.0, alpha(th.panel_solid, 0.4));
        car.top_detail.fill(p, xf, alpha(th.panel_solid, 0.55));
        car.top_cabin.fill(p, xf, body);
        car.top_wing.fill(p, xf, body);
        car.top_wing.stroke(p, xf, 0.8, th.panel_solid);
    }

    fn side_view(&self, p: &Painter, th: &Theme, c: Pos2, pitch: f32) {
        let k = 84.0 / 80.0;
        let xf = Xf::new(c, [0.0, 0.0], k);
        p.line_segment([xf.map([-38.0, 14.0]), xf.map([38.0, 14.0])], Stroke::new(1.5 * k, th.line_strong));
        let xf = xf.turned(-pitch.to_radians());
        let car = &self.cars;
        car.side_body.fill(p, xf, th.wheel);
        car.side_detail.fill(p, xf, alpha(th.panel_solid, 0.55));
        car.side_trim.stroke(p, xf, 1.0, alpha(th.panel_solid, 0.4));
        car.side_wing.fill(p, xf, th.wheel);
        car.side_wing.stroke(p, xf, 0.8, th.panel_solid);
        for cxw in [-20.0, 21.0] {
            let tyre = svg::circle(cxw, 7.5, 6.5);
            wheel_shape(p, th, &tyre, xf);
            p.circle_stroke(xf.map([cxw, 7.5]), 3.2 * k, Stroke::new(1.0 * k, th.ghost_wheel));
        }
    }

    fn rear_view(&self, p: &Painter, th: &Theme, c: Pos2, roll: f32) {
        let k = 84.0 / 80.0;
        let xf = Xf::new(c, [0.0, 0.0], k);
        p.line_segment([xf.map([-36.0, 14.0]), xf.map([36.0, 14.0])], Stroke::new(1.5 * k, th.line_strong));
        let xf = xf.turned(roll.to_radians());
        let car = &self.cars;
        for wheel in &car.rear_wheels {
            wheel_shape(p, th, wheel, xf);
        }
        car.rear_body.fill(p, xf, th.wheel);
        car.rear_detail.fill(p, xf, alpha(th.panel_solid, 0.55));
        car.rear_plate.fill(p, xf, alpha(th.panel_solid, 0.55));
        car.rear_trim.stroke(p, xf, 1.0, alpha(th.panel_solid, 0.4));
        for l in &car.rear_lights {
            l.fill(p, xf, th.brake);
        }
        for wing in &car.rear_wing {
            wing.fill(p, xf, th.wheel);
            wing.stroke(p, xf, 0.8, th.panel_solid);
        }
    }

    fn balance(&self, p: &Painter, th: &Theme, side: Rect, live: Option<f32>) {
        let value = live.or(self.last_balance);
        let is_live = live.is_some();
        let v = value.unwrap_or(0.0);
        let (word, color) = match value {
            None => ("Learning your car", th.ink2),
            Some(v) if v < -0.25 => ("Understeer", th.lapped),
            Some(v) if v > 0.25 => ("Oversteer", th.attention),
            Some(_) => ("Neutral", th.gain),
        };
        let color = if is_live { color } else { th.ink2 };
        let bottom = side.bottom();
        let ends_y = bottom - 7.0;
        let track = Rect::from_min_max(Pos2::new(side.left(), ends_y - 7.0 - 4.0 - 10.0), Pos2::new(side.right(), ends_y - 7.0 - 4.0));
        let head_y = track.top() - 4.0 - 10.0;
        kit::text(p, Pos2::new(side.left(), head_y + 2.0), Align2::LEFT_CENTER, "Balance", th.medium(12.0), th.ink3);
        kit::text(p, Pos2::new(side.right(), head_y), Align2::RIGHT_CENTER, word, th.bold(16.0), color);
        // Blue for understeer through grey to amber for oversteer.
        let grey = alpha(th.ink, 0.08);
        let stops = [(0.0, alpha(th.lapped, 0.55)), (0.4, grey), (0.6, grey), (1.0, alpha(th.attention, 0.55))];
        kit::pill_gradient(p, track, &stops);
        p.rect_filled(Rect::from_center_size(Pos2::new(track.center().x, track.center().y), Vec2::new(2.0, 16.0)), 0.0, th.ink3);
        if value.is_some() {
            let x = track.left() + track.width() * (0.5 + v.clamp(-1.0, 1.0) * 0.5);
            let c = Pos2::new(x, track.center().y);
            p.circle_filled(c, 9.0, th.panel_solid);
            p.circle_filled(c, 7.0, if is_live { th.ink } else { th.ink2 });
        }
        kit::text(p, Pos2::new(side.left(), ends_y), Align2::LEFT_CENTER, "Under", th.medium(11.0), th.ink3);
        kit::text(p, Pos2::new(side.right(), ends_y), Align2::RIGHT_CENTER, "Over", th.medium(11.0), th.ink3);
    }
}

/// A tyre: dark fill with a light edge.
fn wheel_shape(p: &Painter, th: &Theme, shape: &Path, xf: Xf) {
    shape.fill(p, xf, Color32::from_rgb(0x2a, 0x34, 0x42));
    shape.stroke(p, xf, 1.2, th.ghost_wheel);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_holds_then_returns() {
        assert_eq!(envelope_reach(2.0, 0.0, 5.0, 10.0, 10.0), 2.0);
        assert_eq!(envelope_reach(2.0, 0.0, 15.0, 10.0, 10.0), 1.0);
        assert_eq!(envelope_reach(2.0, 0.0, 25.0, 10.0, 10.0), 0.0);
        assert_eq!(envelope_reach(2.0, 0.0, 10.5, 10.0, 0.0), 0.0);
    }
}
