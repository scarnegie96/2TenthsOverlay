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

/** iRacing's SoF formula, for a subset of the field. */
export function strengthOfField(iratings: number[]): number | null {
  const irs = iratings.filter((ir) => ir > 0);
  if (!irs.length) return null;
  const br = 1600 / Math.LN2;
  const sum = irs.reduce((acc, ir) => acc + Math.exp(-ir / br), 0);
  return Math.round(br * Math.log(irs.length / sum));
}
