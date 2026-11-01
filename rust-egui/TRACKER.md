# egui port tracker

Every panel of the Tauri app, ported to the egui prototype (`rust-egui/`), with
each feature and option listed so parity can be ticked off. The web app's code
is the specification: read the named source before implementing, and match
what it shows (text, colours, rounding, empty states, edge cases).

Branch: **development only.** Nothing from this work goes to `main`.

Legend: ⬜ to do · 🟨 in progress · ✅ done (built, screenshot checked) · ⏭️ deferred (reason given)

## How each panel is done

1. Read the web widget (`src/widgets/<Name>.tsx`), its styles (search
   `src/styles/overlay.css` for its class prefix), any `src/lib/*.ts` it uses,
   and its options in `src/lib/config.ts` (`WIDGETS`).
2. Port front-end-only logic to Rust in `rust-egui/src/logic/` with unit tests
   (port the matching `scripts/check-*.ts` cases where they exist).
3. Draw it in `rust-egui/src/widgets/<name>.rs` with the shared helpers in
   `theme.rs` / `kit.rs`. Size from its content and options, never from the
   window.
4. Read every option from the widget's `options` in `config.json` (defaults
   from `config.ts` when missing).
5. Check: `cargo build --release`, `cargo test`, `cargo clippy` clean; render
   with `--demo --shot` under `xvfb-run` and look at it; the `--offline` (no
   session) render must not panic and must show the widget's empty state.
6. Tick the boxes here and commit (one commit per panel).

---

## 0. Foundations (before the panels)

