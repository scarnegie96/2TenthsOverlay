//! Twitch chat (experimental, `src/widgets/TwitchChat.tsx`): a channel's live
//! chat, read-only. Newest at the bottom; older messages fade out after a
//! while if asked, so it stays quiet when chat is. The connection and parser
//! are in `logic/twitch.rs`.
//!
//! Emotes are pictures (fetched once each in the background, see
//! `logic/emotes.rs`), placed inline with the words; until one arrives, or if
//! it can't, its code shows as text.
//!
//! Demo: a made-up chat plays instead of connecting when `TENTHS_CHAT_DEMO`
//! is set (on any channel, or none), or when the channel is `tenths_racing`
//! and the app runs with `--demo` (the default off Windows), like the web
//! `?chat` preview.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::epaint::Galley;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Painter, Pos2, Rect, TextureHandle, TextureId, TextureOptions, Vec2};

use super::{Ctx, Widget};
use crate::kit::{self, seg};
use crate::logic::emotes::{self, Emotes};
use crate::logic::twitch::{channel_name, demo_message, flow, pieces, split_to_fit, Chat, ChatLog, ChatMessage, ChatStatus, Item, Token};
use crate::theme::{hex_or, Theme};

const W: f32 = 340.0;
const LINE_H: f32 = 14.0 * 1.3;
const EMOTE_H: f32 = 18.0;
const DEMO_EVERY: f64 = 2.5;
/// Emote textures kept (about 3 KB each at 1x, 12 KB at 2x).
const MAX_TEXTURES: usize = 200;
/// The demo chat's emote (Kappa, id 25), from a bundled picture so it shows without the network.
const DEMO_EMOTE: (&str, &[u8]) = ("25", include_bytes!("../../assets/emotes/demo-face.png"));

#[derive(Default)]
pub struct Twitch {
    /// The channel we're connected to (or playing the demo for) and how.
    source: Option<(String, Source)>,
    /// Emote pictures for this source, and the textures made from them.
    emotes: Option<Emotes>,
    tex: HashMap<emotes::Key, Tex>,
    /// Frames drawn, for dropping the least recently used texture.
    tick: u64,
}

enum Source {
    Live(Chat),
    Demo { log: ChatLog, next: usize, next_at: f64 },
}

fn demo_wanted(channel: &Option<String>) -> Option<String> {
    if std::env::var_os("TENTHS_CHAT_DEMO").is_some_and(|v| !v.is_empty() && v != "0") {
        return Some(channel.clone().unwrap_or_else(|| "tenths_racing".to_string()));
    }
    let with_demo_flag = std::env::args().any(|a| a == "--demo") || cfg!(not(windows));
    channel.clone().filter(|c| c == "tenths_racing" && with_demo_flag)
}

