# Tenths on Python + PySide6: plan

Status: **plan only**. Nothing here has been built yet. This folder is where the
port would live if it goes ahead (`pyside-port/tenths/…`), next to the current
Tauri app, until it reaches parity.

Contents:

- [README.md](README.md): this file. Would it work, the architecture, the phases, risks, and a go/no-go gate.
- [INVENTORY.md](INVENTORY.md): every current feature and file, and what replaces it in Python/Qt.

---

## 1. Would it work?

**Yes, technically.** Everything Tenths does has a working Python/Qt
equivalent, and several other iRacing overlays are built this way. The real
questions are cost and what gets worse, not feasibility.

| Need | Python / Qt answer | Confidence |
|---|---|---|
| Transparent, always-on-top, frameless overlay | `Qt.FramelessWindowHint \| Qt.WindowStaysOnTopHint \| Qt.Tool` + `WA_TranslucentBackground` | High: standard Qt |
| Clicks pass through to iRacing; clickable while editing | Win32 `WS_EX_TRANSPARENT \| WS_EX_LAYERED` toggled with `ctypes` (no window re-create); `Qt.WindowTransparentForInput` as fallback | High |
| Don't steal focus from iRacing | `WS_EX_NOACTIVATE` + `Qt.WindowDoesNotAcceptFocus` | High |
| One overlay per display, DPI per display | `QGuiApplication.screens()`, `QScreen.geometry()/devicePixelRatio()`, `screenAdded/Removed` signals | High |
| Read iRacing shared memory at 60 Hz | `mmap` + the data-valid event (`WaitForSingleObject` via `ctypes`), or the existing Rust reader through PyO3 | High |
| Session YAML | `PyYAML` with the C loader (`CSafeLoader`), only when the update count changes | High |
| Pit commands, chat macros | `SendNotifyMessageW` broadcast via `ctypes` (as `pit.rs` does today) | High |
| Global hotkeys | `RegisterHotKey` on a hidden native window (`ctypes`) or `QAbstractNativeEventFilter` | High |
| Wheel / button box buttons | Raw Input + HID as today (`ctypes`, port of `controllers.rs`), or SDL2 via `pysdl2` | Medium: the port is fiddly, the result works |
| Smooth 60 Hz graphics (inputs trace, delta, g-meter, LED flag board) | `QPainter` on cached `QPixmap`s, repainting only what changed; Qt Quick (QML) scene graph if QPainter isn't enough | Medium-high: needs the spike to confirm CPU cost |
| Settings window, forms, lists | Qt Widgets | High |
| Interactive race-report charts (hover, legend, zoom) | `pyqtgraph` (fast, interactive) or Qt Charts | High |
| Tray icon and menu | `QSystemTrayIcon` | High |
| Self-update from GitHub releases with signature check | Custom: read the same `latest.json`, download, verify the **same minisign key** (Ed25519) with `PyNaCl`, run the installer | Medium: has to be written, roughly 300 lines |
| OBS streaming | No web pages any more. Choose: (a) OBS *Window Capture* of a dedicated "stream" window with a key colour, (b) widgets rendered off-screen to PNG and served over local HTTP/MJPEG, (c) Spout2/NDI later | Medium: a different shape from today |
| Twitch chat | `QtWebSockets` (`QWebSocket`), same anonymous IRC protocol | High |
| VR later (OpenVR/OpenXR) | Easier than today: Qt renders each widget to a `QImage` → texture (`pyopenvr`), no browser capture needed | Medium |

### What gets better

- **Memory.** One process, no WebView2. A Qt overlay with 15–20 widgets should
  sit around **100–200 MB** total, not today's ~600 MB across `Tenths.exe` plus
  several `msedgewebview2.exe` processes, and without the WebView2 growth from
  script-based event delivery (the suspect for the climb at Spa).
- **No IPC.** Telemetry and widgets share memory in one process: no
  60-times-a-second JSON serialisation and `eval` into every window.
- **One language** for the app shell, logic and UI (if the core is ported too).
- **VR** becomes much more reachable.

### What gets worse

- **It's a rewrite of the whole UI**: about 10,600 lines of TypeScript/TSX and
  4,200 lines of CSS. CSS-level polish (gradients, blur, rounded tables, fonts,
  three themes) has to be redone with QPainter/QSS, and that's slower to iterate
  on than CSS.
