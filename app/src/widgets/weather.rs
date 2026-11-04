//! Weather (`src/widgets/Weather.tsx`): track and air temperature, how wet
//! the track is, rain, humidity and fog, and a wind compass. In "car" mode
//! the compass card turns under the car (which always points up) so the
//! arrow shows where the wind hits you from right now; in "north" mode the
//! card stays put and the car turns. Small arrows show which way the
//! temperatures, wetness and rain have moved over the last few minutes.

use egui::{Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use super::{Ctx, Widget};
use crate::data::Live;
use crate::kit::{self, seg};
use crate::logic::format::{fixed, temp, temp_unit};
use crate::logic::weather::{add_sample, trend3, Sample, EVERY_MS, KEEP_MS, SKIES, WETNESS};
use crate::theme::{alpha, Theme};

#[derive(Default)]
pub struct Weather {
    track: Vec<Sample>,
    air: Vec<Sample>,
    wet: Vec<Sample>,
    rain: Vec<Sample>,
}

const WIDTH: f32 = 300.0;
const LINE: f32 = 1.2;
const SIZE: f32 = 168.0;
const C: f32 = SIZE / 2.0;
const RING: f32 = C - 14.0;

impl Widget for Weather {
    /// Samples every 15 s of sim time, so trends pause with the sim and a
    /// new session starts afresh.
    fn observe(&mut self, live: &Live, _now: f64) {
        let (Some(field), Some(tick)) = (live.field.as_deref(), live.tick.as_deref()) else { return };
        let clock = &field.clock;
        let at = tick.session_time * 1000.0;
        add_sample(&mut self.track, at, clock.track_temp as f64, EVERY_MS, KEEP_MS);
        add_sample(&mut self.air, at, clock.air_temp as f64, EVERY_MS, KEEP_MS);
        if let Some(w) = clock.track_wetness.filter(|w| *w != 0) {
            add_sample(&mut self.wet, at, w as f64, EVERY_MS, KEEP_MS);
        }
        if let Some(r) = clock.precipitation {
            add_sample(&mut self.rain, at, r as f64, EVERY_MS, KEEP_MS);
        }
    }

    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let units = cx.units;
        let clock = cx.live.field.as_deref().map(|f| &f.clock);
        let heading = cx.live.tick.as_deref().and_then(|t| t.heading);
        let show_trends = o.b("showTrends");
        let shown = |t: i32| if show_trends { t } else { 0 };

        let sky = clock.and_then(|c| c.skies).and_then(|s| SKIES.get(s as usize).copied());
        let wetness = clock.and_then(|c| c.track_wetness).unwrap_or(0).max(0);
        let rain = clock.and_then(|c| c.precipitation);
        let declared_wet = clock.is_some_and(|c| c.declared_wet);

        // The header: the sky, with a tag after it when the session is declared wet.
        let tag_font = th.bold(12.0);
        let tag_w = kit::width(p, "Wet", tag_font.clone()) + 8.0;
        let mut aside = Vec::new();
        if let Some(s) = sky {
            aside.push(seg(s, kit::aside_font(th), th.ink2));
        }
        if declared_wet {
            // Room for the tag: blank text as wide as it and its 6px margin.
            let space = kit::width(p, "  ", kit::aside_font(th)) / 2.0;
            aside.push(seg(" ".repeat(((tag_w + 6.0) / space).ceil() as usize), kit::aside_font(th), Color32::TRANSPARENT));
        }
        let panel = kit::Panel::begin(p, th, WIDTH, Some("Weather"), &aside);
        if declared_wet {
            let r = Rect::from_center_size(Pos2::new(WIDTH - panel.inset - 10.0 - tag_w / 2.0, panel.inset + 13.0), Vec2::new(tag_w, 12.0 * LINE));
            tag(p, th, r, "Wet");
        }
        let x = panel.x();
        let mut y = panel.body_top;

        // Track and air temperature, side by side.
        let top = y + 8.0;
        let col_w = (x.span() - 24.0 - 10.0) / 2.0;
        let readings = [("Track", clock.map(|c| c.track_temp), shown(trend3(&self.track, 0.5))), ("Air", clock.map(|c| c.air_temp), shown(trend3(&self.air, 0.5)))];
        for (k, (label, value, t)) in readings.iter().enumerate() {
            let rx = x.min + 12.0 + k as f32 * (col_w + 10.0);
            let sp = 12.0 * 0.05;
            kit::text_sp(p, Pos2::new(rx, top + 12.0 * LINE / 2.0), Align2::LEFT_CENTER, &label.to_uppercase(), th.semi(12.0), th.ink3, sp);
            let base = top + 12.0 * LINE + 25.0;
            let text = value.map(|v| fixed(temp(v, units) as f64, 1)).unwrap_or_else(|| "–".into());
            let r = kit::segs_baseline(p, rx, base, &[seg(text, th.bold(28.0), th.ink)]);
            let r2 = kit::segs_baseline(p, r.max + 3.0, base, &[seg(temp_unit(units), th.medium(14.0), th.ink3)]);
            trend_arrow(p, th, r2.max + 6.0, base - 10.0, *t, true);
        }
        y = top + 12.0 * LINE + 28.0 * 1.05 + 6.0;

        // Track wetness: a word and seven steps.
        y = row_rule(p, th, x, y);
        let wet_word = WETNESS.get(wetness as usize).copied().filter(|w| !w.is_empty()).unwrap_or("–");
        y = meter_head(p, th, x, y, "Track", wet_word, wetness >= 3, shown(trend3(&self.wet, 0.5)));
        {
            let gap = 3.0;
            let w = (x.span() - 24.0 - 6.0 * gap) / 7.0;
            for k in 1..=7 {
                let r = Rect::from_min_size(Pos2::new(x.min + 12.0 + (k - 1) as f32 * (w + gap), y + 5.0), Vec2::new(w, 6.0));
                let color = if k <= wetness { if k >= 3 { th.lapped } else { th.attention } } else { alpha(th.ink, 0.09) };
                kit::fill(p, r, 2.0, color);
            }
            y += 5.0 + 6.0 + 6.0;
        }

        // Rain.
        y = row_rule(p, th, x, y);
        let rain_word = rain.map(|r| format!("{}%", (r as f64 * 100.0 + 0.5).floor() as i32)).unwrap_or_else(|| "Not reported".into());
        y = meter_head(p, th, x, y, "Rain", &rain_word, rain.is_some_and(|r| r >= 0.05), shown(trend3(&self.rain, 0.03)));
        if let Some(r) = rain {
            let bar = Rect::from_min_size(Pos2::new(x.min + 12.0, y + 5.0), Vec2::new(x.span() - 24.0, 6.0));
            kit::fill(p, bar, 3.0, alpha(th.ink, 0.09));
            let w = bar.width() * r.clamp(0.0, 1.0);
            if w > 0.0 {
                let clip = p.with_clip_rect(Rect::from_min_size(bar.min, Vec2::new(w, 6.0)));
                kit::fill(&clip, bar, 3.0, th.lapped);
            }
            y += 5.0 + 6.0;
        }
        y += 6.0;

        // Humidity and fog, when reported.
        let mut grid: Vec<(&str, String)> = Vec::new();
        if let Some(h) = clock.and_then(|c| c.humidity) {
            grid.push(("Humidity", format!("{}%", (h as f64 * 100.0 + 0.5).floor() as i32)));
        }
        if let Some(f) = clock.and_then(|c| c.fog_level).filter(|f| *f > 0.005) {
            grid.push(("Fog", format!("{}%", (f as f64 * 100.0 + 0.5).floor() as i32)));
        }
        if !grid.is_empty() {
            y = row_rule(p, th, x, y);
            y += 6.0;
            for (k, (label, value)) in grid.iter().enumerate() {
                if k > 0 {
                    y += 2.0;
                }
                let h = 15.0 * LINE;
                kit::text(p, Pos2::new(x.min + 12.0, y + h / 2.0), Align2::LEFT_CENTER, label, th.medium(15.0), th.ink2);
                kit::text(p, Pos2::new(x.max - 12.0, y + h / 2.0), Align2::RIGHT_CENTER, value, th.semi(15.0), th.ink);
                y += h;
            }
            y += 6.0;
        }

        if o.b("showCompass") {
            y = row_rule(p, th, x, y);
            let speed = clock.and_then(|c| c.wind_speed);
            let dir = clock.and_then(|c| c.wind_dir);
            y = wind(p, th, x.min, y, speed, dir, heading, o.s("compass") != "north", units);
        }
        panel.finish(p, th, y)
    }
}

