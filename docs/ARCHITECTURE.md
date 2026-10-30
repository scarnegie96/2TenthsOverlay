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
| `tick`    | every frame, 60 Hz | Player car: inputs, gear, speed, lap times, delta, fuel level, spotter, nearby cars for the radar | overlay only |
| `field`   | 10 Hz         | Relative, standings, fuel strategy, tires, session clock | all windows    |
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
- **Sectors**: track sectors come from `SplitTimeInfo.Sectors` in the session
  YAML; the widget can use equal mini sectors instead. The front end
  (`src/lib/deltaHistory.ts`) records the delta curve per lap and the gain or loss
  per sector, for every comparison at once.
- **Fuel**: average of the last 5 green laps. A lap is skipped if you joined
  mid-lap, visited pit road or refuelled. Laps to go come from
  `SessionLapsRemainEx` in lap races, or time remaining divided by average lap
  time (plus the lap you finish on) in timed races.

## The app shell (`src-tauri`)

- **overlay** window: transparent, borderless, always on top, sized to the
  primary monitor, ignoring the mouse. Edit mode turns mouse capture on.
- **settings** window: normal window. Closing it hides it; the tray icon brings
  it back, and Quit lives in the tray menu.
- Config is a JSON file owned by the front end (`src/lib/config.ts`). Rust only
  stores it and broadcasts `config-changed` so both windows stay in sync.

## Front end (`src`)

- `lib/bridge.ts` calls Tauri when present and `lib/mock.ts` in a plain browser.
- `lib/store.ts` holds live data and config in one zustand store. Widgets select
  only what they need, so a 60 Hz tick doesn't re-render the standings.
- The input trace draws to a canvas from a store subscription, outside React.
- `overlay/WidgetFrame.tsx` positions each widget with `translate + scale` and
  handles drag, snapping to screen and widget edges, and corner resize.

## Adding a widget

1. Add the data you need to the Rust payloads (`model.rs`, filled in `engine.rs`)
   and mirror it in `src/lib/types.ts`.
2. Create `src/widgets/MyWidget.tsx`. It receives `{ config, editing }`.
3. Add metadata (name, default position, option schema) to `WIDGETS` in
   `src/lib/config.ts`. The settings page builds its controls from that schema.
4. Register the component in `src/widgets/registry.ts`, and add the id to the
   `WidgetId` union.

## Ideas for next

- Broadcast messages (`IRSDK_BROADCASTMSG`): pit service buttons, replay control.
- Track map from `CarIdxLapDistPct` plus a recorded track outline.
- Show each car's tire compound in the relative (`CarIdxTireCompound`), e.g. wets.
- Profiles per car or series, and multi-monitor placement.
