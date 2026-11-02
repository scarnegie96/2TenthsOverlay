//! The stream window, for OBS's Window Capture.
//!
//! The Tauri version served each widget as a web page. Here OBS captures one
//! ordinary window instead: "Tenths stream", decorated, in the taskbar, not
//! on top and not click-through, so iRacing can cover it and OBS still gets
//! its picture (Window Capture with the "Windows 10 (1903 and up)" method).
//! It draws the widgets where the layout puts them on the chosen display,
//! scaled to a share of that display (50, 75 or 100 %), on a solid colour OBS
//! can key out. The drawing is the overlay's own (`Layout::draw_stream`): same
//! theme, options and opacity. Closing the window turns the option off.
//!
//! Settings: `general.streaming` (`window`, `background`, `display`, `widgets`,
//! `scale`; `enabled` and `port` are the Tauri app's). Off by default, and while
//! off nothing here runs.

use egui::{Color32, Rect, Vec2, ViewportBuilder, ViewportCommand, ViewportId};

use crate::config::{Config, StreamBackground};
use crate::displays::{self, Display};
use crate::frame::StreamFrame;
use crate::schema::WIDGETS;
use crate::App;

/// The window's title: what OBS's Window Capture lists it as.
pub const TITLE: &str = "Tenths stream";

pub fn viewport_id() -> ViewportId {
    ViewportId::from_hash_of("tenths-stream")
}

pub fn background_color(b: StreamBackground) -> Color32 {
    match b {
        StreamBackground::Green => Color32::from_rgb(0, 255, 0),
        StreamBackground::Magenta => Color32::from_rgb(255, 0, 255),
        StreamBackground::Black => Color32::BLACK,
        StreamBackground::Grey => Color32::from_rgb(20, 24, 29),
    }
}

pub fn background_name(b: StreamBackground) -> &'static str {
    match b {
        StreamBackground::Green => "Green (#00FF00)",
        StreamBackground::Magenta => "Magenta (#FF00FF)",
        StreamBackground::Black => "Black",
        StreamBackground::Grey => "Dark grey",
    }
}

/// What the window follows: the display, and the widgets to draw, each with the
/// size (points) of the display it is placed on. With none chosen, the widgets
/// that are on and on that display; with some chosen, those, wherever they
/// are placed, even if the overlay has them off.
pub fn wanted<'a>(cfg: &Config, displays: &'a [Display]) -> Option<(&'a Display, Vec<(usize, Vec2)>)> {
    let st = &cfg.general.streaming;
    let find = |key: Option<&str>| displays::widget_display(key, displays).and_then(|k| displays.iter().find(|d| d.key == k));
    let target = find(st.display.as_deref())?;
    let mut want = Vec::new();
    for (idx, meta) in WIDGETS.iter().enumerate() {
        let w = cfg.widget(meta.id);
        let own = find(w.display.as_deref()).unwrap_or(target);
        let chosen = if st.widgets.is_empty() { w.enabled && own.key == target.key } else { st.widgets.iter().any(|id| id == meta.id) };
        if chosen {
            want.push((idx, own.css_size()));
        }
    }
    Some((target, want))
}

/// The window's size in points when its screen shows `scale` of `d`, at `ppp` pixels per point.
pub fn window_size(d: &Display, scale: f32, ppp: f32) -> Vec2 {
    Vec2::new((d.width as f32 * scale / ppp).round().max(1.0), (d.height as f32 * scale / ppp).round().max(1.0))
}

/// What the stream window remembers between frames.
#[derive(Default)]
pub struct Stream {
    /// The size last asked of the window, so it is asked once, not every frame.
    asked: Option<Vec2>,
    /// What the tray's item was last told, to tell it only when it changes.
    pub tray: Option<bool>,
}

