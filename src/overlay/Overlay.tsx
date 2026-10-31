import { useCallback, useEffect, useRef, useState } from "react";
import { api, isTauri, on } from "../lib/bridge";
import { WIDGETS, type WidgetConfig, type WidgetId } from "../lib/config";
import { usePitControl } from "../lib/pit";
import { cycleProfile, switchProfile, updateConfig, useLive } from "../lib/store";
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
  const profiles = useLive((s) => s.profiles);
  const activeProfile = config.general.activeProfile;
  const toast = useLayoutToast(activeProfile, profiles.find((p) => p.id === activeProfile)?.name);
  usePitControl();

  // Rust owns the global shortcuts and controller buttons; keep it up to date with the config.
  const controls = config.general.controls;
  useEffect(() => {
    if (loaded) void api.setControls(controls);
  }, [loaded, controls]);

  useEffect(() => {
    // Ctrl+Shift+L and Ctrl+Shift+1..9, from the app's global shortcuts.
    const off = on<{ step: number | null; index: number | null }>("layout-shortcut", (t) => void cycleProfile(t));
    return () => void off.then((f) => f());
  }, []);

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
      // ...and L cycles through saved layouts.
      if (!isTauri && e.key.toLowerCase() === "l" && !e.ctrlKey && !e.metaKey) void cycleProfile({ step: 1 });
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
          <select
            className="edit-layout"
            aria-label="Layout"
            title={activeProfile ? "Changes save into this layout" : "Save layouts in Settings → Layouts"}
            value={activeProfile ?? ""}
            onChange={(e) => e.target.value && void switchProfile(e.target.value)}
          >
            {!activeProfile && <option value="">Unsaved layout</option>}
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
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
      {toast && <div key={toast.key} className="layout-toast">Layout: <b>{toast.name}</b></div>}
      {!isTauri && !editing && <div className="preview-hint">Browser preview. Press E to edit the layout, L to switch layouts.</div>}
    </div>
  );
}

/** Names the layout for a moment after it changes, so a shortcut press is confirmed. */
function useLayoutToast(id: string | null, name: string | undefined) {
  const [toast, setToast] = useState<{ key: number; name: string } | null>(null);
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    if (!id || !name) return;
    setToast({ key: Date.now(), name });
    const t = window.setTimeout(() => setToast(null), 1800);
    return () => window.clearTimeout(t);
    // Only on switching, not on a rename.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);
  return toast;
}
