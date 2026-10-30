# Tenths

iRacing overlays you own. Tenths is a Tauri 2 app for Windows that reads the
iRacing SDK directly and draws a set of configurable widgets over the sim: no
subscriptions, no third-party services, and every line of it is yours to change.

![Tenths overlay in the demo race](docs/screenshot.png)

## Widgets

| Widget | What it shows |
| --- | --- |
| **Relative** | Cars around you on track with gaps, class, iRating, license and last lap. Cars lapping you or being lapped are colored. Your class's SoF is in the header. |
| **Standings** | The field grouped by class with gaps, iRating, last and best laps. SoF per class, or for the whole field in single-class races. |
| **Session** | Track and length, field size and SoF per class, time of day, air and track temperature, sky, track wetness, wind and humidity, your incidents against the limit and fast repairs left (off by default). |
| **Delta** | A horizontal trace of your lap against a comparison lap, rising where you're faster and dipping where you're slower, with per-sector gains and losses. Your best, optimal and last laps sit underneath. A simple mode shrinks it to just the live gap. |
| **Coach** | One live cue at a time from an imported reference lap: brake in 50 m, brake now, brake harder or lighter, carry more speed, throttle now, shift down, turn in more. |
| **Inputs** | Gear, speed, wheel angle, throttle/brake/clutch % and a scrolling input trace. The gear turns red at the shift point. With a reference lap loaded, a slimmer strip above shows the reference's inputs. |
| **Car dynamics** | g-meter with your grip envelope, top/side/rear views of slip, pitch and roll, and a live understeer/oversteer bar. |
| **Instruments** | Warning lights for every iRacing engine warning plus low fuel, engine gauges, and a brake bias dial that flashes when you adjust it. |
| **Fuel** | Fuel per lap (average of the last 5 green laps), laps in the tank, laps to go and how much to add to finish. |
| **Radar** | Transparent proximity radar with your car, nearby cars and side warnings. |
| **Tires** | Temperatures, wear and pressures per corner, compound and sets left. |
| **Spotter** | Big car-left / car-right warnings (off by default). |

Last laps are purple for the fastest in class and green for a personal best.

### Delta and reference laps

The Delta widget compares against your best lap, optimal lap, session best,
session optimal, last lap, or a lap you import. Import one under
**Settings → Delta → Reference lap and coaching**:

- **`.blap` / `.olap`**: iRacing's saved best and optimal laps, from
  `Documents\iRacing\lapfiles`. These don't record steering, so there's no
  reference wheel or steering cues.
- **`.ibt`**: iRacing telemetry files. Pick any complete lap; these include
  steering.

A reference is only used when its track and layout match the session.

Along the bottom, the panel shows your best, optimal and last laps, with the
one you're comparing against highlighted. Turn off **Delta → Show lap times**
to hide them. Turn on **Delta → Simple pace readout** to swap the graph and
sectors for one large number: how far ahead or behind the comparison lap you
are right now.

### Notes

- iRacing only updates tire temperatures and wear while your car is in its pit
  box. On track, the Tires widget shows the reading from your last stop.
- Low fuel is worked out by Tenths: under 2 laps of fuel, or under 8% of the
  tank before there's a per-lap figure.
- Gauges a car doesn't model read "–", and so does brake bias on cars without
  an adjustable bias.
- SoF uses iRacing's formula over every entry in the class, including cars
  that aren't on track, so Relative, Standings and Session always agree.
- The shift point is where the car's own shift lights start blinking in the
  current gear (or are all lit, for cars without a blink point). Turn off
  **Inputs → Gear turns red to shift** to keep the gear yellow.
- iRacing doesn't publish an optimal lap time, so the Delta panel's Optimal
  adds up your best clean sectors this session. It can differ from the
  all-time optimal in iRacing's own lap files.
- In team events, the Session panel counts the team's incidents, since that's
  what the limit applies to.

## Install and run (Windows)

One-time setup:

1. Install the [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   with the "Desktop development with C++" workload.
2. Install Rust from [rustup.rs](https://rustup.rs) (the default MSVC toolchain).
3. Install [Node.js](https://nodejs.org) 20 or newer, and [Git](https://git-scm.com).
4. WebView2 is already on Windows 10/11. If it's missing, install the Evergreen runtime.

Then:

```powershell
git clone https://github.com/YOUR-USERNAME/tenths.git
cd tenths
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # installer in target\release\bundle\nsis\
```

In iRacing, set the display mode to **borderless windowed**. No overlay can draw
over exclusive fullscreen.

## Using it

The overlay covers your primary monitor and lets every click pass through to
iRacing. A tray icon opens the settings window and quits the app.

| Shortcut           | Does |
| ------------------ | ---- |
| `Ctrl + Shift + E` | Edit layout: drag to move, drag the corner or use −/+ to resize, Hide to turn a widget off. `Esc` finishes. |
| `Ctrl + Shift + H` | Show or hide all overlays |

### Layout and screen sizes

- Every widget can be shown or hidden, resized (50–300%) and faded in
  **Settings**, on its own page or all at once under **General → Widgets**.
- Widgets snap to screen edges and to each other while you drag.
- Each widget is pinned to its nearest screen edge or the centre, so a layout
  made at 1080p lands in the same places at 1440p or 4K. Resizing grows a
  widget away from the edge it's pinned to.
- On a 1440p screen at 100% Windows scaling, set every widget to 125% to get
  about the size they'd have at 1080p. If Windows scaling is already 125%,
  leave them at 100%.

Turn on **General → Use the demo race** to lay things out without iRacing
running. **General → Reset layout** puts everything back to the defaults.

Settings are saved to `%APPDATA%\dev.tenths.overlay\config.json`, and the
imported reference lap to `reference.json` in the same folder.

## Working on it from a Mac or Linux

The front end runs in a normal browser with a fake 22-car, 3-class race, so you
can work on widgets anywhere:

```bash
npm install
npm run dev
```

Open `http://localhost:1420/overlay.html` and `http://localhost:1420/settings.html`
side by side. The two tabs stay in sync. Add `?speed=6` to the overlay URL to
fast-forward the fake race, and press `E` in the overlay to edit the layout.

The Tauri app builds only on Windows, but the core crate builds anywhere:

```bash
cargo test -p telemetry                                    # engine and parser tests
cargo check -p telemetry --features typecheck-windows      # type-check the Windows SDK reader
cargo run -p telemetry --example dump                      # print demo-race output as JSON
cargo run -p telemetry --example radar                     # what the radar sees, once a second
cargo run -p telemetry --example make_ibt -- demo.ibt      # write a demo .ibt to try importing
cargo run -p telemetry --example inspect_lap -- lap.blap   # list the corners the coach finds in a lap
cargo run -p telemetry --example coach_demo                # how often each cue fires in the demo
```

Before committing, run `npm run build` (type-checks the front end) and
`cargo clippy --workspace --all-targets`.

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
  dynamics.rs         g-forces, slip, pitch, roll and the understeer/oversteer estimate
  instruments.rs      Engine warning lights, gauges, low fuel and brake bias
  demo.rs             Fake 22-car, 3-class race with a scripted battle for the radar
  runner.rs           Telemetry thread: 60 Hz tick, 10 Hz field
src-tauri/            Windows, tray, hotkeys, click-through, config file
src/
  lib/                Types, config schema, bridge to Rust (or browser mock)
  overlay/            Transparent overlay: drag, snap, resize, screen anchoring
  widgets/            One file per widget
  settings/           Settings window
docs/ARCHITECTURE.md  How data flows, the maths behind each widget, adding a widget
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how data flows, how each
number is worked out, and how to add a widget.
