//! One small window per panel (the default; `--fullscreen-overlay` brings back
//! one full-screen window per display).
//!
//! A full-screen transparent always-on-top window makes Windows' compositor
//! blend a whole screen over the game every frame and takes the game off its
//! fast present path. So outside edit mode each shown widget gets its own
//! borderless, transparent, click-through window exactly the size of the widget,
//! placed where the full-screen window would have drawn it. Only those areas
//! are composited.
//!
//! * The root window (it has to stay: it runs `App::ui`, so telemetry, hotkeys,
//!   the tray and Settings keep going) is shrunk to 1×1 at the primary
//!   display's corner. Hiding it would stop eframe calling `App::ui` once no
//!   panel window is visible.
//! * Each panel window is an immediate viewport created hidden. The widget is
//!   drawn in it every frame, which also tells us its size; once the window is
//!   at its place, and refuses activation, it is shown. A widget that draws
//!   nothing (radar with no car near, no flag out) hides its window
//!   (`Visible(false)`) and keeps it, so nothing is created and destroyed
//!   over and over, and nothing is composited.
//! * Resizing: sizes are whole physical pixels (rounded up, so nothing is
//!   clipped); growth is followed at once, a shrink of a single pixel is ignored
//!   (float noise), and a window is only asked to move or resize when it is off.
//! * Edit mode switches back to the full-screen windows (`main.rs`) and the
//!   panel windows are closed; they are made again when editing ends.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Pos2, Rect, Vec2, ViewportCommand, ViewportId};

use crate::anchor;
use crate::config::WidgetCfg;
use crate::displays::{self, Display, PhysRect, Placer};
use crate::frame::PanelFrame;
use crate::schema::WIDGETS;
use crate::theme::Theme;
use crate::widgets::ClickAction;
use crate::{App, PRIMARY_TITLE};

/// The window's title (the Windows code finds it by this): "Tenths · relative".
pub fn title(id: &str) -> String {
    format!("Tenths · {id}")
}

pub fn viewport_id(id: &str) -> ViewportId {
    ViewportId::from_hash_of(("tenths-panel", id))
}

/// Where a panel's window goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// Inner area on the desktop, physical pixels.
    pub phys: PhysRect,
    /// Top-left on its display, CSS px (points).
    pub pos: Pos2,
    /// Size in CSS px (points): the widget's size times its scale.
    pub size: Vec2,
}

/// The window for a widget `natural` big (unscaled) with this config, on display `d`:
/// the same position the full-screen window draws it at (`anchor::resolve` on the
/// display's CSS size), in whole physical pixels.
pub fn place(d: &Display, cfg: &WidgetCfg, natural: Vec2) -> Place {
    let size = natural * cfg.scale;
    let pos = anchor::resolve(cfg, size, d.css_size(), natural);
    let px = |v: f32| (v as f64 * d.scale).round() as i32;
    // Rounded up so the last row and column aren't clipped, less a hair so float noise doesn't add a pixel.
    let up = |v: f32| ((v as f64 * d.scale - 0.02).ceil().max(1.0)) as u32;
    Place { phys: PhysRect { x: d.x + px(pos.x), y: d.y + px(pos.y), w: up(size.x), h: up(size.y) }, pos, size }
}

/// A shrink of one pixel in a dimension is float noise: keep the old size. Growth and bigger shrinks are followed.
pub fn settle(prev: Option<PhysRect>, next: PhysRect) -> PhysRect {
    let Some(prev) = prev else { return next };
    let keep = |old: u32, new: u32| if new > old || old - new >= 2 { new } else { old };
    PhysRect { w: keep(prev.w, next.w), h: keep(prev.h, next.h), ..next }
}

/// What a panel window needs to remember between frames.
#[derive(Default)]
struct State {
    placer: Placer,
    /// Where it was last asked to be (size settled).
    at: Option<PhysRect>,
    shown: bool,
}

/// The panel windows.
#[derive(Default)]
pub struct Panels {
    states: HashMap<&'static str, State>,
    /// The windows asked for this frame.
    pub seen: Vec<ViewportId>,
    /// How many of them are on screen (the others are hidden: they draw nothing).
    pub visible: usize,
}

