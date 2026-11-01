//! Places the widgets (`WidgetFrame.tsx`): each is drawn at its own origin
//! on its own layer, then moved and scaled into place from its anchored
//! position. In edit mode: drag to move (snapping to the others and the
//! screen), drag the corner or use − and + to resize, Hide; each change is
//! written back into `config.json` for that widget only.
//!
//! A widget that panics shows a small note in edit mode and nothing
//! otherwise, and is tried again when its settings change or after a while
//! (`WidgetBoundary.tsx`).

use std::panic::{catch_unwind, AssertUnwindSafe};

use egui::emath::TSTransform;
use egui::{Align2, Color32, Id, LayerId, Order, Painter, Pos2, Rect, Stroke, Vec2};
use serde_json::json;

use crate::anchor::{self, Guide};
use crate::config::{Opts, Store, SCALE_MAX, SCALE_MIN};
use crate::data::Live;
use crate::kit;
use crate::ratings::Ratings;
use crate::schema::{Meta, WIDGETS};
use crate::theme::{alpha, Theme};
use crate::widgets::{self, Ctx, Widget};

const SCALE_STEP: f32 = 0.05;
/// Seconds before a widget that failed is tried again.
const RETRY_S: f64 = 3.0;

fn clamp_scale(s: f32) -> f32 {
    ((s / SCALE_STEP).round() * SCALE_STEP).clamp(SCALE_MIN, SCALE_MAX)
}

struct Slot {
    meta: &'static Meta,
    widget: Box<dyn Widget>,
    /// Unscaled size from the last draw.
    natural: Vec2,
    /// Where it is on screen this frame (None when not drawn).
    rect: Option<Rect>,
    /// The panic message and when it happened.
    error: Option<(String, f64, String)>,
}

enum Drag {
    Move { idx: usize, press: Pos2, start: Pos2, live: Pos2, moved: bool },
    Scale { idx: usize, press: Pos2, scale: f32, w: f32, live: f32 },
}

pub struct Layout {
    slots: Vec<Slot>,
    drag: Option<Drag>,
    guides: Vec<Guide>,
}

/// What a click in edit mode landed on.
enum Hit {
    Smaller(usize),
    Bigger(usize),
    Hide(usize),
    Handle(usize),
    Body(usize),
}

pub struct Frame<'a> {
    pub ctx: &'a egui::Context,
    pub store: &'a mut Store,
    pub th: &'a Theme,
    pub live: &'a Live,
    pub ratings: &'a Ratings,
    pub units: crate::config::Units,
    pub editing: bool,
    pub show: bool,
    pub now: f64,
    /// The area to place widgets in (the window, in points).
    pub screen: Rect,
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

impl Layout {
    pub fn new() -> Self {
        let slots = WIDGETS
            .iter()
            .map(|meta| Slot { meta, widget: widgets::make(meta.id), natural: Vec2::ZERO, rect: None, error: None })
            .collect();
        Self { slots, drag: None, guides: Vec::new() }
    }

