import { useEffect, useState, type ReactNode } from "react";
import { api, isTauri, on } from "../lib/bridge";
import {
  WIDGETS, defaultConfig, defaultWidgetConfig, widgetMeta,
  type AppConfig, type GeneralConfig, type OptionSpec, type WidgetConfig, type WidgetId,
} from "../lib/config";
import {
  ACTIONS, buttonLabel, defaultControls, keyLabel, sameButton, sameKey,
  type ActionId, type ControlBinding, type KeyBinding,
} from "../lib/controls";
import { lapTime } from "../lib/format";
import { deleteProfile, saveNewProfile, switchProfile, updateConfig, useLive } from "../lib/store";
import { COLOR_GROUPS, THEMES, THEME_COLORS, effectiveColor, type ColorKey } from "../lib/theme";
import type { ButtonPress, Controller, IbtSummary } from "../lib/types";

type Pane = "general" | "layouts" | "appearance" | "controls" | WidgetId;

export function Settings() {
  const [pane, setPane] = useState<Pane>("general");
  const config = useLive((s) => s.config);
  const loaded = useLive((s) => s.configLoaded);
  if (!loaded) return null;

  return (
    <div className="app">
      <TopBar />
      <nav className="nav" aria-label="Sections">
        <NavItem active={pane === "general"} onClick={() => setPane("general")}>
          General
        </NavItem>
        <NavItem active={pane === "layouts"} onClick={() => setPane("layouts")}>
          Layouts
        </NavItem>
        <NavItem active={pane === "appearance"} onClick={() => setPane("appearance")}>
          Appearance
        </NavItem>
        <NavItem active={pane === "controls"} onClick={() => setPane("controls")}>
          Controls
        </NavItem>
        <div className="nav-heading">Widgets</div>
        {WIDGETS.map((w) => (
          <NavItem key={w.id} active={pane === w.id} onClick={() => setPane(w.id)}>
            {w.name}
            <span className={`dot ${config.widgets[w.id].enabled ? "is-on" : ""}`} aria-label={config.widgets[w.id].enabled ? "shown" : "hidden"} />
          </NavItem>
        ))}
      </nav>
      <main className="pane">
        {pane === "general" ? <GeneralPane />
          : pane === "layouts" ? <LayoutsPane />
          : pane === "appearance" ? <AppearancePane />
          : pane === "controls" ? <ControlsPane />
          : <WidgetPane id={pane} />}
      </main>
    </div>
  );
}

function NavItem({ active, onClick, children }: { active: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button className={`nav-item ${active ? "is-active" : ""}`} onClick={onClick} aria-current={active ? "page" : undefined}>
      {children}
    </button>
  );
}

function TopBar() {
  const status = useLive((s) => s.status);
  const editing = useLive((s) => s.editMode);
  const visible = useLive((s) => s.overlayVisible);
  const label = !status
    ? "Starting"
    : status.source === "demo"
      ? "Demo race running"
      : status.connected
        ? "Connected to iRacing"
        : status.message || "Waiting for iRacing";
  const tone = status?.connected ? (status.source === "demo" ? "demo" : "live") : "idle";

  return (
    <header className="topbar">
      <div className="brand">
        <svg viewBox="0 0 64 64" aria-hidden>
          <g transform="skewX(-14) translate(9 0)">
            <rect x="14" y="17" width="8" height="30" rx="1.5" fill="currentColor" />
            <rect x="27" y="17" width="8" height="30" rx="1.5" fill="currentColor" />
            <rect x="40" y="33" width="8" height="14" rx="1.5" className="brand-accent" />
          </g>
        </svg>
        Tenths
      </div>
      <span className={`status status-${tone}`}>{label}</span>
      <div className="topbar-actions">
        <button className="btn" onClick={() => void api.setOverlayVisible(!visible)}>
          {visible ? "Hide overlays" : "Show overlays"}
        </button>
        <button className={`btn btn-primary ${editing ? "is-active" : ""}`} onClick={() => void api.setEditMode(!editing)}>
          {editing ? "Finish editing" : "Edit layout"}
        </button>
      </div>
    </header>
  );
}

const setGeneral = (patch: Partial<GeneralConfig>) =>
  updateConfig((c) => ({ ...c, general: { ...c.general, ...patch } }));

