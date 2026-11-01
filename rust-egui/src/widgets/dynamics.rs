//! Car dynamics: not ported yet.

use egui::{Painter, Vec2};

use super::{Ctx, Widget};
use crate::kit;

#[derive(Default)]
pub struct Dynamics;

impl Widget for Dynamics {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let panel = kit::Panel::begin(p, cx.th, 300.0, Some("Car dynamics"), &[]);
        let h = kit::empty(p, cx.th, panel.x(), panel.body_top, "Not ported yet.");
        let bottom = panel.body_top + h;
        panel.finish(p, cx.th, bottom)
    }
}
