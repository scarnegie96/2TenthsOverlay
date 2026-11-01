// The maths behind the Battle widget: the gap to the car ahead or behind in
// your class, and how much it changes lap by lap.
// Gaps come from the Race pace history (raceHistory.ts): every car's gap to
// its class leader over time, so the gap between two cars at any moment is
// the difference of theirs.

/** A car's gap to its class leader over time, as flat pairs: t0, g0, t1, g1… */
export type Trace = number[];

/** The trace's gap at session time `t` (straight line between samples), or null outside it. */
export function gapAt(points: Trace, t: number): number | null {
  const n = points.length / 2;
  if (n === 0 || t < points[0] || t > points[(n - 1) * 2]) return null;
  for (let i = 1; i < n; i++) {
    const t1 = points[i * 2];
    if (t1 < t) continue;
    const t0 = points[(i - 1) * 2];
    const g0 = points[(i - 1) * 2 + 1];
    const g1 = points[i * 2 + 1];
    return t1 === t0 ? g1 : g0 + ((g1 - g0) * (t - t0)) / (t1 - t0);
  }
  return points[(n - 1) * 2 + 1];
}

/**
 * Seconds between two cars at `t`, positive when `other` is behind `me`.
 * Null when either has no sample then.
 */
export function intervalAt(me: Trace, other: Trace, t: number): number | null {
  const a = gapAt(me, t);
  const b = gapAt(other, t);
  return a == null || b == null ? null : b - a;
}

/**
 * The interval averaged over the `window` seconds up to `t`. Timing only
 * updates as cars pass timing lines, so a single reading jumps about; the
 * average over part of a lap doesn't.
 */
export function smoothIntervalAt(me: Trace, other: Trace, t: number, window: number): number | null {
  const steps = 8;
  let sum = 0;
  let n = 0;
  for (let k = 0; k <= steps; k++) {
    const v = intervalAt(me, other, t - (window * k) / steps);
    if (v != null) {
      sum += v;
      n++;
    }
  }
  return n ? sum / n : null;
}

/** The latest moment both traces have a sample for (history is stored every couple of seconds). */
function lastShared(a: Trace, b: Trace, now: number): number {
  const end = Math.min(a[a.length - 2] ?? -Infinity, b[b.length - 2] ?? -Infinity);
  return Math.min(now, end);
}

export interface Trend {
  /** Seconds the gap shrinks by each lap (negative when it grows). */
  closingPerLap: number;
  /** Laps the trend is measured over. */
  laps: number;
}

/**
 * How fast the gap is closing, from its size now and `laps` laps ago (the
 * time of a lap given by `lapTime`), measured on what history there is.
 * Null without a lap's worth of history.
 */
export function trend(me: Trace, other: Trace, now: number, lapTime: number, laps: number): Trend | null {
  if (!(lapTime > 0) || laps <= 0) return null;
  now = lastShared(me, other, now);
  // Each end is averaged over a third of a lap, so one timing jump doesn't swing it.
  const window = lapTime / 3;
  const gapNow = smoothIntervalAt(me, other, now, window);
  if (gapNow == null) return null;
  // Go back as many whole laps as there's history for, up to `laps`.
  const start = Math.max(me[0] ?? Infinity, other[0] ?? Infinity) + window;
  const have = Math.floor((now - start) / lapTime);
  const n = Math.min(laps, have);
  if (n < 1) return null;
  const then = smoothIntervalAt(me, other, now - n * lapTime, window);
  if (then == null) return null;
  return { closingPerLap: (Math.abs(then) - Math.abs(gapNow)) / n, laps: n };
}

/** Closing or opening by less than this (s a lap) counts as holding steady. */
export const STEADY = 0.02;

/**
 * The gap at evenly spaced moments over the last `laps` laps, oldest first,
 * each averaged over a third of a lap, for a sparkline.
 */
export function history(me: Trace, other: Trace, now: number, lapTime: number, laps: number, points = 40): number[] {
  if (!(lapTime > 0)) return [];
  now = lastShared(me, other, now);
  const out: number[] = [];
  for (let k = 0; k <= points; k++) {
    const t = now - lapTime * laps * (1 - k / points);
    const v = smoothIntervalAt(me, other, t, lapTime / 3);
    if (v != null) out.push(Math.abs(v));
  }
  return out;
}
