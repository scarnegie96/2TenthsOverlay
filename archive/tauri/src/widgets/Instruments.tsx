import { useEffect, useRef, useState, type ComponentType } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import type { Units } from "../lib/format";
import { useLive } from "../lib/store";
import type { InstrumentsPayload, Warnings } from "../lib/types";
import {
  FuelPressureIcon, LowFuelIcon, OilPressureIcon, OilTempIcon, OptionalRepairIcon, PitLimiterIcon, RepairIcon,
  RevLimiterIcon, StalledIcon, WaterTempIcon,
} from "./dashIcons";
import { Panel } from "./Panel";

type Level = "danger" | "caution" | "info";

/** Every warning light, in dashboard order, with how serious it is when lit. */
const LIGHTS: { key: keyof Warnings; label: string; level: Level; Icon: ComponentType }[] = [
  { key: "oilPressure", label: "Oil press", level: "danger", Icon: OilPressureIcon },
  { key: "oilTemp", label: "Oil temp", level: "caution", Icon: OilTempIcon },
  { key: "waterTemp", label: "Water temp", level: "danger", Icon: WaterTempIcon },
  { key: "fuelPressure", label: "Fuel press", level: "danger", Icon: FuelPressureIcon },
  { key: "lowFuel", label: "Low fuel", level: "caution", Icon: LowFuelIcon },
  { key: "stalled", label: "Stalled", level: "danger", Icon: StalledIcon },
  { key: "pitLimiter", label: "Limiter", level: "info", Icon: PitLimiterIcon },
  { key: "revLimiter", label: "Rev limit", level: "caution", Icon: RevLimiterIcon },
  { key: "repairRequired", label: "Repair", level: "danger", Icon: RepairIcon },
  { key: "repairOptional", label: "Opt. repair", level: "caution", Icon: OptionalRepairIcon },
];

const temp = (c: number, u: Units) => (u === "imperial" ? (c * 9) / 5 + 32 : c);
const press = (bar: number, u: Units) => (u === "imperial" ? bar * 14.5038 : bar);

/**
 * Dashboard: warning lights that light up when iRacing raises them, engine
 * gauges, and a brake bias dial.
 */
export function Instruments({ config, editing }: { config: WidgetConfig; editing: boolean }) {
  const ins = useLive((s) => s.tick?.instruments ?? null);
  const units = useLive((s) => s.config.general.units);
  const showGauges = opt(config, "showGauges", true);
  const showBias = opt(config, "showBias", true);
  const testLights = editing && opt(config, "testLights", false);

  const lit = (k: keyof Warnings) => testLights || (ins?.warnings[k] ?? false);
  const anyDanger = LIGHTS.some((l) => l.level === "danger" && lit(l.key));

  return (
    <Panel className={`instruments ${anyDanger ? "has-danger" : ""}`}>
      <div className="dash-lights" role="list" aria-label="Warning lights">
        {LIGHTS.map(({ key, label, level, Icon }) => (
          <div key={key} role="listitem" className={`dash-light level-${level} ${lit(key) ? "is-on" : ""}`} title={label}>
            <Icon />
            <span>{label}</span>
          </div>
        ))}
      </div>
      {(showGauges || showBias) && (
        <div className="dash-lower">
          {showGauges && <Gauges ins={ins} units={units} />}
          {showBias && <BrakeBias ins={ins} />}
        </div>
      )}
    </Panel>
  );
}

function Gauges({ ins, units }: { ins: InstrumentsPayload | null; units: Units }) {
  const tUnit = units === "imperial" ? "°F" : "°C";
  const pUnit = units === "imperial" ? "psi" : "bar";
  const rows: { label: string; value: number | null | undefined; unit: string; min: number; max: number; warn?: boolean; digits: number }[] = [
    { label: "Oil temp", value: ins?.oilTemp != null ? temp(ins.oilTemp, units) : null, unit: tUnit, min: temp(60, units), max: temp(140, units), warn: ins?.warnings.oilTemp, digits: 0 },
    { label: "Oil press", value: ins?.oilPress != null ? press(ins.oilPress, units) : null, unit: pUnit, min: 0, max: press(8, units), warn: ins?.warnings.oilPressure, digits: 1 },
    { label: "Water", value: ins?.waterTemp != null ? temp(ins.waterTemp, units) : null, unit: tUnit, min: temp(50, units), max: temp(120, units), warn: ins?.warnings.waterTemp, digits: 0 },
    { label: "Fuel press", value: ins?.fuelPress != null ? press(ins.fuelPress, units) : null, unit: pUnit, min: 0, max: press(6, units), warn: ins?.warnings.fuelPressure, digits: 1 },
    { label: "Battery", value: ins?.voltage, unit: "V", min: 10, max: 15, digits: 1 },
  ];
  return (
    <div className="dash-gauges">
      {rows.map((r, i) => {
        const frac = r.value == null ? 0 : Math.max(0, Math.min(1, (r.value - r.min) / (r.max - r.min)));
        return (
          <div key={i} className={`dash-gauge ${r.warn ? "is-warn" : ""} ${r.value == null ? "is-missing" : ""}`}>
            <span className="dash-gauge-label">{r.label}</span>
            <span className="dash-gauge-bar">
              <span style={{ transform: `scaleX(${frac})` }} />
            </span>
            <span className="dash-gauge-value">
              {r.value == null ? "–" : r.value.toFixed(r.digits)}
              <small>{r.unit}</small>
            </span>
          </div>
        );
      })}
    </div>
  );
}

