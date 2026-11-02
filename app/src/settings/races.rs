//! Settings, Races: every saved race report, and one race in full (`Races.tsx`).
//! Only recorded data is shown: crossing times, lap times between them, the
//! order across the line, pit stops and stints as timed, and your laps as logged.

use std::collections::BTreeMap;

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoint, PlotPoints};
use serde_json::json;

use crate::kit::{compound_kind, tyre_color};
use crate::logic::format::lap_time;
use crate::logic::race_report::{self as rr, RaceReport, ReportCar};
use crate::shell::Paths;
use crate::theme::{parse_hex, Theme};

use super::charts::{self, Chart, Mark, Series};
use super::edits::Editor;
use super::reports_io::{self as io, ReportSummary};
use super::timefmt;
use super::ui::{self, Look};
use super::Env;

const PLAYER: Color32 = Color32::from_rgb(0xff, 0xb5, 0x47);

const ZOOMS_GAP: [(f64, &str); 4] = [(0.0, "Everything"), (60.0, "Up to 60 s"), (30.0, "Up to 30 s"), (10.0, "Up to 10 s")];
const ZOOMS_LAP: [(f64, &str); 4] = [(0.0, "Every lap"), (10.0, "Best + 10 s"), (5.0, "Best + 5 s"), (3.0, "Best + 3 s")];

#[derive(Default)]
pub struct State {
    list: Option<Vec<ReportSummary>>,
    view: Option<View>,
    error: Option<String>,
}

struct View {
    report: RaceReport,
    class_id: i32,
    focus: Option<i32>,
    gap_zoom: usize,
    lap_zoom: usize,
    /// The lap under the mouse in a chart or the laps table, shared between them.
    lap_hover: Option<i32>,
}

impl State {
    pub fn open_report(&mut self, paths: &Paths, id: &str) {
        let Some(dir) = paths.reports_dir() else { return };
        match io::read(&dir, id).and_then(|v| RaceReport::parse(&v.to_string())) {
            Ok(report) => {
                self.view = Some(View {
                    class_id: report.player_class_id,
                    focus: Some(report.player_car_idx),
                    report,
                    gap_zoom: 0,
                    lap_zoom: 2,
                    lap_hover: None,
                });
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn show(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) {
        if self.view.is_some() && ui::button(ui, th, "← All races", Look::Normal).clicked() {
            self.view = None;
        }
        if let Some(view) = &mut self.view {
            report_view(ui, th, view);
            return;
        }
        self.list_page(ui, th, env, ed);
    }

    fn refresh(&mut self, env: &Env<'_>) {
        self.list = env.shell.paths.reports_dir().map(|d| io::list(&d));
    }

    fn list_page(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, ed: &mut Editor) {
        if self.list.is_none() {
            self.refresh(env);
        }
        ui::title(
            ui,
            th,
            "Races",
            Some("With race reports on, every race you drive is saved when you take the flag (or when the session ends if you don't): everyone's laps, gaps and positions, tyre stints and pit stops, and your own fuel and laps. Your inputs aren't kept."),
        );
        ui.add_space(22.0);
        let save = ed.raw["general"]["saveReports"].as_bool().unwrap_or(true);
        ui::section(ui, th, "Reports", |ui| {
            ui::field(ui, th, "Save race reports", Some("Off: nothing is recorded to disk. Reports already saved stay until you delete them."), |ui| {
                let mut on = save;
                if ui::toggle(ui, th, &mut on).changed() {
                    ed.set(env.store, &["general", "saveReports"], json!(on));
                }
            });
        });
        if let Some(e) = &self.error {
            ui.add_space(12.0);
            ui::notice(ui, th, e, false);
        }
        ui.add_space(16.0);
        let list = self.list.clone().unwrap_or_default();
        if list.is_empty() {
            ui::note(ui, th, "No races yet. Finish a race and it shows up here.");
            return;
        }
        let mut open: Option<String> = None;
        let mut delete: Option<String> = None;
        for r in &list {
            let width = ui.available_width().min(ui::COLUMN);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(width - 8.0 - 76.0, 64.0), Sense::click());
                let p = ui.painter();
                let hover = resp.hovered();
                p.rect(rect, 5.0, if hover { th.panel } else { th.panel_raised }, Stroke::new(1.0, th.line_strong), StrokeKind::Inside);
                let when = p.layout_no_wrap(timefmt::medium(r.started_at), th.medium(13.0), th.ink3);
                let wsize = when.size();
                p.galley(Pos2::new(rect.right() - 12.0 - wsize.x, rect.top() + 10.0), when, th.ink3);
                let title = format!("{}{}", r.track, if r.track_config.is_empty() { String::new() } else { format!(" · {}", r.track_config) });
                let g = p.layout_no_wrap(title, th.bold(17.0), th.ink);
                p.galley(rect.min + Vec2::new(12.0, 8.0), g, th.ink);
                let g = p.layout_no_wrap(r.car.clone(), th.medium(14.0), th.ink2);
                p.galley(rect.min + Vec2::new(12.0, 28.0), g, th.ink2);
                let result = format!(
                    "{} · {} laps{}",
                    if r.finished { format!("P{} of {} in class", r.class_position, r.class_cars) } else { "Didn't finish".to_string() },
                    r.laps,
                    if r.official { "" } else { " · unofficial" }
                );
                let g = p.layout_no_wrap(result, th.medium(14.0), th.ink3);
                p.galley(rect.min + Vec2::new(12.0, 44.0), g, th.ink3);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    open = Some(r.id.clone());
                }
                if ui::button(ui, th, "Delete", Look::Danger).on_hover_text("Delete this report").clicked() {
                    delete = Some(r.id.clone());
                }
            });
            ui.add_space(6.0);
        }
        if let Some(id) = delete {
            if let Some(dir) = env.shell.paths.reports_dir() {
                if let Err(e) = io::delete(&dir, &id) {
                    self.error = Some(e);
                }
            }
            self.refresh(env);
        }
        if let Some(id) = open {
            self.open_report(&env.shell.paths, &id);
        }
    }
}