impl Panels {
    pub fn windows(&self) -> usize {
        self.seen.len()
    }
}

/// A panel window as created: hidden, 1×1, at its display's corner, click-through
/// and not taking the keyboard. Always the same for a panel on the same display, so
/// eframe doesn't send it commands that fight the ones below.
fn builder(title: String, d: &Display) -> egui::ViewportBuilder {
    let r = PhysRect { x: d.x, y: d.y, w: 1, h: 1 }.in_points(d.scale as f32);
    egui::ViewportBuilder::default()
        .with_title(title)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_resizable(false)
        .with_active(false)
        .with_visible(false)
        .with_position(r.min)
        .with_inner_size(r.size())
        .with_mouse_passthrough(true)
}

const TOAST_W: f32 = 640.0;
const TOAST_H: f32 = 72.0;
const TOAST_KEY: &str = "layout-toast";

impl App {
    /// Outside edit mode: the root window out of the way, and a window for each shown widget.
    pub(crate) fn panel_windows(&mut self, ctx: &egui::Context, th: &Theme, live: &crate::data::Live, units: crate::config::Units, show: bool, now: f64) {
        let displays: Arc<Vec<Display>> = self.displays.clone();
        let Some(primary) = displays.iter().find(|d| d.primary).or(displays.first()).cloned() else { return };
        self.layout.stop_editing();
        self.panels.seen.clear();
        self.panels.visible = 0;

        // The root window: 1×1 in the corner, click-through, never the active one.
        self.placers.entry(ViewportId::ROOT).or_default().update_rect(ctx, PhysRect { x: primary.x, y: primary.y, w: 1, h: 1 }, now);
        self.click_through(ctx, &primary, &[], PRIMARY_TITLE);

        // "Layout: Night" has no room in a 1×1 window: it gets its own while it shows.
        if self.toast.is_some() {
            let id = viewport_id(TOAST_KEY);
            self.panels.seen.push(id);
            let css = primary.css_size();
            let want = {
                let top = css.y * 0.18;
                let at = Rect::from_min_size(Pos2::new((css.x - TOAST_W) / 2.0, top), Vec2::new(TOAST_W, TOAST_H));
                let s = primary.scale;
                PhysRect { x: primary.x + (at.min.x as f64 * s).round() as i32, y: primary.y + (at.min.y as f64 * s).round() as i32, w: (TOAST_W as f64 * s).ceil() as u32, h: (TOAST_H as f64 * s).ceil() as u32 }
            };
            let d = &primary;
            ctx.show_viewport_immediate(id, builder(title(TOAST_KEY), d), |ui, class| {
                if class == egui::ViewportClass::EmbeddedWindow {
                    return;
                }
                let ctx = ui.ctx().clone();
                let screen = ctx.content_rect();
                self.layout_toast(&ctx, th, now, screen, screen.top());
                self.manage_window(&ctx, TOAST_KEY, d, want, &[], now);
            });
        }

        for (idx, meta) in WIDGETS.iter().enumerate() {
            let cfg = self.store.config.widget(meta.id);
            if !show || !cfg.enabled {
                continue;
            }
            let Some(key) = displays::widget_display(cfg.display.as_deref(), &displays) else { continue };
            let Some(d) = displays.iter().find(|d| d.key == key) else { continue };
            let id = viewport_id(meta.id);
            self.panels.seen.push(id);
            ctx.show_viewport_immediate(id, builder(title(meta.id), d), |ui, class| {
                if class == egui::ViewportClass::EmbeddedWindow {
                    return;
                }
                let ctx = ui.ctx().clone();
                self.panel_window(&ctx, idx, d, th, live, units, now);
            });
        }

        // Panels no longer wanted lose their windows (egui closes what isn't shown): forget them.
        let seen = self.panels.seen.clone();
        self.panels.states.retain(|k, _| seen.contains(&viewport_id(k)));
    }

