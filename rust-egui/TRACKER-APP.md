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
| Bindings from `general.controls` (keys and controller buttons per action), defaults from `controls.ts` | ⬜ |
| Global hotkeys for every action (`RegisterHotKey` via `global-hotkey`), re-registered when the config changes; pit keys only while the Pit stop widget is on and you're in the car (as `set_pit_active`) | ⬜ |
| Wheel / button-box buttons: port `controllers.rs` (Raw Input + HID, `vid:pid#instance` ids, SM_SWAPBUTTON etc.), same ids as the Tauri app so saved bindings work | ⬜ |
| Actions: `overlay.edit`, `overlay.toggle` (show/hide), `layout.next`, `layout.1`–`layout.9` | ⬜ |
| Pit actions: port `runPitAction` (fuel/fuelUp/fuelDown/autoFuel/tires/fastRepair/tearoff/clear) and `pitCommands`; send through `RunnerControl::pit_command`; pressed/sent feedback shown in the Pit stop panel | ⬜ |
| Auto fuel to finish (`keepFuelSet`, 10 s throttle, pause on manual change, tankful cap) and set-fuel-at-pit-entry (`autoFuel`) | ⬜ |

## G2. Displays and the overlay windows — `src-tauri/src/displays.rs`, `src/overlay/*`, `src/lib/displays.ts`

| Item | Status |
|---|---|
| One overlay per monitor (egui viewports), keyed like the Tauri app's display keys so `widget.display` works; widgets on a missing display fall back to the primary | ⬜ |
| Cross-display drag in edit mode (drop on another monitor moves the widget there and saves `display`) | ⬜ |
| Edit bar on every display; secondary displays say which display it is | ⬜ |
| Edit bar: layout picker (switch saved layout), widget picker (turn widgets on/off), Done | ⬜ |
| Per-widget edit controls: − / + scale, hide, move-to-next-display button | ⬜ |
| Layout toast when the layout changes | ⬜ |
| Click-through regions: after the flag, the Standings rate thumbs take clicks while everything else passes through (cursor polling + toggling passthrough, as `useClickableParts`) | ⬜ |
| Standings rate buttons (thumb up/down, one at a time) writing `driver-ratings.json` (port `ratings.rs` `set`, keep-unreadable rule) | ⬜ |

## G3. Layouts (profiles) — `src-tauri/src/profiles.rs`

| Item | Status |
|---|---|
| List / load / save / delete saved layouts in `layouts/` (same files as the Tauri app), `general.activeProfile` | ⬜ |
| Edits save into the active layout; switching applies its widgets | ⬜ |
| Import / export layout files | ⬜ |

## G4. Tray and app lifecycle — `src-tauri/src/lib.rs`

| Item | Status |
|---|---|
| Tray icon and menu: Open settings, Edit layout, Show/hide overlays, Next layout, Check for updates, Quit (`tray-icon` crate) | ✅ tray-icon, Windows only (stub elsewhere, untested on Windows); Next layout is a no-op until G2/G3 adds layout switching; Check for updates ⏭️ (G7) |
| Single instance (a second launch focuses the first) | ✅ |
| Demo toggle (`general.demo`) switching the runner live | ✅ |
| Corrupt config: keep it aside and show a notice (as `config_notice`) | ✅ |
| Laps saved (`laps_dir`), personal bests across sessions | ✅ |
| Reference lap: import `.ibt`/`.blap`, pick a lap, save `reference.json`, load at start, clear (`inspect_lap_file`, `load_reference`, `clear_reference`) | ✅ |

## G5. Settings window — `src/settings/Settings.tsx`

A second egui viewport (normal window, not an overlay).

| Item | Status |
|---|---|
| General: demo, units, show-when, shortcuts list, controls button, corrupt-config notice | ✅ |
| Controls pane: every action, key capture and wheel-button capture (suspends hotkeys while capturing), reset | ✅ key capture and reset; ⏭️ wheel-button capture UI (bindings keep and show `button`; capture needs the G1 input thread to call `feed_button`) |
| Layouts pane: save, switch, rename, delete, import/export, shortcut numbers | ✅ files in `layouts/`; applying a switch live is G2/G3 |
| Appearance: theme, every colour token with reset, follows rules | ✅ |
| A pane per widget generated from `schema.rs` options (toggle / number with min/max/step/suffix / select with help / text with maxLength), enable, scale, opacity, display | ✅ |
| Delta pane: reference lap import (file picker via `rfd`), lap list, clear | ✅ |
| About: version, release notes, updates status | ✅ version and credits; ⏭️ release notes and updates status (G7) |
| Streaming pane | ⏭️ the OBS pages are web pages; decide later (window capture, off-screen images or Spout) |

## G6. Race reports — `crates/telemetry/src/report.rs`, `src-tauri/src/reports.rs`, `src/settings/Races.tsx`

| Item | Status |
|---|---|
| Saving: `reports_dir` + `general.saveReports` wired into the runner | ✅ |
| Races page in Settings: list, delete, report view with facts, results table, lap time / position / gap charts (`egui_plot`: hover, legend focus, zoom, pit marks), stint chart, your laps | ✅ |

## G7. Updates and release

| Item | Status |
|---|---|
| Updater: `latest.json` + minisign verify (same key) + run installer, tray item, About pane | ⏭️ until the egui build becomes the release |
| Installer and release workflow for the egui build | ⏭️ same |
