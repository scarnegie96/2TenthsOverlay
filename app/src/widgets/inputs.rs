//! Inputs (`src/widgets/Inputs.tsx`): gear, speed, a steering wheel shaped
//! like the car's, the pedal trace and pedal bars. With a reference lap
//! loaded, a slimmer strip grows out of the top with the reference's inputs
//! at the same spot, on the same columns so the traces line up.

use egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use super::{Ctx, Widget};
use crate::config::Units;
use crate::data::TracePoint;
use crate::kit;
use crate::logic::format::{gear, speed};
use crate::theme::{alpha, mix, Theme};

#[derive(Default)]
pub struct Inputs {
    /// Height of the reference strip above the panel, when it is drawn inside the window.
    over: (f32, f32),
    /// Room above the panel, to see whether the strip fits there (unknown until placed).
    room: Option<f32>,
}

/// The reference strip's height, less the pixel it overlaps the panel by.
const GHOST_H: f32 = 82.0;

const TRACE_W: f32 = 300.0;
const TRACE_H: f32 = 92.0;
const GHOST_TRACE_H: f32 = 62.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelStyle {
    Round,
    Gt,
    Formula,
    Oval,
}

/// Picks a wheel for the car when the option is "auto", from whole words in its name.
pub fn resolve_wheel(option: &str, car_name: &str) -> WheelStyle {
    match option {
        "round" => return WheelStyle::Round,
        "gt" => return WheelStyle::Gt,
        "formula" => return WheelStyle::Formula,
        "oval" => return WheelStyle::Oval,
        _ => {}
    }
    let n = car_name.to_lowercase();
    let has = |words: &[&str]| {
        words.iter().any(|w| {
            let mut from = 0;
            while let Some(i) = n[from..].find(w) {
                let at = from + i;
                let end = at + w.len();
                let before = n[..at].chars().next_back();
                let after = n[end..].chars().next();
                let edge = |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric());
                if edge(before) && edge(after) {
                    return true;
                }
                from = at + 1;
            }
            false
        })
    };
    if has(&[
        "formula", "f1", "f2", "f3", "f4", "fr2.0", "ff1600", "ir18", "ir-18", "indycar", "dallara", "super formula", "sf23", "w12", "w13", "mp4",
        "lotus 79", "lotus 49", "skip barber", "pro mazda", "usf", "indy pro", "porsche 919", "lmp", "gtp", "hypercar",
    ]) {
        return WheelStyle::Formula;
    }
    if has(&["nascar", "cup", "xfinity", "truck", "late model", "modified", "sprint car", "midget", "legends", "street stock", "arca", "super late"]) {
        // "Cup" also matches one-make sports car cups.
        if has(&["porsche", "carrera", "992", "991", "radical", "mx-5", "gr86", "clio"]) {
            return if has(&["porsche", "992", "991"]) { WheelStyle::Gt } else { WheelStyle::Round };
        }
        return WheelStyle::Oval;
    }
    if has(&["gt3", "gt4", "gte", "gtd", "gt1", "gt2", "992", "991", "rsr", "evo", "amg", "m4", "nsx", "huracan", "r8", "296", "720s"]) {
        return WheelStyle::Gt;
    }
    WheelStyle::Round
}

/// Colours of one strip: yours, or the muted reference's.
struct Palette {
    gear: Color32,
    speed: Color32,
    active_pct: Color32,
    throttle: Color32,
    brake: Color32,
    clutch: Color32,
    wheel: Color32,
    plate: Color32,
    mark: Color32,
}

struct Strip<'a> {
    ghost: bool,
    label: Option<&'a str>,
    gear: i32,
    speed_ms: f32,
    steering: Option<f32>,
    steering_note: Option<&'a str>,
    throttle: f32,
    brake: f32,
    abs: bool,
    /// None keeps the clutch bar's space empty so columns stay aligned.
    clutch: Option<f32>,
    shift: bool,
    leds: f32,
}

struct Layout {
    show_clutch: bool,
    show_steering: bool,
    show_trace: bool,
    seconds: f64,
    units: Units,
    wheel: WheelStyle,
}

