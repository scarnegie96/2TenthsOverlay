# Tenths

iRacing overlays you own. A Tauri 2 app for Windows that reads the iRacing SDK
directly and draws a set of configurable widgets over the sim.

Widgets so far: **Relative**, **Standings** (grouped by class), **Inputs**,
**Delta**, **Fuel**, **Radar**, **Tires** and **Spotter**, all draggable,
resizable and toggleable. Last laps are purple for the fastest in class and green
for a personal best.

The **Delta** widget is a horizontal strip comparing your lap with your best lap,
optimal lap, session best, session optimal, last lap, or a lap you import from an
iRacing `.ibt` telemetry file. The comparison rises above your lap's baseline where
it's faster and dips below where it's slower, with per-sector gains and losses
underneath. Import a lap under **Settings → Delta → Reference lap and coaching**:
iRacing's saved best/optimal laps (`.blap`/`.olap` from `Documents\iRacing\lapfiles`)
or a lap from an `.ibt`.

With a reference loaded you also get **coaching**: the Inputs panel shows the
reference's wheel, pedals, gear and speed as a purple ghost, and the **Coach**
widget gives one live cue at a time (brake in 50 m, brake now, braking early,
brake harder or lighter, carry more speed, throttle now, shift down to 4th,
better exit, turn in more, gaining or losing time).

A note on tires: iRacing only updates tire temperatures and wear while your car
is in its pit box. On track the Tires widget shows the reading from your last stop.

## Running it on Windows

One-time setup:

1. Install the [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   with the "Desktop development with C++" workload.
2. Install Rust from [rustup.rs](https://rustup.rs) (the default MSVC toolchain).
3. Install Node.js 20 or newer.
4. WebView2 is already on Windows 10/11. If it's missing, install the Evergreen runtime.

Then, from this folder:

```powershell
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # installer in target\release\bundle\nsis\
```

In iRacing, set the display mode to **borderless windowed**. No overlay can draw
over exclusive fullscreen.

## Using it

The overlay covers your primary monitor and lets every click pass through to
iRacing. A tray icon opens the settings window.

| Shortcut           | Does                                     |
| ------------------ | ---------------------------------------- |
| `Ctrl + Shift + E` | Edit layout: drag to move, corner to resize. `Esc` finishes. |
| `Ctrl + Shift + H` | Show or hide all overlays                |

Turn on **General → Use the demo race** to lay things out without iRacing running.
Settings are saved to `%APPDATA%\dev.tenths.overlay\config.json`.

## Working on the UI from a Mac

The front end runs in a normal browser with a fake race, so you can work on
widgets anywhere:

```bash
npm install
npm run dev
```

Open `http://localhost:1420/overlay.html` and `http://localhost:1420/settings.html`
side by side. Add `?speed=6` to the overlay URL to fast-forward the fake race. The two tabs stay in sync. Press `E` in the overlay to edit the layout.

The Rust side builds only on Windows, apart from the core crate:

```bash
cargo test -p telemetry                                    # engine + parser tests, any OS
cargo check -p telemetry --features typecheck-windows      # type-check the Windows SDK reader
cargo run -p telemetry --example dump                      # print demo-race output as JSON
cargo run -p telemetry --example radar                     # what the radar sees, once a second
cargo run -p telemetry --example make_ibt -- demo.ibt      # write a demo .ibt to try importing
cargo run -p telemetry --example inspect_lap -- lap.blap   # list the corners the coach finds in a lap
cargo run -p telemetry --example coach_demo                # how often each cue fires in the demo
```

## Project layout

```
crates/telemetry/     Pure Rust, no Tauri. Everything about iRacing data.
  irsdk.rs            Shared-memory reader (our own, about 300 lines)
  session.rs          Session YAML parser, tolerant of iRacing's quirks
  engine.rs           Relative gaps, standings, fuel maths, SoF, deltas
  vars.rs             Variable header parsing shared by live data and .ibt files
  ibt.rs              .ibt telemetry file reader: finds complete laps
  blap.rs             iRacing .blap/.olap reader (best/optimal lap files)
  reference.rs        Imported comparison lap: time, speed, pedals, gear, steering, line
  coach.rs            Corners from the reference and the live coaching cues
  demo.rs             Fake 22-car, 3-class race with a scripted battle for the radar
  runner.rs           Telemetry thread: 60 Hz tick, 10 Hz field
src-tauri/            Windows, tray, hotkeys, click-through, config file
src/
  lib/                Types, config schema, bridge to Rust (or browser mock)
  overlay/            Transparent overlay, drag, snap and resize
  widgets/            One file per widget
  settings/           Settings window
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how data flows and how to
add a widget.
