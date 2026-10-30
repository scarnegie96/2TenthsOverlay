import { useState, type ReactNode } from "react";
import { api, isTauri } from "../lib/bridge";
import {
  WIDGETS, defaultConfig, defaultWidgetConfig, widgetMeta,
  type AppConfig, type GeneralConfig, type OptionSpec, type WidgetConfig, type WidgetId,
} from "../lib/config";
import { lapTime } from "../lib/format";
import { updateConfig, useLive } from "../lib/store";
import type { IbtSummary } from "../lib/types";

type Pane = "general" | WidgetId;

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
        <div className="nav-heading">Widgets</div>
        {WIDGETS.map((w) => (
          <NavItem key={w.id} active={pane === w.id} onClick={() => setPane(w.id)}>
            {w.name}
            <span className={`dot ${config.widgets[w.id].enabled ? "is-on" : ""}`} aria-label={config.widgets[w.id].enabled ? "shown" : "hidden"} />
          </NavItem>
        ))}
      </nav>
      <main className="pane">{pane === "general" ? <GeneralPane /> : <WidgetPane id={pane} />}</main>
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
        <p className="note">
          Run iRacing in borderless windowed mode. Overlays can't draw on top of exclusive fullscreen.
        </p>
      </Section>

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

function setWidget(id: WidgetId, mutate: (w: WidgetConfig) => WidgetConfig) {
  updateConfig((c: AppConfig) => ({ ...c, widgets: { ...c.widgets, [id]: mutate(c.widgets[id]) } }));
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
        <Field label="Position" help={`${w.x}, ${w.y} from the top left. Drag it in edit mode to move it.`}>
          <button className="btn" onClick={() => setWidget(id, (x) => ({ ...x, x: meta.x, y: meta.y, scale: 1 }))}>
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
      const path = await api.pickIbt();
      if (!path) return;
      const summary = await api.inspectIbt(path);
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
    <Section title="Reference lap">
      <Field
        label={reference?.loaded ? "Comparing against" : "No reference lap"}
        help={
          reference?.loaded
            ? [
                `${lapTime(reference.lapTime)} by ${reference.driver || "unknown driver"} in the ${reference.carName || "car"}`,
                `${reference.trackName}${reference.trackConfig ? `, ${reference.trackConfig}` : ""}`,
                reference.matchesTrack ? null : "Not this track, so it's paused until you drive there.",
              ]
                .filter(Boolean)
                .join(". ")
            : "Import a lap from an iRacing telemetry file (.ibt). In the car, Alt+L starts and stops recording; files are saved in Documents\\iRacing\\telemetry."
        }
      >
        {reference?.loaded && (
          <button className="btn" disabled={busy} onClick={() => run(() => api.clearReference())}>
            Remove
          </button>
        )}
        <button className="btn btn-primary" disabled={busy} onClick={pick}>
          {reference?.loaded ? "Import another lap" : "Import .ibt file"}
        </button>
      </Field>

      {file && (
        <div className="lap-picker">
          <p className="note">
            {file.summary.trackName}
            {file.summary.trackConfig ? `, ${file.summary.trackConfig}` : ""}. {file.summary.carName}, driven by{" "}
            {file.summary.driver || "unknown"}. Pick a lap:
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
                <span>Lap {l.lap}</span>
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
              Use lap {chosen}
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
        <Field label={spec.label}>
          <Slider value={Number(value)} min={spec.min} max={spec.max} step={spec.step ?? 1}
            format={(v) => `${spec.step && spec.step < 1 ? v.toFixed(2).replace(/0$/, "") : v}${spec.suffix ? ` ${spec.suffix}` : ""}`}
            onChange={onChange} />
        </Field>
      );
    case "select":
      return (
        <Field label={spec.label}>
          <Select value={String(value)} choices={spec.choices} onChange={onChange} />
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
