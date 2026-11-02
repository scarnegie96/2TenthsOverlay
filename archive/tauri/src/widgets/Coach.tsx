import { opt, type WidgetConfig } from "../lib/config";
import { cueText, issueText } from "../lib/coach";
import { signed, speed, speedUnit } from "../lib/format";
import { useLive } from "../lib/store";
import { Panel } from "./Panel";

/**
 * One live coaching message comparing you with the imported reference lap:
 * where to brake, how hard, apex speed, throttle pickup, gear and line, and a
 * verdict as you leave each corner. Below it, the corner costing you most.
 */
export function Coach({ config, editing }: { config: WidgetConfig; editing: boolean }) {
  const coach = useLive((s) => s.tick?.coach ?? null);
  const delta = useLive((s) => s.tick?.deltas.reference ?? null);
  const reference = useLive((s) => s.reference);
  const units = useLive((s) => s.config.general.units);
  const showDetail = opt(config, "showDetail", true);
  const showStats = opt(config, "showStats", true);
  const showFocus = opt(config, "showFocus", true);

  if (!reference?.loaded) {
    return editing ? (
      <Panel className="coach is-idle">
        <div className="coach-label">Coach</div>
        <div className="coach-detail">Import a reference lap in settings (Delta) to get live coaching.</div>
      </Panel>
    ) : null;
  }
  if (!reference.matchesTrack) {
    return editing ? (
      <Panel className="coach is-idle">
        <div className="coach-label">Coach paused</div>
        <div className="coach-detail">The reference lap is for {reference.trackName}.</div>
      </Panel>
    ) : null;
  }

  const cue = coach?.cue ?? null;
  const text = cue
    ? cueText(cue, units)
    : coach
      ? { label: "Coaching paused", detail: "Off track or under yellow" }
      : { label: "Waiting for you to drive", detail: "Coaching starts on track" };
  const sd = coach?.speedDelta ?? null;
  const focus = coach?.focus ?? null;

  return (
    <Panel className={`coach tone-${cue?.tone ?? "neutral"}`}>
      <div className="coach-main" key={cue?.kind ?? "none"}>
        <div className="coach-label">{text.label}</div>
        {showDetail && <div className="coach-detail">{text.detail}</div>}
      </div>
      {showStats && (
        <div className="coach-stats">
          {cue?.corner != null && <span>Corner {cue.corner}</span>}
          <span className={delta == null ? "" : delta > 0 ? "is-loss" : "is-gain"}>
            {delta == null ? "–" : `${signed(delta)}s`}
          </span>
          <span className={sd == null ? "" : sd < -0.3 ? "is-loss" : sd > 0.3 ? "is-gain" : ""}>
            {sd == null ? "–" : `${sd >= 0 ? "+" : "−"}${speed(Math.abs(sd), units)} ${speedUnit(units)}`}
          </span>
        </div>
      )}
      {showFocus && focus && (
        <div className="coach-focus">
          <span className="coach-focus-tag">Focus</span>
          <span>Corner {focus.corner}{focus.issue ? `, ${issueText(focus.issue)}` : ""}</span>
          <span className="is-loss">+{focus.lossS.toFixed(2)}s</span>
        </div>
      )}
    </Panel>
  );
}
