// Weather maths for the Weather widget, kept apart from the drawing so it can
// be reasoned about (and checked) on its own.
//
// Angles are compass bearings in radians, clockwise from north. iRacing's
// WindDir is the direction the wind blows *from* (like a weather report), at
// the start/finish line. The engine turns YawNorth into the same kind of
// bearing for the car (`heading`).

const TAU = Math.PI * 2;

export const SKIES = ["Clear", "Partly cloudy", "Mostly cloudy", "Overcast"];
/** irsdk_TrackWetness, 1..7 (0 = unknown). */
export const WETNESS = ["", "Dry", "Mostly dry", "Very lightly wet", "Lightly wet", "Moderately wet", "Very wet", "Extremely wet"];
const COMPASS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

export const wrap = (a: number) => ((a % TAU) + TAU) % TAU;

/** "SW" for a bearing. */
export function compassPoint(rad: number): string {
  return COMPASS[Math.round((wrap(rad) * 180) / Math.PI / 45) % 8];
}

/** Where the wind comes from, seen from the driver's seat: 0 = dead ahead, π/2 = from the right. */
export function windFromCar(windDir: number, heading: number): number {
  return wrap(windDir - heading);
}

/**
 * The wind split along and across the car. `head` is positive for a
 * headwind, negative for a tailwind; `cross` is positive when it comes from
 * the right, negative from the left. Same units as `speed`.
 */
export function windComponents(speed: number, windDir: number, heading: number): { head: number; cross: number } {
  const rel = windFromCar(windDir, heading);
  return { head: speed * Math.cos(rel), cross: speed * Math.sin(rel) };
}

/** A reading worth watching for change, sampled now and then. */
export interface Sample {
  /** ms of session time */
  at: number;
  value: number;
}

/**
 * Keeps one sample every `everyMs`, for `keepMs`. Returns the new list (the
 * same one if nothing changed), so it can live in a ref.
 */
export function addSample(list: Sample[], at: number, value: number, everyMs = 15_000, keepMs = 4 * 60_000): Sample[] {
  const last = list[list.length - 1];
  if (last && at - last.at < everyMs && at >= last.at) return list;
  // Time went backwards (new session, replay): start again.
  const kept = last && at < last.at ? [] : list.filter((s) => at - s.at <= keepMs);
  return [...kept, { at, value }];
}

/**
 * Which way a reading has moved over the last few minutes: +1 rising, -1
 * falling, 0 steady or not enough history. Compares the latest sample with
 * the oldest one inside `windowMs`, which needs to span at least `minSpanMs`,
 * and the change must be bigger than `threshold`.
 */
export function trend(list: Sample[], threshold: number, windowMs = 3 * 60_000, minSpanMs = 90_000): -1 | 0 | 1 {
  const last = list[list.length - 1];
  if (!last) return 0;
  const first = list.find((s) => last.at - s.at <= windowMs);
  if (!first || last.at - first.at < minSpanMs) return 0;
  const d = last.value - first.value;
  return d > threshold ? 1 : d < -threshold ? -1 : 0;
}
