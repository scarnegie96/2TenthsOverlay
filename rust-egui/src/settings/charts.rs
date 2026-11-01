//! The Races page's charts, drawn with `egui_plot`: laps across, a value up
//! the side (smaller at the top for positions, gaps and lap times). Hover
//! shows every car's value on that lap; clicking a line or a name in the
//! legend picks that car out on every chart; dragging pans, Ctrl+scroll or a
//! right-button box zooms, a double click resets; pit stops are ringed.

use std::collections::BTreeMap;

use egui::{Color32, Sense, Stroke, Ui, Vec2};
use egui_plot::{GridInput, GridMark, Line, Plot, PlotPoint, PlotPoints, Points, Text, VLine};

use crate::logic::race_report::int_ticks;
use crate::theme::Theme;

use super::ui;

/// One car's values: lap to value.
pub struct Series {
    pub id: i32,
    pub label: String,
    pub color: Color32,
    pub points: BTreeMap<i32, f64>,
}

/// A pit stop to ring on a car's line.
pub struct Mark {
    pub id: i32,
    pub lap: i32,
    pub text: String,
}

pub struct Chart<'a> {
    /// Distinguishes this chart's zoom and pan from the others'.
    pub key: String,
    /// Smaller values at the top.
    pub invert: bool,
    /// Only whole numbers on the side (positions).
    pub integer_y: bool,
    pub format: &'a dyn Fn(f64) -> String,
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
    pub marks: &'a [Mark],
    pub legend: bool,
    pub height: f32,
}

/// What the chart colours of a car look like when it isn't the one picked out.
fn dim(c: Color32) -> Color32 {
    c.gamma_multiply(0.3)
}

/// Round lap numbers across the bottom: a step that gives about a dozen labels.
fn lap_marks(input: GridInput) -> Vec<GridMark> {
    let (lo, hi) = input.bounds;
    let ticks = int_ticks(lo.floor(), hi.ceil(), 12);
    let step = if ticks.len() > 1 { ticks[1] - ticks[0] } else { 1.0 };
    ticks.into_iter().map(|value| GridMark { value, step_size: step }).collect()
}

