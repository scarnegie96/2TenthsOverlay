// Small building blocks shared by the timing tables.

import { api } from "../lib/bridge";
import { irating, lapTime } from "../lib/format";
import { useLive } from "../lib/store";
import type { CarStatus, DriverCell, LapFlag, Rating } from "../lib/types";

/** The position cell; the F1 theme boxes places outside the top ten in red. */
export const posClass = (pos: number) => (pos > 10 ? "pos is-outside-ten" : "pos");

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

/**
 * iRating as a light badge. With `change` (the estimated gain or loss if the
 * race finished now), the badge carries it too: "2.8k" then "+37" in green or
 * "−21" in red, as one pill.
 */
export function IRatingBadge({ value, change }: { value: number; change?: number }) {
  if (!value) return <span className="ir" />;
  if (change == null) return <span className="ir">{irating(value)}</span>;
  const v = Math.round(change);
  return (
    <span className="ir has-change" title={`Estimated iRating change if the race finished now: ${v > 0 ? "+" : ""}${v}`}>
      <span className="ir-value">{irating(value)}</span>
      <span className={`ir-delta ${v > 0 ? "is-up" : v < 0 ? "is-down" : "is-same"}`}>{v > 0 ? `+${v}` : v < 0 ? `−${-v}` : "0"}</span>
    </span>
  );
}

/** Last lap colored like a timing screen: purple = class fastest, green = personal best. */
export function LastLap({ time, flag }: { time: number | null; flag: LapFlag }) {
  return <span className={`time lap-${flag}`}>{lapTime(time)}</span>;
}

export function BestLap({ time, fastest }: { time: number | null; fastest: boolean }) {
  return <span className={`time ${fastest ? "lap-fastest" : "lap-best"}`}>{lapTime(time)}</span>;
}

/** A thumb, up or (turned over) down. */
export function Thumb({ down = false, size = 14 }: { down?: boolean; size?: number }) {
  return (
    <svg className="thumb" width={size} height={size} viewBox="0 0 24 24" aria-hidden style={down ? { transform: "scaleY(-1)" } : undefined}>
      <path d="M2 21h4V9H2v12Zm20-11a2 2 0 0 0-2-2h-6.3l1-4.6v-.3c0-.4-.2-.8-.4-1.1L13.2 1 6.6 7.6C6.2 8 6 8.5 6 9v10a2 2 0 0 0 2 2h9c.8 0 1.5-.5 1.8-1.2l3-7.1c.1-.2.2-.5.2-.7v-2Z" />
    </svg>
  );
}

/**
 * A small thumb next to a driver you've rated before: green for up, red for
 * down. Nothing for drivers you haven't.
 */
export function RatingMark({ userId }: { userId: number }) {
  const r = useLive((s) => (userId > 0 ? s.driverRatings[String(userId)] : undefined));
  if (!r) return null;
  const when = new Date(r.at * 1000).toLocaleDateString([], { dateStyle: "medium" });
  return (
    <span className={`rating-mark is-${r.rating}`} title={`You gave ${r.name} a thumbs ${r.rating} (${when})`}>
      <Thumb down={r.rating === "down"} size={11} />
    </span>
  );
}

/**
 * Thumbs up and down for a driver, after you've finished. White until picked,
 * then green or red; picking the same one again clears it.
 */
export function RateButtons({ driver }: { driver: DriverCell }) {
  const current = useLive((s) => s.driverRatings[String(driver.userId)]?.rating ?? null);
  const pick = (r: Rating) => void api.setDriverRating(driver.userId, driver.name, current === r ? null : r);
  return (
    <span className="rate" data-clickable>
      <button className={`rate-btn is-up ${current === "up" ? "is-on" : ""}`} aria-pressed={current === "up"}
        title={`Thumbs up for ${driver.name}`} onClick={() => pick("up")}>
        <Thumb />
      </button>
      <button className={`rate-btn is-down ${current === "down" ? "is-on" : ""}`} aria-pressed={current === "down"}
        title={`Thumbs down for ${driver.name}`} onClick={() => pick("down")}>
        <Thumb down />
      </button>
    </span>
  );
}
