# Tenths (in active development)

iRacing overlays you own. Tenths is a Tauri 2 app for Windows that reads the
iRacing SDK directly and draws a set of configurable widgets over the sim: no
subscriptions, no third-party services, and every line of it is yours to change.

![Tenths overlay in the demo race](docs/screenshot.png)

<details>
<summary>The same layout in the light theme</summary>

![Tenths overlay in the light theme](docs/screenshot-light.png)

</details>

## Features

- **14 widgets**: timing tables, a lap delta graph, a lap log, live coaching,
  inputs, car dynamics, dash lights, fuel, tires, weather, a radar and an LED
  flag board. Each one has its own options.
- **Edit layout mode**: drag any widget anywhere, resize it, snap it to the
  screen edges and other widgets, and turn widgets on or off from the edit bar.
- **Saved layouts**: keep as many layouts as you like (sprint, endurance,
  streaming…), switch with `Ctrl+Shift+L` or `Ctrl+Shift+1`–`9`, and export
  them as files to back up or share.
- **Any screen size**: widgets are pinned to the nearest screen edge or the
  centre, so a layout made at 1080p lands in the same places at 1440p or 4K.
- **Dark and light themes**, and a color picker for every color the overlays
  use, kept separately for each theme.
- **Reference laps and coaching**: import iRacing's `.blap`/`.olap` best laps or
  any lap from an `.ibt` telemetry file, then compare against it live with the
  delta graph, ghost inputs and live coaching cues.
- **Lap history**: every lap is logged with whether it was clean, and your
  personal best for each car and track is saved between sessions.
- **Strength of field** worked out with iRacing's own formula, per class.
- **Click-through**: the overlay never takes the mouse from iRacing, except
  while you're editing the layout.
- **A demo race** with 21 cars in three classes, so you can set everything up
  without iRacing running.

## Widgets

Every widget can be shown or hidden, resized from 50% to 300%, faded, and moved
anywhere. Options are under **Settings → (widget name)**.

### Relative

![Relative widget](docs/widgets/relative.png)

The cars just ahead of and behind you on track, with the time gap to each. Cars
a lap up on you are orange, cars you're lapping are blue. Each row has the
class color, car number, license and safety rating, iRating and last lap. A
last lap is purple when it's the fastest in class and green when it's that
driver's personal best. The last column is each car's gap to the car directly
in front of it on track (yours included), amber when it's under a second, so
you can see the fights going on around you. A car in the pit lane has an
outlined **P** next to its name, and a solid **P** once it's stopped in its
pit box. Cars that have left the sim aren't on track, so they drop out of the
relative. The header shows the lap count and your class's strength of field.

*Options*: how many cars ahead and behind, and whether to show iRating,
license, last lap, the gap to the car ahead, cars in the pits and SoF.

### Standings

![Standings widget](docs/widgets/standings.png)

The race order grouped by class, with each class's car count and strength of
field. Your class shows more rows, and you're always kept in view. Gaps are to
the class leader or to the car ahead, with iRating, last and best laps.

In races, the **+/−** column shows how many places each car has gained (green
▲) or lost (red ▼) in its class since the start. The grid is the order on
track before the green flag; if you join a race already under way, the
qualifying order stands in.

Next to the name, an outlined **P** means the car is in the pit lane and a
solid **P** that it's stopped in its pit box. A car that has left the sim gets
**OUT** in a race (retired, quit or towed to the garage) and **GAR** in
practice and qualifying, and its row fades.

*Options*: rows for your class and for others, your class only, gap to leader
or interval, and which columns to show (including places gained or lost).

### Session

![Session widget](docs/widgets/session.png)

Everything about the event in one panel: track and length, field size with
cars and SoF per class, time of day, air and track temperature, sky, track
wetness, wind and humidity. At the bottom are your incidents against the limit
(amber from 75% of it) and how many fast repairs are left. Off by default.

*Options*: list each class, show conditions, show incidents and fast repairs.

### Delta

![Delta widget](docs/widgets/delta.png)

Your lap against a comparison lap, all the way round. The line rises where the
comparison lap is faster and dips where you are, so red above the line is time
lost and green below it is time gained. Last lap's line is dashed behind it.
Below that, each sector's gain or loss, and on the right the sector you're in.
Your best, optimal and last laps run along the bottom, with the comparison lap
highlighted.

