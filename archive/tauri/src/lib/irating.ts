// Estimated iRating change for each driver if the race finished as it stands.
//
// iRacing doesn't publish its formula. This is the community one that overlays
// and calculators use, from the "iRacing SOF iRating Calculator" spreadsheet,
// ported line for line from the `irating` Rust crate (which follows it). It's
// an Elo-style estimate: usually within a few points of the real change.
//
// Worked out per class: in multi-class races each class is rated on its own.

export interface IRatingEntry {
  carIdx: number;
  /** iRating before the race. */
  irating: number;
  /** Finishing position in the class, from 1. */
  rank: number;
  /** False for drivers who registered but never took the start. */
  started: boolean;
}

const BR1 = 1600 / Math.LN2;

/** The chance a driver rated `a` finishes ahead of one rated `b`. */
function chance(a: number, b: number): number {
  const ea = Math.exp(-a / BR1);
  const eb = Math.exp(-b / BR1);
  return ((1 - ea) * eb) / ((1 - eb) * ea + (1 - ea) * eb);
}

/** carIdx -> estimated change (unrounded). Empty when there's no one to race. */
export function iratingChanges(entries: IRatingEntry[]): Map<number, number> {
  const out = new Map<number, number>();
  const registrations = entries.length;
  const starters = entries.filter((e) => e.started).length;
  const nonStarters = registrations - starters;
  if (registrations < 2 || starters === 0) return out;

  // Each driver's expected number of drivers beaten (the 0.5 is themselves).
  const expected = entries.map((a) => entries.reduce((sum, b) => sum + chance(a.irating, b.irating), 0) - 0.5);

  let sumStarters = 0;
  entries.forEach((e, i) => {
    if (!e.started) return;
    const fudge = ((registrations - nonStarters / 2) / 2 - e.rank) / 100;
    const change = ((registrations - e.rank - expected[i] - fudge) * 200) / starters;
    out.set(e.carIdx, change);
    sumStarters += change;
  });

  if (nonStarters > 0) {
    const sumExpectedNon = entries.reduce((sum, e, i) => sum + (e.started ? 0 : expected[i]), 0);
    entries.forEach((e, i) => {
      if (e.started) return;
      out.set(e.carIdx, ((-sumStarters / nonStarters) * expected[i]) / (sumExpectedNon / nonStarters));
    });
  }
  return out;
}
