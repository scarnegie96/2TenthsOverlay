// Records every car's gap to its class leader through a race, for the Race
// pace chart. Fed from the 10 Hz field updates, outside React, so the history
// survives the widget being hidden, moved or redrawn. Starts afresh in a new
// session, or when session time goes backwards.

import { useLive } from "./store";
import type { CarStatus, FieldPayload, StandingRow } from "./types";

/** Seconds of session time between stored points (it doubles as the race goes on). */
const START_EVERY_S = 2;
/** Points kept per car before the history is thinned out. */
const MAX_POINTS = 2400;

export interface CarTrace {
  carIdx: number;
  name: string;
  abbrev: string;
  number: string;
  classId: number;
  classColor: string;
  isPlayer: boolean;
  /** Session time and gap to the class leader (s), as flat pairs: t0, g0, t1, g1… */
  points: number[];
  /** Latest values, drawn as the live end of the line. */
  t: number;
  gap: number;
  position: number;
  lapsDown: number;
  status: CarStatus;
  /** Gaps since the last stored point, averaged into the next one to smooth timing steps. */
  sum: number;
  count: number;
}

export interface LapMark {
  /** Session time the class leader started this lap. */
  t: number;
  lap: number;
}

class RaceHistory {
  cars = new Map<number, CarTrace>();
  /** Per class, when its leader started each lap. */
  laps = new Map<number, LapMark[]>();
  /** Session time of the first point. */
  start: number | null = null;
  now = 0;
  /** Bumped on every change, so a canvas knows when to redraw. */
  version = 0;
  private session = "";
  private every = START_EVERY_S;
  private lastSample = -Infinity;
  private leaderLap = new Map<number, number>();

  reset() {
    this.cars.clear();
    this.laps.clear();
    this.leaderLap.clear();
    this.start = null;
    this.every = START_EVERY_S;
    this.lastSample = -Infinity;
    this.version++;
  }

  push(field: FieldPayload, sessionTime: number, sessionNum: string) {
    const race = /race/i.test(field.clock.sessionType);
    if (sessionNum !== this.session || sessionTime < this.now - 1) {
      this.session = sessionNum;
      this.reset();
    }
    this.now = sessionTime;
    if (!race) return;

    const rows = field.standings.filter((r) => r.position > 0);
    if (rows.length === 0) return;
    if (this.start == null) this.start = sessionTime;

    // Lap lines: when each class leader starts a new lap.
    for (const r of rows) {
      if (r.classPosition !== 1) continue;
      const seen = this.leaderLap.get(r.classId);
      if (seen !== r.lap) {
        this.leaderLap.set(r.classId, r.lap);
        const marks = this.laps.get(r.classId) ?? [];
        // The first lap seen is where the chart starts, not a real lap start.
        if (seen !== undefined) marks.push({ t: sessionTime, lap: r.lap });
        this.laps.set(r.classId, marks);
      }
    }

    const sample = sessionTime - this.lastSample >= this.every;
    if (sample) this.lastSample = sessionTime;
    for (const r of rows) this.update(r, sessionTime, sample);
    this.version++;
  }

  private update(r: StandingRow, t: number, sample: boolean) {
    const gap = r.classPosition === 1 ? 0 : r.gap;
    let c = this.cars.get(r.carIdx);
    if (!c) {
      c = {
        carIdx: r.carIdx, name: r.name, abbrev: r.abbrev, number: r.number, classId: r.classId,
        classColor: r.classColor, isPlayer: r.isPlayer, points: [], t, gap: gap ?? 0,
        position: r.classPosition, lapsDown: r.lapsDown, status: r.status, sum: 0, count: 0,
      };
      this.cars.set(r.carIdx, c);
    }
    c.position = r.classPosition;
    c.lapsDown = r.lapsDown;
    c.status = r.status;
    c.isPlayer = r.isPlayer;
    c.classId = r.classId;
    c.classColor = r.classColor;
    // A car that has left the sim keeps its line up to where it stopped.
    if (gap == null || r.status === "out") return;
    c.t = t;
    c.gap = gap;
    c.sum += gap;
    c.count++;
    if (sample) {
      c.points.push(t, c.sum / c.count);
      c.sum = 0;
      c.count = 0;
      if (c.points.length / 2 > MAX_POINTS) this.thin();
    }
  }

  /** Halves every car's points and the sample rate, so a long race fits. */
  private thin() {
    for (const c of this.cars.values()) {
      const kept: number[] = [];
      for (let i = 0; i < c.points.length; i += 4) kept.push(c.points[i], c.points[i + 1]);
      c.points = kept;
    }
    this.every *= 2;
  }
}

export const raceHistory = new RaceHistory();

useLive.subscribe((s, prev) => {
  if (!s.field || s.field === prev.field) return;
  const t = s.tick?.sessionTime;
  if (t == null) return;
  // Session identity: the session type plus the track, so a new session resets.
  raceHistory.push(s.field, t, `${s.session?.trackName ?? ""}|${s.field.clock.sessionType}`);
});
