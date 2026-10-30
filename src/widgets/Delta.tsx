import { opt, type WidgetConfig } from "../lib/config";
import { lapTime, signed } from "../lib/format";
import { useLive } from "../lib/store";
import { Panel } from "./Panel";

export function Delta({ config }: { config: WidgetConfig }) {
  const delta = useLive((s) => s.tick?.deltaBest ?? null);
  const last = useLive((s) => s.tick?.lapLast ?? null);
  const best = useLive((s) => s.tick?.lapBest ?? null);
  const range = opt(config, "range", 1);
  const showLaps = opt(config, "showLaps", true);

  const d = delta ?? 0;
  const frac = Math.min(1, Math.abs(d) / range);
  const state = delta == null ? "" : d < -0.005 ? "is-gain" : d > 0.005 ? "is-loss" : "";

  // Number-line layout: faster (negative) grows left, slower grows right.
  const fill =
    d < 0
      ? { right: "50%", width: `${frac * 50}%` }
      : { left: "50%", width: `${frac * 50}%` };

  return (
    <Panel className={`delta ${state}`}>
      <div className="delta-value">{delta == null ? "–.––" : signed(d)}</div>
      <div className="delta-bar">
        <div className="delta-fill" style={fill} />
        <div className="delta-zero" />
      </div>
      {showLaps && (
        <div className="delta-laps">
          <span>
            Last <b>{lapTime(last)}</b>
          </span>
          <span>
            Best <b className="is-best">{lapTime(best)}</b>
          </span>
        </div>
      )}
    </Panel>
  );
}
