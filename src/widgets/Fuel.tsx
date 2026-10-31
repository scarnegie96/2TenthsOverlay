import { opt, type WidgetConfig } from "../lib/config";
import { fuel as conv, fuelUnit } from "../lib/format";
import { fuelToFinish } from "../lib/pit";
import { useLive } from "../lib/store";
import { Panel } from "./Panel";

export function Fuel({ config }: { config: WidgetConfig }) {
  const f = useLive((s) => s.field?.fuel);
  const units = useLive((s) => s.config.general.units);
  const margin = opt(config, "marginLaps", 0.5);
  const u = fuelUnit(units);
  const v = (l: number | null | undefined, d = 1) => (l == null ? "–" : conv(l, units).toFixed(d));

  const level = f?.level ?? 0;
  const tankPct = f && f.max > 0 ? Math.min(1, level / f.max) : 0;
  const need = fuelToFinish(f, margin);
  const enough = need != null && need <= 0;

  return (
    <Panel className="fuel" title="Fuel" aside={f?.samples ? `${f.samples} lap avg` : "Measuring"}>
      <div className="fuel-level">
        <span className="big">
          {v(level)}
          <small>{u}</small>
        </span>
        <div className="tank">
          <div className="tank-fill" style={{ transform: `scaleX(${tankPct})` }} />
        </div>
      </div>
      <dl className="fuel-grid">
        <dt>Per lap</dt>
        <dd>{v(f?.perLap, 2)}</dd>
        <dt>Laps in tank</dt>
        <dd>{f?.lapsOfFuel == null ? "–" : f.lapsOfFuel.toFixed(1)}</dd>
        <dt>Laps to go</dt>
        <dd>{f?.lapsToGo == null ? "–" : f.lapsToGo.toFixed(1)}</dd>
      </dl>
      <div className={`fuel-call ${need == null ? "" : enough ? "is-ok" : "is-add"}`}>
        {need == null
          ? "Complete a green lap to start the calculation"
          : enough
            ? `Enough to finish, ${v(-need)} ${u} spare`
            : `Add ${v(need)} ${u} to finish`}
      </div>
    </Panel>
  );
}