pub fn line_chart(ui: &mut Ui, th: &Theme, series: &[Series], c: &Chart<'_>, focus: &mut Option<i32>) {
    let all: Vec<(i32, f64)> = series.iter().flat_map(|s| s.points.iter().map(|(l, v)| (*l, *v))).collect();
    if all.is_empty() {
        ui::note(ui, th, "Nothing recorded for this yet.");
        return;
    }
    let x0 = all.iter().map(|p| p.0).min().unwrap() as f64;
    let x1 = (all.iter().map(|p| p.0).max().unwrap() as f64).max(x0 + 1.0);
    let vmin = all.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let vmax = all.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let lo = c.y_min.unwrap_or(vmin);
    let hi = c.y_max.unwrap_or(vmax);
    let span = if hi - lo > 1e-9 { hi - lo } else { 1.0 };
    let pad = if c.integer_y { 0.5 } else { span * 0.04 };
    let clipped = |v: f64| v > hi || v < lo;

    let by_id: BTreeMap<i32, &Series> = series.iter().map(|s| (s.id, s)).collect();
    let mut hover_lap: Option<i32> = None;
    let mut clicked_car: Option<i32> = None;
    let format = c.format;
    let integer_y = c.integer_y;
    let focus_now = *focus;

    let plot = Plot::new(("race-chart", &c.key))
        .height(c.height)
        .width(ui.available_width().min(ui::COLUMN))
        .invert_y(c.invert)
        .allow_scroll(false)
        .allow_boxed_zoom(true)
        .show_x(false)
        .show_y(false)
        .show_grid(true)
        .grid_color(th.line)
        .default_x_bounds(x0 - 0.5, x1 + 0.5)
        .default_y_bounds(lo - pad, hi + pad)
        .x_grid_spacer(lap_marks)
        .x_axis_formatter(|m, _| if m.value.fract() == 0.0 { format!("{}", m.value as i64) } else { String::new() })
        .y_axis_formatter(move |m, _| if integer_y && m.value.fract() != 0.0 { String::new() } else { format(m.value) })
        .y_axis_min_width(46.0)
        .clamp_grid(true);

    let resp = plot.show(ui, |plot_ui| {
        // Picked-out car on top.
        let mut order: Vec<&Series> = series.iter().collect();
        order.sort_by_key(|s| Some(s.id) == focus_now);
        for s in order {
            let faded = focus_now.is_some_and(|f| f != s.id);
            let color = if faded { dim(s.color) } else { s.color };
            let width = if focus_now == Some(s.id) { 2.6 } else { 1.5 };
            // Break the line where a lap is missing, so nothing is drawn that wasn't recorded.
            let mut run: Vec<[f64; 2]> = Vec::new();
            let mut prev: Option<i32> = None;
            let mut runs: Vec<Vec<[f64; 2]>> = Vec::new();
            for (&lap, &v) in &s.points {
                if prev.is_some_and(|p| lap != p + 1) && !run.is_empty() {
                    runs.push(std::mem::take(&mut run));
                }
                run.push([lap as f64, v.clamp(lo, hi)]);
                prev = Some(lap);
            }
            if !run.is_empty() {
                runs.push(run);
            }
            for r in runs {
                plot_ui.line(Line::new(s.label.clone(), PlotPoints::from(r)).color(color).width(width));
            }
            let pts: Vec<[f64; 2]> = s.points.iter().map(|(l, v)| [*l as f64, v.clamp(lo, hi)]).collect();
            plot_ui.points(Points::new(s.label.clone(), PlotPoints::from(pts)).color(color).radius(if focus_now == Some(s.id) { 2.6 } else { 1.8 }).filled(true));
            // Laps cut off by the zoom sit on the edge, ringed.
            let cut: Vec<[f64; 2]> = s.points.iter().filter(|(_, v)| clipped(**v)).map(|(l, v)| [*l as f64, v.clamp(lo, hi)]).collect();
            if !cut.is_empty() {
                plot_ui.points(Points::new(s.label.clone(), PlotPoints::from(cut)).color(color).radius(4.0).filled(false));
            }
        }
        for m in c.marks.iter().filter(|m| focus_now.is_none() || Some(m.id) == focus_now) {
            let Some(s) = by_id.get(&m.id) else { continue };
            let Some(v) = s.points.get(&m.lap).or_else(|| s.points.get(&(m.lap - 1))) else { continue };
            let at = [m.lap as f64, v.clamp(lo, hi)];
            plot_ui.points(Points::new(format!("{}: {}", s.label, m.text), PlotPoints::from(vec![at])).color(s.color).radius(6.5).filled(false));
            plot_ui.text(Text::new("", PlotPoint::new(at[0], at[1]), egui::RichText::new("P").size(9.0).strong()).color(s.color));
        }
        if let Some(p) = plot_ui.pointer_coordinate() {
            let lap = p.x.round();
            if lap >= x0 && lap <= x1 {
                hover_lap = Some(lap as i32);
                plot_ui.vline(VLine::new("", lap).color(th.ink3).width(1.0));
            }
        }
        // A click on a line picks that car out.
        if plot_ui.response().clicked() {
            if let (Some(pos), Some(lap)) = (plot_ui.ctx().input(|i| i.pointer.hover_pos()), hover_lap) {
                let t = plot_ui.transform();
                let mut best: Option<(f32, i32)> = None;
                for s in series {
                    // The point on that lap, or on the lap either side.
                    for l in [lap - 1, lap, lap + 1] {
                        if let Some(v) = s.points.get(&l) {
                            let d = t.position_from_point(&PlotPoint::new(l as f64, v.clamp(lo, hi))).distance(pos);
                            if best.is_none_or(|(bd, _)| d < bd) {
                                best = Some((d, s.id));
                            }
                        }
                    }
                }
                clicked_car = best.filter(|(d, _)| *d < 16.0).map(|(_, id)| id);
            }
        }
    });

    if let Some(lap) = hover_lap {
        let mut at: Vec<(&Series, f64)> = series.iter().filter_map(|s| s.points.get(&lap).map(|v| (s, *v))).collect();
        at.sort_by(|a, b| a.1.total_cmp(&b.1));
        let pits: Vec<&Mark> = c.marks.iter().filter(|m| m.lap == lap && (focus_now.is_none() || Some(m.id) == focus_now)).collect();
        if !at.is_empty() {
            resp.response.on_hover_ui_at_pointer(|ui| {
                ui.set_max_width(320.0);
                ui.label(egui::RichText::new(format!("Lap {lap}")).font(th.bold(15.0)).color(th.ink));
                for (s, v) in at.iter().take(14) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let (r, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                        ui.painter().rect_filled(r, 2.0, s.color);
                        let strong = focus_now == Some(s.id);
                        let t = egui::RichText::new(&s.label).font(if strong { th.bold(14.0) } else { th.medium(14.0) }).color(th.ink);
                        ui.label(t);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format(*v)).font(th.semi(14.0)).color(th.ink2));
                        });
                    });
                }
                if at.len() > 14 {
                    ui.label(ui::text(th, format!("+{} more", at.len() - 14), 13.0, th.ink3));
                }
                for m in pits {
                    if let Some(s) = by_id.get(&m.id) {
                        ui.label(ui::text(th, format!("P  {}: {}", s.label, m.text), 13.0, th.attention));
                    }
                }
            });
        }
    }
    if let Some(id) = clicked_car {
        *focus = if *focus == Some(id) { None } else { Some(id) };
    }
    if c.legend {
        legend(ui, th, series, focus);
    }
    ui.add_space(2.0);
    ui.label(ui::text(th, "Drag to pan, Ctrl+scroll or right-drag a box to zoom, double-click to reset.", 12.0, th.ink3));
}