Compare against your best lap, optimal lap (best sectors), session best,
session optimal, last lap, or an imported reference lap.

*Options*: comparison lap, iRacing's sectors or 4–30 equal mini sectors, last
lap's line, the lap times row, and a **simple pace readout** that swaps the
graph for one big number: how far ahead or behind you are right now.

### Lap log

![Lap log widget](docs/widgets/laplog.png)

Every lap this session, newest first, with its gap to your best and the track
temperature when you set it. Your session best is green. A red dot marks a lap
that wasn't clean (off track or an incident), "Pit" marks in and out laps, and
"PB" marks a lap that beat your all-time personal best. The header shows the
session best and your personal best for this car and track, with when you set
it. It shows 8 laps; older ones scroll below, with a fade at the bottom when
there are more. While you drive the overlay lets the mouse through to the
game, so scroll the list in edit layout mode.

*Options*: how many laps to show before the list scrolls, gap to session best
or to personal best, and the gap, temperature and fuel-used columns.

### Coach

![Coach widget](docs/widgets/coach.png)

One live cue at a time from an imported reference lap: brake or lift in 50 m,
brake now, you're braking early, brake harder or lighter (measured from your
own brake point, so braking late isn't mistaken for braking lightly), trail
the brake, carry more speed, throttle now, gear at the apex, turn in more, and
whether you're gaining or losing. As you leave each corner it gives a verdict:
the time you gained or lost there and the main reason, like "Braked 20 m
early" or "Throttle earlier". Below it, your gap to the reference and your
speed difference, and the focus corner: the one costing you most on average,
and why. Corners where you're consistently on pace stop getting minor cues,
and coaching pauses under yellow and off track. Only shown while a reference
lap for this track is loaded.

*Options*: show the reason line, the gap and speed, and the focus corner.

### Inputs

![Inputs widget](docs/widgets/inputs.png)

Gear, speed, a steering wheel that turns with yours, a scrolling throttle and
brake trace, and throttle, brake and clutch bars. The gear turns red at the
shift point. With a reference lap loaded, a slimmer strip above shows the
reference's gear, speed, pedals and (for `.ibt` laps) wheel at the same spot on
track.

![Steering wheel styles](docs/widgets/wheels.png)

The wheel comes in four styles: round (road cars), GT, formula and oval. The
GT and formula wheels have shift lights that fill green, amber and red as you
near the shift point. *Automatic* picks one from the car's name (formula and
prototype cars get the formula wheel, GT cars the GT wheel, stock cars and
short-track cars the oval wheel, anything else the round one).

*Options*: trace length, wheel style, and whether to show the trace, wheel,
clutch, shift light and reference strip.

### Car dynamics

![Car dynamics widget](docs/widgets/dynamics.png)

A g-meter with the grip you've been using drawn around it: each point of the
outline holds for 10 seconds, then shrinks back to the centre over the next
10, so it shows recent grip and fades away on the straights. Beside it are the
lateral, longitudinal and vertical g, top, side and rear views of the car
showing slip, pitch and roll, and a balance bar estimating understeer or
oversteer from how the car rotates for the steering you put in.

*Options*: g-meter range, how long the grip outline holds and how quickly it
shrinks back to the centre, which way the dot moves, slip/pitch/roll views and
how much to exaggerate them, and the balance bar.

### Instruments

![Instruments widget](docs/widgets/instruments.png)

Dash lights for every iRacing engine warning (oil and fuel pressure, oil and
water temperature, stalled, pit limiter, rev limiter, repairs) plus a low fuel
light, engine gauges, and a brake bias dial that flashes when you change it.

*Options*: engine gauges, brake bias, and a bulb test that lights every warning
while editing.

### Fuel

![Fuel widget](docs/widgets/fuel.png)

Fuel in the tank, use per lap (the average of your last 5 green laps), laps
the tank will last, laps to go, and how much to add at your stop to finish.

*Options*: a safety margin in laps.

### Weather

![Weather widget](docs/widgets/weather.png)