| # | Item | Source | Status |
|---|---|---|---|
| F1 | Read the Tauri app's `config.json` (Windows `%APPDATA%\dev.tenths.overlay\config.json`; elsewhere the matching config dir; `--config <path>` override) as `serde_json::Value`, with every widget's `enabled`, position, `scale`, `opacity`, `options`, and `general` (units, theme, colors, showWhen) | `src/lib/config.ts` (`defaultConfig`, `normalizeConfig`), `src-tauri/src/lib.rs` (`config_path`) | ✅ |
| F2 | Option lookup with defaults from the `WIDGETS` schema (a Rust table of every widget's defaults) | `config.ts` `WIDGETS` | ✅ |
| F3 | Widget positions: anchors (`ax`/`ay`, offsets in logical px), unanchored = absolute on 1920×1080, clamped on screen; scale (zoom the panel) and opacity | `src/overlay/anchor.ts`, `WidgetFrame.tsx` | ✅ |
| F4 | Edit mode: drag to move, then write back `x`/`y`/`ax`/`ay` for that widget only (temp file + rename, unknown fields kept); snapping guides to other widgets and screen centre/edges | `WidgetFrame.tsx`, `snapping.ts` | ✅ |
| F5 | Watch `config.json` (poll mtime every second) and apply changes made by the Tauri app's settings | — | ✅ |
| F6 | Themes: Dark, Light, Box Box (`f1`) with every colour token, and per-theme custom colours from `general.colors`; fonts per theme (Box Box uses Titillium Web) | `src/lib/theme.ts`, `src/styles/base.css`, `overlay.css` theme blocks | ✅ |
| F7 | Units (metric/imperial) for speed, fuel, temperature, pressure | `src/lib/format.ts` | ✅ |
| F8 | `showWhen`: connected / on track; always in edit mode | `Overlay.tsx` | ✅ |
| F9 | `--offline` flag (no data at all) for empty-state checks | `src/lib/mock.ts` `?offline` | ✅ |
| F10 | Shared kit: panel chrome per theme, class stripe, car number, iRating badge (+ change pill), licence chip (letter + SR, licence colour), tyre badge (W/S/M/H from compound name), status badges (PIT/OUT), lap-time colouring (purple class fastest, green PB), rating thumb mark, sparkline, `Empty` text | `src/widgets/cells.tsx`, `Panel.tsx`, `Stints.tsx` `compoundKind` | ✅ |
| F11 | Driver ratings: read `driver-ratings.json` (app data dir) for the thumb marks | `src-tauri/src/ratings.rs` | ✅ |
| F12 | Per-widget panic safety: a panel that fails draws a small error note in edit mode, never takes the others down | `WidgetBoundary.tsx` | ✅ |

## 1. Relative — `src/widgets/Relative.tsx`

| Feature / option | Status |
|---|---|
| Rows from `field.relative` (gap positive ahead), fixed slot count `ahead` (3) / `behind` (3), player centred, empty slots keep height | ✅ |
| Title = session type; aside = session time/laps (`sessionAside`) and `SoF` (`showSoF`) | ✅ |
| Incidents in header (`showIncidents`), next limit via `incidentTarget` with `incidentStep` (4); amber / red tones | ✅ |
| Columns: position, class stripe, number, name, tyre (`showCompound` auto/always/never), licence (`showLicense`), iRating (`showIRating`), last lap (`showLastLap`, coloured), gap, interval to car ahead (`showInterval`, amber under 1 s) | ✅ |
| Lapping / lapped colours (lap_status), other-class dimming in multiclass, close fight highlight (`CLOSE_S`) | ✅ (the web Relative has no other-class dimming; colours and CLOSE_S interval match it) |
| Cars in pits: `showPit` filter, PIT/OUT badges, dimmed | ✅ |
| Rating marks (`showRatings`) | ✅ |
| Empty state: "Relative appears once you're in the car." | ✅ |

## 2. Standings — `src/widgets/Standings.tsx`, `src/lib/irating.ts`, `src/lib/rejoin.ts`

| Feature / option | Status |
|---|---|
| Classes in order; your class `rows` (10) around you, others `otherClassRows` (3); `myClassOnly` | ✅ |
| Class headers with cars and SoF (`showSoF`) | ✅ |
| Gap column `gapMode` leader / interval; `+NL` laps down; leader text | ✅ |
| Places gained/lost vs grid (`showChange`, arrows, races only) | ✅ |
| Rejoin line after a stop (`showRejoin`, from `field.pitLoss`, `rejoinAfterStop`) | ✅ |
| iRating badge (`showIRating`) with change pill in official races (`showIRatingChange`, `iratingChanges`, port with `check-irating.ts` tests) | ✅ |
| Last (`showLast`) and best (`showBest`) laps with colours | ✅ |
| Status badges, out cars dimmed | ✅ |
| Rate buttons after the flag (`rateDrivers`): clickable thumbs while click-through. ⏭️ allowed if click-through regions aren't built yet; rating marks (`showRatings`) required | ⏭️ click-through regions aren't built (deferred below); rating marks are shown |
| Empty state | ✅ |

## 3. Session — `src/widgets/Session.tsx`

| Feature / option | Status |
|---|---|
| Track and config, session type, time/laps | ✅ |
| Field, SoF; per-class list (`showClasses`) | ✅ |
| Conditions (`showConditions`): time of day, air/track temp, sky, wetness, wind | ✅ |
| Limits (`showLimits`): incidents vs limit, fast repairs | ✅ |
| Empty state | ✅ |

## 4. Inputs — `src/widgets/Inputs.tsx`

| Feature / option | Status |
|---|---|
| Gear (yellow; red at shift RPM when `shiftLight`), speed with units | ✅ |
| Steering wheel (`showSteering`), shapes `wheel` auto/round/gt/formula/oval (auto from car) | ✅ |
| Trace (`showTrace`) over `traceSeconds` (6): throttle, brake, clutch (`showClutch`) | ✅ |
| Pedal bars with values; clutch bar optional | ✅ |
| Reference ghost strip (`showGhost`, from `tick.coach` ghost inputs) | ✅ |
| Theme colours for pedals and wheel | ✅ |

## 5. Delta — `src/widgets/Delta.tsx`, `src/lib/deltaHistory.ts`

| Feature / option | Status |
|---|---|
| Comparison `compareTo` (best/optimal/sessionBest/sessionOptimal/last/reference) from `tick.deltas` | ✅ |
| Delta trace round the lap (`deltaHistory` port: 320 bins), previous lap dashed (`showPrevious`) | ✅ |
| Sectors `track` (session sectors) / `mini` (`miniSectors`), per-sector gain/loss boxes | ✅ |
| Big live delta, sector number, this-sector and last-sector readouts | ✅ |
| Lap times strip (`showLapTimes`): best, optimal, last | ✅ |
| `simple` mode: just the live gap | ✅ |
| Empty states ("No best lap to compare against yet.") | ✅ |

## 6. Lap log — `src/widgets/LapLog.tsx`

| Feature / option | Status |
|---|---|
| Laps from `field.lapLog` newest first, `rows` (8) | ✅ (newest `rows` shown with a fade when there are more; the web's edit-mode scroll isn't ported) |
| Header: session best, personal best (`showPersonalBest`, with age) | ✅ |
| Gap column (`showDelta`) to session or personal best (`compareTo`) | ✅ |
| Track temp (`showTemp`), fuel used (`showFuel`) | ✅ |
| Flags: PB badge, pit, off-track/incident marks | ✅ |
| Empty state | ✅ |

## 7. Coach — `src/widgets/Coach.tsx`, `src/lib/coach.ts`

| Feature / option | Status |
|---|---|
| Cue text and colour from `tick.coach` (`cueText`, `issueText` port) | ✅ |
| Reason line (`showDetail`), delta and speed (`showStats`), focus corner (`showFocus`) | ✅ |
| No-reference state ("Import a reference lap…") | ✅ |

## 8. Car dynamics — `src/widgets/Dynamics.tsx`

| Feature / option | Status |
|---|---|
| G-meter with rings to `maxG`, dot (`direction` felt/accel), readouts lateral/accel/vertical | ✅ |
| Grip envelope with hold (`envelopeHold`) and return (`envelopeReturn`) | ✅ |
| Slip, pitch, roll car views (`showAttitude`, `exaggerate`), car silhouettes | ✅ |
| Balance bar (`showBalance`), "Learning your car" state | ✅ |
| Narrow g-meter-only layout when attitude and balance are off ("G-meter" title) | ✅ |

## 9. Instruments — `src/widgets/Instruments.tsx`, `dashIcons.tsx`

| Feature / option | Status |
|---|---|
| Warning lights row (oil press/temp, water temp, fuel press, low fuel, stalled, limiter, rev limit, repair, optional repair) with icons | ✅ |
| Engine gauges (`showGauges`) with units and warning colours | ✅ |
| Brake bias dial (`showBias`, adjustable cars only, "Bias not adjustable") | ✅ |
| Bulb test while editing (`testLights`) | ✅ |

## 10. Fuel — `src/widgets/Fuel.tsx`

| Feature / option | Status |
|---|---|
| Level with bar, per lap, laps in tank, laps to go, lap average label | ⬜ |
| Add-to-finish line with `marginLaps`; "Complete a green lap…" state; units | ⬜ |

## 11. Radar — `src/widgets/Radar.tsx`

| Feature / option | Status |
|---|---|
| Cars within `range` from `tick.nearby`, you in the middle | ⬜ |
| Alongside warning (amber/red), side brackets | ⬜ |
| `alwaysShow`, `classColors` | ⬜ |

## 12. Tires — `src/widgets/Tires.tsx`

| Feature / option | Status |
|---|---|
| Four corners: inner/middle/outer temps (coloured), wear %, compound and sets left | ⬜ |
| Cold pressure (`showPressure`), units | ⬜ |
| "Readings appear after you stop in your pit box" state; live-in-box note | ⬜ |

## 13. Pit stop — `src/widgets/PitStop.tsx`, `src/lib/pit.ts`

| Feature / option | Status |
|---|---|
| Rows: fuel (amount, need to finish, AUTO tag), tires (pressures, compound, sets), fast repair (left), tear-off; ticked state | ⬜ |
| Hotkey hints (`showKeys`) from `general.controls` | ⬜ |
| Next stop / last sent message | ⬜ |
| Pit commands (fuel/tires/repair/tear-off/clear, fuel up/down, auto fuel with `fuelMode`, `fuelStep`, `keepFuelSet`, `autoFuel`): ⏭️ allowed (needs the controls/hotkeys service); the display must be complete | ⬜ |
| Empty state | ⬜ |

## 14. Flags — `src/widgets/Flags.tsx`

| Feature / option | Status |
|---|---|
| LED matrix board (28×28 dots, halo), every pattern: fills, SC text board (`scBoard`), checker, meatball, debris stripes, black-and-white warning, DSQ/BLK text, "1" to green | ⬜ |
| Priority order and flashing for waved flags | ⬜ |
| Green only in races and for `greenSeconds` after a start/restart | ⬜ |
| Caption per flag (`showCaption`, every `text*` option) | ⬜ |
| `alwaysShow` dark board; "No flag" in edit mode | ⬜ |

## 15. Weather — `src/widgets/Weather.tsx`, `src/lib/weather.ts`

| Feature / option | Status |
|---|---|
| Track and air temperature with units, sky in header | ⬜ |
| Track wetness bar and word, rain %, humidity % | ⬜ |
| Trend arrows (`showTrends`, `addSample`/`trend` port) | ⬜ |
| Wind compass (`showCompass`), `compass` car/north, wind arrow from `windDir`, car icon, cardinal letters upright, "No heading" note | ⬜ |
| Calm wind, speed with units | ⬜ |

## 16. Race pace — `src/widgets/RacePace.tsx`, `src/lib/raceHistory.ts`

| Feature / option | Status |
|---|---|
| History of every car's gap to the class leader (port `raceHistory`: sampling, thinning, lap marks) | ⬜ |
| Chart: leader flat at top, each car a line, labels at the right with gap and trend (`showTrend`) | ⬜ |
| `window` race/5/10/20 laps, `show` all/around with `around`, `maxGap` fixed scale | ⬜ |
| Lapped / out handling, your line highlighted, races only state | ⬜ |

## 17. Stints — `src/widgets/Stints.tsx`

| Feature / option | Status |
|---|---|
| Rows per class (`classes`, `rows`, `otherClassRows`) with tyre (`showCompound`), stint laps, stops | ⬜ |
| Timeline (`showTimeline`) coloured by compound with stop ticks, lengths (`showLengths`), lap axis | ⬜ |
| Partial stints, out cars, empty state | ⬜ |

## 18. Battle — `src/widgets/Battle.tsx`, `src/lib/battle.ts`

| Feature / option | Status |
|---|---|
| Ahead/behind in class, `lapsBetween`, gap now, P in header | ⬜ |
| Change per lap (`trendLaps`, `trend`, STEADY colouring) | ⬜ |
| Sparkline (`showSparkline`, `sparkLaps`, `history`) with area fill | ⬜ |
| iRating, last/best vs yours (`showLaps`), tyre + laps on set + stops vs yours (`showTyres`) | ⬜ |
| Near edge (< 2 s), races only state; port `check-battle.ts` tests | ⬜ |

## 19. Twitch chat (experimental) — `src/widgets/TwitchChat.tsx`, `src/lib/twitch.ts`

| Feature / option | Status |
|---|---|
| Anonymous IRC over WebSocket (`tungstenite` on a thread), tags, PRIVMSG/CLEARMSG/CLEARCHAT/RECONNECT, back-off, status | ⬜ |
| `channel` (name or link via `channelName`), `rows`, `fadeAfter`, name colours, /me actions | ⬜ |
| Emotes (`showEmotes`): images fetched and cached | ⬜ |
| Port `check-twitch.ts` tests | ⬜ |

---

## Deferred beyond this pass (tracked, not in scope)

- Multiple displays (one overlay per monitor, cross-display drag)
- Settings window (the Tauri app's Settings stays the editor; F5 picks up changes)
- Click-through regions (post-race rating thumbs), pit commands and controller buttons (need the controls service)
- Tray icon, updater, streaming, race report saving (Races page)
