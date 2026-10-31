// Small building blocks shared by the timing tables.

import { irating, lapTime } from "../lib/format";
import type { CarStatus, LapFlag } from "../lib/types";

export function ClassStripe({ color }: { color: string }) {
  return <span className="class-bar" style={{ background: color || undefined }} />;
}

/**
 * Pit and out symbols for a timing row: an outlined P in the pit lane, a
 * solid P stopped in the pit box, and OUT for a car that has left the sim
 * (in races that means retired or quit; otherwise it's in the garage).
 */
export function StatusBadge({ status, race }: { status: CarStatus; race: boolean }) {
  switch (status) {
    case "pitLane":
      return <span className="status-badge is-pit" title="In the pit lane">P</span>;
    case "pitStall":
      return <span className="status-badge is-stall" title="In the pit box">P</span>;
    case "out":
      return race
        ? <span className="status-badge is-out" title="Out of the race: retired, quit or towed to the garage">OUT</span>
        : <span className="status-badge is-out" title="In the garage">GAR</span>;
    default:
      return null;
  }
}

export function IRatingBadge({ value }: { value: number }) {
  if (!value) return <span className="ir" />;
  return <span className="ir">{irating(value)}</span>;
}

/** Last lap colored like a timing screen: purple = class fastest, green = personal best. */
export function LastLap({ time, flag }: { time: number | null; flag: LapFlag }) {
  return <span className={`time lap-${flag}`}>{lapTime(time)}</span>;
}

export function BestLap({ time, fastest }: { time: number | null; fastest: boolean }) {
  return <span className={`time ${fastest ? "lap-fastest" : "lap-best"}`}>{lapTime(time)}</span>;
}