/// The names under a chart: click one to pick that car out everywhere.
pub fn legend(ui: &mut Ui, th: &Theme, series: &[Series], focus: &mut Option<i32>) {
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 4.0);
        for s in series {
            let on = *focus == Some(s.id);
            let faded = focus.is_some_and(|f| f != s.id);
            let color = if faded { th.ink3 } else if on { th.ink } else { th.ink2 };
            let font = if on { th.bold(13.0) } else { th.medium(13.0) };
            let g = ui.painter().layout_no_wrap(s.label.clone(), font, color);
            let (rect, r) = ui.allocate_exact_size(Vec2::new(g.size().x + 16.0, 20.0), Sense::click());
            let p = ui.painter();
            if on {
                p.rect_filled(rect.expand(2.0), 4.0, th.panel_raised);
            } else if r.hovered() {
                p.rect_filled(rect.expand(2.0), 4.0, crate::theme::alpha(th.ink, 0.05));
            }
            p.rect_filled(egui::Rect::from_center_size(egui::pos2(rect.left() + 5.0, rect.center().y), Vec2::splat(8.0)), 2.0, if faded { dim(s.color) } else { s.color });
            p.galley(egui::pos2(rect.left() + 14.0, rect.center().y - g.size().y / 2.0), g, color);
            if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                *focus = if on { None } else { Some(s.id) };
            }
        }
    });
}

/// A horizontal rule under a chart section's last line, for spacing only.
pub fn rule(ui: &mut Ui, th: &Theme) {
    let y = ui.cursor().min.y;
    let x0 = ui.cursor().min.x;
    ui.painter().hline(x0..=x0 + ui.available_width().min(ui::COLUMN), y, Stroke::new(1.0, th.line));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lap_marks_are_whole_laps_about_a_dozen() {
        let m = lap_marks(GridInput { bounds: (0.5, 30.5), base_step_size: 1.0 });
        assert!(m.len() <= 12 && m.iter().all(|g| g.value.fract() == 0.0), "{m:?}");
        let m = lap_marks(GridInput { bounds: (0.5, 5.5), base_step_size: 1.0 });
        assert_eq!(m.iter().map(|g| g.value as i64).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4, 5, 6]);
    }
}
