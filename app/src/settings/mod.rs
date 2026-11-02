//! The Settings window: a second, normal egui window (an immediate viewport
//! of the app) with the panes of `src/settings/Settings.tsx`: General, Races,
//! Layouts, Appearance, Controls, Streaming, About, and one per widget.
//!
//! It edits `config.json` exactly like the Tauri app's Settings (`edits.rs`),
//! so the overlay and the Tauri app pick the changes up from the file.
//!
//! Integration points for the other parts of the app (see the report):
//! * `SettingsWindow::capturing()`: true while it waits for a key or wheel
//!   button; the hotkey service suspends itself meanwhile (`suspend_controls`).
//! * `SettingsWindow::feed_button`, `set_controllers`: the controls service
//!   passes in wheel button presses and the plugged-in devices.
//! * `Env::displays`: the monitors (name, key), for each widget's Screen field.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use egui::{Color32, Margin, RichText, Sense, Stroke, Ui, Vec2, ViewportBuilder, ViewportId};
use telemetry::runner::RunnerControl;

use crate::config::{self, General, Store};
use crate::data::Live;
use crate::schema;
use crate::shell::{Request, Shell};
use crate::theme::Theme;

mod about;
mod appearance;
mod charts;
mod controls;
pub mod edits;
mod general;
pub mod layouts_io;
mod layouts;
mod races;
pub mod reports_io;
pub mod specs;
mod timefmt;
pub mod ui;
mod widget;

use ui::Look;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    General,
    Races,
    Layouts,
    Appearance,
    Controls,
    Streaming,
    About,
    Widget(&'static str),
}

impl Pane {
    /// `--pane` on the command line: a page name or a widget's id.
    pub fn parse(s: &str) -> Option<Pane> {
        Some(match s {
            "general" => Pane::General,
            "races" => Pane::Races,
            "layouts" => Pane::Layouts,
            "appearance" => Pane::Appearance,
            "controls" => Pane::Controls,
            "streaming" => Pane::Streaming,
            "about" => Pane::About,
            id => Pane::Widget(schema::WIDGETS.iter().find(|m| m.id == id)?.id),
        })
    }
}

/// Everything the window needs from the app, each frame.
pub struct Env<'a> {
    pub store: &'a mut Store,
    pub live: &'a Live,
    pub control: &'a RunnerControl,
    pub shell: &'a Shell,
    /// The settings in force: the config's, over the command line's theme and units.
    pub general: &'a General,
    pub editing: bool,
    pub overlays_visible: bool,
    /// The monitors as `(key, name)`, primary first. Empty or one: no Screen field.
    pub displays: &'a [(String, String)],
}

/// Results of work done off the UI thread (file pickers, reading big lap files).
pub enum Msg {
    PickedLapFile(Option<PathBuf>),
    LapSummary(Result<(PathBuf, telemetry::ibt::IbtSummary), String>),
    ReferenceLoaded(Result<(), String>),
    ImportLayout(Option<PathBuf>),
    ExportLayout(String, Option<PathBuf>),
}

pub struct Tasks {
    pub tx: Sender<Msg>,
    rx: Receiver<Msg>,
}

pub struct ControllerInfo {
    pub device: String,
    pub name: String,
}

/// What `--settings`, `--pane` and `--report` ask for.
#[derive(Default, Clone)]
pub struct Launch {
    pub open: bool,
    pub pane: Option<Pane>,
    pub report: Option<String>,
}

pub struct SettingsWindow {
    pub open: bool,
    pane: Pane,
    editor: edits::Editor,
    tasks: Tasks,
    focus: bool,
    notice_closed: bool,
    /// Dev: `TENTHS_SETTINGS_SCROLL=<px>` starts the pane scrolled (for screenshots of long pages).
    scroll_y: Option<f32>,
    controls: controls::State,
    layouts: layouts::State,
    reference: widget::RefState,
    races: races::State,
}

const ID: &str = "tenths-settings";

