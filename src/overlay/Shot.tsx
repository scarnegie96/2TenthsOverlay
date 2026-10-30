import { useLayoutEffect, useRef } from "react";
import type { WidgetId } from "../lib/config";
import { useLive } from "../lib/store";
import { COMPONENTS } from "../widgets/registry";

/**
 * Browser preview only: `overlay.html?shot=relative` draws one widget on its
 * own for README screenshots. Its size goes on <body data-shot-size="w,h"> so
 * scripts/screenshots.mjs can size the headless browser window to fit.
 */
export function Shot({ id }: { id: WidgetId }) {
  const config = useLive((s) => s.config);
  const loaded = useLive((s) => s.configLoaded);
  const box = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      document.body.dataset.shotSize = `${Math.ceil(el.offsetWidth)},${Math.ceil(el.offsetHeight)}`;
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [loaded]);

  if (!loaded) return null;
  const Widget = COMPONENTS[id];
  const w = config.widgets[id];
  return (
    <div className="overlay is-preview is-shot">
      <div ref={box} className={`shot shot-${id}`}>
        <Widget config={{ ...w, enabled: true, scale: 1, opacity: 1, options: { ...w.options, alwaysShow: true } }} editing={false} />
      </div>
    </div>
  );
}
