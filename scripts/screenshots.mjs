// Regenerates the README screenshots from the browser preview's demo race.
//
//   npm run dev                      (in another terminal)
//   node scripts/screenshots.mjs     (Windows: uses Edge or Chrome, headless)
//
// Each widget is drawn on its own with overlay.html?shot=<id>. A first pass
// reads its size from <body data-shot-size>, the second takes the picture at 2x.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const BASE = process.env.TENTHS_URL ?? "http://localhost:1420";
const OUT = resolve("docs");
const BROWSERS = [
  process.env.BROWSER,
  "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
  "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
  "C:/Program Files/Google/Chrome/Application/chrome.exe",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
].filter(Boolean);
const browser = BROWSERS.find((b) => existsSync(b));
if (!browser) throw new Error("No Edge or Chrome found. Set BROWSER to one.");

/** Widget id -> extra URL parameters. */
const WIDGETS = {
  relative: {},
  // A car in the pit lane, so the pit symbol shows, and long enough for the rejoin line.
  standings: { params: "warmup=490&pits" },
  session: {},
  delta: {},
  laplog: {},
  coach: { params: "reference" },
  inputs: { params: "reference" },
  dynamics: {},
  instruments: {},
  fuel: {},
  radar: {},
  tires: {},
  pitstop: {},
  flags: { params: "flag=16384" },
  weather: {},
  // A longer race, so the lines have a few laps of history.
  racepace: { params: "warmup=560" },
  stints: { params: "warmup=560" },
};

const profile = mkdtempSync(join(tmpdir(), "tenths-shots-"));
const run = (args) =>
  execFileSync(browser, ["--headless=new", "--disable-gpu", "--no-first-run", "--hide-scrollbars", `--user-data-dir=${profile}`, ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });

function shot(url, file, size, wait, scale = 2) {
  run([`--virtual-time-budget=${wait}`, `--force-device-scale-factor=${scale}`, `--window-size=${size}`, `--screenshot=${file}`, url]);
  console.log("wrote", file);
}

try {
  mkdirSync(join(OUT, "widgets"), { recursive: true });
  mkdirSync(join(OUT, "settings"), { recursive: true });
  // The demo races this long before the picture (a lap is about 92 s), so graphs and traces are full.
  const race = "warmup=150";
  const wait = 5000;
  for (const [id, { params = "" }] of Object.entries(WIDGETS)) {
    // A widget's own warmup replaces the default one.
    const url = `${BASE}/overlay.html?shot=${id}${params.includes("warmup") ? "" : `&${race}`}${params ? `&${params}` : ""}`;
    // The first launch of a fresh profile sometimes returns before the page has run, so try again.
    let size = null;
    for (let i = 0; i < 3 && !size; i++) size = /data-shot-size="(\d+),(\d+)"/.exec(run([`--virtual-time-budget=${wait}`, "--dump-dom", url]));
    if (!size) {
      console.warn("no size for", id, "- is the dev server running?");
      continue;
    }
    shot(url, join(OUT, "widgets", `${id}.png`), `${size[1]},${size[2]}`, wait);
  }
  // The settings window, a page at a time.
  for (const pane of ["general", "layouts", "appearance", "controls", "pitstop"]) {
    shot(`${BASE}/settings.html?pane=${pane}&sample`, join(OUT, "settings", `${pane}.png`), "1100,760", wait, 1);
  }
  // The overviews: every widget on (?all lays them out for a 2560x1440 screen),
  // with a reference lap loaded so the Coach and the reference inputs show.
  const all = "clean&all&reference&pits&flag=16384&warmup=490";
  shot(`${BASE}/overlay.html?${all}`, join(OUT, "screenshot.png"), "2560,1440", wait, 1);
  shot(`${BASE}/overlay.html?${all}&theme=light`, join(OUT, "screenshot-light.png"), "2560,1440", wait, 1);
  shot(`${BASE}/overlay.html?${all}&theme=f1`, join(OUT, "screenshot-boxbox.png"), "2560,1440", wait, 1);
} finally {
  rmSync(profile, { recursive: true, force: true });
}
