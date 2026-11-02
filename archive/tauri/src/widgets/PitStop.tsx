import { useEffect, useState, type ReactNode } from "react";
import { opt } from "../lib/config";
import { fullLabel, shortLabel, type ActionId } from "../lib/controls";
import { fuel as conv, fuelUnit, pressure } from "../lib/format";
import { fuelPlan, useAutoFuel, usePitPressed, usePitSent, type FuelMode } from "../lib/pit";
import { useLive } from "../lib/store";
import { Empty, Panel } from "./Panel";
import type { WidgetProps } from "./registry";

const CORNERS = ["LF", "RF", "LR", "RR"];
/** irsdk_PitSvStatus values from 100 up: why iRacing won't service the car. */
const PROBLEMS: Record<number, string> = {
  100: "Too far left",
  101: "Too far right",
  102: "Too far forward",
  103: "Too far back",
  104: "Bad angle",
  105: "Can't fix that",
};
const SENT_MS = 2500;
/** How long a key hint stays lit after its key or button is pressed. */
const LIT_MS = 300;

/**
 * What iRacing has ticked for your next stop, read back from its telemetry,
 * and the hotkeys that change it. iRacing never confirms a command, so this
 * always shows what it reports rather than what was asked for.
 */
export function PitStop({ config }: WidgetProps) {
  const pit = useLive((s) => s.field?.pit);
  const fuel = useLive((s) => s.field?.fuel);
  const sets = useLive((s) => s.field?.tires.setsAvailable ?? null);
  const units = useLive((s) => s.config.general.units);
  const margin = useLive((s) => opt(s.config.widgets.fuel, "marginLaps", 0.5));
  const inCar = useLive((s) => s.tick?.isOnTrack ?? false);
  const mode = opt<FuelMode>(config, "fuelMode", "finish");
  const autoOn = opt(config, "keepFuelSet", false);
  const autoPaused = useAutoFuel((s) => s.paused);
  const showKeys = opt(config, "showKeys", true);
  const controls = useLive((s) => s.config.general.controls);
  const lit = useLitAction();
  /** Hints for a row, a line per group: what's bound to each action, or nothing when hints are off. */
  const hints = (...groups: ActionId[][]): Hint[][] =>
    showKeys
      ? groups
          .map((ids) =>
            ids.flatMap((id) => {
              const label = shortLabel(controls[id]);
              return label ? [{ id, label, lit: lit === id }] : [];
            }),
          )
          .filter((line) => line.length > 0)
      : [];
  const clear = fullLabel(controls["pit.clear"]);
  const sent = useRecentlySent();

  const u = fuelUnit(units);
  const v = (l: number) => conv(l, units).toFixed(units === "imperial" ? 1 : 0);
  const plan = fuelPlan(mode, fuel, margin);

  if (!pit?.available) {
    return (
      <Panel className="pitstop" title="Pit stop">
        <Empty>Pit service shows here once you're in the car.</Empty>
      </Panel>
    );
  }

  const problem = PROBLEMS[pit.status];
  const aside = problem ?? (pit.status === 1 ? "Servicing" : pit.status === 2 ? "Service done" : "Next stop");
  const anyTire = pit.tires.some(Boolean);

  let fuelNote: ReactNode;
  let fuelWarn = false;
  const finish = fuelPlan("finish", fuel, margin);
  if (autoOn) {
    // Auto fuel to finish keeps iRacing's amount right, so the note says what it's doing.
    if (autoPaused) {
      fuelNote = finish == null ? "Auto paused: set by hand" : `Auto paused · needs ${v(finish)} ${u}`;
      fuelWarn = finish != null && finish > 0 && (!pit.fuel || Math.abs(pit.fuelAmount - finish) >= 1);
    } else {
      fuelNote = finish == null ? "Auto: plan after a green lap" : finish === 0 ? "Auto: enough to finish" : `Auto: ${v(finish)} ${u} to finish`;
    }
  } else if (mode === "manual") {
    fuelNote = "Hotkey keeps iRacing's amount";
  } else if (plan == null) {
    fuelNote = "Plan after a green lap";
  } else if (plan === 0) {
    fuelNote = mode === "full" ? "Tank is full" : "Enough to finish";
    fuelWarn = pit.fuel;
  } else {
    fuelWarn = !pit.fuel || Math.abs(pit.fuelAmount - plan) >= 1;
    fuelNote = mode === "full" ? `${v(plan)} ${u} fills the tank` : `Needs ${v(plan)} ${u} to finish`;
  }

  return (
    <Panel className="pitstop" title="Pit stop" aside={<span className={problem ? "pit-problem" : ""}>{aside}</span>}>
      <Row label="Fuel" on={pit.fuel} hotkeys={hints(["pit.fuel", "pit.autoFuel"], ["pit.fuelUp", "pit.fuelDown"])}>
        <span className="pit-value">
          {pit.fuel ? `+${v(pit.fuelAmount)} ${u}` : "No fuel"}
          {autoOn && <span className={`pit-auto ${autoPaused ? "is-paused" : ""}`} title="Auto fuel to finish">AUTO</span>}
        </span>
        <span className={`pit-note ${fuelWarn ? "is-warn" : ""}`}>{fuelNote}</span>
      </Row>
      <Row label="Tires" on={anyTire} hotkeys={hints(["pit.tires"])}>
        <div className="pit-tires">
          {CORNERS.map((c, k) => (
            <span key={c} className={`pit-tire ${pit.tires[k] ? "is-on" : ""}`}>
              {c}
              {pit.pressures[k] > 0 && <b>{pressure(pit.pressures[k], units)}</b>}
            </span>
          ))}
        </div>
        <span className="pit-note">{[pit.compound, sets != null && `${sets} sets left`].filter(Boolean).join(" · ")}</span>
      </Row>
      <Row label="Fast repair" on={pit.fastRepair} hotkeys={hints(["pit.fastRepair"])}>
        {pit.fastRepairsLeft != null && (
          <span className={`pit-note ${pit.fastRepair && pit.fastRepairsLeft === 0 ? "is-warn" : ""}`}>
            {pit.fastRepairsLeft === 0 ? "None left" : `${pit.fastRepairsLeft} left`}
          </span>
        )}
      </Row>
      <Row label="Tear-off" on={pit.tearoff} hotkeys={hints(["pit.tearoff"])} />
      {(sent || showKeys) && (
        <div className={`pit-foot ${sent ? "is-sent" : ""}`}>
          {sent ?? (!inCar ? "Hotkeys work while you're in the car" : clear ? `${clear} unticks everything` : "Set keys in Settings → Controls")}
        </div>
      )}
    </Panel>
  );
}

