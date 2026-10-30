import { useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { widgetMeta, type WidgetConfig, type WidgetId } from "../lib/config";
import { COMPONENTS } from "../widgets/registry";
import { snap, type Guide, type Rect } from "./snapping";

const MIN_SCALE = 0.5;
const MAX_SCALE = 3;

interface Props {
  id: WidgetId;
  config: WidgetConfig;
  editing: boolean;
  /** Screen rects of the other visible widgets, for snapping. */
  others: () => Rect[];
  onRect: (id: WidgetId, rect: Rect | null) => void;
  onGuides: (g: Guide[]) => void;
  onCommit: (id: WidgetId, patch: Partial<WidgetConfig>) => void;
}

type Drag =
  | { mode: "move"; px: number; py: number; x: number; y: number }
  | { mode: "scale"; px: number; py: number; scale: number; w: number };

export function WidgetFrame({ id, config, editing, others, onRect, onGuides, onCommit }: Props) {
  const Widget = COMPONENTS[id];
  const ref = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  // While dragging we render from local state and only save on release.
  const [live, setLive] = useState<{ x: number; y: number; scale: number } | null>(null);
  const x = live?.x ?? config.x;
  const y = live?.y ?? config.y;
  const scale = live?.scale ?? config.scale;

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const report = () => onRect(id, { x, y, w: el.offsetWidth * scale, h: el.offsetHeight * scale });
    report();
    const ro = new ResizeObserver(report);
    ro.observe(el);
    return () => {
      ro.disconnect();
      onRect(id, null);
    };
  }, [id, x, y, scale, onRect]);

  const down = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!editing || e.button !== 0) return;
    const isHandle = (e.target as HTMLElement).closest(".frame-handle");
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = isHandle
      ? { mode: "scale", px: e.clientX, py: e.clientY, scale, w: ref.current!.offsetWidth * scale }
      : { mode: "move", px: e.clientX, py: e.clientY, x, y };
    e.preventDefault();
  };

  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    const el = ref.current;
    if (!d || !el) return;
    const screen = { w: window.innerWidth, h: window.innerHeight };
    if (d.mode === "move") {
      const w = el.offsetWidth * scale;
      const h = el.offsetHeight * scale;
      const s = snap({ x: d.x + e.clientX - d.px, y: d.y + e.clientY - d.py, w, h }, others(), screen);
      setLive({ x: s.x, y: s.y, scale });
      onGuides(s.guides);
    } else {
      const next = d.scale * ((d.w + e.clientX - d.px) / d.w);
      const clamped = Math.min(MAX_SCALE, Math.max(MIN_SCALE, Math.round(next * 20) / 20));
      setLive({ x, y, scale: clamped });
    }
  };

  const up = () => {
    if (!drag.current) return;
    drag.current = null;
    onGuides([]);
    if (live) onCommit(id, { x: Math.round(live.x), y: Math.round(live.y), scale: live.scale });
    setLive(null);
  };

  return (
    <div
      ref={ref}
      className={`frame ${editing ? "is-editing" : ""}`}
      style={{
        transform: `translate3d(${x}px, ${y}px, 0) scale(${scale})`,
        opacity: editing ? Math.max(config.opacity, 0.85) : config.opacity,
      }}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
    >
      <Widget config={config} editing={editing} />
      {editing && (
        <>
          <div className="frame-label" style={{ transform: `scale(${1 / scale})` }}>
            {widgetMeta(id).name}
            <span>{Math.round(scale * 100)}%</span>
          </div>
          <div className="frame-handle" style={{ transform: `scale(${1 / scale})` }} title="Drag to resize" />
        </>
      )}
    </div>
  );
}
