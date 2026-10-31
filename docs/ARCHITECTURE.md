# Architecture

```
iRacing ──shared memory──► irsdk.rs ─┐
                                     ├─► RawFrame ─► Engine ─► payloads ─► TauriSink ─► events ─► React
demo.rs (fake race) ─────────────────┘   + YAML                                          (zustand store)
```

## The telemetry thread (`crates/telemetry`)

`runner.rs` owns one source at a time: iRacing, or the demo when the demo toggle
is on. It switches when the toggle changes, and reconnects every second while
iRacing isn't running.

Each source produces the same normalized `RawFrame` (the iRacing variables we
use, already typed) plus the session YAML whenever it changes. The engine never
knows which source it's reading from, so the demo exercises the real maths.

| Event     | Rate          | Contents                                          | Goes to        |
| --------- | ------------- | ------------------------------------------------- | -------------- |
| `tick`    | every frame, 60 Hz | Player car: inputs, gear, speed, lap times, delta, fuel level, side-by-side warnings and nearby cars for the radar | overlay only |
| `field`   | 10 Hz         | Relative, standings, fuel strategy, tires, pit service, session clock | all windows    |
| `session` | on change     | Track, session type, car, fuel tank size, shift RPM | all windows  |
| `status`  | on change     | Source and connection state                       | all windows    |

### The IRSDK reader

`irsdk.rs` maps `Local\IRSDKMemMapFileName` read-only and waits on
`Local\IRSDKDataValidEvent`. Each frame it:

1. Picks the telemetry buffer with the highest tick count (iRacing rotates 3–4).
2. Copies it out, then checks the tick didn't change mid-copy. If it did, it retries.
3. Reads variables by name through the variable headers, converting any declared
   type to the number we want. Nothing is hard-coded to an offset.

Session YAML is decoded from Latin-1. Free-text keys (driver and team names) are
quoted before parsing, because iRacing doesn't escape them.

### Pit commands

The only thing Tenths sends to iRacing. `irsdk::send_pit_command` does what the
SDK's `irsdk_broadcastMsg` does: it registers the `IRSDK_BROADCASTMSG` window
message and broadcasts it with `SendNotifyMessageW`, packing the message type
(9, pit command) and the command into wParam and the parameter (litres, kPa or a
compound number) into lParam. The numbering is in `pit.rs`, from
`irsdk_defines.h`. iRacing never replies, so the readback is the `PitSv*`
variables (`PitSvFlags`, `PitSvFuel`, `PitSvLFP`…, `PitSvTireCompound`) plus
`PlayerCarPitSvStatus` and `FastRepairAvailable`, sent as `field.pit`.

Commands go through `RunnerControl::pit_command` to the telemetry thread, which
sends them to iRacing or applies them to the demo race's own pit service
(`PitService::apply`, mirrored in the browser mock). The overlay decides what to
send (`src/lib/pit.ts`), because it has the fuel plan and the widget's options:
each hotkey toggles against what iRacing last reported. There's no command to
untick one tire, so the tire hotkey is all four or none. Rust only takes the
pit keys and buttons (`set_pit_active`) while the Pit stop widget is on and the player is
in the car. To be confirmed in the sim: the variable names above, and that a
fuel amount of 0 keeps the amount already set.

### Engine maths worth knowing

- **Relative gap**: `CarIdxEstTime` is the time to reach a car's spot on track
  at *its own class's* pace. Dividing by that class's estimated lap time gives a
  lap fraction we can compare across classes. Multiplied by the player's lap time
  and wrapped to ±half a lap, that's the gap.
- **Lapping colours**: total race progress minus on-track offset leaves whole
  laps. +1 means the car is lapping you, −1 means you're lapping it.
- **Standings gaps**: race uses `CarIdxF2Time` within each class. Practice and
  qualifying use best-lap differences.
- **Lap colors**: a last lap equal to the driver's best is a personal best
  (green). If that also equals the best in their class, it's the fastest (purple).
- **Radar**: distance along the track is `CarIdxLapDistPct` difference × track
  length. iRacing doesn't give lateral position, only `CarLeftRight`, so the
  nearest overlapping cars take the reported sides. Lanes are sticky: a car
  keeps its side while within 14 m, so cars don't jump lanes as they pull away.
- **Tires**: `LFtempCL/CM/CR`, `LFwearL/M/R` etc. are only refreshed by iRacing
  while `PlayerCarInPitStall` is true. The widget labels readings as live or
  from the last stop. Compound names come from `DriverInfo.DriverTires`.