interface Hint {
  id: ActionId;
  label: string;
  /** Just pressed. */
  lit: boolean;
}

function Row({ label, on, hotkeys, children }: { label: string; on: boolean; hotkeys: Hint[][]; children?: ReactNode }) {
  return (
    <div className={`pit-row ${on ? "is-on" : ""}`}>
      <span className="pit-check" aria-label={on ? "Ticked" : "Not ticked"} />
      <span className="pit-label">{label}</span>
      <div className="pit-detail">{children}</div>
      {hotkeys.length > 0 && (
        <span className="pit-keys">
          {hotkeys.map((line) => (
            <span key={line[0].id} className="pit-key-line">
              {line.map((k) => (
                <kbd key={k.id} className={`pit-key ${k.lit ? "is-lit" : ""}`}>{k.label}</kbd>
              ))}
            </span>
          ))}
        </span>
      )}
    </div>
  );
}

/** The pit action pressed in the last moment, as its action id, so its key hint lights up. */
function useLitAction(): ActionId | null {
  const { action, at } = usePitPressed();
  const [, expire] = useState(0);
  useEffect(() => {
    if (!action) return;
    const t = setTimeout(() => expire((n) => n + 1), LIT_MS);
    return () => clearTimeout(t);
  }, [action, at]);
  return action && performance.now() - at < LIT_MS ? (`pit.${action}` as ActionId) : null;
}

/** The last command sent from a hotkey, for a couple of seconds. */
function useRecentlySent(): string | null {
  const { text, at } = usePitSent();
  const [, expire] = useState(0);
  useEffect(() => {
    if (!text) return;
    const t = setTimeout(() => expire((n) => n + 1), SENT_MS);
    return () => clearTimeout(t);
  }, [text, at]);
  return text && performance.now() - at < SENT_MS ? text : null;
}
