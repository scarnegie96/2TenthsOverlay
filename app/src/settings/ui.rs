//! The Settings window's controls, drawn like `src/styles/settings.css`:
//! sections of fields (label and help on the left, the control on the right),
//! switches, buttons, segmented choices, key caps and notices.

use egui::{Align, Color32, CornerRadius, FontId, Layout, Response, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::{mix, Theme};

/// Content column width, as `.section { max-width: 720px }`.
pub const COLUMN: f32 = 720.0;

/// Fonts, colours and spacing for everything in this window, so plain egui
/// widgets (combo boxes, text edits, colour pickers) match the rest.
pub fn apply_style(ui: &mut Ui, th: &Theme) {
    let mut style = (**ui.style()).clone();
    use egui::TextStyle::*;
    style.text_styles.insert(Body, th.medium(16.0));
    style.text_styles.insert(Button, th.semi(16.0));
    style.text_styles.insert(Small, th.medium(13.0));
    style.text_styles.insert(Heading, th.bold(30.0));
    style.text_styles.insert(Monospace, FontId::monospace(14.0));
    style.visuals = if th.light() { egui::Visuals::light() } else { egui::Visuals::dark() };
    let v = &mut style.visuals;
    let body = body_color(th);
    v.panel_fill = body;
    v.window_fill = th.panel_solid;
    v.extreme_bg_color = th.panel_raised;
    v.faint_bg_color = mix(th.panel_raised, body, 0.5);
    v.override_text_color = Some(th.ink);
    v.selection.bg_fill = mix(th.lapped, th.panel_solid, 0.45);
    v.selection.stroke = Stroke::new(1.0, th.ink);
    v.window_stroke = Stroke::new(1.0, th.line_strong);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, th.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, th.ink);
    for (w, hover) in [(&mut v.widgets.inactive, false), (&mut v.widgets.hovered, true), (&mut v.widgets.active, true)] {
        w.bg_fill = th.panel_raised;
        w.weak_bg_fill = th.panel_raised;
        w.bg_stroke = Stroke::new(1.0, if hover { th.ink3 } else { th.line_strong });
        w.fg_stroke = Stroke::new(1.0, th.ink);
        w.corner_radius = CornerRadius::same(5);
    }
    v.widgets.open.bg_fill = th.panel_raised;
    v.widgets.open.weak_bg_fill = th.panel_raised;
    v.widgets.open.bg_stroke = Stroke::new(1.0, th.ink3);
    v.widgets.open.corner_radius = CornerRadius::same(5);
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.button_padding = Vec2::new(14.0, 6.0);
    style.spacing.combo_width = 240.0;
    style.spacing.slider_width = 200.0;
    ui.set_style(style);
}

/// `.settings-body`: the panel colour, a little darker.
pub fn body_color(th: &Theme) -> Color32 {
    mix(th.panel_solid, th.well, 0.8)
}

pub fn text(th: &Theme, s: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(s).font(th.medium(size)).color(color)
}

/// A titled group of fields, with a rule above the first.
pub fn section(ui: &mut Ui, th: &Theme, title: &str, add: impl FnOnce(&mut Ui)) {
    ui.add_space(28.0);
    ui.label(RichText::new(title).font(th.semi(15.0)).color(th.ink3));
    ui.add_space(-2.0);
    let top = ui.cursor().min.y;
    let line = Stroke::new(1.0, th.line);
    let x0 = ui.cursor().min.x;
    ui.painter().hline(x0..=x0 + ui.available_width().min(COLUMN), top, line);
    ui.spacing_mut().item_spacing.y = 0.0;
    add(ui);
    ui.spacing_mut().item_spacing.y = 8.0;
}

/// One setting: its label and help on the left, the control on the right.
pub fn field(ui: &mut Ui, th: &Theme, label: &str, help: Option<&str>, controls: impl FnOnce(&mut Ui)) {
    let width = ui.available_width().min(COLUMN);
    let x0 = ui.cursor().min.x;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        ui.set_max_width(width);
        let text_w = (width * 0.56).min(420.0);
        let left = ui.vertical(|ui| {
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add(egui::Label::new(RichText::new(label).font(th.medium(16.0)).color(th.ink)).wrap());
            if let Some(h) = help {
                ui.add(egui::Label::new(RichText::new(h).font(th.medium(14.0)).color(th.ink3)).wrap());
            }
        });
        let h = left.response.rect.height().max(38.0);
        let rest = (width - text_w - 24.0).max(40.0);
        ui.allocate_ui_with_layout(Vec2::new(rest, h), Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            controls(ui);
        });
    });
    ui.add_space(8.0);
    let y = ui.cursor().min.y;
    ui.painter().hline(x0..=x0 + width, y, Stroke::new(1.0, th.line));
}

