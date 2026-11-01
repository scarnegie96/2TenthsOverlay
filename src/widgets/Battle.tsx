import type React from "react";
import { STEADY, history, lapsBetween, trend, type Trend } from "../lib/battle";
import { opt } from "../lib/config";
import { lapTime, signed } from "../lib/format";
import { raceHistory } from "../lib/raceHistory";
import { useLive } from "../lib/store";
import type { CarStints, StandingRow } from "../lib/types";
import { ClassStripe, IRatingBadge } from "./cells";
import { Empty, Panel } from "./Panel";
import type { WidgetProps } from "./registry";
import { compoundKind } from "./Stints";

const SPARK_W = 170;
const SPARK_H = 40;

/** A gap of more than this isn't a battle yet, so it's shown quieter. */
const NEAR_S = 2;

interface Shared {
  me: StandingRow;
  mine: CarStints | undefined;
  stints: CarStints[];
  compounds: string[];
  lap: number;
  trendLaps: number;
  sparkLaps: number;
  showSpark: boolean;
  showLaps: boolean;
  showTyres: boolean;
}

/**
 * Battle: the cars directly ahead of and behind you in your class (ahead on
 * top). For each, the gap now, how much it changes per lap, the gap over the
 * last laps, their last and best laps against yours, their tyres and stops.
 * Only what the timing shows: no predictions.
 */
export function Battle({ config }: WidgetProps) {
  const field = useLive((s) => s.field);
  const compounds = useLive((s) => s.session?.tireCompounds ?? []);

  const race = /race/i.test(field?.clock.sessionType ?? "");
  const rows = field?.standings ?? [];
  const me = rows.find((r) => r.isPlayer);

  if (!race || !me) {
    return (
      <Panel className="battle" title="Battle">
        <Empty>{!race ? "Battles show in races." : "Battles show once you're in the race."}</Empty>
      </Panel>
    );
  }

  const cls = rows.filter((r) => r.classId === me.classId && r.position > 0).sort((a, b) => a.classPosition - b.classPosition);
  const at = cls.findIndex((r) => r.isPlayer);
  const ahead = at > 0 ? cls[at - 1] : null;
  const behind = at >= 0 && at < cls.length - 1 ? cls[at + 1] : null;
  const stints = field?.stints ?? [];
  const shared: Shared = {
    me,
    mine: stints.find((c) => c.carIdx === me.carIdx),
    stints,
    compounds,
    // A lap's time for the trend: your recent lap, else your best.
    lap: me.lastLap ?? me.bestLap ?? 0,
    trendLaps: opt(config, "trendLaps", 3),
    sparkLaps: opt(config, "sparkLaps", 8),
    showSpark: opt(config, "showSparkline", true),
    showLaps: opt(config, "showLaps", true),
    showTyres: opt(config, "showTyres", true),
  };

  return (
    <Panel className="battle" title="Battle" aside={me.classPosition > 0 ? `P${me.classPosition}` : undefined}>
      <Opponent side="ahead" car={ahead} {...shared} />
      <Opponent side="behind" car={behind} {...shared} />
    </Panel>
  );
}

