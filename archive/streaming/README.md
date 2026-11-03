# Twitch chat and the OBS stream window (archived)

Taken out of the app for the first release (see Future work in the main
README). The files here are as they were, at the same paths under `app/src/`:

| File | What it is |
| --- | --- |
| `app/src/widgets/twitch.rs` | The Twitch chat widget |
| `app/src/logic/twitch.rs` | Anonymous read-only Twitch IRC over a secure WebSocket, the message parser, and the demo chat (`TENTHS_CHAT_DEMO`) |
| `app/src/logic/emotes.rs` | Downloading and caching emote pictures |
| `app/src/stream.rs` | The stream window for OBS ("Tenths stream") |
| `app/src/settings/streaming.rs` | Its Settings page |

## Wiring them back in

Move the files back, then restore what was taken out of the rest of the app
alongside them. Every removed piece is below, by file. Also:

- `app/Cargo.toml`: the dependency the chat used:
  `tungstenite = { version = "0.24", default-features = false, features = ["handshake", "rustls-tls-webpki-roots"] }`
- `tools/screenshots.py`: `"twitch": (2132, 740)` in `SHOWCASE`,
  `"twitch": {"channel": "tenths_racing", "fadeAfter": 0, "rows": 8}` in `OPTIONS`,
  and `TENTHS_CHAT_DEMO="1"` in the app's environment.
- `app/src/settings/mod.rs`: the nav list's length goes back to 7.
- The README's "Twitch chat (experimental)" widget section and "Streaming"
  section, and the mentions of them, are in git history.

## Removed from the rest of the app

### `app/src/main.rs`

```rust
mod stream;
```

### `app/src/main.rs`

```rust
    /// The OBS stream window (stream.rs).
    stream: stream::Stream,
```

### `app/src/main.rs`

```rust
            stream: Default::default(),
```

### `app/src/main.rs`

```rust
        // The OBS stream window, when it's on: shown whenever the overlay would be, whatever the hide hotkey says.
        let stream_show = self.editing || (live.connected() && (general.show_when == config::ShowWhen::Connected || on_track));
        self.stream_window(ctx, &th, &live, general.units, stream_show, now);
```

### `app/src/main.rs`

```rust
            ToggleStream => {
                let on = !self.store.config.general.streaming.window;
                self.store.set_stream_window(on);
            }
```

### `app/src/frame.rs`

```rust
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
```

### `app/src/frame.rs`

```rust
    /// The stream window: draws each wanted widget where the layout puts it on its
    /// display, scaled by `k`. Same drawing, theme, options and opacity as the
    /// overlay. Doesn't feed the widgets (`observe` already ran this frame) or
    /// touch where the overlay thinks they are.
    pub fn draw_stream(&mut self, f: StreamFrame) {
        let StreamFrame { ctx, store, th, live, ratings, units, now, want, k } = f;
        for &(idx, screen) in want {
            let slot = &mut self.slots[idx];
            let cfg = store.config.widget(slot.meta.id).clone();
            let paint = Paint { ctx, th, store, live, ratings, units, editing: false, windowed: false, now, hover: None };
            let (size, _clicks, layer) = paint_slot(slot, &paint, &cfg, None);
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            let pos = anchor::resolve(&cfg, size * cfg.scale, screen, size);
            ctx.transform_layer_shapes(layer, TSTransform::new(pos.to_vec2() * k, cfg.scale * k));
        }
    }
```

### `app/src/config.rs`

```rust
    /// `general.streaming`: the OBS stream window (stream.rs).
    pub streaming: Streaming,
```

### `app/src/config.rs`

```rust
        streaming: normalize_streaming(g.get("streaming").unwrap_or(&Value::Null)),
```

### `app/src/config.rs`

```rust
/// The stream window's background: a colour for OBS's Chroma Key filter, or a dark one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamBackground {
    #[default]
    Green,
    Magenta,
    Black,
    Grey,
}

impl StreamBackground {
    pub const ALL: [StreamBackground; 4] = [Self::Green, Self::Magenta, Self::Black, Self::Grey];
    pub fn key(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Magenta => "magenta",
            Self::Black => "black",
            Self::Grey => "grey",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|b| b.key() == s)
    }
}

/// `general.streaming`. `enabled` and `port` are the Tauri app's (its OBS web
/// pages), kept as read; the rest is the stream window's.
#[derive(Debug, Clone, PartialEq)]
pub struct Streaming {
    pub enabled: bool,
    pub port: u16,
    /// The stream window is open.
    pub window: bool,
    pub background: StreamBackground,
    /// The display (`displays::Display::key`) whose layout and size it follows; None = the main one.
    pub display: Option<String>,
    /// Only these widgets (ids); empty = every widget on that display.
    pub widgets: Vec<String>,
    /// The window's size as a share of that display: 0.5, 0.75 or 1.
    pub scale: f32,
}

impl Default for Streaming {
    fn default() -> Self {
        Self { enabled: false, port: 7457, window: false, background: StreamBackground::default(), display: None, widgets: Vec::new(), scale: 0.5 }
    }
}
```

### `app/src/config.rs`

```rust
fn normalize_streaming(v: &Value) -> Streaming {
    let d = Streaming::default();
    let Some(o) = v.as_object() else { return d };
    Streaming {
        enabled: o.get("enabled").and_then(Value::as_bool).unwrap_or(d.enabled),
        port: o.get("port").and_then(num).filter(|p| (1024.0..=65535.0).contains(p)).map(|p| p as u16).unwrap_or(d.port),
        window: o.get("window").and_then(Value::as_bool).unwrap_or(d.window),
        background: o.get("background").and_then(Value::as_str).and_then(StreamBackground::parse).unwrap_or(d.background),
        display: o.get("display").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from),
        widgets: o
            .get("widgets")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).filter(|id| schema::meta(id).is_some()).map(String::from).collect())
            .unwrap_or_default(),
        scale: o.get("scale").and_then(num).map(|s| s as f32).filter(|s| (0.25..=1.0).contains(s)).unwrap_or(d.scale),
    }
}
```

