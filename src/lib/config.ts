import type { Units } from "./format";

export type WidgetId = "relative" | "standings" | "inputs" | "delta" | "fuel" | "spotter";

export interface WidgetConfig {
  enabled: boolean;
  /** CSS px from the top-left of the overlay */
  x: number;
  y: number;
  scale: number;
  opacity: number;
  options: Record<string, number | boolean | string>;
}

export interface GeneralConfig {
  /** Use the built-in fake race instead of iRacing. */
  demo: boolean;
  units: Units;
  /** When widgets are drawn outside of edit mode. */
  showWhen: "connected" | "onTrack";
}

export interface AppConfig {
  version: 1;
  general: GeneralConfig;
  widgets: Record<WidgetId, WidgetConfig>;
}

// --- Option schema, used by the settings window to render controls ---------

export type OptionSpec =
  | { key: string; label: string; type: "toggle"; default: boolean; help?: string }
  | { key: string; label: string; type: "number"; default: number; min: number; max: number; step?: number; suffix?: string }
  | { key: string; label: string; type: "select"; default: string; choices: { value: string; label: string }[] };

export interface WidgetMeta {
  id: WidgetId;
  name: string;
  description: string;
  /** Default placement on a 1920×1080 screen. */
  x: number;
  y: number;
  enabled: boolean;
  options: OptionSpec[];
}

export const WIDGETS: WidgetMeta[] = [
  {
    id: "relative",
    name: "Relative",
    description: "Cars around you on track, with the time gap to each.",
    x: 1480, y: 600, enabled: true,
    options: [
      { key: "ahead", label: "Cars ahead", type: "number", default: 3, min: 1, max: 8 },
      { key: "behind", label: "Cars behind", type: "number", default: 3, min: 1, max: 8 },
      { key: "showIRating", label: "Show iRating", type: "toggle", default: true },
      { key: "showLicense", label: "Show licence", type: "toggle", default: true },
      { key: "showPit", label: "Show cars in the pits", type: "toggle", default: true },
    ],
  },
  {
    id: "standings",
    name: "Standings",
    description: "Race order with gaps and lap times. Always keeps you in view.",
    x: 24, y: 200, enabled: true,
    options: [
      { key: "rows", label: "Rows", type: "number", default: 14, min: 5, max: 40 },
      { key: "myClassOnly", label: "My class only", type: "toggle", default: true, help: "In multi-class races" },
      {
        key: "gapMode", label: "Gap column", type: "select", default: "leader",
        choices: [
          { value: "leader", label: "Gap to leader" },
          { value: "interval", label: "Interval to car ahead" },
        ],
      },
      { key: "showLast", label: "Show last lap", type: "toggle", default: true },
      { key: "showIRating", label: "Show iRating", type: "toggle", default: false },
    ],
  },
  {
    id: "inputs",
    name: "Inputs",
    description: "Throttle and brake trace, pedal bars, gear and speed.",
    x: 780, y: 930, enabled: true,
    options: [
      { key: "traceSeconds", label: "Trace length", type: "number", default: 6, min: 2, max: 20, suffix: "s" },
      { key: "showClutch", label: "Show clutch", type: "toggle", default: false },
      { key: "showSteering", label: "Show steering", type: "toggle", default: true },
    ],
  },
  {
    id: "delta",
    name: "Delta",
    description: "Live time against your best lap.",
    x: 780, y: 120, enabled: true,
    options: [
      { key: "range", label: "Bar range", type: "number", default: 1, min: 0.2, max: 5, step: 0.1, suffix: "s" },
      { key: "showLaps", label: "Show last and best lap", type: "toggle", default: true },
    ],
  },
  {
    id: "fuel",
    name: "Fuel",
    description: "Usage per lap, laps remaining and how much to add at your stop.",
    x: 1560, y: 340, enabled: true,
    options: [
      { key: "marginLaps", label: "Safety margin", type: "number", default: 0.5, min: 0, max: 3, step: 0.25, suffix: "laps" },
    ],
  },
  {
    id: "spotter",
    name: "Spotter",
    description: "Lights up when a car is alongside you.",
    x: 850, y: 760, enabled: true,
    options: [
      { key: "alwaysShow", label: "Always visible", type: "toggle", default: false, help: "Otherwise it only appears when a car is alongside" },
    ],
  },
];

export const widgetMeta = (id: WidgetId) => WIDGETS.find((w) => w.id === id)!;

function defaultOptions(meta: WidgetMeta): WidgetConfig["options"] {
  return Object.fromEntries(meta.options.map((o) => [o.key, o.default]));
}

export function defaultWidgetConfig(meta: WidgetMeta): WidgetConfig {
  return { enabled: meta.enabled, x: meta.x, y: meta.y, scale: 1, opacity: 1, options: defaultOptions(meta) };
}

export function defaultConfig(): AppConfig {
  return {
    version: 1,
    general: { demo: false, units: "metric", showWhen: "connected" },
    widgets: Object.fromEntries(WIDGETS.map((m) => [m.id, defaultWidgetConfig(m)])) as AppConfig["widgets"],
  };
}

/** Fills in anything missing from a saved config (new widgets, new options). */
export function normalizeConfig(raw: unknown): AppConfig {
  const base = defaultConfig();
  if (!raw || typeof raw !== "object") return base;
  const saved = raw as Partial<AppConfig>;
  const widgets = { ...base.widgets };
  for (const meta of WIDGETS) {
    const w = saved.widgets?.[meta.id];
    if (w) {
      widgets[meta.id] = {
        ...base.widgets[meta.id],
        ...w,
        options: { ...base.widgets[meta.id].options, ...(w.options ?? {}) },
      };
    }
  }
  return { version: 1, general: { ...base.general, ...(saved.general ?? {}) }, widgets };
}

export function opt<T extends number | boolean | string>(w: WidgetConfig, key: string, fallback: T): T {
  const v = w.options[key];
  return (v === undefined ? fallback : v) as T;
}
