import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { api, isTauri } from "../lib/bridge";
import { widgetMeta, type WidgetConfig, type WidgetId } from "../lib/config";
import { cssSize, dropOnto, nextDisplay } from "../lib/displays";
import type { Display } from "../lib/types";
import { COMPONENTS } from "../widgets/registry";
import { WidgetBoundary } from "../widgets/WidgetBoundary";
import { clampOnScreen, resolvePosition, toAnchored, type Size } from "./anchor";
import { snap, type Guide, type Rect } from "./snapping";

const MIN_SCALE = 0.5;
const MAX_SCALE = 3;
const SCALE_STEP = 0.05;

/** How often to ask where the mouse is while a widget is dragged (ms). */
const POINTER_POLL_MS = 25;

const clampScale = (s: number) => Math.min(MAX_SCALE, Math.max(MIN_SCALE, Math.round(s / SCALE_STEP) * SCALE_STEP));

interface Props {
  id: WidgetId;
  config: WidgetConfig;
  editing: boolean;
  /** Size of the overlay in CSS px. */
  screen: Size;
  /** The display this overlay window covers, and all of them (for moving between screens). */
  display: Display | null;
  displays: Display[];
  /** Screen rects of the other visible widgets, for snapping. */
  others: () => Rect[];
  onRect: (id: WidgetId, rect: Rect | null) => void;
  onGuides: (g: Guide[]) => void;
  onCommit: (id: WidgetId, patch: Partial<WidgetConfig>) => void;
}

type Drag =
  | { mode: "move"; px: number; py: number; x: number; y: number; moved: boolean }
  | { mode: "scale"; px: number; py: number; scale: number; w: number };

/**
 * Positions one widget. Saved positions are relative to the nearest screen
 * edge or centre (see anchor.ts), so layouts carry over between 1080p, 1440p
 * and 4K. While dragging we work in absolute pixels and re-anchor on release.
 */
