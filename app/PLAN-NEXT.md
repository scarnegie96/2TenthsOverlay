# Plan: updates, installer and releases, streaming, Twitch emotes

The four things the egui app still lacks compared with the Tauri version
(see `TRACKER-APP.md` G5/G7 and `TRACKER.md` 19). Work happens on
`development`; nothing goes to `main` until the user says so.

## 1 and 2. Installer, releases and updates

These go together: an update is "download the next installer, check it, run it".

### Compatibility with existing installs (important)

People on the Tauri version (0.4.x) must be able to update straight into the
egui version from their app, with no manual step. The Tauri updater:

- reads `latest.json` from
  `https://github.com/scarnegie96/Tenths-iRacing-Overlay/releases/latest/download/latest.json`
  (`archive/tauri/src-tauri/tauri.conf.json`);
- picks `platforms["windows-x86_64"]` (Tauri also accepts
  `windows-x86_64-nsis`), downloads `url`, and verifies `signature` with the
  public key in `tauri.conf.json`: the signature is **base64 of a minisign
  signature file**, the key is base64 of a minisign public key file;
- tauri-plugin-updater 2.14: if the signature's trusted comment carries
  `version:<x>` (tab-separated `key:value` pairs, e.g.
  `timestamp:1700000000\tfile:Tenths_0.5.0_x64-setup.exe\tversion:0.5.0`),
  it must equal the announced `version`;
- runs the downloaded `.exe` with `ShellExecute("open")` and the arguments
  `/P /UPDATE /R /ARGS <its own command line>` (passive install mode,
  restart after), then exits.

So the egui release must be:

- an **NSIS installer** `Tenths_<version>_x64-setup.exe` that installs per
  user to the same place as the Tauri one (`$LOCALAPPDATA\Tenths`, program
  `tenths.exe`, the same Start menu shortcut and the same
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Tenths` entry),
  replacing the Tauri files; it closes a running `tenths.exe` first; `/P` or
  `/S` installs without pages (`/P` with a progress window), `/R` starts the
  app afterwards, `/UPDATE` and `/ARGS …` are accepted and ignored; it leaves
  `%APPDATA%\dev.tenths.overlay` (settings, layouts, laps, reports) alone,
  including on uninstall;
- signed with the **same key** (the repository secrets
  `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: base64
  of a minisign *encrypted secret key* file, and its password), with the
  version in the trusted comment;
- published with a Tauri-format `latest.json`:

```json
{
  "version": "0.5.0",
  "notes": "the CHANGELOG.md section for 0.5.0",
  "pub_date": "2026-11-02T12:00:00Z",
  "platforms": {
    "windows-x86_64": { "signature": "<base64 .sig>", "url": "https://github.com/…/releases/download/v0.5.0/Tenths_0.5.0_x64-setup.exe" },
    "windows-x86_64-nsis": { "signature": "…", "url": "…" }
  }
}
```

### Pieces

