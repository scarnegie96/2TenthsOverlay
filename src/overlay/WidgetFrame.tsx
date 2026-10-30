import { useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { widgetMeta, type WidgetConfig, type WidgetId } from "../lib/config";
import { COMPONENTS } from "../widgets/registry";
import { clampOnScreen, resolvePosition, toAnchored, type Size } from "./anchor";
import { snap, type Guide, type Rect } from "./snapping";

const MIN_SCALE = 0.5;
const MAX_SCALE = 3;
const SCALE_STEP = 0.05;

const clampScale = (s: number) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, Math.round(s / SCALE_STEP) * SCALE_STEP));

interface Props {
  id: WidgetId;
  config: WidgetConfig;
  editing: boolean;
  /** Size of the overlay in CSS px. */
  screen: Size;
  /** Screen rects of the other visible widgets, for snapping. */
  others: () => Rect[];
  onRect: (id: WidgetId, rect: Rect | null) => void;
  onGuides: (g: Guide[]) => void;
  onCommit: (id: WidgetId, patch: Partial<WidgetConfig>) => void;
}

type Drag =
  | { mode: "move"; px: number; py: number; x: number; y: number }
  | { mode: "scale"; px: number; py: number; scale: number; w: number };

/**
 * Positions one widget. Saved positions are relative to the nearest screen
 * edge or centre (see anchor.ts), so layouts carry over between 1080p, 1440p
 * and 4K. While dragging we work in absolute pixels and re-anchor on release.
 */
export function WidgetFrame({ id, config, editing, screen, others, onRect, onGuides, onCommit }: Props) {
  const Widget = COMPONENTS[id];
  const ref = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  // Unscaled size of the widget's content, measured.
  const [natural, setNatural] = useState<Size | null>(null);
  // While dragging we render from local state and only save on release.
  const [live, setLive] = useState<{ x: number; y: number; scale: number } | null>(null);

  const scale = live?.scale ?? config.scale;
  const size: Size = natural ? { w: natural.w * scale, h: natural.h * scale } : { w: 0, h: 0 };
  const pos = live ?? resolvePosition(config, size, screen, natural ?? size);
  const { x, y } = pos;

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const measure = () =>
      setNatural((prev) =>
        prev && prev.w === el.offsetWidth && prev.h === el.offsetHeight ? prev : { w: el.offsetWidth, h: el.offsetHeight },
      );
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useLayoutEffect(() => {
    if (!natural) return;
    onRect(id, { x, y, w: size.w, h: size.h });
  }, [id, x, y, size.w, size.h, natural, onRect]);
  useLayoutEffect(() => () => onRect(id, null), [id, onRect]);

  /** Save an absolute position (and scale) as an anchored one. */
  const save = (left: number, top: number, s: number) => {
    const sz = natural ? { w: natural.w * s, h: natural.h * s } : size;
    const p = clampOnScreen(left, top, sz, screen);
    onCommit(id, { ...toAnchored(p.x, p.y, sz, screen), scale: s });
  };

  const down = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (!editing || e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest(".frame-tools")) return;
    const isHandle = target.closest(".frame-handle");
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = isHandle
      ? { mode: "scale", px: e.clientX, py: e.clientY, scale, w: size.w }
      : { mode: "move", px: e.clientX, py: e.clientY, x, y };
    e.preventDefault();
  };

  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    if (d.mode === "move") {
      const s = snap({ x: d.x + e.clientX - d.px, y: d.y + e.clientY - d.py, w: size.w, h: size.h }, others(), screen);
      setLive({ x: s.x, y: s.y, scale });
      onGuides(s.guides);
    } else {
      setLive({ x, y, scale: clampScale(d.scale * ((d.w + e.clientX - d.px) / d.w)) });
    }
  };

  const up = () => {
    if (!drag.current) return;
    drag.current = null;
    onGuides([]);
    if (live) save(live.x, live.y, live.scale);
    setLive(null);
  };

  /** Resize from the edge the widget is pinned to, so it grows into the screen. */
  const nudgeScale = (by: number) => {
    const next = clampScale(scale + by);
    if (next === scale || !natural) return;
    const a = toAnchored(x, y, size, screen);
    const nw = natural.w * next;
    const nh = natural.h * next;
    const left = a.ax === "l" ? x : a.ax === "r" ? x + size.w - nw : x + (size.w - nw) / 2;
    const top = a.ay === "t" ? y : a.ay === "b" ? y + size.h - nh : y + (size.h - nh) / 2;
    save(left, top, next);
  };

  // The label sits above the widget, or below it when there's no room above.
  const labelBelow = y < 34;

  return (
    <div
      ref={ref}
      className={`frame ${editing ? "is-editing" : ""}`}
      style={{
        transform: `translate3d(${x}px, ${y}px, 0) scale(${scale})`,
        opacity: editing ? Math.max(config.opacity, 0.85) : config.opacity,
        visibility: natural ? undefined : "hidden",
      }}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
    >
      <Widget config={config} editing={editing} />
      {editing && (
        <>
          <div
            className={`frame-label ${labelBelow ? "is-below" : ""}`}
            style={{ transform: `scale(${1 / scale})` }}
          >
            <span className="frame-name">{widgetMeta(id).name}</span>
            <span className="frame-tools" onPointerDown={(e) => e.stopPropagation()}>
              <button title="Smaller" disabled={scale <= MIN_SCALE} onClick={() => nudgeScale(-0.1)}>−</button>
              <span className="frame-scale">{Math.round(scale * 100)}%</span>
              <button title="Bigger" disabled={scale >= MAX_SCALE} onClick={() => nudgeScale(0.1)}>+</button>
              <button className="frame-hide" title="Hide this widget (turn it back on from Widgets in the edit bar)" onClick={() => onCommit(id, { enabled: false })}>
                Hide
              </button>
            </span>
          </div>
          <div className="frame-handle" style={{ transform: `scale(${1 / scale})` }} title="Drag to resize" />
        </>
      )}
    </div>
  );
}
