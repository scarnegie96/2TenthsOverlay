# Inventory: today's Tenths → Python/PySide6

What exists now, its size, and what replaces it. Line counts are from the
`development` branch at 0.4.13.

| Area | Today | Lines | Python/Qt |
|---|---|---|---|
| Telemetry core | `crates/telemetry` (Rust) | 8,873 | Kept via PyO3 first (`core/`), optionally ported later |
| App shell | `src-tauri/src` (Rust) | 2,587 | `app.py`, `services/*`, `ui/overlay_window.py` |
| Widgets | `src/widgets` (React) | 3,978 | `ui/widgets/*` (QPainter) |
| Settings + Races | `src/settings` (React) | 1,683 | `ui/settings/*` (Qt Widgets + pyqtgraph) |
| Overlay, editing | `src/overlay` (React) | 717 | `ui/overlay_window.py`, `ui/widget_frame.py` |
| Shared logic | `src/lib` (TS) | 4,245 | `core/` (maths), `services/` (config, Twitch), `ui/theme.py` |
| Styles | `src/styles` (CSS) | 4,208 | `ui/theme.py` + painter code + a small QSS for Settings |
| OBS pages | `src/stream` + `stream.rs` | 379 | `services/stream.py` (see the plan, §5) |

---

## Telemetry core (`crates/telemetry/src`)

Kept as is behind PyO3 in Phase 2. The Python API mirrors `Engine`:
`tick(frame) -> Tick`, `field(frame) -> Field`, `set_session(yaml)`, plus
`ibt`, `blap`, `reference`, `report` helpers. Listed so an optional later port
knows the size of each part.

| File | Lines | What it does | Port notes if moved to Python |
|---|---|---|---|
| `shm.rs`, `irsdk.rs`, `vars.rs` | 696 | Shared memory, variable table, frame read, broadcast messages | `mmap` + `ctypes`; `numpy` views for per-car arrays |
| `session.rs` | 422 | Session YAML → drivers, classes, sessions, weekend info | PyYAML `CSafeLoader`; dataclasses |
| `engine.rs` | 1,323 | Trackers → Tick/Field payloads; relative, standings, gaps | Biggest single file; numpy for the 64-car loops |
| `coach.rs`, `reference.rs`, `blap.rs`, `ibt.rs`, `lapfile.rs` | 2,478 | Reference laps, coaching, .ibt/.blap read and write | Pure maths; port with its tests |
| `laps.rs` | 539 | Lap log, background writer, session laps | |
| `pitloss.rs`, `stints.rs`, `grid.rs`, `finish.rs`, `pit.rs` | 1,076 | Pit loss, stints and tyres, grid, finish rules, pit commands | Tests are the spec (timed finish rule, tow filtering) |
| `report.rs` | 428 | Race recorder and saving | Same JSON format, so old reports still open |
| `dynamics.rs`, `instruments.rs` | 305 | Car dynamics, warning lights | |
| `demo.rs`, `runner.rs`, `model.rs`, `lib.rs` | 1,606 | Demo race, run loop, payload types | Runner becomes the telemetry thread |

## App shell (`src-tauri/src`)

| File | Today | Python/Qt |
|---|---|---|
| `lib.rs` | Commands, windows, edit mode, click-through, config load/save, demo toggle | `app.py`, `ui/overlay_window.py`, `services/config.py` |
| `displays.rs` | One overlay per monitor, monitor keys, pointer polling | `QGuiApplication.screens()`; Qt gives the cursor directly (no polling) |
| `controls.rs` | Global shortcuts, pit keys, suspend while capturing | `RegisterHotKey` via `ctypes` on a hidden native window |
| `controllers.rs` | Raw Input/HID wheel and button-box buttons | `ctypes` port, or `pysdl2`; keep `vid:pid#instance` ids |
| `profiles.rs` | Saved layouts (files in `layouts/`) | `services/profiles.py`, same files |
| `ratings.rs` | `driver-ratings.json` | `services/ratings.py`, same file, same keep-unreadable rule |
| `reports.rs` | List/read/delete race reports | `services/reports.py` |
| `updates.rs` | Tauri updater, tray item, periodic check | `services/updater.py`: `latest.json` + PyNaCl minisign verify + installer |
| `stream.rs` | Local HTTP + SSE for OBS | `services/stream.py` (shape changes, see the plan) |

## Overlay and editing (`src/overlay`)

| Part | Python/Qt |
|---|---|
| `Overlay.tsx`: widgets per display, edit bar, widget picker, layout toast, clickable regions after a race | `OverlayWindow` per screen; edit bar is a child widget; clickable regions via `WS_EX_TRANSPARENT` toggled on hover, or a small separate window over the clickable part |
| `WidgetFrame.tsx`: drag, resize (corner, −/+), scale, opacity, display button, hide | `WidgetFrame(QWidget)` wrapping each widget |
| `snapping.ts`, `anchor.ts`: snap guides, anchoring to screen edges | Port as pure functions + tests |
| Cross-display drag | Native: Qt reports global positions while the mouse is held, so no WebView2 workaround |
| `Shot.tsx`: screenshot mode | `tools/screenshots.py` (off-screen) |

## Widgets (`src/widgets`)

Shared drawing to build first (`ui/painter_kit.py`): panel chrome and title bar,
class stripe, car number, iRating badge with change pill, licence chip, tyre
badge (S/M/H/W), status badges (pit, out), thumbs and rating mark, last/best
lap colouring, sparkline, `Empty` states.

