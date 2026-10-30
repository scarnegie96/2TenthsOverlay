import { useCallback, useEffect, useRef, useState } from "react";
import { api, isTauri } from "../lib/bridge";
import { WIDGETS, type WidgetConfig, type WidgetId } from "../lib/config";
import { updateConfig, useLive } from "../lib/store";
import type { Guide, Rect } from "./snapping";
import { WidgetFrame } from "./WidgetFrame";

export function Overlay() {
  const config = useLive((s) => s.config);
  const loaded = useLive((s) => s.configLoaded);
  const editing = useLive((s) => s.editMode);
  const connected = useLive((s) => s.status?.connected ?? false);
  const onTrack = useLive((s) => s.tick?.isOnTrack ?? false);
  const [guides, setGuides] = useState<Guide[]>([]);
  // The list of every widget, for turning hidden ones back on without opening Settings.
  const [picker, setPicker] = useState(false);
  const rects = useRef(new Map<WidgetId, Rect>());
  const [screen, setScreen] = useState(() => ({ w: window.innerWidth, h: window.innerHeight }));

  useEffect(() => {
    // The overlay window follows the monitor, so this changes with resolution or display scaling.
    const resize = () => setScreen({ w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape" && editing) void api.setEditMode(false);
      // Browser preview has no global hotkey; E toggles editing instead.
      if (!isTauri && e.key.toLowerCase() === "e" && !e.ctrlKey && !e.metaKey) void api.setEditMode(!editing);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [editing]);

  const onRect = useCallback((id: WidgetId, r: Rect | null) => {
    if (r) rects.current.set(id, r);
    else rects.current.delete(id);
  }, []);

  const commit = useCallback((id: WidgetId, patch: Partial<WidgetConfig>) => {
    updateConfig((c) => ({ ...c, widgets: { ...c.widgets, [id]: { ...c.widgets[id], ...patch } } }));
  }, []);

  if (!loaded) return null;

  const show = editing || (connected && (config.general.showWhen === "connected" || onTrack));

  return (
    <div className={`overlay ${editing ? "is-editing" : ""} ${isTauri ? "" : "is-preview"}`}>
      {editing && (
        <div className="edit-bar">
          <span>Drag to move. Drag the corner, or use − and +, to resize.</span>
          <button
            className={`edit-widgets-btn ${picker ? "is-open" : ""}`}
            aria-expanded={picker}
            onClick={() => setPicker((p) => !p)}
          >
            Widgets {WIDGETS.filter((m) => config.widgets[m.id].enabled).length}/{WIDGETS.length}
          </button>
          <button onClick={() => void api.setEditMode(false)}>Done</button>
          {picker && (
            <div className="edit-widgets" role="group" aria-label="Show or hide widgets">
              {WIDGETS.map((m) => {
                const on = config.widgets[m.id].enabled;
                return (
                  <button key={m.id} role="switch" aria-checked={on} className={`edit-widget ${on ? "is-on" : ""}`}
                    title={m.description} onClick={() => commit(m.id, { enabled: !on })}>
                    <span className="edit-widget-check" aria-hidden />
                    {m.name}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      )}
      {guides.map((g, i) => (
        <div key={i} className={`guide guide-${g.axis}`} style={g.axis === "x" ? { left: g.pos } : { top: g.pos }} />
      ))}
      {show &&
        WIDGETS.filter((m) => config.widgets[m.id].enabled).map((m) => (
          <WidgetFrame
            key={m.id}
            id={m.id}
            config={config.widgets[m.id]}
            editing={editing}
            screen={screen}
            others={() => [...rects.current].filter(([k]) => k !== m.id).map(([, r]) => r)}
            onRect={onRect}
            onGuides={setGuides}
            onCommit={commit}
          />
        ))}
      {!isTauri && !editing && <div className="preview-hint">Browser preview. Press E to edit the layout.</div>}
    </div>
  );
}
