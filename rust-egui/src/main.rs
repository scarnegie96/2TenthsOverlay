//! Tenths, drawn natively with egui: a prototype with three widgets
//! (Standings, Relative, Inputs) on the same telemetry core as the Tauri app.
//!
//! One transparent, always-on-top window over the primary display. Clicks go
//! through to iRacing except while editing the layout.
//!
//!   tenths-egui                 iRacing (Windows), falls back to waiting
//!   tenths-egui --demo          the demo race
//!   tenths-egui --edit          start in edit mode (drag the panels)
//!   tenths-egui --imperial      mph
//!   tenths-egui --bench 600     print memory and frame stats every 10 s, quit after 600 s
//!   tenths-egui --shot out.png  save a screenshot after 5 s (--shot-after N) and quit
//!
//! Ctrl+Shift+E (Windows, anywhere) or F2 (in edit mode) toggles editing.
//! Ctrl+Shift+Q quits.

mod data;
mod sys;
mod theme;
mod widgets;

use std::time::{Duration, Instant};

use egui::{Align2, Color32, Id, Pos2, Rect, Stroke, Vec2};

struct Args {
    demo: bool,
    edit: bool,
    imperial: bool,
    bench: Option<f64>,
    shot: Option<String>,
    shot_after: f64,
}

fn args() -> Args {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| all.iter().position(|a| a == name).and_then(|i| all.get(i + 1)).cloned();
    Args {
        demo: all.iter().any(|a| a == "--demo") || cfg!(not(windows)),
        edit: all.iter().any(|a| a == "--edit"),
        imperial: all.iter().any(|a| a == "--imperial"),
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
    editing: bool,
    imperial: bool,
    sized: bool,
    started: Instant,
    bench: Option<f64>,
    shot: Option<String>,
    shot_after: f64,
    shot_asked: bool,
    shot_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
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
        let (shared, _control) = data::start(cc.egui_ctx.clone(), args.demo);
        Self {
            shared,
            editing: args.edit,
            imperial: args.imperial,
            sized: false,
            started: Instant::now(),
            bench: args.bench,
            shot: args.shot,
            shot_after: args.shot_after,
            shot_asked: false,
            shot_done: Default::default(),
            stats: Stats::default(),
            #[cfg(windows)]
            hotkeys: hotkeys::Hotkeys::register(),
        }
    }

    fn set_editing(&mut self, ctx: &egui::Context, on: bool) {
        self.editing = on;
        ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(!on));
        if on {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
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

        // Take what's needed and let go of the lock before drawing.
        let (tick, field, session, status, trace, ticks) = {
            let l = self.shared.lock().unwrap();
            (l.tick.clone(), l.field.clone(), l.session.clone(), l.status.clone(), l.trace.iter().copied().collect::<Vec<_>>(), l.ticks)
        };
        let connected = status.as_ref().is_some_and(|s| s.connected);
        let show = self.editing || connected;
        let _ = session;

        if show {
            let field = field.as_deref();
            panel(ctx, "standings", Pos2::new(40.0, 40.0), widgets::standings::size(field), self.editing, |p, r| {
                widgets::standings::draw(p, r, field)
            });
            panel(ctx, "relative", Pos2::new(40.0, 560.0), widgets::relative::SIZE, self.editing, |p, r| {
                widgets::relative::draw(p, r, field)
            });
            let imperial = self.imperial;
            panel(ctx, "inputs", Pos2::new(560.0, 860.0), widgets::inputs::SIZE, self.editing, |p, r| {
                widgets::inputs::draw(p, r, tick.as_deref(), &trace, imperial)
            });
        }

        self.stats(ctx, ticks, frame_start, status.as_deref());
        if self.editing {
            self.edit_bar(ctx, status.as_deref());
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
        // Keep looking for hotkeys even when no telemetry arrives.
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

/// One widget: an egui Area at a remembered position (draggable while
/// editing), drawn by `draw` into a rect of `size`.
fn panel(ctx: &egui::Context, id: &str, default: Pos2, size: Vec2, editing: bool, draw: impl FnOnce(&egui::Painter, Rect)) {
    egui::Area::new(Id::unique(id))
        .default_pos(default)
        .movable(editing)
        .constrain(true)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
            draw(ui.painter(), rect);
            if editing {
                let color = if response.hovered() { theme::ACCENT } else { theme::ACCENT.gamma_multiply(0.6) };
                ui.painter().rect_stroke(rect.expand(3.0), theme::RADIUS + 2.0, Stroke::new(1.5, color), egui::StrokeKind::Outside);
                ui.painter().text(rect.left_top() - Vec2::new(-2.0, 6.0), Align2::LEFT_BOTTOM, id, theme::bold(12.0), theme::ACCENT);
            }
        });
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

    fn edit_bar(&mut self, ctx: &egui::Context, status: Option<&telemetry::model::StatusPayload>) {
        let s = &self.stats;
        let mut done = false;
        let mut quit = false;
        egui::Area::new(Id::unique("edit-bar"))
            .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 16.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_premultiplied(18, 21, 28, 240))
                    .stroke(Stroke::new(1.0, theme::ACCENT))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Tenths egui prototype").font(theme::bold(15.0)).color(theme::INK));
                            ui.label(egui::RichText::new("Drag panels to move them.").font(theme::medium(14.0)).color(theme::INK2));
                            ui.separator();
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:.0} MB · {:.0} fps · telemetry {:.0} Hz · UI {:.2} ms/frame · {}",
                                    s.memory_mb,
                                    s.fps,
                                    s.tick_hz,
                                    s.cpu_ms,
                                    status.map(|s| s.message.as_str()).unwrap_or("starting")
                                ))
                                .font(theme::medium(14.0))
                                .color(theme::INK2),
                            );
                            ui.separator();
                            done = ui.button(egui::RichText::new("Done").font(theme::bold(14.0))).clicked();
                            quit = ui.button(egui::RichText::new("Quit").font(theme::bold(14.0))).clicked();
                        });
                    });
            });
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
