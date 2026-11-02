# Changelog

Each version's section becomes its GitHub release notes, which Tenths shows in
Settings → About when it offers the update. Add a section like the ones below
when you bump the version.

## 0.5.0 (in development, no release yet)

- Tenths is now a native app drawn with egui instead of a Tauri app with web pages: about 120 MB in one process instead of several WebView2 processes, with each widget in its own small window over the game. Settings, layouts, laps, ratings and race reports carry over.
- Twitch chat shows emotes as pictures, inline with the words.
- Every widget, the settings window (including the Races page), hotkeys and wheel buttons, pit commands and auto fuel, several monitors, saved layouts and the tray are ported. Not yet: updates and an installer.
- OBS streaming is a **stream window** now: Settings → Streaming (or the tray's Stream window) opens a window called "Tenths stream" with your widgets on a green, magenta, black or grey background, for OBS's Window Capture. The Tauri version's web pages for Browser Sources are gone.
- Redraws up to 60 times a second (`--fps` to lower it), without waiting for the screen refresh (`--vsync` to).
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
