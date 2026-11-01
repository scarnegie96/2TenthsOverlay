//! Radar: not ported yet.

use egui::{Painter, Vec2};

use super::{Ctx, Widget};
use crate::kit;

pub struct Radar;

impl Widget for Radar {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let panel = kit::Panel::begin(p, cx.th, 300.0, Some("Radar"), &[]);
        let h = kit::empty(p, cx.th, panel.x(), panel.body_top, "Not ported yet.");
        let bottom = panel.body_top + h;
        panel.finish(p, cx.th, bottom)
    }
}
