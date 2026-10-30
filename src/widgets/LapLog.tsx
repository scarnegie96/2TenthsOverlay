import { opt, type WidgetConfig } from "../lib/config";
import { fuel, fuelUnit, lapTime, signed, type Units } from "../lib/format";
import { useLive } from "../lib/store";
import type { LapRecord } from "../lib/types";
import { Empty, Panel } from "./Panel";

const temp = (c: number, u: Units) => `${Math.round(u === "imperial" ? (c * 9) / 5 + 32 : c)}°`;

/** "today", "yesterday", "12 days ago" */
function age(unix: number) {
  const days = Math.floor((Date.now() / 1000 - unix) / 86400);
  return days <= 0 ? "today" : days === 1 ? "yesterday" : `${days} days ago`;
}

/** Every lap this session, newest first, with the session and all-time bests on top. */
export function LapLog({ config }: { config: WidgetConfig }) {
  const log = useLive((s) => s.field?.lapLog);
  const units = useLive((s) => s.config.general.units);
  const rows = opt(config, "rows", 8);
  const compareTo = opt<string>(config, "compareTo", "session");
  const showDelta = opt(config, "showDelta", true);
  const showTemp = opt(config, "showTemp", true);
  const showFuel = opt(config, "showFuel", false);
  const showPersonalBest = opt(config, "showPersonalBest", true);

  const sessionBest = log?.sessionBest ?? null;
  const personalBest = log?.personalBest ?? null;
  const best = compareTo === "personal" ? personalBest?.time ?? sessionBest : sessionBest;
  const laps = [...(log?.laps ?? [])].reverse().slice(0, rows);
  const cols = ["34px", "1fr", showDelta && "64px", showTemp && "40px", showFuel && "62px"].filter(Boolean).join(" ");

  return (
    <Panel className="laplog" title="Lap log" aside={log?.laps.length ? `${log.laps.length} laps` : undefined}>
      <div className="ll-bests">
        <div className="ll-best is-session">
          <span>Session best</span>
          <b>{lapTime(sessionBest)}</b>
        </div>
        {showPersonalBest && (
          <div className="ll-best" title={log?.saving ? "Saved for this car and track" : "Not saved in the demo"}>
            <span>Personal best{personalBest ? ` · ${age(personalBest.at)}` : ""}</span>
            <b>{lapTime(personalBest?.time)}</b>
          </div>
        )}
      </div>
      <div className="ll-row ll-head" style={{ gridTemplateColumns: cols }}>
        <span>Lap</span>
        <span>Time</span>
        {showDelta && <span className="num">{compareTo === "personal" ? "To PB" : "To best"}</span>}
        {showTemp && <span className="num">Track</span>}
        {showFuel && <span className="num">Fuel</span>}
      </div>
      {laps.length === 0 ? (
        <Empty>Laps appear here as you complete them.</Empty>
      ) : (
        laps.map((l) => (
          <Row key={l.lap} lap={l} best={best} sessionBest={sessionBest} cols={cols} units={units}
            showDelta={showDelta} showTemp={showTemp} showFuel={showFuel} />
        ))
      )}
    </Panel>
  );
}

function Row({ lap, best, sessionBest, cols, units, showDelta, showTemp, showFuel }: {
  lap: LapRecord; best: number | null; sessionBest: number | null; cols: string; units: Units;
  showDelta: boolean; showTemp: boolean; showFuel: boolean;
}) {
  const isSessionBest = lap.clean && lap.time != null && lap.time === sessionBest;
  const delta = lap.time != null && best != null ? lap.time - best : null;
  const why = [lap.offTrack && "off track", lap.incidents > 0 && `${lap.incidents}x`, lap.pit && "pit lane"].filter(Boolean).join(", ");
  return (
    <div className={`ll-row ${lap.clean ? "" : "is-invalid"} ${isSessionBest ? "is-best" : ""}`} style={{ gridTemplateColumns: cols }}>
      <span className="ll-lap">{lap.lap}</span>
      <span className="ll-time" title={why ? `Not clean: ${why}` : undefined}>
        {lapTime(lap.time)}
        {!lap.clean && !lap.pit && <i className="ll-dot" aria-label="Not clean" />}
        {lap.pit && <i className="ll-tag">Pit</i>}
        {lap.personalBest && <i className="ll-tag is-pb">PB</i>}
      </span>
      {showDelta && (
        <span className={`num ${delta == null || !lap.clean ? "" : delta <= 0 ? "is-gain" : "is-loss"}`}>
          {delta == null ? "–" : signed(delta, 3)}
        </span>
      )}
      {showTemp && <span className="num muted">{temp(lap.trackTemp, units)}</span>}
      {showFuel && (
        <span className="num muted">{lap.fuelUsed == null ? "–" : `${fuel(lap.fuelUsed, units).toFixed(2)} ${fuelUnit(units)}`}</span>
      )}
    </div>
  );
}
