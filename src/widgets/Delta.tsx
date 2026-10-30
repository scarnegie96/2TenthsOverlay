import { useEffect, useReducer, useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { BINS, deltaHistory } from "../lib/deltaHistory";
import { lapTime, signed } from "../lib/format";
import { useLive } from "../lib/store";
import type { DeltaMode } from "../lib/types";
import { Panel } from "./Panel";

const GRAPH_W = 640;
const GRAPH_H = 92;
/** Space under the plot for sector numbers. */
const LABEL_H = 14;

export const MODE_LABELS: Record<DeltaMode, string> = {
  best: "Best lap",
  optimal: "Optimal lap",
  sessionBest: "Session best",
  sessionOptimal: "Session optimal",
  last: "Last lap",
  reference: "Reference lap",
};

/** Sector layout: iRacing's track sectors, or equal mini sectors. */
function sectorStarts(mode: string, mini: number, track: number[] | undefined): number[] {
  if (mode === "track" && track && track.length >= 2) return track;
  const n = Math.max(2, Math.round(mini));
  return Array.from({ length: n }, (_, i) => i / n);
}

/**
 * Horizontal delta strip. Our current lap is the flat baseline; the comparison
 * lap's line rises above it where the comparison is faster (we're losing time,
 * red) and drops below where it's slower (we're gaining, green). Underneath,
 * each sector shows what it gained or lost.
 */
export function Delta({ config }: { config: WidgetConfig }) {
  const mode = opt<string>(config, "compareTo", "best") as DeltaMode;
  const sectorMode = opt<string>(config, "sectors", "track");
  const mini = opt(config, "miniSectors", 12);
  const showPrevious = opt(config, "showPrevious", true);

  const trackSectors = useLive((s) => s.session?.sectors);
  const reference = useLive((s) => s.reference);
  const lapBest = useLive((s) => s.tick?.lapBest ?? null);
  const lapLast = useLive((s) => s.tick?.lapLast ?? null);

  // Redraw the text parts 10x a second; the graph runs on its own animation frame.
  const [, refresh] = useReducer((n: number) => n + 1, 0);
  useEffect(() => {
    const id = window.setInterval(refresh, 100);
    return () => window.clearInterval(id);
  }, []);

  const starts = sectorStarts(sectorMode, mini, trackSectors);
  useEffect(() => {
    deltaHistory.setSectors(starts);
  }, [starts.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  const h = deltaHistory.modes[mode];
  const tick = useLive.getState().tick;
  const now = h.now;
  const state = now == null ? "" : now > 0.005 ? "is-loss" : now < -0.005 ? "is-gain" : "";

  const comparisonTime =
    mode === "reference" ? (reference?.loaded ? reference.lapTime : null)
    : mode === "last" ? lapLast
    : mode === "sessionBest" ? lapBest
    : null;

  const unavailable =
    mode === "reference" && !reference?.loaded
      ? "Import a lap in settings to compare against it."
      : mode === "reference" && !reference?.matchesTrack
        ? `Your reference lap is for ${[reference?.trackName, reference?.trackConfig].filter(Boolean).join(" ")}.`
        : now == null
          ? `No ${MODE_LABELS[mode].toLowerCase()} to compare against yet.`
          : null;

  const live = deltaHistory.liveSector(mode);
  const lastSector = deltaHistory.lastSector(mode);
  const sectorNo = deltaHistory.currentSector + 1;

  return (
    <Panel className={`delta-strip ${state}`}>
      <div className="ds-main">
        <div className="ds-head">
          <span className="ds-mode">vs {MODE_LABELS[mode]}</span>
          {comparisonTime != null && comparisonTime > 0 && <span className="ds-ref">{lapTime(comparisonTime)}</span>}
          <span className="ds-lap">{lapTime(tick?.lapCurrent ?? null)}</span>
          <span className="ds-total">{now == null ? "–.––" : signed(now)}</span>
        </div>
        <div className="ds-graph-wrap">
          <Graph mode={mode} showPrevious={showPrevious} />
          {unavailable && <div className="ds-empty">{unavailable}</div>}
        </div>
        <div className="ds-sectors">
          {deltaHistory.sectors.map((start, i) => {
            const end = deltaHistory.sectors[i + 1] ?? 1;
            const isCurrent = i === deltaHistory.currentSector;
            const v = isCurrent ? live : h.sectors[i];
            const done = !isCurrent && v != null;
            const tone = v == null ? "" : v > 0.005 ? "is-loss" : v < -0.005 ? "is-gain" : "is-even";
            const roomy = (end - start) * GRAPH_W >= 38;
            return (
              <div
                key={i}
                className={`ds-sector ${tone} ${isCurrent ? "is-current" : done ? "is-done" : "is-upcoming"}`}
                style={{ flexGrow: end - start }}
                title={`Sector ${i + 1}`}
              >
                {v != null && roomy ? signed(v) : ""}
              </div>
            );
          })}
        </div>
      </div>
      <div className="ds-side">
        <div className="ds-side-sector">
          <span className="ds-side-label">Sector</span>
          <span className="ds-side-num">{sectorNo}</span>
        </div>
        <div className="ds-side-splits">
          <Split label="This sector" value={live} />
          <Split label={lastSector ? `Sector ${lastSector.index + 1}` : "Last sector"} value={lastSector?.value ?? null} />
        </div>
      </div>
    </Panel>
  );
}

function Split({ label, value }: { label: string; value: number | null }) {
  const tone = value == null ? "" : value > 0.005 ? "is-loss" : value < -0.005 ? "is-gain" : "";
  return (
    <div className={`ds-split ${tone}`}>
      <span className="ds-split-label">{label}</span>
      <span className="ds-split-value">{value == null ? "–" : `${signed(value)}s`}</span>
    </div>
  );
}

/** The lap graph, drawn on a canvas every animation frame. */
function Graph({ mode, showPrevious }: { mode: DeltaMode; showPrevious: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2;
    el.width = GRAPH_W * dpr;
    el.height = GRAPH_H * dpr;
    ctx.scale(dpr, dpr);

    const css = getComputedStyle(el);
    const color = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
    const gain = color("--gain", "#4be08f");
    const loss = color("--loss", "#ff5468");
    const ink = color("--ink", "#e9eef3");
    const ink3 = color("--ink-3", "#6d7c8d");
    const font = color("--font", "sans-serif");

    const plotH = GRAPH_H - LABEL_H;
    const mid = plotH / 2;
    let range = 0.5;
    let raf = 0;

    const x = (bin: number) => ((bin + 0.5) / BINS) * GRAPH_W;
    // Positive delta (comparison faster) plots above the baseline.
    const y = (d: number) => mid - (d / range) * (mid - 4);

    const tracePath = (data: Float32Array) => {
      const path = new Path2D();
      let open = false;
      let first = -1;
      let lastBin = -1;
      for (let i = 0; i < BINS; i++) {
        const d = data[i];
        if (Number.isNaN(d)) continue;
        if (!open) {
          path.moveTo(x(i), y(d));
          open = true;
          first = i;
        } else path.lineTo(x(i), y(d));
        lastBin = i;
      }
      return { path, first, lastBin };
    };

    const draw = () => {
      const h = deltaHistory.modes[mode];
      const sectors = deltaHistory.sectors;
      ctx.clearRect(0, 0, GRAPH_W, GRAPH_H);

      // Fit the scale to what's on screen, easing so it doesn't jump.
      let maxAbs = Math.abs(h.now ?? 0);
      for (let i = 0; i < BINS; i++) {
        const a = h.current[i];
        const b = showPrevious ? h.previous[i] : NaN;
        if (!Number.isNaN(a)) maxAbs = Math.max(maxAbs, Math.abs(a));
        if (!Number.isNaN(b)) maxAbs = Math.max(maxAbs, Math.abs(b));
      }
      const target = Math.max(0.25, Math.ceil(maxAbs * 1.2 * 4) / 4);
      range += (target - range) * 0.08;

      // Sector bands, dividers and numbers.
      ctx.font = `600 11px ${font}`;
      ctx.textAlign = "center";
      ctx.textBaseline = "alphabetic";
      sectors.forEach((start, i) => {
        const end = sectors[i + 1] ?? 1;
        const x0 = start * GRAPH_W;
        const x1 = end * GRAPH_W;
        if (i % 2 === 1) {
          ctx.fillStyle = "rgba(233, 238, 243, 0.035)";
          ctx.fillRect(x0, 0, x1 - x0, plotH);
        }
        if (i === deltaHistory.currentSector) {
          ctx.fillStyle = "rgba(233, 238, 243, 0.05)";
          ctx.fillRect(x0, 0, x1 - x0, plotH);
        }
        if (x1 - x0 >= 12) {
          ctx.fillStyle = i === deltaHistory.currentSector ? ink : ink3;
          ctx.fillText(String(i + 1), (x0 + x1) / 2, GRAPH_H - 3);
        }
      });

      // Previous lap's line, dashed, for context.
      if (showPrevious) {
        const { path } = tracePath(h.previous);
        ctx.setLineDash([3, 3]);
        ctx.strokeStyle = "rgba(233, 238, 243, 0.4)";
        ctx.lineWidth = 1.25;
        ctx.stroke(path);
        ctx.setLineDash([]);
      }

      // This lap: filled to the baseline, red above, green below.
      const { path, first, lastBin } = tracePath(h.current);
      if (first >= 0) {
        const area = new Path2D(path);
        area.lineTo(x(lastBin), mid);
        area.lineTo(x(first), mid);
        area.closePath();
        for (const [top, height, col] of [
          [0, mid, loss],
          [mid, plotH - mid, gain],
        ] as const) {
          ctx.save();
          ctx.beginPath();
          ctx.rect(0, top, GRAPH_W, height);
          ctx.clip();
          const grad = ctx.createLinearGradient(0, top === 0 ? 0 : plotH, 0, mid);
          grad.addColorStop(0, hexAlpha(col, 0.55));
          grad.addColorStop(1, hexAlpha(col, 0.12));
          ctx.fillStyle = grad;
          ctx.fill(area);
          ctx.strokeStyle = col;
          ctx.lineWidth = 2;
          ctx.lineJoin = "round";
          ctx.stroke(path);
          ctx.restore();
        }
      }

      // Baseline: our current lap.
      ctx.fillStyle = "rgba(233, 238, 243, 0.55)";
      ctx.fillRect(0, Math.round(mid) - 0.5, GRAPH_W, 1);

      // Where we are now.
      const px = Math.round(deltaHistory.pct * GRAPH_W) + 0.5;
      ctx.fillStyle = ink;
      ctx.fillRect(px - 0.5, 0, 1, plotH);

      // Scale hint.
      ctx.textAlign = "left";
      ctx.fillStyle = ink3;
      ctx.font = `500 10px ${font}`;
      ctx.fillText(`±${range.toFixed(2)}s`, 4, 11);

      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [mode, showPrevious]);

  return <canvas ref={canvas} className="ds-graph" style={{ width: GRAPH_W, height: GRAPH_H }} />;
}

function hexAlpha(color: string, alpha: number) {
  const m = /^#([0-9a-f]{6})$/i.exec(color);
  if (!m) return color;
  const n = parseInt(m[1], 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}