| Widget | Lines | Main parts | Hard bits |
|---|---|---|---|
| Relative | 167 | Rows around you, class colours, laps up/down, tyre column, incidents and next limit in the header | Many small cells; must stay crisp at 60 Hz scrolling |
| Standings | 240 | Multiclass table, gaps/intervals, iRating change, rate buttons after the finish | Clickable thumbs while the overlay is click-through |
| Session | 131 | Track, time/laps, field and SoF, weather summary | |
| Inputs | 366 | Pedal bars, steering wheel, scrolling trace | 60 Hz trace: draw incrementally into a pixmap |
| Delta | 346 | Delta bar and trace, sectors, best/optimal/last | Same as Inputs |
| Lap log | 103 | Laps table, PBs | |
| Coach | 73 (+131 lib) | Reference-lap coaching text | |
| Car dynamics | 368 | G-meter, slip/pitch/roll car views, balance; narrow g-meter-only mode | Car silhouettes as resources |
| Instruments | 164 | Warning lights, temperatures and pressures, brake bias gauge | Icon set (`dashIcons.tsx`) to SVG resources |
| Fuel | 47 | Level, per lap, laps left, to finish | |
| Tires | 102 | Temperatures and wear per corner, sets left | |
| Radar | 112 | Cars alongside | |
| Flags | 228 | LED matrix board, captions, green only in races | Draw the matrix once per pattern into a pixmap; flashing by toggling |
| Pit stop | 183 (+255 lib) | Service menu state, hotkeys, auto fuel to finish | Logic in `core`/`services`, the widget is simple |
| Weather | 244 (+70 lib) | Temperatures, wetness, rain, humidity, wind compass that turns with the car | Rotated text; trend arrows |
| Race pace | 287 (+147 lib) | Gap chart over time for the class | Chart drawn with QPainter (or pyqtgraph embedded) |
| Stints | 191 | Every car's stints and tyres as bars | |
| Battle | 182 (+118 lib) | Cars ahead/behind: gap, change per lap, gap graph, last/best vs yours, tyres, stops | |
| Twitch chat | 124 (+204 lib) | Read-only chat, emotes, fading | Emote images: `QNetworkAccessManager` + cache |

## Settings (`src/settings`)

| Pane | Python/Qt |
|---|---|
| General: demo, units, show-when, shortcuts, controls button | `QFormLayout`; key capture widget |
| Races: list, Save reports toggle, report view | `QListView` + report view below |
| Report view: facts, class picker, results table, lap/position/gap charts with hover, legend focus, zoom, pit marks; stint chart; your laps (fuel bars, track temp) | `pyqtgraph` (`PlotWidget`, `InfiniteLine` for hover, `LegendItem`); `QTableView` |
| Layouts: save, switch, rename, import/export, shortcuts 1–9 | `QListWidget` + file dialogs |
| Appearance: theme, scale, fonts | Live preview on the overlay |
| Controls: every action, keys and wheel buttons, capture | Capture mode suspends hotkeys (as today) |
| Streaming (experimental): enable, port, URLs | Depends on the streaming choice |
| About: version, updates, release notes, corrupt-config notice | `QTextBrowser` for notes; memory readout |
| A pane per widget, generated from the option schema in `config.ts` | Same schema in Python generates the forms |

## Shared logic (`src/lib`)

| File | Goes to |
|---|---|
| `config.ts` (schema, defaults, normalise) | `services/config.py`, same JSON |
| `irating.ts`, `incidents.ts`, `battle.ts`, `rejoin.ts`, `weather.ts`, `report.ts`, `raceHistory.ts`, `deltaHistory.ts`, `coach.ts`, `pit.ts` | `core/` pure functions, pytest ports of the check scripts |
| `format.ts` | `ui/format.py` |
| `theme.ts` + CSS variables | `ui/theme.py` |
| `twitch.ts` | `services/twitch.py` |
| `controls.ts`, `displays.ts` | `services/hotkeys.py`, `ui/overlay_window.py` |
| `store.ts`, `bridge.ts` | `ui/store.py` (QObject with signals): no bridge needed |
| `mock.ts` | Replaced by the Rust/Python demo race, plus an `--offline` flag |
| `types.ts` | Dataclasses in `core/model.py` (or generated from the Rust types) |

## Files on disk that must carry over unchanged

Everything under `%APPDATA%\dev.tenths.overlay\` (or the matching config/data
folders):

- `config.json`, and the `.unreadable-*` copies
- `layouts/*.json` (saved layouts, in the config folder)
- `driver-ratings.json`
- race reports (`reports/*.json`, in the data folder)
- lap history and personal bests (`laps/`), the reference lap (`reference.json`)

## Build and release

| Today | Python |
|---|---|
| `npm run tauri build` → NSIS installer (~5 MB) | Nuitka or PyInstaller one-folder → NSIS/Inno Setup (~60–120 MB) |
| `tauri-action` + `latest.json` + minisign signature | Custom step: sign with the same key, write the same `latest.json` |
| Release when the version changes on `main` | Unchanged; only the build job differs |
| Browser preview + Playwright screenshots | `QT_QPA_PLATFORM=offscreen` renders + `tools/screenshots.py` |
| `cargo test`, `tsc`, `eslint`, check scripts | `pytest`, `pytest-qt`, `ruff`, `mypy`, plus `cargo test` while the core is Rust |
