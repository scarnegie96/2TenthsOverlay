//! Instruments (`src/widgets/Instruments.tsx`, `dashIcons.tsx`): warning
//! lights that light up when iRacing raises them, engine gauges, and a brake
//! bias dial that flashes the change when you adjust it.

use egui::{Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use telemetry::instruments::Warnings;

use super::{Ctx, Widget};
use crate::config::Units;
use crate::kit::{self, seg};
use crate::logic::format::fixed;
use crate::svg::{self, Path, Xf};
use crate::theme::{alpha, Theme};

#[derive(Clone, Copy, PartialEq)]
enum Level {
    Danger,
    Caution,
    Info,
}

/// One part of an icon: stroked, or filled.
struct Part {
    path: Path,
    fill: bool,
    /// Circles are drawn as rings (egui's own) rather than flattened.
    circle: Option<(f32, f32, f32)>,
}

fn stroke(d: &str) -> Part {
    Part { path: Path::parse(d), fill: false, circle: None }
}
fn filled(d: &str) -> Part {
    Part { path: Path::parse(d), fill: true, circle: None }
}
fn ring(cx: f32, cy: f32, r: f32, fill: bool) -> Part {
    Part { path: svg::circle(cx, cy, r), fill, circle: Some((cx, cy, r)) }
}
fn rect(x: f32, y: f32, w: f32, h: f32, rx: f32) -> Part {
    Part { path: svg::rect(x, y, w, h, rx), fill: false, circle: None }
}

/// The lights in dashboard order: label, how serious, which flag, icon.
fn lights() -> Vec<Light> {
    vec![
        (
            "Oil press",
            Level::Danger,
            |w| w.oil_pressure,
            vec![
                stroke("M5 13h4l3-2h8l7-3-6 10H7a2 2 0 0 1-2-2z"),
                stroke("M14 11V8m-2 0h4"),
                filled("M28 18c0 1.6-1 2.6-2 2.6s-2-1-2-2.6 2-3.4 2-3.4 2 1.8 2 3.4z"),
                stroke("M4 24h22"),
            ],
        ),
        (
            "Oil temp",
            Level::Caution,
            |w| w.oil_temp,
            vec![stroke("M11 5s-6 7-6 11a6 6 0 0 0 12 0c0-4-6-11-6-11z"), stroke("M23 6v13"), ring(23.0, 22.0, 3.0, false), stroke("M26 9h2M26 13h2M26 17h2")],
        ),
        (
            "Water temp",
            Level::Danger,
            |w| w.water_temp,
            vec![stroke("M16 4v13"), ring(16.0, 20.0, 3.0, false), stroke("M19 7h3M19 11h3M19 15h3"), stroke("M4 26c2-1.5 4-1.5 6 0s4 1.5 6 0 4-1.5 6 0 4 1.5 6 0")],
        ),
        (
            "Fuel press",
            Level::Danger,
            |w| w.fuel_pressure,
            vec![rect(5.0, 6.0, 12.0, 21.0, 2.0), stroke("M17 12h3l3 3v8a2 2 0 0 0 4 0V10l-3-3"), ring(11.0, 13.0, 4.0, false), stroke("M11 13l2-2"), stroke("M3 27h16")],
        ),
        (
            "Low fuel",
            Level::Caution,
            |w| w.low_fuel,
            vec![rect(5.0, 6.0, 12.0, 21.0, 2.0), rect(8.0, 9.0, 6.0, 5.0, 1.0), stroke("M17 12h3l3 3v8a2 2 0 0 0 4 0V10l-3-3"), stroke("M3 27h16")],
        ),
        (
            "Stalled",
            Level::Danger,
            |w| w.stalled,
            vec![stroke("M7 12h3v-3h9v3h3l3 3h2v8h-2l-3 3H10l-3-3H4v-8h3z"), stroke("M13 6h5"), stroke("M15.5 15v5"), stroke("M13 17.5h5")],
        ),
        ("Limiter", Level::Info, |w| w.pit_limiter, vec![stroke("M6 22a11 11 0 1 1 20 0"), stroke("M13 22V11h4a3 3 0 0 1 0 6h-4"), stroke("M4 26h24")]),
        (
            "Rev limit",
            Level::Caution,
            |w| w.rev_limiter,
            vec![stroke("M5 22a11 11 0 1 1 22 0"), stroke("M16 22l7-8"), ring(16.0, 22.0, 2.0, true), stroke("M23 10l2-2M27 15h2")],
        ),
        (
            "Repair",
            Level::Danger,
            |w| w.repair_required,
            vec![stroke("M20 5a6 6 0 0 0-5.6 8.2L5 22.6a2.6 2.6 0 0 0 3.7 3.7l9.4-9.4A6 6 0 0 0 26 11.4l-3.6 3.6-4-1.4-1.4-4 3.6-3.6A6 6 0 0 0 20 5z")],
        ),
        (
            "Opt. repair",
            Level::Caution,
            |w| w.repair_optional,
            vec![stroke("M17 7a5 5 0 0 0-4.7 6.8L5 21.1a2.2 2.2 0 0 0 3.1 3.1l7.3-7.3A5 5 0 0 0 22 12.2l-3 3-3.3-1.2-1.2-3.3 3-3A5 5 0 0 0 17 7z"), stroke("M25 20v8M21 24h8")],
        ),
    ]
}

type Light = (&'static str, Level, fn(&Warnings) -> bool, Vec<Part>);
/// Label, value, unit, min, max, warning, digits.
type GaugeRow<'a> = (&'a str, Option<f32>, &'a str, f32, f32, bool, usize);

pub struct Instruments {
    lights: Vec<Light>,
    /// The last bias seen, and the latest change: how much, and when (app seconds).
    prev_bias: Option<f32>,
    change: Option<(f32, f64)>,
}

impl Default for Instruments {
    fn default() -> Self {
        Self { lights: lights(), prev_bias: None, change: None }
    }
}

const BIAS_MIN: f32 = 40.0;
const BIAS_MAX: f32 = 70.0;

fn temp(c: f32, u: Units) -> f32 {
    crate::logic::format::temp(c, u)
}
fn press(bar: f32, u: Units) -> f32 {
    if u == Units::Imperial {
        bar * 14.5038
    } else {
        bar
    }
}

impl Widget for Instruments {
    fn observe(&mut self, live: &crate::data::Live, now: f64) {
        let bias = live.tick.as_ref().and_then(|t| t.instruments.brake_bias);
        if let Some(b) = bias {
            if let Some(before) = self.prev_bias {
                if (b - before).abs() > 0.01 {
                    self.change = Some((b - before, now));
                }
            }
            self.prev_bias = Some(b);
        }
        if self.change.is_some_and(|(_, at)| now - at > 2.5) {
            self.change = None;
        }
    }

    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let ins = cx.live.tick.as_ref().map(|t| t.instruments);
        let test = cx.editing && o.b("testLights");
        let lit = |f: fn(&Warnings) -> bool| test || ins.as_ref().is_some_and(|i| f(&i.warnings));
        let any_danger = self.lights.iter().any(|l| l.1 == Level::Danger && lit(l.2));
        let w = 480.0;
        let panel = kit::Panel::begin(p, th, w, None, &[]);
        let x = panel.x();
        let top = panel.body_top;

        // Warning lights.
        let cell_w = (x.span() - 16.0 - 18.0) / 10.0;
        let pulse = 1.0 - 0.45 * (0.5 - 0.5 * (cx.now * std::f64::consts::PI / 0.8).cos()) as f32;
        for (i, (label, level, flag, parts)) in self.lights.iter().enumerate() {
            let on = lit(*flag);
            let r = Rect::from_min_size(Pos2::new(x.min + 8.0 + i as f32 * (cell_w + 2.0), top + 8.0), Vec2::new(cell_w, 49.0));
            let (color, bg) = match (on, level) {
                (false, _) => (alpha(th.ink, 0.16), None),
                (true, Level::Danger) => (th.loss, Some(alpha(th.loss, 0.16))),
                (true, Level::Caution) => (th.attention, Some(alpha(th.attention, 0.14))),
                (true, Level::Info) => (th.lapped, Some(alpha(th.lapped, 0.16))),
            };
            if let Some(bg) = bg {
                kit::fill(p, r, 5.0, bg);
            }
            let icon_color = if on && *level == Level::Danger { color.gamma_multiply(pulse) } else { color };
            let xf = Xf::new(Pos2::new(r.center().x, r.top() + 4.0 + 15.0), [16.0, 16.0], 30.0 / 32.0);
            if on {
                // A soft glow behind a lit icon.
                p.circle_filled(xf.at, 13.0, alpha(color, 0.12));
            }
            for part in parts {
                draw_part(p, part, xf, icon_color);
            }
            let label_c = if on { th.ink } else { alpha(th.ink3, 0.55) };
            kit::text(p, Pos2::new(r.center().x, r.bottom() - 3.0), Align2::CENTER_BOTTOM, label, th.medium(10.0), label_c);
        }
        let mut y = top + 8.0 + 49.0 + 6.0;
        let (show_gauges, show_bias) = (o.b("showGauges"), o.b("showBias"));
        if show_gauges || show_bias {
            p.hline(x, y + 0.5, Stroke::new(1.0, th.line));
            y += 1.0;
            // The bias column sets the height: dial, value, label, and the peak line when there is one.
            let extra = show_bias && ins.as_ref().is_some_and(|i| i.peak_brake_bias.is_some() || i.brake_bias_fine.is_some());
            let bias_h: f32 = 4.0 + 77.0 - 2.0 + 24.0 + 2.0 + 14.4 + if extra { 2.0 + 13.2 } else { 0.0 };
            let lower_h = if show_bias { bias_h.max(if show_gauges { 110.0 } else { 0.0 }) } else { 110.0 };
            let inner = Rect::from_min_max(Pos2::new(x.min + 12.0, y + 8.0), Pos2::new(x.max - 12.0, y + 8.0 + lower_h));
            let mut gauges_right = inner.right();
            if show_bias {
                let b = Rect::from_min_max(Pos2::new(inner.right() - 150.0, inner.top()), inner.max);
                gauges_right = b.left() - 14.0;
                self.bias(p, th, b, ins.as_ref(), cx.now);
            }
            if show_gauges {
                gauges(p, th, Rect::from_min_max(inner.min, Pos2::new(gauges_right, inner.bottom())), ins.as_ref(), cx.units);
            }
            y = inner.bottom() + 10.0;
        }
        let size = panel.finish(p, th, y);
        if any_danger && !th.broadcast() {
            p.rect_stroke(Rect::from_min_size(Pos2::ZERO, size), kit::radius(th.radius), Stroke::new(1.0, alpha(th.loss, 0.6)), egui::StrokeKind::Inside);
        }
        size
    }
}

fn draw_part(p: &Painter, part: &Part, xf: Xf, color: Color32) {
    if let Some((cx, cy, r)) = part.circle {
        if part.fill {
            p.circle_filled(xf.map([cx, cy]), r * xf.k, color);
        } else {
            p.circle_stroke(xf.map([cx, cy]), r * xf.k, Stroke::new(2.0 * xf.k, color));
        }
    } else if part.fill {
        part.path.fill(p, xf, color);
    } else {
        part.path.stroke_round(p, xf, 2.0, color);
        // Round joins: a dot at every corner.
        for s in &part.path.subs {
            for q in &s.pts {
                p.circle_filled(xf.map(*q), xf.k, color);
            }
        }
    }
}

fn gauges(p: &Painter, th: &Theme, r: Rect, ins: Option<&telemetry::instruments::InstrumentsPayload>, units: Units) {
    let (tu, pu) = if units == Units::Imperial { ("°F", "psi") } else { ("°C", "bar") };
    let w = ins.map(|i| i.warnings).unwrap_or_default();
    let rows: [GaugeRow; 5] = [
        ("Oil temp", ins.and_then(|i| i.oil_temp).map(|v| temp(v, units)), tu, temp(60.0, units), temp(140.0, units), w.oil_temp, 0),
        ("Oil press", ins.and_then(|i| i.oil_press).map(|v| press(v, units)), pu, 0.0, press(8.0, units), w.oil_pressure, 1),
        ("Water", ins.and_then(|i| i.water_temp).map(|v| temp(v, units)), tu, temp(50.0, units), temp(120.0, units), w.water_temp, 0),
        ("Fuel press", ins.and_then(|i| i.fuel_press).map(|v| press(v, units)), pu, 0.0, press(6.0, units), w.fuel_pressure, 1),
        ("Battery", ins.and_then(|i| i.voltage), "V", 10.0, 15.0, false, 1),
    ];
    let row_h = 18.0;
    let total = rows.len() as f32 * row_h + (rows.len() - 1) as f32 * 5.0;
    let mut y = r.center().y - total / 2.0;
    for (label, value, unit, min, max, warn, digits) in rows {
        let mut pp = p.clone();
        if value.is_none() {
            pp.multiply_opacity(0.45);
        }
        let p = &pp;
        let cy = y + row_h / 2.0;
        kit::text(p, Pos2::new(r.left(), cy), Align2::LEFT_CENTER, label, th.medium(13.0), th.ink3);
        let bar = Rect::from_min_max(Pos2::new(r.left() + 66.0 + 8.0, cy - 3.0), Pos2::new(r.right() - 66.0 - 8.0, cy + 3.0));
        kit::fill(p, bar, 3.0, alpha(th.ink, 0.08));
        let frac = value.map(|v| ((v - min) / (max - min)).clamp(0.0, 1.0)).unwrap_or(0.0);
        if frac > 0.0 {
            kit::fill(p, Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, bar.height())), 3.0, if warn { th.loss } else { th.ink2 });
        }
        let vs = [
            seg(value.map(|v| fixed(v as f64, digits)).unwrap_or("–".into()), th.bold(15.0), if warn { th.loss } else { th.ink }),
            seg(format!(" {unit}"), th.medium(11.0), th.ink3),
        ];
        let vw = kit::segs_width(p, &vs, 0.0);
        kit::segs_baseline(p, r.right() - vw, cy + 5.5, &vs);
        y += row_h + 5.0;
    }
}