Track and air temperature, how wet the track is (iRacing's seven steps from
dry to extremely wet), rain, humidity and fog, with arrows showing which way
each has moved over the last three minutes. Below, a wind compass that turns
with your car: the car always points up and the arrow shows where the wind is
hitting you from, with the wind speed beside it. iRacing
doesn't publish a forecast or rain radar, so the trend arrows are the closest
thing to one. Wind is measured at the start/finish line. Off by default.

*Options*: show the compass, turn it with the car or keep north up, and the
trend arrows.

### Tires

![Tires widget](docs/widgets/tires.png)

Temperature across each tire (inside, middle, outside), tread left, cold
pressures, the compound and how many sets you have left.

*Options*: show cold pressure.

### Radar

![Radar widget](docs/widgets/radar.png)

A see-through top-down view of the cars around you. Cars turn amber when
they're close and red when they overlap, and the side arcs light up when
someone's alongside.

*Options*: range, always visible or only when a car is near, and class colors.

### Flags

![Flags widget](docs/widgets/flags.png)

A trackside LED flag board. Yellow (flashing when waved), safety car ("SC" in a
yellow border), blue (flashing), debris, white, chequered, red, black, meatball,
black-and-white warning and disqualified each have their own pattern. Green
shows for a few seconds after a start or restart. Under the lights, a bar in
the flag's color says what it means.

*Options*: the text for each flag (for example "Accident ahead" for yellow), up
to three letters for the safety car board, how long green shows, and whether
the empty board stays visible.

## Saved layouts

Open **Settings → Layouts** to save what's on screen as a layout, give it a
name, and switch between layouts. While a layout is active, anything you
change (moving, resizing, options, showing or hiding widgets) is saved into it.

- **Switch**: `Ctrl+Shift+L` for the next layout, `Ctrl+Shift+1` to `9` for a
  layout by its number in the list, the picker in the edit bar, or **Switch
  to** in Settings. The overlay shows the layout's name for a moment.
- **Rename, duplicate, delete** from the list.
- **Export** saves a layout as a `.json` file, and **Import** adds one, so
  layouts can be backed up or shared.

A layout holds every widget's position, size, opacity, on/off state and
options. The theme, colors and units stay the same across layouts.

## Appearance

**Settings → Appearance** switches between the dark and light themes and has a
color picker for each color: text, panels, borders, the timing colors
(fastest, gaining, losing, warnings, lapped cars) and the pedals. Each theme
keeps its own colors, and every color has a button to go back to its default.
Shades such as the reference lap's muted colors are mixed from these, so they
follow your changes. Class colors come from iRacing, and the flag board uses
real flag colors.

## Reference laps

The Delta, Coach and Inputs widgets can compare you with a lap you import
under **Settings → Delta → Reference lap and coaching**:

- **`.blap` / `.olap`**: iRacing's saved best and optimal laps, from
  `Documents\iRacing\lapfiles`. These don't record steering, so there's no
  reference wheel; line cues use the car's heading instead.
- **`.ibt`**: iRacing telemetry files (record with `Alt+L`). Pick any complete
  lap; these include steering.

A reference is only used when its track and layout match the session.

## Notes

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
- A lap counts as clean when you stay on track, off pit road and pick up no
  incidents. Only clean laps count for session and personal bests. Lap times
  are iRacing's own, or Tenths' timing if iRacing doesn't report one.
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
git clone https://github.com/scarnegie96/Tenths---iRacing-Overlay.git
cd Tenths---iRacing-Overlay
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # installer in target\release\bundle\nsis\
```

In iRacing, set the display mode to **borderless windowed**. No overlay can draw
over exclusive fullscreen.

Quit Tenths from its tray icon before running `npm run tauri build`. If a copy
started from `target\release\tenths.exe` is still open, Windows locks the file
and the build stops with "Access is denied". Running the installer instead
puts Tenths in its own folder, so it can stay open while you build.

### Making a release

1. Set the same version in `package.json`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json`, and commit it.
2. Build the installer with `npm run tauri build`.
3. Tag the commit (`git tag v0.2.0`) and push the tag
   (`git push origin v0.2.0`).
4. On GitHub, draft a release from the tag and attach
   `target\release\bundle\nsis\Tenths_<version>_x64-setup.exe`.

The installer is attached to the release, not committed to the repo. It isn't
code-signed, so Windows SmartScreen asks people to click **More info → Run
anyway** the first time.

## Using it

