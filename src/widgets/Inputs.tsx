import { useEffect, useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { gear, speed, type Units } from "../lib/format";
import { useLive } from "../lib/store";
import { cssColor, paletteKey } from "../lib/theme";
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
  const shiftRpm = opt(config, "shiftLight", true) ? tick?.shiftRpm ?? null : null;
  const showClutch = opt(config, "showClutch", true);
  const showSteering = opt(config, "showSteering", true);
  const showTrace = opt(config, "showTrace", true);
  const seconds = opt(config, "traceSeconds", 6);
  const showGhost = opt(config, "showGhost", true);
  const carName = useLive((s) => s.session?.carName ?? "");
  const wheel = resolveWheel(opt<string>(config, "wheel", "auto"), carName);
  // Shift lights on the wheels that have them: fill from 75% of the shift point.
  const rpm = tick?.rpm ?? 0;
  const leds = tick?.shiftRpm ? Math.max(0, Math.min(1, (rpm / tick.shiftRpm - 0.75) / 0.25)) : 0;

  const ref = showGhost ? tick?.coach ?? null : null;
  const layout = { showClutch, showSteering, showTrace, seconds, units, wheel };

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
            leds={0}
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
          shift={shiftRpm != null && (tick?.rpm ?? 0) >= shiftRpm}
          leds={leds}
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
  /** Shift lights, 0..1 (wheels that have them). */
  leds: number;
  wheel: WheelStyle;
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
          {p.steering == null ? <div className="hud-wheel" /> : <Wheel angle={p.steering} style={p.wheel} leds={p.leds} />}
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

export type WheelStyle = "round" | "gt" | "formula" | "oval";

/**
 * Picks a wheel for the car when the option is "auto", from words in its
 * name. Anything unrecognised gets the round road-car wheel.
 */
export function resolveWheel(option: string, carName: string): WheelStyle {
  if (option === "round" || option === "gt" || option === "formula" || option === "oval") return option;
  const n = carName.toLowerCase();
  // Whole words only, so "r8" doesn't match "GR86".
  const has = (...words: string[]) =>
    words.some((w) => new RegExp(`(^|[^a-z0-9])${w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}($|[^a-z0-9])`).test(n));
  if (has("formula", "f1", "f2", "f3", "f4", "fr2.0", "ff1600", "ir18", "ir-18", "indycar", "dallara", "super formula", "sf23",
    "w12", "w13", "mp4", "lotus 79", "lotus 49", "skip barber", "pro mazda", "usf", "indy pro", "porsche 919", "lmp", "gtp", "hypercar")) {
    return "formula";
  }
  if (has("nascar", "cup", "xfinity", "truck", "late model", "modified", "sprint car", "midget", "legends", "street stock", "arca", "super late")) {
    // "Cup" also matches one-make sports car cups.
    if (has("porsche", "carrera", "992", "991", "radical", "mx-5", "gr86", "clio")) return has("porsche", "992", "991") ? "gt" : "round";
    return "oval";
  }
  if (has("gt3", "gt4", "gte", "gtd", "gt1", "gt2", "992", "991", "rsr", "evo", "amg", "m4", "nsx", "huracan", "r8", "296", "720s")) return "gt";
  return "round";
}

/**
 * A wheel that turns with yours. The red mark is 12 o'clock at centre.
 * Styles: a round road-car wheel, a GT wheel (flat top and bottom, button
 * plate), a formula wheel (yoke with a display and shift lights) and an oval
 * wheel (big, thin, two low spokes).
 */
function Wheel({ angle, style, leds }: { angle: number; style: WheelStyle; leds: number }) {
  // iRacing: radians, positive = turned left (anticlockwise).
  const deg = (-angle * 180) / Math.PI;
  return (
    <svg className={`hud-wheel wheel-${style}`} viewBox="0 0 100 100" aria-label={`Steering ${Math.round(-deg)}°`}>
      <g style={{ transform: `rotate(${deg}deg)`, transformOrigin: "50px 50px" }}>
        {style === "round" && <RoundWheel />}
        {style === "gt" && <GtWheel leds={leds} />}
        {style === "formula" && <FormulaWheel leds={leds} />}
        {style === "oval" && <OvalWheel />}
      </g>
    </svg>
  );
}

function RoundWheel() {
  return (
    <>
      <circle className="wheel-rim" cx="50" cy="50" r="41" />
      {/* Spokes: left, right and down to a centre hub. */}
      <path className="wheel-spoke" d="M 10 52 Q 30 46 40 50 L 60 50 Q 70 46 90 52" />
      <path className="wheel-spoke wheel-fill" d="M 44 55 L 47 89 L 53 89 L 56 55 Z" />
      <rect className="wheel-hub" x="37" y="44" width="26" height="16" rx="5" />
      <path className="wheel-mark" d="M 50 9 m -4.5 0 a 41 41 0 0 1 9 0" />
    </>
  );
}

/** Lights across the top of a wheel: green, then amber, then red. */
function ShiftLights({ leds, y, x0, x1, count }: { leds: number; y: number; x0: number; x1: number; count: number }) {
  const lit = Math.round(leds * count);
  const flash = leds >= 1;
  return (
    <>
      {Array.from({ length: count }, (_, i) => {
        const x = x0 + ((x1 - x0) * (i + 0.5)) / count;
        const kind = flash ? "red" : i < count * 0.4 ? "green" : i < count * 0.75 ? "amber" : "red";
        return <circle key={i} className={`wheel-led led-${kind} ${i < lit ? "is-on" : ""}`} cx={x} cy={y} r={2.1} />;
      })}
    </>
  );
}

function GtWheel({ leds }: { leds: number }) {
  return (
    <>
      {/* Rim: round at the sides, flat across the top and bottom. */}
      <path className="wheel-rim" d="M 28 15 L 72 15 Q 91 17 91 50 Q 91 81 74 86 L 26 86 Q 9 81 9 50 Q 9 17 28 15 Z" />
      {/* Thicker grips where your hands go. */}
      <path className="wheel-grip" d="M 13 34 Q 9 50 13 68" />
      <path className="wheel-grip" d="M 87 34 Q 91 50 87 68" />
      {/* Button plate joining the rim at the sides and bottom. */}
      <path className="wheel-plate" d="M 13 44 L 30 38 L 70 38 L 87 44 L 87 58 L 66 72 L 58 86 L 42 86 L 34 72 L 13 58 Z" />
      <rect className="wheel-screen" x="40" y="43" width="20" height="11" rx="1.5" />
      {[30, 70].map((x) => [48, 58].map((y) => <circle key={`${x}${y}`} className="wheel-button" cx={x} cy={y} r={2.6} />))}
      <circle className="wheel-button" cx="44" cy="64" r={2.2} />
      <circle className="wheel-button" cx="56" cy="64" r={2.2} />
      <ShiftLights leds={leds} y={33} x0={34} x1={66} count={8} />
      <path className="wheel-mark" d="M 46 15 L 54 15" />
    </>
  );
}

function FormulaWheel({ leds }: { leds: number }) {
  return (
    <>
      {/* Grips either side of a wide body; no rim across the top or bottom. */}
      <rect className="wheel-grip-block" x="5" y="28" width="15" height="46" rx="7" />
      <rect className="wheel-grip-block" x="80" y="28" width="15" height="46" rx="7" />
      <path className="wheel-plate" d="M 17 30 Q 17 24 24 24 L 76 24 Q 83 24 83 30 L 83 64 Q 83 72 74 76 L 60 80 L 40 80 L 26 76 Q 17 72 17 64 Z" />
      <rect className="wheel-screen" x="34" y="36" width="32" height="18" rx="2" />
      <ShiftLights leds={leds} y={30} x0={26} x1={74} count={10} />
      {/* Rotary switches and buttons. */}
      <circle className="wheel-button" cx="27" cy="44" r={3.6} />
      <circle className="wheel-button" cx="73" cy="44" r={3.6} />
      <circle className="wheel-button" cx="40" cy="66" r={3.2} />
      <circle className="wheel-button" cx="60" cy="66" r={3.2} />
      <circle className="wheel-button" cx="27" cy="58" r={2.4} />
      <circle className="wheel-button" cx="73" cy="58" r={2.4} />
      <path className="wheel-mark" d="M 46 24 L 54 24" />
    </>
  );
}

function OvalWheel() {
  return (
    <>
      <circle className="wheel-rim wheel-rim-thin" cx="50" cy="50" r="43" />
      {/* Two spokes dropping to a low, dished hub. */}
      <path className="wheel-spoke" d="M 9 58 Q 30 60 42 64" />
      <path className="wheel-spoke" d="M 91 58 Q 70 60 58 64" />
      <circle className="wheel-hub" cx="50" cy="64" r="9" />
      <path className="wheel-mark" d="M 50 7 m -4.5 0 a 43 43 0 0 1 9 0" />
    </>
  );
}

/** Rolling throttle/brake trace drawn on a canvas, outside React's render loop. */
function Trace({ seconds, source }: { seconds: number; source: Source }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const palette = useLive((s) => paletteKey(s.config.general));
  const height = source === "reference" ? GHOST_TRACE_H : TRACE_H;

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2; // stays crisp when scaled up
    el.width = TRACE_W * dpr;
    el.height = height * dpr;
    ctx.scale(dpr, dpr);

    const throttleColor = cssColor(el, "var(--throttle)");
    const brakeColor = cssColor(el, "var(--brake)");
    const gridColor = cssColor(el, "var(--grid)");

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
  }, [seconds, source, height, palette]);

  return <canvas ref={canvas} className="trace" style={{ width: TRACE_W, height }} />;
}
