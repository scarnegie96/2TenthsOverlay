//! Places the widgets (`WidgetFrame.tsx`): each is drawn at its own origin
//! on its own layer, then moved and scaled into place from its anchored
//! position. In edit mode: drag to move (snapping to the others and the
//! screen), drag the corner or use − and + to resize, Hide; each change is
//! written back into `config.json` for that widget only.
//!
//! With several displays there is one overlay window per display and `run`
//! is called once for each: it draws the widgets placed on that display
//! (`widget.display`, else the primary). A widget dragged past the window's
//! edge onto another display moves there when dropped, and each widget's label
//! has a button that sends it to the next display.
//!
//! A widget that panics shows a small note in edit mode and nothing
//! otherwise, and is tried again when its settings change or after a while
//! (`WidgetBoundary.tsx`).

use std::cell::RefCell;
use std::panic::{catch_unwind, AssertUnwindSafe};

use egui::emath::TSTransform;
use egui::{Align2, Color32, Id, LayerId, Order, Painter, Pos2, Rect, Stroke, Vec2};
use serde_json::json;

use crate::anchor::{self, Guide};
use crate::config::{Opts, Store, SCALE_MAX, SCALE_MIN};
use crate::data::Live;
use crate::displays::{self, Display};
use crate::kit;
use crate::ratings::Ratings;
use crate::schema::{Meta, WIDGETS};
use crate::theme::{alpha, Theme};
use crate::widgets::{self, ClickAction, Ctx, Widget};

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
    /// The display (key) whose window `rect` is in.
    shown_on: Option<String>,
    /// The panic message and when it happened.
    error: Option<(String, f64, String)>,
}

enum Drag {
    Move { idx: usize, press: Pos2, start: Pos2, live: Pos2, moved: bool, pointer: Pos2 },
    Scale { idx: usize, press: Pos2, scale: f32, w: f32, live: f32 },
}

pub struct Layout {
    slots: Vec<Slot>,
    drag: Option<Drag>,
    /// The display (key) the drag started on.
    drag_on: String,
    /// While a widget is dragged past its window's edge: the number of the
    /// display under the pointer, or 0 when it's over none.
    leaving: Option<u32>,
    guides: Vec<Guide>,
}

/// What a click in edit mode landed on.
enum Hit {
    Smaller(usize),
    Bigger(usize),
    Hide(usize),
    NextDisplay(usize),
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
    /// Every display, and the one this window covers.
    pub displays: &'a [Display],
    pub me: &'a Display,
    /// Gets the clickable parts of the widgets drawn here (screen rects).
    pub clicks: &'a mut Vec<(Rect, ClickAction)>,
}

/// What `Layout::draw_panel` needs.
pub struct PanelFrame<'a> {
    pub ctx: &'a egui::Context,
    pub store: &'a Store,
    pub th: &'a Theme,
    pub live: &'a Live,
    pub ratings: &'a Ratings,
    pub units: crate::config::Units,
    pub now: f64,
}