    pub fn run(&mut self, f: Frame) {
        let Frame { ctx, store, th, live, ratings, units, editing, show, now, screen } = f;
        for s in &mut self.slots {
            s.widget.observe(live, now);
        }
        if !editing {
            self.drag = None;
            self.guides.clear();
        }
        let screen_size = screen.size();

        for (idx, slot) in self.slots.iter_mut().enumerate() {
            slot.rect = None;
            let cfg = store.config.widget(slot.meta.id).clone();
            if !show || !cfg.enabled {
                continue;
            }
            let layer = LayerId::new(Order::Middle, Id::unique(("tenths-widget", slot.meta.id)));
            let mut painter = Painter::new(ctx.clone(), layer, Rect::EVERYTHING);
            painter.set_opacity(if editing { cfg.opacity.max(0.85) } else { cfg.opacity });

            // A failed widget is tried again when its settings change, or after a while.
            let fingerprint = format!("{:?}", cfg.options);
            if let Some((_, at, fp)) = &slot.error {
                if *fp != fingerprint || now - at > RETRY_S {
                    slot.error = None;
                }
            }
            let size = if let Some((msg, _, _)) = &slot.error {
                if editing {
                    kit::error_note(&painter, th, slot.meta.name, msg)
                } else {
                    Vec2::ZERO
                }
            } else {
                let cx = Ctx {
                    th,
                    o: Opts::new(&store.config, slot.meta.id),
                    general: &store.config.general,
                    cfg: &store.config,
                    units,
                    live,
                    ratings,
                    editing,
                    now,
                    ctx,
                };
                let widget = &mut slot.widget;
                let first = ctx.graphics_mut(|g| g.entry(layer).next_idx().0);
                // TENTHS_EGUI_FAIL=<id> makes that widget fail, to check this path.
                let fail = std::env::var("TENTHS_EGUI_FAIL").is_ok_and(|v| v == slot.meta.id);
                match catch_unwind(AssertUnwindSafe(|| {
                    let size = widget.draw(&painter, &cx);
                    if fail {
                        panic!("failing on purpose (TENTHS_EGUI_FAIL)");
                    }
                    size
                })) {
                    Ok(size) => size,
                    Err(e) => {
                        let msg = e
                            .downcast_ref::<&str>()
                            .map(|s| s.to_string())
                            .or_else(|| e.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "it stopped unexpectedly".into());
                        eprintln!("[tenths-egui] the {} widget failed: {msg}", slot.meta.id);
                        // Drop whatever it drew before failing.
                        ctx.graphics_mut(|g| {
                            let list = g.entry(layer);
                            for i in first..list.next_idx().0 {
                                list.reset_shape(egui::layers::ShapeIdx(i));
                            }
                        });
                        let size = if editing { kit::error_note(&painter, th, slot.meta.name, &msg) } else { Vec2::ZERO };
                        slot.error = Some((msg, now, fingerprint));
                        size
                    }
                }
            };
            slot.natural = size;
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            let (pos, scale) = match &self.drag {
                Some(Drag::Move { idx: i, live, .. }) if *i == idx => (*live, cfg.scale),
                Some(Drag::Scale { idx: i, live: s, .. }) if *i == idx => {
                    let p = slot.rect_from_cfg(&cfg, *s, screen_size);
                    (p, *s)
                }
                _ => (slot.rect_from_cfg(&cfg, cfg.scale, screen_size), cfg.scale),
            };
            let pos = pos + screen.min.to_vec2();
            ctx.transform_layer_shapes(layer, TSTransform::new(pos.to_vec2(), scale));
            slot.rect = Some(Rect::from_min_size(pos, size * scale));
        }

        if editing {
            self.interact(ctx, store, screen);
            self.chrome(ctx, th, store, screen);
        }
    }

