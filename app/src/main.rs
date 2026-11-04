//! Tenths, drawn natively with egui, on the same telemetry core as the Tauri
//! app, reading the Tauri app's own `config.json` for which widgets are on,
//! where they are, their options, theme and units.
//!
//! One transparent, always-on-top window over each display (`displays.rs`),
//! showing the widgets placed on it. Clicks go through to iRacing except
//! while editing the layout, and over the Standings' rate thumbs after the flag.
//!
//!   tenths-egui                   iRacing (Windows), falls back to waiting
//!   tenths-egui --demo            the demo race
//!   tenths-egui --offline         no data at all (empty states)
//!   tenths-egui --edit            start in edit mode (drag the panels)
//!   tenths-egui --config <path>   another config.json than the Tauri app's
//!   tenths-egui --theme f1        dark, light, night, f1 or retro, over the config's
//!   tenths-egui --imperial        imperial units, over the config's
//!   tenths-egui --glow            Windows: draw with OpenGL instead of DirectX 12 (wgpu), if that misbehaves
//!   tenths-egui --high-gpu        Windows (DirectX 12): draw on the high-performance GPU instead of the low-power one
//!   tenths-egui --panel-windows   Windows: a small window per panel instead of one per display
//!   tenths-egui --fullscreen-overlay  one full-screen window per display (the old way) instead of a
//!                                 small window per panel (panels.rs); --shot implies it
//!   tenths-egui --bench 600       print memory and frame stats every 10 s, quit after 600 s
//!   tenths-egui --shot out.png    save a screenshot after 5 s (--shot-after N) and quit
//!   tenths-egui --settings        open the Settings window at start (--pane <name>, --report <id>;
//!                                 with --shot the window is drawn inside the overlay window,
//!                                 which is what a screenshot can capture)
//!   tenths-egui --data-dir <dir>  laps, race reports and the reference lap in another folder
//!   tenths-egui --background      start without opening Settings (a normal start opens it; it
//!                                 steps out of the way when iRacing connects)
//!   tenths-egui --heap-profile [f] record every allocation and write them to f (default
//!                                 tenths-heap.json) on quit; needs a build with the heap-profile
//!                                 feature (heap.rs). With --bench N the line gains the live heap.
//!   tenths-egui --replay <file>   play an iRacing .ibt telemetry file as if it were the sim (any
//!                                 platform; --speed N runs it N times faster, --loop starts it
//!                                 over at the end). Other cars show only if the file recorded them.
//!
//!   tenths-egui --press pit.fuel  run an action 4 s in (--press-at N), as if its key was pressed
//!
//! Hotkeys and wheel buttons (controls/): Ctrl+Shift+E (Windows, anywhere) or
//! F2 (in edit mode) toggles editing; the rest are in `general.controls`.
//! Ctrl+Shift+Q quits.

// While the port is under way some helpers wait for the panels that use them.
#![allow(dead_code)]

/// Diagnostics to stderr: debug builds only. A release build prints nothing but what was asked for
/// (`--bench`, `--shot`, the heap profiler), so no formatting or I/O runs while it's drawing.
macro_rules! log {
    ($($t:tt)*) => {
        if cfg!(debug_assertions) {
            eprintln!($($t)*);
        }
    };
}

mod anchor;
mod config;
mod controls;
mod data;
mod displays;
mod frame;
mod heap;
mod kit;
mod layouts;
mod logic;
mod panels;
mod ratings;
mod schema;
mod settings;
mod shell;
mod svg;
mod sys;
mod theme;
mod updates;
mod widgets;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
    /// Demo race seconds to fast-forward at the start (`--warmup`), so a screenshot shows a race well under way.
    warmup: f64,
    /// Actions to run once the app has been up `press_at` seconds (--press).
    press: Vec<String>,
    press_at: f64,
    // Settings window and app shell.
    settings: bool,
    pane: Option<String>,
    report: Option<String>,
    data_dir: Option<PathBuf>,
    /// Redraws a second while telemetry flows (`--fps`, default 60).
    fps: u32,
    /// Wait for the display's refresh before showing each frame (`--vsync`;
    /// off by default, so the overlay never holds up its frame to line up with the sim's).
    vsync: bool,
    /// The old overlay: one full-screen window per display instead of a window per panel.
    fullscreen_overlay: bool,
    /// Play an iRacing .ibt telemetry file instead of the sim or the demo (`--replay FILE`).
    replay: Option<PathBuf>,
    /// How much faster than real time the replay runs (`--speed`, default 1).
    replay_speed: f64,
    /// Start the replay again when it ends (`--loop`).
    replay_loop: bool,
    /// Don't open Settings at start (`--background`), for starting with the computer.
    background: bool,
    /// Record the heap and write it to this file on quit (`--heap-profile`; see heap.rs).
    heap_profile: Option<String>,
}

fn args() -> Args {
    args_from(&std::env::args().collect::<Vec<_>>())
}

