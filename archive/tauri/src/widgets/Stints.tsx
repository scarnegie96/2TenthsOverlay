import type React from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { useLive, useTireCompounds } from "../lib/store";
import type { CarStints, Stint, StandingRow } from "../lib/types";
import { ClassStripe, StatusBadge, posClass } from "./cells";
import { Empty, Panel } from "./Panel";
import { groupByClass, pickRows } from "./Standings";

const TIMELINE_W = 240;
const TIMELINE_H = 14;

/** A colour family for a compound name: wets blue, softs red, mediums yellow, the rest neutral. */
export function compoundKind(name: string): "wet" | "soft" | "medium" | "hard" | "other" {
  const n = name.toLowerCase();
  if (/wet|rain|inter/.test(n)) return "wet";
  if (/soft/.test(n)) return "soft";
  if (/medium/.test(n)) return "medium";
  if (/hard|dry|primary|prime/.test(n)) return "hard";
  return "other";
}

/** A round lap step for the timeline axis, giving about 4 to 8 labels. */
function lapStep(max: number) {
  for (const s of [1, 2, 5, 10, 20, 25, 50, 100]) if (max / s <= 8) return s;
  return 200;
}

/**
 * Stints: for every driver, how long they've been out since their last stop
 * (live), the tyre they're on, how many stops they've made, and a timeline of
 * the whole session: each stint a bar coloured by compound, with its length
 * in laps, and a tick at each stop.
 */