    /// Edit-mode mouse handling.
    fn interact(&mut self, ctx: &egui::Context, store: &mut Store, screen: Rect) {
        let (pressed, down, released, pos) =
            ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.interact_pos()));
        let screen_size = screen.size();
        if pressed && self.drag.is_none() && !ctx.is_pointer_over_egui() {
            if let Some(p) = pos {
                match self.hit(ctx, store, p) {
                    Some(Hit::Smaller(i)) => self.nudge_scale(store, i, -0.1, screen),
                    Some(Hit::Bigger(i)) => self.nudge_scale(store, i, 0.1, screen),
                    Some(Hit::Hide(i)) => store.save_widget(self.slots[i].meta.id, &[("enabled", json!(false))]),
                    Some(Hit::Handle(i)) => {
                        let r = self.slots[i].rect.unwrap();
                        let scale = store.config.widget(self.slots[i].meta.id).scale;
                        self.drag = Some(Drag::Scale { idx: i, press: p, scale, w: r.width(), live: scale });
                    }
                    Some(Hit::Body(i)) => {
                        let r = self.slots[i].rect.unwrap();
                        let start = r.min - screen.min.to_vec2();
                        self.drag = Some(Drag::Move { idx: i, press: p, start, live: start, moved: false });
                    }
                    None => {}
                }
            }
        }
        if let (Some(p), true) = (pos, down) {
            let others: Vec<Rect> = self
                .slots
                .iter()
                .enumerate()
                .filter(|(i, _)| !matches!(&self.drag, Some(Drag::Move { idx, .. }) if idx == i))
                .filter_map(|(_, s)| s.rect.map(|r| r.translate(-screen.min.to_vec2())))
                .collect();
            match &mut self.drag {
                Some(Drag::Move { idx, press, start, live, moved }) => {
                    let delta = p - *press;
                    if delta.length() > 0.0 {
                        *moved = true;
                    }
                    let size = self.slots[*idx].rect.map(|r| r.size()).unwrap_or_default();
                    let (snapped, guides) = anchor::snap(Rect::from_min_size(*start + delta, size), &others, screen_size);
                    *live = snapped;
                    self.guides = guides;
                }
                Some(Drag::Scale { press, scale, w, live, .. }) => {
                    *live = clamp_scale(*scale * ((*w + p.x - press.x) / *w));
                }
                None => {}
            }
        }
        if released || (!down && self.drag.is_some()) {
            match self.drag.take() {
                Some(Drag::Move { idx, live, moved: true, .. }) => {
                    let slot = &self.slots[idx];
                    let scale = store.config.widget(slot.meta.id).scale;
                    save(store, slot.meta.id, live, slot.natural * scale, scale, screen_size);
                }
                Some(Drag::Scale { idx, live, .. }) => {
                    let slot = &self.slots[idx];
                    if let Some(r) = slot.rect {
                        save(store, slot.meta.id, r.min - screen.min.to_vec2(), slot.natural * live, live, screen_size);
                    }
                }
                _ => {}
            }
            self.guides.clear();
        }
        if self.drag.is_some() {
            ctx.request_repaint();
        }
    }

    /// Resizes from the edge the widget is pinned to, so it grows into the screen.
    fn nudge_scale(&mut self, store: &mut Store, i: usize, by: f32, screen: Rect) {
        let slot = &self.slots[i];
        let Some(r) = slot.rect else { return };
        let cfg = store.config.widget(slot.meta.id);
        let next = clamp_scale(cfg.scale + by);
        if (next - cfg.scale).abs() < 1e-4 {
            return;
        }
        let r = r.translate(-screen.min.to_vec2());
        let (ax, ay, _, _) = anchor::to_anchored(r.left(), r.top(), r.size(), screen.size());
        let n = slot.natural * next;
        use crate::config::{AnchorX, AnchorY};
        let left = match ax {
            AnchorX::L => r.left(),
            AnchorX::R => r.right() - n.x,
            AnchorX::C => r.left() + (r.width() - n.x) / 2.0,
        };
        let top = match ay {
            AnchorY::T => r.top(),
            AnchorY::B => r.bottom() - n.y,
            AnchorY::M => r.top() + (r.height() - n.y) / 2.0,
        };
        save(store, slot.meta.id, Pos2::new(left, top), n, next, screen.size());
    }

    fn label_rect(&self, ctx: &egui::Context, th: &Theme, store: &Store, i: usize) -> Option<(Rect, [Rect; 3])> {
        let slot = &self.slots[i];
        let r = slot.rect?;
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("tenths-edit-chrome")));
        let name_w = kit::width(&painter, slot.meta.name, th.bold(13.0));
        let scale = store.config.widget(slot.meta.id).scale;
        let pct = format!("{}%", (scale * 100.0).round());
        let pct_w = kit::width(&painter, &pct, th.medium(14.0)).max(38.0);
        let hide_w = kit::width(&painter, "Hide", th.bold(12.0)) + 12.0;
        let tools_w = 2.0 + 24.0 + 2.0 + pct_w + 2.0 + 24.0 + 2.0 + 4.0 + hide_w + 2.0;
        let h = 28.0;
        let below = r.top() < 34.0;
        let top = if below { r.bottom() + 6.0 } else { r.top() - 6.0 - h };
        let label = Rect::from_min_size(Pos2::new(r.left(), top), Vec2::new(name_w + 8.0 + tools_w, h));
        let tools_left = r.left() + name_w + 8.0;
        let minus = Rect::from_min_size(Pos2::new(tools_left + 2.0, top + 3.0), Vec2::new(24.0, 22.0));
        let plus = Rect::from_min_size(Pos2::new(minus.right() + 2.0 + pct_w + 2.0, top + 3.0), Vec2::new(24.0, 22.0));
        let hide = Rect::from_min_size(Pos2::new(plus.right() + 2.0 + 4.0, top + 3.0), Vec2::new(hide_w, 22.0));
        Some((label, [minus, plus, hide]))
    }

    fn handle_rect(r: Rect) -> Rect {
        Rect::from_min_size(Pos2::new(r.right() + 9.0 - 14.0, r.bottom() + 9.0 - 14.0), Vec2::splat(14.0))
    }

    fn hit(&self, ctx: &egui::Context, store: &Store, p: Pos2) -> Option<Hit> {
        let th = Theme::from_general(&store.config.general);
        for i in (0..self.slots.len()).rev() {
            let Some(r) = self.slots[i].rect else { continue };
            if let Some((_, [minus, plus, hide])) = self.label_rect(ctx, &th, store, i) {
                if minus.contains(p) {
                    return Some(Hit::Smaller(i));
                }
                if plus.contains(p) {
                    return Some(Hit::Bigger(i));
                }
                if hide.contains(p) {
                    return Some(Hit::Hide(i));
                }
            }
            if Self::handle_rect(r).contains(p) {
                return Some(Hit::Handle(i));
            }
        }
        (0..self.slots.len()).rev().find(|&i| self.slots[i].rect.is_some_and(|r| r.contains(p))).map(Hit::Body)
    }

    /// Outlines, labels with tools, resize handles and snapping guides.
    fn chrome(&self, ctx: &egui::Context, th: &Theme, store: &Store, screen: Rect) {
        let p = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("tenths-edit-chrome")));
        let hover = ctx.input(|i| i.pointer.hover_pos());
        for g in &self.guides {
            match *g {
                Guide::X(x) => {
                    p.vline(screen.left() + x, screen.y_range(), Stroke::new(1.0, th.attention));
                }
                Guide::Y(y) => {
                    p.hline(screen.x_range(), screen.top() + y, Stroke::new(1.0, th.attention));
                }
            }
        }
        for (i, slot) in self.slots.iter().enumerate() {
            let Some(r) = slot.rect else { continue };
            kit::dashed_rect(&p, r.expand(3.5), alpha(th.attention, 0.75), 1.0, 4.0, 3.0);
            let Some((label, [minus, plus, hide])) = self.label_rect(ctx, th, store, i) else { continue };
            let y = label.center().y;
            // The name, with a shadow so it reads over anything.
            kit::text(&p, Pos2::new(label.left() + 1.0, y + 1.0), Align2::LEFT_CENTER, slot.meta.name, th.bold(13.0), Color32::from_black_alpha(220));
            kit::text(&p, Pos2::new(label.left(), y), Align2::LEFT_CENTER, slot.meta.name, th.bold(13.0), th.attention);
            let tools = Rect::from_min_max(Pos2::new(minus.left() - 2.0, label.top() + 1.0), Pos2::new(hide.right() + 2.0, label.bottom() - 1.0));
            kit::fill(&p, tools, 6.0, th.panel_solid);
            kit::outline(&p, tools, 6.0, 1.0, th.line_strong);
            let scale = store.config.widget(slot.meta.id).scale;
            for (b, s, enabled) in [(minus, "−", scale > SCALE_MIN + 1e-3), (plus, "+", scale < SCALE_MAX - 1e-3)] {
                if enabled && hover.is_some_and(|h| b.contains(h)) {
                    kit::fill(&p, b, 4.0, th.panel_raised);
                }
                let c = if enabled { th.ink } else { alpha(th.ink, 0.35) };
                kit::text(&p, b.center(), Align2::CENTER_CENTER, s, th.bold(14.0), c);
            }
            let pct = format!("{}%", (scale * 100.0).round());
            kit::text(&p, Pos2::new((minus.right() + plus.left()) / 2.0, y), Align2::CENTER_CENTER, &pct, th.medium(14.0), th.ink3);
            p.vline(hide.left(), hide.y_range(), Stroke::new(1.0, th.line_strong));
            let hide_c = if hover.is_some_and(|h| hide.contains(h)) { th.loss } else { th.ink2 };
            kit::text(&p, hide.center(), Align2::CENTER_CENTER, "Hide", th.bold(12.0), hide_c);
            kit::fill(&p, Self::handle_rect(r), 3.0, th.attention);
        }
    }
}

impl Slot {
    /// Top-left on the screen (relative to its origin) at `scale`.
    fn rect_from_cfg(&self, cfg: &crate::config::WidgetCfg, scale: f32, screen: Vec2) -> Pos2 {
        anchor::resolve(cfg, self.natural * scale, screen, self.natural)
    }
}

/// Saves an absolute position (and scale) as an anchored one.
fn save(store: &mut Store, id: &str, left_top: Pos2, size: Vec2, scale: f32, screen: Vec2) {
    let p = anchor::clamp_on_screen(left_top.x, left_top.y, size, screen);
    let (ax, ay, x, y) = anchor::to_anchored(p.x, p.y, size, screen);
    let scale = (scale as f64 * 100.0).round() / 100.0;
    store.save_widget(
        id,
        &[("x", json!(x as f64)), ("y", json!(y as f64)), ("ax", json!(ax.key())), ("ay", json!(ay.key())), ("scale", json!(scale))],
    );
}
