# egui port tracker: the app around the panels

`TRACKER.md` covers the 19 panels. This file covers everything else the Tauri
app does, so the egui build can replace it. The Tauri code is the spec:
`src-tauri/src/*.rs` (shell) and `src/settings/*`, `src/overlay/*`,
`src/lib/{controls,pit,store,displays}.ts` (front end).

Branch: **development only.** Nothing goes to `main`.

Legend: ⬜ to do · 🟨 in progress · ✅ done · ⏭️ deferred (reason given)

Work starts once the panel agents are merged (these touch the shared files:
`main.rs`, `config.rs`, `frame.rs`, `data.rs`).

## G1. Controls: hotkeys, wheel buttons, actions — `src-tauri/src/controls.rs`, `controllers.rs`, `src/lib/controls.ts`

| Item | Status |
|---|---|
| Bindings from `general.controls` (keys and controller buttons per action), defaults from `controls.ts` | ✅ `controls/mod.rs` `parse_bindings`: saved entry when well formed (as `normalize`), else Ctrl+Shift+default; re-read when the config changes |
| Global hotkeys for every action (`RegisterHotKey` via `global-hotkey`), re-registered when the config changes; pit keys only while the Pit stop widget is on and you're in the car (as `set_pit_active`) | ✅ `controls/hotkeys.rs` (`global-hotkey`), re-registered on change; pit keys only with the widget on, connected and in the car; suspended via `Controls::set_suspended`. Windows-only, untested here; Ctrl+Shift+Q (quit) kept as an extra |
| Wheel / button-box buttons: port `controllers.rs` (Raw Input + HID, `vid:pid#instance` ids, SM_SWAPBUTTON etc.), same ids as the Tauri app so saved bindings work | ✅ `controls/controllers.rs`, a copy of the Tauri code (Raw Input + HID, same ids, hat 1001-1004, twin rule in `button_actions`); Windows-only, untested here |
| Actions: `overlay.edit`, `overlay.toggle` (show/hide), `layout.next`, `layout.1`–`layout.9` | ✅ edit and show/hide are `AppAction`s handled in `main.rs`; `layout.*` call `controls/layouts.rs` `switch_layout`, a logging no-op marked MERGE POINT until saved layouts (G3) land |
| Pit actions: port `runPitAction` (fuel/fuelUp/fuelDown/autoFuel/tires/fastRepair/tearoff/clear) and `pitCommands`; send through `RunnerControl::pit_command`; pressed/sent feedback shown in the Pit stop panel | ✅ `controls/pit.rs` `PitControl::run_action`, `pit_commands`; sent through `RunnerControl::pit_command`; lit key, last-sent footer and AUTO paused shown in the Pit stop panel |
| Auto fuel to finish (`keepFuelSet`, 10 s throttle, pause on manual change, tankful cap) and set-fuel-at-pit-entry (`autoFuel`) | ✅ `PitControl::update`: 10 s throttle, new field payload only, not on pit road, paused by hand / clear, tankful cap; set-fuel-at-pit-entry with the by-hand skip. Case tests in `controls/pit.rs` |

## G2. Displays and the overlay windows — `src-tauri/src/displays.rs`, `src/overlay/*`, `src/lib/displays.ts`

| Item | Status |
|---|---|
| One overlay per monitor (egui viewports), keyed like the Tauri app's display keys so `widget.display` works; widgets on a missing display fall back to the primary | ✅ |
| Cross-display drag in edit mode (drop on another monitor moves the widget there and saves `display`) | ✅ |
| Edit bar on every display; secondary displays say which display it is | ✅ |
| Edit bar: layout picker (switch saved layout), widget picker (turn widgets on/off), Done | ✅ |
| Per-widget edit controls: − / + scale, hide, move-to-next-display button | ✅ |
| Layout toast when the layout changes | ✅ |
| Click-through regions: after the flag, the Standings rate thumbs take clicks while everything else passes through (cursor polling + toggling passthrough, as `useClickableParts`) | ✅ (Windows-only: cursor polling is `GetCursorPos`, untested here) |
| Standings rate buttons (thumb up/down, one at a time) writing `driver-ratings.json` (port `ratings.rs` `set`, keep-unreadable rule) | ✅ |

## G3. Layouts (profiles) — `src-tauri/src/profiles.rs`

| Item | Status |
|---|---|
| List / load / save / delete saved layouts in `layouts/` (same files as the Tauri app), `general.activeProfile` | ✅ |
| Edits save into the active layout; switching applies its widgets | ✅ |
| Import / export layout files | ✅ (functions `Layouts::import/export`; the file-picker UI belongs to the G5 Layouts pane) |

## G4. Tray and app lifecycle — `src-tauri/src/lib.rs`

| Item | Status |
|---|---|
| Tray icon and menu: Open settings, Edit layout, Show/hide overlays, Next layout, Check for updates, Quit (`tray-icon` crate) | ⬜ |
| Single instance (a second launch focuses the first) | ⬜ |
| Demo toggle (`general.demo`) switching the runner live | ⬜ |
| Corrupt config: keep it aside and show a notice (as `config_notice`) | ⬜ |
| Laps saved (`laps_dir`), personal bests across sessions | ⬜ |
| Reference lap: import `.ibt`/`.blap`, pick a lap, save `reference.json`, load at start, clear (`inspect_lap_file`, `load_reference`, `clear_reference`) | ⬜ |

## G5. Settings window — `src/settings/Settings.tsx`

A second egui viewport (normal window, not an overlay).

| Item | Status |
|---|---|
| General: demo, units, show-when, shortcuts list, controls button, corrupt-config notice | ⬜ |
| Controls pane: every action, key capture and wheel-button capture (suspends hotkeys while capturing), reset | ⬜ |
| Layouts pane: save, switch, rename, delete, import/export, shortcut numbers | ⬜ |
| Appearance: theme, every colour token with reset, follows rules | ⬜ |
| A pane per widget generated from `schema.rs` options (toggle / number with min/max/step/suffix / select with help / text with maxLength), enable, scale, opacity, display | ⬜ |
| Delta pane: reference lap import (file picker via `rfd`), lap list, clear | ⬜ |
| About: version, release notes, updates status | ⬜ |
| Streaming pane | ⏭️ the OBS pages are web pages; decide later (window capture, off-screen images or Spout) |

## G6. Race reports — `crates/telemetry/src/report.rs`, `src-tauri/src/reports.rs`, `src/settings/Races.tsx`

| Item | Status |
|---|---|
| Saving: `reports_dir` + `general.saveReports` wired into the runner | ⬜ |
| Races page in Settings: list, delete, report view with facts, results table, lap time / position / gap charts (`egui_plot`: hover, legend focus, zoom, pit marks), stint chart, your laps | ⬜ |

## G7. Updates and release

| Item | Status |
|---|---|
| Updater: `latest.json` + minisign verify (same key) + run installer, tray item, About pane | ⏭️ until the egui build becomes the release |
| Installer and release workflow for the egui build | ⏭️ same |