function GeneralPane() {
  const g = useLive((s) => s.config.general);
  return (
    <>
      <h1>General</h1>
      <Section title="Data">
        <Field label="Use the demo race" help="A fake multi-class race, for setting up layouts without iRacing running.">
          <Toggle
            checked={g.demo}
            onChange={(demo) => {
              setGeneral({ demo });
              void api.setDemo(demo);
            }}
          />
        </Field>
        <Field label="Units">
          <Select
            value={g.units}
            onChange={(units) => setGeneral({ units: units as GeneralConfig["units"] })}
            choices={[
              { value: "metric", label: "Metric (km/h, litres)" },
              { value: "imperial", label: "Imperial (mph, gallons)" },
            ]}
          />
        </Field>
        <Field label="Show overlays">
          <Select
            value={g.showWhen}
            onChange={(showWhen) => setGeneral({ showWhen: showWhen as GeneralConfig["showWhen"] })}
            choices={[
              { value: "connected", label: "Whenever iRacing is running" },
              { value: "onTrack", label: "Only when I'm in the car" },
            ]}
          />
        </Field>
      </Section>

      <Section title="Shortcuts">
        <Field label="Edit layout">
          <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>E</kbd>
        </Field>
        <Field label="Show or hide overlays">
          <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>H</kbd>
        </Field>
        <Field label="Next saved layout">
          <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>L</kbd>
        </Field>
        <Field label="Saved layout 1 to 9">
          <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>1</kbd>–<kbd>9</kbd>
        </Field>
        <p className="note">
          Run iRacing in borderless windowed mode. Overlays can't draw on top of exclusive fullscreen.
        </p>
      </Section>

      <WidgetsSection />

      <Section title="Layout">
        <Field label="Reset every widget" help="Puts all widgets back to their default size, position and options.">
          <button className="btn btn-danger" onClick={() => updateConfig((c) => ({ ...defaultConfig(), general: c.general }))}>
            Reset layout
          </button>
        </Field>
      </Section>
      {!isTauri && <p className="note">Browser preview: settings are saved in this browser only.</p>}
    </>
  );
}

function LayoutsPane() {
  const profiles = useLive((s) => s.profiles);
  const active = useLive((s) => s.config.general.activeProfile);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const activeName = profiles.find((p) => p.id === active)?.name;

  const run = async (work: () => Promise<unknown>) => {
    setError(null);
    try {
      await work();
    } catch (e) {
      setError(String(e).replace(/^Error: /, ""));
    }
  };
  const create = () =>
    run(async () => {
      await saveNewProfile(name.trim());
      setName("");
    });

  return (
    <>
      <h1>Layouts</h1>
      <p className="lede">
        Save where every widget sits, its size, opacity and options as a layout, and switch between them, for example
        one for sprints and one for endurance races. Changes you make while a layout is active are saved into it.
      </p>
      <Section title="On screen now">
        <Field
          label={activeName ?? "Unsaved layout"}
          help={activeName ? "Moving, resizing or changing widgets updates this layout." : "Save it to switch back to it later."}
        >
          <input className="text-input" type="text" value={name} maxLength={40} placeholder="Name, e.g. Endurance"
            onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && name.trim() && void create()} />
          <button className="btn btn-primary" disabled={!name.trim()} onClick={() => void create()}>
            {activeName ? "Save as new" : "Save layout"}
          </button>
        </Field>
      </Section>
      <Section title="Saved layouts">
        {profiles.length === 0 && <p className="note">No saved layouts yet.</p>}
        {profiles.map((p, i) => (
          <LayoutRow key={p.id} id={p.id} name={p.name} updatedAt={p.updatedAt} index={i} active={p.id === active} run={run} />
        ))}
        <Field label="Import a layout" help="A layout file exported from Tenths, on this PC or someone else's.">
          <button className="btn" onClick={() => void run(async () => {
            const id = await api.importProfile();
            if (id) await switchProfile(id);
          })}>
            Import…
          </button>
        </Field>
      </Section>
      {error && <p className="error">{error}</p>}
      <p className="note">
        Switch with <kbd>Ctrl</kbd> <kbd>Shift</kbd> <kbd>L</kbd> for the next layout, or <kbd>Ctrl</kbd> <kbd>Shift</kbd> and
        the layout's number. Layouts are saved as files in the Tenths settings folder, under <code>layouts</code>.
      </p>
    </>
  );
}

