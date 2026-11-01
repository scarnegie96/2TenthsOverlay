//! Coach (`src/widgets/Coach.tsx`): one live message comparing you with the
//! imported reference lap, the reason under it, your delta and speed against
//! it, and the corner costing you most.

use egui::{Align2, Color32, CornerRadius, Painter, Pos2, Rect, Stroke, Vec2};
use telemetry::coach::Tone;

use super::{Ctx, Widget};
use crate::kit::{self, seg, Seg};
use crate::logic::coach::{cue_text, issue_text};
use crate::logic::format::{fixed, signed, speed, speed_unit};
use crate::theme::Theme;

pub struct Coach;

const W: f32 = 400.0;

fn tone_color(th: &Theme, tone: Option<Tone>) -> Option<Color32> {
    match tone {
        Some(Tone::Info) => Some(th.lapped),
        Some(Tone::Good) => Some(th.gain),
        Some(Tone::Warn) => Some(th.attention),
        Some(Tone::Bad) => Some(th.loss),
        _ => None,
    }
}

/// The panel with its coloured left edge; returns the size.
fn chrome(p: &Painter, th: &Theme, edge: Color32, draw: impl FnOnce(f32) -> f32) -> Vec2 {
    let panel = kit::Panel::begin(p, th, W, None, &[]);
    let bottom = draw(panel.inset + 4.0);
    let size = panel.finish(p, th, bottom);
    let rad = th.radius as u8;
    p.rect_filled(Rect::from_min_size(Pos2::ZERO, Vec2::new(4.0, size.y)), CornerRadius { nw: rad, sw: rad, ne: 0, se: 0 }, edge);
    size
}

impl Widget for Coach {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let o = cx.o;
        let units = cx.units;
        let tick = cx.live.tick.as_deref();
        let coach = tick.and_then(|t| t.coach.as_ref());
        let reference = cx.live.reference.as_deref();
        let x0 = 14.0 + 4.0;
        let max_w = W - x0 - 14.0;

        // No reference (or one for another track): a note while editing, nothing otherwise.
        let idle = match reference {
            Some(r) if r.loaded && !r.matches_track => Some(("Coach paused", format!("The reference lap is for {}.", r.track_name))),
            Some(r) if r.loaded => None,
            _ => Some(("Coach", "Import a reference lap in settings (Delta) to get live coaching.".to_string())),
        };
        if let Some((label, detail)) = idle {
            if !cx.editing {
                return Vec2::ZERO;
            }
            return chrome(p, th, th.ink3, |top| {
                let r = kit::text(p, Pos2::new(x0, top + 10.0), Align2::LEFT_TOP, label, th.bold(20.0), th.ink2);
                let d = kit::text_wrap(p, Pos2::new(x0, r.bottom() + 3.0), &detail, th.medium(15.0), th.ink2, max_w);
                d.bottom() + 10.0
            });
        }

        let cue = coach.and_then(|c| c.cue.as_ref());
        let text = match (cue, coach) {
            (Some(c), _) => cue_text(c, units),
            (None, Some(_)) => crate::logic::coach::Text { label: "Coaching paused".into(), detail: "Off track or under yellow".into() },
            (None, None) => crate::logic::coach::Text { label: "Waiting for you to drive".into(), detail: "Coaching starts on track".into() },
        };
        let tone = tone_color(th, cue.map(|c| c.tone));
        let delta = tick.and_then(|t| t.deltas.reference);
        let sd = coach.and_then(|c| c.speed_delta);
        let focus = coach.and_then(|c| c.focus.as_ref());
        chrome(p, th, tone.unwrap_or(th.ink3), |top| {
            let r = kit::text_wrap(p, Pos2::new(x0, top + 10.0), &text.label, th.bold(30.0), tone.unwrap_or(th.ink), max_w);
            let mut y = r.bottom();
            if o.b("showDetail") {
                let d = kit::text_wrap(p, Pos2::new(x0, y + 3.0), &text.detail, th.medium(15.0), th.ink2, max_w);
                y = d.bottom();
            }
            y += 8.0;
            let x = egui::Rangef::new(4.0, W - 1.0);
            if o.b("showStats") {
                p.hline(x, y + 0.5, Stroke::new(1.0, th.line));
                let cy = y + 1.0 + 6.0 + 8.5;
                let font = th.semi(14.0);
                let mut parts: Vec<Seg> = Vec::new();
                if let Some(c) = cue.and_then(|c| c.corner) {
                    parts.push(seg(format!("Corner {c}"), font.clone(), th.ink2));
                }
                let dc = match delta {
                    None => th.ink2,
                    Some(d) if d > 0.0 => th.loss,
                    Some(_) => th.gain,
                };
                parts.push(seg(delta.map(|d| format!("{}s", signed(d as f64, 2))).unwrap_or("–".into()), font.clone(), dc));
                let sc = match sd {
                    Some(v) if v < -0.3 => th.loss,
                    Some(v) if v > 0.3 => th.gain,
                    _ => th.ink2,
                };
                parts.push(seg(sd.map(|v| format!("{}{} {}", if v >= 0.0 { "+" } else { "−" }, speed(v.abs(), units), speed_unit(units))).unwrap_or("–".into()), font, sc));
                // The first sits on the left, the rest on the right 14 px apart.
                let first = parts.remove(0);
                kit::text(p, Pos2::new(x0, cy), Align2::LEFT_CENTER, &first.text, first.font.clone(), first.color);
                let mut rx = W - 14.0;
                for s in parts.iter().rev() {
                    let r = kit::text(p, Pos2::new(rx, cy), Align2::RIGHT_CENTER, &s.text, s.font.clone(), s.color);
                    rx = r.left() - 14.0;
                }
                y += 1.0 + 6.0 + 17.0 + 6.0;
            }
            if let (true, Some(f)) = (o.b("showFocus"), focus) {
                p.hline(x, y + 0.5, Stroke::new(1.0, th.line));
                let cy = y + 1.0 + 5.0 + 8.0;
                let tag = kit::text_sp(p, Pos2::new(x0, cy), Align2::LEFT_CENTER, "FOCUS", th.bold(11.0), th.attention, 0.66);
                let issue = issue_text(f.issue);
                let what = if issue.is_empty() { format!("Corner {}", f.corner) } else { format!("Corner {}, {issue}", f.corner) };
                let loss = kit::text(p, Pos2::new(W - 14.0, cy), Align2::RIGHT_CENTER, &format!("+{}s", fixed(f.loss_s as f64, 2)), th.semi(13.0), th.loss);
                kit::text_fit(p, Pos2::new(tag.right() + 10.0, cy), Align2::LEFT_CENTER, &what, th.medium(13.0), th.ink2, loss.left() - tag.right() - 20.0, 0.0);
                y += 1.0 + 5.0 + 16.0 + 6.0;
            }
            y
        })
    }
}
