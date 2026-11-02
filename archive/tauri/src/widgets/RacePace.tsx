import { useEffect, useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { raceHistory, type CarTrace } from "../lib/raceHistory";
import { useLive } from "../lib/store";
import { cssColor, paletteKey, withAlpha } from "../lib/theme";
import { Empty, Panel } from "./Panel";

const W = 520;
const H = 250;
/** Plot area inside the canvas: room for gap labels left, lap labels below, names right. */
const PAD = { left: 40, right: 132, top: 12, bottom: 20 };
const LABEL_H = 13;

/** A round step for the gap gridlines, giving 3 to 6 lines. */
function niceStep(max: number) {
  for (const s of [1, 2, 5, 10, 15, 30, 60, 120, 300]) if (max / s <= 6) return s;
  return 600;
}

const fmtGap = (s: number) => (s >= 60 ? `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, "0")}` : `${s.toFixed(1)}`);

/**
 * Race pace: everyone in your class against the class leader over the race.
 * The leader is the flat line along the top; every other car is a line below
 * it at its gap to the leader, so a line sloping down is a car slower than the
 * leader and one rising is catching. The steeper, the bigger the difference
 * in pace. Lap lines mark when the leader started each lap.
 */
export function RacePace({ config }: { config: WidgetConfig }) {
  const race = useLive((s) => /race/i.test(s.field?.clock.sessionType ?? ""));
  const hasData = useLive((s) => (s.field?.standings ?? []).some((r) => r.position > 0));
  const leaderName = useLive((s) => {
    const rows = s.field?.standings ?? [];
    const me = rows.find((r) => r.isPlayer);
    const cls = me?.classId ?? rows[0]?.classId;
    return rows.find((r) => r.classId === cls && r.classPosition === 1)?.name ?? null;
  });

  return (
    <Panel className="race-pace" title="Race pace" aside={leaderName ? `Gap to ${leaderName}` : "Gap to class leader"}>
      {!race ? (
        <Empty>The pace chart runs in races.</Empty>
      ) : !hasData ? (
        <Empty>Lines appear once the race is under way.</Empty>
      ) : (
        <Chart
          window={opt<string>(config, "window", "race")}
          show={opt<string>(config, "show", "all")}
          around={opt(config, "around", 3)}
          maxGap={opt(config, "maxGap", 0)}
          showTrend={opt(config, "showTrend", true)}
        />
      )}
    </Panel>
  );
}

interface ChartProps {
  window: string;
  show: string;
  around: number;
  maxGap: number;
  showTrend: boolean;
}

function Chart(p: ChartProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const palette = useLive((s) => paletteKey(s.config.general));

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2;
    el.width = W * dpr;
    el.height = H * dpr;
    ctx.scale(dpr, dpr);

    const col = {
      ink: cssColor(el, "var(--ink)"),
      ink2: cssColor(el, "var(--ink-2)"),
      ink3: cssColor(el, "var(--ink-3)"),
      line: cssColor(el, "var(--line)"),
      grid: cssColor(el, "var(--grid)"),
      you: cssColor(el, "var(--attention)"),
      gain: cssColor(el, "var(--gain)"),
      loss: cssColor(el, "var(--loss)"),
    };
    const font = getComputedStyle(el).getPropertyValue("--font").trim() || "sans-serif";
    const x0 = PAD.left;
    const x1 = W - PAD.right;
    const y0 = PAD.top;
    const y1 = H - PAD.bottom;

    let raf = 0;
    let drawn = -1;

    const draw = () => {
      raf = requestAnimationFrame(draw);
      if (raceHistory.version === drawn) return;
      drawn = raceHistory.version;
      ctx.clearRect(0, 0, W, H);

      const all = [...raceHistory.cars.values()];
      const me = all.find((c) => c.isPlayer);
      const classId = me?.classId ?? all[0]?.classId;
      let cars = all.filter((c) => c.classId === classId);
      if (p.show === "around" && me) {
        cars = cars.filter((c) => c.position === 1 || Math.abs(c.position - me.position) <= p.around);
      }
      if (cars.length === 0 || raceHistory.start == null) return;

      // Time window: the whole race, or the leader's last few laps.
      const marks = raceHistory.laps.get(classId!) ?? [];
      const tEnd = raceHistory.now;
      let tStart = raceHistory.start;
      if (p.window !== "race") {
        const n = Number(p.window);
        if (marks.length >= n) tStart = marks[marks.length - n].t;
      }
      const span = Math.max(1, tEnd - tStart);
      const xAt = (t: number) => x0 + ((t - tStart) / span) * (x1 - x0);

      // Gap scale: fit the cars still on the lead lap, unless capped. Lapped
      // and retired cars would squash everyone else, so they run along the
      // bottom edge instead.
      let most = 0;
      for (const c of cars) {
        if (!onScale(c)) continue;
        most = Math.max(most, c.gap);
        for (let i = 0; i < c.points.length; i += 2) if (c.points[i] >= tStart) most = Math.max(most, c.points[i + 1]);
      }
      const max = p.maxGap > 0 ? p.maxGap : Math.max(5, most * 1.08);
      const yAt = (g: number) => y0 + (Math.min(g, max) / max) * (y1 - y0);

      // Grid: gap lines with labels on the left.
      ctx.font = `500 10px ${font}`;
      ctx.textBaseline = "middle";
      ctx.textAlign = "right";
      const step = niceStep(max);
      for (let g = step; g <= max; g += step) {
        const y = Math.round(yAt(g)) + 0.5;
        ctx.strokeStyle = col.grid;
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(x0, y);
        ctx.lineTo(x1, y);
        ctx.stroke();
        ctx.fillStyle = col.ink3;
        ctx.fillText(g >= 60 ? `+${fmtGap(g)}` : `+${g}s`, x0 - 6, y);
      }
      // Lap lines, labelled along the bottom (thinned so labels don't crowd).
      ctx.textAlign = "center";
      ctx.textBaseline = "alphabetic";
      const visible = marks.filter((m) => m.t >= tStart);
      const every = Math.max(1, Math.ceil(visible.length / 10));
      visible.forEach((m, k) => {
        const x = Math.round(xAt(m.t)) + 0.5;
        ctx.strokeStyle = col.grid;
        ctx.setLineDash([2, 3]);
        ctx.beginPath();
        ctx.moveTo(x, y0);
        ctx.lineTo(x, y1);
        ctx.stroke();
        ctx.setLineDash([]);
        if ((visible.length - 1 - k) % every === 0) {
          ctx.fillStyle = col.ink3;
          ctx.fillText(`L${m.lap}`, x, H - 5);
        }
      });

      // Each car's line, you last so you're on top.
      const order = [...cars].sort((a, b) => Number(a.isPlayer) - Number(b.isPlayer) || b.position - a.position);
      ctx.lineJoin = "round";
      for (const c of order) {
        if (c.position === 1) continue;
        const stroke = c.isPlayer ? col.you : withAlpha(col.ink2, c.status === "out" ? 0.25 : 0.55);
        ctx.strokeStyle = stroke;
        ctx.lineWidth = c.isPlayer ? 2.5 : 1.4;
        ctx.setLineDash(c.status === "out" ? [3, 3] : []);
        ctx.beginPath();
        let started = false;
        const pts = c.points;
        for (let i = 0; i < pts.length; i += 2) {
          // Keep the point just before the window so the line enters from the edge.
          if (pts[i] < tStart && !(i + 2 < pts.length && pts[i + 2] >= tStart)) continue;
          const x = Math.max(x0, xAt(pts[i]));
          const y = yAt(pts[i + 1]);
          if (!started) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
          started = true;
        }
        if (c.t >= tStart) {
          if (!started) ctx.moveTo(xAt(c.t), yAt(c.gap));
          ctx.lineTo(xAt(c.t), yAt(c.gap));
        }
        ctx.stroke();
      }
      ctx.setLineDash([]);

      // The leader: a flat line along the top.
      const leader = cars.find((c) => c.position === 1);
      ctx.strokeStyle = leader?.isPlayer ? col.you : col.ink;
      ctx.lineWidth = leader?.isPlayer ? 2.5 : 2;
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y0);
      ctx.stroke();

      // Names on the right at each car's current gap, nudged apart so they don't overlap.
      const labels = [...cars]
        .filter((c) => c.status !== "out" || c.t >= tStart)
        .sort((a, b) => a.position - b.position)
        .map((c) => ({ c, y: c.position === 1 ? y0 : yAt(c.gap) }));
      for (let i = 1; i < labels.length; i++) labels[i].y = Math.max(labels[i].y, labels[i - 1].y + LABEL_H);
      const overflow = labels.length ? labels[labels.length - 1].y - (H - 6) : 0;
      if (overflow > 0) {
        labels[labels.length - 1].y -= overflow;
        for (let i = labels.length - 2; i >= 0; i--) labels[i].y = Math.min(labels[i].y, labels[i + 1].y - LABEL_H);
      }
      ctx.textBaseline = "middle";
      for (const { c, y } of labels) {
        const ly = Math.max(6, y);
        const dot = c.position === 1 ? y0 : yAt(c.gap);
        const color = c.isPlayer ? col.you : c.position === 1 ? col.ink : col.ink2;
        // Current position dot and a leader line to the label.
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.arc(x1, dot, c.isPlayer ? 3.5 : 2.5, 0, Math.PI * 2);
        ctx.fill();
        ctx.strokeStyle = withAlpha(color, 0.4);
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(x1 + 4, dot);
        ctx.lineTo(x1 + 10, ly);
        ctx.stroke();

        ctx.textAlign = "left";
        ctx.font = `${c.isPlayer ? 700 : 600} 11px ${font}`;
        ctx.fillStyle = color;
        const name = `${c.position} ${c.abbrev || c.name}`;
        ctx.fillText(name.length > 15 ? `${name.slice(0, 14)}…` : name, x1 + 13, ly);
        ctx.textAlign = "right";
        ctx.font = `500 11px ${font}`;
        if (c.position === 1) {
          ctx.fillStyle = col.ink3;
          ctx.fillText("Leader", W - 2, ly);
        } else {
          const trend = p.showTrend && c.status !== "out" ? lapTrend(c) : null;
          const gapText = c.status === "out" ? "Out" : c.lapsDown > 0 ? `+${c.lapsDown}L` : `+${fmtGap(c.gap)}`;
          if (trend != null && Math.abs(trend) >= 0.05) {
            // Gaining on the leader (gap shrinking) is green, losing is red.
            ctx.fillStyle = trend < 0 ? col.gain : col.loss;
            ctx.fillText(`${trend < 0 ? "▲" : "▼"}${Math.abs(trend).toFixed(1)}`, W - 2, ly);
            ctx.fillStyle = col.ink3;
            ctx.fillText(gapText, W - 34, ly);
          } else {
            ctx.fillStyle = col.ink3;
            ctx.fillText(gapText, W - 2, ly);
          }
        }
      }
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [p.window, p.show, p.around, p.maxGap, p.showTrend, palette]);

  return <canvas ref={canvas} className="pace-chart" style={{ width: W, height: H }} />;
}

/** On the lead lap and still racing: counts towards the gap scale. */
const onScale = (c: CarTrace) => c.lapsDown === 0 && c.status !== "out";

/**
 * How much the gap to the leader changed over the leader's last lap
 * (negative = catching), or null with less than a lap of history.
 */
function lapTrend(c: CarTrace): number | null {
  const marks = raceHistory.laps.get(c.classId) ?? [];
  if (marks.length < 2) return null;
  const lapLength = marks[marks.length - 1].t - marks[marks.length - 2].t;
  const then = c.t - lapLength;
  const pts = c.points;
  for (let i = pts.length - 2; i >= 0; i -= 2) {
    if (pts[i] <= then) return c.gap - pts[i + 1];
  }
  return null;
}