function LayoutRow({ id, name, updatedAt, index, active, run }: {
  id: string; name: string; updatedAt: number; index: number; active: boolean; run: (w: () => Promise<unknown>) => Promise<void>;
}) {
  const widgets = useLive((s) => s.config.widgets);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const widgetsOf = async () => (active ? widgets : (await api.getProfile(id)).widgets);
  const rename = () =>
    run(async () => {
      const next = renaming?.trim();
      setRenaming(null);
      if (next && next !== name) await api.saveProfile(id, next, await widgetsOf());
    });
  const updated = new Date(updatedAt * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

  return (
    <div className={`field layout-row ${active ? "is-active" : ""}`}>
      <span className="layout-key" title={index < 9 ? `Ctrl+Shift+${index + 1}` : "Use Ctrl+Shift+L to reach it"}>
        {index < 9 ? index + 1 : ""}
      </span>
      <div className="field-text">
        {renaming != null ? (
          <input className="text-input" autoFocus value={renaming} maxLength={40} aria-label="Layout name"
            onChange={(e) => setRenaming(e.target.value)} onBlur={() => void rename()}
            onKeyDown={(e) => (e.key === "Enter" ? void rename() : e.key === "Escape" && setRenaming(null))} />
        ) : (
          <span className="field-label">
            {name}
            {active && <span className="layout-active">On screen</span>}
          </span>
        )}
        <span className="field-help">Saved {updated}</span>
      </div>
      <div className="field-control">
        {!active && <button className="btn btn-primary" onClick={() => void run(() => switchProfile(id))}>Switch to</button>}
        <button className="btn btn-quiet" onClick={() => setRenaming(name)}>Rename</button>
        <button className="btn btn-quiet" onClick={() => void run(async () => api.saveProfile(null, `${name} copy`, await widgetsOf()))}>
          Duplicate
        </button>
        <button className="btn btn-quiet" onClick={() => void run(() => api.exportProfile(id, name))}>Export</button>
        <button className={`btn ${confirmDelete ? "btn-danger" : "btn-quiet"}`} onBlur={() => setConfirmDelete(false)}
          onClick={() => (confirmDelete ? void run(() => deleteProfile(id)) : setConfirmDelete(true))}>
          {confirmDelete ? "Really delete?" : "Delete"}
        </button>
      </div>
    </div>
  );
}

function AppearancePane() {
  const g = useLive((s) => s.config.general);
  const own = g.colors[g.theme] ?? {};
  const changed = Object.keys(own).length;
  const setColors = (colors: Partial<Record<ColorKey, string>>) => setGeneral({ colors: { ...g.colors, [g.theme]: colors } });
  const setColor = (key: ColorKey, value: string | null) => {
    const next = { ...own };
    if (value && value.toLowerCase() !== THEME_COLORS[g.theme][key]) next[key] = value;
    else delete next[key];
    setColors(next);
  };
  const themeName = THEMES.find((t) => t.value === g.theme)?.label ?? "Dark";

  return (
    <>
      <h1>Appearance</h1>
      <p className="lede">The theme and colors apply to the overlays and this window.</p>
      <Section title="Theme">
        <Field label="Theme" help="Light panels can be easier to read over bright tracks and daytime skies. Box Box dresses the panels up like a TV timing tower.">
          <div className="segmented">
            {THEMES.map((t) => (
              <button key={t.value} className={g.theme === t.value ? "is-on" : ""} onClick={() => setGeneral({ theme: t.value })}>
                {t.label}
              </button>
            ))}
          </div>
        </Field>
      </Section>
      {COLOR_GROUPS.map((group) => (
        <Section key={group.title} title={group.title}>
          {group.colors.map((c) => {
            const value = effectiveColor(g, c.key);
            const custom = Boolean(own[c.key]);
            const follows = c.follows && !custom ? COLOR_GROUPS.flatMap((x) => x.colors).find((x) => x.key === c.follows) : null;
            return (
              <Field key={c.key} label={c.label} help={[c.help, follows && `Matches ${follows.label.toLowerCase()} until you change it`].filter(Boolean).join(". ") || undefined}>
                <button className="btn btn-quiet" disabled={!custom} onClick={() => setColor(c.key, null)}>
                  Default
                </button>
                <label className="swatch" style={{ background: value }}>
                  <input type="color" value={value} aria-label={c.label} onChange={(e) => setColor(c.key, e.target.value)} />
                </label>
                <code className="swatch-hex">{value}</code>
              </Field>
            );
          })}
        </Section>
      ))}
      <Section title="Reset">
        <Field label={`Reset the ${themeName} theme's colors`} help={changed ? `${changed} color${changed === 1 ? "" : "s"} changed. Other themes keep their own.` : "Nothing changed yet."}>
          <button className="btn btn-danger" disabled={!changed} onClick={() => setColors({})}>
            Reset colors
          </button>
        </Field>
      </Section>
    </>
  );
}

const SCALE_PRESETS = [0.75, 1, 1.25, 1.5, 2];

/** Quick show/hide for every panel, and one size for all of them. */
function WidgetsSection() {
  const widgets = useLive((s) => s.config.widgets);
  const scales = new Set(WIDGETS.map((m) => widgets[m.id].scale));
  const common = scales.size === 1 ? [...scales][0] : null;
  const setAll = (patch: Partial<WidgetConfig>) =>
    updateConfig((c) => ({
      ...c,
      widgets: Object.fromEntries(Object.entries(c.widgets).map(([k, w]) => [k, { ...w, ...patch }])) as AppConfig["widgets"],
    }));
  return (
    <Section title="Widgets">
      <Field label="Size of every widget" help="Sets all widgets to the same size. At 1440p, 125% looks about the same as 100% at 1080p.">
        <div className="segmented">
          {SCALE_PRESETS.map((v) => (
            <button key={v} className={common === v ? "is-on" : ""} onClick={() => setAll({ scale: v })}>
              {Math.round(v * 100)}%
            </button>
          ))}
        </div>
      </Field>
      <div className="widget-toggles">
        {WIDGETS.map((m) => (
          <label key={m.id} className="widget-toggle">
            <span>{m.name}</span>
            <Toggle checked={widgets[m.id].enabled} onChange={(enabled) => setWidget(m.id, (x) => ({ ...x, enabled }))} />
          </label>
        ))}
      </div>
    </Section>
  );
}

const ANCHOR_WORDS = {
  l: "left", c: "centre", r: "right", t: "top", m: "middle", b: "bottom",
} as const;

function positionHelp(w: WidgetConfig) {
  if (!w.ax || !w.ay) return "Default spot, placed for your screen size. Drag it in edit mode to move it.";
  const where = w.ax === "c" && w.ay === "m" ? "centre" : `${ANCHOR_WORDS[w.ay]} ${ANCHOR_WORDS[w.ax]}`;
  return `Pinned to the ${where} of the screen, so it stays in place at any resolution. Drag it in edit mode to move it.`;
}

function setWidget(id: WidgetId, mutate: (w: WidgetConfig) => WidgetConfig) {
  updateConfig((c: AppConfig) => ({ ...c, widgets: { ...c.widgets, [id]: mutate(c.widgets[id]) } }));
}

// --- Controls ---------------------------------------------------------------

const MODIFIER_KEYS = ["Control", "Shift", "Alt", "Meta", "AltGraph"];
const GROUP_NOTES: Record<string, string> = {
  "Pit stop": "These work while the Pit stop widget is on and you're in the car.",
};

const setControl = (id: ActionId, patch: Partial<ControlBinding>) =>
  updateConfig((c) => ({
    ...c,
    general: { ...c.general, controls: { ...c.general.controls, [id]: { ...c.general.controls[id], ...patch } } },
  }));

/** Every action, with a key and a controller button each. Click a box, then press what you want. */
function ControlsPane() {
  const controls = useLive((s) => s.config.general.controls);
  const [capture, setCapture] = useState<{ id: ActionId; kind: "key" | "button" } | null>(null);
  const [connected, setConnected] = useState<Controller[] | null>(null);

  useEffect(() => {
    void api.listControllers().then(setConnected);
  }, []);

  // Nothing fires while waiting, so pressing a bound key or button doesn't also run its action.
  useEffect(() => {
    if (!capture) return;
    void api.suspendControls(true);
    const key = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") return setCapture(null);
      if (capture.kind !== "key" || MODIFIER_KEYS.includes(e.key)) return;
      setControl(capture.id, { key: { code: e.code, ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey, meta: e.metaKey } });
      setCapture(null);
    };
    window.addEventListener("keydown", key, true);
    const off =
      capture.kind === "button"
        ? on<ButtonPress>("controller-button", (p) => {
            setControl(capture.id, { button: { device: p.device, name: p.name, button: p.button } });
            setCapture(null);
          })
        : null;
    return () => {
      window.removeEventListener("keydown", key, true);
      void off?.then((f) => f());
      void api.suspendControls(false);
    };
  }, [capture]);

  const groups = [...new Set(ACTIONS.map((a) => a.group))];
  const isOn = (id: ActionId, kind: "key" | "button") => capture?.id === id && capture.kind === kind;

  return (
    <>
      <Section title="Controllers">
        <Field
          label="Wheels and button boxes"
          help={
            !isTauri
              ? "Controller buttons need the desktop app. The browser preview only learns keys."
              : "Buttons work while iRacing has the focus. Plug a controller in and come back here to see it."
          }
        >
          <span className="controllers">
            {connected == null ? "Looking…" : connected.length ? connected.map((c) => c.name).join(", ") : "None found"}
          </span>
        </Field>
      </Section>
      {groups.map((group) => (
        <Section key={group} title={group}>
          {GROUP_NOTES[group] && <p className="note section-note">{GROUP_NOTES[group]}</p>}
          {ACTIONS.filter((a) => a.group === group).map((a) => {
            const b = controls[a.id];
            return (
              <Field key={a.id} label={a.label} help={bindingWarning(a.id, b, controls, connected)}>
                <BindBox
                  label={b.key ? keyLabel(b.key) : null}
                  waiting={isOn(a.id, "key") ? "Press a key…" : null}
                  empty="No key"
                  onClick={() => setCapture(isOn(a.id, "key") ? null : { id: a.id, kind: "key" })}
                  onClear={() => setControl(a.id, { key: null })}
                />
                <BindBox
                  label={b.button ? buttonLabel(b.button) : null}
                  title={b.button?.name}
                  waiting={isOn(a.id, "button") ? "Press a button…" : null}
                  empty="No button"
                  onClick={() => setCapture(isOn(a.id, "button") ? null : { id: a.id, kind: "button" })}
                  onClear={() => setControl(a.id, { button: null })}
                />
              </Field>
            );
          })}
        </Section>
      ))}
      <Section title="Reset">
        <Field label="Restore the default keys" help="Ctrl+Shift with a letter for each action. Clears every controller button.">
          <button className="btn" onClick={() => updateConfig((c) => ({ ...c, general: { ...c.general, controls: defaultControls() } }))}>
            Restore defaults
          </button>
        </Field>
      </Section>
    </>
  );
}

