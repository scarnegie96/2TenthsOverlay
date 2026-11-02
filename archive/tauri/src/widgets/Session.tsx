import type React from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { irating, type Units } from "../lib/format";
import { incidentTarget } from "../lib/incidents";
import { useLive } from "../lib/store";
import type { SessionClock } from "../lib/types";
import { Panel } from "./Panel";
import { sessionAside } from "./Relative";

const SKIES = ["Clear", "Partly cloudy", "Mostly cloudy", "Overcast"];
/** irsdk_TrackWetness, 1..7 */
const WETNESS = ["", "Dry", "Mostly dry", "Very lightly wet", "Lightly wet", "Moderately wet", "Very wet", "Extremely wet"];
const COMPASS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

const temp = (c: number, u: Units) => `${(u === "imperial" ? (c * 9) / 5 + 32 : c).toFixed(1)}°`;
const wind = (ms: number, u: Units) => (u === "imperial" ? `${Math.round(ms * 2.23694)} mph` : `${Math.round(ms * 3.6)} km/h`);
const compass = (rad: number) => COMPASS[Math.round((((rad * 180) / Math.PI) % 360 + 360) % 360 / 45) % 8];

/** Seconds since midnight -> "14:05" */
function timeOfDay(s: number) {
  const m = Math.floor(s / 60) % (24 * 60);
  return `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
}

/** Track, field strength, conditions and the event's limits in one panel. */
export function Session({ config }: { config: WidgetConfig }) {
  const session = useLive((s) => s.session);
  const clock = useLive((s) => s.field?.clock);
  const units = useLive((s) => s.config.general.units);
  const showClasses = opt(config, "showClasses", true);
  const showConditions = opt(config, "showConditions", true);
  const showLimits = opt(config, "showLimits", true);

  const classes = clock?.classes ?? [];
  const cars = classes.reduce((n, c) => n + c.cars, 0);
  const length = session?.trackLengthKm
    ? units === "imperial" ? `${(session.trackLengthKm * 0.621371).toFixed(2)} mi` : `${session.trackLengthKm.toFixed(2)} km`
    : null;

  return (
    <Panel className="session-info" title={clock?.sessionType || session?.sessionType || "Session"} aside={sessionAside(clock)}>
      <div className="si-track">
        <b>{session?.trackName || "–"}</b>
        <span>{[session?.trackConfig, length].filter(Boolean).join("   ")}</span>
      </div>
      <dl className="si-grid">
        <dt>Field</dt>
        <dd>
          {cars ? `${cars} cars` : "–"}
          {clock?.strengthOfField ? `   SoF ${irating(clock.strengthOfField)}` : ""}
        </dd>
        {showClasses && classes.length > 1 &&
          classes.map((c) => (
            <ClassLine key={c.classId} name={c.name} color={c.color} cars={c.cars} sof={c.strengthOfField} />
          ))}
        {showConditions && <Conditions c={clock} units={units} />}
        {showLimits && <Limits c={clock} />}
      </dl>
    </Panel>
  );
}

function ClassLine({ name, color, cars, sof }: { name: string; color: string; cars: number; sof: number | null }) {
  return (
    <>
      <dt className="si-class" style={{ "--row-class": color } as React.CSSProperties}>{name || "Class"}</dt>
      <dd className="si-sub">
        {cars} cars{sof ? `   ${irating(sof)}` : ""}
      </dd>
    </>
  );
}

function Conditions({ c, units }: { c: SessionClock | undefined; units: Units }) {
  const sky = c?.skies != null ? SKIES[c.skies] : null;
  const wet = c?.trackWetness ? WETNESS[c.trackWetness] : null;
  const isWet = (c?.trackWetness ?? 0) >= 3;
  return (
    <>
      {c?.timeOfDay != null && (
        <>
          <dt>Time of day</dt>
          <dd>{timeOfDay(c.timeOfDay)}</dd>
        </>
      )}
      <dt>Air / track</dt>
      <dd>{c ? `${temp(c.airTemp, units)} / ${temp(c.trackTemp, units)}` : "–"}</dd>
      {(sky || wet) && (
        <>
          <dt>Sky / surface</dt>
          <dd className={isWet ? "si-wet" : undefined}>
            {[sky, wet].filter(Boolean).join(" / ")}
            {c?.declaredWet && <span className="tag">Wet</span>}
          </dd>
        </>
      )}
      {c?.windSpeed != null && (
        <>
          <dt>Wind</dt>
          <dd>
            {wind(c.windSpeed, units)}
            {c.windDir != null ? ` ${compass(c.windDir)}` : ""}
            {c.humidity != null ? `   ${Math.round(c.humidity * 100)}% hum.` : ""}
          </dd>
        </>
      )}
    </>
  );
}

function Limits({ c }: { c: SessionClock | undefined }) {
  // The step after the first limit is set on the Relative.
  const step = useLive((s) => opt(s.config.widgets.relative, "incidentStep", 4));
  if (!c) return null;
  const { next, passed } = incidentTarget(c.incidents, c.incidentLimit, step);
  const near = next != null && (passed > 0 || c.incidents >= next * 0.75);
  return (
    <>
      <dt>Incidents</dt>
      <dd className={near ? "si-warn" : undefined}>
        {c.incidents}x{next != null ? ` / ${next}x` : ""}
      </dd>
      {c.fastRepairsLimit != null && (
        <>
          <dt>Fast repairs</dt>
          <dd>{Math.max(0, c.fastRepairsLimit - c.fastRepairsUsed)} left</dd>
        </>
      )}
    </>
  );
}