impl SettingsWindow {
    pub fn new(shell: &Shell, launch: Launch) -> Self {
        let (tx, rx) = channel();
        let mut s = Self {
            open: launch.open,
            pane: launch.pane.unwrap_or(Pane::General),
            editor: edits::Editor::new(shell.paths.config.clone(), shell.paths.layouts_dir()),
            tasks: Tasks { tx, rx },
            focus: false,
            notice_closed: false,
            scroll_y: std::env::var("TENTHS_SETTINGS_SCROLL").ok().and_then(|v| v.parse().ok()),
            controls: Default::default(),
            layouts: Default::default(),
            reference: Default::default(),
            races: Default::default(),
        };
        if let Some(id) = launch.report {
            s.races.open_report(&shell.paths, &id);
            s.pane = Pane::Races;
        }
        s
    }

    /// Opens the window, or brings it to the front, optionally on a page.
    pub fn show_pane(&mut self, pane: Option<Pane>) {
        self.open = true;
        self.focus = true;
        if let Some(p) = pane {
            self.pane = p;
        }
    }

    /// Waiting for a key or a button: the hotkeys and wheel buttons should do nothing meanwhile.
    pub fn capturing(&self) -> bool {
        self.open && self.controls.capturing()
    }

    /// A wheel or button-box button was pressed (from the controls service).
    pub fn feed_button(&mut self, press: crate::logic::settings_bindings::ButtonBinding) {
        self.controls.pending_button = Some(press);
    }

    /// The wheels and button boxes plugged in now (`None`: not known).
    pub fn set_controllers(&mut self, list: Option<Vec<ControllerInfo>>) {
        self.controls.controllers = list;
    }

    /// Draws the window if it's open. Returns what it asks of the overlay window.
    pub fn show(&mut self, ctx: &egui::Context, mut env: Env<'_>) -> Vec<Request> {
        let mut out = Vec::new();
        if !self.open {
            return out;
        }
        let th = Theme::build(env.general.theme, env.general.colors.get(&env.general.theme));
        let builder = ViewportBuilder::default()
            .with_title("Tenths settings")
            .with_inner_size([1100.0, std::env::var("TENTHS_SETTINGS_HEIGHT").ok().and_then(|v| v.parse().ok()).unwrap_or(780.0)])
            .with_min_inner_size([760.0, 480.0]);
        let id = ViewportId::from_hash_of(ID);
        if std::mem::take(&mut self.focus) {
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
        }
        ctx.show_viewport_immediate(id, builder, |ui, _class| {
            if ui.ctx().input(|i| i.viewport().close_requested()) {
                self.open = false;
                self.controls.cancel();
                return;
            }
            ui::apply_style(ui, &th);
            self.frame(ui, &th, &mut env, &mut out);
        });
        out
    }

    fn frame(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>, out: &mut Vec<Request>) {
        self.editor.refresh(env.store);
        self.drain_tasks(env);
        self.top_bar(ui, th, env, out);
        self.nav(ui, th, env);
        let body = ui::body_color(th);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(body).inner_margin(Margin { left: 36, right: 36, top: 26, bottom: 48 }))
            .show(ui, |ui| {
                let mut area = egui::ScrollArea::vertical().id_salt(("pane", format!("{:?}", self.pane))).auto_shrink([false, false]);
                if let Some(y) = self.scroll_y.take() {
                    area = area.vertical_scroll_offset(y);
                }
                area.show(ui, |ui| {
                    ui.set_max_width(ui::COLUMN);
                    self.pane_body(ui, th, env);
                });
            });
        // Writes wait for the mouse button to come up, so a slider drag is one write.
        let down = ui.ctx().input(|i| i.pointer.any_down());
        self.editor.flush(down);
        // Live effects of settings that the runner holds.
        env.control.save_reports.store(self.editor.raw["general"]["saveReports"].as_bool().unwrap_or(true), std::sync::atomic::Ordering::Relaxed);
        if self.controls.capturing() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn drain_tasks(&mut self, env: &mut Env<'_>) {
        while let Ok(msg) = self.tasks.rx.try_recv() {
            match msg {
                Msg::PickedLapFile(p) => self.reference.picked(p, &self.tasks.tx),
                Msg::LapSummary(r) => self.reference.summary(r),
                Msg::ReferenceLoaded(r) => self.reference.loaded(r),
                Msg::ImportLayout(p) => self.layouts.imported(p, &mut self.editor, env),
                Msg::ExportLayout(id, p) => self.layouts.exported(&id, p, env),
            }
        }
    }

