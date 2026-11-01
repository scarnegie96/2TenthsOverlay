// Saved race reports (crates/telemetry/src/report.rs) and what the Races page
// works out from them. Everything here comes straight from recorded data:
// lap times are the time between two recorded line crossings, the order on a
// lap is the order cars crossed the line, and a gap is the difference between
// two crossing times. Where something wasn't recorded (the first lap after
// Tenths started, a car that retired), it's left out, not estimated.

import type { LapRecord, Stint } from "./types";

export interface ReportClass {
  classId: number;
  name: string;
  color: string;
  cars: number;
  strengthOfField: number | null;
}

export interface Crossing {
  lap: number;
  /** Session seconds. */
  t: number;
}

export interface ReportStop {
  lap: number;
  laneS: number;
  stallS: number;
}

export interface ReportCar {
  carIdx: number;
  userId: number;
  name: string;
  team: string;
  number: string;
  classId: number;
  irating: number;
  license: string;
  isPlayer: boolean;
  startPosition: number | null;
  finishPosition: number;
  finishClassPosition: number;
  lapsCompleted: number;
  finished: boolean;
  out: boolean;
  crossings: Crossing[];
  stints: Stint[];
  stops: number;
  pitStops: ReportStop[];
}

export interface RaceReport {
  version: number;
  id: string;
  startedAt: number;
  savedAt: number;
  track: string;
  trackConfig: string;
  trackLengthKm: number;
  car: string;
  sessionType: string;
  official: boolean;
  teamRacing: boolean;
  totalLaps: number | null;
  playerCarIdx: number;
  playerClassId: number;
  finished: boolean;
  classes: ReportClass[];
  tireCompounds: string[];
  cars: ReportCar[];
  playerLaps: LapRecord[];
  incidents: number;
  incidentLimit: number | null;
}

export interface ReportSummary {
  id: string;
  startedAt: number;
  track: string;
  trackConfig: string;
  car: string;
  sessionType: string;
  official: boolean;
  finished: boolean;
  classPosition: number;
  classCars: number;
  laps: number;
}

/** Lap number -> seconds, for laps with both ends recorded. */
export function lapTimes(car: ReportCar): Map<number, number> {
  const out = new Map<number, number>();
  const at = new Map(car.crossings.map((c) => [c.lap, c.t]));
  for (const c of car.crossings) {
    const prev = at.get(c.lap - 1);
    if (prev != null && c.t > prev) out.set(c.lap, c.t - prev);
  }
  return out;
}

/**
 * For each lap, the cars in the order they crossed the line to complete it.
 * Lap -> car indices, first first.
 */
export function orderByLap(cars: ReportCar[]): Map<number, number[]> {
  const byLap = new Map<number, { car: number; t: number }[]>();
  for (const car of cars) {
    for (const c of car.crossings) {
      let list = byLap.get(c.lap);
      if (!list) byLap.set(c.lap, (list = []));
      list.push({ car: car.carIdx, t: c.t });
    }
  }
  const out = new Map<number, number[]>();
  for (const [lap, list] of byLap) out.set(lap, list.sort((a, b) => a.t - b.t).map((x) => x.car));
  return out;
}

/** carIdx -> (lap -> position across the line on that lap, from 1). */
export function positionsByLap(cars: ReportCar[]): Map<number, Map<number, number>> {
  const out = new Map<number, Map<number, number>>(cars.map((c) => [c.carIdx, new Map()]));
  for (const [lap, order] of orderByLap(cars)) order.forEach((car, k) => out.get(car)?.set(lap, k + 1));
  return out;
}

/** carIdx -> (lap -> seconds behind the first of `cars` to complete that lap). */
export function gapsByLap(cars: ReportCar[]): Map<number, Map<number, number>> {
  const first = new Map<number, number>();
  for (const car of cars) for (const c of car.crossings) first.set(c.lap, Math.min(first.get(c.lap) ?? Infinity, c.t));
  return new Map(cars.map((car) => [car.carIdx, new Map(car.crossings.map((c) => [c.lap, c.t - first.get(c.lap)!]))]));
}

export interface LapStats {
  laps: number;
  best: number | null;
  median: number | null;
}

/** Best and median of a set of lap times. */
export function stats(times: number[]): LapStats {
  if (times.length === 0) return { laps: 0, best: null, median: null };
  const s = [...times].sort((a, b) => a - b);
  const mid = s.length >> 1;
  return { laps: s.length, best: s[0], median: s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2 };
}