/// `args` for a given command line (the first item is the program).
fn args_from(command_line: &[String]) -> Args {
    let all: Vec<String> = command_line.iter().skip(1).cloned().collect();
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
        warmup: value("--warmup").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        press: value("--press").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default(),
        press_at: value("--press-at").and_then(|v| v.parse().ok()).unwrap_or(4.0),
        settings: has("--settings"),
        pane: value("--pane"),
        report: value("--report"),
        data_dir: value("--data-dir").map(PathBuf::from),
        fps: value("--fps").and_then(|v| v.parse().ok()).filter(|f: &u32| (5..=240).contains(f)).unwrap_or(60),
        vsync: has("--vsync"),
        // A screenshot only sees the root window, so it needs the full-screen one.
        // Windows: one window per display by default. It measured about 33 fps and 200 MB against 10-20 fps
        // and 255 MB for a window per panel (14 transparent swapchains presenting from one thread).
        // `--panel-windows` brings the small windows back; elsewhere they stay the default.
        fullscreen_overlay: has("--fullscreen-overlay") || value("--shot").is_some() || (cfg!(windows) && !has("--panel-windows")),
        replay: value("--replay").map(PathBuf::from),
        replay_speed: value("--speed").and_then(|v| v.parse().ok()).filter(|s: &f64| (0.0..=1000.0).contains(s)).unwrap_or(1.0),
        replay_loop: has("--loop"),
        background: has("--background"),
        heap_profile: has("--heap-profile").then(|| heap::file_arg(value("--heap-profile").as_deref())),
    }
}

/// A normal start opens Settings, the app itself; the overlay panels are only part of it. Runs
/// for screenshots, benchmarks, editing and the empty-state check don't, and neither does
/// `--background`. `--settings` always does.
fn opens_settings_at_start(args: &Args) -> bool {
    args.settings || !(args.background || args.shot.is_some() || args.bench.is_some() || args.edit || args.offline)
}

/// Whether iRacing is connected, and whether that's new since last frame: Settings steps aside
/// the moment the overlay goes live in the sim, and not again until the sim has gone and come back.
fn sim_connected(was: bool, status: Option<&telemetry::model::StatusPayload>) -> (bool, bool) {
    let now = status.is_some_and(|s| s.connected && s.source == telemetry::model::SourceKind::Iracing);
    (now, now && !was)
}

fn main() -> eframe::Result {
    let args = args();
    // First, so everything the app allocates from here on is counted.
    if let Some(file) = &args.heap_profile {
        heap::start(file);
    }
    // One copy at a time: a second launch asks the first to open Settings. Runs
    // with their own config, a screenshot or a benchmark are never "the app".
    let single = args.config.is_none() && args.shot.is_none() && args.bench.is_none() && !args.offline && args.replay.is_none();
    let listener = if single {
        match shell::instance::claim() {
            shell::instance::Instance::AlreadyRunning => {
                println!("2Tenths is already running: asked it to open Settings.");
                return Ok(());
            }
            shell::instance::Instance::First(l) => Some(l),
        }
    } else {
        None
    };
    // One window per display: start the primary overlay already covering its display. Windows only
    // takes the mouse over the size a click-through (layered) window had when it became layered, so
    // the old 1920x1080 start left the rest of a bigger screen dead. It has to be layered from the
    // start, though: making a DirectComposition window layered later turns it white.
    let first = if args.fullscreen_overlay { displays::primary_at_start() } else { None };
    let first_rect = first.as_ref().map(|d| displays::window_rect(d, d.scale as f32));
    // Windows, full-screen overlay: layered (click-through) from the start even in edit mode, which
    // only clears the click-through bit (`App::click_through`).
    let layered_now = cfg!(windows) && args.fullscreen_overlay;
    let viewport = egui::ViewportBuilder::default()
        .with_title(PRIMARY_TITLE)
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_resizable(false)
        .with_position(first_rect.map_or(egui::Pos2::ZERO, |r| r.min))
        // Fitted to the whole display on the first frames, once the monitor is known
        // (or, with a window per panel, left tiny: panels.rs).
        .with_inner_size(match first_rect {
            Some(r) => r.size(),
            None if args.fullscreen_overlay || args.edit => egui::Vec2::new(1920.0, 1080.0),
            None => egui::Vec2::new(1.0, 1.0),
        })
        .with_mouse_passthrough(!args.edit || layered_now);
    log!(
        "[tenths] overlay: {}",
        if args.fullscreen_overlay { "one full-screen window per display (--panel-windows for a window per panel)" } else { "one small window per panel (--fullscreen-overlay for one window per display)" }
    );
    if let Some(bits) = std::env::args().skip_while(|a| a != "--flag").nth(1).and_then(|v| v.parse::<u32>().ok()) {
        widgets::flags::FORCED.store(bits, std::sync::atomic::Ordering::Relaxed);
    }
    panels::TIMING.store(args.bench.is_some(), std::sync::atomic::Ordering::Relaxed);
    data::FRAME_MS.store((1000 / args.fps).max(1) as u64, std::sync::atomic::Ordering::Relaxed);
    let glow_options = eframe::egui_glow::GlowConfiguration { vsync: args.vsync, ..Default::default() };
    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, glow_options, ..Default::default() };
    #[cfg(windows)]
    if !std::env::args().any(|a| a == "--glow") {
        use eframe::egui_wgpu::{SurfaceConfig, WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
        // DirectX 12 only: Vulkan surfaces on Windows can't be transparent, which is the whole point.
        let mut setup = WgpuSetupCreateNew::without_display_handle();
        setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
        // The default swapchain (from the window handle) can't be transparent; DirectComposition's can.
        setup.instance_descriptor.backend_options.dx12.presentation_system = eframe::wgpu::Dx12SwapchainKind::DxgiFromVisual;
        // The overlay is light work: leave the big GPU to the game (on a laptop it's also the one that doesn't
        // need every frame copied across to the screen's GPU). `--high-gpu` to choose it anyway.
        setup.power_preference = if std::env::args().any(|a| a == "--high-gpu") {
            eframe::wgpu::PowerPreference::HighPerformance
        } else {
            eframe::wgpu::PowerPreference::LowPower
        };
        options.renderer = eframe::Renderer::Wgpu;
        options.wgpu_options = WgpuConfiguration {
            surface: SurfaceConfig {
                present_mode: if args.vsync { eframe::wgpu::PresentMode::AutoVsync } else { eframe::wgpu::PresentMode::AutoNoVsync },
                ..SurfaceConfig::LOW_LATENCY
            },
            wgpu_setup: WgpuSetup::CreateNew(setup),
            ..Default::default()
        };
    }
    eframe::run_native("tenths", options, Box::new(move |cc| Ok(Box::new(App::new(cc, args, listener)))))
}

