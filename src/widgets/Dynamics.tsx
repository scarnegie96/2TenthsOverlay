import { useEffect, useRef, useState } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { useLive } from "../lib/store";
import type { TickPayload } from "../lib/types";
import { Panel } from "./Panel";

const METER = 176;
const TRAIL_S = 1.2;
const ENVELOPE_BUCKETS = 48;

/**
 * Car dynamics: a g-meter (with the grip you've used recently as a faint
 * envelope), the car's attitude seen from above (slip), the side (pitch) and
 * behind (roll), and an understeer / oversteer balance bar.
 */
export function Dynamics({ config }: { config: WidgetConfig }) {
  const dyn = useLive((s) => s.tick?.dynamics ?? null);
  const maxG = opt(config, "maxG", 3);
  const felt = opt<string>(config, "direction", "felt") === "felt";
  const exaggerate = opt(config, "exaggerate", 4);
  const showAttitude = opt(config, "showAttitude", true);
  const showBalance = opt(config, "showBalance", true);

  // Hold the last balance reading through straights, dimmed.
  const [lastBalance, setLastBalance] = useState<number | null>(null);
  useEffect(() => {
    if (dyn?.balance != null) setLastBalance(dyn.balance);
  }, [dyn?.balance]);
  const balance = dyn?.balance ?? lastBalance;
  const live = dyn?.balance != null;

  const lat = dyn?.latG ?? 0;
  const long = dyn?.longG ?? 0;
  const total = Math.hypot(lat, long);

  return (
    <Panel className="dynamics" title="Car dynamics" aside={`${total.toFixed(2)} g`}>
      <div className="dyn-body">
        <div className="dyn-meter">
          <GMeter maxG={maxG} felt={felt} />
          <div className="dyn-readouts">
            <Readout label="Lateral" value={Math.abs(lat)} unit="g" />
            <Readout label={long < 0 ? "Braking" : "Accel"} value={Math.abs(long)} unit="g" />
            <Readout label="Vertical" value={dyn?.vertG ?? 1} unit="g" />
          </div>
        </div>
        {showAttitude && (
          <div className="dyn-side">
            <div className="dyn-views">
              <View label="Slip" value={dyn?.slipDeg ?? 0} title="Angle between where the car points and where it's going">
                <TopView slip={dyn?.slipDeg ?? 0} />
              </View>
              <View label="Pitch" value={dyn?.pitchDeg ?? 0} title="Nose up (+) or down (−)">
                <SideView pitch={(dyn?.pitchDeg ?? 0) * exaggerate} />
              </View>
              <View label="Roll" value={dyn?.rollDeg ?? 0} title="Leaning right (+) or left (−)">
                <RearView roll={(dyn?.rollDeg ?? 0) * exaggerate} />
              </View>
            </div>
            {showBalance && <Balance value={balance} live={live} />}
          </div>
        )}
      </div>
    </Panel>
  );
}

function Readout({ label, value, unit }: { label: string; value: number; unit: string }) {
  return (
    <div className="dyn-readout">
      <span className="dyn-readout-label">{label}</span>
      <span className="dyn-readout-value">
        {value.toFixed(2)}
        <small>{unit}</small>
      </span>
    </div>
  );
}

function View({ label, value, title, children }: { label: string; value: number; title: string; children: React.ReactNode }) {
  return (
    <div className="dyn-view" title={title}>
      <svg viewBox="-40 -40 80 80" className="dyn-view-svg" aria-hidden>
        {children}
      </svg>
      <span className="dyn-view-label">
        {label} <b>{value >= 0 ? "" : "−"}{Math.abs(value).toFixed(1)}°</b>
      </span>
    </div>
  );
}

/** From above: the dashed line is the direction of travel; the car turns away from it as it slides. */
function TopView({ slip }: { slip: number }) {
  const sliding = Math.abs(slip) > 3;
  return (
    <>
      <line className="dyn-travel" x1="0" y1="34" x2="0" y2="-34" />
      <path className="dyn-travel-head" d="M 0 -36 L -4 -29 L 4 -29 Z" />
      <g style={{ transform: `rotate(${slip}deg)` }} className={sliding ? "is-sliding" : ""}>
        <rect className="dyn-car" x="-9" y="-22" width="18" height="44" rx="6" />
        <rect className="dyn-car-detail" x="-6" y="-10" width="12" height="9" rx="2" />
        <rect className="dyn-wheel" x="-12" y="-17" width="4" height="9" rx="1" />
        <rect className="dyn-wheel" x="8" y="-17" width="4" height="9" rx="1" />
        <rect className="dyn-wheel" x="-12" y="9" width="4" height="9" rx="1" />
        <rect className="dyn-wheel" x="8" y="9" width="4" height="9" rx="1" />
      </g>
    </>
  );
}

