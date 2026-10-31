// Checks that normalizeConfig turns broken or hostile saved layouts into
// something every widget can draw. No test runner is set up, so run it with
// tsc and node, from the repo root (node_modules/.cache is git-ignored):
//
//   npx tsc --outDir node_modules/.cache/check --module commonjs --moduleResolution node --target ES2021 --strict --skipLibCheck scripts/check-config.ts
//   echo '{"type":"commonjs"}' > node_modules/.cache/check/package.json
//   node node_modules/.cache/check/scripts/check-config.js
//
// It prints "config checks passed" or throws on the first failure.

import { defaultConfig, normalizeConfig, OPACITY_MAX, OPACITY_MIN, SCALE_MAX, SCALE_MIN } from "../src/lib/config";

let count = 0;
function check(ok: boolean, what: string) {
  count++;
  if (!ok) throw new Error(`config check failed: ${what}`);
}
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

const base = defaultConfig();

// Nothing usable at all gives the defaults.
for (const raw of [null, undefined, 7, "x", [], { widgets: "nope", general: 3 }]) {
  check(same(normalizeConfig(raw), base), `defaults for ${JSON.stringify(raw)}`);
}

// A valid saved widget is kept as it is.
{
  const w = { ...base.widgets.delta, enabled: false, x: 40, y: -12, ax: "r", ay: "b", display: "\\\\.\\DISPLAY2", scale: 1.5, opacity: 0.6 };
  const c = normalizeConfig({ widgets: { delta: w } });
  check(same(c.widgets.delta, w), "valid widget kept");
}

// Wrong types fall back to the defaults; out-of-range numbers are clamped.
{
  const c = normalizeConfig({
    widgets: {
      relative: { enabled: "yes", x: "10", y: NaN, scale: "x", opacity: 7, display: 5, options: { ahead: 5, behind: { n: 1 }, showPit: [true], showSoF: false, extra: "kept" } },
      standings: { scale: 0.1, opacity: -1 },
      radar: { scale: 99, opacity: Infinity },
      fuel: "broken",
      tires: { options: [1, 2] },
    },
  });
  const r = c.widgets.relative;
  const d = base.widgets.relative;
  check(r.enabled === d.enabled, "enabled must be boolean");
  check(r.x === d.x && r.y === d.y, "x/y must be finite numbers");
  check(r.scale === d.scale, "scale must be a number");
  check(r.opacity === OPACITY_MAX, "opacity clamped to 1");
  check(r.display === undefined, "display must be a string");
  check(r.options.ahead === 5, "valid option kept");
  check(r.options.behind === d.options.behind, "object option dropped");
  check(r.options.showPit === d.options.showPit, "array option dropped");
  check(r.options.showSoF === false, "boolean option kept");
  check(r.options.extra === "kept", "unknown primitive option kept");
  check(c.widgets.standings.scale === SCALE_MIN && c.widgets.standings.opacity === OPACITY_MIN, "low values clamped");
  check(c.widgets.radar.scale === SCALE_MAX && c.widgets.radar.opacity === base.widgets.radar.opacity, "high scale clamped, infinite opacity dropped");
  check(same(c.widgets.fuel, base.widgets.fuel), "non-object widget gives defaults");
  check(same(c.widgets.tires.options, base.widgets.tires.options), "array options ignored");
}

// Anchors: both halves must be valid, or the offsets go back to the default position.
{
  const c = normalizeConfig({
    widgets: {
      delta: { x: 10, y: 20, ax: "q", ay: "t" },
      coach: { x: 10, y: 20, ax: "l" },
      laplog: { x: 10, y: 20 },
    },
  });
  check(c.widgets.delta.ax === undefined && c.widgets.delta.ay === undefined, "bad anchor dropped");
  check(c.widgets.delta.x === base.widgets.delta.x && c.widgets.delta.y === base.widgets.delta.y, "bad anchor resets offsets");
  check(c.widgets.coach.ax === undefined && c.widgets.coach.x === base.widgets.coach.x, "half an anchor resets offsets");
  check(c.widgets.laplog.x === 10 && c.widgets.laplog.y === 20, "unanchored absolute position kept");
}

// General settings.
{
  const c = normalizeConfig({
    general: {
      demo: "true", units: "furlongs", showWhen: "always", theme: "neon", activeProfile: 4,
      colors: { dark: { ink: "#fff", edge: 3 }, light: "red", neon: { ink: "#000" } },
      controls: { "pit.fuel": { key: null, button: null }, "pit.clear": "x", "overlay.edit": { key: 3, button: null } },
    },
  });
  const g = c.general;
  check(g.demo === base.general.demo && g.units === base.general.units && g.showWhen === base.general.showWhen && g.theme === base.general.theme, "bad enums fall back");
  check(g.activeProfile === null, "activeProfile must be a string");
  check(same(g.colors.dark, { ink: "#fff" }) && same(g.colors.light, {}) && !("neon" in g.colors), "colors are strings per known theme");
  check(same(g.controls["pit.fuel"], { key: null, button: null }), "valid control kept");
  check(same(g.controls["pit.clear"], base.general.controls["pit.clear"]), "bad control falls back");
  check(same(g.controls["overlay.edit"], base.general.controls["overlay.edit"]), "control with a non-object key falls back");

  const ok = normalizeConfig({ general: { units: "imperial", showWhen: "onTrack", theme: "f1", activeProfile: "Race" } }).general;
  check(ok.units === "imperial" && ok.showWhen === "onTrack" && ok.theme === "f1" && ok.activeProfile === "Race", "valid general kept");
}

console.log(`config checks passed (${count})`);