impl Widget for Twitch {
    fn draw(&mut self, p: &Painter, cx: &Ctx) -> Vec2 {
        let th = cx.th;
        let raw = cx.o.s("channel");
        let channel = channel_name(raw);
        let rows = cx.o.int("rows", 1);
        let fade_after = cx.o.n("fadeAfter");
        let show_emotes = cx.o.b("showEmotes");

        // The channel to show: the demo's, or the real one (re)connected on a change.
        let demo = demo_wanted(&channel);
        let shown_channel = demo.clone().or(channel.clone());
        if self.source.as_ref().map(|(c, _)| c) != shown_channel.as_ref() {
            self.source = shown_channel.as_ref().map(|c| {
                let source = if demo.is_some() {
                    Source::Demo { log: ChatLog { status: ChatStatus::Connected, ..Default::default() }, next: 0, next_at: cx.now }
                } else {
                    let ctx = cx.ctx.clone();
                    Source::Live(Chat::connect(c, move || ctx.request_repaint()))
                };
                (c.clone(), source)
            });
            // Fresh pictures per channel; the demo's one is bundled.
            self.tex.clear();
            let ctx = cx.ctx.clone();
            let e = Emotes::new(move || ctx.request_repaint());
            if demo.is_some() {
                for size in [1, 2] {
                    e.seed(DEMO_EMOTE.0, size, DEMO_EMOTE.1);
                }
            }
            self.emotes = Some(e);
        }
        let log = match &mut self.source {
            Some((_, Source::Live(chat))) => chat.snapshot(),
            Some((_, Source::Demo { log, next, next_at })) => {
                if *next == 0 {
                    for _ in 0..5 {
                        log.add(demo_message(*next, Instant::now()));
                        *next += 1;
                    }
                    *next_at = cx.now + DEMO_EVERY;
                }
                while cx.now >= *next_at {
                    log.add(demo_message(*next, Instant::now()));
                    *next += 1;
                    *next_at += DEMO_EVERY;
                }
                log.clone()
            }
            None => ChatLog::default(),
        };
        if shown_channel.is_some() && (fade_after > 0.0 || demo.is_some()) {
            cx.ctx.request_repaint_after(Duration::from_secs(1));
        }

        // Header: the connection's dot and #channel.
        let aside_font = kit::aside_font(th);
        let aside_text = shown_channel.as_ref().map(|c| format!("#{c}"));
        let aside: Vec<_> = aside_text.iter().map(|t| seg(t.clone(), aside_font.clone(), th.ink2)).collect();
        let panel = kit::Panel::begin(p, th, W, Some("Twitch chat"), &aside);
        if let Some(t) = &aside_text {
            let spacing = if th.f1() { 14.0 * 0.04 } else { 0.0 };
            let shown = if th.f1() { t.to_uppercase() } else { t.clone() };
            let w = kit::segs_width(p, &[seg(shown, aside_font, th.ink2)], spacing);
            let color = match log.status {
                ChatStatus::Connected => th.gain,
                ChatStatus::Error | ChatStatus::Reconnecting => th.attention,
                ChatStatus::Connecting => th.ink3,
            };
            let x = panel.x().max - 10.0 - w - 5.0 - 3.5;
            p.circle_filled(Pos2::new(x, panel.inset + (kit::HEAD_H - 1.0) / 2.0 + 1.0), 3.5, color);
        }

        let msg = match &shown_channel {
            None if raw.trim().is_empty() => Some("Set a channel in this widget's settings.".to_string()),
            None => Some(format!("\"{raw}\" isn't a Twitch channel name.")),
            Some(_) => None,
        };
        if let Some(msg) = msg {
            let h = kit::empty(p, th, panel.x(), panel.body_top, &msg);
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        }

        let start = log.messages.len().saturating_sub(rows);
        let visible: Vec<&ChatMessage> =
            log.messages.iter().skip(start).filter(|m| fade_after <= 0.0 || m.at.elapsed().as_secs_f64() < fade_after).collect();
        if visible.is_empty() {
            let msg = match log.status {
                ChatStatus::Connected => "No new messages.",
                ChatStatus::Error => "Can't reach Twitch chat.",
                _ => "Joining chat…",
            };
            let h = kit::empty(p, th, panel.x(), panel.body_top, msg);
            let bottom = panel.body_top + h;
            return panel.finish(p, th, bottom);
        }

        let left = panel.x().min + 10.0;
        let width = panel.x().span() - 20.0;
        let on_screen = cx.cfg.widget("twitch").scale * cx.ctx.pixels_per_point();
        let size = emotes::size_for(on_screen);
        self.tick += 1;
        let space = {
            let w = |t: &str| p.layout_no_wrap(t.to_string(), th.medium(14.0), th.ink).size().x;
            w("a a") - w("aa")
        };
        let mut y = panel.body_top + 8.0;
        for m in visible {
            let ents = self.entries(p, cx, th, m, show_emotes, size, space, width);
            let items: Vec<Item> = ents.iter().map(|e| Item { w: e.w(), gap: e.gap }).collect();
            let placed = flow(&items, width);
            let lines = placed.last().map_or(1, |l| l.line + 1);
            for (e, at) in ents.into_iter().zip(&placed) {
                let top = y + at.line as f32 * LINE_H;
                match e.what {
                    What::Text(g) => {
                        let off = (LINE_H - g.size().y) / 2.0;
                        p.galley(Pos2::new(left + at.x, top + off), g, th.ink);
                    }
                    What::Image(id, w) => {
                        let r = Rect::from_min_size(Pos2::new(left + at.x, top + (LINE_H - EMOTE_H) / 2.0), Vec2::new(w, EMOTE_H));
                        p.image(id, r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
                    }
                }
            }
            y += lines as f32 * LINE_H + 3.0;
        }
        panel.finish(p, th, y - 3.0 + 10.0)
    }
}

/// One placed thing in a message: a word (laid out) or an emote picture.
struct Entry {
    what: What,
    gap: f32,
}

enum What {
    Text(Arc<Galley>),
    Image(TextureId, f32),
}

impl Entry {
    fn w(&self) -> f32 {
        match &self.what {
            What::Text(g) => g.size().x,
            What::Image(_, w) => *w,
        }
    }
}

/// An emote's texture, and when it was last drawn (for dropping the least recent).
struct Tex {
    handle: TextureHandle,
    /// Width over height.
    aspect: f32,
    used: u64,
}

impl Twitch {
    /// The picture for an emote if it's here, asking for it if not.
    fn emote_tex(&mut self, cx: &Ctx, id: &str, size: u8) -> Option<(TextureId, f32)> {
        let key = (id.to_string(), size);
        if let Some(t) = self.tex.get_mut(&key) {
            t.used = self.tick;
            return Some((t.handle.id(), t.aspect));
        }
        let emotes = self.emotes.as_ref()?;
        let emotes::Poll::Ready(img) = emotes.poll(id, size) else { return None };
        let color = egui::ColorImage::from_rgba_unmultiplied([img.w, img.h], &img.rgba);
        let handle = cx.ctx.load_texture(format!("emote-{id}-{size}"), color, TextureOptions::LINEAR);
        let (aspect, tid) = (img.w as f32 / img.h as f32, handle.id());
        self.tex.insert(key, Tex { handle, aspect, used: self.tick });
        // Bounded: drop the least recently drawn (never one used this frame).
        while self.tex.len() > MAX_TEXTURES {
            let Some(old) = self.tex.iter().filter(|(_, t)| t.used != self.tick).min_by_key(|(_, t)| t.used).map(|(k, _)| k.clone()) else { break };
            self.tex.remove(&old);
            emotes.forget(&old.0, old.1);
        }
        Some((tid, aspect))
    }