const BIAS_MIN = 40;
const BIAS_MAX = 70;

/** Semicircle dial: percentage of braking on the front axle. Flashes the change when you adjust it. */
function BrakeBias({ ins }: { ins: InstrumentsPayload | null }) {
  const bias = ins?.brakeBias ?? null;
  const [change, setChange] = useState<{ by: number; key: number } | null>(null);
  const prev = useRef<number | null>(null);

  useEffect(() => {
    if (bias == null) return;
    const before = prev.current;
    prev.current = bias;
    if (before != null && Math.abs(bias - before) > 0.01) {
      setChange({ by: bias - before, key: Date.now() });
    }
  }, [bias]);
  useEffect(() => {
    if (!change) return;
    const id = window.setTimeout(() => setChange(null), 2500);
    return () => window.clearTimeout(id);
  }, [change]);

  const frac = bias == null ? 0.5 : Math.max(0, Math.min(1, (bias - BIAS_MIN) / (BIAS_MAX - BIAS_MIN)));
  const angle = -90 + frac * 180;
  const ticks = [];
  for (let v = BIAS_MIN; v <= BIAS_MAX; v += 5) {
    const a = ((-90 + ((v - BIAS_MIN) / (BIAS_MAX - BIAS_MIN)) * 180) * Math.PI) / 180;
    const major = v % 10 === 0;
    ticks.push(
      <line key={v} className={major ? "bias-tick is-major" : "bias-tick"}
        x1={Math.sin(a) * 40} y1={-Math.cos(a) * 40} x2={Math.sin(a) * (major ? 33 : 36)} y2={-Math.cos(a) * (major ? 33 : 36)} />,
    );
  }

  return (
    <div className={`bias ${bias == null ? "is-missing" : ""} ${change ? "is-changed" : ""}`}>
      <svg viewBox="-48 -48 96 56" className="bias-dial" aria-hidden>
        <path className="bias-track" d="M -40 0 A 40 40 0 0 1 40 0" />
        {ticks}
        {bias != null && (
          <g style={{ transform: `rotate(${angle}deg)` }} className="bias-needle">
            <path d="M -2.5 0 L 0 -34 L 2.5 0 Z" />
          </g>
        )}
        <circle className="bias-hub" cx="0" cy="0" r="4" />
        <text className="bias-end" x="-40" y="9" textAnchor="middle">{BIAS_MIN}</text>
        <text className="bias-end" x="40" y="9" textAnchor="middle">{BIAS_MAX}</text>
      </svg>
      <div className="bias-readout">
        <span className="bias-value">{bias == null ? "–" : `${bias.toFixed(1)}%`}</span>
        <span className="bias-label">
          {bias == null
            ? "Bias not adjustable"
            : change
              ? `Moved ${Math.abs(change.by).toFixed(1)} to the ${change.by > 0 ? "front" : "rear"}`
              : "Brake bias front"}
        </span>
        {(ins?.peakBrakeBias != null || ins?.brakeBiasFine != null) && (
          <span className="bias-extra">
            {ins?.peakBrakeBias != null && <>Peak {ins.peakBrakeBias.toFixed(1)}%</>}
            {ins?.peakBrakeBias != null && ins?.brakeBiasFine != null && "   "}
            {ins?.brakeBiasFine != null && <>Fine {ins.brakeBiasFine.toFixed(1)}</>}
          </span>
        )}
      </div>
    </div>
  );
}