- **Delta**: iRacing gives live deltas against best lap, optimal lap, session
  best, session optimal and last lap (`LapDeltaTo…`; note the last-lap variable is
  really spelled `LapDeltaToSessionLastlLap`). All are sent every tick, so the
  widget can switch instantly. For an imported lap we compute it ourselves: our
  lap time so far minus the reference's time at the same lap position.
- **Reference laps**: `ibt.rs` reads `.ibt` files (same layout as live memory plus
  a 32-byte disk header), splits the samples into laps that run line to line
  without resets, and times each crossing by interpolating between samples.
  `reference.rs` stores the chosen lap as elapsed time at every 0.1% of the lap,
  saved as `reference.json` in the app data folder. It's only used when the track
  and layout match the session.
- **.blap / .olap files**: iRacing's own best and optimal laps, undocumented but
  simple: a header (driver, car id, track id, lap time), a sector table, then one
  28-byte record every ~2 m with time into the sector, sideways position,
  heading, pitch, roll, and throttle/brake/clutch/gear bytes. The full layout is in
  `blap.rs`. Steering isn't stored, so .blap/.olap references have no reference
  wheel (.ibt files record `SteeringWheelAngle`, so those do). The heading
  gives the path's curvature, which the coach uses for line cues.
  Speed comes from the time between records.
- **Coaching** (`coach.rs`): the reference is split into corners. Braking
  corners run from the brake point through the slowest point to the throttle
  back over 50%; lift corners do the same off full throttle without braking.
  The scan starts mid-straight and all bin maths wraps, so corners across the
  start line work. Each corner is watched from 250 m before its entry to
  150 m after its pickup (windows never overlap).
  - *Live cues*: each frame we compare your inputs and offer cues with a
    priority; the highest wins. A cue stays up for 0.9 s (verdicts 2.5 s)
    unless something more urgent comes along. Initial brake pressure is
    compared with the reference's the same distance after *its* brake point
    as you are after yours, so braking late isn't read as braking lightly;
    trail and release are compared by position. Your brake point only counts
    once you've held the brake 0.1 s. Gear is only cued within 40 m of the
    apex. Line cues compare your curvature (yaw rate / speed) with the
    reference's, which every reference has; steering angle is the fallback.
    No cues under yellow, off track or in the pits.
  - *Corner passes*: each pass records your brake point, peak pressure,
    slowest speed and gear, throttle pickup, and the delta at the window
    start, the reference's apex and the window end (kept continuous across
    the line). Entry loss is up to the apex, exit loss after it; the bigger
    side picks the cause (`Issue`), shown as a verdict on the way out. Passes
    are dropped after a jump in position, a tow or a reset. Each corner keeps
    a weighted average loss: the worst is sent as `focus`, and corners
    averaging under 0.03 s after two passes only get urgent cues.
- **Inputs panel**: with a reference loaded, a slimmer strip extends from the
  top with the reference's gear, speed, wheel (.ibt only), trace and pedals. It
  uses the same columns as yours, so the two traces line up.
- **Car dynamics** (`dynamics.rs`): g from `LatAccel`/`LongAccel`/`VertAccel`,
  attitude from `Pitch`/`Roll`, slip angle from `atan2(VelocityY, VelocityX)`.
  Balance: in gentle corners (0.15-0.6 g, little slip) we fit yaw rate against
  speed x steering to learn how your car normally rotates. In hard corners,
  actual yaw rate below that expectation is understeer, above it oversteer; a
  big slip angle with opposite lock is always oversteer. The sign conventions of
  iRacing's `Pitch` and `Roll` should be checked in the sim (see the views).
- **Instruments** (`instruments.rs`): `EngineWarnings` bits are water temp 0x1,
  fuel pressure 0x2, oil pressure 0x4, stalled 0x8, pit limiter 0x10, rev limiter
  0x20, and in newer SDKs oil temp 0x40, repair required 0x80, optional repair
  0x100. Low fuel is ours: under 2 laps of fuel by the fuel tracker, or under 8%
  of the tank before there's a per-lap figure. Gauges reading 0 are treated as
  "not modelled for this car". Brake bias is `dcBrakeBias` (plus
  `dcBrakeBiasFine`/`dcPeakBrakeBias` on cars that have them), shown as % front.