impl Layout {
    /// Width of the main panel (border box).
    fn width(&self) -> f32 {
        let mut w = 1.0 + 18.0 + 52.0 + 16.0 + 78.0;
        if self.show_steering {
            w += 16.0 + 86.0;
        }
        if self.show_trace {
            w += 16.0 + TRACE_W;
        }
        let bars = if self.show_clutch { 3.0 } else { 2.0 };
        w += 16.0 + bars * 24.0 + (bars - 1.0) * 6.0 + 14.0 + 1.0;
        w
    }
}

impl Widget for Inputs {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let tick = cx.live.tick.as_deref();
        let car = cx.live.session.as_ref().map(|s| s.car_name.as_str()).unwrap_or("");
        let layout = Layout {
            show_clutch: o.b("showClutch"),
            show_steering: o.b("showSteering"),
            show_trace: o.b("showTrace"),
            seconds: o.n("traceSeconds").clamp(1.0, crate::data::TRACE_SECONDS),
            units: cx.units,
            wheel: resolve_wheel(o.s("wheel"), car),
        };
        let shift_rpm = if o.b("shiftLight") { tick.and_then(|t| t.shift_rpm) } else { None };
        let rpm = tick.map(|t| t.rpm).unwrap_or(0.0);
        let leds = tick.and_then(|t| t.shift_rpm).filter(|s| *s > 0.0).map(|s| ((rpm / s - 0.75) / 0.25).clamp(0.0, 1.0)).unwrap_or(0.0);
        let w = layout.width();
        let h = 112.0 + 2.0;

        // Your strip.
        // The reference strip sticks out above the panel. In a window of its own that is part of the
        // window: the panel moves down by it and the window is placed that much higher.
        let ghost = tick.and_then(|t| t.coach.as_ref()).filter(|_| o.b("showGhost"));
        let extra = GHOST_H - 1.0;
        // Above if it fits on the display, else below.
        let below = cx.windowed && ghost.is_some() && self.room.is_some_and(|r| r < extra);
        let dy = if cx.windowed && ghost.is_some() && !below { extra } else { 0.0 };
        self.over = if below { (0.0, extra) } else { (dy, 0.0) };
        let rect = Rect::from_min_size(Pos2::new(0.0, dy), Vec2::new(w, h));
        panel_bg(p, th, rect, false, false);
        let mine = Strip {
            ghost: false,
            label: None,
            gear: tick.map(|t| t.gear).unwrap_or(0),
            speed_ms: tick.map(|t| t.speed).unwrap_or(0.0),
            steering: Some(tick.map(|t| t.steering).unwrap_or(0.0)),
            steering_note: None,
            throttle: tick.map(|t| t.throttle).unwrap_or(0.0),
            brake: tick.map(|t| t.brake).unwrap_or(0.0),
            abs: tick.is_some_and(|t| t.abs_active),
            clutch: Some(tick.map(|t| t.clutch).unwrap_or(0.0)),
            shift: shift_rpm.is_some_and(|s| rpm >= s),
            leds,
        };
        strip(p, th, &layout, &mine, Rect::from_min_size(Pos2::new(1.0 + 18.0, dy + 1.0 + 10.0), Vec2::new(w - 2.0 - 32.0, 92.0)), &cx.live.trace);

        // The reference's strip, growing upwards out of the top.
        if let Some(coach) = ghost {
            let gh = GHOST_H;
            let g = if below {
                Rect::from_min_max(Pos2::new(12.0, h - 1.0), Pos2::new(w - 12.0, h - 1.0 + gh))
            } else {
                Rect::from_min_max(Pos2::new(12.0, dy + 1.0 - gh), Pos2::new(w - 12.0, dy + 1.0))
            };
            panel_bg(p, th, g, true, below);
            let s = Strip {
                ghost: true,
                label: Some("Ref"),
                gear: coach.ref_gear.unwrap_or(0),
                speed_ms: coach.ref_speed.unwrap_or(0.0),
                steering: coach.ref_steering,
                steering_note: coach.ref_steering.is_none().then_some("No wheel data"),
                throttle: coach.ref_throttle.unwrap_or(0.0),
                brake: coach.ref_brake.unwrap_or(0.0),
                abs: false,
                clutch: None,
                shift: false,
                leds: 0.0,
            };
            // Same columns as yours: 12 px inset plus 6 px padding = 18.
            strip(p, th, &layout, &s, Rect::from_min_max(Pos2::new(19.0, g.top() + 10.0), Pos2::new(w - 15.0, g.bottom() - 10.0)), &cx.live.trace);
        }
        Vec2::new(w, h + self.over.0 + self.over.1)
    }

    fn overhang(&self) -> (f32, f32) {
        self.over
    }

    fn set_room_above(&mut self, px: f32) {
        self.room = Some(px);
    }
}

