import type { Units } from "./format";
import type { ColorKey, Theme } from "./theme";

export type WidgetId =
  | "relative" | "standings" | "session" | "inputs" | "delta" | "laplog" | "coach" | "dynamics" | "instruments" | "fuel"
  | "radar" | "tires" | "flags" | "spotter";

export type AnchorX = "l" | "c" | "r";
export type AnchorY = "t" | "m" | "b";

export interface WidgetConfig {
  enabled: boolean;
  /**
   * Offset in CSS px from the anchor: from the left/top edge, from the
   * right/bottom edge, or of the widget's centre from the screen centre.
   * Without an anchor, x/y are absolute on a 1920x1080 screen (the defaults
   * and layouts saved by older versions); the overlay anchors them on first move.
   */
  x: number;
  y: number;
  ax?: AnchorX;
  ay?: AnchorY;
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
  theme: Theme;
  /** Colors changed from the theme's defaults, kept separately for each theme. */
  colors: Record<Theme, Partial<Record<ColorKey, string>>>;
  /** The saved layout the widgets belong to; changes to them are saved into it. */
  activeProfile: string | null;
}

export interface AppConfig {
  version: 1;
  general: GeneralConfig;
  widgets: Record<WidgetId, WidgetConfig>;
}

// --- Option schema, used by the settings window to render controls ---------

