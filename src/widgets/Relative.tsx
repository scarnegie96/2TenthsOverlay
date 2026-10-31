import type React from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { clock, gap, irating, license } from "../lib/format";
import { useLive } from "../lib/store";
import type { RelativeRow, SessionClock } from "../lib/types";
import { ClassStripe, IRatingBadge, LastLap } from "./cells";
import { Empty, Panel } from "./Panel";

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
  const ahead = opt(config, "ahead", 3);
  const behind = opt(config, "behind", 3);
  const showIR = opt(config, "showIRating", true);
  const showLic = opt(config, "showLicense", true);
  const showLast = opt(config, "showLastLap", true);
  const showPit = opt(config, "showPit", true);
  const showInterval = opt(config, "showInterval", true);

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
  const cols = [
    "24px 8px 34px minmax(0, 1fr)",
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
      aside={[sessionAside(field?.clock), sof ? `SoF ${irating(sof)}` : null].filter(Boolean).join("   ")}
    >
      {me < 0 ? (
        <Empty>Relative appears once you're in the car.</Empty>
      ) : (
        <div className="rows" style={{ "--cols": cols } as React.CSSProperties}>
          {slots.map((s, k) =>
            s ? (
              <Row key={s.r.carIdx} r={s.r} interval={showInterval ? s.interval : undefined}
                showIR={showIR} showLic={showLic} showLast={showLast} multiclass={multiclass} />
            ) : (
              <div key={`e${k}`} className="row row-empty" />
            ),
          )}
        </div>
      )}
    </Panel>
  );
}

/** Within this many seconds of the car ahead counts as a close fight. */
const CLOSE_S = 1.0;

function Row({ r, interval, showIR, showLic, showLast, multiclass }: {
  r: RelativeRow;
  /** Seconds to the car in front; null for the front row; undefined when the column is off. */
  interval: number | null | undefined;
  showIR: boolean; showLic: boolean; showLast: boolean; multiclass: boolean;
}) {
  const lic = license(r.license);
  const status = r.isPlayer ? "is-player" : r.lapStatus > 0 ? "is-ahead-lap" : r.lapStatus < 0 ? "is-behind-lap" : "";
  return (
    <div className={`row ${status}`} style={{ "--row-class": r.classColor } as React.CSSProperties}>
      <span className="pos" title={multiclass ? `P${r.position} overall` : undefined}>{r.classPosition || "–"}</span>
      <ClassStripe color={r.classColor} />
      <span className="num">{r.number}</span>
      <span className="name">
        {r.name}
        {r.onPitRoad && <span className="tag">Pit</span>}
      </span>
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
