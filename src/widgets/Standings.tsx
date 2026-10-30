import type React from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { gap as fmtGap, irating } from "../lib/format";
import { useLive } from "../lib/store";
import type { StandingRow } from "../lib/types";
import { BestLap, ClassStripe, IRatingBadge, LastLap } from "./cells";
import { Empty, Panel } from "./Panel";
import { sessionAside } from "./Relative";

type Slot = StandingRow | "break";

/** The top of a class plus a window around the player when they're further back. */
function pickRows(rows: StandingRow[], max: number): Slot[] {
  if (rows.length <= max) return rows;
  const me = rows.findIndex((r) => r.isPlayer);
  if (me < max) return rows.slice(0, max);
  const around = Math.min(5, Math.floor(max / 2));
  const top = max - around;
  const start = Math.min(rows.length - around, me - Math.floor(around / 2));
  return [...rows.slice(0, top), "break", ...rows.slice(start, start + around)];
}

interface Group {
  classId: number;
  name: string;
  color: string;
  rows: StandingRow[];
}

/** Classes in running order (the class of the overall leader first). */
function groupByClass(rows: StandingRow[]): Group[] {
  const groups = new Map<number, Group>();
  for (const r of rows) {
    let g = groups.get(r.classId);
    if (!g) {
      g = { classId: r.classId, name: r.className, color: r.classColor, rows: [] };
      groups.set(r.classId, g);
    }
    g.rows.push(r);
  }
  return [...groups.values()];
}

export function Standings({ config }: { config: WidgetConfig }) {
  const field = useLive((s) => s.field);
  const session = useLive((s) => s.session);
  const myRows = opt(config, "rows", 10);
  const otherRows = opt(config, "otherClassRows", 3);
  const myClassOnly = opt(config, "myClassOnly", false);
  const gapMode = opt<string>(config, "gapMode", "leader");
  const showLast = opt(config, "showLast", true);
  const showBest = opt(config, "showBest", true);
  const showIR = opt(config, "showIRating", true);
  const showSoF = opt(config, "showSoF", true);

  const all = field?.standings ?? [];
  const playerClass = all.find((r) => r.isPlayer)?.classId ?? session?.playerClassId;
  let groups = groupByClass(all);
  if (myClassOnly) groups = groups.filter((g) => g.classId === playerClass);
  const showClassHeads = groups.length > 1;
  const classSoF = (id: number) => field?.clock.classes.find((c) => c.classId === id)?.strengthOfField ?? null;
  // With one class there are no class headers, so the SoF goes in the top line.
  const headSoF = showSoF && !showClassHeads && groups.length === 1 ? classSoF(groups[0].classId) : null;

  const cols = [
    "24px 8px 34px minmax(0, 1fr)",
    showIR && "40px",
    "54px",
    showLast && "62px",
    showBest && "62px",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <Panel className="standings">
      {all.length === 0 ? (
        <Empty>Standings appear when a session is running.</Empty>
      ) : (
        <div className="rows" style={{ "--cols": cols } as React.CSSProperties}>
          {/* One header row: track and lap on the left, column labels on the right. */}
          <div className="row row-head">
            <span className="row-head-title">
              <b>{session?.trackName || "Standings"}</b>
              {sessionAside(field?.clock)}
              {headSoF ? <span>SoF {irating(headSoF)}</span> : null}
            </span>
            {showIR && <span>iR</span>}
            <span>{gapMode === "interval" ? "Int" : "Gap"}</span>
            {showLast && <span>Last</span>}
            {showBest && <span>Best</span>}
          </div>
          {groups.map((g) => {
            const max = g.classId === playerClass ? myRows : otherRows;
            const sof = showSoF ? classSoF(g.classId) : null;
            return (
              <div key={g.classId} className="class-group" style={{ "--row-class": g.color } as React.CSSProperties}>
                {showClassHeads && (
                  <div className="class-head">
                    <span className="class-name">{g.name || "Class"}</span>
                    <span>
                      {g.rows.length} cars{sof ? `   SoF ${irating(sof)}` : ""}
                    </span>
                  </div>
                )}
                {pickRows(g.rows, max).map((r, k) =>
                  r === "break" ? (
                    <div key={`b${k}`} className="row row-break" />
                  ) : (
                    <div
                      key={r.carIdx}
                      className={`row ${r.isPlayer ? "is-player" : ""}`}
                      style={{ "--row-class": r.classColor } as React.CSSProperties}
                    >
                      <span className="pos">{r.classPosition || r.position || "–"}</span>
                      <ClassStripe color={r.classColor} />
                      <span className="num">{r.number}</span>
                      <span className="name">
                        {r.name}
                        {r.onPitRoad && <span className="tag">Pit</span>}
                      </span>
                      {showIR && <IRatingBadge value={r.irating} />}
                      <span className="gap">{gapCell(r, gapMode)}</span>
                      {showLast && <LastLap time={r.lastLap} flag={r.lastFlag} />}
                      {showBest && <BestLap time={r.bestLap} fastest={r.classFastest} />}
                    </div>
                  ),
                )}
              </div>
            );
          })}
        </div>
      )}
    </Panel>
  );
}

function gapCell(r: StandingRow, mode: string) {
  if (r.lapsDown > 0) return `+${r.lapsDown}L`;
  const v = mode === "interval" ? r.interval : r.gap;
  if (v == null) return r.classPosition === 1 ? <span className="muted">Leader</span> : "";
  return fmtGap(v);
}
