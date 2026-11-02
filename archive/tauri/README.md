# Tenths: the Tauri version (archived)

The previous version of Tenths: a Tauri 2 app with a React front end, the
widgets drawn as web pages in WebView2. It was replaced by the native egui app
in [`app/`](../../app), which reads and writes the same settings files. This
folder is kept as it was at 0.4.13 (the last release, plus the fixes after it
on `development`), and the `archive/tauri` branch has the whole repository as
it was before the switch.

The egui widgets were ported from the code here (`src/widgets/*.tsx`,
`src/lib/*.ts`), so it's still the reference for how each one should look and
behave.

To build it (Windows, with Rust and Node.js 20+):

```powershell
cd archive\tauri
npm install
npm run tauri dev      # run with hot reload
npm run tauri build    # installer in src-tauri\target\release\bundle\nsis\
```

The front end still runs in a browser with a fake race (`npm run dev`, then
`http://localhost:1420/overlay.html`). `docs/ARCHITECTURE.md` describes how
data flows and how every number is worked out; that's the same in the egui app.
Releases of this version came from `.github/workflows/release-tauri.yml`, which
now only runs when started by hand.
