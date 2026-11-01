//! Tenths, drawn natively with egui, on the same telemetry core as the Tauri
//! app, reading the Tauri app's own `config.json` for which widgets are on,
//! where they are, their options, theme and units.
//!
//! One transparent, always-on-top window over the primary display. Clicks go
//! through to iRacing except while editing the layout.
//!
//!   tenths-egui                   iRacing (Windows), falls back to waiting
//!   tenths-egui --demo            the demo race
//!   tenths-egui --offline         no data at all (empty states)
//!   tenths-egui --edit            start in edit mode (drag the panels)
//!   tenths-egui --config <path>   another config.json than the Tauri app's
//!   tenths-egui --theme f1        dark, light or f1, over the config's
//!   tenths-egui --imperial        imperial units, over the config's
//!   tenths-egui --bench 600       print memory and frame stats every 10 s, quit after 600 s
//!   tenths-egui --shot out.png    save a screenshot after 5 s (--shot-after N) and quit
//!
//! Ctrl+Shift+E (Windows, anywhere) or F2 (in edit mode) toggles editing.
//! Ctrl+Shift+Q quits.

// While the port is under way some helpers wait for the panels that use them.
#![allow(dead_code)]

mod anchor;
mod config;
mod data;
mod frame;
mod kit;
mod logic;
mod ratings;
mod schema;
mod svg;
mod sys;
mod theme;
mod widgets;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, Id, LayerId, Order, Pos2, Rect, Stroke, Vec2};

struct Args {
    demo: bool,
    offline: bool,
    edit: bool,
    imperial: bool,
    theme: Option<config::ThemeId>,
    config: Option<PathBuf>,
    bench: Option<f64>,
    shot: Option<String>,
    shot_after: f64,
}

fn args() -> Args {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| all.iter().position(|a| a == name).and_then(|i| all.get(i + 1)).cloned();
    let has = |name: &str| all.iter().any(|a| a == name);
    Args {
        demo: has("--demo") || cfg!(not(windows)),
        offline: has("--offline"),
        edit: has("--edit"),
        imperial: has("--imperial"),
        theme: value("--theme").and_then(|t| config::ThemeId::parse(&t)),
        config: value("--config").map(PathBuf::from),
        bench: value("--bench").and_then(|v| v.parse().ok()),
        shot: value("--shot"),
        shot_after: value("--shot-after").and_then(|v| v.parse().ok()).unwrap_or(5.0),
    }
}

fn main() -> eframe::Result {
    let args = args();
    let viewport = egui::ViewportBuilder::default()
        .with_title("Tenths (egui)")
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_resizable(false)
        .with_position([0.0, 0.0])
        // Resized to the whole display on the first frame, once the monitor is known.
        .with_inner_size([1920.0, 1080.0])
        .with_mouse_passthrough(!args.edit);
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("tenths-egui", options, Box::new(move |cc| Ok(Box::new(App::new(cc, args)))))
}

struct App {
    shared: data::Shared,
    store: config::Store,
    ratings: ratings::Ratings,
    layout: frame::Layout,
    editing: bool,
    imperial: bool,
    theme: Option<config::ThemeId>,
    sized: bool,
    started: Instant,
    bench: Option<f64>,
    shot: Option<String>,
    shot_after: f64,
    shot_asked: bool,
    shot_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    picker: bool,
    stats: Stats,
    #[cfg(windows)]
    hotkeys: Option<hotkeys::Hotkeys>,
}