// --- One race ----------------------------------------------------------------------------

fn cars_in_class(r: &RaceReport, class_id: i32) -> Vec<&ReportCar> {
    let mut cars: Vec<&ReportCar> = r.cars.iter().filter(|c| c.class_id == class_id).collect();
    cars.sort_by_key(|c| if c.finish_class_position == 0 { 999 } else { c.finish_class_position });
    cars
}

fn report_view(ui: &mut Ui, th: &Theme, v: &mut View) {
    let r = &v.report;
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(egui::RichText::new(&r.track).font(th.bold(30.0)).color(th.ink));
        if !r.track_config.is_empty() {
            ui.label(egui::RichText::new(format!(" · {}", r.track_config)).font(th.medium(20.0)).color(th.ink3));
        }
    });
    ui.add_space(4.0);
    ui.add(egui::Label::new(ui::text(th, format!("{} · {} · {}{}", timefmt::full(r.started_at), r.car, r.session_type, if r.official { " · official" } else { " · unofficial" }), 16.0, th.ink2)).wrap());

    let cars = cars_in_class(r, v.class_id);
    let colors: BTreeMap<i32, Color32> = cars
        .iter()
        .enumerate()
        .map(|(k, c)| {
            let [rr_, g, b] = rr::hsl_color(k, cars.len());
            (c.car_idx, if c.is_player { PLAYER } else { Color32::from_rgb(rr_, g, b) })
        })
        .collect();

    // Facts.
    let me = r.me();
    let cls = r.classes.iter().find(|c| c.class_id == r.player_class_id);
    let my_best = me.map(|m| rr::stats(&rr::lap_times(m).values().copied().collect::<Vec<_>>()).best);
    let in_class = r.cars.iter().filter(|c| c.class_id == r.player_class_id).count();
    ui.add_space(16.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
        let finished = me.is_some() && r.finished;
        fact(
            ui,
            th,
            "Finish",
            &if finished { format!("P{}", me.unwrap().finish_class_position) } else { "Didn't finish".into() },
            finished.then(|| format!("of {in_class} in {} · P{} overall", cls.map(|c| c.name.as_str()).filter(|n| !n.is_empty()).unwrap_or("class"), me.unwrap().finish_position)),
        );
        fact(ui, th, "Start", &me.and_then(|m| m.start_position).map(|p| format!("P{p}")).unwrap_or_else(|| "–".into()), Some("in class".into()));
        fact(ui, th, "Laps", &me.map(|m| m.laps_completed).unwrap_or(0).to_string(), r.total_laps.map(|t| format!("of {t}")));
        fact(ui, th, "Best lap", &lap_time(my_best.flatten().map(|b| b as f32)), None);
        fact(ui, th, "Incidents", &format!("{}x", r.incidents), r.incident_limit.map(|l| format!("limit {l}x")));
        fact(ui, th, "Stops", &me.map(|m| m.stops).unwrap_or(0).to_string(), None);
        if let Some(c) = cls.filter(|c| c.strength_of_field.unwrap_or(0) > 0) {
            fact(ui, th, "SoF", &c.strength_of_field.unwrap_or(0).to_string(), Some(c.name.clone()));
        }
    });

    // Classes.
    if r.classes.len() > 1 {
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for c in &r.classes {
                let on = c.class_id == v.class_id;
                let n = r.cars.iter().filter(|x| x.class_id == c.class_id).count();
                let color = parse_hex(&c.color).unwrap_or(th.ink3);
                let text = format!("{} {}", if c.name.is_empty() { "Class" } else { &c.name }, n);
                let g = ui.painter().layout_no_wrap(text, th.semi(14.0), if on { th.panel_solid } else { th.ink });
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 24.0, 28.0), Sense::click());
                let p = ui.painter();
                p.rect(rect, 14.0, if on { color } else { th.panel_raised }, Stroke::new(1.5, color), StrokeKind::Inside);
                let fg = if on { th.panel_solid } else { th.ink };
                p.galley(Pos2::new(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, fg);
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    v.class_id = c.class_id;
                    v.focus = None;
                }
            }
        });
    }

    let r = &v.report;
    let cars = cars_in_class(r, v.class_id);
    results(ui, th, &cars, &colors, &mut v.focus);

    let make_series = |data: &BTreeMap<i32, BTreeMap<i32, f64>>| -> Vec<Series> {
        cars.iter().map(|c| Series { id: c.car_idx, label: format!("#{} {}", c.number, c.name), color: colors[&c.car_idx], points: data.get(&c.car_idx).cloned().unwrap_or_default() }).collect()
    };
    let marks: Vec<Mark> = cars
        .iter()
        .flat_map(|c| c.pit_stops.iter().map(|p| Mark { id: c.car_idx, lap: p.lap + 1, text: format!("Pit lane {:.1} s, stopped {:.1} s", p.lane_s, p.stall_s) }))
        .collect();
    let class_key = v.class_id;
    let rid = r.id.clone();

    chart_section(ui, th, "Position", Some("Order across the line at the end of each lap, in this class. Click a name to pick out a driver."), None, |ui| {
        let fmt = |x: f64| format!("P{x}");
        let c = Chart { key: format!("{rid}-pos-{class_key}"), invert: true, integer_y: true, format: &fmt, y_min: None, y_max: None, marks: &marks, legend: true, height: 300.0 };
        charts::line_chart(ui, th, &make_series(&rr::positions_by_lap(&cars)), &c, &mut v.focus);
    });

    let gap_zoom = ZOOMS_GAP[v.gap_zoom].0;
    let mut new_gap = v.gap_zoom;
    chart_section(
        ui,
        th,
        "Gap to the class leader",
        Some("Seconds behind the first car in this class to finish each lap, at the line."),
        Some(&mut |ui: &mut Ui| {
            if let Some(i) = zoom_select(ui, "gap-zoom", &ZOOMS_GAP, v.gap_zoom) {
                new_gap = i;
            }
        }),
        |ui| {
            let fmt = |x: f64| if x == 0.0 { "Leader".to_string() } else { format!("+{x:.1} s") };
            let c = Chart { key: format!("{rid}-gap-{class_key}-{gap_zoom}"), invert: true, integer_y: false, format: &fmt, y_min: Some(0.0), y_max: (gap_zoom > 0.0).then_some(gap_zoom), marks: &marks, legend: true, height: 300.0 };
            charts::line_chart(ui, th, &make_series(&rr::gaps_by_lap(&cars)), &c, &mut v.focus);
        },
    );
    v.gap_zoom = new_gap;

    let times: BTreeMap<i32, BTreeMap<i32, f64>> = cars.iter().map(|c| (c.car_idx, rr::lap_times(c))).collect();
    let best = times.values().flat_map(|m| m.values().copied()).fold(f64::INFINITY, f64::min);
    let lap_zoom = ZOOMS_LAP[v.lap_zoom].0;
    let mut new_lap = v.lap_zoom;
    chart_section(
        ui,
        th,
        "Lap times",
        Some("Time between crossing the line on one lap and the next. Quicker is higher. Laps cut off by the zoom sit on the bottom edge, ringed."),
        Some(&mut |ui: &mut Ui| {
            if let Some(i) = zoom_select(ui, "lap-zoom", &ZOOMS_LAP, v.lap_zoom) {
                new_lap = i;
            }
        }),
        |ui| {
            let fmt = |x: f64| lap_time(Some(x as f32));
            let finite = best.is_finite();
            let c = Chart {
                key: format!("{rid}-lap-{class_key}-{lap_zoom}"),
                invert: true,
                integer_y: false,
                format: &fmt,
                y_min: finite.then_some(best - 0.3),
                y_max: (lap_zoom > 0.0 && finite).then_some(best + lap_zoom),
                marks: &marks,
                legend: true,
                height: 300.0,
            };
            charts::line_chart(ui, th, &make_series(&times), &c, &mut v.focus);
        },
    );
    v.lap_zoom = new_lap;

    chart_section(ui, th, "Tyre stints", Some("Each stint from the pit lane to the pit lane, coloured by compound, with each car's pit stops. Hover for times."), None, |ui| {
        stint_chart(ui, th, &cars, &r.tire_compounds, &colors, &mut v.focus);
    });

    if !r.player_laps.is_empty() {
        your_laps(ui, th, v);
    }
}

