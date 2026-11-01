//! Read-only Twitch chat over Twitch's IRC WebSocket (`src/lib/twitch.ts`).
//! No account or token is needed to read a public channel: Twitch lets
//! anonymous "justinfan" logins join and listen. Messages arrive with IRCv3
//! tags (name colour, display name, emotes), and moderation (a deleted message,
//! a timeout or ban) removes what was shown.
//!
//! The parser and the protocol decisions are plain functions (tested below);
//! `Chat` runs the connection on its own thread.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const URL: &str = "wss://irc-ws.chat.twitch.tv:443";

/// Messages kept in memory, whatever the widget shows.
pub const KEEP: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatEmote {
    pub id: String,
    /// Code point positions in the text, inclusive.
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    /// Lower-case login, for timeouts and bans.
    pub login: String,
    pub name: String,
    pub color: Option<String>,
    pub text: String,
    pub emotes: Vec<ChatEmote>,
    /// "/me" messages.
    pub action: bool,
    /// When it arrived.
    pub at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatStatus {
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Valid Twitch login: 3-25 letters, digits or underscores. Takes a name, "#name" or a twitch.tv link.
pub fn channel_name(raw: &str) -> Option<String> {
    let mut c = raw.trim();
    c = c.strip_prefix('#').unwrap_or(c);
    let lower = c.to_ascii_lowercase();
    for prefix in ["https://www.twitch.tv/", "https://twitch.tv/", "http://www.twitch.tv/", "http://twitch.tv/"] {
        if lower.starts_with(prefix) {
            c = &c[prefix.len()..];
            break;
        }
    }
    let c = c.split(['/', '?', '#']).next().unwrap_or("").to_lowercase();
    let ok = (3..=25).contains(&c.chars().count()) && c.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');
    ok.then_some(c)
}

/// IRCv3 tag values escape ; space \ CR LF.
fn unescape_tag(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut it = v.chars();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match it.next() {
            Some(':') => out.push(';'),
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[derive(Debug, Clone, Default)]
pub struct IrcLine {
    pub tags: HashMap<String, String>,
    pub prefix: Option<String>,
    pub command: String,
    pub params: Vec<String>,
}

impl IrcLine {
    pub fn tag(&self, key: &str) -> &str {
        self.tags.get(key).map(String::as_str).unwrap_or("")
    }
}

pub fn parse_line(line: &str) -> Option<IrcLine> {
    let mut rest = line;
    let mut tags = HashMap::new();
    if let Some(t) = rest.strip_prefix('@') {
        let (tag_part, after) = t.split_once(' ').unwrap_or((t, ""));
        for kv in tag_part.split(';') {
            match kv.split_once('=') {
                None => tags.insert(kv.to_string(), String::new()),
                Some((k, v)) => tags.insert(k.to_string(), unescape_tag(v)),
            };
        }
        rest = after;
    }
    let mut prefix = None;
    if let Some(r) = rest.strip_prefix(':') {
        let (pre, after) = r.split_once(' ').unwrap_or((r, ""));
        prefix = Some(pre.to_string());
        rest = after;
    }
    let mut params: Vec<String> = Vec::new();
    while !rest.is_empty() {
        if let Some(trailing) = rest.strip_prefix(':') {
            params.push(trailing.to_string());
            break;
        }
        match rest.split_once(' ') {
            None => {
                params.push(rest.to_string());
                break;
            }
            Some((a, b)) => {
                params.push(a.to_string());
                rest = b;
            }
        }
    }
    if params.is_empty() {
        return None;
    }
    let command = params.remove(0);
    if command.is_empty() {
        return None;
    }
    Some(IrcLine { tags, prefix, command, params })
}

/// "25:0-4,12-16/1902:6-10" -> emotes, in text order.
pub fn parse_emotes(tag: &str) -> Vec<ChatEmote> {
    let mut out = Vec::new();
    for part in tag.split('/') {
        let Some((id, ranges)) = part.split_once(':') else { continue };
        if id.is_empty() || ranges.is_empty() {
            continue;
        }
        for r in ranges.split(',') {
            let Some((a, b)) = r.split_once('-') else { continue };
            if let (Ok(start), Ok(end)) = (a.parse::<usize>(), b.parse::<usize>()) {
                out.push(ChatEmote { id: id.to_string(), start, end });
            }
        }
    }
    out.sort_by_key(|e| e.start);
    out
}

/// A PRIVMSG as a chat message. `seed` makes up an id when the line has none.
pub fn to_message(l: &IrcLine, at: Instant, seed: u64) -> Option<ChatMessage> {
    if l.command != "PRIVMSG" {
        return None;
    }
    let login = l.prefix.as_deref().unwrap_or("").split('!').next().unwrap_or("").to_string();
    let mut text = l.params.get(1).cloned().unwrap_or_default();
    let mut action = false;
    // "/me" arrives wrapped in CTCP: \x01ACTION ...\x01.
    if text.starts_with("\u{1}ACTION ") && text.ends_with('\u{1}') && text.len() >= 9 {
        text = text[8..text.len() - 1].to_string();
        action = true;
    }
    let id = match l.tag("id") {
        "" => format!("{login}-{seed}"),
        id => id.to_string(),
    };
    let name = match l.tag("display-name") {
        "" => login.clone(),
        n => n.to_string(),
    };
    Some(ChatMessage {
        id,
        login,
        name,
        color: Some(l.tag("color").to_string()).filter(|c| !c.is_empty()),
        text,
        emotes: parse_emotes(l.tag("emotes")),
        action,
        at,
    })
}

/// What a line from the server means for us.
#[derive(Debug, Clone)]
pub enum Event {
    Message(ChatMessage),
    /// A message removed by a moderator.
    Deleted(String),
    /// Everything from one user removed (timeout or ban), or the whole chat cleared (None).
    Cleared(Option<String>),
    /// Joined the channel (JOIN or ROOMSTATE).
    Joined,
    /// Answer a PING with this.
    Pong(String),
    /// The server asks us to reconnect.
    Reconnect,
}

pub fn react(l: &IrcLine, at: Instant, seed: u64) -> Option<Event> {
    match l.command.as_str() {
        "PING" => Some(Event::Pong(l.params.first().cloned().unwrap_or_else(|| "tmi.twitch.tv".into()))),
        "JOIN" | "ROOMSTATE" => Some(Event::Joined),
        "PRIVMSG" => to_message(l, at, seed).map(Event::Message),
        "CLEARMSG" => Some(l.tag("target-msg-id")).filter(|id| !id.is_empty()).map(|id| Event::Deleted(id.to_string())),
        "CLEARCHAT" => Some(Event::Cleared(l.params.get(1).map(|u| u.to_lowercase()))),
        "RECONNECT" => Some(Event::Reconnect),
        _ => None,
    }
}

/// The lines sent after connecting, in order.
pub fn handshake(nick_number: u32, channel: &str) -> Vec<String> {
    vec![
        "CAP REQ :twitch.tv/tags twitch.tv/commands".to_string(),
        "PASS SCHMOOPIIE".to_string(),
        format!("NICK justinfan{nick_number}"),
        format!("JOIN #{channel}"),
    ]
}

/// A random anonymous nick number, 10000..90000.
pub fn nick_number() -> u32 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    10_000 + (nanos ^ (nanos >> 7)) % 80_000
}

/// Back off: 1 s, 2 s, 4 s... up to 30 s.
pub fn backoff(retry: u32) -> Duration {
    Duration::from_millis((1000u64.saturating_mul(1u64 << retry.min(10))).min(30_000))
}

/// The messages and status shown, with moderation applied.
#[derive(Debug, Clone)]
pub struct ChatLog {
    pub messages: VecDeque<ChatMessage>,
    pub status: ChatStatus,
}

impl Default for ChatLog {
    fn default() -> Self {
        Self { messages: VecDeque::new(), status: ChatStatus::Connecting }
    }
}

impl ChatLog {
    pub fn add(&mut self, m: ChatMessage) {
        while self.messages.len() >= KEEP {
            self.messages.pop_front();
        }
        self.messages.push_back(m);
    }

    pub fn deleted(&mut self, id: &str) {
        self.messages.retain(|m| m.id != id);
    }

    pub fn cleared(&mut self, login: Option<&str>) {
        match login {
            Some(l) => self.messages.retain(|m| m.login != l),
            None => self.messages.clear(),
        }
    }

    pub fn apply(&mut self, e: Event) {
        match e {
            Event::Message(m) => self.add(m),
            Event::Deleted(id) => self.deleted(&id),
            Event::Cleared(l) => self.cleared(l.as_deref()),
            Event::Joined => self.status = ChatStatus::Connected,
            Event::Pong(_) | Event::Reconnect => {}
        }
    }
}

// --- The connection ------------------------------------------------------------------

/// A live connection to one channel's chat. Dropping it disconnects.
pub struct Chat {
    log: Arc<Mutex<ChatLog>>,
    stop: Arc<AtomicBool>,
}

impl Chat {
    /// Connects on its own thread and keeps reconnecting. `wake` is called when something changes.
    pub fn connect(channel: &str, wake: impl Fn() + Send + 'static) -> Self {
        let log = Arc::new(Mutex::new(ChatLog::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (l, s, channel) = (log.clone(), stop.clone(), channel.to_string());
        std::thread::Builder::new()
            .name("twitch-chat".into())
            .spawn(move || run(&channel, &l, &s, &wake))
            .ok();
        Self { log, stop }
    }

    pub fn snapshot(&self) -> ChatLog {
        self.log.lock().map(|l| l.clone()).unwrap_or_default()
    }
}

impl Drop for Chat {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Sleeps `d`, waking early when stopped. Returns true if stopped.
fn nap(d: Duration, stop: &AtomicBool) -> bool {
    let end = Instant::now() + d;
    while Instant::now() < end {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    stop.load(Ordering::Relaxed)
}

type Socket = tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>;

fn set_read_timeout(ws: &mut Socket, d: Duration) {
    use tungstenite::stream::MaybeTlsStream;
    match ws.get_mut() {
        MaybeTlsStream::Plain(s) => {
            let _ = s.set_read_timeout(Some(d));
        }
        MaybeTlsStream::Rustls(s) => {
            let _ = s.get_ref().set_read_timeout(Some(d));
        }
        _ => {}
    }
}

fn run(channel: &str, log: &Mutex<ChatLog>, stop: &AtomicBool, wake: &dyn Fn()) {
    let set = |s: ChatStatus| {
        if let Ok(mut l) = log.lock() {
            l.status = s;
        }
        wake();
    };
    let mut retry: u32 = 0;
    // Couldn't connect: stays "error" (so the widget says so) until a retry gets through.
    let mut failed = false;
    let mut seed: u64 = 0;
    while !stop.load(Ordering::Relaxed) {
        if !failed {
            set(if retry > 0 { ChatStatus::Reconnecting } else { ChatStatus::Connecting });
        }
        match tungstenite::connect(URL) {
            Err(_) => {
                failed = true;
                set(ChatStatus::Error);
            }
            Ok((mut ws, _)) => {
                set_read_timeout(&mut ws, Duration::from_millis(500));
                let mut sent = true;
                for line in handshake(nick_number(), channel) {
                    sent &= ws.send(tungstenite::Message::text(line)).is_ok();
                }
                if !sent {
                    failed = true;
                    set(ChatStatus::Error);
                } else {
                    loop {
                        if stop.load(Ordering::Relaxed) {
                            let _ = ws.close(None);
                            return;
                        }
                        match ws.read() {
                            Ok(tungstenite::Message::Text(data)) => {
                                let mut reconnect = false;
                                for raw in data.split("\r\n").filter(|r| !r.is_empty()) {
                                    seed += 1;
                                    let Some(line) = parse_line(raw) else { continue };
                                    match react(&line, Instant::now(), seed) {
                                        Some(Event::Pong(token)) => {
                                            let _ = ws.send(tungstenite::Message::text(format!("PONG :{token}")));
                                        }
                                        Some(Event::Reconnect) => reconnect = true,
                                        Some(e) => {
                                            if matches!(e, Event::Joined) {
                                                retry = 0;
                                                failed = false;
                                            }
                                            if let Ok(mut l) = log.lock() {
                                                l.apply(e);
                                            }
                                            wake();
                                        }
                                        None => {}
                                    }
                                }
                                if reconnect {
                                    let _ = ws.close(None);
                                    break;
                                }
                            }
                            Ok(tungstenite::Message::Close(_)) => break,
                            Ok(_) => {}
                            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => break,
                            Err(_) => {
                                failed = true;
                                set(ChatStatus::Error);
                                break;
                            }
                        }
                    }
                }
            }
        }
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let wait = backoff(retry);
        retry += 1;
        if !failed {
            set(ChatStatus::Reconnecting);
        }
        if nap(wait, stop) {
            return;
        }
    }
}

// --- Demo ------------------------------------------------------------------------------

/// The made-up chat of the web `?chat` preview: (name, colour, text).
const DEMO: [(&str, &str, &str); 6] = [
    ("PitWallPete", "#1E90FF", "that pass into turn 1 was clean"),
    ("sim_sarah", "#FF69B4", "what's the fuel looking like?"),
    ("LateApex_", "#9ACD32", "box box box"),
    ("Grumpy_Marshal", "#FF4500", "track limits!!"),
    ("tenths_fan", "#DAA520", "gap to P3 is coming down"),
    ("CurbHopper", "#00CED1", "send it"),
];

/// The `k`th demo message. Every so often one has an emote or is a /me, so
/// those render too.
pub fn demo_message(k: usize, at: Instant) -> ChatMessage {
    let (name, color, text) = DEMO[k % DEMO.len()];
    let (text, emotes, action) = match k % 9 {
        4 => (format!("{text} Kappa"), vec![ChatEmote { id: "25".into(), start: text.chars().count() + 1, end: text.chars().count() + 5 }], false),
        7 => ("waves at the pit crew".to_string(), Vec::new(), true),
        _ => (text.to_string(), Vec::new(), false),
    };
    ChatMessage { id: format!("demo-{k}"), login: name.to_lowercase(), name: name.to_string(), color: Some(color.into()), text, emotes, action, at }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "@badge-info=;badges=moderator/1;color=#1E90FF;display-name=Pit\\sWall;emotes=25:0-4,12-16/1902:6-10;id=abc-123;mod=1 \
                        :pitwall!pitwall@pitwall.tmi.twitch.tv PRIVMSG #tenths :Kappa Keepo Kappa";

    #[test]
    fn parses_command_channel_and_escaped_tags() {
        let l = parse_line(LINE).unwrap();
        assert!(l.command == "PRIVMSG" && l.params[0] == "#tenths", "command and channel");
        assert_eq!(l.tag("display-name"), "Pit Wall", "escaped tag value");
    }

    #[test]
    fn message_fields() {
        let l = parse_line(LINE).unwrap();
        let m = to_message(&l, Instant::now(), 1).unwrap();
        assert!(m.id == "abc-123" && m.login == "pitwall" && m.name == "Pit Wall" && m.color.as_deref() == Some("#1E90FF"), "message fields");
        assert!(m.text == "Kappa Keepo Kappa" && !m.action, "text");
        let e: Vec<(&str, usize, usize)> = m.emotes.iter().map(|e| (e.id.as_str(), e.start, e.end)).collect();
        assert_eq!(e, [("25", 0, 4), ("1902", 6, 10), ("25", 12, 16)], "emotes in order");
    }

    #[test]
    fn me_actions() {
        let l = parse_line(":a!a@a.tmi.twitch.tv PRIVMSG #c :\u{1}ACTION waves\u{1}").unwrap();
        let me = to_message(&l, Instant::now(), 1).unwrap();
        assert!(me.action && me.text == "waves", "/me");
    }

    #[test]
    fn ping_and_clearchat() {
        assert_eq!(parse_line("PING :tmi.twitch.tv").unwrap().command, "PING", "ping");
        let clear = parse_line("@room-id=1 :tmi.twitch.tv CLEARCHAT #tenths :spammer").unwrap();
        assert!(clear.command == "CLEARCHAT" && clear.params[1] == "spammer", "timeout");
        assert!(parse_emotes("").is_empty(), "no emotes");
    }

    #[test]
    fn channel_names() {
        assert_eq!(channel_name("Tenths_Racing").as_deref(), Some("tenths_racing"), "plain name");
        assert_eq!(channel_name("https://www.twitch.tv/SimSarah/videos").as_deref(), Some("simsarah"), "link");
        assert_eq!(channel_name("#abc").as_deref(), Some("abc"), "hash");
        assert!(channel_name("no spaces allowed").is_none() && channel_name("ab").is_none(), "invalid");
        assert_eq!(channel_name("  twitch.tv/x_y_z?foo=1  ").as_deref(), None, "no scheme: not a link");
        assert_eq!(channel_name("HTTP://TWITCH.TV/Abc?x").as_deref(), Some("abc"), "scheme and host in any case");
    }

    #[test]
    fn lines_without_tags_or_trailing_and_junk() {
        let l = parse_line(":tmi.twitch.tv 001 justinfan1 :Welcome, GLHF!").unwrap();
        assert!(l.command == "001" && l.params == ["justinfan1", "Welcome, GLHF!"]);
        assert!(l.prefix.as_deref() == Some("tmi.twitch.tv"));
        assert!(parse_line("").is_none() && parse_line("@a=b").is_none());
        let t = parse_line("@flag;k=a\\:b\\\\c :x PING").unwrap();
        assert_eq!(t.tag("flag"), "");
        assert_eq!(t.tag("k"), "a;b\\c");
    }

    #[test]
    fn name_falls_back_to_login_and_id_is_made_up() {
        let l = parse_line(":bob!bob@bob.tmi.twitch.tv PRIVMSG #c :hi").unwrap();
        let m = to_message(&l, Instant::now(), 7).unwrap();
        assert!(m.name == "bob" && m.id == "bob-7" && m.color.is_none());
        assert!(to_message(&parse_line("PING :x").unwrap(), Instant::now(), 0).is_none());
    }

    #[test]
    fn protocol_reactions() {
        let at = Instant::now();
        let ev = |s: &str| react(&parse_line(s).unwrap(), at, 1);
        assert!(matches!(ev("PING :tmi.twitch.tv"), Some(Event::Pong(t)) if t == "tmi.twitch.tv"));
        assert!(matches!(ev("PING"), Some(Event::Pong(t)) if t == "tmi.twitch.tv"));
        assert!(matches!(ev(":a!a@a.tmi.twitch.tv JOIN #c"), Some(Event::Joined)));
        assert!(matches!(ev("@emote-only=0 :tmi.twitch.tv ROOMSTATE #c"), Some(Event::Joined)));
        assert!(matches!(ev("@target-msg-id=xyz :tmi.twitch.tv CLEARMSG #c :oops"), Some(Event::Deleted(id)) if id == "xyz"));
        assert!(ev(":tmi.twitch.tv CLEARMSG #c :oops").is_none());
        assert!(matches!(ev(":tmi.twitch.tv CLEARCHAT #c :Spammer"), Some(Event::Cleared(Some(l))) if l == "spammer"));
        assert!(matches!(ev(":tmi.twitch.tv CLEARCHAT #c"), Some(Event::Cleared(None))));
        assert!(matches!(ev(":tmi.twitch.tv RECONNECT"), Some(Event::Reconnect)));
        assert!(ev(":tmi.twitch.tv 372 x :motd").is_none());
    }

    #[test]
    fn handshake_lines() {
        assert_eq!(
            handshake(12345, "tenths"),
            ["CAP REQ :twitch.tv/tags twitch.tv/commands", "PASS SCHMOOPIIE", "NICK justinfan12345", "JOIN #tenths"]
        );
        for _ in 0..50 {
            assert!((10_000..90_000).contains(&nick_number()));
        }
    }

    #[test]
    fn backs_off_from_one_second_to_thirty() {
        let secs: Vec<u64> = (0..8).map(|r| backoff(r).as_secs()).collect();
        assert_eq!(secs, [1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(backoff(u32::MAX).as_secs(), 30);
    }

    #[test]
    fn moderation_removes_messages() {
        let at = Instant::now();
        let mut log = ChatLog::default();
        let mk = |id: &str, login: &str| {
            let mut m = demo_message(0, at);
            m.id = id.into();
            m.login = login.into();
            m
        };
        for (id, who) in [("1", "a"), ("2", "b"), ("3", "a")] {
            log.apply(Event::Message(mk(id, who)));
        }
        assert_eq!(log.status, ChatStatus::Connecting);
        log.apply(Event::Joined);
        assert_eq!(log.status, ChatStatus::Connected);
        log.apply(Event::Deleted("2".into()));
        assert_eq!(log.messages.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["1", "3"]);
        log.apply(Event::Cleared(Some("a".into())));
        assert!(log.messages.is_empty());
        log.apply(Event::Message(mk("4", "c")));
        log.apply(Event::Cleared(None));
        assert!(log.messages.is_empty());
    }

    #[test]
    fn keeps_the_last_hundred() {
        let at = Instant::now();
        let mut log = ChatLog::default();
        for k in 0..250 {
            log.add(demo_message(k, at));
        }
        assert_eq!(log.messages.len(), KEEP);
        assert_eq!(log.messages.back().unwrap().id, "demo-249");
        assert_eq!(log.messages.front().unwrap().id, "demo-150");
    }

    #[test]
    fn demo_emote_ranges_match_their_text() {
        let m = demo_message(4, Instant::now());
        let e = &m.emotes[0];
        let code: String = m.text.chars().skip(e.start).take(e.end - e.start + 1).collect();
        assert_eq!(code, "Kappa");
        assert!(demo_message(7, Instant::now()).action);
    }
}
