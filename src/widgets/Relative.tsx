import type React from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { clock, gap, irating, license } from "../lib/format";
import { useLive } from "../lib/store";
import type { RelativeRow, SessionClock } from "../lib/types";
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
  const ahead = opt(config, "ahead", 3);
  const behind = opt(config, "behind", 3);
  const showIR = opt(config, "showIRating", true);
  const showLic = opt(config, "showLicense", true);
  const showPit = opt(config, "showPit", true);

  const rows = (field?.relative ?? []).filter((r) => showPit || !r.onPitRoad || r.isPlayer);
  const me = rows.findIndex((r) => r.isPlayer);

  // Fixed slot count so the widget never changes height.
  const slots: (RelativeRow | null)[] = [];
  for (let k = -ahead; k <= behind; k++) {
    slots.push(me >= 0 ? rows[me + k] ?? null : null);
  }

  const sof = field?.clock.strengthOfField;
  const cols = ["22px 3px 34px 1fr", showLic && "auto", showIR && "34px", "42px"].filter(Boolean).join(" ");
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
          {slots.map((r, k) =>
            r ? <Row key={r.carIdx} r={r} showIR={showIR} showLic={showLic} /> : <div key={`e${k}`} className="row row-empty" />,
          )}
        </div>
      )}
    </Panel>
  );
}

function Row({ r, showIR, showLic }: { r: RelativeRow; showIR: boolean; showLic: boolean }) {
  const lic = license(r.license);
  const status = r.isPlayer ? "is-player" : r.lapStatus > 0 ? "is-ahead-lap" : r.lapStatus < 0 ? "is-behind-lap" : "";
  return (
    <div className={`row ${status}`}>
      <span className="pos">{r.classPosition || "–"}</span>
      <span className="class-bar" style={{ background: r.classColor }} />
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
      {showIR && <span className="ir">{irating(r.irating)}</span>}
      <span className="gap">{r.isPlayer ? "" : gap(r.gap)}</span>
    </div>
  );
}