/** From the side, front to the right; tilt exaggerated so small angles are visible. */
function SideView({ pitch }: { pitch: number }) {
  return (
    <>
      <line className="dyn-ground" x1="-36" y1="14" x2="36" y2="14" />
      <g style={{ transform: `rotate(${-pitch}deg)` }}>
        <path className="dyn-car" d="M -32 4 L -30 -6 L -12 -8 L -4 -16 L 12 -16 L 20 -8 L 32 -5 L 33 4 Z" />
        <circle className="dyn-wheel" cx="-19" cy="6" r="6" />
        <circle className="dyn-wheel" cx="20" cy="6" r="6" />
      </g>
    </>
  );
}

/** From behind; roll exaggerated like pitch. */
function RearView({ roll }: { roll: number }) {
  return (
    <>
      <line className="dyn-ground" x1="-36" y1="14" x2="36" y2="14" />
      <g style={{ transform: `rotate(${roll}deg)` }}>
        <path className="dyn-car" d="M -26 6 L -26 -4 L -16 -8 L -10 -18 L 10 -18 L 16 -8 L 26 -4 L 26 6 Z" />
        <rect className="dyn-car-detail" x="-14" y="-3" width="28" height="4" rx="1" />
        <rect className="dyn-wheel" x="-30" y="0" width="9" height="14" rx="2" />
        <rect className="dyn-wheel" x="21" y="0" width="9" height="14" rx="2" />
      </g>
    </>
  );
}

function Balance({ value, live }: { value: number | null; live: boolean }) {
  const v = value ?? 0;
  const tone = value == null ? "" : v < -0.25 ? "is-under" : v > 0.25 ? "is-over" : "is-neutral";
  const word = value == null ? "Learning your car" : v < -0.25 ? "Understeer" : v > 0.25 ? "Oversteer" : "Neutral";
  return (
    <div className={`dyn-balance ${tone} ${live ? "is-live" : ""}`} title="Estimated from how much the car rotates for the steering you put in">
      <div className="dyn-balance-head">
        <span>Balance</span>
        <b>{word}</b>
      </div>
      <div className="dyn-balance-track">
        <span className="dyn-balance-mid" />
        {value != null && <span className="dyn-balance-dot" style={{ left: `${50 + v * 50}%` }} />}
      </div>
      <div className="dyn-balance-ends">
        <span>Under</span>
        <span>Over</span>
      </div>
    </div>
  );
}

/**
 * The g-meter, drawn on a canvas every frame. "Felt" puts the dot where the
 * force pushes you (braking moves it up, a left turn moves it right).
 */
