# Changelog

Each version's section becomes its GitHub release notes, which Tenths shows in
Settings → About when it offers the update. Add a section like the ones below
when you bump the version.

## 0.5.0 (in development, no release yet)

- Tenths is now a native app drawn with egui instead of a Tauri app with web pages: about 120 MB in one process instead of several WebView2 processes, with the widgets in one transparent window per display over the game (a small window per widget on other systems). Settings, layouts, laps, ratings and race reports carry over.
- Twitch chat shows emotes as pictures, inline with the words.
- OBS streaming is a **stream window** now: Settings → Streaming (or the tray's Stream window) opens a window called "Tenths stream" with your widgets on a green, magenta, black or grey background, for OBS's Window Capture. The Tauri version's web pages for Browser Sources are gone.
- Lighter on the CPU and GPU: each panel window now repaints on its own timer instead of every frame. Inputs, G-force, Delta, Radar and Instruments stay at 60 Hz, Flags, Pit stop, Battle and Coach run at 30 Hz, and Relative, Standings (gap included) and the other panels at 4 Hz. Car drawings are kept as ready meshes instead of rebuilt each frame.
- Every widget, the settings window (including the Races page), hotkeys and wheel buttons, pit commands and auto fuel, several monitors, saved layouts and the tray are ported.
- Redraws up to 60 times a second (`--fps` to lower it), without waiting for the screen refresh (`--vsync` to).
- Updates: Settings → About (and the tray) check GitHub for a newer version, show what's new and install it when you press Update. Downloads are checked against the release signing key, the same one the Tauri version used, so a 0.4 install can update straight into this version.
- A proper installer (`Tenths_<version>_x64-setup.exe`): per user, no administrator rights, in the same folder as before, with a Start menu shortcut and an entry in Apps. Settings, layouts, laps and reports are left alone when installing and uninstalling.
- `tenths.exe` now has the Tenths icon and version information.
- Replay: `tenths --replay race.ibt` plays an iRacing telemetry file as if it were the sim, on any platform (`--speed 2` for twice as fast, `--loop` to start over). Your own car's channels always come through; other cars (Relative, Standings, Radar, Stints, Race pace, Battle) only if the file recorded them. `cargo run -p telemetry --example make_replay_ibt` writes a demo-race file to try it with.
- Standings in practice and qualifying: ranked by best lap, with laps, the gap to the class best (to the thousandth), last and best lap, and no iRating columns. It reads iRacing's session results, which fill in as cars set laps. (Settings → Standings → Show laps.)
- Starting Tenths opens Settings, the app itself, and steps aside when iRacing connects: to the tray on Windows, minimised to the Dock where there's no tray. `--background` starts without it. Settings has a Quit button now, and on macOS its close button minimises it.
- Heap profiling for developers: `cargo build --profile heap --features heap-profile`, then `tenths --replay race.ibt --bench 60 --heap-profile` records every allocation with its call stack; `python3 tools/heap_top.py tenths-heap.json` lists the biggest sites (at the peak, at the end, and total churn). The `--bench` line gains the live heap.
- macOS: the stats line shows memory, and the file picker is the native one. (Builds are for designing layouts and working on the code; iRacing doesn't run there.)
- Inputs: the brake trace turns amber while ABS is working, and the brake bar shows an ABS tag. The ghost strip goes below the widget when there's no room above it.
- Fuel: a "Min / max lap" row shows the least and most fuel used on any clean green lap of the session.
- Flags: under a blue flag a second box shows who is behind you (position, number, name and gap), the nearest car behind that is a lap or more ahead, else the nearest behind. Switch it off with "Show who is behind under a blue flag".
- Lap log: scroll the mouse wheel over it in edit mode to see older laps.
- Windows: draws with DirectX 12 (wgpu) instead of OpenGL, so the panel windows are properly transparent on NVIDIA cards (OpenGL gave black or frosted-glass windows there). `--glow` goes back to OpenGL.
- Windows: one transparent window per display by default (about 33 fps and 200 MB, against 10-20 fps and 255 MB with a window per panel). `--panel-windows` goes back to a small window for each panel.
- The Tauri version is kept in `archive/tauri/` and on the `archive/tauri` branch.

## 0.4.13

- Flags: "Green next lap" and the green flag only show in races; in practice and test sessions the board stays dark until there's a real flag.

## 0.4.12

- Fixed: the Weather widget's compass was mirrored left to right (iRacing's car heading was read the wrong way round), so it showed north-east when you faced north-west, and turned the wrong way in corners. The wind relative to the car was wrong with it.

## 0.4.11

- Fixed: editing the layout while iRacing wasn't running showed nothing on the main display (the Relative, Stints and Battle widgets looped with no session and took every widget down with them). Fixed since 0.4.4.
- A widget that fails now shows a short note in edit mode instead of hiding every widget.

## 0.4.10

- Race reports: every race saved with everyone's laps, gaps, positions, stints and pit stops, and your fuel and laps; Settings → Races shows them with interactive charts. Saving them can be switched off there.
- Battle widget: the cars ahead of and behind you in your class, with the gap, its change per lap, a graph of the gap over the last laps, their iRating, last and best laps against yours, tyres and stops.
- Fixed: auto fuel to finish no longer re-sends every few seconds when the race needs more than a tankful.
- Fixed: an unreadable driver-ratings.json is kept aside instead of being written over.
- Fixed: turning streaming off now disconnects open OBS pages.
- Fixed: the Twitch chat widget says when it can't reach Twitch.

## 0.4.9

- Auto fuel to finish in the Pit stop widget: keeps iRacing's fuel amount at what you need to finish (Ctrl+Shift+A turns it on and off).
- Experimental: every widget as a web page for an OBS Browser Source (Settings → Streaming).
- Experimental: Twitch chat widget.
- Releases now build by themselves when the version changes on main.

## 0.4.8

- Rate drivers after the race with a thumbs up or down; a small thumb shows by their name in later races.

## 0.4.7

- Car dynamics can be just a narrow g-meter.

## 0.4.6

- The iRating change shows inside the iRating badge.
- Incidents and the next limit in the Relative's header, including limits that move on after a penalty.

## 0.4.5

- Tyre compound for each car in the Relative.
- Estimated iRating change in the Standings (official races).

## 0.4.4

- Fix dragging widgets between screens.

## 0.4.3

- First fix for dragging widgets between screens.

## 0.4.2

- Tenths can update itself (Settings → About).
