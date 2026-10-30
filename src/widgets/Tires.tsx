import { opt, type WidgetConfig } from "../lib/config";
import type { Units } from "../lib/format";
import { useLive } from "../lib/store";
import type { TireCorner } from "../lib/types";
import { Empty, Panel } from "./Panel";

const CORNER_NAMES = ["Left front", "Right front", "Left rear", "Right rear"];
const CORNER_SHORT = ["LF", "RF", "LR", "RR"];

/**
 * iRacing only updates tire temperatures and wear while you're in your pit
 * box, so this shows the last reading and says when it was live.
 */
export function Tires({ config }: { config: WidgetConfig }) {
  const tires = useLive((s) => s.field?.tires);
  const units = useLive((s) => s.config.general.units);
  const showPressure = opt(config, "showPressure", true);

  const sets =
    tires == null
      ? null
      : tires.setsAvailable == null
        ? `${tires.setsUsed} sets used`
        : `${tires.setsAvailable} of ${tires.setsUsed + tires.setsAvailable} sets left`;

  return (
    <Panel className="tires" title="Tires" aside={[tires?.compound, sets].filter(Boolean).join("   ")}>
      {!tires?.hasData ? (
        <Empty>Readings appear after you stop in your pit box.</Empty>
      ) : (
        <>
          <div className="tire-grid">
            {tires.corners.map((c, k) => (
              <Corner key={k} name={CORNER_NAMES[k]} short={CORNER_SHORT[k]} corner={c} units={units} showPressure={showPressure} />
            ))}
          </div>
          <div className={`tire-note ${tires.live ? "is-live" : ""}`}>
            {tires.live ? "Live while you're in the pit box" : "From your last pit stop"}
          </div>
        </>
      )}
    </Panel>
  );
}

function Corner({ name, short, corner, units, showPressure }: {
  name: string; short: string; corner: TireCorner; units: Units; showPressure: boolean;
}) {
  const wear = Math.min(...corner.wear);
  return (
    <div className="tire" aria-label={name}>
      <div className="tire-temps">
        {corner.temp.map((t, j) => (
          <span key={j} className="tire-temp" style={{ background: heat(t) }}>
            {temp(t, units)}
          </span>
        ))}
      </div>
      <div className="tire-wear">
        {corner.wear.map((w, j) => (
          <span key={j} className="tire-tread">
            <span style={{ transform: `scaleX(${Math.max(0, Math.min(1, w))})`, background: wearColor(w) }} />
          </span>
        ))}
      </div>
      <div className="tire-meta">
        <span className="tire-pos">{short}</span>
        <span title="Least tread remaining across the tire">{Math.round(wear * 100)}%</span>
        {showPressure && corner.coldPressure > 0 && <span className="muted">{pressure(corner.coldPressure, units)}</span>}
      </div>
    </div>
  );
}

function temp(c: number, units: Units) {
  return Math.round(units === "imperial" ? (c * 9) / 5 + 32 : c);
}

function pressure(kpa: number, units: Units) {
  return units === "imperial" ? `${(kpa * 0.145038).toFixed(1)} psi` : `${Math.round(kpa)} kPa`;
}

/** Cold blue, working green, overheating red. */
function heat(c: number) {
  const stops: [number, number][] = [
    [40, 215],
    [70, 175],
    [85, 135],
    [100, 45],
    [115, 0],
  ];
  let hue = stops[0][1];
  if (c >= stops[stops.length - 1][0]) hue = stops[stops.length - 1][1];
  for (let i = 0; i < stops.length - 1; i++) {
    const [t0, h0] = stops[i];
    const [t1, h1] = stops[i + 1];
    if (c >= t0 && c < t1) hue = h0 + ((c - t0) / (t1 - t0)) * (h1 - h0);
  }
  return `hsl(${hue} 62% 38%)`;
}

function wearColor(w: number) {
  if (w > 0.7) return "var(--gain)";
  if (w > 0.4) return "var(--attention)";
  return "var(--loss)";
}
