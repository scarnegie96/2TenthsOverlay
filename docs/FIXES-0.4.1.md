# Fixes for 0.4.1

Progress on the fixes from the October 2026 audit. Each group of fixes is done
on its own branch (`fix/<group>`) and merged into `development` once checked.

Status: ⏳ to do · 🔧 in progress · ✅ done · ⚠️ done with a caveat

## Phase 1: wrong answers

| # | Fix | Group | Status | Notes |
|---|-----|-------|--------|-------|
| 1 | Tows and resets aren't counted as pit stops; tiny stops dropped from the average | `pitloss` | ✅ | Entry must be a small step onto pit road from track; average keeps 0.5–1.5× median. 3 new tests. |
| 2 | Variable list re-read after iRacing disconnects or the layout moves | `irsdk` | ✅ | Re-read when count, header offset or buffer size change, and on every disconnect (session info re-sent too). Type-checked against the Windows API; untested in iRacing. |
| 3 | Closing Settings mid-capture no longer leaves hotkeys switched off | `shell` | ✅ | Rust resumes controls when Settings hides; capture also ends on blur or after 15 s, with a countdown in the box. |
| 4 | Fuel to finish in timed races: one tested function, `timed_finish` example | `engine` | ⚠️ | `laps_to_go()` with both end-of-race rules; defaults to white flag then one more lap, which never asks for less fuel than 0.4.0. Counts to when the overall leader finishes, so a car behind isn't a lap short. Rule still to confirm: `cargo run -p telemetry --example timed_finish -- race.ibt` |

## Phase 2: robustness

| # | Fix | Group | Status | Notes |
|---|-----|-------|--------|-------|
| 5 | Bounds checks on iRacing's shared memory | `irsdk` | ✅ | View size from VirtualQuery; every read checked (negative, overflow, past end, 16 MB cap); a bad frame is skipped and logged once. |
| 6 | Lap file writes moved off the telemetry thread | `laps` | ✅ | Laps kept in memory; one writer thread writes the newest copy (tmp + rename). A lap finished in the last instant before quitting can still be lost, as before. |
| 7 | Repeated identical official lap times recognised | `laps` | ✅ | After the 3 s wait, iRacing's time is used if within 0.25 s of our own. |
| 8 | Corrupt config kept aside with a notice; reference saved atomically | `shell` | ✅ | Kept as `config.corrupt-<UTC date>.json`; dismissible notice in Settings (`?corrupt` in the preview). |
| 9 | Demo race keeps real time | `engine` | ✅ | Paced by a deadline clock: 5 s of demo now takes 5.0 s (was 4.8 s). |

## Phase 3: small fixes and tidying

| # | Fix | Group | Status | Notes |
|---|-----|-------|--------|-------|
| 10 | Unknown tyre compound shows nothing | `engine` | ✅ | |
| 11 | Identical controllers told apart | `shell` | ✅ | USB instance added to the id only when a twin is connected; existing bindings still match by model. Twins still share a display name. |
| 12 | Saved and imported layouts checked | `frontend` | ✅ | Every field type-checked and clamped (size 0.5–3, opacity 0.2–1). 32 checks in `scripts/check-config.ts`. |
| 13 | Delta graph's previous lap cleared on a new session | `frontend` | ✅ | |
| 14 | Duplicate comment, Flags hook warning, eslint config | `frontend` | ✅ | `npm run lint` now works (eslint added as a dev dependency); clean. |

## Release

| Step | Status |
|------|--------|
| Merge every group into `development` | ✅ |
| Full check: tsc, eslint, Rust tests, clippy, shell type-check | ✅ 92 Rust tests, clippy (incl. Windows type-check), tsc, eslint, 32 config checks, browser smoke test |
| README, ARCHITECTURE and project doc updated | ✅ |
| Version 0.4.1, merged to `main`, pushed | ✅ |
| Tag `v0.4.1` and Windows build (on your PC) | ⏳ |

## Needs checking on Windows

- Close Settings while a Controls box is waiting: hotkeys and wheel buttons still work.
- Restart iRacing in a different car: telemetry reads correctly.
- Two identical button boxes: each button binds separately.
- A timed race's fuel to finish (once the end-of-race rule is confirmed from an .ibt).

## Log

- 2026-10-31: plan agreed; six groups started in parallel.
- 2026-10-31: `pitloss` merged (#1).
- 2026-10-31: `laps` merged (#6, #7).
- 2026-10-31: `irsdk` merged (#2, #5). 71 Rust tests pass.
- 2026-10-31: `engine` merged (#4, #9, #10). 81 Rust tests pass.
- 2026-10-31: `frontend` merged (#12, #13, #14). tsc, eslint and config checks pass.
- 2026-10-31: `shell` merged (#3, #8, #11). All checks pass; released as 0.4.1 on `main`.

## Follow-ups

- Confirm iRacing's timed-race finish with `timed_finish` on a real .ibt and set `TIMED_FINISH`.
- Join the telemetry thread on quit so a lap finished in the last instant is saved.
- Show which of two identical controllers is which (they share a product name).