    /// One panel's window, from inside it: draws the widget, then sizes, places and shows or hides the window.
    #[allow(clippy::too_many_arguments)]
    fn panel_window(&mut self, ctx: &egui::Context, idx: usize, d: &Display, th: &Theme, live: &crate::data::Live, units: crate::config::Units, now: f64) {
        let id = WIDGETS[idx].id;
        let (natural, clicks) = self.layout.draw_panel(idx, PanelFrame { ctx, store: &self.store, th, live, ratings: &self.ratings, units, now });
        if natural.x <= 0.0 || natural.y <= 0.0 {
            // Draws nothing now: hide the window and keep it for when it does.
            let st = self.panels.states.entry(id).or_default();
            if std::mem::take(&mut st.shown) {
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
            return;
        }
        let want = place(d, self.store.config.widget(id), natural).phys;
        self.manage_window(ctx, id, d, want, &clicks, now);
    }

    /// From inside a window: moves and sizes it to `want` (when it isn't there), keeps it
    /// click-through except over the clickable parts (`clicks`, in its points), and shows it
    /// once it's in place and refuses the keyboard. `want`'s size is settled first.
    fn manage_window(&mut self, ctx: &egui::Context, key: &'static str, d: &Display, want: PhysRect, clicks: &[(Rect, ClickAction)], now: f64) {
        let vid = ctx.viewport_id();
        let st = self.panels.states.entry(key).or_default();
        let want = settle(st.at, want);
        st.at = Some(want);
        let placed = st.placer.update_rect(ctx, want, now);
        let shown = st.shown;
        // The pointer maths for the thumbs works on a "display" that is just this window.
        let me = Display { key: key.to_string(), number: 0, primary: false, x: want.x, y: want.y, width: want.w, height: want.h, scale: d.scale };
        self.click_through(ctx, &me, clicks, &title(key));
        // Never show it before it can't be activated, or it would take iRacing's focus once.
        let ready = self.no_activate.get(&vid) == Some(&true);
        if !shown && placed && ready {
            ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            if let Some(st) = self.panels.states.get_mut(key) {
                st.shown = true;
            }
        }
        if self.panels.states.get(key).is_some_and(|s| s.shown) {
            self.panels.visible += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AnchorX, AnchorY, Config};

    fn d(x: i32, y: i32, w: u32, h: u32, scale: f64) -> Display {
        Display { key: format!("{x},{y}"), number: 1, primary: true, x, y, width: w, height: h, scale }
    }

    fn cfg(x: f64, y: f64, a: (AnchorX, AnchorY), scale: f32) -> WidgetCfg {
        let mut c = Config::default().widget("relative").clone();
        c.x = x;
        c.y = y;
        c.anchor = Some(a);
        c.scale = scale;
        c
    }

    #[test]
    fn titles_and_ids_are_per_panel() {
        assert_eq!(title("relative"), "Tenths · relative");
        assert_ne!(viewport_id("relative"), viewport_id("standings"));
        assert_eq!(viewport_id("relative"), viewport_id("relative"));
    }

    #[test]
    fn top_left_at_100_percent_is_the_css_position() {
        let p = place(&d(0, 0, 1920, 1080, 1.0), &cfg(24.0, 604.0, (AnchorX::L, AnchorY::T), 1.0), Vec2::new(520.0, 240.0));
        assert_eq!(p.phys, PhysRect { x: 24, y: 604, w: 520, h: 240 });
        assert_eq!((p.pos, p.size), (Pos2::new(24.0, 604.0), Vec2::new(520.0, 240.0)));
    }

    #[test]
    fn scaled_widget_on_a_150_percent_display() {
        // 3840×2160 at 150% is 2560×1440 CSS px. A 500×200 widget at 80% is 400×160 px,
        // pinned 10 from the right and 20 from the bottom.
        let p = place(&d(0, 0, 3840, 2160, 1.5), &cfg(10.0, 20.0, (AnchorX::R, AnchorY::B), 0.8), Vec2::new(500.0, 200.0));
        assert_eq!((p.pos, p.size), (Pos2::new(2150.0, 1260.0), Vec2::new(400.0, 160.0)));
        assert_eq!(p.phys, PhysRect { x: 3225, y: 1890, w: 600, h: 240 });
        // Logical (points) for a window on that display: back to the CSS numbers.
        let r = p.phys.in_points(1.5);
        assert_eq!((r.min, r.size()), (Pos2::new(2150.0, 1260.0), Vec2::new(400.0, 160.0)));
        assert!(p.phys.covered_by(r, 1.5));
    }

    #[test]
    fn second_display_left_of_the_primary() {
        // 2560×1440 at 100% with its origin at -2560: centred anchors count from the display's centre.
        let disp = d(-2560, 0, 2560, 1440, 1.0);
        let p = place(&disp, &cfg(0.0, 0.0, (AnchorX::C, AnchorY::M), 1.0), Vec2::new(300.0, 100.0));
        assert_eq!((p.pos, p.phys), (Pos2::new(1130.0, 670.0), PhysRect { x: -2560 + 1130, y: 670, w: 300, h: 100 }));
        // Top-left of that display.
        let p = place(&disp, &cfg(0.0, 0.0, (AnchorX::L, AnchorY::T), 1.0), Vec2::new(300.0, 100.0));
        assert_eq!(p.phys.x, -2560);
        assert_eq!(p.phys.in_points(1.0).min, Pos2::new(-2560.0, 0.0));
    }

    #[test]
    fn second_display_right_of_a_scaled_primary() {
        // 1920×1080 at 125% on the right of a 4K primary: 1536×864 CSS px.
        let disp = d(3840, 0, 1920, 1080, 1.25);
        let p = place(&disp, &cfg(16.0, 10.0, (AnchorX::R, AnchorY::T), 1.0), Vec2::new(200.5, 40.0));
        // left = 1536 - 200.5 - 16 = 1319.5 -> 1320 (rounded whole CSS px), x = 3840 + 1650.
        assert_eq!(p.pos, Pos2::new(1320.0, 10.0));
        assert_eq!(p.phys, PhysRect { x: 3840 + 1650, y: 13, w: 251, h: 50 });
        assert!((p.phys.in_points(1.25).width() - 200.8).abs() < 1e-3);
    }

    #[test]
    fn a_panel_bigger_than_the_screen_is_pulled_back() {
        let p = place(&d(0, 0, 800, 600, 1.0), &cfg(700.0, -50.0, (AnchorX::L, AnchorY::T), 1.0), Vec2::new(300.0, 100.0));
        assert_eq!((p.phys.x, p.phys.y), (500, 0));
    }

    #[test]
    fn float_noise_does_not_change_the_size() {
        let disp = d(0, 0, 1920, 1080, 1.0);
        let c = cfg(0.0, 0.0, (AnchorX::L, AnchorY::T), 1.0);
        let a = place(&disp, &c, Vec2::new(520.0, 240.0)).phys;
        let b = place(&disp, &c, Vec2::new(520.03, 240.03)).phys;
        let n = place(&disp, &c, Vec2::new(520.004, 240.003)).phys;
        let e = place(&disp, &c, Vec2::new(519.9995, 239.9999)).phys;
        assert_eq!((a.w, a.h), (520, 240));
        assert_eq!((b.w, b.h), (521, 241), "a real fraction of a pixel rounds up");
        assert_eq!(a, e);
        assert_eq!(a, n);
        // A one-pixel shrink is ignored, growth and bigger shrinks are followed.
        let prev = PhysRect { x: 5, y: 6, w: 100, h: 50 };
        assert_eq!(settle(Some(prev), PhysRect { x: 7, y: 6, w: 99, h: 49 }), PhysRect { x: 7, y: 6, w: 100, h: 50 });
        assert_eq!(settle(Some(prev), PhysRect { x: 5, y: 6, w: 101, h: 48 }), PhysRect { x: 5, y: 6, w: 101, h: 48 });
        assert_eq!(settle(None, prev), prev);
    }

    #[test]
    fn zero_sized_widgets_still_get_a_one_pixel_rect() {
        // (Never asked for: they are hidden instead. But the maths mustn't produce a zero-sized window.)
        let p = place(&d(0, 0, 1920, 1080, 1.0), &cfg(0.0, 0.0, (AnchorX::L, AnchorY::T), 1.0), Vec2::ZERO);
        assert_eq!((p.phys.w, p.phys.h), (1, 1));
    }
}
