// Checks the Battle widget's maths. Run it like check-config.ts:
//
//   npx tsc --outDir node_modules/.cache/check --module commonjs --moduleResolution node --target ES2021 --strict --skipLibCheck scripts/check-battle.ts
//   echo '{"type":"commonjs"}' > node_modules/.cache/check/package.json
//   node node_modules/.cache/check/scripts/check-battle.js

import { gapAt, history, intervalAt, lapsBetween, trend } from "../src/lib/battle";
import type { StandingRow } from "../src/lib/types";

let count = 0;
function check(ok: boolean, what: string) {
  count++;
  if (!ok) throw new Error(`battle check failed: ${what}`);
}
const near = (a: number | null | undefined, b: number, eps = 1e-6) => a != null && Math.abs(a - b) < eps;

// Two cars over 10 laps of 100 s, sampled every 2 s: you (to the class leader)
// lose nothing, the car ahead of you is 5 s up and you take 0.3 s a lap off it.
const me: number[] = [];
const ahead: number[] = [];
for (let t = 0; t <= 1000; t += 2) {
  me.push(t, 20);
  ahead.push(t, 20 - (5 - 0.003 * t));
}

check(near(gapAt(me, 500), 20), "flat trace");
check(gapAt(me, -1) === null && gapAt([], 0) === null, "outside the trace");
check(near(gapAt([0, 1, 10, 2], 5), 1.5), "between samples");
check(near(intervalAt(me, ahead, 1000), -2), "ahead by 2 s at the end (negative: in front)");
const t = trend(me, ahead, 1000, 100, 3)!;
check(near(t.closingPerLap, 0.3) && t.laps === 3, `closing 0.3 s a lap, got ${JSON.stringify(t)}`);
check(trend(me, ahead, 50, 100, 3) === null, "no trend without a lap of history");
check(trend(me, ahead, 250, 100, 5)!.laps === 2, "trend over the history there is");
// A jumpy reading doesn't swing the trend much: add ±0.5 s noise at every other sample.
const noisy = ahead.map((v, i) => (i % 2 === 1 ? v + ((i >> 1) % 2 ? 0.5 : -0.5) : v));
check(near(trend(me, noisy, 1000, 100, 3)!.closingPerLap, 0.3, 0.05), "steady through noise");
// Each point is averaged over the third of a lap before it, so ~0.05 s behind the instant value.
const h = history(me, ahead, 1000, 100, 8, 8);
check(h.length === 9 && near(h[0], 4.45, 1e-3) && near(h[8], 2.05, 1e-3), `history ${h}`);

// Laps apart comes from where the two cars are, not from their laps down to the leader.
{
  const row = (lap: number, lapPct: number, lapsDown: number) => ({ lap, lapPct, lapsDown }) as unknown as StandingRow;
  // Leader 1.2 laps ahead of me and 0.9 ahead of the car in front: one lap down vs none, but 0.3 laps apart.
  check(lapsBetween(row(10, 0.6, 0), row(10, 0.3, 1)) === 0, "cars a fraction of a lap apart aren't a lap apart");
  check(lapsBetween(row(11, 0.5, 0), row(10, 0.3, 1)) === 1, "a car a lap and a bit ahead is a lap apart");
  check(lapsBetween(row(10, -1, 0), row(9, 0.2, 2)) === 2, "off track falls back to laps down");
}
console.log(`battle checks passed (${count})`);