struct App {
    shared: data::Shared,
    store: config::Store,
    ratings: ratings::Ratings,
    layouts: layouts::Layouts,
    /// Shared with the panel windows, which draw it from their own repaints (panels.rs).
    layout: Arc<Mutex<frame::Layout>>,
    /// What those windows draw from, and the state they keep between repaints.
    host: Arc<panels::Host>,
    /// The connected displays (one overlay window each), re-read every couple of seconds.
    displays: Arc<Vec<displays::Display>>,
    displays_at: Option<Instant>,
    /// Per overlay window: moving it onto its display, click-through or not,
    /// and whether it refuses the keyboard.
    placers: HashMap<egui::ViewportId, displays::Placer>,
    passthrough: HashMap<egui::ViewportId, bool>,
    no_activate: HashMap<egui::ViewportId, bool>,
    /// The layout name shown for a moment after switching, and when it started.
    toast: Option<(String, f64)>,
    /// The active layout as last seen (None before the first frame), to spot a switch.
    seen_active: Option<Option<String>>,
    editing: bool,
    imperial: bool,
    theme: Option<config::ThemeId>,
    started: Instant,
    bench: Option<f64>,
    shot: Option<String>,
    shot_after: f64,
    /// While the demo fast-forwards (`--warmup`): when to drop back to real time.
    warmup_until: Option<f64>,
    shot_asked: bool,
    shot_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The display (key) whose widget list is open in its edit bar.
    picker: Option<String>,
    stats: Stats,
    controls: controls::Controls,
    /// `overlay.toggle`: every overlay hidden. Editing shows them again.
    visible: bool,
    press: Vec<String>,
    press_at: f64,
    // App shell (shell/) and the Settings window (settings/).
    control: telemetry::runner::RunnerControl,
    /// `--demo` (and every non-Windows run): the demo race whatever the setting says.
    force_demo: bool,
    shell: shell::Shell,
    settings: settings::SettingsWindow,
    tray_synced: bool,
    /// iRacing was connected last frame (`sim_connected`).
    in_sim: bool,
    /// Looks for newer releases (updates.rs); automatic checks are off for `--offline`, `--demo` and screenshots.
    updates: updates::Updates,
    auto_updates: bool,
    /// A window per panel (false: `--fullscreen-overlay`), and their state (panels.rs).
    panel_mode: bool,
    panels: panels::Panels,
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
    fn new(cc: &eframe::CreationContext<'_>, args: Args, listener: Option<shell::instance::Listener>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        // eframe can't screenshot a second window: for a --shot of Settings, draw it inside this one.
        if args.settings && args.shot.is_some() {
            cc.egui_ctx.set_embed_viewports(true);
        }
        let paths = shell::Paths::new(args.config.clone(), args.data_dir.clone());
        // A config.json that can't be read is kept aside, and Settings says so.
        let notice = paths.config.as_deref().and_then(shell::keep_corrupt_config);
        let store = config::Store::open(paths.config.clone());
        // The Tauri app's "demo" setting counts too.
        // A replay is its own source: no demo race under it.
        let demo = args.replay.is_none() && (args.demo || store.config.general.demo);
        let (shared, control) = data::start_with(cc.egui_ctx.clone(), demo, args.offline, |c| {
            shell::setup_runner(&paths, c);
            if let Some(file) = &args.replay {
                c.replay = Some(file.clone());
                c.replay_speed.store((args.replay_speed * 1000.0).round() as u32, std::sync::atomic::Ordering::Relaxed);
                c.replay_loop.store(args.replay_loop, std::sync::atomic::Ordering::Relaxed);
            }
        });
        let layouts = layouts::Layouts::open(paths.config.as_deref(), &store);
        let shell = shell::Shell::new(paths, notice);
        let updates = updates::Updates::new(shell.sender());
        updates.load_auto(shell.paths.config.as_deref());
        if let Some(l) = listener {
            l.serve(shell.sender(), cc.egui_ctx.clone());
        }
        let launch = settings::Launch {
            open: opens_settings_at_start(&args),
            pane: args.pane.as_deref().and_then(settings::Pane::parse),
            report: args.report.clone(),
        };
        let settings = settings::SettingsWindow::new(&shell, launch);
        let settings_shot = args.settings && args.shot.is_some();
        let layout = Arc::new(Mutex::new(frame::Layout::new()));
        Self {
            controls: controls::Controls::new(cc.egui_ctx.clone(), control.clone()),
            control,
            force_demo: args.demo && args.replay.is_none(),
            shell,
            settings,
            tray_synced: false,
            in_sim: false,
            updates,
            // (TENTHS_UPDATE_URL: trying a release by hand, screenshots included.)
            auto_updates: std::env::var_os("TENTHS_UPDATE_URL").is_some() || (!args.offline && !args.demo && args.replay.is_none() && args.shot.is_none()),
            panel_mode: !args.fullscreen_overlay,
            panels: Default::default(),
            shared,
            store,
            ratings: ratings::Ratings::open(config::data_dir()),
            layouts,
            layout: layout.clone(),
            host: Arc::new(panels::Host::new(layout)),
            displays: Arc::new(Vec::new()),
            displays_at: None,
            placers: HashMap::new(),
            // What each window's click-through was last set to. The root window starts click-through
            // unless we start in edit mode outside the Windows full-screen overlay (see main()).
            passthrough: HashMap::from([(egui::ViewportId::ROOT, !args.edit || (cfg!(windows) && args.fullscreen_overlay))]),
            no_activate: HashMap::new(),
            toast: None,
            seen_active: None,
            editing: args.edit,
            imperial: args.imperial,
            theme: args.theme,
            started: Instant::now(),
            bench: args.bench,
            shot: args.shot,
            shot_after: args.shot_after,
            warmup_until: (args.warmup > 0.0).then(|| args.warmup / WARMUP_SPEED as f64),
            shot_asked: false,
            shot_done: Default::default(),
            picker: None,
            stats: Stats::default(),
            // Hidden for a --shot of Settings, so the picture is just the window.
            visible: !settings_shot,
            press: args.press,
            press_at: args.press_at,
        }
    }