/// A line of small muted text under a section (`.note`).
pub fn note(ui: &mut Ui, th: &Theme, s: &str) {
    ui.add_space(12.0);
    ui.add(egui::Label::new(text(th, s, 14.0, th.ink3)).wrap());
}

/// The page title and the line under it (`h1` and `.lede`).
pub fn title(ui: &mut Ui, th: &Theme, title: &str, lede: Option<&str>) {
    ui.label(RichText::new(title).font(th.bold(30.0)).color(th.ink));
    if let Some(l) = lede {
        ui.add_space(6.0);
        ui.scope(|ui| {
            ui.set_max_width(560.0);
            ui.add(egui::Label::new(text(th, l, 16.0, th.ink2)).wrap());
        });
    }
}

/// A warning box: the amber border and a close button.
pub fn notice(ui: &mut Ui, th: &Theme, message: &str, closable: bool) -> bool {
    let mut closed = false;
    egui::Frame::new()
        .fill(th.panel_raised)
        .stroke(Stroke::new(1.0, th.attention))
        .corner_radius(5.0)
        .inner_margin(egui::Margin { left: 12, right: 8, top: 8, bottom: 8 })
        .show(ui, |ui| {
            ui.set_max_width(COLUMN - 20.0);
            ui.horizontal_top(|ui| {
                ui.add(egui::Label::new(text(th, message, 14.0, th.ink)).wrap());
                if closable {
                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        let r = ui.add(egui::Button::new(text(th, "×", 16.0, th.ink3)).frame(false));
                        closed = r.on_hover_text("Dismiss").clicked();
                    });
                }
            });
        });
    ui.add_space(16.0);
    closed
}

pub fn error(ui: &mut Ui, th: &Theme, message: &str) {
    ui.add_space(10.0);
    ui.add(egui::Label::new(text(th, message, 14.0, th.loss)).wrap());
}

/// The switch (`.toggle`): green when on.
pub fn toggle(ui: &mut Ui, th: &Theme, on: &mut bool) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(Vec2::new(40.0, 22.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(r.id, *on, 0.12);
    let p = ui.painter();
    let fill = mix(th.gain, th.panel_raised, t);
    let stroke = if *on { th.gain } else { th.line_strong };
    p.rect(rect, 11.0, fill, Stroke::new(1.0, stroke), StrokeKind::Inside);
    let knob = mix(th.panel_solid, th.ink3, t);
    let cx = rect.left() + 11.0 + 18.0 * t;
    p.circle_filled(egui::pos2(cx, rect.center().y), 8.0, knob);
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *on, ""));
    r
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Normal,
    Primary,
    Danger,
    Quiet,
}

/// A button (`.btn`, `.btn-primary`, `.btn-danger`, `.btn-quiet`).
pub fn button(ui: &mut Ui, th: &Theme, label: &str, look: Look) -> Response {
    button_on(ui, th, label, look, true)
}

pub fn button_on(ui: &mut Ui, th: &Theme, label: &str, look: Look, enabled: bool) -> Response {
    let (fill, fg, stroke) = match look {
        Look::Normal => (th.panel_raised, th.ink, th.line_strong),
        Look::Primary => (th.ink, th.panel_solid, th.ink),
        Look::Danger => (th.panel_raised, th.loss, th.line_strong),
        Look::Quiet => (Color32::TRANSPARENT, th.ink2, Color32::TRANSPARENT),
    };
    let galley = ui.painter().layout_no_wrap(label.to_string(), th.semi(16.0), fg);
    let pad = if look == Look::Quiet { 8.0 } else { 14.0 };
    let size = Vec2::new(galley.size().x + pad * 2.0, 32.0);
    let (rect, r) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hover = enabled && r.hovered();
    let p = ui.painter();
    let alpha = if enabled { 1.0 } else { 0.5 };
    let fade = |c: Color32| c.gamma_multiply(alpha);
    let stroke = if hover && look != Look::Primary { th.ink3 } else { stroke };
    let fill = if hover && look == Look::Primary { mix(th.ink, th.panel_solid, 0.88) } else { fill };
    p.rect(rect, 5.0, fade(fill), Stroke::new(1.0, fade(stroke)), StrokeKind::Inside);
    p.galley(egui::pos2(rect.left() + pad, rect.center().y - galley.size().y / 2.0), galley, fade(fg));
    if enabled {
        r.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        r
    }
}

