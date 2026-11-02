# app/

The Tenths app, drawn natively with [egui](https://github.com/emilk/egui) on
the telemetry core in `crates/telemetry`. How to run, build and use it is in
the [main README](../README.md); this file covers how it's put together.

## How it works

- **One process.** The telemetry thread is the core's own `runner::spawn`,
  with a `Sink` that keeps the latest payloads in memory (`src/data.rs`).
  Nothing is serialised; each telemetry frame asks for a redraw (at most
  `--fps` a second), and egui merges those requests.
- **A window per widget** (`src/panels.rs`) outside edit mode: each widget is
  its own small, transparent, always-on-top, click-through window, placed from
  its anchored position and the display's DPI. Widgets that draw nothing (the
  radar with no car near, no flag out) hide their window and keep it. The main
  window shrinks to 1×1 and keeps the app running.
- **Edit mode** switches to one full-screen overlay per display
  (`src/frame.rs`): drag, snap, resize, move between displays, the edit bar.
  Positions are written back to `config.json` in the same anchored form the
  Tauri app used.
- **Widgets** are plain painting code (`src/widgets/*.rs`), using the shared
  kit (`src/kit.rs`) and theme tokens (`src/theme.rs`). Each reads its options
  from `config.json`, with defaults from `src/schema.rs`.
- **Logic from the web front end** (race history, delta history, battle,
  iRating, incidents, rejoin, weather, pit fuel plan, coach texts, Twitch
  parser, race report maths) lives in `src/logic/`, with the old
  `scripts/check-*.ts` cases as unit tests.
- **The rest of the app**: hotkeys, wheel buttons and pit commands
  (`src/controls/`), saved layouts (`src/layouts.rs`), the settings window
  (`src/settings/`), and the tray and one-copy rule (`src/shell/`).
- Fonts are bundled (`assets/fonts`, SIL Open Font Licence).

`TRACKER.md` (the 19 widgets) and `TRACKER-APP.md` (everything around them)
list each feature of the Tauri version and its state here.