    fn set_editing(&mut self, ctx: &egui::Context, on: bool) {
        self.editing = on;
        self.picker = None;
        if on {
            self.visible = true;
        }
        // Each overlay window's click-through follows `editing` on its next frame (`click_through`).
        if on {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Focus);
        }
    }

    // --- Layouts: for the hotkeys, the tray and the edit bar ---------------------

    /// Puts a saved layout on screen and makes it the active one (edits save into it).
    pub fn switch_layout(&mut self, id: &str) {
        if let Err(e) = self.layouts.switch(&mut self.store, id) {
            log!("[tenths] couldn't switch layout: {e}");
            self.store.error = Some(e);
        }
    }

    /// The next saved layout (`step` 1) or the previous one (-1), wrapping round.
    pub fn next_layout(&mut self, step: i32) {
        self.layouts.cycle(&mut self.store, Some(step), None);
    }

    /// The saved layout at a position in the list (0 = first), for Ctrl+Shift+1..9.
    pub fn layout_at(&mut self, index: usize) {
        self.layouts.cycle(&mut self.store, None, Some(index));
    }

    /// Names the layout for a moment after it changes (also when the Tauri app switched it).
    fn note_layout(&mut self, now: f64) {
        let active = self.store.config.general.active_profile.clone();
        if self.seen_active.as_ref() == Some(&active) {
            return;
        }
        let first = self.seen_active.is_none();
        self.seen_active = Some(active.clone());
        if let (false, Some(id)) = (first, active) {
            if let Some(name) = self.layouts.name_of(&id) {
                self.toast = Some((name.to_string(), now));
            }
        }
    }

    /// Reads the connected displays every couple of seconds. Where the system
    /// can't list them, the one display is the window's monitor. False until
    /// there's at least one.
    fn refresh_displays(&mut self, ctx: &egui::Context) -> bool {
        if self.displays_at.is_none_or(|t| t.elapsed() >= Duration::from_secs(2)) || self.displays.is_empty() {
            self.displays_at = Some(Instant::now());
            let mut list = displays::current();
            if list.is_empty() {
                let (size, ppp) = ctx.input(|i| (i.viewport().monitor_size, i.viewport().native_pixels_per_point));
                if let Some(size) = size {
                    let ppp = ppp.unwrap_or(1.0);
                    list = displays::finish(vec![displays::Display {
                        key: "primary".into(),
                        number: 0,
                        primary: true,
                        x: 0,
                        y: 0,
                        width: (size.x * ppp).round() as u32,
                        height: (size.y * ppp).round() as u32,
                        scale: ppp as f64,
                    }]);
                }
            }
            if list != *self.displays {
                self.displays = Arc::new(list);
            }
        }
        !self.displays.is_empty()
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
    /// eframe exits the process after this, so it's where the heap profile is written.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // The graphics and window teardown after this has been seen to hang, leaving the process
        // (and its windows) up with nothing running.
        exit_watchdog();
        heap::finish();
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        // Closed from the window or the taskbar: make sure it really goes.
        if ctx.input(|i| i.viewport().close_requested()) {
            exit_watchdog();
        }
        let frame_start = Instant::now();
        if let Some(until) = self.warmup_until {
            let speed = if self.started.elapsed().as_secs_f64() < until { WARMUP_SPEED } else { 1 };
            self.control.demo_speed.store(speed, std::sync::atomic::Ordering::Relaxed);
            if speed == 1 {
                self.warmup_until = None;
            }
        }

        // One overlay window per display; wait until the first one's size is known.
        if !self.refresh_displays(ctx) {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }

        if self.editing && ctx.input(|i| i.key_pressed(egui::Key::F2) || i.key_pressed(egui::Key::Escape)) {
            self.set_editing(ctx, false);
        }

        if !std::mem::replace(&mut self.tray_synced, true) {
            self.update_tray_shortcuts();
        }
        // Automatic update checks, and the tray item's text ("Check for updates" / "Update to x.y.z").
        self.updates.tick(ctx, self.auto_updates);
        let available = self.updates.status().available.map(|r| r.version);
        self.shell.set_update_item(available.as_deref());
        // What the tray and a second launch ask for.
        for r in self.shell.poll() {
            self.handle(ctx, r);
        }

        // Settings saved by the Tauri app, and new driver ratings.
        if self.store.poll() {
            self.updates.load_auto(self.shell.paths.config.as_deref());
            self.control.save_reports.store(shell::save_reports_setting(self.shell.paths.config.as_deref()), std::sync::atomic::Ordering::Relaxed);
            self.update_tray_shortcuts();
        }
        // The demo race on or off at once, from this or the Tauri app's settings.
        self.control.demo.store(self.force_demo || self.store.config.general.demo, std::sync::atomic::Ordering::Relaxed);
        self.ratings.poll();
        self.layouts.poll();
        self.layouts.sync(&mut self.store);
        let now = self.started.elapsed().as_secs_f64();
        self.note_layout(now);
        let general = self.general();
        let th = theme::Theme::from_general(&general);

        let live = Arc::new(data::Live::take(&self.shared));
        let now = self.started.elapsed().as_secs_f64();
        // The overlay is live in iRacing: Settings goes to the tray (minimised where there's none).
        let (in_sim, entered) = sim_connected(self.in_sim, live.status.as_deref());
        self.in_sim = in_sim;
        if entered {
            self.settings.hide(ctx, self.shell.has_tray());
        }

        // Hotkeys, wheel buttons and pit commands (controls/).
        if !self.press.is_empty() && now >= self.press_at {
            for action in std::mem::take(&mut self.press) {
                self.controls.inject(&action);
            }
        }
        // While Settings captures a key or button, the shortcuts stand down and button presses go to it.
        self.controls.set_suspended(self.settings.capturing());
        let out = self.controls.frame(&mut self.store, &live, general.units, now);
        for p in out.presses {
            self.settings.feed_button(logic::settings_bindings::ButtonBinding { device: p.device, name: p.name, button: p.button as i64 });
        }
        for action in out.app {
            match action {
                controls::AppAction::ToggleEdit => self.set_editing(ctx, !self.editing),
                controls::AppAction::ToggleVisible => self.visible = !self.visible,
                controls::AppAction::Layout(controls::layouts::LayoutSwitch::Next) => self.next_layout(1),
                controls::AppAction::Layout(controls::layouts::LayoutSwitch::Index(i)) => self.layout_at(i),
                controls::AppAction::Quit => quit_app(ctx),
            }
        }
        let on_track = live.tick.as_ref().is_some_and(|t| t.is_on_track);
        let show = self.editing || (self.visible && live.connected() && (general.show_when == config::ShowWhen::Connected || on_track));
        self.layout.lock().unwrap().observe(&live, now);
        // Each display's window draws the widgets placed on it.
        let displays = self.displays.clone();
        let windows_per_panel = self.panel_mode && !self.editing;
        if windows_per_panel {
            // Outside edit mode: a small window per panel (panels.rs).
            self.panel_windows(ctx, &th, &live, general.units, show, now);
        } else {
            self.panels.seen.clear();
            // Their windows are closed while editing: they start again hidden.
            self.host.keep(&[]);
        }
        for d in displays.iter().filter(|_| !windows_per_panel) {
            if d.primary {
                self.overlay(ctx, d, &th, &live, general.units, show, now);
            } else {
                let id = egui::ViewportId::from_hash_of(("tenths-overlay", &d.key));
                ctx.show_viewport_immediate(id, overlay_viewport(d), |ui, class| {
                    if class == egui::ViewportClass::EmbeddedWindow {
                        return;
                    }
                    let ctx = ui.ctx().clone();
                    self.overlay(&ctx, d, &th, &live, general.units, show, now);
                });
            }
        }
        // Windows of displays that have gone are closed by egui (no longer shown); forget their state.
        let mut live_ids: Vec<egui::ViewportId> = displays.iter().map(|d| egui::ViewportId::from_hash_of(("tenths-overlay", &d.key))).collect();
        live_ids.extend(self.panels.seen.iter().copied());
        for map in [&mut self.passthrough, &mut self.no_activate] {
            map.retain(|id, _| *id == egui::ViewportId::ROOT || live_ids.contains(id));
        }
        self.placers.retain(|id, _| *id == egui::ViewportId::ROOT || live_ids.contains(id));

        self.stats(ctx, live.ticks, frame_start, live.status.as_deref());

        // The Settings window, when it's open.
        let screens: Vec<(String, String)> = self.displays.iter().map(|d| (d.key.clone(), d.name())).collect();
        let requests = self.settings.show(
            ctx,
            settings::Env {
                store: &mut self.store,
                live: &live,
                control: &self.control,
                shell: &self.shell,
                updates: &self.updates,
                general: &general,
                editing: self.editing,
                overlays_visible: self.visible,
                displays: &screens,
            },
        );
        for r in requests {
            self.handle(ctx, r);
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

impl App {
    /// What the tray, a second launch or the Settings window ask of the overlay window.
    fn handle(&mut self, ctx: &egui::Context, r: shell::Request) {
        use shell::Request::*;
        match r {
            OpenSettings => self.settings.show_pane(None),
            OpenAbout => self.settings.show_pane(Some(settings::Pane::About)),
            // The tray's update item: the About page, and a check if nothing is known yet.
            UpdateItem => {
                self.settings.show_pane(Some(settings::Pane::About));
                if self.updates.status().available.is_none() {
                    self.updates.check_now(ctx);
                }
            }
            ToggleEdit => self.set_editing(ctx, !self.editing),
            ToggleOverlays => self.visible = !self.visible,
            NextLayout => self.next_layout(1),
            Quit => quit_app(ctx),
        }
    }

    /// The tray's items carry their shortcuts, as in the Tauri app.
    fn update_tray_shortcuts(&mut self) {
        use logic::settings_bindings as bind;
        let all = bind::from_json(&self.store.config.general.controls);
        let key = |id: &str| all.iter().find(|(a, _)| a.id == id).and_then(|(_, b)| b.key.as_ref()).map(bind::key_label);
        self.shell.set_tray_shortcuts(&[("overlay.edit", key("overlay.edit")), ("overlay.toggle", key("overlay.toggle")), ("layout.next", key("layout.next"))]);
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
                "{:>5.0}s  memory {:>6.1} MB  {:>5.1} fps  telemetry {:>5.1} Hz  UI {:.2} ms/frame  panel windows {} ({} visible)  {}{}",
                self.started.elapsed().as_secs_f64(),
                s.memory_mb,
                s.fps,
                s.tick_hz,
                s.cpu_ms,
                self.panels.windows(),
                self.host.visible(),
                status.map(|s| s.message.as_str()).unwrap_or(""),
                heap::stats().map(|h| format!("  {h}")).unwrap_or_default()
            );
            let costs: Vec<String> = self.host.take_costs().iter().take(12).map(|(id, ms, n)| format!("{id} {ms:.2}ms x{:.0}/s", *n as f64 / 10.0)).collect();
            println!("         draw per panel: {}", costs.join(", "));
        }
        let _ = ctx;
    }

    /// One display's overlay window, from inside it: puts it on its display,
    /// draws its widgets, the edit bar and the layout toast, and sets whether it
    /// is click-through.
    #[allow(clippy::too_many_arguments)]
    fn overlay(&mut self, ctx: &egui::Context, d: &displays::Display, th: &theme::Theme, live: &data::Live, units: config::Units, show: bool, now: f64) {
        let vid = ctx.viewport_id();
        let title = if d.primary { PRIMARY_TITLE.to_string() } else { overlay_title(d) };
        let placer = self.placers.entry(vid).or_default();
        placer.update(ctx, d, now);
        placer.fit_native(ctx, &title, d, now);
        if self.editing && ctx.input(|i| i.key_pressed(egui::Key::F2) || i.key_pressed(egui::Key::Escape)) {
            self.set_editing(ctx, false);
        }
        let screen = ctx.content_rect();
        if self.editing {
            edit_backdrop(ctx, screen);
        }
        let displays = self.displays.clone();
        let mut clicks = Vec::new();
        let layout = self.layout.clone();
        let mut layout = layout.lock().unwrap();
        layout.run(frame::Frame {
            ctx,
            store: &mut self.store,
            th,
            live,
            ratings: &self.ratings,
            units,
            editing: self.editing,
            show,
            now,
            screen,
            displays: &displays,
            me: d,
            clicks: &mut clicks,
        });
        if self.editing {
            self.edit_bar(ctx, th, live.status.as_deref(), d, displays.len());
        }
        if d.primary {
            let screen = ctx.content_rect();
            self.layout_toast(ctx, th, now, screen, screen.top() + screen.height() * 0.18);
        }
        self.click_through(ctx, d, &clicks, &title);
    }

    /// Outside edit mode every click goes through to iRacing. The thumbs in the
    /// Standings after the flag take clicks anyway (`useClickableParts`), so
    /// while any are on screen, follow the mouse (the window gets no events
    /// while click-through, so read it from the system) and take clicks only
    /// while it's over one. Also keeps the window from taking the keyboard when
    /// clicked, so iRacing keeps it.
    fn click_through(&mut self, ctx: &egui::Context, d: &displays::Display, clicks: &[(egui::Rect, widgets::ClickAction)], title: &str) {
        let vid = ctx.viewport_id();
        let mut over = false;
        if !self.editing && !clicks.is_empty() {
            let pos = displays::pointer().map(|p| displays::to_client(d, p.x, p.y)).or_else(|| ctx.input(|i| i.pointer.hover_pos()));
            over = pos.is_some_and(|p| click_at(clicks, p).is_some());
            if over && ctx.input(|i| i.pointer.primary_pressed()) {
                if let Some(at) = ctx.input(|i| i.pointer.interact_pos()) {
                    if let Some(action) = click_at(clicks, at).cloned() {
                        self.perform(action);
                    }
                }
            }
            // Keep following the mouse (CLICKABLE_POLL_MS in the Tauri overlay).
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        // TENTHS_EGUI_VOTING=1 (a check, with no cursor to poll off Windows): never click-through.
        let want = !self.editing && !over && !crate::sys::voting_forced();
        if self.passthrough.get(&vid).copied().unwrap_or(true) != want {
            if cfg!(windows) {
                // Not winit's MousePassthrough: see `displays::set_click_through`.
                if displays::set_click_through(title, want) {
                    self.passthrough.insert(vid, want);
                    ctx.request_repaint();
                } else {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            } else {
                self.passthrough.insert(vid, want);
                ctx.send_viewport_cmd(egui::ViewportCommand::MousePassthrough(want));
            }
        }
        let no_activate = !self.editing;
        if self.no_activate.get(&vid) != Some(&no_activate) && displays::set_no_activate(title, no_activate) {
            self.no_activate.insert(vid, no_activate);
        }
    }

    fn perform(&mut self, action: widgets::ClickAction) {
        match action {
            widgets::ClickAction::Rate { user_id, name, rating } => {
                if let Err(e) = self.ratings.set(user_id, &name, rating) {
                    log!("[tenths] couldn't save the rating: {e}");
                    self.store.error = Some(format!("Couldn't save the rating: {e}"));
                }
            }
        }
    }

    /// "Layout: Night", fading in and out near the top for a moment (`.layout-toast`).
    /// `top` is where the toast's band starts (0.18 down a full-screen window; the top of its own window).
    fn layout_toast(&mut self, ctx: &egui::Context, th: &theme::Theme, now: f64, screen: Rect, top: f32) {
        let Some((name, at)) = self.toast.clone() else { return };
        let t = ((now - at) / TOAST_S) as f32;
        if !(0.0..1.0).contains(&t) {
            self.toast = None;
            return;
        }
        let opacity = toast_opacity(t);
        let p = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("tenths-layout-toast")));
        let font_regular = th.medium(17.0);
        let font_bold = th.bold(17.0);
        let (a, b) = ("Layout: ", name.as_str());
        let (wa, wb) = (kit::width(&p, a, font_regular.clone()), kit::width(&p, b, font_bold.clone()));
        let rect = Rect::from_center_size(
            Pos2::new(screen.center().x, top + 20.0 + (1.0 - (t / 0.1).min(1.0)) * 6.0),
            Vec2::new(wa + wb + 36.0, 40.0),
        );
        let fade = |c: Color32| c.gamma_multiply(opacity);
        p.rect_filled(rect, 8.0, fade(th.panel_solid));
        p.rect_stroke(rect, 8.0, Stroke::new(1.0, fade(th.line_strong)), egui::StrokeKind::Inside);
        let left = rect.left() + 18.0;
        kit::text(&p, Pos2::new(left, rect.center().y), Align2::LEFT_CENTER, a, font_regular, fade(th.ink2));
        kit::text(&p, Pos2::new(left + wa, rect.center().y), Align2::LEFT_CENTER, b, font_bold, fade(th.ink));
        ctx.request_repaint();
    }

    fn edit_bar(&mut self, ctx: &egui::Context, th: &theme::Theme, status: Option<&telemetry::model::StatusPayload>, d: &displays::Display, display_count: usize) {
        let s = &self.stats;
        let mut done = false;
        let mut quit = false;
        let mut toggle: Option<(&'static str, bool)> = None;
        let mut switch: Option<String> = None;
        let screen = ctx.content_rect();
        let enabled = schema::WIDGETS.iter().filter(|m| self.store.config.widget(m.id).enabled).count();
        let button = |text: &str, fill: Color32, fg: Color32| {
            egui::Button::new(egui::RichText::new(text).font(th.bold(15.0)).color(fg)).fill(fill).corner_radius(5.0).min_size(Vec2::new(0.0, 28.0))
        };
        let active = self.store.config.general.active_profile.clone();
        let active_name = active.as_deref().and_then(|id| self.layouts.name_of(id)).unwrap_or("Unsaved layout").to_string();
        let open = self.picker.as_deref() == Some(d.key.as_str());
        let mut picker_click = false;
        egui::Area::new(Id::unique(("edit-bar", &d.key)))
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
                            // Which display this is, when there's more than one.
                            if display_count > 1 {
                                ui.label(egui::RichText::new(d.name()).font(th.bold(15.0)).color(th.attention));
                            }
                            let hint = if d.primary || display_count < 2 {
                                "Drag to move. Drag the corner, or use − and +, to resize."
                            } else {
                                "Drag widgets here from another screen, or use the display button on a widget."
                            };
                            ui.label(egui::RichText::new(hint).font(th.medium(15.0)).color(th.ink2));
                            if d.primary {
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
                            }
                            if let Some(err) = &self.store.error {
                                ui.label(egui::RichText::new(err).font(th.semi(14.0)).color(th.loss));
                            }
                            // The saved layout to switch to; changes save into the active one.
                            let hover = if active.is_some() { "Changes save into this layout" } else { "Save layouts in Settings → Layouts" };
                            egui::ComboBox::from_id_salt(("edit-layout", &d.key))
                                .selected_text(egui::RichText::new(&active_name).font(th.semi(15.0)).color(th.ink))
                                .width(160.0)
                                .show_ui(ui, |ui| {
                                    for p in &self.layouts.list {
                                        let picked = active.as_deref() == Some(p.id.as_str());
                                        let label = egui::RichText::new(&p.name).font(th.semi(15.0));
                                        if ui.selectable_label(picked, label).clicked() && !picked {
                                            switch = Some(p.id.clone());
                                        }
                                    }
                                    if self.layouts.list.is_empty() {
                                        ui.label(egui::RichText::new("No saved layouts").font(th.medium(14.0)).color(th.ink3));
                                    }
                                })
                                .response
                                .on_hover_text(hover);
                            let label = format!("Widgets {enabled}/{}", schema::WIDGETS.len());
                            if ui.add(button(&label, th.panel_raised, th.ink)).clicked() {
                                picker_click = true;
                            }
                            done = ui.add(button("Done", th.ink, th.panel_solid)).clicked();
                            if d.primary {
                                quit = ui.add(button("Quit", th.panel_raised, th.ink)).clicked();
                            }
                        });
                        if open {
                            ui.add_space(6.0);
                            egui::Grid::new(("widget-picker", &d.key)).num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
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
        if picker_click {
            self.picker = if open { None } else { Some(d.key.clone()) };
        }
        if let Some((id, on)) = toggle {
            self.store.save_widget(id, &[("enabled", serde_json::json!(on))]);
        }
        if let Some(id) = switch {
            self.switch_layout(&id);
        }
        if done {
            self.set_editing(ctx, false);
        }
        if quit {
            quit_app(ctx);
        }
    }
}

