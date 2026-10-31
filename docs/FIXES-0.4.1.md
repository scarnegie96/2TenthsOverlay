# Fixes for 0.4.1

Progress on the fixes from the October 2026 audit. Each group of fixes is done
on its own branch (`fix/<group>`) and merged into `development` once checked.

Status: ⏳ to do · 🔧 in progress · ✅ done · ⚠️ done with a caveat

## Phase 1: wrong answers

| # | Fix | Group | Status | Notes |
|---|-----|-------|--------|-------|
| 1 | Tows and resets aren't counted as pit stops; tiny stops dropped from the average | `pitloss` | 🔧 | |
| 2 | Variable list re-read after iRacing disconnects or the layout moves | `irsdk` | 🔧 | |
| 3 | Closing Settings mid-capture no longer leaves hotkeys switched off | `shell` | 🔧 | |
| 4 | Fuel to finish in timed races: one tested function, `timed_finish` example | `engine` | 🔧 | End-of-race rule needs a timed-race .ibt to confirm |

## Phase 2: robustness

| # | Fix | Group | Status | Notes |
|---|-----|-------|--------|-------|
| 5 | Bounds checks on iRacing's shared memory | `irsdk` | 🔧 | |
| 6 | Lap file writes moved off the telemetry thread | `laps` | 🔧 | |
| 7 | Repeated identical official lap times recognised | `laps` | 🔧 | |
| 8 | Corrupt config kept aside with a notice; reference saved atomically | `shell` | 🔧 | |
| 9 | Demo race keeps real time | `engine` | 🔧 | |

## Phase 3: small fixes and tidying

| # | Fix | Group | Status | Notes |
|---|-----|-------|--------|-------|
| 10 | Unknown tyre compound shows nothing | `engine` | 🔧 | |
| 11 | Identical controllers told apart | `shell` | 🔧 | |
| 12 | Saved and imported layouts checked | `frontend` | 🔧 | |
| 13 | Delta graph's previous lap cleared on a new session | `frontend` | 🔧 | |
| 14 | Duplicate comment, Flags hook warning, eslint config | `frontend` | 🔧 | |

## Release

| Step | Status |
|------|--------|
| Merge every group into `development` | ⏳ |
| Full check: tsc, eslint, Rust tests, clippy, shell type-check | ⏳ |
| README, ARCHITECTURE and project doc updated | ⏳ |
| Version 0.4.1, merged to `main`, pushed | ⏳ |
| Tag `v0.4.1` and Windows build (on your PC) | ⏳ |

## Needs checking on Windows

- Close Settings while a Controls box is waiting: hotkeys and wheel buttons still work.
- Restart iRacing in a different car: telemetry reads correctly.
- Two identical button boxes: each button binds separately.
- A timed race's fuel to finish (once the end-of-race rule is confirmed from an .ibt).

## Log

- 2026-10-31: plan agreed; six groups started in parallel.
