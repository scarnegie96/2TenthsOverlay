//! Delta (`src/widgets/Delta.tsx`): your lap against a comparison lap. Your
//! lap is the flat baseline; the comparison's line rises above it where the
//! comparison is faster (red) and dips below where it's slower (green).
//! Underneath, each sector's gain or loss; at the side, the sector you're in.

use egui::epaint::Mesh;
use egui::{Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use super::{Ctx, Widget};
use crate::kit;
use crate::logic::delta_history::{DeltaHistory, Mode, BINS};
use crate::logic::format::{lap_time, signed, speed, speed_unit};
use crate::theme::{alpha, mix, Theme};

const GRAPH_W: f32 = 640.0;
const GRAPH_H: f32 = 92.0;
/// Space under the plot for sector numbers.
const LABEL_H: f32 = 14.0;
const SIDE_W: f32 = 190.0;

pub struct Delta {
    history: DeltaHistory,
    last_time: f64,
    /// The graph's ± scale, eased towards what's on screen.
    range: f32,
}

impl Default for Delta {
    fn default() -> Self {
        Self { history: DeltaHistory::default(), last_time: f64::NAN, range: 0.5 }
    }
}

/// Sector layout: iRacing's track sectors, or equal mini sectors.
fn sector_starts(mode: &str, mini: f64, track: Option<&[f32]>) -> Vec<f32> {
    if let Some(t) = track.filter(|t| mode == "track" && t.len() >= 2) {
        return t.to_vec();
    }
    let n = (mini.round() as usize).max(2);
    (0..n).map(|i| i as f32 / n as f32).collect()
}

fn tone(th: &Theme, v: Option<f32>) -> Option<Color32> {
    match v {
        Some(v) if v > 0.005 => Some(th.loss),
        Some(v) if v < -0.005 => Some(th.gain),
        _ => None,
    }
}

impl Widget for Delta {
    fn observe(&mut self, live: &crate::data::Live, _now: f64) {
        let Some(t) = live.tick.as_deref() else { return };
        if t.session_time == self.last_time {
            return;
        }
        self.last_time = t.session_time;
        let session = live.session.as_ref().map(|s| format!("{}|{}", s.track_name, s.session_type));
        self.history.push(t, session.as_deref());
    }

    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let mode = Mode::parse(o.s("compareTo"));
        let starts = sector_starts(o.s("sectors"), o.n("miniSectors"), cx.live.session.as_ref().map(|s| s.sectors.as_slice()));
        self.history.set_sectors(&starts);
        let h = self.history.mode(mode);
        let now = h.now;
        let tick = cx.live.tick.as_deref();
        let reference = cx.live.reference.as_deref();
        let total_color = tone(th, now).unwrap_or(th.ink2);
        let total = now.map(|n| signed(n as f64, 2)).unwrap_or("–.––".into());
        let unavailable = match (mode, reference) {
            (Mode::Reference, r) if !r.is_some_and(|r| r.loaded) => Some("Import a lap in settings to compare against it.".to_string()),
            (Mode::Reference, Some(r)) if !r.matches_track => Some(format!(
                "Your reference lap is for {}.",
                [r.track_name.as_str(), r.track_config.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ")
            )),
            _ if now.is_none() => Some(format!("No {} to compare against yet.", mode.label().to_lowercase())),
            _ => None,
        };

        if o.b("simple") {
            let panel = kit::Panel::begin(p, th, 300.0, None, &[]);
            let cxm = 150.0;
            let mut y = panel.body_top + 8.0;
            let chip = mode_chip(p, th, Pos2::new(cxm, y), true, mode);
            y = chip.bottom() + 4.0;
            let r = kit::text(p, Pos2::new(cxm, y), Align2::CENTER_TOP, &total, th.bold(56.0), total_color);
            y = r.bottom();
            if let Some(note) = &unavailable {
                y += 4.0;
                let g = p.layout(note.clone(), th.medium(12.0), th.ink3, 280.0);
                let gs = g.size();
                p.galley(Pos2::new(cxm - gs.x / 2.0, y), g, th.ink3);
                y += gs.y;
            }
            y += 10.0;
            if o.b("showLapTimes") {
                y = lap_times(p, th, cx, panel.x(), y, mode, true);
            }
            return panel.finish(p, th, y);
        }

        let w = 1.0 + 10.0 + GRAPH_W + 10.0 + SIDE_W + 1.0;
        let panel = kit::Panel::begin(p, th, w, None, &[]);
        let left = panel.inset + 10.0;
        let top = panel.body_top + 8.0;

        // Head: what it's against, the comparison's time, this lap so far, the gap now.
        let hy = top + 15.0;
        let chip = mode_chip(p, th, Pos2::new(left, hy), false, mode);
        let mut x = chip.right() + 12.0;
        let comparison = match mode {
            Mode::Reference => reference.filter(|r| r.loaded).map(|r| r.lap_time),
            Mode::Last => tick.and_then(|t| t.lap_last),
            Mode::SessionBest => tick.and_then(|t| t.lap_best),
            _ => None,
        };
        if let Some(c) = comparison.filter(|c| *c > 0.0) {
            let r = kit::text(p, Pos2::new(x, hy + 3.0), Align2::LEFT_CENTER, &lap_time(Some(c)), th.semi(15.0), th.ink2);
            x = r.right() + 12.0;
        }
        kit::text(p, Pos2::new(x, hy), Align2::LEFT_CENTER, &lap_time(tick.map(|t| t.lap_current)), th.bold(26.0), th.ink);
        kit::text(p, Pos2::new(left + GRAPH_W, hy), Align2::RIGHT_CENTER, &total, th.bold(28.0), total_color);

        // The graph.
        let g = Rect::from_min_size(Pos2::new(left, top + 30.0 + 6.0), Vec2::new(GRAPH_W, GRAPH_H));
        self.graph(p, th, g, mode, o.b("showPrevious"));
        if let Some(note) = &unavailable {
            let r = Rect::from_min_max(g.min, Pos2::new(g.right(), g.bottom() - LABEL_H));
            kit::fill(p, r, 4.0, alpha(th.panel_solid, 0.55));
            kit::text(p, r.center(), Align2::CENTER_CENTER, note, th.medium(14.0), th.ink2);
        }

        // Each sector's gain or loss.
        let sy = g.bottom() + 6.0;
        let h = self.history.mode(mode);
        let sectors = &self.history.sectors;
        let n = sectors.len();
        let free = GRAPH_W - 2.0 * (n as f32 - 1.0);
        let mut sx = left;
        let live = self.history.live_sector(mode);
        for i in 0..n {
            let end = sectors.get(i + 1).copied().unwrap_or(1.0);
            let sw = ((end - sectors[i]) * free).max(3.0);
            let r = Rect::from_min_size(Pos2::new(sx, sy), Vec2::new(sw, 24.0));
            sx += sw + 2.0;
            let current = i == self.history.current_sector;
            let v = if current { live } else { h.sectors[i] };
            let t = tone(th, v);
            let (bg, fg) = if current {
                (alpha(th.ink, 0.08), t.unwrap_or(th.ink))
            } else if v.is_some() {
                let bg = match t {
                    Some(c) if c == th.gain => mix(th.gain, th.panel_solid, 0.62),
                    Some(_) => mix(th.loss, th.panel_solid, 0.58),
                    None => alpha(th.ink, 0.18),
                };
                (bg, th.ink)
            } else {
                (alpha(th.ink, 0.06), th.ink)
            };
            kit::fill(p, r, 3.0, bg);
            if current {
                kit::outline(p, r, 3.0, 1.5, th.ink2);
            }
            if let (Some(v), true) = (v, (end - sectors[i]) * GRAPH_W >= 38.0) {
                kit::text(p, r.center(), Align2::CENTER_CENTER, &signed(v as f64, 2), th.bold(13.0), fg);
            }
        }
        let main_bottom = sy + 24.0 + 10.0;

        // The side: sector number, this sector, the last one, and the speed against the reference.
        let side = Rect::from_min_max(Pos2::new(left + GRAPH_W + 10.0, panel.body_top), Pos2::new(w - panel.inset, main_bottom));
        kit::fill(p, side, 0.0, alpha(th.ink, 0.03));
        p.vline(side.left() + 0.5, side.y_range(), Stroke::new(1.0, th.line));
        let inner = side.shrink2(Vec2::new(12.0, 10.0));
        kit::text(p, inner.left_top(), Align2::LEFT_TOP, "Sector", th.semi(14.0), th.ink2);
        let rows = if mode == Mode::Reference { 2.0 } else { 1.0 };
        let split_h = 42.0;
        let splits_top = inner.bottom() - rows * split_h - (rows - 1.0) * 4.0;
        let num_c = Pos2::new(inner.center().x, (inner.top() + 12.0 + splits_top - 6.0) / 2.0 + 4.0);
        kit::text(p, num_c, Align2::CENTER_CENTER, &(self.history.current_sector + 1).to_string(), th.bold(64.0), th.ink);
        let half = (inner.width() - 4.0) / 2.0;
        let (last_i, last_v) = self.history.last_sector(mode);
        split(p, th, Rect::from_min_size(Pos2::new(inner.left(), splits_top), Vec2::new(half, split_h)), "This sector", live.map(|v| format!("{}s", signed(v as f64, 2))), tone(th, live));
        split(
            p,
            th,
            Rect::from_min_size(Pos2::new(inner.left() + half + 4.0, splits_top), Vec2::new(half, split_h)),
            &format!("Sector {}", last_i + 1),
            last_v.map(|v| format!("{}s", signed(v as f64, 2))),
            tone(th, last_v),
        );
        if mode == Mode::Reference {
            let sd = tick.and_then(|t| t.coach.as_ref()).and_then(|c| c.speed_delta);
            let t = match sd {
                Some(v) if v < -0.3 => Some(th.loss),
                Some(v) if v > 0.3 => Some(th.gain),
                _ => None,
            };
            let text = sd.map(|v| format!("{}{} {}", if v >= 0.0 { "+" } else { "−" }, speed(v.abs(), cx.units), speed_unit(cx.units)));
            split(p, th, Rect::from_min_size(Pos2::new(inner.left(), splits_top + split_h + 4.0), Vec2::new(inner.width(), split_h)), "Speed vs reference", text, t);
        }

        let mut bottom = main_bottom;
        if o.b("showLapTimes") {
            bottom = lap_times(p, th, cx, panel.x(), bottom, mode, false);
        }
        panel.finish(p, th, bottom)
    }
}

/// "vs Best lap" in a chip; centred at `at` (simple) or starting there.
fn mode_chip(p: &Painter, th: &Theme, at: Pos2, centred: bool, mode: Mode) -> Rect {
    let text = format!("vs {}", mode.label());
    let w = kit::width(p, &text, th.semi(13.0)) + 16.0;
    let r = if centred { Rect::from_min_size(Pos2::new(at.x - w / 2.0, at.y), Vec2::new(w, 20.0)) } else { Rect::from_min_size(Pos2::new(at.x, at.y - 10.0), Vec2::new(w, 20.0)) };
    kit::fill(p, r, 4.0, alpha(th.ink, 0.10));
    kit::text(p, r.center(), Align2::CENTER_CENTER, &text, th.semi(13.0), th.ink2);
    r
}

fn split(p: &Painter, th: &Theme, r: Rect, label: &str, value: Option<String>, tone: Option<Color32>) {
    let bg = tone.map(|c| alpha(c, 0.22)).unwrap_or(alpha(th.ink, 0.06));
    kit::fill(p, r, 4.0, bg);
    kit::text(p, Pos2::new(r.center().x, r.top() + 4.0), Align2::CENTER_TOP, label, th.medium(11.0), th.ink3);
    kit::text(p, Pos2::new(r.center().x, r.bottom() - 5.0), Align2::CENTER_BOTTOM, value.as_deref().unwrap_or("–"), th.bold(16.0), tone.unwrap_or(th.ink));
}

/// Best, optimal and last along the bottom, the one compared against highlighted.
fn lap_times(p: &Painter, th: &Theme, cx: &Ctx, x: egui::Rangef, top: f32, mode: Mode, stacked: bool) -> f32 {
    let tick = cx.live.tick.as_deref();
    let best = tick.and_then(|t| t.lap_best);
    let optimal = tick.and_then(|t| t.lap_optimal);
    let last = tick.and_then(|t| t.lap_last);
    let last_is_best = matches!((last, best), (Some(l), Some(b)) if (l - b).abs() < 0.0005);
    let h = if stacked { 5.0 + 15.0 + 18.0 + 6.0 } else { 32.0 };
    p.hline(x, top + 0.5, Stroke::new(1.0, th.line));
    let cw = x.span() / 3.0;
    let items = [
        ("Best", best, matches!(mode, Mode::Best | Mode::SessionBest), false),
        ("Optimal", optimal, matches!(mode, Mode::Optimal | Mode::SessionOptimal), false),
        ("Last", last, mode == Mode::Last, last_is_best),
    ];
    for (i, (label, v, active, personal)) in items.into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x.min + cw * i as f32, top + 1.0), Vec2::new(cw, h));
        if i > 0 {
            p.vline(r.left() + 0.5, r.y_range(), Stroke::new(1.0, th.line));
        }
        if active {
            kit::fill(p, r, 0.0, alpha(th.ink, 0.07));
            kit::fill(p, Rect::from_min_size(r.min, Vec2::new(r.width(), 2.0)), 0.0, th.ink2);
        }
        let color = if personal {
            th.gain
        } else if active {
            th.ink
        } else {
            th.ink2
        };
        let value = lap_time(v);
        if stacked {
            kit::text(p, Pos2::new(r.center().x, r.top() + 5.0), Align2::CENTER_TOP, label, th.semi(12.0), th.ink3);
            kit::text(p, Pos2::new(r.center().x, r.bottom() - 6.0), Align2::CENTER_BOTTOM, &value, th.bold(15.0), color);
        } else {
            let lw = kit::width(p, label, th.semi(12.0));
            let vw = kit::width(p, &value, th.bold(17.0));
            let start = r.center().x - (lw + 8.0 + vw) / 2.0;
            let base = r.center().y + 1.0;
            kit::segs_baseline(p, start, base + 5.0, &[kit::seg(label, th.semi(12.0), th.ink3), kit::seg("  ", th.semi(12.0), th.ink3), kit::seg(value, th.bold(17.0), color)]);
        }
    }
    top + 1.0 + h
}