/// Quits Tenths: asks the main window to close, and if the process is still there a moment later
/// (a minimised or hidden window that never redraws to see the request, or the graphics teardown
/// stalling) ends it anyway, so Quit always quits.
fn quit_app(ctx: &egui::Context) {
    ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
    ctx.request_repaint_of(egui::ViewportId::ROOT);
    exit_watchdog();
}

/// Ends the process a moment from now whatever the window system and graphics are doing (once).
fn exit_watchdog() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(1500));
            kill_now();
        });
    }
}

/// Ends the process at once. On Windows `std::process::exit` still runs every DLL's shutdown code
/// (`ExitProcess`), and the graphics driver's has been seen to hang there, leaving Tenths in Task
/// Manager after Quit. `TerminateProcess` runs none of it. Like `std::process::exit` before it, it
/// skips Rust's own clean-up too; the normal shutdown has had its 1.5 s by then.
fn kill_now() -> ! {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        TerminateProcess(GetCurrentProcess(), 0);
    }
    std::process::exit(0)
}

const PRIMARY_TITLE: &str = "2Tenths";
/// How much faster than real time `--warmup` runs the demo race.
const WARMUP_SPEED: u32 = 10;
/// Seconds the layout toast is up.
const TOAST_S: f64 = 1.8;

fn overlay_title(d: &displays::Display) -> String {
    format!("2Tenths overlay {}", d.number)
}