    /// A message as things to place: bold coloured name, ":", then its words and emotes
    /// (italic for /me). An emote without its picture (yet, or ever) is its code in text.
    #[allow(clippy::too_many_arguments)]
    fn entries(&mut self, p: &Painter, cx: &Ctx, th: &Theme, m: &ChatMessage, show_emotes: bool, size: u8, space: f32, width: f32) -> Vec<Entry> {
        let text = |t: &str, font: egui::FontId, color: Color32, italics: bool| {
            let fmt = TextFormat { font_id: font, color, italics, ..Default::default() };
            p.layout_job(LayoutJob::single_section(t.to_string(), fmt))
        };
        let mut out = Vec::new();
        let name_color = m.color.as_deref().map(|c| hex_or(c, th.ink)).unwrap_or(th.ink);
        out.push(Entry { what: What::Text(text(&m.name, th.bold(14.0), name_color, false)), gap: 0.0 });
        if !m.action {
            out.push(Entry { what: What::Text(text(":", th.medium(14.0), th.ink, false)), gap: 0.0 });
        }
        for piece in pieces(&m.text, &m.emotes, show_emotes) {
            let gap = if piece.gap { space } else { 0.0 };
            let (word, font, color) = match &piece.token {
                Token::Emote { id, code } => {
                    if let Some((tid, aspect)) = self.emote_tex(cx, id, size) {
                        out.push(Entry { what: What::Image(tid, (EMOTE_H * aspect).min(EMOTE_H * 3.0)), gap });
                        continue;
                    }
                    (code.as_str(), th.semi(14.0), th.ink2)
                }
                Token::Word(w) => (w.as_str(), th.medium(14.0), th.ink),
            };
            let measure = |t: &str| p.layout_no_wrap(t.to_string(), font.clone(), color).size().x;
            let chunks = if measure(word) > width { split_to_fit(word, width, measure) } else { vec![word.to_string()] };
            for (i, c) in chunks.iter().enumerate() {
                out.push(Entry { what: What::Text(text(c, font.clone(), color, m.action)), gap: if i == 0 { gap } else { 0.0 } });
            }
        }
        out
    }
}