impl Delta {
    /// The lap graph.
    fn graph(&mut self, p: &Painter, th: &Theme, r: Rect, mode: Mode, show_previous: bool) {
        kit::fill(p, r, 4.0, alpha(th.well, 0.25));
        let clip = p.with_clip_rect(r);
        let p = &clip;
        let h = self.history.mode(mode);
        let plot_h = GRAPH_H - LABEL_H;
        let mid = r.top() + plot_h / 2.0;

        // Fit the scale to what's on screen, easing so it doesn't jump.
        let mut max_abs = h.now.unwrap_or(0.0).abs();
        for i in 0..BINS {
            if let Some(a) = h.current[i] {
                max_abs = max_abs.max(a.abs());
            }
            if let (true, Some(b)) = (show_previous, h.previous[i]) {
                max_abs = max_abs.max(b.abs());
            }
        }
        let target = ((max_abs * 1.2 * 4.0).ceil() / 4.0).max(0.25);
        self.range += (target - self.range) * 0.08;
        let range = self.range;
        let xb = |bin: usize| r.left() + (bin as f32 + 0.5) / BINS as f32 * GRAPH_W;
        let y = |d: f32| mid - (d / range) * (plot_h / 2.0 - 4.0);

        // Sector bands and numbers.
        let sectors = &self.history.sectors;
        for (i, &start) in sectors.iter().enumerate() {
            let end = sectors.get(i + 1).copied().unwrap_or(1.0);
            let (x0, x1) = (r.left() + start * GRAPH_W, r.left() + end * GRAPH_W);
            let band = Rect::from_min_max(Pos2::new(x0, r.top()), Pos2::new(x1, r.top() + plot_h));
            if i % 2 == 1 {
                p.rect_filled(band, 0.0, alpha(th.ink, 0.035));
            }
            let current = i == self.history.current_sector;
            if current {
                p.rect_filled(band, 0.0, alpha(th.ink, 0.05));
            }
            if x1 - x0 >= 12.0 {
                kit::text(p, Pos2::new((x0 + x1) / 2.0, r.bottom() - 2.0), Align2::CENTER_BOTTOM, &(i + 1).to_string(), th.semi(11.0), if current { th.ink } else { th.ink3 });
            }
        }

        // Last lap's line, dashed.
        if show_previous {
            for run in runs(&h.previous) {
                let pts: Vec<Pos2> = run.iter().map(|&(i, d)| Pos2::new(xb(i), y(d))).collect();
                p.extend(Shape::dashed_line(&pts, Stroke::new(1.25, alpha(th.ink, 0.4)), 3.0, 3.0));
            }
        }

        // This lap: filled to the baseline, red above, green below.
        for run in runs(&h.current) {
            let pts: Vec<Pos2> = run.iter().map(|&(i, d)| Pos2::new(xb(i), y(d))).collect();
            area(p, th, &pts, mid, r.top(), r.top() + plot_h);
        }

        p.rect_filled(Rect::from_min_size(Pos2::new(r.left(), mid.round() - 0.5), Vec2::new(GRAPH_W, 1.0)), 0.0, alpha(th.ink, 0.55));
        let px = (r.left() + self.history.pct * GRAPH_W).round() + 0.5;
        p.vline(px, egui::Rangef::new(r.top(), r.top() + plot_h), Stroke::new(1.0, th.ink));
        kit::text(p, Pos2::new(r.left() + 4.0, r.top() + 2.0), Align2::LEFT_TOP, &format!("±{range:.2}s"), th.medium(10.0), th.ink3);
        kit::outline(p, r, 4.0, 1.0, th.line);
    }
}