/// A secondary display's overlay window, set up like the primary one in main().
fn overlay_viewport(d: &displays::Display) -> egui::ViewportBuilder {
    let r = displays::window_rect(d, d.scale as f32);
    egui::ViewportBuilder::default()
        .with_title(overlay_title(d))
        .with_decorations(false)
        .with_transparent(true)
        .with_always_on_top()
        .with_taskbar(false)
        .with_resizable(false)
        .with_active(false)
        .with_position(r.min)
        .with_inner_size(r.size())
        // Click-through (layered) from the start, at its display's size; `click_through` changes it from there.
        .with_mouse_passthrough(true)
}

/// The toast's opacity over its life (0..1): in over the first tenth, held, out from three quarters.
fn toast_opacity(t: f32) -> f32 {
    if t < 0.1 {
        t / 0.1
    } else if t < 0.75 {
        1.0
    } else {
        ((1.0 - t) / 0.25).clamp(0.0, 1.0)
    }
}

/// The clickable part under a point, topmost first.
fn click_at(clicks: &[(Rect, widgets::ClickAction)], p: Pos2) -> Option<&widgets::ClickAction> {
    clicks.iter().rev().find(|(r, _)| r.contains(p)).map(|(_, a)| a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_fades_in_holds_and_out() {
        assert_eq!(toast_opacity(0.0), 0.0);
        assert!((toast_opacity(0.05) - 0.5).abs() < 1e-5);
        assert_eq!(toast_opacity(0.5), 1.0);
        assert!((toast_opacity(0.875) - 0.5).abs() < 1e-5);
        assert_eq!(toast_opacity(1.0), 0.0);
    }

    #[test]
    fn clicks_find_the_thumb_under_the_pointer() {
        let rate = |id: i64| widgets::ClickAction::Rate { user_id: id, name: "A".into(), rating: Some(kit::Thumb::Up) };
        let clicks = vec![
            (Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(22.0, 20.0)), rate(1)),
            (Rect::from_min_size(Pos2::new(36.0, 10.0), Vec2::new(22.0, 20.0)), rate(2)),
        ];
        assert_eq!(click_at(&clicks, Pos2::new(12.0, 12.0)), Some(&rate(1)));
        assert_eq!(click_at(&clicks, Pos2::new(40.0, 29.0)), Some(&rate(2)));
        // In the gap, or outside: nothing, so the window stays click-through.
        assert_eq!(click_at(&clicks, Pos2::new(33.0, 12.0)), None);
        assert_eq!(click_at(&clicks, Pos2::new(12.0, 31.0)), None);
        assert_eq!(click_at(&[], Pos2::ZERO), None);
    }

    fn status(source: telemetry::model::SourceKind, connected: bool) -> telemetry::model::StatusPayload {
        telemetry::model::StatusPayload { source, connected, message: String::new() }
    }

    #[test]
    fn settings_steps_aside_once_when_iracing_connects() {
        use telemetry::model::SourceKind as K;
        // Starting up, waiting for the sim: nothing.
        assert_eq!(sim_connected(false, None), (false, false));
        assert_eq!(sim_connected(false, Some(&status(K::None, false))), (false, false));
        // It connects: the moment to hide.
        assert_eq!(sim_connected(false, Some(&status(K::Iracing, true))), (true, true));
        // Still connected: don't hide again, so a Settings window opened since stays.
        assert_eq!(sim_connected(true, Some(&status(K::Iracing, true))), (true, false));
        // The sim closes and comes back: hide again.
        assert_eq!(sim_connected(true, Some(&status(K::Iracing, false))), (false, false));
        assert_eq!(sim_connected(false, Some(&status(K::Iracing, true))), (true, true));
        // The demo race and a replay are not "in the sim": Settings stays for designing layouts.
        assert_eq!(sim_connected(false, Some(&status(K::Demo, true))), (false, false));
        assert_eq!(sim_connected(false, Some(&status(K::Replay, true))), (false, false));
    }

    #[test]
    fn a_normal_start_opens_settings() {
        let none = |extra: &[&str]| {
            let mut a = vec!["tenths".to_string()];
            a.extend(extra.iter().map(|s| s.to_string()));
            args_from(&a)
        };
        assert!(opens_settings_at_start(&none(&[])));
        assert!(opens_settings_at_start(&none(&["--demo"])));
        assert!(opens_settings_at_start(&none(&["--replay", "x.ibt"])));
        assert!(!opens_settings_at_start(&none(&["--background"])));
        assert!(!opens_settings_at_start(&none(&["--shot", "a.png"])));
        assert!(!opens_settings_at_start(&none(&["--bench", "10"])));
        assert!(!opens_settings_at_start(&none(&["--edit"])));
        assert!(!opens_settings_at_start(&none(&["--offline"])));
        // Asking for Settings wins.
        assert!(opens_settings_at_start(&none(&["--background", "--settings"])));
    }
}
