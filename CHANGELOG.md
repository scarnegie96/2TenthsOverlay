# Changelog

Each version's section becomes its GitHub release notes, which Tenths shows in
Settings → About when it offers the update. Add a section like the ones below
when you bump the version.

## Unreleased

- Two new themes in Settings → Appearance. **Night**: dim and warm for night stints and dark rooms, with nearly solid panels, amber text and nothing close to white, and the flag board, radar and tyre colours turned down so they don't glare. **'90s**: yellow and white on deep blue like the TV graphics of the time, with heavy condensed italic type, headings on slanted yellow tabs and positions in yellow and white boxes. Both have their own colour pickers like the other themes.

## 0.6.2

- Windows: panels on the right and bottom of a screen bigger than 1920x1080 can be clicked, moved, resized and hidden again. The overlay window started at 1920x1080 and was made click-through before it was stretched to the screen, and Windows kept taking the mouse only over that first size, so everything past it fell through to the desktop. The overlay window now starts at its display's size.
- Quit really ends Tenths on Windows now. The 1.5 s safety net still went through Windows' normal process exit, which can hang in the graphics driver's shutdown and leave Tenths in Task Manager; it now ends the process outright.
- Radar: a new look. Your car with a translucent band beside it for each car alongside, placed where that car is along the track (red while it overlaps you, orange when only partly alongside), and a glow for the nearest car close ahead and behind that grows stronger as the gap shrinks. iRacing only says *that* a car is alongside, not where it is across the track, so the old top-down view had to guess the lanes and sometimes got them wrong. (The top-down view is still there with `"style": "classic"` in the radar's options in config.json.)
- Radar: a car that drops in behind or pulls in ahead of you leaves the side as soon as iRacing says nobody is beside you, instead of staying there while it follows within 14 m. A car that pulled away keeps its side for at most 1.5 s. And when someone comes alongside, a car that was just on the other side no longer takes their side.
- Delta: works in races. iRacing doesn't publish its lap deltas there, so Tenths now records your own laps and compares against your best and last clean laps this session whenever iRacing has no delta of its own. (Optimal and session optimal still come from iRacing only.)
- Delta: in qualifying and races, "Best lap" and "Optimal lap" compare against this session's best lap and sectors. iRacing's own are your all-time best for the car and track, which say little about how a race is going. Practice is unchanged.
- Battle: the gap to the cars around you is live now, worked out from where the cars are on track like the Relative. Before, it came from iRacing's timing, which only changes as cars cross timing lines, so it sat still and then jumped. (Cars a lap or more apart still use the timing.)
- Battle: the gap graph is gone (it looked the same whether you were catching or being caught). Instead each car shows its latest time through every sector and the difference to yours: red where they were faster, green where slower.
- Race pace: the lines follow the same live gaps, so they move every couple of seconds instead of only when timing updates. Battle's change per lap comes from the same history and is smoother too.
- Race pace: cars no longer drop to the bottom of the chart for a few seconds as they cross the line. Their lap counter ticks a moment after they cross, which made them look a lap down; whole laps now come from iRacing's timing gap instead. The same fix stops Battle briefly showing a car as a lap away.
- Race pace shows the leader's last 5 laps by default (also 3, 10, 20 or the whole race, in its settings).
- Race pace: a "Relative to you" option (off by default) puts you along the middle with the cars either side of you (3 by default) at their gap to you, instead of everyone against the leader.
- Twitch chat and the OBS stream window are taken out for now, to keep the overlay focused on driving. They're kept aside for later (see Future work in the README); a saved layout with Twitch chat in it still loads, without it.

## 0.6.1

- Fixed the screen staying white after pressing Done in layout editing in the full-screen overlay on Windows.
- Quit (tray, hotkey, edit bar) always quits now. Before, it could be left running and need Task Manager.
- Edit Layout: clicks on panels are no longer ignored by a stale "over the UI" check, and the top of the screen shows what the last click did (hit, over the edit bar, no panel there) to help track down any panel that still won't respond.
- Windows with more than one display (or a mix of scales): each overlay window now covers its whole display. Before, a window could be smaller than what was drawn, so the right and bottom parts of the screen ignored clicks (panels there couldn't be moved or clicked).
- Edit Layout: panels that overlap can all be picked now. Clicking where several overlap grabs the smallest one, so a small panel under a big one is no longer stuck.

## 0.6.0

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
- Release builds no longer print diagnostics (they're debug-build only), no longer time every panel's draw unless `--bench` asks, and no longer format each widget's options or read environment variables on every repaint.
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
