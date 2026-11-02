//! Session (`src/widgets/Session.tsx`): track, field strength, conditions and
//! the event's limits in one panel.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use telemetry::model::SessionClock;

use super::relative::session_aside;
use super::{Ctx, Widget};
use crate::config::{Opts, Units};
use crate::kit::{self, seg, Seg};
use crate::logic::format::{fixed, irating, js_round};
use crate::logic::incidents;
use crate::theme::hex_or;

pub struct Session;

pub const SKIES: [&str; 4] = ["Clear", "Partly cloudy", "Mostly cloudy", "Overcast"];
/// irsdk_TrackWetness, 1..7
pub const WETNESS: [&str; 8] = ["", "Dry", "Mostly dry", "Very lightly wet", "Lightly wet", "Moderately wet", "Very wet", "Extremely wet"];
const COMPASS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

fn temp(c: f32, u: Units) -> String {
    format!("{}°", fixed(crate::logic::format::temp(c, u) as f64, 1))
}

pub fn wind(ms: f32, u: Units) -> String {
    if u == Units::Imperial {
        format!("{} mph", js_round(ms as f64 * 2.23694))
    } else {
        format!("{} km/h", js_round(ms as f64 * 3.6))
    }
}

/// Wind direction (radians) as a compass point.
pub fn compass(rad: f32) -> &'static str {
    let deg = ((rad.to_degrees() % 360.0) + 360.0) % 360.0;
    COMPASS[(js_round(deg as f64 / 45.0) as usize) % 8]
}

/// Seconds since midnight -> "14:05"
pub fn time_of_day(s: f32) -> String {
    let m = (s / 60.0).floor() as i64 % (24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// One `dt`/`dd` line: label left, value right.
struct Lines<'a> {
    p: &'a Painter,
    x: egui::Rangef,
    y: f32,
    tag_font: FontId,
}

const LINE_H: f32 = 17.0;

impl Lines<'_> {
    fn line(&mut self, label: &str, label_font: FontId, label_color: Color32, value: &[Seg], class_bar: Option<Color32>) {
        self.tagged(label, label_font, label_color, value, class_bar, None)
    }

    /// With a small tag (like "Wet") after the value.
    fn tagged(&mut self, label: &str, label_font: FontId, label_color: Color32, value: &[Seg], class_bar: Option<Color32>, tag: Option<(&str, Color32, Color32)>) {
        let cy = self.y + LINE_H / 2.0;
        let mut lx = self.x.min;
        if let Some(c) = class_bar {
            self.p.rect_filled(Rect::from_min_size(Pos2::new(lx, self.y), Vec2::new(2.0, LINE_H)), 0.0, c);
            lx += 8.0;
        }
        kit::text(self.p, Pos2::new(lx, cy), Align2::LEFT_CENTER, label, label_font, label_color);
        let mut right = self.x.max;
        if let Some((text, bg, fg)) = tag {
            let font = self.tag_font.clone();
            let w = kit::width(self.p, text, font.clone()) + 8.0;
            let r = Rect::from_min_size(Pos2::new(right - w, self.y + 1.0), Vec2::new(w, LINE_H - 2.0));
            kit::fill(self.p, r, 3.0, bg);
            kit::text(self.p, r.center(), Align2::CENTER_CENTER, text, font, fg);
            right = r.left() - 6.0;
        }
        kit::segs(self.p, right, cy, true, value, 0.0);
        self.y += LINE_H + 2.0;
    }
}

impl Widget for Session {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let units = cx.units;
        let session = cx.live.session.as_deref();
        let clock = cx.live.field.as_deref().map(|f| &f.clock);
        let title = clock
            .map(|c| c.session_type.as_str())
            .filter(|s| !s.is_empty())
            .or(session.map(|s| s.session_type.as_str()).filter(|s| !s.is_empty()))
            .unwrap_or("Session");
        let aside: Vec<Seg> = session_aside(clock).map(|a| vec![seg(a, kit::aside_font(th), th.ink2)]).unwrap_or_default();
        let panel = kit::Panel::begin(p, th, 300.0, Some(title), &aside);
        let x = panel.x();

        // Track and layout.
        let mut y = panel.body_top + 8.0;
        let name = session.map(|s| s.track_name.as_str()).filter(|s| !s.is_empty()).unwrap_or("–");
        let r = kit::text_fit(p, Pos2::new(x.min + 10.0, y), Align2::LEFT_TOP, name, th.bold(16.0), th.ink, x.span() - 20.0, 0.0);
        y = r.bottom();
        let length = session.filter(|s| s.track_length_km > 0.0).map(|s| {
            if units == Units::Imperial {
                format!("{} mi", fixed(s.track_length_km as f64 * 0.621371, 2))
            } else {
                format!("{} km", fixed(s.track_length_km as f64, 2))
            }
        });
        let sub = [session.map(|s| s.track_config.clone()).filter(|s| !s.is_empty()), length].into_iter().flatten().collect::<Vec<_>>().join("   ");
        let r = kit::text(p, Pos2::new(x.min + 10.0, y), Align2::LEFT_TOP, if sub.is_empty() { " " } else { &sub }, th.medium(13.0), th.ink3);
        y = r.bottom().max(y + 15.0) + 6.0;
        p.hline(x, y - 0.5, Stroke::new(1.0, th.line));