fn panel_bg(p: &Painter, th: &Theme, r: Rect, ghost: bool, flipped: bool) {
    let rad = th.radius as u8;
    if ghost {
        // The reference strip: rounded on the free edge (top when above the panel, bottom when below), accent line there.
        let (round, edge) = if flipped {
            (CornerRadius { nw: 0, ne: 0, sw: rad, se: rad }, Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 2.0), r.max))
        } else {
            (CornerRadius { nw: rad, ne: rad, sw: 0, se: 0 }, Rect::from_min_size(r.min, Vec2::new(r.width(), 2.0)))
        };
        p.rect_filled(r, round, th.panel);
        let pts = if flipped {
            vec![r.left_top(), r.left_bottom(), r.right_bottom(), r.right_top()]
        } else {
            vec![r.left_bottom(), r.left_top(), r.right_top(), r.right_bottom()]
        };
        if !th.broadcast() {
            p.add(Shape::line(pts, Stroke::new(1.0, th.line)));
        }
        p.rect_filled(edge, round, th.fastest);
    } else if th.broadcast() {
        let sh = egui::epaint::Shadow { offset: [0, 2], blur: 12, spread: 0, color: Color32::from_black_alpha(89) };
        p.add(sh.as_shape(r, CornerRadius::ZERO));
        p.rect_filled(r, 0.0, th.panel);
    } else {
        p.rect_filled(r, kit::radius(th.radius), th.panel);
        p.rect_stroke(r, kit::radius(th.radius), Stroke::new(1.0, th.line), egui::StrokeKind::Inside);
    }
}

fn palette(th: &Theme, ghost: bool) -> Palette {
    if ghost {
        Palette {
            gear: th.ghost_attention,
            speed: th.ink2,
            active_pct: th.ink2,
            throttle: th.ghost_throttle,
            brake: th.ghost_brake,
            clutch: th.clutch,
            wheel: th.ghost_wheel,
            plate: mix(th.ghost_wheel, th.panel_solid, 0.60),
            mark: th.ghost_brake,
        }
    } else {
        Palette {
            gear: th.attention,
            speed: th.ink,
            active_pct: th.ink,
            throttle: th.throttle,
            brake: th.brake,
            clutch: th.clutch,
            wheel: th.wheel,
            plate: mix(th.wheel, th.panel_solid, 0.72),
            mark: th.loss,
        }
    }
}