impl App {
    /// The stream window, when it's on: draws the widgets in it and keeps it the right size.
    /// Called once a frame from the root window, after the widgets have observed the data.
    pub(crate) fn stream_window(&mut self, ctx: &egui::Context, th: &crate::theme::Theme, live: &crate::data::Live, units: crate::config::Units, show: bool, now: f64) {
        let on = self.store.config.general.streaming.window;
        if self.stream.tray != Some(on) {
            self.stream.tray = Some(on);
            self.shell.set_tray_stream(on);
        }
        if !on {
            self.stream.asked = None;
            return;
        }
        let displays = self.displays.clone();
        let Some((d, want)) = wanted(&self.store.config, &displays) else { return };
        let st = self.store.config.general.streaming.clone();
        let want = if show { want } else { Vec::new() };
        let builder = ViewportBuilder::default()
            .with_title(TITLE)
            .with_decorations(true)
            .with_resizable(false)
            .with_inner_size(window_size(d, st.scale, d.scale as f32));
        let mut closed = false;
        ctx.show_viewport_immediate(viewport_id(), builder, |ui, class| {
            if class == egui::ViewportClass::EmbeddedWindow {
                return;
            }
            let ctx = ui.ctx().clone();
            if ctx.input(|i| i.viewport().close_requested()) {
                closed = true;
                return;
            }
            // The window's own pixels per point (it may sit on a monitor with other scaling than the display it follows).
            let ppp = ctx.input(|i| i.viewport().native_pixels_per_point).unwrap_or(d.scale as f32).max(0.1);
            let size = window_size(d, st.scale, ppp);
            let have = ctx.content_rect().size();
            if self.stream.asked != Some(size) && ((have.x - size.x).abs() > 1.0 || (have.y - size.y).abs() > 1.0) {
                self.stream.asked = Some(size);
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
            }
            let k = st.scale * d.scale as f32 / ppp;
            let screen = ctx.content_rect();
            ctx.layer_painter(egui::LayerId::background()).rect_filled(Rect::from_min_size(screen.min, screen.size().max(size)), 0.0, background_color(st.background));
            self.layout.draw_stream(StreamFrame { ctx: &ctx, store: &self.store, th, live, ratings: &self.ratings, units, now, want: &want, k });
        });
        if closed {
            self.store.set_stream_window(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn display(key: &str, number: u32, primary: bool, w: u32, h: u32, scale: f64) -> Display {
        Display { key: key.into(), number, primary, x: 0, y: 0, width: w, height: h, scale }
    }

    fn ids(want: &[(usize, Vec2)]) -> Vec<&'static str> {
        want.iter().map(|(i, _)| WIDGETS[*i].id).collect()
    }

    #[test]
    fn an_old_config_has_the_window_off_and_the_defaults() {
        let c = crate::config::normalize(&json!({ "general": { "streaming": { "enabled": true, "port": 8000 } } }));
        let s = &c.general.streaming;
        assert!(s.enabled && !s.window);
        assert_eq!(s.port, 8000);
        assert_eq!((s.background, s.display.clone(), s.widgets.clone(), s.scale), (StreamBackground::Green, None, vec![], 0.5));
        assert!(!crate::config::normalize(&json!({})).general.streaming.window);
        // Nonsense falls back instead of failing.
        let c = crate::config::normalize(&json!({ "general": { "streaming": { "port": 80, "background": "pink", "scale": 7, "widgets": ["relative", "nope", 3], "window": "yes" } } }));
        let s = &c.general.streaming;
        assert_eq!((s.port, s.background, s.scale, s.window), (7457, StreamBackground::Green, 0.5, false));
        assert_eq!(s.widgets, vec!["relative".to_string()]);
    }

    #[test]
    fn the_new_fields_are_read() {
        let c = crate::config::normalize(&json!({ "general": { "streaming": { "window": true, "background": "magenta", "display": "DISPLAY2", "widgets": ["fuel"], "scale": 0.75 } } }));
        let s = &c.general.streaming;
        assert!(s.window);
        assert_eq!((s.background, s.scale), (StreamBackground::Magenta, 0.75));
        assert_eq!(s.display.as_deref(), Some("DISPLAY2"));
        assert_eq!(s.widgets, vec!["fuel".to_string()]);
    }

    #[test]
    fn the_layout_of_a_display_is_what_is_on_it() {
        let displays = displays::finish(vec![display("a", 1, true, 1920, 1080, 1.0), display("b", 2, false, 2560, 1440, 1.0)]);
        let c = crate::config::normalize(&json!({ "widgets": {
            "relative": { "enabled": true, "x": 0, "y": 0 },
            "fuel": { "enabled": true, "x": 0, "y": 0, "display": "b" },
            "radar": { "enabled": false, "x": 0, "y": 0 },
        }, "general": { "streaming": { "display": "b" } } }));
        let (d, want) = wanted(&c, &displays).unwrap();
        assert_eq!(d.key, "b");
        assert_eq!(ids(&want), vec!["fuel"]);
        let c = crate::config::normalize(&json!({ "widgets": { "relative": { "enabled": true, "x": 0, "y": 0 }, "fuel": { "enabled": true, "x": 0, "y": 0, "display": "b" } } }));
        let (d, want) = wanted(&c, &displays).unwrap();
        assert_eq!(d.key, "a");
        assert!(ids(&want).contains(&"relative") && !ids(&want).contains(&"fuel"));
        // A display that's gone: the main one.
        let c = crate::config::normalize(&json!({ "general": { "streaming": { "display": "gone" } } }));
        assert_eq!(wanted(&c, &displays).unwrap().0.key, "a");
    }

    #[test]
    fn chosen_widgets_show_even_when_the_overlay_has_them_off() {
        let displays = displays::finish(vec![display("a", 1, true, 1920, 1080, 1.0), display("b", 2, false, 2560, 1440, 1.0)]);
        let c = crate::config::normalize(&json!({ "widgets": {
            "relative": { "enabled": false, "x": 0, "y": 0 },
            "fuel": { "enabled": true, "x": 0, "y": 0, "display": "b" },
        }, "general": { "streaming": { "widgets": ["relative", "fuel"] } } }));
        let (_, want) = wanted(&c, &displays).unwrap();
        assert_eq!(ids(&want), vec!["relative", "fuel"]);
        // Each is placed against the display it is on.
        assert_eq!(want[0].1, Vec2::new(1920.0, 1080.0));
        assert_eq!(want[1].1, Vec2::new(2560.0, 1440.0));
        assert!(wanted(&c, &[]).is_none());
    }

    #[test]
    fn the_window_is_a_share_of_the_display_in_pixels() {
        let d = display("a", 1, true, 2560, 1440, 1.5);
        assert_eq!(window_size(&d, 0.5, 1.5), Vec2::new(853.0, 480.0));
        assert_eq!(window_size(&d, 1.0, 1.0), Vec2::new(2560.0, 1440.0));
        assert_eq!(window_size(&d, 0.75, 2.0), Vec2::new(960.0, 540.0));
    }
}