function BindBox({ label, title, waiting, empty, onClick, onClear }: {
  label: string | null; title?: string; waiting: string | null; empty: string; onClick: () => void; onClear: () => void;
}) {
  return (
    <span className={`bind ${waiting ? "is-waiting" : ""} ${label ? "" : "is-empty"}`}>
      <button className="bind-main" onClick={onClick} title={waiting ? "Esc cancels" : title}>
        {waiting ?? label ?? empty}
      </button>
      {label && !waiting && (
        <button className="bind-clear" onClick={onClear} aria-label={`Clear ${label}`}>
          ×
        </button>
      )}
    </span>
  );
}

/** Clashes and other things worth knowing about a binding. */
function bindingWarning(
  id: ActionId, b: ControlBinding, all: Record<ActionId, ControlBinding>, connected: Controller[] | null,
): string | undefined {
  const others = ACTIONS.filter((a) => a.id !== id);
  const keyClash = others.find((a) => sameKey(all[a.id].key, b.key));
  const buttonClash = others.find((a) => sameButton(all[a.id].button, b.button));
  const unplugged = b.button && connected && !connected.some((c) => c.device === b.button!.device);
  const notes = [
    keyClash && `Same key as ${keyClash.label.toLowerCase()}`,
    buttonClash && `Same button as ${buttonClash.label.toLowerCase()}`,
    b.key && bareKey(b.key) && "Without Ctrl, Alt or Shift, this key stops working in other programs while Tenths runs",
    b.button && `On ${b.button.name}${unplugged ? ", not plugged in" : ""}`,
  ].filter(Boolean);
  return notes.length ? notes.join(". ") : undefined;
}

