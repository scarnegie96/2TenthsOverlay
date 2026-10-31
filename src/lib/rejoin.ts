// Where you'd rejoin if you pitted now: your gap to the class leader plus the
// session's average pit stop loss for your class, against everyone else's gap.

import type { StandingRow } from "./types";

export interface Rejoin {
  /** Class position you'd come out in. */
  position: number;
  /** The car you'd come out behind, and by how much (s). */
  behind: { row: StandingRow; by: number } | null;
  /** The car you'd come out ahead of, and by how much (s). */
  ahead: { row: StandingRow; by: number } | null;
  /** The stop loss used (s). */
  loss: number;
}

/**
 * `rows` are your class in race order, you included; gaps are seconds behind
 * the class leader. Cars that have left the sim don't count.
 */
export function rejoinAfterStop(rows: StandingRow[], loss: number): Rejoin | null {
  const me = rows.find((r) => r.isPlayer);
  if (!me || me.status === "out") return null;
  const gap = (r: StandingRow) => (r.classPosition === 1 ? 0 : r.gap);
  const mine = gap(me);
  if (mine == null) return null;
  const at = mine + loss;
  let behind: Rejoin["behind"] = null;
  let ahead: Rejoin["ahead"] = null;
  let before = 0;
  for (const r of rows) {
    const g = gap(r);
    if (r.isPlayer || r.status === "out" || g == null) continue;
    if (g <= at) {
      before++;
      if (!behind || g > gap(behind.row)!) behind = { row: r, by: at - g };
    } else if (!ahead || g < gap(ahead.row)!) {
      ahead = { row: r, by: g - at };
    }
  }
  return { position: before + 1, behind, ahead, loss };
}
