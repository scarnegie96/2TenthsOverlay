import { opt, type WidgetConfig } from "../lib/config";
import { useLive } from "../lib/store";
import { CarLeftRight } from "../lib/types";

/**
 * iRacing tells us whether a car is overlapping on each side (CarLeftRight),
 * not exactly where, so this is a clear "someone's there" light per side.
 */
export function Spotter({ config, editing }: { config: WidgetConfig; editing: boolean }) {
  const clr = useLive((s) => s.tick?.carLeftRight ?? CarLeftRight.Off);
  const always = opt(config, "alwaysShow", false);

  const left =
    clr === CarLeftRight.CarLeft || clr === CarLeftRight.CarLeftRight ? 1 : clr === CarLeftRight.TwoCarsLeft ? 2 : 0;
  const right =
    clr === CarLeftRight.CarRight || clr === CarLeftRight.CarLeftRight ? 1 : clr === CarLeftRight.TwoCarsRight ? 2 : 0;
  const visible = editing || always || left > 0 || right > 0;

  return (
    <div className={`spotter ${visible ? "" : "is-hidden"}`}>
      <Side count={left} side="left" />
      <svg className="spotter-car" viewBox="0 0 40 80" aria-hidden>
        <rect x="6" y="4" width="28" height="72" rx="10" />
        <rect x="11" y="20" width="18" height="14" rx="3" className="glass" />
      </svg>
      <Side count={right} side="right" />
    </div>
  );
}

function Side({ count, side }: { count: number; side: string }) {
  return (
    <div className={`spotter-side spotter-${side} ${count ? "is-on" : ""} ${count > 1 ? "is-double" : ""}`}>
      <span />
      <span />
    </div>
  );
}
