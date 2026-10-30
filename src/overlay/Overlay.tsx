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
  const rects = useRef(new Map<WidgetId, Rect>());

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
          <span>Drag widgets to move them. Drag the corner handle to resize.</span>
          <button onClick={() => void api.setEditMode(false)}>Done</button>
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
