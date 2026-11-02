// Checks auto fuel to finish: what it sends for a given pit menu and plan.
// Run it like check-config.ts:
//
//   npx tsc --outDir node_modules/.cache/check --module commonjs --moduleResolution node --target ES2021 --strict --skipLibCheck scripts/check-pit.ts
//   echo '{"type":"commonjs"}' > node_modules/.cache/check/package.json
//   node node_modules/.cache/check/scripts/check-pit.js

import { autoFuelCommand } from "../src/lib/pit";
import type { PitPayload } from "../src/lib/types";

const menu = (fuel: boolean, fuelAmount: number) => ({ fuel, fuelAmount }) as PitPayload;
const cases: [PitPayload, number | null, string][] = [
  [menu(false, 0), null, "null"], // no plan yet: leave it
  [menu(false, 0), 28, '{"kind":"fuel","litres":28}'], // tick it with the amount
  [menu(true, 27.6), 28, "null"], // within a litre: leave it
  [menu(true, 20), 28, '{"kind":"fuel","litres":28}'], // a litre or more off: update
  [menu(true, 35), 28, '{"kind":"fuel","litres":28}'], // too much: bring it down
  [menu(true, 10), 0, '{"kind":"clearFuel"}'], // enough already: untick
  [menu(false, 10), 0, "null"], // already unticked
];
for (const [pit, need, want] of cases) {
  const got = JSON.stringify(autoFuelCommand(pit, need));
  if (got !== want) throw new Error(`fuel ${pit.fuel} ${pit.fuelAmount}, need ${need}: got ${got}, want ${want}`);
}
// More than a tankful needed (another stop to come): set a tankful, then leave it.
const tank = [
  [JSON.stringify(autoFuelCommand(menu(false, 0), 140, 110.4)), '{"kind":"fuel","litres":110}'],
  [JSON.stringify(autoFuelCommand(menu(true, 110), 140, 110.4)), "null"],
];
for (const [got, want] of tank) if (got !== want) throw new Error(`tankful: got ${got}, want ${want}`);
console.log(`pit checks passed (${cases.length + tank.length})`);