/** A key with no modifier, other than F13 to F24 (which keyboards don't have, so button boxes use them). */
function bareKey(k: KeyBinding): boolean {
  return !k.ctrl && !k.alt && !k.shift && !k.meta && !/^F(1[3-9]|2[0-4])$/.test(k.code);
}

function WidgetPane({ id }: { id: WidgetId }) {
  const meta = widgetMeta(id);
  const w = useLive((s) => s.config.widgets[id]);

  return (
    <>
      <h1>{meta.name}</h1>
      <p className="lede">{meta.description}</p>
      <Section title="Display">
        <Field label="Show this widget">
          <Toggle checked={w.enabled} onChange={(enabled) => setWidget(id, (x) => ({ ...x, enabled }))} />
        </Field>
        <Field label="Size">
          <Slider value={w.scale} min={0.5} max={3} step={0.05} format={(v) => `${Math.round(v * 100)}%`}
            onChange={(scale) => setWidget(id, (x) => ({ ...x, scale }))} />
        </Field>
        <Field label="Opacity">
          <Slider value={w.opacity} min={0.2} max={1} step={0.05} format={(v) => `${Math.round(v * 100)}%`}
            onChange={(opacity) => setWidget(id, (x) => ({ ...x, opacity }))} />
        </Field>
        <Field label="Position" help={positionHelp(w)}>
          <button className="btn" onClick={() => setWidget(id, (x) => ({ ...x, x: meta.x, y: meta.y, ax: undefined, ay: undefined, scale: 1 }))}>
            Reset position
          </button>
        </Field>
      </Section>
      {meta.options.length > 0 && (
        <Section title="Options">
          {meta.options.map((o) => (
            <OptionField key={o.key} spec={o} value={w.options[o.key]}
              onChange={(v) => setWidget(id, (x) => ({ ...x, options: { ...x.options, [o.key]: v } }))} />
          ))}
          <Field label="Restore defaults">
            <button className="btn" onClick={() => setWidget(id, (x) => ({ ...x, options: defaultWidgetConfig(meta).options }))}>
              Reset options
            </button>
          </Field>
        </Section>
      )}
      {id === "delta" && <ReferenceSection />}
    </>
  );
}

