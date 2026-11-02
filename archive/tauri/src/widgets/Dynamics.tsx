import { useEffect, useRef, useState } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { useLive } from "../lib/store";
import { cssColor, paletteKey, withAlpha } from "../lib/theme";
import type { TickPayload } from "../lib/types";
import { Panel } from "./Panel";

const METER = 176;
const TRAIL_S = 1.2;
const ENVELOPE_BUCKETS = 48;

/**
 * How far a point of the grip outline sits from the centre at `now`: its peak
 * for `holdS` seconds after it was set, then easing back to the centre over
 * `returnS` seconds.
 */
function envelopeReach(peak: number, setAt: number, now: number, holdS: number, returnS: number): number {
  const age = now - setAt;
  if (age <= holdS) return peak;
  if (returnS <= 0) return 0;
  const f = Math.min(1, (age - holdS) / returnS);
  // Ease in and out, so points start drifting gently and settle at the centre.
  return peak * (1 - f * f * (3 - 2 * f));
}

/**
 * Car dynamics: a g-meter (with the grip you've used recently as a faint
 * envelope that withdraws to the centre once it's old), the car's attitude seen from above (slip), the side (pitch) and
 * behind (roll), and an understeer / oversteer balance bar.
 */
export function Dynamics({ config }: { config: WidgetConfig }) {
  const dyn = useLive((s) => s.tick?.dynamics ?? null);
  const maxG = opt(config, "maxG", 3);
  const holdS = opt(config, "envelopeHold", 10);
  const returnS = opt(config, "envelopeReturn", 10);
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
    // With neither, it's just the g-meter, as narrow as the meter.
    <Panel className={`dynamics ${!showAttitude && !showBalance ? "is-meter-only" : ""}`} title={!showAttitude && !showBalance ? "G-meter" : "Car dynamics"} aside={`${total.toFixed(2)} g`}>
      <div className="dyn-body">
        <div className="dyn-meter">
          <GMeter maxG={maxG} felt={felt} holdS={holdS} returnS={returnS} />
          <div className="dyn-readouts">
            <Readout label="Lateral" value={Math.abs(lat)} unit="g" />
            <Readout label={long < 0 ? "Braking" : "Accel"} value={Math.abs(long)} unit="g" />
            <Readout label="Vertical" value={dyn?.vertG ?? 1} unit="g" />
          </div>
        </div>
        {(showAttitude || showBalance) && (
          <div className="dyn-side">
            {showAttitude && <div className="dyn-views">
              <View label="Slip" value={dyn?.slipDeg ?? 0} title="Angle between where the car points and where it's going">
                <TopView slip={dyn?.slipDeg ?? 0} />
              </View>
              <View label="Pitch" value={dyn?.pitchDeg ?? 0} title="Nose up (+) or down (−)">
                <SideView pitch={(dyn?.pitchDeg ?? 0) * exaggerate} />
              </View>
              <View label="Roll" value={dyn?.rollDeg ?? 0} title="Leaning right (+) or left (−)">
                <RearView roll={(dyn?.rollDeg ?? 0) * exaggerate} />
              </View>
            </div>}
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
        <rect className="dyn-wheel" x="-14.5" y="-20" width="5" height="10" rx="1.5" />
        <rect className="dyn-wheel" x="9.5" y="-20" width="5" height="10" rx="1.5" />
        <rect className="dyn-wheel" x="-15.5" y="11" width="6" height="11" rx="1.5" />
        <rect className="dyn-wheel" x="9.5" y="11" width="6" height="11" rx="1.5" />
        <path
          className="dyn-car"
          d="M 0 -32 C 6 -32 10 -31 11.5 -28 L 12.5 -21 C 13 -17 13 -13 12.5 -9 C 12 -6 11.5 -3 11.5 0 C 11.5 4 13.5 7 13.5 11
             L 13.5 22 C 13.5 25.5 12 27.5 9 27.5 L -9 27.5 C -12 27.5 -13.5 25.5 -13.5 22 L -13.5 11 C -13.5 7 -11.5 4 -11.5 0
             C -11.5 -3 -12 -6 -12.5 -9 C -13 -13 -13 -17 -12.5 -21 L -11.5 -28 C -10 -31 -6 -32 0 -32 Z"
        />
        <path className="dyn-car-trim" d="M -8 -29.5 Q 0 -31.5 8 -29.5 M -4.5 -24 L -4.5 -17 M 4.5 -24 L 4.5 -17" />
        <path className="dyn-car-detail" d="M -7 -10 C -5 -12 5 -12 7 -10 L 8 7 C 6 9 -6 9 -8 7 Z" />
        <rect className="dyn-car" x="-6" y="-4" width="12" height="8" rx="2" />
        <rect className="dyn-car-wing" x="-14.5" y="23.5" width="29" height="5" rx="1" />
      </g>
    </>
  );
}

/** From the side, front to the right; tilt exaggerated so small angles are visible. */
function SideView({ pitch }: { pitch: number }) {
  return (
    <>
      <line className="dyn-ground" x1="-38" y1="14" x2="38" y2="14" />
      <g style={{ transform: `rotate(${-pitch}deg)` }}>
        <path
          className="dyn-car"
          d="M -33 8.5 L -34 2 C -34 -1 -33 -3 -31 -4 L -22 -5 C -16 -6 -12 -12 -6 -14 C -2 -15.5 4 -15.5 7 -14
             C 11 -12 14 -8 18 -6.5 C 24 -5 30 -3 33 0 L 35 6 L 36 8.5 L 29 8.5 A 8 8 0 0 0 13 8.5
             L -12 8.5 A 8 8 0 0 0 -28 8.5 Z"
        />
        <path className="dyn-car-detail" d="M -13 -7 C -10 -10 -7 -12 -3 -12.5 L 5 -12.5 C 8 -11.5 11 -9 13 -7 Z" />
        <path className="dyn-car-trim" d="M -6 6 L 9 6" />
        <path className="dyn-car-wing" d="M -36 -12 L -24 -12.5 L -24 -10 L -35 -9.5 Z M -31 -9.5 L -29.5 -9.5 L -29.5 -4.5 L -31 -4.5 Z" />
        <Tyre cx={-20} />
        <Tyre cx={21} />
      </g>
    </>
  );
}

function Tyre({ cx }: { cx: number }) {
  return (
    <>
      <circle className="dyn-wheel" cx={cx} cy="7.5" r="6.5" />
      <circle className="dyn-rim" cx={cx} cy="7.5" r="3.2" />
    </>
  );
}

/** From behind; roll exaggerated like pitch. */
function RearView({ roll }: { roll: number }) {
  return (
    <>
      <line className="dyn-ground" x1="-36" y1="14" x2="36" y2="14" />
      <g style={{ transform: `rotate(${roll}deg)` }}>
        <rect className="dyn-wheel" x="-29" y="0" width="10" height="14" rx="2.5" />
        <rect className="dyn-wheel" x="19" y="0" width="10" height="14" rx="2.5" />
        <path
          className="dyn-car"
          d="M -27.5 4 L -27.5 0 C -27.5 -3 -25.5 -5 -22 -5.5 L -14 -6.5 C -12 -12 -10 -16 -6 -16.5 L 6 -16.5
             C 10 -16 12 -12 14 -6.5 L 22 -5.5 C 25.5 -5 27.5 -3 27.5 0 L 27.5 4 L 19 4 L 17 10 L -17 10 L -19 4 Z"
        />
        <path className="dyn-car-detail" d="M -8.5 -14.5 L 8.5 -14.5 L 11 -8.5 L -11 -8.5 Z" />
        <rect className="dyn-car-detail" x="-15" y="4" width="30" height="5" rx="1" />
        <path className="dyn-car-trim" d="M -7.5 4.5 L -7.5 9 M 0 4.5 L 0 9 M 7.5 4.5 L 7.5 9" />
        <rect className="dyn-car-light" x="-24.5" y="-3.5" width="8" height="2" rx="1" />
        <rect className="dyn-car-light" x="16.5" y="-3.5" width="8" height="2" rx="1" />
        <path className="dyn-car-wing" d="M -8.5 -17 L -7 -17 L -7 -9 L -8.5 -9 Z M 7 -17 L 8.5 -17 L 8.5 -9 L 7 -9 Z" />
        <rect className="dyn-car-wing" x="-25" y="-20" width="50" height="3" rx="1" />
        <rect className="dyn-car-wing" x="-26.5" y="-22" width="2.5" height="8" rx="0.8" />
        <rect className="dyn-car-wing" x="24" y="-22" width="2.5" height="8" rx="0.8" />
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
 *
 * The grip outline has a point per direction. Each one stays where you last
 * pushed it for `holdS` seconds, then withdraws to the centre over `returnS`
 * seconds, so the outline shows what you've used recently and shrinks away
 * on straights. Pushing past a point's current reach sets it again.
 */
function GMeter({ maxG, felt, holdS, returnS }: { maxG: number; felt: boolean; holdS: number; returnS: number }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const palette = useLive((s) => paletteKey(s.config.general));

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2;
    el.width = METER * dpr;
    el.height = METER * dpr;
    ctx.scale(dpr, dpr);

    const ink = cssColor(el, "var(--ink)");
    const ink3 = cssColor(el, "var(--ink-3)");
    const line = cssColor(el, "var(--line-strong)");
    const attention = cssColor(el, "var(--attention)");
    const font = getComputedStyle(el).getPropertyValue("--font").trim() || "sans-serif";

    const c = METER / 2;
    const r = c - 8;
    const scale = r / maxG;
    const trail: { t: number; x: number; y: number }[] = [];
    // Grip used in each direction: the peak, and the session time it was set.
    const peak = new Float32Array(ENVELOPE_BUCKETS);
    const setAt = new Float64Array(ENVELOPE_BUCKETS);
    // Each point's current reach, worked out once per frame.
    const envelope = new Float32Array(ENVELOPE_BUCKETS);
    let clock = 0;
    const reach = (k: number, now: number) => envelopeReach(peak[k], setAt[k], now, holdS, returnS);
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
      const now = t.sessionTime;
      // Time went backwards (new session, replay): start the outline afresh.
      if (now < clock) {
        peak.fill(0);
        setAt.fill(0);
      }
      clock = now;
      if (mag >= reach(b, now)) {
        peak[b] = mag;
        setAt[b] = now;
      }
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

      // Envelope of grip used, withdrawing to the centre as it ages.
      for (let k = 0; k < ENVELOPE_BUCKETS; k++) envelope[k] = reach(k, clock);
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
      ctx.fillStyle = withAlpha(ink, 0.07);
      ctx.fill();
      ctx.strokeStyle = withAlpha(ink, 0.25);
      ctx.stroke();

      // Fading trail.
      const now = trail.length ? trail[trail.length - 1].t : 0;
      for (let i = 1; i < trail.length; i++) {
        const age = (now - trail[i].t) / TRAIL_S;
        ctx.strokeStyle = withAlpha(attention, 0.65 * (1 - age));
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
  }, [maxG, felt, holdS, returnS, palette]);

  return <canvas ref={canvas} className="dyn-gmeter" style={{ width: METER, height: METER }} />;
}