export function Stints({ config }: { config: WidgetConfig }) {
  const field = useLive((s) => s.field);
  const compounds = useTireCompounds();
  const classes = opt<string>(config, "classes", "all");
  const rows = opt(config, "rows", 10);
  const otherRows = opt(config, "otherClassRows", 5);
  const showTimeline = opt(config, "showTimeline", true);
  const showLengths = opt(config, "showLengths", true);
  const showCompound = opt<string>(config, "showCompound", "auto");

  const standings = (field?.standings ?? []).filter((r) => r.position > 0 || r.status !== "out");
  const byCar = new Map((field?.stints ?? []).map((c) => [c.carIdx, c]));
  const me = standings.find((r) => r.isPlayer);
  let groups = groupByClass(standings);
  if (classes === "mine" && me) groups = groups.filter((g) => g.classId === me.classId);
  const race = /race/i.test(field?.clock.sessionType ?? "");
  // The compound column only earns its space when there's a choice of tyres.
  const compoundCol = showCompound === "always" || (showCompound === "auto" && compounds.length > 1);

  // Timeline scale: laps 0 to the furthest anyone has got.
  let maxLap = 1;
  for (const c of byCar.values()) for (const s of c.stints) maxLap = Math.max(maxLap, s.end);
  maxLap = Math.ceil(maxLap);

  const cols = [
    "24px 8px 34px minmax(0, 1fr)",
    compoundCol && "28px",
    "48px",
    "36px",
    showTimeline && `${TIMELINE_W}px`,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <Panel className={`stints ${showTimeline ? "" : "is-compact"}`}>
      {standings.length === 0 ? (
        <Empty>Stints appear once cars are on track.</Empty>
      ) : (
        <div className="rows" style={{ "--cols": cols } as React.CSSProperties}>
          <div className="row row-head">
            <span className="row-head-title">
              <b>Stints</b>
              {maxLap > 1 ? <span>{`Lap ${maxLap}`}</span> : null}
            </span>
            {compoundCol && <span title="Tyre compound now">Tyre</span>}
            <span title="Laps since the last stop">Stint</span>
            <span title="Stops so far">Stops</span>
            {showTimeline && <Axis maxLap={maxLap} />}
          </div>
          {groups.map((g) => {
            const max = me && g.classId === me.classId ? rows : otherRows;
            return (
              <div key={g.classId} className="class-group" style={{ "--row-class": g.color } as React.CSSProperties}>
                {groups.length > 1 && (
                  <div className="class-head">
                    <span className="class-name">{g.name || "Class"}</span>
                    <span>{g.rows.length} cars</span>
                  </div>
                )}
                {pickRows(g.rows, max).map((r, k) =>
                  r === "break" ? (
                    <div key={`b${k}`} className="row row-break" />
                  ) : (
                    <Row key={r.carIdx} r={r} c={byCar.get(r.carIdx)} compounds={compounds} compoundCol={compoundCol}
                      showTimeline={showTimeline} showLengths={showLengths} maxLap={maxLap} race={race} />
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

function Axis({ maxLap }: { maxLap: number }) {
  const step = lapStep(maxLap);
  const ticks: number[] = [];
  for (let l = step; l <= maxLap; l += step) ticks.push(l);
  return (
    <span className="st-axis">
      {ticks.map((l) => (
        <i key={l} style={{ left: `${(l / maxLap) * 100}%` }}>{l}</i>
      ))}
    </span>
  );
}

function Row({ r, c, compounds, compoundCol, showTimeline, showLengths, maxLap, race }: {
  r: StandingRow;
  c: CarStints | undefined;
  compounds: string[];
  compoundCol: boolean;
  showTimeline: boolean;
  showLengths: boolean;
  maxLap: number;
  race: boolean;
}) {
  const current = c?.stints[c.stints.length - 1];
  const live = current?.open ? current : null;
  const name = (s: Stint | null | undefined) => (s && s.compound >= 0 ? compounds[s.compound] ?? "" : "");
  const tyre = name(live ?? current);
  return (
    <div className={`row ${r.isPlayer ? "is-player" : ""} ${c?.out || r.status === "out" ? "is-out" : ""}`}
      style={{ "--row-class": r.classColor } as React.CSSProperties}>
      <span className={posClass(r.classPosition || r.position)}>{r.classPosition || r.position || "–"}</span>
      <ClassStripe color={r.classColor} />
      <span className="num">{r.number}</span>
      <span className="name">
        <span className="name-text">{r.name}</span>
        <StatusBadge status={r.status} race={race} />
      </span>
      {compoundCol && (
        <span className="st-tyre">
          {tyre && <i className={`tyre-badge tyre-${compoundKind(tyre)}`} title={tyre}>{tyre[0].toUpperCase()}</i>}
        </span>
      )}
      <span className="st-laps" title={live?.partial ? "Since Tenths first saw this car" : "Laps since the last stop"}>
        {live ? `${live.partial ? "≥" : ""}${(live.end - live.start).toFixed(1)}` : c?.inPits ? "–" : ""}
      </span>
      <span className="st-stops">{c ? c.stops : "–"}</span>
      {showTimeline && <Timeline c={c} compounds={compounds} maxLap={maxLap} showLengths={showLengths} />}
    </div>
  );
}

/** Each stint as a bar, coloured by compound, with a tick at each stop. */
function Timeline({ c, compounds, maxLap, showLengths }: { c: CarStints | undefined; compounds: string[]; maxLap: number; showLengths: boolean }) {
  const x = (lap: number) => (lap / maxLap) * TIMELINE_W;
  return (
    <svg className="st-timeline" width={TIMELINE_W} height={TIMELINE_H} viewBox={`0 0 ${TIMELINE_W} ${TIMELINE_H}`}>
      <rect className="st-track" x={0} y={TIMELINE_H / 2 - 1} width={TIMELINE_W} height={2} />
      {c?.stints.map((s, k) => {
        const x0 = x(s.start);
        const w = Math.max(1.5, x(s.end) - x0);
        const kind = s.compound >= 0 ? compoundKind(compounds[s.compound] ?? "") : "other";
        const laps = s.end - s.start;
        return (
          <g key={k}>
            <rect
              className={`st-bar tyre-${kind} ${s.open ? "is-open" : ""} ${s.partial ? "is-partial" : ""}`}
              x={x0} y={2} width={w} height={TIMELINE_H - 4} rx={2}
            >
              <title>{`${s.partial ? "At least " : ""}${laps.toFixed(1)} laps${s.compound >= 0 && compounds[s.compound] ? ` on ${compounds[s.compound]}` : ""}`}</title>
            </rect>
            {showLengths && w >= 16 && (
              <text className="st-len" x={x0 + w / 2} y={TIMELINE_H / 2 + 0.5}>{Math.floor(laps)}</text>
            )}
            {s.endedBy === "pit" && <rect className="st-stop" x={x(s.end) - 0.5} y={0} width={1.5} height={TIMELINE_H} />}
            {s.endedBy === "out" && <text className="st-out" x={x(s.end) + 3} y={TIMELINE_H / 2 + 0.5}>✕</text>}
          </g>
        );
      })}
    </svg>
  );
}