    fn pane_body(&mut self, ui: &mut Ui, th: &Theme, env: &mut Env<'_>) {
        // The settings file couldn't be read at start, so Tenths started fresh.
        if let Some(kept) = env.shell.config_notice.as_deref().filter(|_| !self.notice_closed) {
            let msg = format!("Your settings file couldn't be read, so Tenths started fresh. The old file was kept as {kept}.");
            if ui::notice(ui, th, &msg, true) {
                self.notice_closed = true;
            }
        }
        if let Some(e) = &self.editor.error {
            ui::notice(ui, th, e, false);
        }
        match self.pane {
            Pane::General => {
                if general::show(ui, th, env, &mut self.editor) {
                    self.pane = Pane::Controls;
                }
            }
            Pane::Races => self.races.show(ui, th, env, &mut self.editor),
            Pane::Layouts => self.layouts.show(ui, th, env, &mut self.editor, &self.tasks.tx),
            Pane::Appearance => appearance::show(ui, th, env, &mut self.editor),
            Pane::Controls => self.controls.show(ui, th, env, &mut self.editor),
            Pane::Streaming => about::streaming(ui, th),
            Pane::About => about::show(ui, th, env, &mut self.editor),
            Pane::Widget(id) => widget::show(ui, th, env, &mut self.editor, &mut self.reference, &self.tasks.tx, id),
        }
    }

