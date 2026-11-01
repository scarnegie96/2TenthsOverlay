//! Twitch chat (experimental, `src/widgets/TwitchChat.tsx`): a channel's live
//! chat, read-only. Newest at the bottom; older messages fade out after a
//! while if asked, so it stays quiet when chat is. The connection and parser
//! are in `logic/twitch.rs`.
//!
//! Emotes are shown as their codes (text), not images.
//!
//! Demo: a made-up chat plays instead of connecting when `TENTHS_CHAT_DEMO`
//! is set (on any channel, or none), or when the channel is `tenths_racing`
//! and the app runs with `--demo` (the default off Windows), like the web
//! `?chat` preview.

use std::time::{Duration, Instant};

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Align, Painter, Pos2, Vec2};

use super::{Ctx, Widget};
use crate::kit::{self, seg};
use crate::logic::twitch::{channel_name, demo_message, Chat, ChatLog, ChatMessage, ChatStatus};
use crate::theme::{hex_or, Theme};

const W: f32 = 340.0;
const LINE_H: f32 = 14.0 * 1.3;
const DEMO_EVERY: f64 = 2.5;

#[derive(Default)]
pub struct Twitch {
    /// The channel we're connected to (or playing the demo for) and how.
    source: Option<(String, Source)>,
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
        let mut y = panel.body_top + 8.0;
        for m in visible {
            let galley = p.layout_job(message_job(th, m, show_emotes, width));
            let h = galley.size().y;
            p.galley(Pos2::new(left, y), galley, th.ink);
            y += h + 3.0;
        }
        panel.finish(p, th, y - 3.0 + 10.0)
    }
}

/// One message: bold coloured name, then the text (italic for /me), wrapped
/// anywhere. Emote codes (text, not images) stand out a little.
fn message_job(th: &Theme, m: &ChatMessage, show_emotes: bool, width: f32) -> LayoutJob {
    let mut job = LayoutJob { wrap: TextWrapping { max_width: width, max_rows: usize::MAX, break_anywhere: true, overflow_character: None }, ..Default::default() };
    let fmt = |font, color, italics| TextFormat { font_id: font, color, italics, line_height: Some(LINE_H), valign: Align::Center, ..Default::default() };
    job.append(&m.name, 0.0, fmt(th.bold(14.0), m.color.as_deref().map(|c| hex_or(c, th.ink)).unwrap_or(th.ink), false));
    job.append(if m.action { " " } else { ": " }, 0.0, fmt(th.medium(14.0), th.ink, false));

    // Emote positions count code points, not bytes.
    let chars: Vec<char> = m.text.chars().collect();
    let text_fmt = fmt(th.medium(14.0), th.ink, m.action);
    let emote_fmt = fmt(th.semi(14.0), th.ink2, m.action);
    let mut at = 0;
    if show_emotes {
        for e in &m.emotes {
            if e.start < at || e.end >= chars.len() {
                continue;
            }
            if e.start > at {
                job.append(&chars[at..e.start].iter().collect::<String>(), 0.0, text_fmt.clone());
            }
            job.append(&chars[e.start..=e.end].iter().collect::<String>(), 0.0, emote_fmt.clone());
            at = e.end + 1;
        }
    }
    if at < chars.len() {
        job.append(&chars[at..].iter().collect::<String>(), 0.0, text_fmt);
    }
    job
}