/// Frame timing and memory, averaged over a second.
#[derive(Default)]
struct Stats {
    window_start: Option<Instant>,
    frames: u32,
    cpu: Duration,
    ticks_at_start: u64,
    // Last second's results.
    fps: f32,
    tick_hz: f32,
    cpu_ms: f32,
    memory_mb: f32,
    last_print: Option<Instant>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let path = args.config.clone().or_else(config::default_path);
        let store = config::Store::open(path);
        // The Tauri app's "demo" setting counts too.
        let demo = args.demo || store.config.general.demo;
        let (shared, _control) = data::start(cc.egui_ctx.clone(), demo, args.offline);
        Self {
            shared,
            store,
            ratings: ratings::Ratings::open(config::data_dir()),
            layout: frame::Layout::new(),
            editing: args.edit,
            imperial: args.imperial,
            theme: args.theme,
            sized: false,
            started: Instant::now(),
            bench: args.bench,
            shot: args.shot,
            shot_after: args.shot_after,
            shot_asked: false,
            shot_done: Default::default(),
            picker: false,
            stats: Stats::default(),
            #[cfg(windows)]
            hotkeys: hotkeys::Hotkeys::register(),
        }
    }

    fn set_editing(&mut self, ctx: &egui::Context, on: bool) {
        self.editing = on;
        self.picker = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!on));
        if on {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    fn general(&self) -> config::General {
        let mut g = self.store.config.general.clone();
        if let Some(t) = self.theme {
            g.theme = t;
        }
        if self.imperial {
            g.units = config::Units::Imperial;
        }
        g
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        let frame_start = Instant::now();

        // Cover the whole primary display once its size is known.
        if !self.sized {
            if let Some(size) = ctx.input(|i| i.viewport().monitor_size) {
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::ZERO));
                self.sized = true;
            }
        }

        #[cfg(windows)]
        if let Some(action) = self.hotkeys.as_ref().and_then(|h| h.poll()) {
            match action {
                hotkeys::Action::ToggleEdit => self.set_editing(ctx, !self.editing),
                hotkeys::Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
        if self.editing && ctx.input(|i| i.key_pressed(egui::Key::F2) || i.key_pressed(egui::Key::Escape)) {
            self.set_editing(ctx, false);
        }

        // Settings saved by the Tauri app, and new driver ratings.
        self.store.poll();
        self.ratings.poll();
        let general = self.general();
        let th = theme::Theme::from_general(&general);

        let live = data::Live::take(&self.shared);
        let on_track = live.tick.as_ref().is_some_and(|t| t.is_on_track);
        let show = self.editing || (live.connected() && (general.show_when == config::ShowWhen::Connected || on_track));
        let screen = ctx.content_rect();
        if self.editing {
            edit_backdrop(ctx, screen);
        }
        let now = self.started.elapsed().as_secs_f64();
        self.layout.run(frame::Frame {
            ctx,
            store: &mut self.store,
            th: &th,
            live: &live,
            ratings: &self.ratings,
            units: general.units,
            editing: self.editing,
            show,
            now,
            screen,
        });

        self.stats(ctx, live.ticks, frame_start, live.status.as_deref());
        if self.editing {
            self.edit_bar(ctx, &th, live.status.as_deref());
        }

        // Screenshot mode (for the README and checks without a screen).
        if let Some(path) = self.shot.clone() {
            if !self.shot_asked && self.started.elapsed().as_secs_f64() >= self.shot_after {
                self.shot_asked = true;
                let done = self.shot_done.clone();
                let repaint = ctx.clone();
                ctx.request_screenshot(move |img| {
                    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                    if let Some(buf) = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, rgba) {
                        match buf.save(&path) {
                            Ok(()) => println!("saved {path}"),
                            Err(e) => eprintln!("couldn't save {path}: {e}"),
                        }
                    }
                    done.store(true, std::sync::atomic::Ordering::Relaxed);
                    repaint.request_repaint();
                });
            }
            if self.shot_done.load(std::sync::atomic::Ordering::Relaxed) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }

        if self.bench.is_some_and(|secs| self.started.elapsed().as_secs_f64() >= secs) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Keep looking for hotkeys and config changes even when no telemetry arrives.
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

/// The dimmed, dotted backdrop while editing (`.overlay.is-editing`).
fn edit_backdrop(ctx: &egui::Context, screen: Rect) {
    let p = ctx.layer_painter(LayerId::new(Order::Background, Id::unique("tenths-edit-backdrop")));
    p.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(4, 7, 11, 97));
    let dot = Color32::from_rgba_unmultiplied(233, 238, 243, 31);
    let mut y = screen.top();
    while y < screen.bottom() {
        let mut x = screen.left();
        while x < screen.right() {
            p.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::splat(1.5)), 0.0, dot);
            x += 24.0;
        }
        y += 24.0;
    }
}

impl App {
    fn stats(&mut self, ctx: &egui::Context, ticks: u64, frame_start: Instant, status: Option<&telemetry::model::StatusPayload>) {
        let s = &mut self.stats;
        let now = Instant::now();
        let start = *s.window_start.get_or_insert(now);
        if s.frames == 0 {
            s.ticks_at_start = ticks;
        }
        s.frames += 1;
        s.cpu += now - frame_start;
        let elapsed = (now - start).as_secs_f32();
        if elapsed >= 1.0 {
            s.fps = s.frames as f32 / elapsed;
            s.tick_hz = (ticks - s.ticks_at_start) as f32 / elapsed;
            s.cpu_ms = s.cpu.as_secs_f32() * 1000.0 / s.frames as f32;
            s.memory_mb = sys::memory_bytes().map(|b| b as f32 / 1_048_576.0).unwrap_or(0.0);
            s.window_start = Some(now);
            s.frames = 0;
            s.cpu = Duration::ZERO;
        }
        if self.bench.is_some() && s.last_print.is_none_or(|t| now - t >= Duration::from_secs(10)) && s.fps > 0.0 {
            s.last_print = Some(now);
            println!(
                "{:>5.0}s  memory {:>6.1} MB  {:>5.1} fps  telemetry {:>5.1} Hz  UI {:.2} ms/frame  {}",
                self.started.elapsed().as_secs_f64(),
                s.memory_mb,
                s.fps,
                s.tick_hz,
                s.cpu_ms,
                status.map(|s| s.message.as_str()).unwrap_or("")
            );
        }
        let _ = ctx;
    }

