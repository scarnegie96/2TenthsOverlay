import { useEffect, useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { gear, speed } from "../lib/format";
import { useLive } from "../lib/store";
import type { TickPayload } from "../lib/types";
import { Panel } from "./Panel";

const TRACE_W = 300;
const TRACE_H = 92;

/**
 * Driver HUD strip, left to right: gear, speed, steering wheel, pedal trace,
 * pedal bars. Big readouts on the left, detail on the right. With a reference
 * lap loaded, its inputs at the same spot show as a purple ghost on each part.
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

  const shift = shiftRpm > 0 && (tick?.rpm ?? 0) >= shiftRpm;
  const ghost = showGhost ? tick?.coach ?? null : null;
  const gearOff = ghost?.refGear != null && tick != null && tick.gear > 0 && ghost.refGear !== tick.gear;
  const sd = ghost?.speedDelta ?? null;

  return (
    <Panel className={`inputs ${ghost ? "has-ghost" : ""}`}>
      <div className="inputs-body">
        <div className="hud-gear-wrap">
          <span className={`hud-gear ${shift ? "is-shift" : ""}`}>{gear(tick?.gear ?? 0)}</span>
          {ghost?.refGear != null && (
            <span className={`hud-ghost ${gearOff ? "is-off" : ""}`} title="Reference gear">
              ref {gear(ghost.refGear)}
            </span>
          )}
        </div>
        <div className="hud-speed">
          <span className="hud-speed-value">{speed(tick?.speed ?? 0, units)}</span>
          <span className="hud-speed-unit">{units === "imperial" ? "MPH" : "KPH"}</span>
          {ghost?.refSpeed != null && (
            <span
              className={`hud-ghost ${sd != null && sd < -0.5 ? "is-down" : sd != null && sd > 0.5 ? "is-up" : ""}`}
              title="Reference speed here"
            >
              ref {speed(ghost.refSpeed, units)}
            </span>
          )}
        </div>
        {showSteering && (
          <Wheel
            angle={tick?.steering ?? 0}
            ghost={ghost?.refSteering ?? null}
            estimated={ghost?.steeringEstimated ?? false}
          />
        )}
        {showTrace && <Trace seconds={seconds} />}
        <div className="pedals">
          {showClutch && <Bar label="Clutch" value={tick?.clutch ?? 0} kind="clutch" />}
          <Bar label="Brake" value={tick?.brake ?? 0} kind="brake" ghost={ghost?.refBrake ?? null} />
          <Bar label="Throttle" value={tick?.throttle ?? 0} kind="throttle" ghost={ghost?.refThrottle ?? null} />
        </div>
      </div>
    </Panel>
  );
}

function Bar({ label, value, kind, ghost = null }: { label: string; value: number; kind: string; ghost?: number | null }) {
  const pct = Math.round(Math.min(1, Math.max(0, value)) * 100);
  return (
    <div className={`bar bar-${kind} ${pct > 0 ? "is-active" : ""}`} aria-label={`${label} ${pct}%`} title={label}>
      <span className="bar-pct">{pct}</span>
      <div className="bar-track">
        <div className="bar-fill" style={{ transform: `scaleY(${pct / 100})` }} />
        {ghost != null && (
          <div
            className="bar-ghost"
            style={{ bottom: `${Math.min(1, Math.max(0, ghost)) * 100}%` }}
            title={`Reference ${Math.round(ghost * 100)}%`}
          />
        )}
      </div>
    </div>
  );
}

/**
 * A wheel that turns with yours; the red mark is 12 o'clock at centre.
 * The purple ghost is the reference's wheel (dashed when it's estimated
 * from the reference's line rather than recorded).
 */
function Wheel({ angle, ghost, estimated }: { angle: number; ghost: number | null; estimated: boolean }) {
  // iRacing: radians, positive = turned left (anticlockwise).
  const deg = (-angle * 180) / Math.PI;
  const ghostDeg = ghost == null ? null : (-ghost * 180) / Math.PI;
  return (
    <svg className="hud-wheel" viewBox="0 0 100 100" aria-label={`Steering ${Math.round(-deg)}°`}>
      {ghostDeg != null && (
        <g className={`wheel-ghost ${estimated ? "is-estimated" : ""}`} style={{ transform: `rotate(${ghostDeg}deg)`, transformOrigin: "50px 50px" }}>
          <circle cx="50" cy="50" r="46.5" />
          <path d="M 50 1.5 L 45.5 9 L 54.5 9 Z" />
        </g>
      )}
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
function Trace({ seconds }: { seconds: number }) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2; // stays crisp when scaled up
    el.width = TRACE_W * dpr;
    el.height = TRACE_H * dpr;
    ctx.scale(dpr, dpr);

    const css = getComputedStyle(el);
    const throttleColor = css.getPropertyValue("--throttle").trim() || "#4be08f";
    const brakeColor = css.getPropertyValue("--brake").trim() || "#ff5468";
    const gridColor = css.getPropertyValue("--grid").trim() || "rgba(255,255,255,.07)";

    const ghostColor = css.getPropertyValue("--fastest").trim() || "#b98cff";
    const samples: { t: number; th: number; br: number; gth: number | null; gbr: number | null }[] = [];
    let raf = 0;

    const unsub = useLive.subscribe((s, prev) => {
      if (s.tick && s.tick !== prev.tick) push(s.tick);
    });
    function push(t: TickPayload) {
      samples.push({
        t: t.sessionTime,
        th: t.throttle,
        br: t.brake,
        gth: t.coach?.refThrottle ?? null,
        gbr: t.coach?.refBrake ?? null,
      });
      const cutoff = t.sessionTime - seconds - 0.1;
      while (samples.length && (samples[0].t < cutoff || samples[0].t > t.sessionTime)) samples.shift();
    }

    const pad = 4;
    const y = (v: number) => pad + (1 - v) * (TRACE_H - pad * 2);
    const x = (t: number, now: number) => TRACE_W - ((now - t) / seconds) * TRACE_W;
    const line = (key: "th" | "br" | "gth" | "gbr", color: string, now: number, ghost = false) => {
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
      ctx.lineWidth = ghost ? 1.5 : 2.5;
      ctx.globalAlpha = ghost ? 0.85 : 1;
      ctx.setLineDash(ghost ? [4, 3] : []);
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      ctx.stroke();
      ctx.globalAlpha = 1;
      ctx.setLineDash([]);
    };

    const draw = () => {
      ctx.clearRect(0, 0, TRACE_W, TRACE_H);
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
        ctx.lineTo(px, TRACE_H);
        ctx.stroke();
      }

      if (samples.length > 1) {
        // Reference inputs first, dashed purple, so ours draw on top.
        line("gth", ghostColor, now, true);
        line("gbr", ghostColor, now, true);
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
  }, [seconds]);

  return <canvas ref={canvas} className="trace" style={{ width: TRACE_W, height: TRACE_H }} />;
}