export function WidgetFrame({ id, config, editing, screen, display, displays, others, onRect, onGuides, onCommit }: Props) {
  const Widget = COMPONENTS[id];
  const ref = useRef<HTMLDivElement>(null);
  const drag = useRef<Drag | null>(null);
  // Unscaled size of the widget's content, measured.
  const [natural, setNatural] = useState<Size | null>(null);
  // While dragging we render from local state and only save on release.
  const [live, setLive] = useState<{ x: number; y: number; scale: number } | null>(null);
  // While dragging past this window's edge: the number of the display under
  // the pointer, for the hint, or 0 when it's past the edge but over no display.
  const [leaving, setLeaving] = useState<number | null>(null);
  // The same as `live`, for the pointer poll, which outlives a render.
  const liveRef = useRef(live);
  liveRef.current = live;
  // Set while the pointer poll runs (see startPointerPoll).
  const polling = useRef(false);
  // When the DOM last reported a move, so the poll only steps in when it's gone quiet.
  const lastDomMove = useRef(0);
  // A pointerup the poll was left to handle, in case the poll can't carry on.
  const pendingUp = useRef<{ clientX: number; clientY: number } | null>(null);
  useEffect(() => () => void (polling.current = false), []);

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
      : { mode: "move", px: e.clientX, py: e.clientY, x, y, moved: false };
    e.preventDefault();
    if (!isHandle) startPointerPoll();
  };

  /**
   * WebView2 stops sending pointer events once the mouse leaves its window
   * (and may end the drag there with a pointerup or a cancel), so a drag onto
   * another display can't be followed from the DOM. While moving a widget
   * with more than one display, ask the app where the mouse is on the desktop
   * and whether the button is still held. That drives the hint outside the
   * window, keeps the widget following if the DOM goes quiet, and finishes
   * the drag when the button actually comes up, wherever the mouse is.
   */
  const startPointerPoll = () => {
    if (!isTauri || !display || displays.length < 2 || polling.current) return;
    polling.current = true;
    pendingUp.current = null;
    const from = display;
    const stop = () => {
      polling.current = false;
      // The DOM's pointerup was left to the poll: use it after all.
      const up = pendingUp.current;
      pendingUp.current = null;
      if (up && drag.current) finish(up);
    };
    const tick = async () => {
      if (!polling.current || !drag.current) return stop();
      const p = await api.pointer().catch(() => null);
      if (!polling.current || !drag.current) return stop();
      if (!p) return stop(); // can't tell here: DOM events only
      const at = { clientX: (p.x - from.x) / from.scale, clientY: (p.y - from.y) / from.scale };
      const d = drag.current;
      if (!p.down) {
        // Let go: wherever it is, inside this window or over another display.
        if (d.mode === "move" && (d.moved || !inWindow(at))) d.moved = true;
        pendingUp.current = null;
        finish(at);
        return;
      }
      if (d.mode === "move") {
        if (!inWindow(at)) {
          d.moved = true;
          setLeaving(crossing(at)?.display.number ?? 0);
        } else if (performance.now() - lastDomMove.current > 80) {
          moveTo(at);
        }
      }
      window.setTimeout(() => void tick(), POINTER_POLL_MS);
    };
    void tick();
  };

  const inWindow = (e: { clientX: number; clientY: number }) =>
    e.clientX >= 0 && e.clientY >= 0 && e.clientX < screen.w && e.clientY < screen.h;

  /** Moves the widget with the pointer (in this window's CSS px). */
  const moveTo = (e: { clientX: number; clientY: number }) => {
    const d = drag.current;
    if (!d || d.mode !== "move") return;
    d.moved = true;
    const s = snap({ x: d.x + e.clientX - d.px, y: d.y + e.clientY - d.py, w: size.w, h: size.h }, others(), screen);
    setLive({ x: s.x, y: s.y, scale });
    onGuides(s.guides);
    setLeaving(inWindow(e) ? null : crossing(e)?.display.number ?? 0);
  };

  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    lastDomMove.current = performance.now();
    if (d.mode === "move") {
      moveTo(e);
    } else {
      setLive({ x, y, scale: clampScale(d.scale * ((d.w + e.clientX - d.px) / d.w)) });
    }
  };

  /**
   * The pointer is past this window's edge and over another display: where the
   * widget would land there. The window keeps the pointer while a button is
   * held, so it still sees it over the other screen.
   */
  const crossing = (e: { clientX: number; clientY: number }) => {
    const d = drag.current;
    if (!d || d.mode !== "move" || !display || displays.length < 2) return null;
    if (inWindow(e)) return null;
    return dropOnto(display, e.clientX, e.clientY, { x: d.px - d.x, y: d.py - d.y }, displays);
  };

  /**
   * While the poll runs it decides when the drag ends, from the real button
   * state: WebView2 can send a pointerup or a cancel as the mouse leaves the
   * window, with the button still held.
   */
  const up = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (polling.current) pendingUp.current = { clientX: e.clientX, clientY: e.clientY };
    else finish(e);
  };
  const cancel = (e: ReactPointerEvent<HTMLDivElement>) => up(e);

  /** Ends a drag where the pointer was let go. */
  const finish = (e: { clientX: number; clientY: number }) => {
    polling.current = false;
    pendingUp.current = null;
    if (!drag.current) return;
    const across = drag.current.mode === "move" && drag.current.moved ? crossing(e) : null;
    drag.current = null;
    onGuides([]);
    setLeaving(null);
    if (across) {
      // Dropped on another display: anchor it there, at the same scale.
      const sz = natural ? { w: natural.w * scale, h: natural.h * scale } : size;
      const target = cssSize(across.display);
      const p = clampOnScreen(across.left, across.top, sz, target);
      onCommit(id, { ...toAnchored(p.x, p.y, sz, target), scale, display: across.display.key });
    } else if (liveRef.current) {
      const l = liveRef.current;
      save(l.x, l.y, l.scale);
    }
    setLive(null);
  };

  /** Sends the widget to the next display, at the same spot relative to its edges. */
  const next = nextDisplay(display?.key ?? null, displays);
  const toNextDisplay = () => {
    if (!next || !natural) return;
    // Anchor it on this screen first, so it lands in the matching place on the other.
    const anchored = toAnchored(x, y, size, screen);
    onCommit(id, { ...anchored, display: next.key });
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
      className={`frame ${editing ? "is-editing" : ""} ${leaving ? "is-leaving" : ""}`}
      style={{
        transform: `translate3d(${x}px, ${y}px, 0) scale(${scale})`,
        opacity: editing ? Math.max(config.opacity, 0.85) : config.opacity,
        visibility: natural ? undefined : "hidden",
      }}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={cancel}
      onLostPointerCapture={cancel}
    >
      <WidgetBoundary name={id} editing={editing} resetKey={config}>
        <Widget config={config} editing={editing} />
      </WidgetBoundary>
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
              {next && (
                <button className="frame-display" title={`Move to display ${next.number}`} onClick={toNextDisplay}>
                  ⇥ {next.number}
                </button>
              )}
              <button className="frame-hide" title="Hide this widget (turn it back on from Widgets in the edit bar)" onClick={() => onCommit(id, { enabled: false })}>
                Hide
              </button>
            </span>
          </div>
          <div className="frame-handle" style={{ transform: `scale(${1 / scale})` }} title="Drag to resize" />
          {leaving !== null && (
            <div className="frame-leaving" style={{ transform: `translate(-50%, -50%) scale(${1 / scale})` }}>
              {leaving ? `Release to move to display ${leaving}` : "Move onto another display"}
            </div>
          )}
        </>
      )}
    </div>
  );
}