    fn top_bar(&mut self, ui: &mut Ui, th: &Theme, env: &Env<'_>, out: &mut Vec<Request>) {
        let (label, tone) = match env.live.status.as_deref() {
            None => ("Starting".to_string(), th.attention),
            Some(s) if s.connected && s.source == telemetry::model::SourceKind::Demo => ("Demo race running".into(), th.fastest),
            Some(s) if s.connected => ("Connected to iRacing".into(), th.gain),
            Some(s) if !s.message.is_empty() => (s.message.clone(), th.attention),
            Some(_) => ("Waiting for iRacing".into(), th.attention),
        };
        egui::Panel::top("settings-topbar")
            .exact_size(56.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(th.panel_solid).inner_margin(Margin { left: 18, right: 16, top: 0, bottom: 0 }))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, th.line));
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    brand(ui, th);
                    ui.add_space(4.0);
                    let (dot, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.0, tone);
                    ui.add_space(-8.0);
                    ui.label(ui::text(th, label, 15.0, th.ink2));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        let edit = if env.editing { "Finish editing" } else { "Edit layout" };
                        let look = Look::Primary;
                        if ui::button(ui, th, edit, look).clicked() {
                            out.push(Request::ToggleEdit);
                        }
                        let hide = if env.overlays_visible { "Hide overlays" } else { "Show overlays" };
                        if ui::button(ui, th, hide, Look::Normal).clicked() {
                            out.push(Request::ToggleOverlays);
                        }
                    });
                });
            });
    }

    fn nav(&mut self, ui: &mut Ui, th: &Theme, env: &Env<'_>) {
        egui::Panel::left("settings-nav")
            .exact_size(220.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(th.panel_solid.gamma_multiply(1.0)).inner_margin(Margin { left: 10, right: 10, top: 14, bottom: 14 }))
            .show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().vline(r.right() + 10.0, r.y_range(), Stroke::new(1.0, th.line));
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let items: [(&str, Pane, bool); 7] = [
                        ("General", Pane::General, false),
                        ("Races", Pane::Races, false),
                        ("Layouts", Pane::Layouts, false),
                        ("Appearance", Pane::Appearance, false),
                        ("Controls", Pane::Controls, false),
                        ("Streaming", Pane::Streaming, true),
                        ("About", Pane::About, false),
                    ];
                    for (label, pane, exp) in items {
                        if self.nav_item(ui, th, label, pane, exp, None) {
                            self.pane = pane;
                        }
                    }
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(ui::text(th, "Widgets", 13.0, th.ink3));
                    });
                    ui.add_space(6.0);
                    for s in specs::SPECS {
                        let name = schema::meta(s.id).map(|m| m.name).unwrap_or(s.id);
                        let on = env.store.config.widget(s.id).enabled;
                        if self.nav_item(ui, th, name, Pane::Widget(s.id), s.experimental, Some(on)) {
                            self.pane = Pane::Widget(s.id);
                        }
                    }
                });
            });
    }

    /// One row of the left list (`.nav-item`). `dot`: a widget's shown/hidden dot.
    fn nav_item(&self, ui: &mut Ui, th: &Theme, label: &str, pane: Pane, experimental: bool, dot: Option<bool>) -> bool {
        let (rect, r) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click());
        let active = self.pane == pane;
        let p = ui.painter();
        if active {
            p.rect_filled(rect, 5.0, th.panel_raised);
            p.rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())), 0.0, th.ink);
        } else if r.hovered() {
            p.rect_filled(rect, 5.0, crate::theme::alpha(th.ink, 0.04));
        }
        let fg = if active || r.hovered() { th.ink } else { th.ink2 };
        let g = p.layout_no_wrap(label.to_string(), th.medium(16.0), fg);
        let text_w = g.size().x;
        p.galley(egui::pos2(rect.left() + 10.0, rect.center().y - g.size().y / 2.0), g, fg);
        if experimental {
            let tag = p.layout_no_wrap("EXP".into(), th.bold(10.0), th.attention);
            let pos = egui::pos2(rect.left() + 10.0 + text_w + 6.0, rect.center().y - tag.size().y / 2.0);
            p.galley(pos, tag, th.attention);
        }
        if let Some(on) = dot {
            let c = egui::pos2(rect.right() - 10.0, rect.center().y);
            if on {
                p.circle_filled(c, 3.5, th.gain);
            } else {
                p.circle_stroke(c, 3.0, Stroke::new(1.0, th.ink3));
            }
        }
        r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
    }
}

/// The brand mark: three slanted bars, the last one in the accent colour.
fn brand(ui: &mut Ui, th: &Theme) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::hover());
    let s = rect.width() / 64.0;
    let skew = (-14.0f32).to_radians().tan();
    let bar = |x: f32, y: f32, w: f32, h: f32, color: Color32| {
        // `skewX(-14) translate(9 0)` in a 64 box.
        let pt = |px: f32, py: f32| egui::pos2(rect.left() + ((px + 9.0) + skew * (py - 0.0) * 1.0) * s, rect.top() + py * s);
        let pts = vec![pt(x, y), pt(x + w, y), pt(x + w, y + h), pt(x, y + h)];
        ui.painter().add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
    };
    bar(14.0, 17.0, 8.0, 30.0, th.ink);
    bar(27.0, 17.0, 8.0, 30.0, th.ink);
    bar(40.0, 33.0, 8.0, 14.0, th.fastest);
    ui.add_space(2.0);
    ui.label(RichText::new("Tenths").font(th.bold(20.0)).color(th.ink));
}

/// `widget::OptionField`'s write: a JSON number that is a whole number when it is one.
pub fn number_json(v: f64) -> serde_json::Value {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        serde_json::json!(v as i64)
    } else {
        serde_json::json!(v)
    }
}

/// The overlay config's own copy of what a pane edits, for panes that only read.
pub fn cfg<'a>(env: &'a Env<'_>) -> &'a config::Config {
    &env.store.config
}