fn fact(ui: &mut Ui, th: &Theme, label: &str, value: &str, sub: Option<String>) {
    egui::Frame::new().fill(th.panel_raised).stroke(Stroke::new(1.0, th.line)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.set_min_width(76.0);
        ui.label(ui::text(th, label.to_uppercase(), 12.0, th.ink3));
        ui.label(egui::RichText::new(value).font(th.bold(24.0)).color(th.ink));
        if let Some(s) = sub {
            ui.label(ui::text(th, s, 12.0, th.ink3));
        }
    });
}

fn chart_section(ui: &mut Ui, th: &Theme, title: &str, help: Option<&str>, tools: Option<&mut dyn FnMut(&mut Ui)>, body: impl FnOnce(&mut Ui)) {
    ui.add_space(28.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(th.semi(15.0)).color(th.ink3));
        if let Some(t) = tools {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| t(ui));
        }
    });
    if let Some(h) = help {
        ui.add(egui::Label::new(ui::text(th, h, 14.0, th.ink3)).wrap());
        ui.add_space(6.0);
    }
    body(ui);
}

fn zoom_select(ui: &mut Ui, id: &str, choices: &[(f64, &str); 4], current: usize) -> Option<usize> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(id).selected_text(choices[current].1).width(140.0).show_ui(ui, |ui| {
        for (i, (_, label)) in choices.iter().enumerate() {
            if ui.selectable_label(i == current, *label).clicked() {
                picked = Some(i);
            }
        }
    });
    picked
}

