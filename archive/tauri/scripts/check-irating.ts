// Checks the iRating estimate against the `irating` crate's own test (a
// 28-driver race with one non-starter), which follows the community "iRacing
// SOF iRating Calculator" spreadsheet. Run it like check-config.ts:
//
//   npx tsc --outDir node_modules/.cache/check --module commonjs --moduleResolution node --target ES2021 --strict --skipLibCheck scripts/check-irating.ts
//   echo '{"type":"commonjs"}' > node_modules/.cache/check/package.json
//   node node_modules/.cache/check/scripts/check-irating.js

import { iratingChanges } from "../src/lib/irating";

const field: [number, number, boolean, number][] = [
  // iRating, started, expected change (from the crate's snapshot)
  [1, 7526, true, 17.63672], [2, 5982, true, 25.833923], [3, 5463, true, 25.432884], [4, 4279, true, 37.92791],
  [5, 4137, true, 33.394478], [6, 4044, true, 27.948332], [7, 3891, true, 23.814116], [8, 3612, true, 22.626814],
  [9, 3147, true, 26.485985], [10, 2823, true, 27.702335], [11, 2715, true, 23.36419], [12, 2603, true, 19.21653],
  [13, 2512, true, 14.53251], [14, 2352, false, 10.437519], [15, 2227, true, 8.5288105], [16, 2195, true, 2.2037997],
  [17, 2166, true, -4.2093577], [18, 2089, true, -9.06982], [19, 1773, true, -5.7882223], [20, 1772, true, -13.086736],
  [21, 1752, true, -19.722021], [22, 1748, true, -26.915356], [23, 1705, true, -32.73568], [24, 1662, true, -38.54108],
  [25, 1622, true, -44.439545], [26, 1537, true, -48.679874], [27, 1464, true, -53.308353], [28, 1203, true, -50.590836],
].map(([rank, ir, started, change]) => [rank as number, ir as number, started as boolean, change as number]);

const changes = iratingChanges(field.map(([rank, irating, started]) => ({ carIdx: rank, irating, rank, started })));
let count = 0;
for (const [rank, , , want] of field) {
  const got = changes.get(rank);
  // The crate works in f32, so allow a little rounding.
  if (got == null || Math.abs(got - want) > 0.01) throw new Error(`P${rank}: got ${got}, want ${want}`);
  count++;
}
// Nobody to race: no estimate.
if (iratingChanges([{ carIdx: 0, irating: 2000, rank: 1, started: true }]).size !== 0) throw new Error("single driver");
console.log(`iRating checks passed (${count + 1})`);