- **No browser preview.** Today every widget runs in a normal browser with a
  mock race (`?all`, `?warmup`, `?offline`), and screenshots come from
  Playwright. Qt can do the equivalent off-screen (`QT_QPA_PLATFORM=offscreen`
  plus `QWidget.grab()`), so the screenshot and test workflow survives, but it
  has to be rebuilt.
- **Python's speed and the GIL.** Fine for 60 Hz at this data size if the hot
  paths are careful (or stay in Rust), but it's the first thing to measure.
- **Packaging.** A frozen Python app (PyInstaller or Nuitka) is a bigger
  installer (roughly 60–120 MB against today's ~5 MB), and unsigned PyInstaller
  builds are often flagged by antivirus. Code signing becomes more important.
- **OBS pages** change shape (see the table above).

### Honest recommendation

The memory problem alone **doesn't need a rewrite**. Two days on the current
app (fewer and smaller updates, live updates over the local SSE server instead
of Tauri `eval`, a memory readout) will show whether the leak goes away. Do
that first, as **Phase 0** below, because it's cheap and tells us whether
WebView2 is really the cause.

Go to Python/Qt if, after Phase 0, any of these hold:

- memory is still far above ~250 MB, or still climbs;
- you want VR soon;
- you'd rather work in Python than TypeScript/Rust from now on.

---

## 2. Target architecture

```
pyside-port/
  pyproject.toml              # deps, tool config (ruff, mypy, pytest)
  tenths/
    __main__.py               # entry point: python -m tenths
    app.py                    # QApplication, single-instance lock, tray, startup
    core/                     # no Qt imports here: pure logic, fully unit-tested
      irsdk.py                # shared memory reader (or a thin wrapper over the Rust core)
      session.py              # YAML -> SessionInfo
      model.py                # frozen dataclasses: Tick, Field, Session, ...
      engine.py               # trackers -> payloads (port of engine.rs)
      fuel.py laps.py sectors.py stints.py pitloss.py grid.py finish.py
      coach.py reference.py ibt.py blap.py dynamics.py instruments.py
      report.py demo.py pit.py
    services/
      config.py               # same JSON schema and file location as today
      profiles.py ratings.py reports.py
      updater.py              # latest.json + minisign verify + run installer
      hotkeys.py controllers.py   # Win32 via ctypes
      stream.py               # OBS output (see §5)
      twitch.py
    ui/
      theme.py                # tokens -> QColor/QFont; three themes
      overlay_window.py       # one per screen; click-through; edit mode
      widget_frame.py         # drag, resize, snapping guides, display button
      painter_kit.py          # shared drawing: panels, badges, class stripes, sparklines
      widgets/                # one module per widget (19)
      settings/               # settings window, panes, Races page
    resources/                # fonts, icons, car silhouettes
  tests/                      # pytest: ported Rust tests and TS check scripts
  tools/
    screenshots.py            # off-screen renders of every widget (README images)
    build.py                  # freeze + installer
```

### Threads and data flow

```
 telemetry thread (QThread or plain thread)
   wait for iRacing's data-valid event (≤16 ms)
   read frame -> engine.tick(frame) -> Tick (every frame), Field (10 Hz)
   publish: latest snapshot in a lock-free slot + Qt signal (queued, coalesced)
        │
        ▼
 Qt main thread
   Store (QObject): holds the latest Tick/Field/Session/config
   each widget subscribes to the parts it uses and repaints only if they changed
   a single 60 Hz QTimer drives the animated widgets (inputs trace, delta, flags)
```

- **Coalescing:** the UI always draws the *latest* frame. If it's late it skips
  frames rather than queueing them, so lag never builds up (today's equivalent
  is React re-rendering on each event).
- **Immutable payloads:** frozen dataclasses (or plain tuples) passed by
  reference, with no copying or serialising.
- **Repaint budget:** every widget declares `wants_tick` (60 Hz) or field-only
  (10 Hz). Static parts (frames, labels, grid lines) are cached in a `QPixmap`
  and redrawn only on resize or theme change.

### The core: keep Rust or port it?

| | **A. Keep the Rust crate via PyO3** (`maturin`) | **B. Port to Python** |
|---|---|---|
| Work | ~1 week (bindings + packaging) | ~3–4 weeks (8,900 lines, 87 tests) |
| Risk | Low: the logic is proven and tested | Medium: subtle maths (coach, pit loss, stints, finish rules) |
| Speed | Rust speed | Needs care; `numpy` for the per-car arrays |
| One language | No | Yes |
| Build | Needs Rust in CI (already there) | Pure Python |

**Recommendation: A first, B optionally later**, one module at a time, with
the Rust tests ported to pytest as the safety net. Phase 1 works with both.

---

## 3. Phases

Rough estimates assume I build and you test on Windows with iRacing. Each phase
ends with something you can run.

### Phase 0: confirm the cause cheaply, in the current app (2–3 days)

1. Memory readout in Settings → About: each Tenths process (Rust and every
   WebView2 child), working set and growth since iRacing connected.
2. Cut event traffic: tick at 30 Hz, only to windows that have widgets, smaller
   tick payload.
3. Live updates over the local SSE server (already exists for OBS) instead of
   Tauri `emit`/`eval`.
4. You run 10 laps at Spa before and after.

**Gate:** if memory is flat and reasonable, stop here and keep Tauri. Otherwise
continue.

### Phase 1: spike, Qt over iRacing (1 week) — go/no-go

Prove the risky parts with throwaway-quality code:

- one overlay window per screen; transparent, on top of iRacing in borderless
  windowed mode, click-through, no focus stealing, DPI-correct;
- Relative (table) and Inputs (60 Hz trace) widgets fed by live telemetry
  (via the Rust core through PyO3);
- edit mode: drag one widget, and drag it to the other screen;
- measure: CPU % and memory after 20 minutes; frame time per widget;
- freeze with PyInstaller *and* Nuitka; check Windows Defender and VirusTotal;
  installer size.

**Gate (all must hold):** memory under 250 MB and flat; total UI CPU under ~5%
of one core with both widgets; no flicker or focus problems over iRacing; the
build not flagged by Defender. If any fail and can't be fixed in the spike,
stop and stay on Tauri.

### Phase 2: foundations (2 weeks)

- `core`: PyO3 bindings for the whole engine (`tick`, `field`, `session`,
  reports, reference laps, `.ibt` import), demo source included.
- `services/config.py`: **reads and writes today's `config.json` at the same
  path**, so settings, layouts, driver ratings and race reports carry over
  untouched (`config.json`, `layouts/`, `laps/`, `reports/`, `driver-ratings.json`, `reference.json`). Same `normalizeConfig` rules (port `config.ts`).
- Theme system: the CSS custom properties (`--ink`, `--gain`, `--loss`,
  `--line`, …) become a `Theme` dataclass; three themes ported; fonts bundled.
- `painter_kit`: panel chrome, headers, class stripe, iRating badge (with
  change), licence chip, tyre badge, thumbs, status badge, sparkline, number
  formatting (port `format.ts`).
- Overlay windows, widget frame, drag/resize/snapping/anchors, cross-display
  drag (Qt gets global mouse positions natively, so no polling workaround is
  needed), edit bar, widget picker, layout switcher.
- Tray menu, single instance, start minimised, hotkeys (port `controls.rs`).
- Off-screen screenshot tool and a mock/demo race for development.

### Phase 3: widgets (3–4 weeks)

In order of use (see INVENTORY.md for each one's parts):

1. Relative, Standings, Session, Fuel, Flags, Delta
2. Inputs, Car dynamics, Instruments, Tires, Lap log, Radar
3. Pit stop (with hotkeys and auto fuel), Coach, Weather (compass)
4. Race pace, Stints, Battle, Twitch chat

Each widget: a pytest for its maths (ported from the TS check scripts where they
exist), an off-screen screenshot in all three themes, and a check against the
current app's screenshot.

### Phase 4: settings and reports (2 weeks)

- Settings window: General, Races, Layouts (save/import/export), Appearance,
  Controls (key capture, wheel button capture), Streaming, About (updates,
  memory), a pane per widget generated from the same option schema as
  `config.ts`.
- Races page: list, report view, results table, interactive charts with
  `pyqtgraph` (lap times, positions, gaps; hover tooltip, legend focus, zoom,
  pit marks), stint chart, your laps (fuel bars, track temperature).

### Phase 5: services (1–2 weeks)

- **Updater:** same `latest.json` and the same minisign public key; verify with
  PyNaCl; download; run the installer silently; restart. Tray item and About
  pane as today.
- **Controllers:** port `controllers.rs` (Raw Input/HID) via `ctypes`, or use
  SDL2. Keep the device id scheme (`vid:pid#instance`) so saved bindings still
  work.
- **Streaming (experimental):** pick one of: dedicated stream window for OBS
  Window Capture (simplest); off-screen PNG/MJPEG over the existing local
  HTTP+SSE design; or Spout2. Keep `127.0.0.1` only and the Host check.
- **Twitch chat:** `QWebSocket`, port of `twitch.ts` (parser, emotes,
  CLEARMSG/CLEARCHAT, back-off).
- Race report saving, driver ratings (same files).

### Phase 6: packaging, release, migration (1 week)

- Freeze with Nuitka (fewer antivirus false positives, faster startup) or
  PyInstaller one-folder; NSIS or Inno Setup installer with the **same install
  folder and app name**.
- GitHub Actions: build on `windows-latest`, sign with the existing minisign key
  (`TAURI_SIGNING_PRIVATE_KEY` secret) to produce `latest.json`, upload as today.
  The release-on-version-bump workflow stays as it is; only the build step
  changes.
- **Migration path for existing installs:** the last Tauri release's updater
  downloads `latest.json` and runs an NSIS installer passively. If the first
  Python release ships as an NSIS installer signed with the same key, existing
  users update into it automatically. The installer must remove the old Tauri
  files (or install over them) and keep `%APPDATA%\dev.tenths.overlay`.
- Code signing certificate (optional but strongly advised for a frozen Python
  app).

### Phase 7: parity and beta (1–2 weeks)

- A side-by-side checklist from INVENTORY.md, each item ticked on Windows in
  iRacing.
- Beta channel: a second `latest.json` (for example `beta.json`) that the
  Python build reads, so you can switch back to the Tauri build at any time
  until parity.
- Soak test: a 60-minute race with all widgets; memory and CPU logged by the
  app.

**Total: about 10–14 weeks** after Phase 0, of which Phase 3 (widgets) and
Phase 4 (settings/reports) are more than half.

---

## 4. Testing

- **Unit tests (pytest):** everything in `core/` and `services/`. The existing
  Rust tests stay valid while the core is Rust; the TS check scripts
  (`check-config`, `check-irating`, `check-incidents`, `check-pit`,
  `check-twitch`, `check-battle`, `check-report`) become pytest modules.
- **Widget render tests:** `pytest-qt` with `QT_QPA_PLATFORM=offscreen`: each
  widget rendered with the demo race, with `?offline`-style empty data, and
  with every option on and off. Nothing may raise, which catches the kind of bug
  behind 0.4.11's blank screen.
- **Screenshots:** `tools/screenshots.py` renders every widget and the
  overviews in three themes for the README, like `scripts/screenshots.mjs`.
- **Performance:** a benchmark that runs 10 minutes of the demo at 60 Hz
  through all widgets off-screen and fails if the frame time or memory goes
  over budget.
- **On Windows (you):** the Phase 7 checklist.

---

## 5. Decisions to make

1. **Do Phase 0 first?** Recommended. It may make the port unnecessary.
2. **Core:** keep Rust via PyO3 (recommended) or port to Python now.
3. **Rendering:** QPainter widgets (recommended: simplest, full control) or
   QML/Qt Quick (smoother animation, but a second language).
4. **Freezer:** Nuitka (recommended) or PyInstaller.
5. **OBS streaming:** window capture, off-screen images, or Spout2.
6. **Licence:** PySide6 is LGPL v3. Fine for a free, open-source app shipped as
   a one-folder build (Qt DLLs replaceable). Note it in the README and the About
   pane.

---

## 6. Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| UI CPU too high at 60 Hz in Python | Medium | High | Pixmap caching; repaint only on change; 30 Hz for non-input widgets; QML fallback; measured in the Phase 1 gate |
| Antivirus flags the frozen app | Medium | High | Nuitka; code signing; submit to Microsoft; Phase 1 check |
| Click-through or focus quirks over iRacing | Low–medium | High | Win32 extended styles directly; tested in Phase 1 |
| Look and feel worse than today | Medium | Medium | Port the theme tokens exactly; screenshot comparison per widget |
| Parity takes longer than planned | High | Medium | Tauri app stays the release until parity; beta channel |
| Settings or bindings lost on migration | Low | High | Same config path and schema; migration tested with real files from your PC |
| OBS users lose the web pages | Medium | Low | Experimental feature; window capture documented |
| Two codebases during the port | High | Medium | Feature freeze on Tauri (fixes only) once Phase 3 starts |
