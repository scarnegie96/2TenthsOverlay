import type React from "react";
import { STEADY, history, trend, type Trend } from "../lib/battle";
import { opt } from "../lib/config";
import { raceHistory } from "../lib/raceHistory";
import { useLive } from "../lib/store";
import type { StandingRow } from "../lib/types";
import { ClassStripe } from "./cells";
import { Empty, Panel } from "./Panel";
import type { WidgetProps } from "./registry";

const SPARK_W = 110;
const SPARK_H = 30;

/** A gap of more than this isn't a battle yet, so it's shown quieter. */
const NEAR_S = 2;

/**
 * Battle: the cars directly ahead of and behind you in your class (ahead on
 * top). For each, the gap now, how much it changes per lap, and the gap over
 * the last laps. Only what the timing shows: no predictions.
 */
export function Battle({ config }: WidgetProps) {
  const field = useLive((s) => s.field);
  const trendLaps = opt(config, "trendLaps", 3);
  const sparkLaps = opt(config, "sparkLaps", 8);
  const showSpark = opt(config, "showSparkline", true);

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
  // A lap's time for the trend: your recent lap, else your best.
  const lap = me.lastLap ?? me.bestLap ?? 0;
  const props = { me, lap, trendLaps, sparkLaps, showSpark };

  return (
    <Panel className="battle" title="Battle">
      <Opponent side="ahead" car={ahead} {...props} />
      <Opponent side="behind" car={behind} {...props} />
    </Panel>
  );
}

function Opponent({ side, car, me, lap, trendLaps, sparkLaps, showSpark }: {
  side: "ahead" | "behind";
  car: StandingRow | null;
  me: StandingRow;
  lap: number;
  trendLaps: number;
  sparkLaps: number;
  showSpark: boolean;
}) {
  if (!car) return <section className={`bt-car is-${side} is-none`} />;

  // The gap now, from the timing (fresher than the history).
  const g = (r: StandingRow) => (r.classPosition === 1 ? 0 : r.gap);
  const lapsApart = Math.abs(car.lapsDown - me.lapsDown);
  const gapNow = g(car) != null && g(me) != null ? Math.abs(g(car)! - g(me)!) : null;

  // How the gap changes per lap, and its history, from the race history.
  const mine = raceHistory.cars.get(me.carIdx)?.points ?? [];
  const theirs = raceHistory.cars.get(car.carIdx)?.points ?? [];
  const now = raceHistory.now;
  const t: Trend | null = lapsApart === 0 ? trend(mine, theirs, now, lap, trendLaps) : null;
  const spark = showSpark && lapsApart === 0 ? history(mine, theirs, now, lap, sparkLaps) : [];

  // The gap shrinking is good when chasing, bad when defending.
  const closing = t ? t.closingPerLap : 0;
  const good = side === "ahead" ? closing > STEADY : closing < -STEADY;
  const bad = side === "ahead" ? closing < -STEADY : closing > STEADY;

  return (
    <section className={`bt-car is-${side} ${gapNow != null && gapNow < NEAR_S && lapsApart === 0 ? "is-near" : ""}`}
      style={{ "--row-class": car.classColor } as React.CSSProperties}>
      <div className="bt-who">
        <span className="bt-pos">P{car.classPosition}</span>
        <ClassStripe color={car.classColor} />
        <span className="num">{car.number}</span>
        <span className="bt-name">{car.name}</span>
      </div>
      <div className="bt-main">
        <div className="bt-gap">
          <b>{lapsApart > 0 ? `+${lapsApart}L` : gapNow != null ? gapNow.toFixed(gapNow < 10 ? 2 : 1) : "–"}</b>
        </div>
        <span className={`bt-rate ${good ? "is-good" : bad ? "is-bad" : ""}`}
          title={t ? `Change in the gap per lap, over the last ${t.laps} lap${t.laps > 1 ? "s" : ""}` : undefined}>
          {t ? `${Math.abs(closing) < 0.005 ? "±" : closing > 0 ? "−" : "+"}${Math.abs(closing).toFixed(2)}` : "–"}
        </span>
        {showSpark && <Sparkline values={spark} tone={good ? "is-good" : bad ? "is-bad" : ""} />}
      </div>
    </section>
  );
}

/** The gap over the last laps, oldest left; lower is closer. Coloured like the change per lap. */
function Sparkline({ values, tone }: { values: number[]; tone: string }) {
  if (values.length < 2) return <svg className="bt-spark" width={SPARK_W} height={SPARK_H} aria-hidden />;
  const max = Math.max(...values, 0.5);
  const x = (i: number) => (i / (values.length - 1)) * (SPARK_W - 2) + 1;
  const y = (v: number) => SPARK_H - 2 - (v / max) * (SPARK_H - 4);
  const d = values.map((v, i) => `${i ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  return (
    <svg className={`bt-spark ${tone}`} width={SPARK_W} height={SPARK_H} viewBox={`0 0 ${SPARK_W} ${SPARK_H}`} aria-hidden>
      <line className="bt-spark-base" x1={0} x2={SPARK_W} y1={SPARK_H - 2} y2={SPARK_H - 2} />
      <path className="bt-spark-line" d={d} />
      <circle className="bt-spark-dot" cx={x(values.length - 1)} cy={y(values[values.length - 1])} r={2.5} />
    </svg>
  );
}