/// Buttons side by side, one picked (`.segmented`). Returns the index clicked.
pub fn segmented(ui: &mut Ui, th: &Theme, labels: &[String], on: Option<usize>) -> Option<usize> {
    let mut clicked = None;
    // Right to left in a field: draw in reverse so the first is leftmost.
    let h = 30.0;
    let galleys: Vec<_> = labels.iter().map(|l| ui.painter().layout_no_wrap(l.clone(), th.semi(16.0), th.ink)).collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 24.0).collect();
    let total: f32 = widths.iter().sum();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, h), Sense::hover());
    let mut x = rect.left();
    let p = ui.painter().clone();
    p.rect_stroke(rect, 5.0, Stroke::new(1.0, th.line_strong), StrokeKind::Inside);
    for (i, (g, w)) in galleys.into_iter().zip(&widths).enumerate() {
        let cell = egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(*w, h));
        let r = ui.interact(cell, ui.scope_id().with(("seg", i)), Sense::click());
        let is_on = on == Some(i);
        let round = CornerRadius { nw: if i == 0 { 5 } else { 0 }, sw: if i == 0 { 5 } else { 0 }, ne: if i + 1 == labels.len() { 5 } else { 0 }, se: if i + 1 == labels.len() { 5 } else { 0 } };
        let fill = if is_on {
            th.ink
        } else if r.hovered() {
            th.panel
        } else {
            th.panel_raised
        };
        p.rect_filled(cell, round, fill);
        if i > 0 {
            p.vline(cell.left(), cell.y_range(), Stroke::new(1.0, th.line_strong));
        }
        let fg = if is_on { th.panel_solid } else { th.ink };
        let galley = p.layout_no_wrap(g.text().to_string(), th.semi(16.0), fg);
        p.galley(egui::pos2(cell.left() + 12.0, cell.center().y - galley.size().y / 2.0), galley, fg);
        if r.clicked() {
            clicked = Some(i);
        }
        x += w;
    }
    p.rect_stroke(rect, 5.0, Stroke::new(1.0, th.line_strong), StrokeKind::Inside);
    clicked
}

/// A key cap (`kbd`).
pub fn kbd(ui: &mut Ui, th: &Theme, label: &str) {
    let galley = ui.painter().layout_no_wrap(label.to_string(), th.medium(15.0), th.ink2);
    let size = Vec2::new((galley.size().x + 14.0).max(26.0), galley.size().y + 6.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(rect, 4.0, th.panel_raised, Stroke::new(1.0, th.line_strong), StrokeKind::Inside);
    p.hline(rect.x_range().shrink(2.0), rect.bottom() - 1.0, Stroke::new(1.0, th.line_strong));
    p.galley(egui::pos2(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, th.ink2);
}

/// A drop-down list: returns the value picked, if the user picked one.
pub fn select(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, current: &str, choices: &[(&str, &str)]) -> Option<String> {
    let mut picked = None;
    let label = choices.iter().find(|(v, _)| *v == current).map(|(_, l)| *l).unwrap_or(current);
    egui::ComboBox::from_id_salt(id).selected_text(label).width(240.0).show_ui(ui, |ui| {
        for (value, label) in choices {
            if ui.selectable_label(*value == current, *label).clicked() {
                picked = Some((*value).to_string());
            }
        }
    });
    picked
}

/// A slider with its value next to it (`.slider`). Returns the new value when it changed.
pub fn slider(ui: &mut Ui, th: &Theme, value: f64, min: f64, max: f64, step: f64, format: &dyn Fn(f64) -> String) -> Option<f64> {
    let mut v = value;
    let mut changed = false;
    // Right to left: the value first, then the slider, so the value stays at the edge.
    ui.allocate_ui_with_layout(Vec2::new(64.0, 22.0), Layout::right_to_left(Align::Center), |ui| {
        ui.label(RichText::new(format(value)).font(th.semi(16.0)).color(th.ink));
    });
    ui.spacing_mut().slider_width = 200.0;
    let s = egui::Slider::new(&mut v, min..=max).step_by(step).show_value(false).trailing_fill(true);
    changed |= ui.add(s).changed();
    changed.then_some(v)
}

/// Number text with the unit: `6 s`, `75%`.
pub fn number_text(v: f64, step: f64, suffix: Option<&str>) -> String {
    let n = if step < 1.0 {
        let s = format!("{v:.2}");
        s.strip_suffix('0').unwrap_or(&s).to_string()
    } else {
        format!("{v}")
    };
    match suffix {
        Some(s) => format!("{n} {s}"),
        None => n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_text_matches_the_web() {
        assert_eq!(number_text(3.0, 1.0, None), "3");
        assert_eq!(number_text(6.0, 1.0, Some("s")), "6 s");
        assert_eq!(number_text(0.5, 0.05, None), "0.5");
        assert_eq!(number_text(0.25, 0.05, None), "0.25");
        // `toFixed(2)` with one trailing zero dropped, as the web does.
        assert_eq!(number_text(1.0, 0.05, Some("x")), "1.0 x");
    }
}
