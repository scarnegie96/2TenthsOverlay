// Records how the delta to each comparison lap evolves around the lap, and
// what each sector gained or lost. Fed from the 60 Hz tick, outside React,
// so nothing is lost when a widget re-renders or is moved. Starts afresh in a
// new session, or when session time goes backwards.

import { useLive } from "./store";
import type { DeltaMode, Deltas, TickPayload } from "./types";

/** Resolution of the delta graph: points per lap. */
export const BINS = 320;

export const DELTA_MODES: DeltaMode[] = ["best", "optimal", "sessionBest", "sessionOptimal", "last", "reference"];

export interface ModeHistory {
  /** Delta at each point of the current lap (NaN where not driven yet). */
  current: Float32Array;
  /** The same for the previous lap. */
  previous: Float32Array;
  /** Time gained (−) or lost (+) per sector this lap; null if not finished. */
  sectors: (number | null)[];
  /** The previous lap's sector results. */
  previousSectors: (number | null)[];
  /** Delta when the current sector began. */
  sectorStart: number | null;
  /** Latest delta. */
  now: number | null;
}

function emptyMode(sectorCount: number): ModeHistory {
  return {
    current: new Float32Array(BINS).fill(NaN),
    previous: new Float32Array(BINS).fill(NaN),
    sectors: new Array(sectorCount).fill(null),
    previousSectors: new Array(sectorCount).fill(null),
    // Unknown until the next sector starts, as we may join mid-lap.
    sectorStart: null,
    now: null,
  };
}

class DeltaHistory {
  /** Sector start points, first is always 0. */
  sectors: number[] = [0];
  modes = {} as Record<DeltaMode, ModeHistory>;
  currentSector = 0;
  pct = 0;
  private lap: number | null = null;
  private session: string | null = null;
  private sessionTime = 0;

  constructor() {
    this.reset();
  }

  /** Forgets both laps and all sector results. */
  reset() {
    for (const m of DELTA_MODES) this.modes[m] = emptyMode(this.sectors.length);
    this.currentSector = 0;
    this.pct = 0;
    this.lap = null;
  }

  /** Changing the sector layout clears the sector splits (the graph is kept). */
  setSectors(starts: number[]) {
    const next = [0, ...starts.filter((s) => s > 0.001 && s < 0.999)].sort((a, b) => a - b);
    if (next.length === this.sectors.length && next.every((v, i) => Math.abs(v - this.sectors[i]) < 1e-6)) return;
    this.sectors = next;
    for (const m of DELTA_MODES) {
      const h = this.modes[m];
      h.sectors = new Array(next.length).fill(null);
      h.previousSectors = new Array(next.length).fill(null);
      h.sectorStart = null;
    }
    this.currentSector = this.sectorAt(this.pct);
  }

  sectorAt(pct: number) {
    let k = 0;
    while (k + 1 < this.sectors.length && pct >= this.sectors[k + 1]) k++;
    return k;
  }

  /**
   * `session` names the session (null while it isn't known yet). A new one,
   * or session time going backwards, starts afresh.
   */
  push(t: TickPayload, session: string | null) {
    const changed = session !== null && this.session !== null && session !== this.session;
    if (changed || t.sessionTime < this.sessionTime - 1) this.reset();
    if (session !== null) this.session = session;
    this.sessionTime = t.sessionTime;
    if (!t.isOnTrack || t.lapDistPct < 0) return;
    const pct = Math.min(0.99999, Math.max(0, t.lapDistPct));

    if (this.lap !== null) {
      // Position wrapping from ~1 back to ~0 means we crossed the line.
      if (pct < this.pct - 0.5) this.newLap();
      // Jumping backwards otherwise is a reset, tow or new session.
      else if (pct < this.pct - 0.02 || t.lap < this.lap) this.clearLap();
    }
    this.lap = t.lap;

    const sector = this.sectorAt(pct);
    if (sector !== this.currentSector) {
      for (const m of DELTA_MODES) {
        const h = this.modes[m];
        const d = t.deltas[m];
        if (sector > this.currentSector && h.sectorStart != null && d != null) {
          h.sectors[this.currentSector] = d - h.sectorStart;
        }
        h.sectorStart = d;
      }
      this.currentSector = sector;
    }

    const bin = Math.min(BINS - 1, Math.floor(pct * BINS));
    for (const m of DELTA_MODES) {
      const d = t.deltas[m];
      const h = this.modes[m];
      h.now = d;
      if (d == null) continue;
      if (h.sectorStart == null && sector === 0 && pct < 0.01) h.sectorStart = 0;
      h.current[bin] = d;
    }
    this.pct = pct;
  }

  private newLap() {
    const last = this.sectors.length - 1;
    for (const m of DELTA_MODES) {
      const h = this.modes[m];
      if (this.currentSector === last && h.sectorStart != null && h.now != null) {
        h.sectors[last] = h.now - h.sectorStart;
      }
      h.previous = h.current;
      h.current = new Float32Array(BINS).fill(NaN);
      h.previousSectors = h.sectors;
      h.sectors = new Array(this.sectors.length).fill(null);
      h.sectorStart = 0; // every delta restarts from zero at the line
    }
    this.currentSector = 0;
    this.pct = 0;
  }

  private clearLap() {
    for (const m of DELTA_MODES) {
      const h = this.modes[m];
      h.current.fill(NaN);
      h.sectors.fill(null);
      h.sectorStart = null;
    }
  }

  /** Live gain/loss in the sector being driven now. */
  liveSector(mode: DeltaMode): number | null {
    const h = this.modes[mode];
    return h.now != null && h.sectorStart != null ? h.now - h.sectorStart : null;
  }

  /** The most recently completed sector's result (wrapping to last lap). */
  lastSector(mode: DeltaMode): { index: number; value: number | null } | null {
    const h = this.modes[mode];
    if (this.currentSector > 0) {
      const i = this.currentSector - 1;
      return { index: i, value: h.sectors[i] };
    }
    const i = this.sectors.length - 1;
    return { index: i, value: h.previousSectors[i] };
  }
}

export const deltaHistory = new DeltaHistory();

// Track every tick from the moment the overlay loads.
useLive.subscribe((s, prev) => {
  if (!s.tick || s.tick === prev.tick) return;
  // Session identity: the track plus the session type, as for the race history.
  deltaHistory.push(s.tick, s.session ? `${s.session.trackName}|${s.session.sessionType}` : null);
});

export type { Deltas };