/// The 1px line above a section; returns the y below it.
fn row_rule(p: &Painter, th: &Theme, x: egui::Rangef, y: f32) -> f32 {
    p.hline(x, y + 0.5, Stroke::new(1.0, th.line));
    y + 1.0
}

/// A small badge: `.tag`.
fn tag(p: &Painter, th: &Theme, r: Rect, label: &str) {
    kit::fill(p, r, if th.broadcast() { 2.0 } else { 3.0 }, th.attention);
    kit::text(p, r.center(), Align2::CENTER_CENTER, label, th.bold(12.0), th.panel_solid);
}

/// ▲ rising (attention) or ▼ falling (the wet colour), `x` its left edge and
/// `y` its centre; nothing when steady. Returns the width used.
fn trend_arrow(p: &Painter, th: &Theme, x: f32, y: f32, t: i32, _big: bool) -> f32 {
    if t == 0 {
        return 0.0;
    }
    let (s, c) = if t > 0 { ("▲", th.attention) } else { ("▼", th.lapped) };
    kit::text(p, Pos2::new(x, y), Align2::LEFT_CENTER, s, th.medium(11.0), c).width()
}

/// "Track ........ Very lightly wet ▲": a label, the reading and its trend.
/// Returns the y below the line.
#[allow(clippy::too_many_arguments)]
fn meter_head(p: &Painter, th: &Theme, x: egui::Rangef, y: f32, label: &str, value: &str, wet: bool, t: i32) -> f32 {
    let top = y + 6.0;
    let h = 15.0 * LINE;
    let cy = top + h / 2.0;
    kit::text(p, Pos2::new(x.min + 12.0, cy), Align2::LEFT_CENTER, label, th.medium(15.0), th.ink2);
    let arrow_w = if t != 0 { 6.0 + kit::width(p, "▲", th.medium(11.0)) } else { 0.0 };
    let right = x.max - 12.0 - arrow_w;
    let r = kit::text(p, Pos2::new(right, cy), Align2::RIGHT_CENTER, value, th.semi(15.0), if wet { th.lapped } else { th.ink });
    trend_arrow(p, th, r.right() + 6.0, cy - 2.0, t, false);
    top + h
}