### `app/src/config.rs`

```rust
    /// Turns the stream window on or off (the tray's item, the window's close button), in memory and in the file.
    pub fn set_stream_window(&mut self, on: bool) {
        self.config.general.streaming.window = on;
        let Some(path) = self.path.clone() else { return };
        let result = write_root(&path, |root| {
            let g = root.entry("general").or_insert_with(|| Value::Object(Map::new()));
            if !g.is_object() {
                *g = Value::Object(Map::new());
            }
            let st = g.as_object_mut().unwrap().entry("streaming").or_insert_with(|| Value::Object(Map::new()));
            if !st.is_object() {
                *st = Value::Object(Map::new());
            }
            st.as_object_mut().unwrap().insert("window".into(), Value::Bool(on));
        });
        self.finish_write(&path, result);
    }
```

### `app/src/shell/mod.rs`

```rust
    /// The tray's "Stream window": the OBS window on or off.
    ToggleStream,
```

### `app/src/shell/mod.rs`

```rust
    /// Ticks the tray's "Stream window" item to match the setting. No tray on this platform: no-op.
    pub fn set_tray_stream(&mut self, on: bool) {
        #[cfg(windows)]
        if let Some(t) = &self.tray {
            t.set_stream(on);
        }
        #[cfg(not(windows))]
        let _ = on;
    }
```

### `app/src/shell/tray.rs`

```rust
//! settings, Edit layout, Show / hide overlays, Next layout, Stream window, Check for
//! updates, Quit.
```

### `app/src/shell/tray.rs`

```rust
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
```

### `app/src/shell/tray.rs`

```rust
    /// "Stream window", ticked while the OBS window is open.
    stream: CheckMenuItem,
```

### `app/src/shell/tray.rs`

```rust
        let stream = CheckMenuItem::new("Stream window", true, false, None);
```

### `app/src/shell/tray.rs`

```rust
&[&open, &edit, &toggle, &next, &stream, &PredefinedMenuItem::separator(), &update, &quit]
```

### `app/src/shell/tray.rs`

```rust
            stream,
            update: update.id().clone(),
```

### `app/src/shell/tray.rs`

```rust
            } else if id == self.stream.id() {
                out.push(Request::ToggleStream);
```

### `app/src/shell/tray.rs`

```rust
    /// Ticks or clears "Stream window" to match the setting (the click toggled it already; the setting has the last word).
    pub fn set_stream(&self, on: bool) {
        self.stream.set_checked(on);
    }
```

### `app/src/settings/mod.rs`

```rust
//! Layouts, Appearance, Controls, Streaming, About, and one per widget.
```

### `app/src/settings/mod.rs`

```rust
mod streaming;
```

### `app/src/settings/mod.rs`

```rust
    Streaming,
```

### `app/src/settings/mod.rs`

```rust
            "streaming" => Pane::Streaming,
```

### `app/src/settings/mod.rs`

```rust
            Pane::Streaming => streaming::show(ui, th, env, &mut self.editor),
```

### `app/src/settings/mod.rs`

```rust
                        ("Streaming", Pane::Streaming, false),
```

### `app/src/settings/about.rs`

```rust
//! About (version and updates). (The Streaming page is streaming.rs.)
```

### `app/src/logic/mod.rs`

```rust
pub mod emotes;
```

### `app/src/logic/mod.rs`

```rust
pub mod twitch;
```

### `app/src/widgets/mod.rs`

```rust
pub mod twitch;
```

### `app/src/widgets/mod.rs`

```rust
        "twitch" => Box::<twitch::Twitch>::default(),
```

### `app/src/panels.rs`

```rust
        "twitch" => 500,
```

### `app/src/schema.rs`

```rust
    Meta {
        id: "twitch",
        name: "Twitch chat",
        x: 620.0,
        y: 300.0,
        enabled: false,
        options: &[("channel", S("")), ("rows", N(10.0)), ("fadeAfter", N(60.0)), ("showEmotes", B(true))],
    },
```

### `app/src/settings/specs.rs`

```rust
    WidgetSpec {
        id: "twitch",
        description: "A Twitch channel's live chat, read-only, so you can see it without leaving the car. Newest at the bottom.",
        experimental: true,
        options: &[
            OptionSpec { key: "channel", label: "Channel", help: Some("Your channel name, or its twitch.tv link. No login needed: chat is read the way anyone watching can"), kind: Kind::Text { max_length: Some(60) } },
            OptionSpec { key: "rows", label: "Messages", help: None, kind: Kind::Number { min: 3.0, max: 30.0, step: 1.0, suffix: None } },
            OptionSpec { key: "fadeAfter", label: "Hide messages after", help: Some("So it stays quiet when chat is. 0 keeps them"), kind: Kind::Number { min: 0.0, max: 600.0, step: 1.0, suffix: Some("s") } },
            OptionSpec { key: "showEmotes", label: "Show emotes", help: None, kind: Kind::Toggle },
        ],
    },
```

### `app/src/settings/specs.rs`

```rust
    ("twitch", "channel", "\"\""),
    ("twitch", "rows", "10"),
    ("twitch", "fadeAfter", "60"),
    ("twitch", "showEmotes", "true"),
```