function GMeter({ maxG, felt }: { maxG: number; felt: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2;
    el.width = METER * dpr;
    el.height = METER * dpr;
    ctx.scale(dpr, dpr);

    const css = getComputedStyle(el);
    const v = (n: string, f: string) => css.getPropertyValue(n).trim() || f;
    const ink = v("--ink", "#e9eef3");
    const ink3 = v("--ink-3", "#6d7c8d");
    const line = v("--line-strong", "rgba(146,170,196,.32)");
    const attention = v("--attention", "#ffb547");
    const font = v("--font", "sans-serif");

    const c = METER / 2;
    const r = c - 8;
    const scale = r / maxG;
    const trail: { t: number; x: number; y: number }[] = [];
    // Grip used in each direction recently: decays so it tracks the current car and conditions.
    const envelope = new Float32Array(ENVELOPE_BUCKETS);
    let raf = 0;

    const toXY = (d: NonNullable<TickPayload["dynamics"]>) => {
      const sx = felt ? d.latG : -d.latG;
      const sy = felt ? d.longG : -d.longG;
      return { x: sx, y: sy };
    };

    const unsub = useLive.subscribe((s, prev) => {
      const t = s.tick;
      if (!t || t === prev.tick || !t.dynamics) return;
      const p = toXY(t.dynamics);
      trail.push({ t: t.sessionTime, ...p });
      while (trail.length && (trail[0].t < t.sessionTime - TRAIL_S || trail[0].t > t.sessionTime)) trail.shift();
      const mag = Math.hypot(p.x, p.y);
      const a = Math.atan2(p.y, p.x);
      const b = ((Math.round((a / (2 * Math.PI)) * ENVELOPE_BUCKETS) % ENVELOPE_BUCKETS) + ENVELOPE_BUCKETS) % ENVELOPE_BUCKETS;
      for (let i = 0; i < ENVELOPE_BUCKETS; i++) envelope[i] *= 0.9997;
      envelope[b] = Math.max(envelope[b], mag);
    });

    const draw = () => {
      ctx.clearRect(0, 0, METER, METER);

      // Rings at every 1 g, and axes.
      ctx.strokeStyle = line;
      ctx.lineWidth = 1;
      for (let g = 1; g <= maxG; g++) {
        ctx.setLineDash(g === maxG ? [] : [2, 4]);
        ctx.beginPath();
        ctx.arc(c, c, g * scale, 0, Math.PI * 2);
        ctx.stroke();
      }
      ctx.setLineDash([]);
      ctx.beginPath();
      ctx.moveTo(c - r, c);
      ctx.lineTo(c + r, c);
      ctx.moveTo(c, c - r);
      ctx.lineTo(c, c + r);
      ctx.stroke();
      ctx.fillStyle = ink3;
      ctx.font = `500 10px ${font}`;
      // Ring labels sit just under the right-hand axis, clear of the direction labels.
      ctx.textAlign = "right";
      for (let g = 1; g <= maxG; g++) ctx.fillText(`${g}g`, c + g * scale - 3, c + 11);
      ctx.textAlign = "center";
      ctx.fillText(felt ? "Brake" : "Accel", c, 9);
      ctx.fillText(felt ? "Accel" : "Brake", c, METER - 2);

      // Envelope of grip used.
      ctx.beginPath();
      for (let i = 0; i <= ENVELOPE_BUCKETS; i++) {
        const k = i % ENVELOPE_BUCKETS;
        // Smooth across neighbours so gaps don't spike inwards.
        const m = Math.max(envelope[k], 0.92 * Math.max(envelope[(k + 1) % ENVELOPE_BUCKETS], envelope[(k + ENVELOPE_BUCKETS - 1) % ENVELOPE_BUCKETS]));
        const a = (k / ENVELOPE_BUCKETS) * Math.PI * 2;
        const px = c + Math.cos(a) * Math.min(m, maxG) * scale;
        const py = c + Math.sin(a) * Math.min(m, maxG) * scale;
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.fillStyle = "rgba(233, 238, 243, 0.07)";
      ctx.fill();
      ctx.strokeStyle = "rgba(233, 238, 243, 0.25)";
      ctx.stroke();

      // Fading trail.
      const now = trail.length ? trail[trail.length - 1].t : 0;
      for (let i = 1; i < trail.length; i++) {
        const age = (now - trail[i].t) / TRAIL_S;
        ctx.strokeStyle = `rgba(255, 181, 71, ${0.65 * (1 - age)})`;
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(c + trail[i - 1].x * scale, c + trail[i - 1].y * scale);
        ctx.lineTo(c + trail[i].x * scale, c + trail[i].y * scale);
        ctx.stroke();
      }

      // Current position.
      const cur = trail[trail.length - 1];
      if (cur) {
        const px = c + Math.max(-maxG, Math.min(maxG, cur.x)) * scale;
        const py = c + Math.max(-maxG, Math.min(maxG, cur.y)) * scale;
        ctx.fillStyle = attention;
        ctx.beginPath();
        ctx.arc(px, py, 7, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = ink;
        ctx.beginPath();
        ctx.arc(px, py, 3, 0, Math.PI * 2);
        ctx.fill();
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => {
      cancelAnimationFrame(raf);
      unsub();
    };
  }, [maxG, felt]);

  return <canvas ref={canvas} className="dyn-gmeter" style={{ width: METER, height: METER }} />;
}