/// One row of the HUD, laid out in `area` (inside the padding).
fn strip(p: &Painter, th: &Theme, l: &Layout, s: &Strip, area: Rect, trace: &[TracePoint]) {
    let pal = palette(th, s.ghost);
    let cy = area.center().y;
    let mut x = area.left();

    // Gear, red at the shift point, with a caption under it on the reference strip.
    let gear_c = if s.shift { th.loss } else { pal.gear };
    let gear_size = if s.ghost { 64.0 } else { 96.0 };
    let line_h = gear_size * 0.8;
    let caption_h = if s.label.is_some() { 6.0 + 16.0 } else { 0.0 };
    let top = cy - (line_h + caption_h) / 2.0;
    kit::text(p, Pos2::new(x + 26.0, top + line_h / 2.0), Align2::CENTER_CENTER, &gear(s.gear), th.bold(gear_size), gear_c);
    if let Some(label) = s.label {
        let w = kit::width(p, label, th.bold(12.0)) + 12.0;
        let r = Rect::from_min_size(Pos2::new(x + 26.0 - w / 2.0, top + line_h + 6.0), Vec2::new(w, 16.0));
        kit::fill(p, r, 3.0, th.fastest);
        kit::text(p, r.center(), Align2::CENTER_CENTER, label, th.bold(12.0), th.panel_solid);
    }
    x += 52.0 + 16.0;

    // Speed.
    let (v_size, u_size, u_gap) = if s.ghost { (32.0, 11.0, 2.0) } else { (44.0, 13.0, 4.0) };
    let v_h = v_size * 0.95;
    let block = v_h + u_gap + u_size * 1.2;
    let top = cy - block / 2.0;
    kit::text(p, Pos2::new(x + 39.0, top + v_h / 2.0), Align2::CENTER_CENTER, &speed(s.speed_ms, l.units).to_string(), th.bold(v_size), pal.speed);
    let unit = if l.units == Units::Imperial { "MPH" } else { "KPH" };
    kit::text_sp(p, Pos2::new(x + 39.0, top + v_h + u_gap), Align2::CENTER_TOP, unit, th.semi(u_size), th.ink3, u_size * 0.06);
    x += 78.0;

    if l.show_steering {
        x += 16.0;
        let c = Pos2::new(x + 43.0, cy);
        let size = if s.ghost { 60.0 } else { 86.0 };
        if let Some(angle) = s.steering {
            wheel(p, th, &pal, c, size, angle, l.wheel, s.leds);
        }
        if let Some(note) = s.steering_note {
            let g = p.layout(note.to_owned(), th.medium(11.0), th.ink3, 80.0);
            let g_size = g.size();
            p.galley(c - g_size / 2.0, g, th.ink3);
        }
        x += 86.0;
    }

    if l.show_trace {
        x += 16.0;
        let h = if s.ghost { GHOST_TRACE_H } else { TRACE_H };
        let r = Rect::from_min_size(Pos2::new(x, cy - h / 2.0), Vec2::new(TRACE_W, h));
        trace_view(p, th, &pal, r, trace, l.seconds, s.ghost);
        x += TRACE_W;
    }

    // Pedal bars.
    x += 16.0;
    let bars_h = if s.ghost { 62.0 } else { 92.0 };
    let top = cy - bars_h / 2.0;
    let mut bars: Vec<Option<(f32, Color32, bool)>> = Vec::new();
    if l.show_clutch {
        bars.push(s.clutch.map(|c| (c, pal.clutch, false)));
    }
    bars.push(Some((s.brake, pal.brake, s.abs)));
    bars.push(Some((s.throttle, pal.throttle, false)));
    let pct_size = if s.ghost { 12.0 } else { 14.0 };
    for b in bars {
        if let Some((v, color, abs)) = b {
            let pct = (v.clamp(0.0, 1.0) * 100.0).round();
            let pct_c = if pct > 0.0 { pal.active_pct } else { th.ink3 };
            kit::text(p, Pos2::new(x + 12.0, top), Align2::CENTER_TOP, &format!("{pct:.0}"), th.bold(pct_size), pct_c);
            let track = Rect::from_min_max(Pos2::new(x, top + pct_size + 4.0), Pos2::new(x + 24.0, top + bars_h));
            kit::fill(p, track, 3.0, alpha(th.ink, 0.08));
            if pct > 0.0 {
                let fh = track.height() * pct / 100.0;
                kit::fill(p, Rect::from_min_max(Pos2::new(track.left(), track.bottom() - fh), track.max), 3.0, color);
            }
            if abs {
                // The ABS is working: an amber tag on the bar.
                let tag = Rect::from_center_size(Pos2::new(track.center().x, track.top() + 9.0), Vec2::new(22.0, 13.0));
                kit::fill(p, tag, 3.0, th.attention);
                kit::text(p, tag.center(), Align2::CENTER_CENTER, "ABS", th.heavy(9.0), th.panel_solid);
            }
            kit::outline(p, track, 3.0, 1.0, th.line);
        }
        x += 24.0 + 6.0;
    }
}