// --- Results -----------------------------------------------------------------------------------

/// Column starts and alignment: `(x, width, right-aligned)`.
const COLS: [(f32, f32, bool); 10] = [
    (0.0, 52.0, false),
    (52.0, 44.0, true),
    (108.0, 190.0, false),
    (302.0, 46.0, true),
    (352.0, 40.0, true),
    (396.0, 70.0, true),
    (470.0, 70.0, true),
    (544.0, 44.0, true),
    (592.0, 66.0, true),
    (668.0, 52.0, false),
];

fn results(ui: &mut Ui, th: &Theme, cars: &[&ReportCar], colors: &BTreeMap<i32, Color32>, focus: &mut Option<i32>) {
    ui.add_space(28.0);
    ui.label(egui::RichText::new("Results").font(th.semi(15.0)).color(th.ink3));
    ui.add_space(6.0);
    let head = ["Pos", "Start", "Driver", "iR", "Laps", "Best", "Median", "Stops", "Pit lane", ""];
    let x0 = ui.cursor().min.x;
    let row_h = 26.0;
    let width = ui.available_width().min(ui::COLUMN);
    let draw = |p: &egui::Painter, y: f32, cells: &[String], font: &egui::FontId, color: Color32, x_off: f32| {
        for (i, text) in cells.iter().enumerate() {
            let (cx, cw, right) = COLS[i];
            let g = p.layout_no_wrap(text.clone(), font.clone(), color);
            let x = if right { x0 + cx + cw - g.size().x } else { x0 + cx + if i == 0 { x_off } else { 0.0 } };
            p.galley(Pos2::new(x, y + (row_h - g.size().y) / 2.0), g, color);
        }
    };
    let (hr, _) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::hover());
    draw(ui.painter(), hr.top(), &head.map(String::from), &th.semi(13.0), th.ink3, 0.0);
    ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, th.line));
    for c in cars {
        let s = rr::stats(&rr::lap_times(c).values().copied().collect::<Vec<_>>());
        let lane: f32 = c.pit_stops.iter().map(|p| p.lane_s).sum();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
        let p = ui.painter();
        if c.is_player {
            p.rect_filled(rect, 3.0, th.player);
        }
        if *focus == Some(c.car_idx) {
            p.rect_stroke(rect, 3.0, Stroke::new(1.0, th.ink3), StrokeKind::Inside);
        } else if resp.hovered() {
            p.rect_filled(rect, 3.0, crate::theme::alpha(th.ink, 0.04));
        }
        p.rect_filled(Rect::from_center_size(Pos2::new(rect.left() + 8.0, rect.center().y), Vec2::new(4.0, 14.0)), 1.0, colors[&c.car_idx]);
        let cells = [
            if c.finish_class_position > 0 { c.finish_class_position.to_string() } else { "–".into() },
            c.start_position.map(|s| s.to_string()).unwrap_or_else(|| "–".into()),
            format!("#{} {}", c.number, c.name),
            if c.irating > 0 { format!("{:.1}k", c.irating as f32 / 1000.0) } else { "–".into() },
            c.laps_completed.to_string(),
            lap_time(s.best.map(|b| b as f32)),
            lap_time(s.median.map(|b| b as f32)),
            c.stops.to_string(),
            if c.pit_stops.is_empty() { "–".into() } else { format!("{lane:.1} s") },
            if c.finished { "Finished".into() } else if c.out { "Out".into() } else { String::new() },
        ];
        let font = if c.is_player { th.bold(15.0) } else { th.medium(15.0) };
        // The driver name is cut to its column.
        let clip = p.with_clip_rect(rect);
        draw(&clip, rect.top(), &cells, &font, th.ink, 18.0);
        p.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, th.line));
        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            *focus = if *focus == Some(c.car_idx) { None } else { Some(c.car_idx) };
        }
    }
    ui.add_space(6.0);
    ui.add(egui::Label::new(ui::text(th, "Best and median are of the laps with both ends recorded. Pit lane is the total time between the pit lane lines.", 14.0, th.ink3)).wrap());
}