| # | Piece | Where |
|---|---|---|
| R1 | Release tool (Rust, small): `sign` (decrypt the secret key with the password, minisign-sign a file with the trusted comment above, write `<file>.sig` = base64 of the signature file) and `manifest` (write `latest.json` from version, notes, url, sig). Uses the `minisign` crate (`minisign-verify` for its own round-trip test). Unit tests with a key pair generated in the test. | `crates/release/` (workspace member, binary `tenths-release`) |
| R2 | NSIS script: per-user install as above, uninstaller, Start menu (and optional desktop) shortcut, closes the running app, `/P` `/S` `/R` handling, version and icon from the build. | `app/installer/tenths.nsi` |
| R3 | App icon and version info in `tenths.exe` (`app/assets/icons/icon.ico` via a `build.rs` with `winresource`, Windows only). | `app/build.rs` |
| R4 | Release workflow: on push to `main` that changes `app/Cargo.toml`'s version (or manual run), on `windows-latest`: build release, `makensis`, sign, write `latest.json`, create the GitHub release `v<version>` with the installer, `.sig` and `latest.json`, notes from `CHANGELOG.md`. Skips if the release exists; fails clearly without the secrets. A manual run can make a pre-release (not "latest", so nobody's updater sees it). | `.github/workflows/release.yml` |
| R5 | Windows build workflow also builds the installer (unsigned) as an artifact, so it can be tried before releasing. | `.github/workflows/build.yml` |
| U1 | Updater in the app: fetch `latest.json` (same URL), compare with `CARGO_PKG_VERSION` (semver), verify the signature with the same public key (`minisign-verify`, base64 decode, version in trusted comment must match), download with progress, verify again on the bytes, run the installer `/P /R`, quit. | `app/src/updates.rs` |
| U2 | When: 20 s after start, then every 6 hours, if `general.checkUpdates` (default on); "Check now" any time. Never during a session on track? (just never interrupt: installing is always the user's click). | `app/src/updates.rs`, `main.rs` |
| U3 | Settings → About: version, last checked, Check now, the new version with its notes, **Update to x.y.z** with progress, the auto-check toggle, errors in words. Tray: "Check for updates" / "Update to x.y.z". A dot on About when there's an update. | `app/src/settings/about.rs`, `shell/tray.rs` |
| U4 | Tests: manifest parsing (both platform keys), version compare, signature check (good, tampered bytes, wrong key, version mismatch) using R1's signer in tests. | |

Note: the repository is **private** at the moment, so GitHub serves release
files only to signed-in users; the updater (Tauri's or this one) can't reach
`latest.json` until the repo is public or releases are hosted elsewhere. The
code handles a 404 as "couldn't check" in plain words.

## 3. Streaming (OBS)

The Tauri version served widgets as web pages. The egui version can't do that
cheaply, so it gets what OBS captures well: a **stream window**.

| # | Piece |
|---|---|
| S1 ✅ | Settings → Streaming (replaces the placeholder): **Stream window** on/off; **Background**: green `#00FF00`, magenta `#FF00FF`, black or transparent-looking dark grey; **Shows**: the layout of display 1, 2… (widgets where they are on that screen) or chosen widgets only (a checklist); **Size**: 50 / 75 / 100 % of that display; a short how-to for OBS (Window Capture → the window "Tenths stream", Capture method "Windows 10 (1903 and up)", add a Chroma Key filter for the background colour). Saved in `general.streaming` (keep the Tauri fields `enabled`/`port`, add `window`, `background`, `display`, `widgets`, `scale`). |
| S2 ✅ | The window: a normal (decorated, not topmost, not click-through, in the taskbar) egui viewport titled "Tenths stream", drawing the chosen widgets at their layout positions scaled, on the solid background, at the app's redraw rate; it works while covered by iRacing (OBS captures it anyway). Closing it turns the option off. |
| S3 ✅ | Tray item "Stream window" (on/off). README section rewritten. |
| S4 ✅ | Off by default; costs nothing when off. |

## 4. Twitch emotes as images

| # | Piece |
|---|---|
| E1 | Fetch `https://static-cdn.jtvnw.net/emoticons/v2/<id>/static/dark/1.0` (static PNG; `2.0` when the widget is scaled above 125%) on a background thread (`ureq`), decode with `image`, upload as an egui texture, cache by id (memory, capped at ~200), one request per id, failures remembered and shown as the code text. |
| E2 | Lay out each message with inline images: words and emotes placed left to right with wrapping at the widget width, emote height ≈ the text line height, vertically centred; the row height grows with wrapped lines. Fading and actions as now. |
| E3 | Tests: splitting a message into text/emote runs from the IRC emote ranges (code points, overlapping/invalid ranges ignored) and the wrapping maths. The demo chat includes an emote id that's present in the cache from a bundled tiny PNG so renders show an image without the network. |
