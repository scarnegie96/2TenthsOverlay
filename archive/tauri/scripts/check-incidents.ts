// Checks the next incident limit after penalties. Run it like check-config.ts:
//
//   npx tsc --outDir node_modules/.cache/check --module commonjs --moduleResolution node --target ES2021 --strict --skipLibCheck scripts/check-incidents.ts
//   echo '{"type":"commonjs"}' > node_modules/.cache/check/package.json
//   node node_modules/.cache/check/scripts/check-incidents.js

import { incidentTarget } from "../src/lib/incidents";

const cases: [number, number | null, number, number | null, number][] = [
  // count, limit, step -> next, passed
  [0, null, 4, null, 0],
  [12, 17, 4, 17, 0],
  [16, 17, 4, 17, 0],
  [17, 17, 4, 21, 1],
  [20, 17, 4, 21, 1],
  [21, 17, 4, 25, 2],
  [26, 17, 4, 29, 3],
  [17, 17, 8, 25, 1],
  [19, 17, 0, 17, 1],
];
for (const [count, limit, step, next, passed] of cases) {
  const t = incidentTarget(count, limit, step);
  if (t.next !== next || t.passed !== passed) throw new Error(`${count}x, limit ${limit}, step ${step}: got ${JSON.stringify(t)}`);
}
console.log(`incident checks passed (${cases.length})`);