/// The recorded bins as one path: the web draws straight across bins it
/// skipped, so this does too.
fn runs(data: &[Option<f32>]) -> Vec<Vec<(usize, f32)>> {
    let run: Vec<(usize, f32)> = data.iter().enumerate().filter_map(|(i, v)| v.map(|d| (i, d))).collect();
    if run.is_empty() {
        Vec::new()
    } else {
        vec![run]
    }
}

/// The filled area between a line and the baseline, with a gradient that's
/// strongest away from the baseline, and the line coloured by side.
fn area(p: &Painter, th: &Theme, pts: &[Pos2], mid: f32, top: f32, bottom: f32) {
    if pts.is_empty() {
        return;
    }
    let shade = |y: f32| {
        let (c, f) = if y < mid { (th.loss, (mid - y) / (mid - top)) } else { (th.gain, (y - mid) / (bottom - mid)) };
        alpha(c, 0.12 + 0.43 * f.clamp(0.0, 1.0))
    };
    let mut mesh = Mesh::default();
    let mut quad = |a: Pos2, b: Pos2| {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(a, shade(a.y));
        mesh.colored_vertex(b, shade(b.y));
        mesh.colored_vertex(Pos2::new(b.x, mid), shade(mid));
        mesh.colored_vertex(Pos2::new(a.x, mid), shade(mid));
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    };
    // Split segments where they cross the baseline, so each side gets its colour.
    let mut segs: Vec<(Pos2, Pos2)> = Vec::new();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a.y - mid) * (b.y - mid) < 0.0 {
            let t = (mid - a.y) / (b.y - a.y);
            let m = Pos2::new(a.x + (b.x - a.x) * t, mid);
            segs.push((a, m));
            segs.push((m, b));
        } else {
            segs.push((a, b));
        }
    }
    for &(a, b) in &segs {
        quad(a, b);
    }
    p.add(Shape::mesh(mesh));
    for (a, b) in segs {
        let above = (a.y + b.y) / 2.0 < mid;
        p.line_segment([a, b], Stroke::new(2.0, if above { th.loss } else { th.gain }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_layouts() {
        assert_eq!(sector_starts("track", 12.0, Some(&[0.0, 0.3, 0.6])), vec![0.0, 0.3, 0.6]);
        assert_eq!(sector_starts("track", 4.0, None), vec![0.0, 0.25, 0.5, 0.75]);
        assert_eq!(sector_starts("mini", 4.0, Some(&[0.0, 0.3])).len(), 4);
    }
}