// --- Stints ------------------------------------------------------------------------------------

fn stint_chart(ui: &mut Ui, th: &Theme, cars: &[&ReportCar], compounds: &[String], colors: &BTreeMap<i32, Color32>, focus: &mut Option<i32>) {
    if cars.iter().all(|c| c.stints.is_empty()) {
        ui::note(ui, th, "Nothing recorded for this yet.");
        return;
    }
    let max_lap = cars.iter().flat_map(|c| c.stints.iter().map(|s| s.end)).fold(1.0f32, f32::max);
    // From the first lap anything was recorded, not lap 0.
    let first = cars.iter().flat_map(|c| c.stints.iter().map(|s| s.start)).fold(max_lap, f32::min);
    let min_lap = first.floor().min(max_lap - 1.0);
    let row = 22.0;
    let left = 170.0;
    let width = ui.available_width().min(ui::COLUMN);
    let w = width - left - 12.0;
    let height = cars.len() as f32 * row + 22.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let x = |l: f32| rect.left() + left + (l - min_lap) / (max_lap - min_lap) * w;
    let p = ui.painter().clone();
    for l in rr::int_ticks(min_lap as f64, max_lap as f64, 12) {
        let lx = x(l as f32);
        p.vline(lx, rect.top()..=rect.top() + cars.len() as f32 * row, Stroke::new(1.0, th.grid));
        p.text(Pos2::new(lx, rect.top() + cars.len() as f32 * row + 4.0), Align2::CENTER_TOP, format!("{l}"), th.medium(12.0), th.ink3);
    }
    for (k, c) in cars.iter().enumerate() {
        let top = rect.top() + k as f32 * row;
        let row_rect = Rect::from_min_size(Pos2::new(rect.left(), top), Vec2::new(width, row));
        let resp = ui.interact(row_rect, ui.scope_id().with(("stint-row", c.car_idx)), Sense::click());
        let dim = focus.is_some_and(|f| f != c.car_idx);
        let fade = |col: Color32| if dim { col.gamma_multiply(0.3) } else { col };
        if resp.hovered() {
            p.rect_filled(row_rect, 3.0, crate::theme::alpha(th.ink, 0.04));
        }
        p.rect_filled(Rect::from_min_size(Pos2::new(rect.left(), top + 4.0), Vec2::new(8.0, row - 8.0)), 2.0, fade(colors[&c.car_idx]));
        let name: String = format!("{}  #{} {}", if c.finish_class_position > 0 { c.finish_class_position.to_string() } else { "–".into() }, c.number, c.name).chars().take(26).collect();
        let font = if c.is_player { th.bold(13.0) } else { th.medium(13.0) };
        p.text(Pos2::new(rect.left() + 14.0, top + row / 2.0), Align2::LEFT_CENTER, name, font, fade(th.ink));
        if resp.clicked() {
            *focus = if *focus == Some(c.car_idx) { None } else { Some(c.car_idx) };
        }
        for (i, s) in c.stints.iter().enumerate() {
            let name = if s.compound >= 0 { compounds.get(s.compound as usize).map(String::as_str).unwrap_or("") } else { "" };
            let base = tyre_color(th, compound_kind(name));
            let sr = Rect::from_min_max(Pos2::new(x(s.start), top + 5.0), Pos2::new(x(s.end).max(x(s.start) + 1.5), top + row - 5.0));
            let alpha = if s.partial { 0.55 } else { 1.0 };
            p.rect_filled(sr, 3.0, fade(base.gamma_multiply(alpha)));
            if s.partial {
                p.rect_stroke(sr, 3.0, Stroke::new(1.0, fade(base)), StrokeKind::Inside);
            }
            let tip = format!("{}: {}{:.1} laps{}, laps {:.1}–{:.1}", c.name, if s.partial { "at least " } else { "" }, s.end - s.start, if name.is_empty() { String::new() } else { format!(" on {name}") }, s.start, s.end);
            ui.interact(sr.expand2(Vec2::new(0.0, 2.0)), ui.scope_id().with(("stint", c.car_idx, i)), Sense::hover()).on_hover_text(tip);
        }
        for (i, stop) in c.pit_stops.iter().enumerate() {
            let sx = x(stop.lap as f32 + 1.0);
            let r = Rect::from_min_max(Pos2::new(sx - 1.0, top + 2.0), Pos2::new(sx + 1.0, top + row - 2.0));
            p.rect_filled(r, 0.0, fade(th.ink));
            let tip = format!("{}: pit stop on lap {}, {:.1} s in the pit lane, {:.1} s stopped", c.name, stop.lap + 1, stop.lane_s, stop.stall_s);
            ui.interact(r.expand2(Vec2::new(3.0, 0.0)), ui.scope_id().with(("stop", c.car_idx, i)), Sense::hover()).on_hover_text(tip);
        }
    }
}

