//! Lap log: not ported yet.

use egui::{Painter, Vec2};

use super::{Ctx, Widget};
use crate::kit;

pub struct LapLog;

impl Widget for LapLog {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let panel = kit::Panel::begin(p, cx.th, 300.0, Some("Lap log"), &[]);
        let h = kit::empty(p, cx.th, panel.x(), panel.body_top, "Not ported yet.");
        let bottom = panel.body_top + h;
        panel.finish(p, cx.th, bottom)
    }
}
