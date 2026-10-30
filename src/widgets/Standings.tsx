import type React from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { gap as fmtGap, irating, lapTime } from "../lib/format";
import { useLive } from "../lib/store";
import type { StandingRow } from "../lib/types";
import { Empty, Panel } from "./Panel";
import { sessionAside } from "./Relative";

/** Top of the order plus a window around the player when they're further back. */
function pickRows(rows: StandingRow[], max: number): (StandingRow | "gap")[] {
  if (rows.length <= max) return rows;
  const me = rows.findIndex((r) => r.isPlayer);
  if (me < max) return rows.slice(0, max);
  const around = Math.min(5, Math.floor(max / 2));
  const top = max - around;
  const start = Math.min(rows.length - around, me - Math.floor(around / 2));
  return [...rows.slice(0, top), "gap", ...rows.slice(start, start + around)];
}

export function Standings({ config }: { config: WidgetConfig }) {
  const field = useLive((s) => s.field);
  const session = useLive((s) => s.session);
  const max = opt(config, "rows", 14);
  const myClassOnly = opt(config, "myClassOnly", true);
  const gapMode = opt(config, "gapMode", "leader");
  const showLast = opt(config, "showLast", true);
  const showIR = opt(config, "showIRating", false);

  let rows = field?.standings ?? [];
  const playerClass = rows.find((r) => r.isPlayer)?.classId ?? session?.playerClassId;
  const filterClass = myClassOnly && session?.multiclass && playerClass != null;
  if (filterClass) rows = rows.filter((r) => r.classId === playerClass);
  const shown = pickRows(rows, max);
  const cols = ["22px 3px 34px 1fr", showIR && "34px", showLast && "auto", "58px"].filter(Boolean).join(" ");
  const className = filterClass ? rows[0]?.className : null;

  return (
    <Panel
      className="standings"
      title={session?.trackName || "Standings"}
      aside={[className, sessionAside(field?.clock)].filter(Boolean).join("   ")}
    >
      {rows.length === 0 ? (
        <Empty>Standings appear when a session is running.</Empty>
      ) : (
        <div className="rows" style={{ "--cols": cols } as React.CSSProperties}>
          {shown.map((r, k) =>
            r === "gap" ? (
              <div key={`g${k}`} className="row row-break" />
            ) : (
              <div key={r.carIdx} className={`row ${r.isPlayer ? "is-player" : ""}`}>
                <span className="pos">{filterClass ? r.classPosition : r.position || "–"}</span>
                <span className="class-bar" style={{ background: r.classColor }} />
                <span className="num">{r.number}</span>
                <span className="name">
                  {r.name}
                  {r.onPitRoad && <span className="tag">Pit</span>}
                </span>
                {showIR && <span className="ir">{irating(r.irating)}</span>}
                {showLast && (
                  <span className={`time ${r.classFastest && r.lastLap === r.bestLap ? "is-fastest" : ""}`}>
                    {lapTime(r.lastLap)}
                  </span>
                )}
                <span className="gap">{gapCell(r, gapMode)}</span>
              </div>
            ),
          )}
        </div>
      )}
    </Panel>
  );
}

function gapCell(r: StandingRow, mode: string) {
  if (r.lapsDown > 0) return `+${r.lapsDown}L`;
  const v = mode === "interval" ? r.interval : r.gap;
  if (v == null) return r.position === 1 || r.classPosition === 1 ? "Leader" : "";
  return `+${fmtGap(v)}`;
}