function Opponent({ side, car, me, mine, stints, compounds, lap, trendLaps, sparkLaps, showSpark, showLaps, showTyres }: Shared & {
  side: "ahead" | "behind";
  car: StandingRow | null;
}) {
  if (!car) return <section className={`bt-car is-${side} is-none`} />;

  // The gap now, from the timing (fresher than the history).
  const g = (r: StandingRow) => (r.classPosition === 1 ? 0 : r.gap);
  const lapsApart = lapsBetween(car, me);
  const gapNow = g(car) != null && g(me) != null ? Math.abs(g(car)! - g(me)!) : null;

  // How the gap changes per lap, and its history, from the race history.
  const myTrace = raceHistory.cars.get(me.carIdx)?.points ?? [];
  const theirTrace = raceHistory.cars.get(car.carIdx)?.points ?? [];
  const now = raceHistory.now;
  const t: Trend | null = lapsApart === 0 ? trend(myTrace, theirTrace, now, lap, trendLaps) : null;
  const spark = showSpark && lapsApart === 0 ? history(myTrace, theirTrace, now, lap, sparkLaps) : [];

  // The gap shrinking is good when chasing, bad when defending.
  const closing = t ? t.closingPerLap : 0;
  const good = side === "ahead" ? closing > STEADY : closing < -STEADY;
  const bad = side === "ahead" ? closing < -STEADY : closing > STEADY;
  const tone = good ? "is-good" : bad ? "is-bad" : "";

  const theirs = stints.find((c) => c.carIdx === car.carIdx);
  const stint = theirs?.stints[theirs.stints.length - 1];
  const tyre = stint && stint.compound >= 0 ? compounds[stint.compound] ?? "" : "";
  const tyreLaps = stint ? Math.max(0, Math.floor(stint.end - stint.start)) : null;

  return (
    <section className={`bt-car is-${side} ${gapNow != null && gapNow < NEAR_S && lapsApart === 0 ? "is-near" : ""}`}
      style={{ "--row-class": car.classColor } as React.CSSProperties}>
      <div className="bt-who">
        <span className="bt-pos">P{car.classPosition}</span>
        <ClassStripe color={car.classColor} />
        <span className="num">{car.number}</span>
        <span className="bt-name">{car.name}</span>
        <IRatingBadge value={car.irating} />
      </div>
      <div className="bt-main">
        <div className="bt-figures">
          <b className="bt-gap">
            {lapsApart > 0 ? `+${lapsApart}L` : gapNow != null ? gapNow.toFixed(gapNow < 10 ? 2 : 1) : "–"}
          </b>
          <span className={`bt-rate ${tone}`}
            title={t ? `Change in the gap per lap, over the last ${t.laps} lap${t.laps > 1 ? "s" : ""}` : undefined}>
            {t ? `${Math.abs(closing) < 0.005 ? "±" : closing > 0 ? "−" : "+"}${Math.abs(closing).toFixed(2)}` : "–"}
            <small>/lap</small>
          </span>
        </div>
        {showSpark && <Sparkline values={spark} tone={tone} />}
      </div>
      {(showLaps || showTyres) && (
        <div className="bt-facts">
          {showLaps && <LapVs label="L" theirs={car.lastLap} mine={me.lastLap} />}
          {showLaps && <LapVs label="B" theirs={car.bestLap} mine={me.bestLap} />}
          {showTyres && stint && (
            <span className="bt-fact" title={`${tyre || "Tyres"}: ${tyreLaps} lap${tyreLaps === 1 ? "" : "s"} seen on this set${stint.partial ? " (fitted before Tenths saw it)" : ""}`}>
              {tyre && <i className={`tyre-badge tyre-${compoundKind(tyre)}`}>{tyre[0].toUpperCase()}</i>}
              {tyreLaps}{stint.partial ? "+" : ""}L
            </span>
          )}
          {showTyres && theirs && (
            <span className="bt-fact" title="Pit stops: theirs (yours)">
              <span className="bt-k">Stops</span>{theirs.stops}<span className="bt-you">({mine?.stops ?? 0})</span>
            </span>
          )}
        </div>
      )}
    </section>
  );
}

/** Their lap, and how it compares with yours: green when theirs was slower (good for you). */
function LapVs({ label, theirs, mine }: { label: string; theirs: number | null; mine: number | null }) {
  const d = theirs != null && mine != null && theirs > 0 && mine > 0 ? theirs - mine : null;
  return (
    <span className="bt-fact" title={label === "L" ? "Their last lap, and against yours" : "Their best lap, and against yours"}>
      <span className="bt-k">{label}</span>
      {lapTime(theirs)}
      {d != null && <span className={d > 0.0005 ? "is-good" : d < -0.0005 ? "is-bad" : ""}>{signed(d, 3)}</span>}
    </span>
  );
}

/** The gap over the last laps, oldest left; lower is closer. Coloured like the change per lap. */
function Sparkline({ values, tone }: { values: number[]; tone: string }) {
  if (values.length < 2) return <svg className="bt-spark" width={SPARK_W} height={SPARK_H} aria-hidden />;
  const max = Math.max(...values, 0.5);
  const x = (i: number) => (i / (values.length - 1)) * (SPARK_W - 6) + 1;
  const y = (v: number) => SPARK_H - 3 - (v / max) * (SPARK_H - 7);
  const d = values.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const area = `${d} L${x(values.length - 1).toFixed(1)},${SPARK_H - 3} L${x(0).toFixed(1)},${SPARK_H - 3} Z`;
  return (
    <svg className={`bt-spark ${tone}`} width={SPARK_W} height={SPARK_H} viewBox={`0 0 ${SPARK_W} ${SPARK_H}`} aria-hidden>
      <path className="bt-spark-area" d={area} />
      <line className="bt-spark-base" x1={0} x2={SPARK_W} y1={SPARK_H - 3} y2={SPARK_H - 3} />
      <path className="bt-spark-line" d={d} />
      <circle className="bt-spark-dot" cx={x(values.length - 1)} cy={y(values[values.length - 1])} r={3} />
    </svg>
  );
}