The overlay covers your primary monitor and lets every click pass through to
iRacing. A tray icon opens the settings window, switches layouts and quits the
app.

| Shortcut                 | Does |
| ------------------------ | ---- |
| `Ctrl + Shift + E`       | Edit layout. `Esc` finishes. |
| `Ctrl + Shift + H`       | Show or hide all overlays |
| `Ctrl + Shift + L`       | Next saved layout |
| `Ctrl + Shift + 1`…`9`   | Saved layout by its number |

### Editing the layout

- Drag a widget to move it. Drag its corner, or use − and +, to resize it.
- Widgets snap to the screen edges and to each other while you drag.
- **Hide** on a widget's label turns it off. **Widgets** in the edit bar lists
  every widget, to turn any of them back on.
- The picker in the edit bar switches saved layouts.
- Each widget is pinned to its nearest screen edge or the centre, so a layout
  made at 1080p lands in the same places at 1440p or 4K. Resizing grows a
  widget away from the edge it's pinned to.
- On a 1440p screen at 100% Windows scaling, set every widget to 125% to get
  about the size they'd have at 1080p (**General → Widgets** sets them all at
  once). If Windows scaling is already 125%, leave them at 100%.

Turn on **General → Use the demo race** to lay things out without iRacing
running. **General → Reset layout** puts everything back to the defaults.

### Where things are saved

Everything is in `%APPDATA%\dev.tenths.overlay\`:

| File or folder   | Holds |
| ---------------- | ----- |
| `config.json`    | Settings, theme, colors and the widgets on screen |
| `layouts\`       | Saved layouts, one `.json` file each |
| `laps\`          | Lap history and personal best, one file per car and track |
| `reference.json` | The imported reference lap |

## Working on it from a Mac or Linux

The front end runs in a normal browser with a fake 21-car, 3-class race, so you
can work on widgets anywhere:

```bash
npm install
npm run dev
```

Open `http://localhost:1420/overlay.html` and `http://localhost:1420/settings.html`
side by side. The two tabs stay in sync. On the overlay, `E` edits the layout,
`L` switches saved layouts and `F` steps through every flag. These URL
parameters help too:

| Parameter       | Does |
| --------------- | ---- |
| `?speed=6`      | Fast-forwards the fake race |
| `?warmup=150`   | Races 150 seconds before the first frame, so graphs and traces are full |
| `?reference`    | Starts with a reference lap loaded, for the Coach and reference inputs |
| `?flag=16384`   | Starts with a flag out (iRacing's flag bits; 16384 is a caution) |
| `?theme=light`  | Shows the light theme |
| `?shot=delta`   | Draws one widget on its own, for screenshots |

To regenerate the README screenshots, keep `npm run dev` running and run
`node scripts/screenshots.mjs`. It uses Edge or Chrome, headless.

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
  engine.rs           Relative gaps, standings, fuel maths, SoF, deltas, optimal lap
  grid.rs             Starting grid for places gained/lost, and each car's pit or garage status
  laps.rs             Lap log and personal bests, saved per car and track
  vars.rs             Variable header parsing shared by live data and .ibt files
  ibt.rs              .ibt telemetry file reader: finds complete laps
  blap.rs             iRacing .blap/.olap reader (best/optimal lap files)
  reference.rs        Imported comparison lap: time, speed, pedals, gear, steering, line
  coach.rs            Corners from the reference and the live coaching cues
  dynamics.rs         g-forces, slip, pitch, roll and the understeer/oversteer estimate
  instruments.rs      Engine warning lights, gauges, low fuel and brake bias
  demo.rs             Fake 21-car, 3-class race with a scripted battle for the radar
  runner.rs           Telemetry thread: 60 Hz tick, 10 Hz field
src-tauri/            Windows, tray, hotkeys, click-through, config file
  profiles.rs         Saved layouts: list, save, import and export
src/
  lib/                Types, config schema, themes, weather maths, bridge to Rust (or browser mock)
  overlay/            Transparent overlay: drag, snap, resize, screen anchoring
  widgets/            One file per widget
  settings/           Settings window
scripts/              README screenshot generator
docs/ARCHITECTURE.md  How data flows, the maths behind each widget, adding a widget
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how data flows, how each
number is worked out, and how to add a widget.
