import { useEffect, useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { gear, speed, type Units } from "../lib/format";
import { useLive } from "../lib/store";
import type { TickPayload } from "../lib/types";
import { Panel } from "./Panel";

const TRACE_W = 300;
const TRACE_H = 92;
const GHOST_TRACE_H = 62;

type Source = "ours" | "reference";

/**
 * Driver HUD strip, left to right: gear, speed, steering wheel, pedal trace,
 * pedal bars. With a reference lap loaded, a slimmer strip extends out of the
 * top showing the reference's inputs at the same spot, laid out on the same
 * columns so the two traces line up.
 */
export function Inputs({ config }: { config: WidgetConfig }) {
  const tick = useLive((s) => s.tick);
  const units = useLive((s) => s.config.general.units);
  const shiftRpm = useLive((s) => s.session?.shiftRpm ?? 0);
  const showClutch = opt(config, "showClutch", true);
  const showSteering = opt(config, "showSteering", true);
  const showTrace = opt(config, "showTrace", true);
  const seconds = opt(config, "traceSeconds", 6);
  const showGhost = opt(config, "showGhost", true);

  const ref = showGhost ? tick?.coach ?? null : null;
  const layout = { showClutch, showSteering, showTrace, seconds, units };

  return (
    <div className={`inputs-stack ${ref ? "has-ghost" : ""}`}>
      {ref && (
        <Panel className="inputs inputs-ghost">
          <Strip
            {...layout}
            source="reference"
            label="Ref"
            gear={ref.refGear ?? 0}
            speedMs={ref.refSpeed ?? 0}
            steering={ref.refSteering}
            steeringNote={ref.refSteering == null ? "No wheel data" : null}
            throttle={ref.refThrottle ?? 0}
            brake={ref.refBrake ?? 0}
            clutch={null}
            shift={false}
          />
        </Panel>
      )}
      <Panel className="inputs inputs-main">
        <Strip
          {...layout}
          source="ours"
          label={null}
          gear={tick?.gear ?? 0}
          speedMs={tick?.speed ?? 0}
          steering={tick?.steering ?? 0}
          steeringNote={null}
          throttle={tick?.throttle ?? 0}
          brake={tick?.brake ?? 0}
          clutch={tick?.clutch ?? 0}
          shift={shiftRpm > 0 && (tick?.rpm ?? 0) >= shiftRpm}
        />
      </Panel>
    </div>
  );
}

interface StripProps {
  source: Source;
  /** Small caption under the gear, e.g. "Ref". */
  label: string | null;
  gear: number;
  speedMs: number;
  steering: number | null;
  steeringNote: string | null;
  throttle: number;
  brake: number;
  /** null hides the clutch bar but keeps its space, so columns stay aligned. */
  clutch: number | null;
  shift: boolean;
  showClutch: boolean;
  showSteering: boolean;
  showTrace: boolean;
  seconds: number;
  units: Units;
}

/** One row of the HUD. Used for both your inputs and the reference's. */
function Strip(p: StripProps) {
  return (
    <div className="inputs-body">
      <div className="hud-gear-wrap">
        <span className={`hud-gear ${p.shift ? "is-shift" : ""}`}>{gear(p.gear)}</span>
        {p.label && <span className="hud-caption">{p.label}</span>}
      </div>
      <div className="hud-speed">
        <span className="hud-speed-value">{speed(p.speedMs, p.units)}</span>
        <span className="hud-speed-unit">{p.units === "imperial" ? "MPH" : "KPH"}</span>
      </div>
      {p.showSteering && (
        <div className="hud-wheel-wrap">
          {p.steering == null ? <div className="hud-wheel" /> : <Wheel angle={p.steering} />}
          {p.steeringNote && <span className="hud-wheel-note">{p.steeringNote}</span>}
        </div>
      )}
      {p.showTrace && <Trace seconds={p.seconds} source={p.source} />}
      <div className="pedals">
        {p.showClutch &&
          (p.clutch == null ? <div className="bar bar-spacer" /> : <Bar label="Clutch" value={p.clutch} kind="clutch" />)}
        <Bar label="Brake" value={p.brake} kind="brake" />
        <Bar label="Throttle" value={p.throttle} kind="throttle" />
      </div>
    </div>
  );
}

function Bar({ label, value, kind }: { label: string; value: number; kind: string }) {
  const pct = Math.round(Math.min(1, Math.max(0, value)) * 100);
  return (
    <div className={`bar bar-${kind} ${pct > 0 ? "is-active" : ""}`} aria-label={`${label} ${pct}%`} title={label}>
      <span className="bar-pct">{pct}</span>
      <div className="bar-track">
        <div className="bar-fill" style={{ transform: `scaleY(${pct / 100})` }} />
      </div>
    </div>
  );
}

/** A wheel that turns with yours. The red mark is 12 o'clock at centre. */
function Wheel({ angle }: { angle: number }) {
  // iRacing: radians, positive = turned left (anticlockwise).
  const deg = (-angle * 180) / Math.PI;
  return (
    <svg className="hud-wheel" viewBox="0 0 100 100" aria-label={`Steering ${Math.round(-deg)}°`}>
      <g style={{ transform: `rotate(${deg}deg)`, transformOrigin: "50px 50px" }}>
        <circle className="wheel-rim" cx="50" cy="50" r="41" />
        {/* Spokes: left, right and down to a centre hub. */}
        <path className="wheel-spoke" d="M 10 52 Q 30 46 40 50 L 60 50 Q 70 46 90 52" />
        <path className="wheel-spoke" d="M 44 55 L 47 89 L 53 89 L 56 55 Z" />
        <rect className="wheel-hub" x="37" y="44" width="26" height="16" rx="5" />
        <path className="wheel-mark" d="M 50 9 m -4.5 0 a 41 41 0 0 1 9 0" />
      </g>
    </svg>
  );
}

/** Rolling throttle/brake trace drawn on a canvas, outside React's render loop. */
function Trace({ seconds, source }: { seconds: number; source: Source }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const height = source === "reference" ? GHOST_TRACE_H : TRACE_H;

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2; // stays crisp when scaled up
    el.width = TRACE_W * dpr;
    el.height = height * dpr;
    ctx.scale(dpr, dpr);

    const css = getComputedStyle(el);
    const throttleColor = css.getPropertyValue("--throttle").trim() || "#4be08f";
    const brakeColor = css.getPropertyValue("--brake").trim() || "#ff5468";
    const gridColor = css.getPropertyValue("--grid").trim() || "rgba(255,255,255,.07)";

    const samples: { t: number; th: number | null; br: number | null }[] = [];
    let raf = 0;

    const unsub = useLive.subscribe((s, prev) => {
      if (s.tick && s.tick !== prev.tick) push(s.tick);
    });
    function push(t: TickPayload) {
      // The reference's inputs at the spot we're at, so both traces share a time axis.
      const th = source === "ours" ? t.throttle : t.coach?.refThrottle ?? null;
      const br = source === "ours" ? t.brake : t.coach?.refBrake ?? null;
      samples.push({ t: t.sessionTime, th, br });
      const cutoff = t.sessionTime - seconds - 0.1;
      while (samples.length && (samples[0].t < cutoff || samples[0].t > t.sessionTime)) samples.shift();
    }

    const pad = 4;
    const y = (v: number) => pad + (1 - v) * (height - pad * 2);
    const x = (t: number, now: number) => TRACE_W - ((now - t) / seconds) * TRACE_W;
    const line = (key: "th" | "br", color: string, now: number) => {
      ctx.beginPath();
      let open = false;
      for (const s of samples) {
        const v = s[key];
        if (v == null) {
          open = false;
          continue;
        }
        if (!open) ctx.moveTo(x(s.t, now), y(v));
        else ctx.lineTo(x(s.t, now), y(v));
        open = true;
      }
      ctx.strokeStyle = color;
      ctx.lineWidth = source === "ours" ? 2.5 : 2;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      ctx.stroke();
    };

    const draw = () => {
      ctx.clearRect(0, 0, TRACE_W, height);
      const now = samples.length ? samples[samples.length - 1].t : 0;

      ctx.strokeStyle = gridColor;
      ctx.lineWidth = 1;
      // Dashed level lines at 25 / 50 / 75 %.
      ctx.setLineDash([3, 4]);
      for (const v of [0.25, 0.5, 0.75]) {
        ctx.beginPath();
        ctx.moveTo(0, Math.round(y(v)) + 0.5);
        ctx.lineTo(TRACE_W, Math.round(y(v)) + 0.5);
        ctx.stroke();
      }
      // Solid time lines that scroll with the trace, one per second (or two).
      ctx.setLineDash([]);
      const step = seconds > 10 ? 2 : 1;
      for (let t = Math.floor(now / step) * step; t > now - seconds; t -= step) {
        const px = Math.round(x(t, now)) + 0.5;
        ctx.beginPath();
        ctx.moveTo(px, 0);
        ctx.lineTo(px, height);
        ctx.stroke();
      }

      if (samples.length > 1) {
        line("th", throttleColor, now);
        line("br", brakeColor, now);
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      unsub();
    };
  }, [seconds, source, height]);

  return <canvas ref={canvas} className="trace" style={{ width: TRACE_W, height }} />;
}
