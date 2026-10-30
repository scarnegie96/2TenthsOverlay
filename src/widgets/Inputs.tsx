import { useEffect, useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { gear, speed, speedUnit } from "../lib/format";
import { useLive } from "../lib/store";
import type { TickPayload } from "../lib/types";
import { Panel } from "./Panel";

const TRACE_W = 236;
const TRACE_H = 78;

export function Inputs({ config }: { config: WidgetConfig }) {
  const tick = useLive((s) => s.tick);
  const units = useLive((s) => s.config.general.units);
  const shiftRpm = useLive((s) => s.session?.shiftRpm ?? 0);
  const showClutch = opt(config, "showClutch", false);
  const showSteering = opt(config, "showSteering", true);
  const seconds = opt(config, "traceSeconds", 6);

  const shift = shiftRpm > 0 && (tick?.rpm ?? 0) >= shiftRpm;

  return (
    <Panel className="inputs">
      <div className="inputs-body">
        <Trace seconds={seconds} />
        <div className="pedals">
          {showClutch && <Bar label="C" value={tick?.clutch ?? 0} kind="clutch" />}
          <Bar label="B" value={tick?.brake ?? 0} kind="brake" />
          <Bar label="T" value={tick?.throttle ?? 0} kind="throttle" />
        </div>
        <div className={`gearbox ${shift ? "is-shift" : ""}`}>
          <span className="gear">{gear(tick?.gear ?? 0)}</span>
          <span className="speed">
            {speed(tick?.speed ?? 0, units)}
            <small>{speedUnit(units)}</small>
          </span>
          {showSteering && <Steering angle={tick?.steering ?? 0} />}
        </div>
      </div>
    </Panel>
  );
}

function Bar({ label, value, kind }: { label: string; value: number; kind: string }) {
  const pct = Math.round(Math.min(1, Math.max(0, value)) * 100);
  return (
    <div className={`bar bar-${kind}`}>
      <div className="bar-track">
        <div className="bar-fill" style={{ transform: `scaleY(${pct / 100})` }} />
      </div>
      <span className="bar-label">{label}</span>
    </div>
  );
}

function Steering({ angle }: { angle: number }) {
  // iRacing: positive = left. Rotate the mark so it matches the wheel.
  const deg = (-angle * 180) / Math.PI;
  return (
    <svg className="steer" viewBox="0 0 24 24" aria-hidden>
      <g style={{ transform: `rotate(${deg}deg)`, transformOrigin: "12px 12px" }}>
        <circle cx="12" cy="12" r="9.5" fill="none" strokeWidth="2.2" />
        <line x1="12" y1="2.5" x2="12" y2="7" strokeWidth="2.6" className="steer-top" />
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
    const gridColor = css.getPropertyValue("--grid").trim() || "rgba(255,255,255,.06)";

    const samples: { t: number; th: number; br: number }[] = [];
    let raf = 0;

    const unsub = useLive.subscribe((s, prev) => {
      if (s.tick && s.tick !== prev.tick) push(s.tick);
    });
    function push(t: TickPayload) {
      samples.push({ t: t.sessionTime, th: t.throttle, br: t.brake });
      const cutoff = t.sessionTime - seconds;
      while (samples.length && (samples[0].t < cutoff || samples[0].t > t.sessionTime)) samples.shift();
    }

    const pad = 4;
    const y = (v: number) => pad + (1 - v) * (TRACE_H - pad * 2);
    const line = (key: "th" | "br", color: string, now: number) => {
      ctx.beginPath();
      samples.forEach((s, i) => {
        const x = TRACE_W - ((now - s.t) / seconds) * TRACE_W;
        if (i === 0) ctx.moveTo(x, y(s[key]));
        else ctx.lineTo(x, y(s[key]));
      });
      ctx.strokeStyle = color;
      ctx.lineWidth = 2;
      ctx.lineJoin = "round";
      ctx.stroke();
    };

    const draw = () => {
      ctx.clearRect(0, 0, TRACE_W, TRACE_H);
      ctx.strokeStyle = gridColor;
      ctx.lineWidth = 1;
      for (const v of [0.25, 0.5, 0.75]) {
        ctx.beginPath();
        ctx.moveTo(0, Math.round(y(v)) + 0.5);
        ctx.lineTo(TRACE_W, Math.round(y(v)) + 0.5);
        ctx.stroke();
      }
      const now = samples.length ? samples[samples.length - 1].t : 0;
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
  }, [seconds]);

  return <canvas ref={canvas} className="trace" style={{ width: TRACE_W, height: TRACE_H }} />;
}