/// The rolling throttle and brake trace (yours, or the reference's at the same spots).
fn trace_view(p: &Painter, th: &Theme, pal: &Palette, r: Rect, trace: &[TracePoint], seconds: f64, ghost: bool) {
    kit::fill(p, r, 4.0, alpha(th.well, 0.22));
    let pad = 4.0;
    let y = |v: f32| r.top() + pad + (1.0 - v.clamp(0.0, 1.0)) * (r.height() - pad * 2.0);
    for v in [0.25, 0.5, 0.75] {
        kit::dashed_h(p, r.x_range(), y(v).round() + 0.5, th.grid, 1.0, 3.0, 4.0);
    }
    let now = trace.last().map(|t| t.t).unwrap_or(0.0);
    let x = |t: f64| r.right() - ((now - t) / seconds) as f32 * r.width();
    // Solid time lines that scroll with the trace, one per second (or two).
    let step = if seconds > 10.0 { 2.0 } else { 1.0 };
    let mut t = (now / step).floor() * step;
    while t > now - seconds {
        p.vline(x(t).round() + 0.5, r.y_range(), Stroke::new(1.0, th.grid));
        t -= step;
    }
    let start = trace.partition_point(|s| s.t < now - seconds - 0.1);
    let samples = &trace[start..];
    if samples.len() > 1 {
        let width = if ghost { 2.0 } else { 2.5 };
        let clip = p.with_clip_rect(r);
        let line = |get: &dyn Fn(&TracePoint) -> Option<f32>, color: Color32| {
            let mut run: Vec<Pos2> = Vec::new();
            for s in samples {
                match get(s) {
                    Some(v) => run.push(Pos2::new(x(s.t), y(v))),
                    None => {
                        if run.len() > 1 {
                            clip.add(Shape::line(std::mem::take(&mut run), Stroke::new(width, color)));
                        }
                        run.clear();
                    }
                }
            }
            if run.len() > 1 {
                clip.add(Shape::line(run, Stroke::new(width, color)));
            }
        };
        if ghost {
            line(&|s| s.ref_throttle, pal.throttle);
            line(&|s| s.ref_brake, pal.brake);
        } else {
            line(&|s| Some(s.throttle), pal.throttle);
            line(&|s| Some(s.brake), pal.brake);
            // Where the ABS was working, the brake line is amber over the top.
            line(&|s| s.abs.then_some(s.brake), th.attention);
        }
    }
    kit::outline(p, r, 4.0, 1.0, th.line);
}

/// Draws in the wheel's 100×100 SVG space, scaled, centred and turned.
struct Vg<'a> {
    p: &'a Painter,
    c: Pos2,
    k: f32,
    cos: f32,
    sin: f32,
}

impl Vg<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let (dx, dy) = ((x - 50.0) * self.k, (y - 50.0) * self.k);
        Pos2::new(self.c.x + dx * self.cos - dy * self.sin, self.c.y + dx * self.sin + dy * self.cos)
    }
    /// A path of line and quadratic segments: `Seg::L(x, y)` / `Seg::Q(cx, cy, x, y)`.
    fn path(&self, start: (f32, f32), segs: &[PathSeg]) -> Vec<Pos2> {
        let mut pts = vec![self.at(start.0, start.1)];
        let mut cur = start;
        for s in segs {
            match *s {
                PathSeg::L(x, y) => {
                    pts.push(self.at(x, y));
                    cur = (x, y);
                }
                PathSeg::Q(cx, cy, x, y) => {
                    for i in 1..=10 {
                        let t = i as f32 / 10.0;
                        let u = 1.0 - t;
                        let px = u * u * cur.0 + 2.0 * u * t * cx + t * t * x;
                        let py = u * u * cur.1 + 2.0 * u * t * cy + t * t * y;
                        pts.push(self.at(px, py));
                    }
                    cur = (x, y);
                }
            }
        }
        pts
    }
    fn stroke(&self, pts: Vec<Pos2>, w: f32, color: Color32, round: bool) {
        let w = w * self.k;
        if round {
            if let (Some(a), Some(b)) = (pts.first(), pts.last()) {
                self.p.circle_filled(*a, w / 2.0, color);
                self.p.circle_filled(*b, w / 2.0, color);
            }
        }
        self.p.add(Shape::line(pts, Stroke::new(w, color)));
    }
    fn closed(&self, pts: Vec<Pos2>, w: f32, color: Color32) {
        self.p.add(Shape::closed_line(pts, Stroke::new(w * self.k, color)));
    }
    fn fill(&self, pts: &[Pos2], color: Color32) {
        kit::fan(self.p, pts, color);
    }
    fn circle(&self, x: f32, y: f32, r: f32, fill: Color32) {
        self.p.circle_filled(self.at(x, y), r * self.k, fill);
    }
    fn ring(&self, x: f32, y: f32, r: f32, w: f32, color: Color32) {
        self.p.circle_stroke(self.at(x, y), r * self.k, Stroke::new(w * self.k, color));
    }
    /// A rounded rectangle as a polygon.
    fn rrect(&self, x: f32, y: f32, w: f32, h: f32, rx: f32) -> Vec<Pos2> {
        let mut pts = Vec::new();
        let corners = [(x + w - rx, y + rx, -90.0f32), (x + w - rx, y + h - rx, 0.0), (x + rx, y + h - rx, 90.0), (x + rx, y + rx, 180.0)];
        for (cx, cy, a0) in corners {
            for i in 0..=4 {
                let a = (a0 + i as f32 * 22.5).to_radians();
                pts.push(self.at(cx + rx * a.cos(), cy + rx * a.sin()));
            }
        }
        pts
    }
    /// An arc of a circle round the centre, from `a0` to `a1` degrees (0 = up, clockwise).
    fn arc(&self, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
        (0..=8)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / 8.0).to_radians();
                self.at(50.0 + r * a.sin(), 50.0 - r * a.cos())
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
enum PathSeg {
    L(f32, f32),
    Q(f32, f32, f32, f32),
}
use PathSeg::{L, Q};