    fn edit_bar(&mut self, ctx: &egui::Context, th: &theme::Theme, status: Option<&telemetry::model::StatusPayload>) {
        let s = &self.stats;
        let mut done = false;
        let mut quit = false;
        let mut toggle: Option<(&'static str, bool)> = None;
        let screen = ctx.content_rect();
        let enabled = schema::WIDGETS.iter().filter(|m| self.store.config.widget(m.id).enabled).count();
        let button = |text: &str, fill: Color32, fg: Color32| {
            egui::Button::new(egui::RichText::new(text).font(th.bold(15.0)).color(fg)).fill(fill).corner_radius(5.0).min_size(Vec2::new(0.0, 28.0))
        };
        egui::Area::new(Id::unique("edit-bar"))
            .pivot(Align2::CENTER_TOP)
            .fixed_pos(Pos2::new(screen.center().x, screen.top() + screen.height() * 0.4))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(th.panel_solid)
                    .stroke(Stroke::new(1.0, th.line_strong))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin { left: 16, right: 8, top: 8, bottom: 8 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 16.0;
                            ui.label(egui::RichText::new("Drag to move. Drag the corner, or use − and +, to resize.").font(th.medium(15.0)).color(th.ink2));
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:.0} MB · {:.0} fps · telemetry {:.0} Hz · UI {:.2} ms · {}",
                                    s.memory_mb,
                                    s.fps,
                                    s.tick_hz,
                                    s.cpu_ms,
                                    status.map(|s| s.message.as_str()).unwrap_or("starting")
                                ))
                                .font(th.medium(14.0))
                                .color(th.ink3),
                            );
                            if let Some(err) = &self.store.error {
                                ui.label(egui::RichText::new(err).font(th.semi(14.0)).color(th.loss));
                            }
                            let label = format!("Widgets {enabled}/{}", schema::WIDGETS.len());
                            if ui.add(button(&label, th.panel_raised, th.ink)).clicked() {
                                self.picker = !self.picker;
                            }
                            done = ui.add(button("Done", th.ink, th.panel_solid)).clicked();
                            quit = ui.add(button("Quit", th.panel_raised, th.ink)).clicked();
                        });
                        if self.picker {
                            ui.add_space(6.0);
                            egui::Grid::new("widget-picker").num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
                                for (i, m) in schema::WIDGETS.iter().enumerate() {
                                    let on = self.store.config.widget(m.id).enabled;
                                    let text = format!("{} {}", if on { "☑" } else { "☐" }, m.name);
                                    let color = if on { th.ink } else { th.ink3 };
                                    let b = egui::Button::new(egui::RichText::new(text).font(th.semi(15.0)).color(color)).frame(false);
                                    if ui.add(b).clicked() {
                                        toggle = Some((m.id, !on));
                                    }
                                    if i % 3 == 2 {
                                        ui.end_row();
                                    }
                                }
                            });
                        }
                    });
            });
        if let Some((id, on)) = toggle {
            self.store.save_widget(id, &[("enabled", serde_json::json!(on))]);
        }
        if done {
            self.set_editing(ctx, false);
        }
        if quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

#[cfg(windows)]
mod hotkeys {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

    pub enum Action {
        ToggleEdit,
        Quit,
    }

    pub struct Hotkeys {
        _manager: GlobalHotKeyManager,
        edit: u32,
        quit: u32,
    }

    impl Hotkeys {
        /// Ctrl+Shift+E and Ctrl+Shift+Q, system-wide. None if they're taken
        /// (the Tauri app uses Ctrl+Shift+E too: don't run both at once).
        pub fn register() -> Option<Self> {
            let manager = GlobalHotKeyManager::new().ok()?;
            let edit = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyE);
            let quit = HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyQ);
            if let Err(e) = manager.register(edit) {
                eprintln!("couldn't register Ctrl+Shift+E: {e}");
            }
            if let Err(e) = manager.register(quit) {
                eprintln!("couldn't register Ctrl+Shift+Q: {e}");
            }
            Some(Self { _manager: manager, edit: edit.id(), quit: quit.id() })
        }

        pub fn poll(&self) -> Option<Action> {
            while let Ok(e) = GlobalHotKeyEvent::receiver().try_recv() {
                if e.state != HotKeyState::Pressed {
                    continue;
                }
                if e.id == self.edit {
                    return Some(Action::ToggleEdit);
                }
                if e.id == self.quit {
                    return Some(Action::Quit);
                }
            }
            None
        }
    }
}