/** Import a lap from an iRacing .ibt telemetry file to compare against. */
function ReferenceSection() {
  const reference = useLive((s) => s.reference);
  const [file, setFile] = useState<{ path: string; summary: IbtSummary } | null>(null);
  const [chosen, setChosen] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (work: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await work();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const pick = () =>
    run(async () => {
      const path = await api.pickLapFile();
      if (!path) return;
      const summary = await api.inspectLapFile(path);
      if (!summary.laps.length) throw new Error("That file has no complete laps. Record at least one full lap from line to line.");
      const fastest = [...summary.laps].sort((a, b) => Number(b.clean) - Number(a.clean) || a.lapTime - b.lapTime)[0];
      setFile({ path, summary });
      setChosen(fastest.lap);
    });

  const use = () =>
    run(async () => {
      if (!file || chosen == null) return;
      await api.loadReference(file.path, chosen);
      setFile(null);
    });

  const fastestTime = file ? Math.min(...file.summary.laps.map((l) => l.lapTime)) : 0;

  return (
    <Section title="Reference lap and coaching">
      <Field
        label={reference?.loaded ? "Comparing against" : "No reference lap"}
        help={
          reference?.loaded
            ? [
                `${lapTime(reference.lapTime)} by ${reference.driver || "unknown driver"} in the ${reference.carName || "car"}`,
                `${reference.trackName}${reference.trackConfig ? `, ${reference.trackConfig}` : ""}`,
                reference.matchesTrack ? null : "Not this track, so it's paused until you drive there.",
                reference.matchesCar ? null : "It's a different car from yours, so coaching will be less accurate.",
                reference.hasSteering ? null : "Steering isn't saved in .blap/.olap files, so there's no reference wheel or steering coaching. Use an .ibt lap for those.",
              ]
                .filter(Boolean)
                .join(". ")
            : "Import iRacing's saved best or optimal lap (.blap or .olap, in Documents\\iRacing\\lapfiles) or a lap from a telemetry file (.ibt, recorded with Alt+L). It powers the Reference lap comparison, the ghost inputs and the Coach widget."
        }
      >
        {reference?.loaded && (
          <button className="btn" disabled={busy} onClick={() => run(() => api.clearReference())}>
            Remove
          </button>
        )}
        <button className="btn btn-primary" disabled={busy} onClick={pick}>
          {reference?.loaded ? "Import another lap" : "Import a lap"}
        </button>
      </Field>

      {file && (
        <div className="lap-picker">
          <p className="note">
            {file.summary.trackName}
            {file.summary.trackConfig ? `, ${file.summary.trackConfig}` : ""}. {file.summary.carName}, driven by{" "}
            {file.summary.driver || "unknown"}. {file.summary.kind === "blap" ? "" : "Pick a lap:"}
          </p>
          <div className="lap-list" role="radiogroup" aria-label="Laps in this file">
            {file.summary.laps.map((l) => (
              <button
                key={l.lap}
                role="radio"
                aria-checked={chosen === l.lap}
                className={`lap-option ${chosen === l.lap ? "is-chosen" : ""}`}
                onClick={() => setChosen(l.lap)}
              >
                <span>{file.summary.kind === "blap" ? "Saved lap" : `Lap ${l.lap}`}</span>
                <b className={l.lapTime === fastestTime ? "is-fastest" : ""}>{lapTime(l.lapTime)}</b>
                <span className="muted">{l.clean ? "Clean" : "Pit or off track"}</span>
              </button>
            ))}
          </div>
          <div className="lap-actions">
            <button className="btn" onClick={() => setFile(null)}>
              Cancel
            </button>
            <button className="btn btn-primary" disabled={busy || chosen == null} onClick={use}>
              {file.summary.kind === "blap" ? "Use this lap" : `Use lap ${chosen}`}
            </button>
          </div>
        </div>
      )}
      {error && <p className="error">{error}</p>}
      {!isTauri && <p className="note">Browser preview: importing uses a sample file.</p>}
    </Section>
  );
}

function OptionField({ spec, value, onChange }: { spec: OptionSpec; value: unknown; onChange: (v: number | boolean | string) => void }) {
  switch (spec.type) {
    case "toggle":
      return (
        <Field label={spec.label} help={spec.help}>
          <Toggle checked={Boolean(value)} onChange={onChange} />
        </Field>
      );
    case "number":
      return (
        <Field label={spec.label} help={spec.help}>
          <Slider value={Number(value)} min={spec.min} max={spec.max} step={spec.step ?? 1}
            format={(v) => `${spec.step && spec.step < 1 ? v.toFixed(2).replace(/0$/, "") : v}${spec.suffix ? ` ${spec.suffix}` : ""}`}
            onChange={onChange} />
        </Field>
      );
    case "select":
      return (
        <Field label={spec.label} help={spec.choices.find((c) => c.value === value)?.help ?? spec.help}>
          <Select value={String(value)} choices={spec.choices} onChange={onChange} />
        </Field>
      );
    case "text":
      return (
        <Field label={spec.label} help={spec.help}>
          <input className="text-input" type="text" value={String(value ?? "")} maxLength={spec.maxLength}
            placeholder={spec.default} onChange={(e) => onChange(e.target.value)} />
        </Field>
      );
  }
}

// --- Controls ---------------------------------------------------------------

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="section">
      <h2>{title}</h2>
      <div className="fields">{children}</div>
    </section>
  );
}

function Field({ label, help, children }: { label: string; help?: string; children: ReactNode }) {
  return (
    <div className="field">
      <div className="field-text">
        <span className="field-label">{label}</span>
        {help && <span className="field-help">{help}</span>}
      </div>
      <div className="field-control">{children}</div>
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button role="switch" aria-checked={checked} className={`toggle ${checked ? "is-on" : ""}`} onClick={() => onChange(!checked)}>
      <span />
    </button>
  );
}

function Slider({ value, min, max, step, format, onChange }: {
  value: number; min: number; max: number; step: number; format: (v: number) => string; onChange: (v: number) => void;
}) {
  return (
    <div className="slider">
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} />
      <output>{format(value)}</output>
    </div>
  );
}

function Select({ value, choices, onChange }: { value: string; choices: { value: string; label: string }[]; onChange: (v: string) => void }) {
  return (
    <select className="select" value={value} onChange={(e) => onChange(e.target.value)}>
      {choices.map((c) => (
        <option key={c.value} value={c.value}>
          {c.label}
        </option>
      ))}
    </select>
  );
}