impl Instruments {
    fn bias(&self, p: &Painter, th: &Theme, r: Rect, ins: Option<&telemetry::instruments::InstrumentsPayload>, now: f64) {
        p.vline(r.left() + 0.5, r.y_range(), Stroke::new(1.0, th.line));
        let bias = ins.and_then(|i| i.brake_bias);
        let change = self.change.filter(|(_, at)| now - at <= 2.5);
        let area = Rect::from_min_max(Pos2::new(r.left() + 12.0, r.top()), r.max);
        let k = 132.0 / 96.0;
        let origin = Pos2::new(area.center().x, area.top() + 4.0 + 48.0 * k);
        let at = |x: f32, y: f32| origin + Vec2::new(x, y) * k;
        let mut dial = p.clone();
        if bias.is_none() {
            dial.multiply_opacity(0.35);
        }
        // Track, ticks every 5 %, the needle and the hub.
        let n = 32;
        let track: Vec<Pos2> = (0..=n).map(|i| {
            let a = std::f32::consts::PI * (1.0 + i as f32 / n as f32);
            at(40.0 * a.cos(), 40.0 * a.sin())
        }).collect();
        dial.add(Shape::line(track.clone(), Stroke::new(7.0 * k, alpha(th.ink, 0.10))));
        for end in [track[0], track[n]] {
            dial.circle_filled(end, 3.5 * k, alpha(th.ink, 0.10));
        }
        let mut v = BIAS_MIN;
        while v <= BIAS_MAX + 0.01 {
            let a = (-90.0 + (v - BIAS_MIN) / (BIAS_MAX - BIAS_MIN) * 180.0).to_radians();
            let major = (v as i32) % 10 == 0;
            let inner = if major { 33.0 } else { 36.0 };
            let (c, w) = if major { (th.ink2, 1.8) } else { (th.ink3, 1.2) };
            dial.line_segment([at(a.sin() * 40.0, -a.cos() * 40.0), at(a.sin() * inner, -a.cos() * inner)], Stroke::new(w * k, c));
            v += 5.0;
        }
        if let Some(b) = bias {
            let frac = ((b - BIAS_MIN) / (BIAS_MAX - BIAS_MIN)).clamp(0.0, 1.0);
            let a = (-90.0 + frac * 180.0f32).to_radians();
            let rot = |x: f32, y: f32| at(x * a.cos() - y * a.sin(), x * a.sin() + y * a.cos());
            dial.add(Shape::convex_polygon(vec![rot(-2.5, 0.0), rot(0.0, -34.0), rot(2.5, 0.0)], th.attention, Stroke::NONE));
        }
        dial.circle_filled(origin, 4.0 * k, th.ink);
        kit::text(&dial, at(-40.0, 9.0), Align2::CENTER_BOTTOM, &format!("{BIAS_MIN:.0}"), th.semi(9.0 * k), th.ink3);
        kit::text(&dial, at(40.0, 9.0), Align2::CENTER_BOTTOM, &format!("{BIAS_MAX:.0}"), th.semi(9.0 * k), th.ink3);

        // The readout, each line in its CSS line box: 24 px value, 12 px label, 11 px extra.
        let cxm = area.center().x;
        let mut y = area.top() + 4.0 + 77.0 - 2.0;
        let color = if change.is_some() { th.attention } else { th.ink };
        kit::text(p, Pos2::new(cxm, y + 12.0), Align2::CENTER_CENTER, &bias.map(|b| format!("{}%", fixed(b as f64, 1))).unwrap_or("–".into()), th.bold(24.0), color);
        y += 24.0 + 2.0;
        let label = match (bias, change) {
            (None, _) => "Bias not adjustable".to_string(),
            (Some(_), Some((by, _))) => format!("Moved {} to the {}", fixed(by.abs() as f64, 1), if by > 0.0 { "front" } else { "rear" }),
            _ => "Brake bias front".to_string(),
        };
        kit::text(p, Pos2::new(cxm, y + 7.2), Align2::CENTER_CENTER, &label, th.medium(12.0), if change.is_some() { th.attention } else { th.ink3 });
        y += 14.4 + 2.0;
        let peak = ins.and_then(|i| i.peak_brake_bias);
        let fine = ins.and_then(|i| i.brake_bias_fine);
        if peak.is_some() || fine.is_some() {
            let mut parts = Vec::new();
            if let Some(pk) = peak {
                parts.push(format!("Peak {}%", fixed(pk as f64, 1)));
            }
            if let Some(f) = fine {
                parts.push(format!("Fine {}", fixed(f as f64, 1)));
            }
            kit::text(p, Pos2::new(cxm, y + 6.6), Align2::CENTER_CENTER, &parts.join("   "), th.medium(11.0), th.ink2);
        }
    }
}