/// A wheel that turns with yours; the red mark is 12 o'clock at centre.
fn wheel(p: &Painter, th: &Theme, pal: &Palette, c: Pos2, size: f32, steering: f32, style: WheelStyle, leds: f32) {
    // iRacing: radians, positive = turned left (anticlockwise on screen).
    let a = -steering;
    let v = Vg { p, c, k: size / 100.0, cos: a.cos(), sin: a.sin() };
    let w = pal.wheel;
    let dark = th.panel_solid;
    let button = alpha(dark, 0.75);
    let screen_edge = mix(w, dark, 0.5);
    let shift_lights = |y: f32, x0: f32, x1: f32, count: usize| {
        let lit = (leds * count as f32).round() as usize;
        let flash = leds >= 1.0;
        for i in 0..count {
            let x = x0 + (x1 - x0) * (i as f32 + 0.5) / count as f32;
            let kind = if flash {
                th.loss
            } else if (i as f32) < count as f32 * 0.4 {
                th.gain
            } else if (i as f32) < count as f32 * 0.75 {
                th.attention
            } else {
                th.loss
            };
            let color = if i < lit { kind } else { alpha(dark, 0.85) };
            v.circle(x, y, 2.1, color);
        }
    };
    match style {
        WheelStyle::Round => {
            v.ring(50.0, 50.0, 41.0, 8.0, w);
            v.stroke(v.path((10.0, 52.0), &[Q(30.0, 46.0, 40.0, 50.0), L(60.0, 50.0), Q(70.0, 46.0, 90.0, 52.0)]), 7.0, w, false);
            let spoke = [v.at(44.0, 55.0), v.at(47.0, 89.0), v.at(53.0, 89.0), v.at(56.0, 55.0)];
            v.fill(&spoke, w);
            v.closed(spoke.to_vec(), 2.0, w);
            v.fill(&v.rrect(37.0, 44.0, 26.0, 16.0, 5.0), w);
            let half = (4.5f32 / 41.0).asin().to_degrees();
            v.stroke(v.arc(41.0, -half, half), 9.0, pal.mark, false);
        }
        WheelStyle::Gt => {
            let rim = v.path((28.0, 15.0), &[L(72.0, 15.0), Q(91.0, 17.0, 91.0, 50.0), Q(91.0, 81.0, 74.0, 86.0), L(26.0, 86.0), Q(9.0, 81.0, 9.0, 50.0), Q(9.0, 17.0, 28.0, 15.0)]);
            v.closed(rim, 8.0, w);
            v.stroke(v.path((13.0, 34.0), &[Q(9.0, 50.0, 13.0, 68.0)]), 13.0, w, true);
            v.stroke(v.path((87.0, 34.0), &[Q(91.0, 50.0, 87.0, 68.0)]), 13.0, w, true);
            let plate: Vec<Pos2> = [(13.0, 44.0), (30.0, 38.0), (70.0, 38.0), (87.0, 44.0), (87.0, 58.0), (66.0, 72.0), (58.0, 86.0), (42.0, 86.0), (34.0, 72.0), (13.0, 58.0)]
                .iter()
                .map(|&(x, y)| v.at(x, y))
                .collect();
            v.fill(&plate, pal.plate);
            v.closed(plate, 2.0, w);
            let screen = v.rrect(40.0, 43.0, 20.0, 11.0, 1.5);
            v.fill(&screen, dark);
            v.closed(screen, 1.0, screen_edge);
            for x in [30.0, 70.0] {
                for y in [48.0, 58.0] {
                    v.circle(x, y, 2.6, button);
                }
            }
            v.circle(44.0, 64.0, 2.2, button);
            v.circle(56.0, 64.0, 2.2, button);
            shift_lights(33.0, 34.0, 66.0, 8);
            v.stroke(vec![v.at(46.0, 15.0), v.at(54.0, 15.0)], 5.0, pal.mark, true);
        }
        WheelStyle::Formula => {
            v.fill(&v.rrect(5.0, 28.0, 15.0, 46.0, 7.0), w);
            v.fill(&v.rrect(80.0, 28.0, 15.0, 46.0, 7.0), w);
            let plate = v.path(
                (17.0, 30.0),
                &[
                    Q(17.0, 24.0, 24.0, 24.0),
                    L(76.0, 24.0),
                    Q(83.0, 24.0, 83.0, 30.0),
                    L(83.0, 64.0),
                    Q(83.0, 72.0, 74.0, 76.0),
                    L(60.0, 80.0),
                    L(40.0, 80.0),
                    L(26.0, 76.0),
                    Q(17.0, 72.0, 17.0, 64.0),
                ],
            );
            v.fill(&plate, pal.plate);
            v.closed(plate, 2.0, w);
            let screen = v.rrect(34.0, 36.0, 32.0, 18.0, 2.0);
            v.fill(&screen, dark);
            v.closed(screen, 1.0, screen_edge);
            shift_lights(30.0, 26.0, 74.0, 10);
            for (x, y, r) in [(27.0, 44.0, 3.6), (73.0, 44.0, 3.6), (40.0, 66.0, 3.2), (60.0, 66.0, 3.2), (27.0, 58.0, 2.4), (73.0, 58.0, 2.4)] {
                v.circle(x, y, r, button);
            }
            v.stroke(vec![v.at(46.0, 24.0), v.at(54.0, 24.0)], 5.0, pal.mark, true);
        }
        WheelStyle::Oval => {
            v.ring(50.0, 50.0, 43.0, 5.0, w);
            v.stroke(v.path((9.0, 58.0), &[Q(30.0, 60.0, 42.0, 64.0)]), 7.0, w, false);
            v.stroke(v.path((91.0, 58.0), &[Q(70.0, 60.0, 58.0, 64.0)]), 7.0, w, false);
            v.circle(50.0, 64.0, 9.0, w);
            let half = (4.5f32 / 43.0).asin().to_degrees();
            v.stroke(v.arc(43.0, -half, half), 9.0, pal.mark, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_from_car_name() {
        assert_eq!(resolve_wheel("auto", "Ferrari 296 GT3"), WheelStyle::Gt);
        assert_eq!(resolve_wheel("auto", "Dallara IR18"), WheelStyle::Formula);
        assert_eq!(resolve_wheel("auto", "NASCAR Cup Series Next Gen Chevrolet Camaro ZL1"), WheelStyle::Oval);
        assert_eq!(resolve_wheel("auto", "Porsche 911 GT3 Cup (992)"), WheelStyle::Gt);
        assert_eq!(resolve_wheel("auto", "Toyota GR86"), WheelStyle::Round);
        assert_eq!(resolve_wheel("auto", "Mazda MX-5 Cup"), WheelStyle::Round);
        assert_eq!(resolve_wheel("oval", "Ferrari 296 GT3"), WheelStyle::Oval);
        assert_eq!(resolve_wheel("auto", "Audi R8 LMS"), WheelStyle::Gt);
    }
}
