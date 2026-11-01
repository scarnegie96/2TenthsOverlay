// Checks what the Races page works out from a race report. Run it like check-config.ts:
//
//   npx tsc --outDir node_modules/.cache/check --module commonjs --moduleResolution node --target ES2021 --strict --skipLibCheck --resolveJsonModule scripts/check-report.ts
//   echo '{"type":"commonjs"}' > node_modules/.cache/check/package.json
//   node node_modules/.cache/check/scripts/check-report.js

import { gapsByLap, lapTimes, orderByLap, positionsByLap, stats, type ReportCar } from "../src/lib/report";

let count = 0;
function check(ok: boolean, what: string) {
  count++;
  if (!ok) throw new Error(`report check failed: ${what}`);
}
const car = (carIdx: number, crossings: [number, number][]) =>
  ({ carIdx, crossings: crossings.map(([lap, t]) => ({ lap, t })) }) as unknown as ReportCar;

// A leads; B passes A on lap 3; C starts recording at lap 2 and retires after lap 3.
const a = car(0, [[1, 100], [2, 190], [3, 282], [4, 372]]);
const b = car(1, [[1, 101], [2, 191.5], [3, 281], [4, 370]]);
const c = car(2, [[2, 200], [3, 295]]);

const t = lapTimes(a);
check(!t.has(1) && t.get(2) === 90 && t.get(4) === 90, "lap times only where both ends were recorded");
check(!lapTimes(c).has(2) && lapTimes(c).get(3) === 95, "no lap time for the first recorded crossing");
check(JSON.stringify(orderByLap([a, b, c]).get(3)) === "[1,0,2]", "order across the line");
const pos = positionsByLap([a, b, c]);
check(pos.get(0)!.get(2) === 1 && pos.get(0)!.get(3) === 2 && pos.get(1)!.get(3) === 1, "positions change at the pass");
check(!pos.get(2)!.has(4), "a retired car has no position after its last lap");
const gaps = gapsByLap([a, b, c]);
check(gaps.get(0)!.get(1) === 0 && gaps.get(1)!.get(1) === 1 && gaps.get(0)!.get(3) === 1 && gaps.get(2)!.get(3) === 14, "gaps at the line");
const s = stats([92, 90, 95, 91]);
check(s.best === 90 && s.median === 91.5 && s.laps === 4, "best and median");
check(stats([]).best === null, "no laps");
console.log(`report checks passed (${count})`);