export type OptionSpec =
  | { key: string; label: string; type: "toggle"; default: boolean; help?: string }
  | { key: string; label: string; type: "number"; default: number; min: number; max: number; step?: number; suffix?: string; help?: string }
  | { key: string; label: string; type: "select"; default: string; choices: { value: string; label: string }[] }
  | { key: string; label: string; type: "text"; default: string; maxLength?: number; help?: string };

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
    x: 1396, y: 600, enabled: true,
    options: [
      { key: "ahead", label: "Cars ahead", type: "number", default: 3, min: 1, max: 8 },
      { key: "behind", label: "Cars behind", type: "number", default: 3, min: 1, max: 8 },
      { key: "showIRating", label: "Show iRating", type: "toggle", default: true },
      { key: "showLicense", label: "Show license", type: "toggle", default: true },
      { key: "showLastLap", label: "Show last lap", type: "toggle", default: true, help: "Purple for the fastest lap in class, green for a personal best" },
      { key: "showPit", label: "Show cars in the pits", type: "toggle", default: true },
      { key: "showSoF", label: "Show strength of field", type: "toggle", default: true, help: "For your class, in the header" },
    ],
  },
  {
    id: "standings",
    name: "Standings",
    description: "Race order by class, with gaps, iRating and lap times. Always keeps you in view.",
    x: 24, y: 160, enabled: true,
    options: [
      { key: "rows", label: "Rows for your class", type: "number", default: 10, min: 3, max: 40 },
      { key: "otherClassRows", label: "Rows for other classes", type: "number", default: 3, min: 1, max: 20 },
      { key: "myClassOnly", label: "My class only", type: "toggle", default: false, help: "Hide the other classes in multi-class races" },
      {
        key: "gapMode", label: "Gap column", type: "select", default: "leader",
        choices: [
          { value: "leader", label: "Gap to class leader" },
          { value: "interval", label: "Interval to car ahead" },
        ],
      },
      { key: "showIRating", label: "Show iRating", type: "toggle", default: true },
      { key: "showLast", label: "Show last lap", type: "toggle", default: true, help: "Purple for the fastest lap in class, green for a personal best" },
      { key: "showBest", label: "Show best lap", type: "toggle", default: true },
      { key: "showSoF", label: "Show strength of field", type: "toggle", default: true, help: "Per class in multi-class races" },
    ],
  },
  {
    id: "session",
    name: "Session",
    description: "Track, field size and strength of field, time of day, weather, and your incidents against the limit.",
    // Off by default: the default layout has no free spot that the coach doesn't use.
    x: 572, y: 250, enabled: false,
    options: [
      { key: "showClasses", label: "List each class", type: "toggle", default: true, help: "Cars and SoF per class in multi-class races" },
      { key: "showConditions", label: "Show conditions", type: "toggle", default: true, help: "Time of day, temperatures, sky, track wetness and wind" },
      { key: "showLimits", label: "Show incidents and fast repairs", type: "toggle", default: true },
    ],
  },
  {
    id: "inputs",
    name: "Inputs",
    description: "Gear, speed, steering wheel, throttle and brake trace, and pedal bars.",
    x: 600, y: 940, enabled: true,
    options: [
      { key: "traceSeconds", label: "Trace length", type: "number", default: 6, min: 2, max: 20, suffix: "s" },
      { key: "showTrace", label: "Show trace", type: "toggle", default: true },
      { key: "showSteering", label: "Show steering wheel", type: "toggle", default: true },
      { key: "showClutch", label: "Show clutch", type: "toggle", default: true },
      { key: "shiftLight", label: "Gear turns red to shift", type: "toggle", default: true, help: "At the RPM where the car's own shift lights blink. Off keeps the gear yellow" },
      { key: "showGhost", label: "Show reference inputs", type: "toggle", default: true, help: "A slimmer strip on top with the imported reference lap's gear, speed, pedals and (for .ibt laps) wheel" },
    ],
  },
  {
    id: "delta",
    name: "Delta",
    description: "Your lap against a comparison lap, round the whole lap and sector by sector. The comparison rises above the line where it's faster and dips below where it's slower.",
    x: 572, y: 24, enabled: true,
    options: [
      {
        key: "compareTo", label: "Compare against", type: "select", default: "best",
        choices: [
          { value: "best", label: "Best lap" },
          { value: "optimal", label: "Optimal lap (best sectors)" },
          { value: "sessionBest", label: "Session best" },
          { value: "sessionOptimal", label: "Session optimal" },
          { value: "last", label: "Last lap" },
          { value: "reference", label: "Reference lap (imported)" },
        ],
      },
      {
        key: "sectors", label: "Sectors", type: "select", default: "track",
        choices: [
          { value: "track", label: "Track sectors from iRacing" },
          { value: "mini", label: "Equal mini sectors" },
        ],
      },
      { key: "miniSectors", label: "Number of mini sectors", type: "number", default: 12, min: 4, max: 30 },
      { key: "showPrevious", label: "Show last lap's line", type: "toggle", default: true, help: "Dashed, for comparing this lap with the one before" },
      { key: "showLapTimes", label: "Show lap times", type: "toggle", default: true, help: "Your best, optimal (best sectors this session) and last laps along the bottom" },
      { key: "simple", label: "Simple pace readout", type: "toggle", default: false, help: "Just the live gap to the comparison lap, without the graph and sectors" },
    ],
  },
  {
    id: "laplog",
    name: "Lap log",
    description: "Every lap this session with its gap to your best and the track temperature. Your personal best for each car and track is saved between sessions.",
    x: 1090, y: 560, enabled: true,
    options: [
      { key: "rows", label: "Laps shown", type: "number", default: 8, min: 3, max: 30 },
      {
        key: "compareTo", label: "Gap to", type: "select", default: "session",
        choices: [
          { value: "session", label: "Session best" },
          { value: "personal", label: "Personal best (all time)" },
        ],
      },
      { key: "showDelta", label: "Show gap column", type: "toggle", default: true },
      { key: "showTemp", label: "Show track temperature", type: "toggle", default: true },
      { key: "showFuel", label: "Show fuel used", type: "toggle", default: false },
      { key: "showPersonalBest", label: "Show personal best", type: "toggle", default: true, help: "In the header, next to the session best" },
    ],
  },
  {
    id: "coach",
    name: "Coach",
    description: "Live coaching against your imported reference lap: brake points, pressure, apex speed, throttle, gear and line.",
    x: 760, y: 270, enabled: true,
    options: [
      { key: "showDetail", label: "Show the reason", type: "toggle", default: true, help: "The line under the cue, like \"Reference brakes 30 m later\"" },
      { key: "showStats", label: "Show delta and speed", type: "toggle", default: true },
    ],
  },
  {
    id: "dynamics",
    name: "Car dynamics",
    description: "G-meter with the grip you've been using, the car's slip, pitch and roll, and an understeer / oversteer estimate.",
    x: 24, y: 716, enabled: true,
    options: [
      { key: "maxG", label: "G-meter range", type: "number", default: 3, min: 2, max: 6, suffix: "g" },
      {
        key: "direction", label: "Dot shows", type: "select", default: "felt",
        choices: [
          { value: "felt", label: "Force you feel (braking pushes it up)" },
          { value: "accel", label: "Car's acceleration (braking pulls it down)" },
        ],
      },
      { key: "showAttitude", label: "Show slip, pitch and roll", type: "toggle", default: true },
      { key: "exaggerate", label: "Exaggerate pitch and roll", type: "number", default: 4, min: 1, max: 10, suffix: "×", help: "Real angles are small; the numbers always show the true value" },
      { key: "showBalance", label: "Show balance", type: "toggle", default: true, help: "Learnt from your driving, so it settles after a few corners" },
    ],
  },
  {
    id: "instruments",
    name: "Instruments",
    description: "Warning lights (oil, water, fuel, stalled, limiters, repairs), engine gauges and a brake bias dial.",
    x: 1396, y: 846, enabled: true,
    options: [
      { key: "showGauges", label: "Show engine gauges", type: "toggle", default: true },
      { key: "showBias", label: "Show brake bias", type: "toggle", default: true, help: "Only for cars with adjustable bias" },
      { key: "testLights", label: "Light every warning while editing", type: "toggle", default: false, help: "Bulb test, to check how they look" },
    ],
  },
  {
    id: "fuel",
    name: "Fuel",
    description: "Usage per lap, laps remaining and how much to add at your stop.",
    x: 1580, y: 340, enabled: true,
    options: [
      { key: "marginLaps", label: "Safety margin", type: "number", default: 0.5, min: 0, max: 3, step: 0.25, suffix: "laps" },
    ],
  },
  {
    id: "radar",
    name: "Radar",
    description: "Top-down view of the cars around you, with a warning when one is alongside.",
    x: 842, y: 596, enabled: true,
    options: [
      { key: "range", label: "Range", type: "number", default: 20, min: 10, max: 50, step: 5, suffix: "m" },
      { key: "alwaysShow", label: "Always visible", type: "toggle", default: true, help: "Turn off to hide it until a car is in range" },
      { key: "classColors", label: "Color cars by class", type: "toggle", default: false, help: "Close cars still turn amber and red" },
    ],
  },
  {
    id: "tires",
    name: "Tires",
    description: "Temperature and tread for each tire. iRacing only updates these while you're in your pit box.",
    x: 1580, y: 120, enabled: true,
    options: [
      { key: "showPressure", label: "Show cold pressure", type: "toggle", default: true },
    ],
  },
  {
    id: "flags",
    name: "Flags",
    description: "A trackside LED flag board with the flag that's out, and a line of text you can change for each flag.",
    x: 1240, y: 270, enabled: true,
    options: [
      { key: "showCaption", label: "Show text under the board", type: "toggle", default: true },
      { key: "greenSeconds", label: "Show green for", type: "number", default: 5, min: 0, max: 30, suffix: "s", help: "After a start or restart; the board then goes dark" },
      { key: "scBoard", label: "Safety car board", type: "text", default: "SC", maxLength: 3, help: "Up to 3 letters or numbers, lit on the board under full-course cautions" },
      { key: "textYellow", label: "Yellow", type: "text", default: "Accident ahead", maxLength: 28 },
      { key: "textCaution", label: "Full-course caution", type: "text", default: "Safety car", maxLength: 28 },
      { key: "textDebris", label: "Debris", type: "text", default: "Debris on track", maxLength: 28 },
      { key: "textBlue", label: "Blue", type: "text", default: "Faster car behind", maxLength: 28 },
      { key: "textWhite", label: "White", type: "text", default: "Last lap", maxLength: 28 },
      { key: "textCheckered", label: "Chequered", type: "text", default: "Finish", maxLength: 28 },
      { key: "textGreen", label: "Green", type: "text", default: "Green flag", maxLength: 28 },
      { key: "textOneToGreen", label: "One lap to green", type: "text", default: "Green next lap", maxLength: 28 },
      { key: "textRed", label: "Red", type: "text", default: "Session stopped", maxLength: 28 },
      { key: "textBlack", label: "Black", type: "text", default: "Penalty: pit now", maxLength: 28 },
      { key: "textRepair", label: "Meatball", type: "text", default: "Damage: pit for repairs", maxLength: 28 },
      { key: "textWarning", label: "Black and white", type: "text", default: "Warning", maxLength: 28 },
      { key: "textDisqualified", label: "Disqualified", type: "text", default: "Disqualified", maxLength: 28 },
      { key: "alwaysShow", label: "Always visible", type: "toggle", default: false, help: "Shows the dark board when there's no flag" },
    ],
  },
  {
    id: "spotter",
    name: "Spotter",
    description: "Simple left and right lights when a car is alongside. The radar includes these too.",
    x: 1130, y: 680, enabled: false,
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
    general: { demo: false, units: "metric", showWhen: "connected", theme: "dark", colors: { dark: {}, light: {} }, activeProfile: null },
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
  const general = { ...base.general, ...(saved.general ?? {}) };
  general.colors = { ...base.general.colors, ...(saved.general?.colors ?? {}) };
  return { version: 1, general, widgets };
}

export function opt<T extends number | boolean | string>(w: WidgetConfig, key: string, fallback: T): T {
  const v = w.options[key];
  return (v === undefined ? fallback : v) as T;
}