// --- Your laps ---------------------------------------------------------------------------------

/// Your laps as Tenths logged them: time, fuel used, incidents, track temperature.
fn your_laps(ui: &mut Ui, th: &Theme, v: &mut View) {
    let laps = v.report.player_laps.clone();
    let fuel: Vec<(i32, f32)> = laps.iter().filter_map(|l| l.fuel_used.map(|f| (l.lap, f))).collect();
    let total: f32 = fuel.iter().map(|f| f.1).sum();
    let help = format!("Litres used on each lap, measured from the tank. Laps with a stop or a refuel aren't measured. {} laps measured, {:.1} L in all.", fuel.len(), total);
    let rid = v.report.id.clone();
    let hover_before = v.lap_hover;
    let mut hover_now: Option<i32> = None;
    chart_section(ui, th, "Your fuel", Some(&help), None, |ui| {
        let bars: Vec<Bar> = fuel
            .iter()
            .map(|(lap, f)| {
                let hot = hover_before == Some(*lap);
                Bar::new(*lap as f64, *f as f64).width(0.8).fill(if hot { th.attention } else { th.lapped }).name("")
            })
            .collect();
        let max = fuel.iter().map(|f| f.1 as f64).fold(0.1, f64::max);
        let last = laps.iter().map(|l| l.lap).max().unwrap_or(1) as f64;
        let first = laps.iter().map(|l| l.lap).min().unwrap_or(1) as f64;
        let lap_info: BTreeMap<i32, Option<f32>> = laps.iter().map(|l| (l.lap, l.fuel_used)).collect();
        let resp = Plot::new(("fuel", &rid))
            .height(160.0)
            .width(ui.available_width().min(ui::COLUMN))
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_drag(false)
            .allow_boxed_zoom(false)
            .show_x(false)
            .show_y(false)
            .default_x_bounds(first - 0.6, last + 0.6)
            .default_y_bounds(0.0, max * 1.12)
            .x_axis_formatter(|m, _| if m.value.fract() == 0.0 { format!("{}", m.value as i64) } else { String::new() })
            .y_axis_formatter(|m, _| format!("{} L", (m.value * 10.0).round() / 10.0))
            .y_axis_min_width(46.0)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(BarChart::new("Fuel", bars));
                plot_ui.pointer_coordinate().map(|p| p.x.round() as i32)
            });
        if let Some(lap) = resp.inner.filter(|l| lap_info.contains_key(l)) {
            hover_now = Some(lap);
            let info = lap_info[&lap];
            resp.response.on_hover_text(match info {
                Some(f) => format!("Lap {lap}: {f:.2} L"),
                None => format!("Lap {lap}: not measured"),
            });
        }
    });

    let temps: Vec<(i32, f32)> = laps.iter().filter(|l| l.track_temp > 0.0).map(|l| (l.lap, l.track_temp)).collect();
    if temps.len() > 1 {
        chart_section(ui, th, "Track temperature", Some("At the end of each of your laps."), None, |ui| {
            let s = Series { id: 0, label: "Track".into(), color: th.attention, points: temps.iter().map(|(l, t)| (*l, *t as f64)).collect() };
            let fmt = |x: f64| format!("{x:.1}°");
            let c = Chart { key: format!("{rid}-temp"), invert: false, integer_y: false, format: &fmt, y_min: None, y_max: None, marks: &[], legend: false, height: 160.0 };
            charts::line_chart(ui, th, &[s], &c, &mut None);
        });
    }

    ui.add_space(28.0);
    ui.label(egui::RichText::new("Your laps").font(th.semi(15.0)).color(th.ink3));
    ui.add_space(6.0);
    let cols: [(f32, f32, bool); 7] = [(0.0, 40.0, false), (50.0, 80.0, true), (140.0, 70.0, true), (222.0, 70.0, true), (302.0, 64.0, true), (376.0, 64.0, true), (456.0, 260.0, false)];
    let x0 = ui.cursor().min.x;
    let width = ui.available_width().min(ui::COLUMN);
    let row_h = 26.0;
    let draw = |p: &egui::Painter, y: f32, cells: &[String], font: &egui::FontId, colors: &[Color32]| {
        for (i, t) in cells.iter().enumerate() {
            let (cx, cw, right) = cols[i];
            let g = p.layout_no_wrap(t.clone(), font.clone(), colors[i]);
            let x = if right { x0 + cx + cw - g.size().x } else { x0 + cx };
            p.galley(Pos2::new(x, y + (row_h - g.size().y) / 2.0), g, colors[i]);
        }
    };
    let (hr, _) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::hover());
    draw(ui.painter(), hr.top(), &["Lap", "Time", "Fuel", "Incidents", "Track", "Air", ""].map(String::from), &th.semi(13.0), &[th.ink3; 7]);
    ui.painter().hline(hr.x_range(), hr.bottom(), Stroke::new(1.0, th.line));
    for l in &laps {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, row_h), Sense::hover());
        if resp.hovered() {
            hover_now = Some(l.lap);
        }
        let p = ui.painter();
        if hover_before == Some(l.lap) {
            p.rect_filled(rect, 3.0, th.player);
        }
        let flags = [l.pit.then_some("Pit"), l.off_track.then_some("Off track"), (!l.clean && !l.pit && !l.off_track && l.time.is_some()).then_some("Not clean")].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let cells = [
            l.lap.to_string(),
            lap_time(l.time),
            l.fuel_used.map(|f| format!("{f:.2} L")).unwrap_or_else(|| "–".into()),
            if l.incidents > 0 { format!("{}x", l.incidents) } else { String::new() },
            if l.track_temp > 0.0 { format!("{:.1}°", l.track_temp) } else { String::new() },
            if l.air_temp > 0.0 { format!("{:.1}°", l.air_temp) } else { String::new() },
            flags,
        ];
        let mut colors = [th.ink; 7];
        if l.personal_best {
            colors[1] = th.gain;
        }
        colors[6] = th.ink3;
        draw(p, rect.top(), &cells, &th.medium(15.0), &colors);
        p.hline(rect.x_range(), rect.bottom(), Stroke::new(1.0, th.line));
    }
    v.lap_hover = hover_now;
    // Something to repaint on while the mouse moves between the table and the chart.
    if hover_now != hover_before {
        ui.ctx().request_repaint();
    }
    let _ = (Line::new("", PlotPoints::from(vec![[0.0, 0.0]])), PlotPoint::new(0.0, 0.0));
}
