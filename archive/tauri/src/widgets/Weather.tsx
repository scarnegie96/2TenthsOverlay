import { useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import type { Units } from "../lib/format";
import { useLive } from "../lib/store";
import {
  addSample, SKIES, trend, WETNESS, type Sample,
} from "../lib/weather";
import { Panel } from "./Panel";

const temp = (c: number, u: Units) => (u === "imperial" ? (c * 9) / 5 + 32 : c);
const tempUnit = (u: Units) => (u === "imperial" ? "°F" : "°C");
const windSpeed = (ms: number, u: Units) => Math.round(Math.abs(ms) * (u === "imperial" ? 2.23694 : 3.6));
const windUnit = (u: Units) => (u === "imperial" ? "mph" : "km/h");

/**
 * Weather: track and air temperature, how wet the track is, rain, humidity
 * and fog, and a wind compass. The compass turns with the car, so the arrow
 * shows where the wind hits you from right now. Small arrows show which way
 * the temperatures, wetness and rain have been moving over the last few
 * minutes; iRacing doesn't publish a forecast or rain radar, so that's the
 * closest thing to one.
 */
export function Weather({ config }: { config: WidgetConfig }) {
  const clock = useLive((s) => s.field?.clock);
  const heading = useLive((s) => s.tick?.heading ?? null);
  const simTime = useLive((s) => s.tick?.sessionTime ?? null);
  const units = useLive((s) => s.config.general.units);
  const showCompass = opt(config, "showCompass", true);
  const turnWithCar = opt<string>(config, "compass", "car") === "car";
  const showTrends = opt(config, "showTrends", true);

  // Recent history, for the trend arrows.
  const history = useRef<Record<"track" | "air" | "wet" | "rain", Sample[]>>({ track: [], air: [], wet: [], rain: [] });
  // Sim time, so trends pause with the sim and a new session starts afresh.
  if (clock && simTime != null) {
    const now = simTime * 1000;
    const h = history.current;
    h.track = addSample(h.track, now, clock.trackTemp);
    h.air = addSample(h.air, now, clock.airTemp);
    if (clock.trackWetness) h.wet = addSample(h.wet, now, clock.trackWetness);
    if (clock.precipitation != null) h.rain = addSample(h.rain, now, clock.precipitation);
  }
  const h = history.current;
  const show = (t: -1 | 0 | 1) => (showTrends ? t : 0);

  const sky = clock?.skies != null ? SKIES[clock.skies] : null;
  const wetness = clock?.trackWetness || 0;
  const rain = clock?.precipitation ?? null;

  return (
    <Panel
      className="weather"
      title="Weather"
      aside={
        <>
          {sky ?? ""}
          {clock?.declaredWet && <span className="tag">Wet</span>}
        </>
      }
    >
      <div className="wx-temps">
        <Reading label="Track" value={clock ? temp(clock.trackTemp, units).toFixed(1) : "–"} unit={tempUnit(units)} trend={show(trend(h.track, 0.5))} />
        <Reading label="Air" value={clock ? temp(clock.airTemp, units).toFixed(1) : "–"} unit={tempUnit(units)} trend={show(trend(h.air, 0.5))} />
      </div>

      <div className="wx-row">
        <div className="wx-row-head">
          <span>Track</span>
          <b className={wetness >= 3 ? "is-wet" : undefined}>
            {WETNESS[wetness] || "–"}
            <Trend t={show(trend(h.wet, 0.5))} up="Getting wetter" down="Drying" />
          </b>
        </div>
        <div className="wx-wetness" title="Track wetness, dry to extremely wet">
          {[1, 2, 3, 4, 5, 6, 7].map((k) => (
            <i key={k} className={k <= wetness ? (k >= 3 ? "on is-wet" : "on") : undefined} />
          ))}
        </div>
      </div>

      <div className="wx-row">
        <div className="wx-row-head">
          <span>Rain</span>
          <b className={rain != null && rain >= 0.05 ? "is-wet" : undefined}>
            {rain == null ? "Not reported" : `${Math.round(rain * 100)}%`}
            <Trend t={show(trend(h.rain, 0.03))} up="Heavier" down="Easing" />
          </b>
        </div>
        {rain != null && (
          <div className="wx-bar">
            <i style={{ transform: `scaleX(${Math.min(1, rain)})` }} />
          </div>
        )}
      </div>

      <dl className="wx-grid">
        {clock?.humidity != null && (
          <>
            <dt>Humidity</dt>
            <dd>{Math.round(clock.humidity * 100)}%</dd>
          </>
        )}
        {clock?.fogLevel != null && clock.fogLevel > 0.005 && (
          <>
            <dt>Fog</dt>
            <dd>{Math.round(clock.fogLevel * 100)}%</dd>
          </>
        )}
      </dl>

      {showCompass && (
        <Wind
          speed={clock?.windSpeed ?? null}
          dir={clock?.windDir ?? null}
          heading={heading}
          turnWithCar={turnWithCar}
          units={units}
        />
      )}
    </Panel>
  );
}

function Reading({ label, value, unit, trend: t }: { label: string; value: string; unit: string; trend: -1 | 0 | 1 }) {
  return (
    <div className="wx-reading">
      <span>{label}</span>
      <b>
        {value}
        <small>{unit}</small>
        <Trend t={t} up="Rising" down="Falling" />
      </b>
    </div>
  );
}

function Trend({ t, up, down }: { t: -1 | 0 | 1; up: string; down: string }) {
  if (!t) return null;
  return (
    <span className={`wx-trend ${t > 0 ? "is-up" : "is-down"}`} title={t > 0 ? up : down}>
      {t > 0 ? "▲" : "▼"}
    </span>
  );
}

const SIZE = 168;
const C = SIZE / 2;
const RING = C - 14;

/**
 * Wind compass. In "turns with car" mode the car always points up and the
 * compass card turns underneath it, like looking down on the car; the wind
 * arrow is fixed to the card, so it swings round as you drive the lap. In
 * "north up" mode the card stays put and the car turns.
 */
function Wind({ speed, dir, heading, turnWithCar, units }: {
  speed: number | null;
  dir: number | null;
  heading: number | null;
  turnWithCar: boolean;
  units: Units;
}) {
  const hasHeading = heading != null;
  const h = heading ?? 0;
  // Rotation of the compass card and of the car, in degrees clockwise.
  const card = turnWithCar && hasHeading ? -h : 0;
  const car = turnWithCar ? 0 : h;
  const deg = (r: number) => (r * 180) / Math.PI;
  const calm = speed == null || speed < 0.3;

  return (
    <div className="wx-wind">
      <svg className="wx-compass" width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`}>
        <circle className="wx-dial" cx={C} cy={C} r={RING} />
        <g transform={`rotate(${deg(card)} ${C} ${C})`}>
          {Array.from({ length: 36 }, (_, i) => {
            const major = i % 9 === 0;
            const a = (i * 10 * Math.PI) / 180;
            const r0 = RING - (major ? 9 : 5);
            return (
              <line
                key={i}
                className={major ? "wx-tick is-major" : "wx-tick"}
                x1={C + Math.sin(a) * r0} y1={C - Math.cos(a) * r0}
                x2={C + Math.sin(a) * RING} y2={C - Math.cos(a) * RING}
              />
            );
          })}
          {["N", "E", "S", "W"].map((l, i) => {
            const a = (i * Math.PI) / 2;
            const x = C + Math.sin(a) * (RING + 8);
            const y = C - Math.cos(a) * (RING + 8);
            // Letters stay upright while the card turns.
            return (
              <text key={l} className={l === "N" ? "wx-cardinal is-north" : "wx-cardinal"} x={x} y={y}
                transform={`rotate(${-deg(card)} ${x} ${y})`}>
                {l}
              </text>
            );
          })}
          {!calm && dir != null && <WindArrow dir={dir} />}
        </g>
        <g transform={`rotate(${deg(car)} ${C} ${C})`} opacity={hasHeading ? 1 : 0.4}>
          <CarIcon />
        </g>
      </svg>
      {/* The compass shows the direction; this is just how hard it blows. */}
      <div className="wx-wind-text">
        <b className="wx-wind-speed">
          {speed == null ? "–" : calm ? "Calm" : windSpeed(speed, units)}
          {!calm && speed != null && <small>{windUnit(units)}</small>}
        </b>
        <span className="wx-wind-label">Wind</span>
        {!hasHeading && <div className="wx-note">No heading from iRacing: north is up</div>}
      </div>
    </div>
  );
}

/** An arrow on the compass card that starts on the side the wind comes from and points the way it blows. */
function WindArrow({ dir }: { dir: number }) {
  const deg = (dir * 180) / Math.PI;
  const tail = C - RING + 3;
  const tip = C - 30; // stops short of the car, pointing at it
  return (
    <g className="wx-arrow" transform={`rotate(${deg} ${C} ${C})`}>
      {/* Drawn as if the wind came from the north (top) and blew south. */}
      <line x1={C} y1={tail} x2={C} y2={tip - 14} />
      <polygon points={`${C},${tip} ${C - 12},${tip - 19} ${C + 12},${tip - 19}`} />
      <circle cx={C} cy={tail} r={5.5} />
    </g>
  );
}

/** A small car seen from above, nose up. */
function CarIcon() {
  return (
    <g className="wx-car">
      <rect x={C - 9} y={C - 18} width={18} height={36} rx={6} />
      <rect className="wx-car-glass" x={C - 6.5} y={C - 9} width={13} height={8} rx={2} />
      <path className="wx-car-nose" d={`M ${C - 5} ${C - 22} L ${C} ${C - 28} L ${C + 5} ${C - 22} Z`} />
    </g>
  );
}