- **Sectors**: track sectors come from `SplitTimeInfo.Sectors` in the session
  YAML; the widget can use equal mini sectors instead. The front end
  (`src/lib/deltaHistory.ts`) records the delta curve per lap and the gain or loss
  per sector, for every comparison at once.
- **Fuel**: average of the last 5 green laps. A lap is skipped if you joined
  mid-lap, visited pit road or refuelled. Laps to go come from
  `SessionLapsRemainEx` in lap races, or time remaining divided by average lap
  time (plus the lap you finish on) in timed races.
- **Places gained / lost** (`grid.rs`): iRacing publishes no starting grid,
  so in race sessions we keep each car's class position while
  `SessionState` is before racing (the grid), fill in cars first classified
  on the opening lap, and fix it once the leader completes lap 1. Joining a
  race already under way falls back to the qualifying order from
  `QualifyResultsInfo.Results`, ranked within each class (so it doesn't
  matter whether iRacing's positions start at 0 or 1). Sent as
  `startPosition`; the widget shows start minus current class position.
- **Pit and out symbols**: each Relative and Standings row carries a
  `status` from `CarIdxTrackSurface` and `CarIdxOnPitRoad`: not in world
  (-1) = out (garage, towed or left the session; drivers who quit keep their
  entry), in pit stall = pit box, on pit road = pit lane. The Relative
  already leaves out cars that aren't in the world.
- **Race pace** (`src/lib/raceHistory.ts`): built in the front end from the
  Standings rows (gap to class leader, which is the `CarIdxF2Time` difference
  in races). Fed from every 10 Hz field update outside React, like
  `deltaHistory.ts`, so it keeps recording while the widget is hidden. Each
  car stores (session time, gap) every 2 s, the gap averaged over those 2 s to
  smooth timing steps; past 2,400 points every car is thinned by half and
  the interval doubles, so a long race stays small. Lap lines are when each
  class leader's completed-lap count goes up. It resets on a new session or
  when session time goes backwards, and records nothing outside races. The
  chart scales to cars on the lead lap; lapped and retired cars are pinned to
  the bottom edge. The gap change per lap compares each car's gap now with
  its stored gap one leader-lap ago.
- **Weather**: `AirTemp`, `TrackTempCrew`, `TrackWetness` (1 dry .. 7 extremely
  wet), `Precipitation`, `RelativeHumidity` and `FogLevel` (each accepted as a
  fraction or a percentage), `Skies`, `WeatherDeclaredWet`, and `WindVel` /
  `WindDir` measured at the start/finish line. `WindDir` is read as the bearing
  the wind blows *from*, clockwise from north. The car's heading comes from
  `YawNorth`, which turns anticlockwise like the yaw rate, so the engine sends
  `heading = -YawNorth` wrapped to 0..2π. The widget's compass card is rotated
  by `-heading` so the car points up; the wind relative to the car is
  `WindDir - heading` (0 = head on, 90° = from the right), giving
  headwind = speed·cos and crosswind = speed·sin of it. Both sign conventions
  should be confirmed in the sim (drive north with a known wind). iRacing has no
  forecast or radar variables; trend arrows compare each reading with ~3
  minutes ago, sampled every 15 s of session time (`src/lib/weather.ts`).

## The app shell (`src-tauri`)

- **overlay** windows (`displays.rs`): one per display, each transparent,
  borderless, always on top, sized to its monitor and ignoring the mouse
  (edit mode turns mouse capture on for all of them). `overlay` from
  tauri.conf.json covers the primary display; others are built as
  `overlay-2`, `overlay-3`… (numbered main first, then left to right, top to
  bottom). One window per display rather than one across the desktop keeps
  each at its monitor's own scale factor, which a spanning window can't do
  on mixed-DPI setups, and avoids compositing a desktop-sized transparent
  surface. A thread polls the monitors every 2 s and adds, moves or closes
  windows to match, then emits `displays-changed`. The `tick` goes to every
  overlay (`emit_overlays`); layout and pit shortcuts still go to `overlay`
  only, and in the front end only the primary display's window sends
  controls and pit commands, so nothing happens once per screen. The
  capability file lists `overlay-*` so new windows get events and commands.
- **Displays in the front end** (`src/lib/displays.ts`): a display's key is
  Windows' device name (`\\.\DISPLAY2`), which survives rearranging.
  `get_displays` lists them and `my_display` tells a window which it covers.
  A widget's `display` picks its window; missing or unplugged means the
  primary, so nothing is lost. Its anchored position is within that display.
  Moving between displays: the edit label's button re-anchors it on the next
  display at the same offsets; dragging past the window edge works because
  the window keeps the pointer while the button is held, so on release
  `dropOnto` turns the pointer into desktop pixels (window origin + client
  position × scale), finds the display under it and converts back to that
  display's CSS pixels, keeping where the widget was held. In the browser
  preview, `?displays=2` adds a second display and `&display=2` makes a tab
  its overlay.
- **settings** window: normal window. Closing it hides it; the tray icon brings
  it back, and Quit lives in the tray menu.
- Config is a JSON file owned by the front end (`src/lib/config.ts`). Rust only
  stores it and broadcasts `config-changed` so both windows stay in sync.
- **Controls** (`controls.rs`): every action ("overlay.edit", "layout.3",
  "pit.fuel"...) has a key and a controller button in `general.controls`
  (`src/lib/controls.ts`). The overlay sends them with `set_controls` on load
  and whenever they change. Rust registers only the shortcuts that should be
  live: pit actions only while `set_pit_active` is on, and nothing while the
  settings window is learning a key (`suspend_controls`). The tray menu's
  shortcut text follows the bindings.
- **Controller buttons** (`controllers.rs`): a message-only window on its own
  thread registers for Raw Input from joysticks, gamepads and multi-axis
  controllers (HID usage page 1, usages 4, 5 and 8) with `RIDEV_INPUTSINK`, so
  input arrives while iRacing has the focus. Each `WM_INPUT` report is decoded
  with the device's preparsed HID data: `HidP_GetUsages` on the button page
  gives the buttons held, and the hat switch (usage 0x39) becomes buttons 1001
  to 1004. A press is a button held now that wasn't in the last report. Every
  press goes to all windows as `controller-button` (Settings learns buttons
  from it) and runs any action bound to it. Devices are keyed by the hardware
  id from their path, like `VID_0EB7&PID_0E04&MI_00`, and named with
  `HidD_GetProductString`. `cargo test -p tenths list_controllers -- --ignored
  --nocapture` prints what's plugged in.

## Front end (`src`)

- `lib/bridge.ts` calls Tauri when present and `lib/mock.ts` in a plain browser.
- `lib/store.ts` holds live data and config in one zustand store. Widgets select
  only what they need, so a 60 Hz tick doesn't re-render the standings.
- The input trace draws to a canvas from a store subscription, outside React.
- `overlay/WidgetFrame.tsx` positions each widget with `translate + scale` and
  handles drag, snapping to screen and widget edges, corner resize, and the
  edit-mode label (−/+ size, Hide).
- **Resolution independence** (`overlay/anchor.ts`): a widget's position is
  saved as an anchor (`ax`: left/centre/right, `ay`: top/middle/bottom, picked
  from the third of the screen its centre is in) plus an offset from that edge
  or from the screen centre. At any screen size the widget keeps that offset,
  and is pulled back on screen if it would hang off. Configs without an anchor
  (the defaults, and layouts from before anchoring) are absolute positions on
  a 1920x1080 screen at 100% size and get anchored from there, so 1080p looks
  exactly as designed. Dragging works in absolute pixels and re-anchors on
  release. Resizing grows a widget away from its pinned edge. The overlay
  window is the monitor's physical size, so CSS pixels follow Windows display
  scaling (2560x1440 at 125% is a 2048x1152 overlay).

## Adding a widget

1. Add the data you need to the Rust payloads (`model.rs`, filled in `engine.rs`)
   and mirror it in `src/lib/types.ts`.
2. Create `src/widgets/MyWidget.tsx`. It receives `{ config, editing }`.
3. Add metadata (name, default position, option schema) to `WIDGETS` in
   `src/lib/config.ts`. The settings page builds its controls from that schema.
4. Register the component in `src/widgets/registry.ts`, and add the id to the
   `WidgetId` union.

## Ideas for next

- More broadcast messages: tire compound in the Pit stop widget, chat macros,
  starting and stopping telemetry recording, replay control.
- Track map from `CarIdxLapDistPct` plus a recorded track outline.
- Coach cues for lateral line, using the sideways-position channel in .blap
  files, and audio for the brake countdown.
- Show each car's tire compound in the relative (`CarIdxTireCompound`), e.g. wets.
- Profiles per car or series.