/// A rounded rectangle as a polygon, relative to the compass centre, turned
/// `turn` radians clockwise about it.
fn rounded_poly(c: Pos2, x: f32, y: f32, w: f32, h: f32, r: f32, turn: f32) -> Vec<Pos2> {
    let mut pts = Vec::new();
    let corners = [(x + w - r, y + r, -90.0f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)];
    for (cx, cy, start) in corners {
        for i in 0..=5 {
            let a = (start + 90.0 * i as f32 / 5.0).to_radians();
            pts.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts.into_iter().map(|(px, py)| rot(c, px, py, turn)).collect()
}

/// A point at offset (`px`, `py`) from `c`, turned `turn` radians clockwise on screen.
fn rot(c: Pos2, px: f32, py: f32, turn: f32) -> Pos2 {
    let (s, co) = turn.sin_cos();
    Pos2::new(c.x + px * co - py * s, c.y + px * s + py * co)
}

/// The wind compass and the speed beside it. Returns the y below.
#[allow(clippy::too_many_arguments)]
fn wind(p: &Painter, th: &Theme, left: f32, top: f32, speed: Option<f32>, dir: Option<f32>, heading: Option<f32>, turn_with_car: bool, units: crate::config::Units) -> f32 {
    let has_heading = heading.is_some();
    let h = heading.unwrap_or(0.0);
    // Rotation of the compass card and of the car, clockwise.
    let card = if turn_with_car && has_heading { -h } else { 0.0 };
    let car = if turn_with_car { 0.0 } else { h };
    let calm = speed.is_none_or(|s| s < 0.3);

    let c = Pos2::new(left + 4.0 + C, top + 4.0 + C);
    p.circle_filled(c, RING, alpha(th.well, 0.25));
    p.circle_stroke(c, RING, Stroke::new(1.0, th.line_strong));
    for i in 0..36 {
        let major = i % 9 == 0;
        let a = (i as f32 * 10.0).to_radians() + card;
        let r0 = RING - if major { 9.0 } else { 5.0 };
        let at = |r: f32| Pos2::new(c.x + a.sin() * r, c.y - a.cos() * r);
        let stroke = if major { Stroke::new(2.0, th.ink3) } else { Stroke::new(1.0, th.line_strong) };
        p.line_segment([at(r0), at(RING)], stroke);
    }
    // The letters turn with the card but stay upright.
    for (i, l) in ["N", "E", "S", "W"].iter().enumerate() {
        let a = i as f32 * std::f32::consts::FRAC_PI_2 + card;
        let at = Pos2::new(c.x + a.sin() * (RING + 8.0), c.y - a.cos() * (RING + 8.0));
        kit::text(p, at, Align2::CENTER_CENTER, l, th.bold(12.0), if *l == "N" { th.loss } else { th.ink2 });
    }
    if let (false, Some(dir)) = (calm, dir) {
        wind_arrow(p, c, dir + card, th.lapped);
    }
    car_icon(p, th, c, car, if has_heading { 1.0 } else { 0.4 });

    // How hard it blows; the compass shows the direction.
    let text_left = left + 4.0 + SIZE + 8.0;
    let text_right = left + WIDTH - 1.0 - 10.0;
    let mid = (text_left + text_right) / 2.0;
    let mut segs = Vec::new();
    match speed {
        None => segs.push(seg("–", th.bold(48.0), th.ink)),
        Some(_) if calm => segs.push(seg("Calm", th.bold(48.0), th.ink)),
        Some(s) => {
            segs.push(seg(crate::logic::format::speed(s, units).to_string(), th.bold(48.0), th.ink));
            segs.push(seg(format!(" {}", crate::logic::format::speed_unit(units)), th.medium(16.0), th.ink3));
        }
    }
    let note = (!has_heading).then_some("No heading from iRacing: north is up");
    let lines = note.map(|n| wrap_lines(p, n, th.medium(13.0), text_right - text_left)).unwrap_or_default();
    let note_h = if lines.is_empty() { 0.0 } else { 6.0 + lines.len() as f32 * 13.0 * LINE };
    let block = 48.0 + 4.0 + 13.0 * LINE + note_h;
    let mut y = c.y - block / 2.0;
    let w = kit::segs_width(p, &segs, 0.0);
    kit::segs_baseline(p, mid - w / 2.0, y + 38.0, &segs);
    y += 48.0 + 4.0;
    let sp = 13.0 * 0.05;
    kit::text_sp(p, Pos2::new(mid, y + 13.0 * LINE / 2.0), Align2::CENTER_CENTER, &th.caps("WIND").to_uppercase(), th.semi(13.0), th.ink3, sp);
    y += 13.0 * LINE;
    for (k, l) in lines.iter().enumerate() {
        kit::text(p, Pos2::new(mid, y + 6.0 + (k as f32 + 0.5) * 13.0 * LINE), Align2::CENTER_CENTER, l, th.medium(13.0), th.ink3);
    }
    top + 4.0 + SIZE + 8.0
}

/// Greedy word wrap to `max_w`.
fn wrap_lines(p: &Painter, s: &str, font: egui::FontId, max_w: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(l) if kit::width(p, &format!("{l} {word}"), font.clone()) <= max_w => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// An arrow on the compass card that starts on the side the wind comes from
/// and points the way it blows, `turn` radians clockwise from "from the north".
fn wind_arrow(p: &Painter, c: Pos2, turn: f32, color: Color32) {
    let tail = -(RING - 3.0);
    let tip = -30.0;
    let at = |px: f32, py: f32| rot(c, px, py, turn);
    let width = 7.0;
    p.line_segment([at(0.0, tail), at(0.0, tip - 14.0)], Stroke::new(width, color));
    p.circle_filled(at(0.0, tip - 14.0), width / 2.0, color);
    let head = vec![at(0.0, tip), at(-12.0, tip - 19.0), at(12.0, tip - 19.0)];
    p.add(Shape::convex_polygon(head, color, Stroke::new(2.0, color)));
    p.circle_filled(at(0.0, tail), 5.5, color);
    p.circle_stroke(at(0.0, tail), 5.5, Stroke::new(2.0, color));
}

/// A small car seen from above, nose up when `turn` is 0.
fn car_icon(p: &Painter, th: &Theme, c: Pos2, turn: f32, opacity: f32) {
    let fade = |col: Color32| col.gamma_multiply(opacity);
    p.add(Shape::convex_polygon(rounded_poly(c, -9.0, -18.0, 18.0, 36.0, 6.0, turn), fade(th.ink2), Stroke::NONE));
    p.add(Shape::convex_polygon(rounded_poly(c, -6.5, -9.0, 13.0, 8.0, 2.0, turn), fade(th.panel_solid), Stroke::NONE));
    let nose = vec![rot(c, -5.0, -22.0, turn), rot(c, 0.0, -28.0, turn), rot(c, 5.0, -22.0, turn)];
    p.add(Shape::convex_polygon(nose, fade(th.attention), Stroke::NONE));
}