        let mut l = Lines { p, x: egui::Rangef::new(x.min + 10.0, x.max - 10.0), y: y + 6.0, tag_font: th.bold(12.0) };
        let dt = th.medium(14.0);
        let dd = th.semi(14.0);
        let classes = clock.map(|c| c.classes.as_slice()).unwrap_or(&[]);
        let cars: i32 = classes.iter().map(|c| c.cars).sum();
        let mut field = if cars > 0 { format!("{cars} cars") } else { "–".into() };
        if let Some(s) = clock.and_then(|c| c.strength_of_field).filter(|s| *s > 0) {
            field += &format!("   SoF {}", irating(s));
        }
        l.line("Field", dt.clone(), th.ink2, &[seg(field, dd.clone(), th.ink)], None);
        if o.b("showClasses") && classes.len() > 1 {
            for c in classes {
                let color = hex_or(&c.color, th.ink);
                let v = format!("{} cars{}", c.cars, c.strength_of_field.filter(|s| *s > 0).map(|s| format!("   {}", irating(s))).unwrap_or_default());
                let name = if c.name.is_empty() { "Class" } else { &c.name };
                l.line(name, th.bold(13.0), color, &[seg(v, th.medium(13.0), th.ink2)], Some(color));
            }
        }
        if o.b("showConditions") {
            conditions(&mut l, cx, clock, &dt, &dd);
        }
        if o.b("showLimits") {
            if let Some(c) = clock {
                limits(&mut l, cx, c, &dt, &dd);
            }
        }
        let bottom = l.y - 2.0 + 8.0;
        panel.finish(p, th, bottom)
    }
}

fn conditions(l: &mut Lines, cx: &Ctx, c: Option<&SessionClock>, dt: &FontId, dd: &FontId) {
    let th = cx.th;
    let units = cx.units;
    if let Some(t) = c.and_then(|c| c.time_of_day) {
        l.line("Time of day", dt.clone(), th.ink2, &[seg(time_of_day(t), dd.clone(), th.ink)], None);
    }
    let temps = c.map(|c| format!("{} / {}", temp(c.air_temp, units), temp(c.track_temp, units))).unwrap_or("–".into());
    l.line("Air / track", dt.clone(), th.ink2, &[seg(temps, dd.clone(), th.ink)], None);
    let sky = c.and_then(|c| c.skies).and_then(|s| SKIES.get(s as usize)).copied();
    let wet = c.and_then(|c| c.track_wetness).filter(|w| *w > 0).and_then(|w| WETNESS.get(w as usize)).copied();
    if sky.is_some() || wet.is_some() {
        let is_wet = c.and_then(|c| c.track_wetness).unwrap_or(0) >= 3;
        let text = [sky, wet].into_iter().flatten().collect::<Vec<_>>().join(" / ");
        let color = if is_wet { th.attention } else { th.ink };
        let v = [seg(text, dd.clone(), color)];
        let tag = c.is_some_and(|c| c.declared_wet).then_some(("Wet", th.attention, th.panel_solid));
        l.tagged("Sky / surface", dt.clone(), th.ink2, &v, None, tag);
    }
    if let Some(ws) = c.and_then(|c| c.wind_speed) {
        let c = c.unwrap();
        let mut v = wind(ws, units);
        if let Some(d) = c.wind_dir {
            v += &format!(" {}", compass(d));
        }
        if let Some(h) = c.humidity {
            v += &format!("   {}% hum.", js_round(h as f64 * 100.0));
        }
        l.line("Wind", dt.clone(), th.ink2, &[seg(v, dd.clone(), th.ink)], None);
    }
}

fn limits(l: &mut Lines, cx: &Ctx, c: &SessionClock, dt: &FontId, dd: &FontId) {
    let th = cx.th;
    // The step after the first limit is set on the Relative.
    let step = Opts::new(cx.cfg, "relative").n("incidentStep").round() as i32;
    let t = incidents::target(c.incidents, c.incident_limit, step);
    let near = t.next.is_some_and(|n| t.passed > 0 || c.incidents as f32 >= n as f32 * 0.75);
    let v = match t.next {
        Some(n) => format!("{}x / {n}x", c.incidents),
        None => format!("{}x", c.incidents),
    };
    l.line("Incidents", dt.clone(), th.ink2, &[seg(v, dd.clone(), if near { th.attention } else { th.ink })], None);
    if let Some(limit) = c.fast_repairs_limit {
        l.line("Fast repairs", dt.clone(), th.ink2, &[seg(format!("{} left", (limit - c.fast_repairs_used).max(0)), dd.clone(), th.ink)], None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts() {
        assert_eq!(time_of_day(14.0 * 3600.0 + 5.0 * 60.0 + 59.0), "14:05");
        assert_eq!(compass(0.0), "N");
        assert_eq!(compass(std::f32::consts::FRAC_PI_2), "E");
        assert_eq!(compass(-std::f32::consts::FRAC_PI_4), "NW");
        assert_eq!(wind(4.17, Units::Metric), "15 km/h");
        assert_eq!(wind(4.17, Units::Imperial), "9 mph");
        assert_eq!(temp(22.8, Units::Metric), "22.8°");
    }
}
