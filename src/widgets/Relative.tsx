import type React from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { clock, gap, irating, license } from "../lib/format";
import { useLive } from "../lib/store";
import type { RelativeRow, SessionClock } from "../lib/types";
import { ClassStripe, IRatingBadge, LastLap, StatusBadge, posClass } from "./cells";
import { Empty, Panel } from "./Panel";
import { compoundKind } from "./Stints";
import { incidentTarget } from "../lib/incidents";

export function sessionAside(c: SessionClock | undefined) {
  if (!c) return null;
  if (c.lapsRemain != null && c.totalLaps) {
    const lap = Math.min(c.totalLaps, c.totalLaps - c.lapsRemain + 1);
    return `Lap ${Math.max(1, lap)} of ${c.totalLaps}`;
  }
  if (c.timeRemain != null) return `${clock(c.timeRemain)} left`;
  return null;
}

export function Relative({ config }: { config: WidgetConfig }) {
  const field = useLive((s) => s.field);
  const multiclass = useLive((s) => s.session?.multiclass ?? false);
  const race = /race/i.test(field?.clock.sessionType ?? "");
  const ahead = opt(config, "ahead", 3);
  const behind = opt(config, "behind", 3);
  const showIR = opt(config, "showIRating", true);
  const showLic = opt(config, "showLicense", true);
  const showLast = opt(config, "showLastLap", true);
  const showPit = opt(config, "showPit", true);
  const showInterval = opt(config, "showInterval", true);
  const compounds = useLive((s) => s.session?.tireCompounds ?? []);
  const showCompound = opt<string>(config, "showCompound", "auto");
  // Only worth the space when there's a choice of tyres.
  const showTyre = showCompound === "always" || (showCompound === "auto" && compounds.length > 1);

  const rows = (field?.relative ?? []).filter((r) => showPit || !r.onPitRoad || r.isPlayer);
  const me = rows.findIndex((r) => r.isPlayer);
  // Each car's gap to the car directly in front of it on track (the row
  // above), from the full list so the top visible row gets one too. Gaps are
  // positive ahead of you, so the row above always has the bigger one.
  const intervals = rows.map((r, i) => (i > 0 ? rows[i - 1].gap - r.gap : null));

  // Fixed slot count so the widget never changes height.
  const slots: ({ r: RelativeRow; interval: number | null } | null)[] = [];
  for (let k = -ahead; k <= behind; k++) {
    const i = me + k;
    slots.push(me >= 0 && rows[i] ? { r: rows[i], interval: intervals[i] } : null);
  }

  const sof = opt(config, "showSoF", true) ? field?.clock.strengthOfField : null;
  const showInc = opt(config, "showIncidents", true);
  const incStep = opt(config, "incidentStep", 4);
  const cols = [
    "24px 8px 34px minmax(0, 1fr)",
    showTyre && "22px",
    showLic && "auto",
    showIR && "40px",
    showLast && "62px",
    "40px",
    showInterval && "40px",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <Panel
      className="relative"
      title={field?.clock.sessionType || "Relative"}
      aside={
        <>
          {[sessionAside(field?.clock), sof ? `SoF ${irating(sof)}` : null].filter(Boolean).join("   ")}
          {showInc && field && <Incidents count={field.clock.incidents} limit={field.clock.incidentLimit} step={incStep} />}
        </>
      }
    >
      {me < 0 ? (
        <Empty>Relative appears once you're in the car.</Empty>
      ) : (
        <div className="rows" style={{ "--cols": cols } as React.CSSProperties}>
          {slots.map((s, k) =>
            s ? (
              <Row key={s.r.carIdx} r={s.r} interval={showInterval ? s.interval : undefined} race={race}
                showIR={showIR} showLic={showLic} showLast={showLast} multiclass={multiclass}
                tyre={showTyre ? (s.r.compound >= 0 ? compounds[s.r.compound] ?? "" : "") : null} />
            ) : (
              <div key={`e${k}`} className="row row-empty" />
            ),
          )}
        </div>
      )}
    </Panel>
  );
}

/**
 * Your incidents and the next limit: "4x / 17x", then "18x / 21x" once the
 * first limit's penalty has been given. Amber from three quarters of the
 * first limit or once past it, red within 4x of the next one.
 */
function Incidents({ count, limit, step }: { count: number; limit: number | null; step: number }) {
  const { next, passed } = incidentTarget(count, limit, step);
  const tone = next == null ? "" : next - count <= 4 ? "is-danger" : passed > 0 || count >= next * 0.75 ? "is-warn" : "";
  const title =
    next == null
      ? "Your incidents (no limit this session)"
      : passed > 0
        ? `Your incidents. Past the ${limit}x limit; the next penalty is at ${next}x`
        : `Your incidents, out of a limit of ${next}x`;
  return (
    <span className={`rel-inc ${tone}`} title={title}>
      {count}x{next != null ? ` / ${next}x` : ""}
    </span>
  );
}

/** Within this many seconds of the car ahead counts as a close fight. */
const CLOSE_S = 1.0;

function Row({ r, interval, race, showIR, showLic, showLast, multiclass, tyre }: {
  r: RelativeRow;
  /** The compound's name ("" when unknown); null when the column is off. */
  tyre: string | null;
  race: boolean;
  /** Seconds to the car in front; null for the front row; undefined when the column is off. */
  interval: number | null | undefined;
  showIR: boolean; showLic: boolean; showLast: boolean; multiclass: boolean;
}) {
  const lic = license(r.license);
  const status = r.isPlayer ? "is-player" : r.lapStatus > 0 ? "is-ahead-lap" : r.lapStatus < 0 ? "is-behind-lap" : "";
  return (
    <div className={`row ${status}`} style={{ "--row-class": r.classColor } as React.CSSProperties}>
      <span className={posClass(r.classPosition)} title={multiclass ? `P${r.position} overall` : undefined}>{r.classPosition || "–"}</span>
      <ClassStripe color={r.classColor} />
      <span className="num">{r.number}</span>
      <span className="name">
        <span className="name-text">{r.name}</span>
        <StatusBadge status={r.status} race={race} />
      </span>
      {tyre !== null && (
        <span className="rel-tyre">
          {tyre && <i className={`tyre-badge tyre-${compoundKind(tyre)}`} title={tyre}>{tyre[0].toUpperCase()}</i>}
        </span>
      )}
      {showLic && (
        <span className="lic" style={{ borderLeftColor: r.licenseColor || undefined }}>
          {lic.letter}
          <small>{lic.sr}</small>
        </span>
      )}
      {showIR && <IRatingBadge value={r.irating} />}
      {showLast && <LastLap time={r.lastLap} flag={r.lastFlag} />}
      <span className="gap">{r.isPlayer ? "" : gap(r.gap)}</span>
      {interval !== undefined && (
        <span
          className={`interval ${interval != null && interval < CLOSE_S ? "is-close" : ""}`}
          title={interval == null ? undefined : `${interval.toFixed(2)} s to the car ahead`}
        >
          {interval == null ? "" : gap(interval)}
        </span>
      )}
    </div>
  );
}
