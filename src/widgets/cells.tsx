// Small building blocks shared by the timing tables.

import { irating, lapTime } from "../lib/format";
import type { LapFlag } from "../lib/types";

export function ClassStripe({ color }: { color: string }) {
  return <span className="class-bar" style={{ background: color || undefined }} />;
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
