import { useEffect, useState } from "react";
import { WIDGETS, type WidgetId } from "../lib/config";
import { primaryKey, widgetDisplay } from "../lib/displays";
import { useLive } from "../lib/store";
import { WidgetFrame } from "../overlay/WidgetFrame";
import { COMPONENTS } from "../widgets/registry";
import { WidgetBoundary } from "../widgets/WidgetBoundary";

const params = new URLSearchParams(location.search);

export function Stream() {
  const widget = params.get("widget") as WidgetId | null;
  if (widget && WIDGETS.some((w) => w.id === widget)) return <OneWidget id={widget} />;
  if (params.has("layout")) return <Layout />;
  return <p className="stream-help">Add <code>?widget=relative</code> or <code>?layout</code> to this address.</p>;
}

/** One widget at the top left, at its own size (or ?scale=). */
function OneWidget({ id }: { id: WidgetId }) {
  const w = useLive((s) => s.config.widgets[id]);
  const loaded = useLive((s) => s.configLoaded);
  if (!loaded) return null;
  const Widget = COMPONENTS[id];
  const scale = Number(params.get("scale")) || 1;
  return (
    <div className="stream-one" style={{ transform: `scale(${scale})` }}>
      <WidgetBoundary name={id} editing={false} resetKey={w}>
        <Widget config={{ ...w, enabled: true, scale: 1, opacity: 1 }} editing={false} />
      </WidgetBoundary>
    </div>
  );
}

/**
 * The widgets on one display, where they are on screen. Size the Browser
 * Source like that display (in its CSS pixels, so 1920×1080 for a 4K screen at
 * 200%) and they line up with the overlay.
 */
function Layout() {
  const config = useLive((s) => s.config);
  const loaded = useLive((s) => s.configLoaded);
  const displays = useLive((s) => s.displays);
  const [screen, setScreen] = useState({ w: window.innerWidth, h: window.innerHeight });
  useEffect(() => {
    const resize = () => setScreen({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);
  if (!loaded) return null;
  const n = Number(params.get("display")) || 1;
  const key = displays[n - 1]?.key ?? primaryKey(displays);
  const here = WIDGETS.filter(
    (m) => config.widgets[m.id].enabled && (displays.length === 0 || widgetDisplay(config.widgets[m.id], displays) === key),
  );
  return (
    <div className="overlay is-stream">
      {here.map((m) => (
        <WidgetFrame
          key={m.id}
          id={m.id}
          config={config.widgets[m.id]}
          editing={false}
          screen={screen}
          display={null}
          displays={[]}
          others={() => []}
          onRect={() => {}}
          onGuides={() => {}}
          onCommit={() => {}}
        />
      ))}
    </div>
  );
}