/// What `Layout::draw_stream` needs (the stream window, stream.rs).
pub struct StreamFrame<'a> {
    pub ctx: &'a egui::Context,
    pub store: &'a Store,
    pub th: &'a Theme,
    pub live: &'a Live,
    pub ratings: &'a Ratings,
    pub units: crate::config::Units,
    pub now: f64,
    /// The widgets to draw (indexes into `WIDGETS`), each with the size, in points, of the display it is placed on.
    pub want: &'a [(usize, Vec2)],
    /// Points in the stream window per point of the display: where and how big everything is drawn.
    pub k: f32,
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
            .map(|meta| Slot { meta, widget: widgets::make(meta.id), natural: Vec2::ZERO, rect: None, shown_on: None, error: None })
            .collect();
        Self { slots, drag: None, drag_on: String::new(), leaving: None, guides: Vec::new() }
    }

    /// Lets every widget see the latest data, once a frame, whether it is shown or not.
    pub fn observe(&mut self, live: &Live, now: f64) {
        for s in &mut self.slots {
            s.widget.observe(live, now);
        }
    }

    /// Per-panel windows (not editing): draws widget `idx` at the origin of its own
    /// window, scaled, and returns its unscaled size (zero when it draws nothing)
    /// and the clickable parts in window points.
    pub fn draw_panel(&mut self, idx: usize, f: PanelFrame) -> (Vec2, Vec<(Rect, ClickAction)>) {
        let PanelFrame { ctx, store, th, live, ratings, units, now } = f;
        let slot = &mut self.slots[idx];
        let cfg = store.config.widget(slot.meta.id).clone();
        let hover = ctx.input(|i| i.pointer.hover_pos());
        let prev = Some(Rect::from_min_size(Pos2::ZERO, slot.natural * cfg.scale));
        let paint = Paint { ctx, th, store, live, ratings, units, editing: false, now, hover };
        let (size, clicks, layer) = paint_slot(slot, &paint, &cfg, prev);
        slot.natural = size;
        slot.rect = None;
        if size.x <= 0.0 || size.y <= 0.0 {
            return (Vec2::ZERO, Vec::new());
        }
        ctx.transform_layer_shapes(layer, TSTransform::new(Vec2::ZERO, cfg.scale));
        let clicks = clicks.into_iter().map(|c| (Rect::from_min_size(c.rect.min * cfg.scale, c.rect.size() * cfg.scale), c.action)).collect();
        (size, clicks)
    }

    /// The stream window: draws each wanted widget where the layout puts it on its
    /// display, scaled by `k`. Same drawing, theme, options and opacity as the
    /// overlay. Doesn't feed the widgets (`observe` already ran this frame) or
    /// touch where the overlay thinks they are.
    pub fn draw_stream(&mut self, f: StreamFrame) {
        let StreamFrame { ctx, store, th, live, ratings, units, now, want, k } = f;
        for &(idx, screen) in want {
            let slot = &mut self.slots[idx];
            let cfg = store.config.widget(slot.meta.id).clone();
            let paint = Paint { ctx, th, store, live, ratings, units, editing: false, now, hover: None };
            let (size, _clicks, layer) = paint_slot(slot, &paint, &cfg, None);
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            let pos = anchor::resolve(&cfg, size * cfg.scale, screen, size);
            ctx.transform_layer_shapes(layer, TSTransform::new(pos.to_vec2() * k, cfg.scale * k));
        }
    }

    /// Editing is over (or never started): forget any drag or guides.
    pub fn stop_editing(&mut self) {
        self.drag = None;
        self.guides.clear();
        self.leaving = None;
    }

    /// The unscaled size widget `idx` had when last drawn.
    pub fn natural(&self, idx: usize) -> Vec2 {
        self.slots[idx].natural
    }

    /// Where the widget is in the window of display `key` (None if it isn't there).
    fn rect_on(&self, i: usize, key: &str) -> Option<Rect> {
        let s = &self.slots[i];
        if s.shown_on.as_deref() == Some(key) {
            s.rect
        } else {
            None
        }
    }

    pub fn run(&mut self, f: Frame) {
        let Frame { ctx, store, th, live, ratings, units, editing, show, now, screen, displays, me, clicks: clicks_out } = f;
        let hover = ctx.input(|i| i.pointer.hover_pos());
        if !editing {
            self.drag = None;
            self.guides.clear();
        }
        let screen_size = screen.size();

        for (idx, slot) in self.slots.iter_mut().enumerate() {
            let prev_rect = slot.rect;
            let cfg = store.config.widget(slot.meta.id).clone();
            // Another display's widget: nothing here (and forget where it was if it just left).
            if displays::widget_display(cfg.display.as_deref(), displays).is_some_and(|k| k != me.key) {
                if slot.shown_on.as_deref() == Some(&me.key) {
                    slot.rect = None;
                    slot.shown_on = None;
                }
                continue;
            }
            slot.rect = None;
            slot.shown_on = Some(me.key.clone());
            if !show || !cfg.enabled {
                continue;
            }
            let paint = Paint { ctx, th, store, live, ratings, units, editing, now, hover };
            let (size, clicks, layer) = paint_slot(slot, &paint, &cfg, prev_rect);
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
            if !editing {
                for c in clicks {
                    clicks_out.push((Rect::from_min_size(pos + c.rect.min.to_vec2() * scale, c.rect.size() * scale), c.action));
                }
            }
        }

        if editing {
            self.interact(ctx, store, screen, displays, me);
            self.chrome(ctx, th, store, screen, displays, me);
        }
    }

    /// Edit-mode mouse handling.
    fn interact(&mut self, ctx: &egui::Context, store: &mut Store, screen: Rect, displays: &[Display], me: &Display) {
        // A drag belongs to the window it started in; the others leave it alone.
        if self.drag.is_some() && self.drag_on != me.key {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
            return;
        }
        let (pressed, mut down, released, mut pos) =
            ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.primary_released(), i.pointer.interact_pos()));
        // While dragging between displays, ask the system where the mouse is: the
        // window may stop getting its events once the mouse leaves it (Windows).
        if self.drag.is_some() && displays.len() > 1 {
            if let Some(ptr) = displays::pointer() {
                pos = Some(displays::to_client(me, ptr.x, ptr.y) + screen.min.to_vec2());
                down = ptr.down;
            }
        }
        let screen_size = screen.size();
        let next = displays::next_display(&me.key, displays);
        if pressed && self.drag.is_none() && !ctx.is_pointer_over_egui() {
            if let Some(p) = pos {
                match self.hit(ctx, store, p, &me.key, next.map(|d| d.number)) {
                    Some(Hit::Smaller(i)) => self.nudge_scale(store, i, -0.1, screen, &me.key),
                    Some(Hit::Bigger(i)) => self.nudge_scale(store, i, 0.1, screen, &me.key),
                    Some(Hit::Hide(i)) => store.save_widget(self.slots[i].meta.id, &[("enabled", json!(false))]),
                    Some(Hit::NextDisplay(i)) => {
                        // Anchored on this screen first, so it lands in the matching place on the other.
                        if let (Some(r), Some(next)) = (self.rect_on(i, &me.key), next) {
                            let slot = &self.slots[i];
                            let scale = store.config.widget(slot.meta.id).scale;
                            save(store, slot.meta.id, r.min - screen.min.to_vec2(), r.size(), scale, screen_size, Some(&next.key));
                        }
                    }
                    Some(Hit::Handle(i)) => {
                        let r = self.rect_on(i, &me.key).unwrap();
                        let scale = store.config.widget(self.slots[i].meta.id).scale;
                        self.drag_on = me.key.clone();
                        self.drag = Some(Drag::Scale { idx: i, press: p, scale, w: r.width(), live: scale });
                    }
                    Some(Hit::Body(i)) => {
                        let r = self.rect_on(i, &me.key).unwrap();
                        let start = r.min - screen.min.to_vec2();
                        self.drag_on = me.key.clone();
                        self.drag = Some(Drag::Move { idx: i, press: p, start, live: start, moved: false, pointer: p });
                    }
                    None => {}
                }
            }
        }
        if let (Some(p), true) = (pos, down) {
            let others: Vec<Rect> = (0..self.slots.len())
                .filter(|i| !matches!(&self.drag, Some(Drag::Move { idx, .. }) if idx == i))
                .filter_map(|i| self.rect_on(i, &me.key).map(|r| r.translate(-screen.min.to_vec2())))
                .collect();
            let mut leaving = None;
            match &mut self.drag {
                Some(Drag::Move { idx, press, start, live, moved, pointer }) => {
                    let delta = p - *press;
                    if delta.length() > 0.0 {
                        *moved = true;
                    }
                    *pointer = p;
                    let size = self.slots[*idx].rect.map(|r| r.size()).unwrap_or_default();
                    let (snapped, guides) = anchor::snap(Rect::from_min_size(*start + delta, size), &others, screen_size);
                    *live = snapped;
                    self.guides = guides;
                    if !screen.contains(p) && displays.len() > 1 {
                        let grab = *press - *start - screen.min.to_vec2();
                        leaving = Some(displays::drop_onto(me, p - screen.min.to_vec2(), grab, displays).map_or(0, |d| d.display.number));
                    }
                }
                Some(Drag::Scale { press, scale, w, live, .. }) => {
                    *live = clamp_scale(*scale * ((*w + p.x - press.x) / *w));
                }
                None => {}
            }
            self.leaving = leaving;
        }
        if released || (!down && self.drag.is_some()) {
            match self.drag.take() {
                Some(Drag::Move { idx, live, moved: true, press, start, pointer, .. }) => {
                    let slot = &self.slots[idx];
                    let scale = store.config.widget(slot.meta.id).scale;
                    let size = slot.natural * scale;
                    // Let go past the edge, over another display: it goes there, at the same scale.
                    let end = pos.unwrap_or(pointer);
                    let across = if screen.contains(end) || displays.len() < 2 {
                        None
                    } else {
                        displays::drop_onto(me, end - screen.min.to_vec2(), press - start - screen.min.to_vec2(), displays)
                    };
                    match across {
                        Some(d) => save(store, slot.meta.id, Pos2::new(d.left, d.top), size, scale, d.display.css_size(), Some(&d.display.key)),
                        None => save(store, slot.meta.id, live, size, scale, screen_size, None),
                    }
                }
                Some(Drag::Scale { idx, live, .. }) => {
                    let slot = &self.slots[idx];
                    if let Some(r) = self.rect_on(idx, &me.key) {
                        save(store, slot.meta.id, r.min - screen.min.to_vec2(), slot.natural * live, live, screen_size, None);
                    }
                }
                _ => {}
            }
            self.guides.clear();
            self.leaving = None;
        }
        if self.drag.is_some() {
            ctx.request_repaint();
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }

    /// Resizes from the edge the widget is pinned to, so it grows into the screen.
    fn nudge_scale(&mut self, store: &mut Store, i: usize, by: f32, screen: Rect, key: &str) {
        let slot = &self.slots[i];
        let Some(r) = self.rect_on(i, key) else { return };
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
        save(store, slot.meta.id, Pos2::new(left, top), n, next, screen.size(), None);
    }

    /// The label above a widget: its name and tools (−, %, +, → next display, Hide).
    fn label_rect(&self, ctx: &egui::Context, th: &Theme, store: &Store, i: usize, key: &str, next: Option<u32>) -> Option<(Rect, [Rect; 3], Option<Rect>)> {
        let slot = &self.slots[i];
        let r = self.rect_on(i, key)?;
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::unique("tenths-edit-chrome")));
        let name_w = kit::width(&painter, slot.meta.name, th.bold(13.0));
        let scale = store.config.widget(slot.meta.id).scale;
        let pct = format!("{}%", (scale * 100.0).round());
        let pct_w = kit::width(&painter, &pct, th.medium(14.0)).max(38.0);
        let hide_w = kit::width(&painter, "Hide", th.bold(12.0)) + 12.0;
        let disp = next.map(|n| format!("→ {n}"));
        let disp_w = disp.as_ref().map(|d| kit::width(&painter, d, th.bold(12.0)) + 12.0 + 2.0).unwrap_or(0.0);
        let tools_w = 2.0 + 24.0 + 2.0 + pct_w + 2.0 + 24.0 + 2.0 + disp_w + 4.0 + hide_w + 2.0;
        let h = 28.0;
        let below = r.top() < 34.0;
        let top = if below { r.bottom() + 6.0 } else { r.top() - 6.0 - h };
        let label = Rect::from_min_size(Pos2::new(r.left(), top), Vec2::new(name_w + 8.0 + tools_w, h));
        let tools_left = r.left() + name_w + 8.0;
        let minus = Rect::from_min_size(Pos2::new(tools_left + 2.0, top + 3.0), Vec2::new(24.0, 22.0));
        let plus = Rect::from_min_size(Pos2::new(minus.right() + 2.0 + pct_w + 2.0, top + 3.0), Vec2::new(24.0, 22.0));
        let display = disp.is_some().then(|| Rect::from_min_size(Pos2::new(plus.right() + 2.0 + 2.0, top + 3.0), Vec2::new(disp_w - 2.0, 22.0)));
        let hide = Rect::from_min_size(Pos2::new(display.map_or(plus.right(), |d| d.right()) + 2.0 + 4.0, top + 3.0), Vec2::new(hide_w, 22.0));
        Some((label, [minus, plus, hide], display))
    }

    fn handle_rect(r: Rect) -> Rect {
        Rect::from_min_size(Pos2::new(r.right() + 9.0 - 14.0, r.bottom() + 9.0 - 14.0), Vec2::splat(14.0))
    }

    fn hit(&self, ctx: &egui::Context, store: &Store, p: Pos2, key: &str, next: Option<u32>) -> Option<Hit> {
        let th = Theme::from_general(&store.config.general);
        for i in (0..self.slots.len()).rev() {
            let Some(r) = self.rect_on(i, key) else { continue };
            if let Some((_, [minus, plus, hide], display)) = self.label_rect(ctx, &th, store, i, key, next) {
                if display.is_some_and(|d| d.contains(p)) {
                    return Some(Hit::NextDisplay(i));
                }
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
        (0..self.slots.len()).rev().find(|&i| self.rect_on(i, key).is_some_and(|r| r.contains(p))).map(Hit::Body)
    }

    /// Outlines, labels with tools, resize handles and snapping guides.
    fn chrome(&self, ctx: &egui::Context, th: &Theme, store: &Store, screen: Rect, displays: &[Display], me: &Display) {
        let next = displays::next_display(&me.key, displays).map(|d| d.number);
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
            let Some(r) = self.rect_on(i, &me.key) else { continue };
            kit::dashed_rect(&p, r.expand(3.5), alpha(th.attention, 0.75), 1.0, 4.0, 3.0);
            let Some((label, [minus, plus, hide], display)) = self.label_rect(ctx, th, store, i, &me.key, next) else { continue };
            let y = label.center().y;
            // The name, with a shadow so it reads over anything.
            kit::text(&p, Pos2::new(label.left() + 1.0, y + 1.0), Align2::LEFT_CENTER, slot.meta.name, th.bold(13.0), Color32::from_black_alpha(220));
            kit::text(&p, Pos2::new(label.left(), y), Align2::LEFT_CENTER, slot.meta.name, th.bold(13.0), th.attention);
            if matches!(&self.drag, Some(Drag::Move { idx, .. }) if *idx == i) {
                if let Some(n) = self.leaving {
                    let text = if n > 0 { format!("Release to move to display {n}") } else { "Move onto another display".to_string() };
                    let w = kit::width(&p, &text, th.bold(13.0)) + 20.0;
                    let box_ = Rect::from_center_size(r.center(), Vec2::new(w, 26.0));
                    kit::fill(&p, box_, 6.0, th.attention);
                    kit::text(&p, box_.center(), Align2::CENTER_CENTER, &text, th.bold(13.0), th.panel_solid);
                }
            }
            let tools = Rect::from_min_max(Pos2::new(minus.left() - 2.0, label.top() + 1.0), Pos2::new(hide.right() + 2.0, label.bottom() - 1.0));
            kit::fill(&p, tools, 6.0, th.panel_solid);
            kit::outline(&p, tools, 6.0, 1.0, th.line_strong);
            if let Some(d) = display {
                p.vline(d.left() - 2.0, d.y_range(), Stroke::new(1.0, th.line_strong));
                let c = if hover.is_some_and(|h| d.contains(h)) { th.attention } else { th.ink2 };
                kit::text(&p, d.center(), Align2::CENTER_CENTER, &format!("→ {}", next.unwrap_or(0)), th.bold(12.0), c);
            }
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

/// What a widget is drawn from (the same for every widget in a frame).
struct Paint<'a> {
    ctx: &'a egui::Context,
    th: &'a Theme,
    store: &'a Store,
    live: &'a Live,
    ratings: &'a Ratings,
    units: crate::config::Units,
    editing: bool,
    now: f64,
    hover: Option<Pos2>,
}

/// Draws one widget at its own origin on its own layer (unscaled, untransformed),
/// catching a panic. Returns its size, the clickable parts and the layer.
fn paint_slot(slot: &mut Slot, paint: &Paint, cfg: &crate::config::WidgetCfg, prev_rect: Option<Rect>) -> (Vec2, Vec<widgets::Click>, LayerId) {
    let Paint { ctx, th, store, live, ratings, units, editing, now, hover } = *paint;
    let clicks = RefCell::new(Vec::new());
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
            clicks: &clicks,
            pointer: hover.zip(prev_rect).map(|(h, r)| ((h - r.min) / cfg.scale).to_pos2()),
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
                eprintln!("[tenths] the {} widget failed: {msg}", slot.meta.id);
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
    (size, clicks.into_inner(), layer)
}

impl Slot {
    /// Top-left on the screen (relative to its origin) at `scale`.
    fn rect_from_cfg(&self, cfg: &crate::config::WidgetCfg, scale: f32, screen: Vec2) -> Pos2 {
        anchor::resolve(cfg, self.natural * scale, screen, self.natural)
    }
}

/// Saves an absolute position (and scale) as an anchored one.
/// With `display`, on that display (a drop onto it, or the next-display button).
fn save(store: &mut Store, id: &str, left_top: Pos2, size: Vec2, scale: f32, screen: Vec2, display: Option<&str>) {
    let p = anchor::clamp_on_screen(left_top.x, left_top.y, size, screen);
    let (ax, ay, x, y) = anchor::to_anchored(p.x, p.y, size, screen);
    let scale = (scale as f64 * 100.0).round() / 100.0;
    let mut patch = vec![("x", json!(x as f64)), ("y", json!(y as f64)), ("ax", json!(ax.key())), ("ay", json!(ay.key())), ("scale", json!(scale))];
    if let Some(d) = display {
        patch.push(("display", json!(d)));
    }
    store.save_widget(id, &patch);
}
