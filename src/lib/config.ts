import { defaultControls, type ActionId, type ControlBinding } from "./controls";
import type { Units } from "./format";
import type { ColorKey, Theme } from "./theme";

export type WidgetId =
  | "relative" | "standings" | "session" | "inputs" | "delta" | "laplog" | "coach" | "dynamics" | "instruments" | "fuel"
  | "radar" | "tires" | "pitstop" | "flags" | "weather" | "racepace" | "stints";

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
  /**
   * Which display it's on (a display key, e.g. "\\.\DISPLAY2"). Missing, or
   * a display that isn't connected, means the primary display.
   */
  display?: string;
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
  /** The key and controller button for each action. */
  controls: Record<ActionId, ControlBinding>;
  /** Look for a new version of Tenths after starting and every few hours. */
  checkUpdates: boolean;
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
  | { key: string; label: string; type: "select"; default: string; choices: { value: string; label: string; help?: string }[]; help?: string }
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
      { key: "showInterval", label: "Show gap to the car ahead", type: "toggle", default: true, help: "Each car's gap to the one in front of it on track, amber when under a second" },
      {
        key: "showCompound", label: "Tyre column", type: "select", default: "auto",
        help: "The compound each car is on: W wet, S soft, M medium, H hard. Names come from your car's tyre list, so another class's may differ.",
        choices: [
          { value: "auto", label: "When the car has more than one compound" },
          { value: "always", label: "Always" },
          { value: "never", label: "Never" },
        ],
      },
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
      { key: "showChange", label: "Show places gained or lost", type: "toggle", default: true, help: "In races: each car's class position now against where it started" },
      { key: "showRejoin", label: "Show where you'd rejoin after a stop", type: "toggle", default: true, help: "In races, once a few pit stops have been timed this session: a line in your class where you'd come out if you pitted now" },
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
      {
        key: "wheel", label: "Steering wheel", type: "select", default: "auto",
        choices: [
          { value: "auto", label: "Automatic (from the car)" },
          { value: "round", label: "Round (road car)" },
          { value: "gt", label: "GT (flat top and bottom)" },
          { value: "formula", label: "Formula (yoke with display)" },
          { value: "oval", label: "Oval (big and thin)" },
        ],
      },
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
      { key: "rows", label: "Laps shown", type: "number", default: 8, min: 3, max: 30, help: "Older laps scroll below these. Scroll in edit layout mode; while driving the newest are shown" },
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
    description: "Live coaching against your imported reference lap: brake and lift points, pressure, apex speed, throttle, gear and line, with a verdict on each corner.",
    x: 760, y: 270, enabled: true,
    options: [
      { key: "showDetail", label: "Show the reason", type: "toggle", default: true, help: "The line under the cue, like \"Reference brakes 30 m later\"" },
      { key: "showStats", label: "Show delta and speed", type: "toggle", default: true },
      { key: "showFocus", label: "Show focus corner", type: "toggle", default: true, help: "The corner costing you most time on average, and why" },
    ],
  },
  {
    id: "dynamics",
    name: "Car dynamics",
    description: "G-meter with the grip you've been using, the car's slip, pitch and roll, and an understeer / oversteer estimate.",
    x: 24, y: 716, enabled: true,
    options: [
      { key: "maxG", label: "G-meter range", type: "number", default: 3, min: 2, max: 6, suffix: "g" },
      { key: "envelopeHold", label: "Grip outline holds for", type: "number", default: 10, min: 1, max: 60, suffix: "s", help: "How long each point of the outline stays where you pushed it" },
      { key: "envelopeReturn", label: "Then shrinks back over", type: "number", default: 10, min: 0, max: 60, suffix: "s", help: "How long a point takes to return to the centre. 0 removes it at once" },
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
    id: "pitstop",
    name: "Pit stop",
    description: "What iRacing will do at your next stop, set while you drive with hotkeys or wheel buttons (choose them in Controls). By default Ctrl+Shift with F for fuel, ↑ and ↓ for more or less, T tires, R fast repair, W tear-off and X to clear everything.",
    // Off by default: it sends commands to iRacing, so it's something to choose.
    x: 572, y: 250, enabled: false,
    options: [
      {
        key: "fuelMode", label: "Fuel hotkey adds", type: "select", default: "finish",
        choices: [
          { value: "finish", label: "Enough to finish", help: "What you need for the rest of the race plus the Fuel widget's safety margin, rounded up to a whole litre" },
          { value: "full", label: "A full tank", help: "Whatever it takes to fill the tank" },
          { value: "manual", label: "iRacing's amount", help: "Ticks fuel and leaves the amount as it's set in iRacing's pit menu" },
        ],
      },
      { key: "fuelStep", label: "Fuel up and down step", type: "number", default: 5, min: 1, max: 20, suffix: "L", help: "How much the more fuel and less fuel controls add or take off, in litres" },
      { key: "autoFuel", label: "Set fuel as you enter the pits", type: "toggle", default: false, help: "Ticks fuel with the planned amount when you cross pit entry, unless it's already set or you've changed it with more or less fuel" },
      { key: "showKeys", label: "Show the hotkeys", type: "toggle", default: true, help: "The key or wheel button for each row" },
    ],
  },
  {
    id: "racepace",
    name: "Race pace",
    description: "Everyone in your class against the class leader through the race: the leader is the flat line at the top, each car a line at its gap below.",
    // Off by default: turn it on in settings and place it where suits your screen.
    x: 572, y: 700, enabled: false,
    options: [
      {
        key: "window", label: "Time shown", type: "select", default: "race",
        choices: [
          { value: "race", label: "The whole race so far" },
          { value: "5", label: "The leader's last 5 laps" },
          { value: "10", label: "The leader's last 10 laps" },
          { value: "20", label: "The leader's last 20 laps" },
        ],
      },
      {
        key: "show", label: "Cars", type: "select", default: "all",
        choices: [
          { value: "all", label: "Everyone in your class" },
          { value: "around", label: "The leader and the cars around you" },
        ],
      },
      { key: "around", label: "Places either side of you", type: "number", default: 3, min: 1, max: 10, help: "When showing the cars around you" },
      { key: "maxGap", label: "Gap scale", type: "number", default: 0, min: 0, max: 600, step: 5, suffix: "s", help: "0 fits everyone shown. A fixed scale keeps far-off cars from squashing the rest; they sit along the bottom" },
      { key: "showTrend", label: "Show gap change per lap", type: "toggle", default: true, help: "Next to each name: how much the gap to the leader changed over the leader's last lap" },
    ],
  },
  {
    id: "stints",
    name: "Stints",
    description: "Every driver's stints: laps since their last stop (live), their tyre, stops so far, and a timeline of the session coloured by compound.",
    // Off by default: turn it on in settings and place it where suits your screen.
    x: 572, y: 330, enabled: false,
    options: [
      {
        key: "classes", label: "Classes", type: "select", default: "all",
        choices: [
          { value: "all", label: "All classes" },
          { value: "mine", label: "Your class only" },
        ],
      },
      { key: "rows", label: "Rows for your class", type: "number", default: 10, min: 3, max: 40 },
      { key: "otherClassRows", label: "Rows for other classes", type: "number", default: 5, min: 1, max: 40 },
      { key: "showTimeline", label: "Show the timeline", type: "toggle", default: true, help: "Each stint as a bar across the session, coloured by tyre, with a tick at every stop" },
      { key: "showLengths", label: "Show stint lengths on the timeline", type: "toggle", default: true, help: "Laps, written on each bar wide enough to fit them" },
      {
        key: "showCompound", label: "Tyre column", type: "select", default: "auto",
        choices: [
          { value: "auto", label: "When the car has more than one compound" },
          { value: "always", label: "Always" },
          { value: "never", label: "Never" },
        ],
      },
    ],
  },
  {
    id: "weather",
    name: "Weather",
    description: "Track and air temperature, track wetness, rain, humidity and a wind compass that turns with your car, with trend arrows.",
    // Off by default: turn it on in settings and place it where suits your screen.
    x: 1396, y: 24, enabled: false,
    options: [
      { key: "showCompass", label: "Show wind compass", type: "toggle", default: true },
      {
        key: "compass", label: "Compass", type: "select", default: "car",
        choices: [
          { value: "car", label: "Turns with the car (car points up)" },
          { value: "north", label: "North up (the car turns)" },
        ],
      },
      { key: "showTrends", label: "Show trend arrows", type: "toggle", default: true, help: "Which way temperatures, wetness and rain have moved over the last few minutes" },
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
    general: { demo: false, units: "metric", showWhen: "connected", theme: "dark", colors: { dark: {}, light: {}, f1: {} }, activeProfile: null, controls: defaultControls(), checkUpdates: true },
    widgets: Object.fromEntries(WIDGETS.map((m) => [m.id, defaultWidgetConfig(m)])) as AppConfig["widgets"],
  };
}

// --- Checking saved and imported configs -----------------------------------

/** Same limits as the sliders in the settings window. */
export const SCALE_MIN = 0.5;
export const SCALE_MAX = 3;
export const OPACITY_MIN = 0.2;
export const OPACITY_MAX = 1;

const ANCHORS_X: readonly AnchorX[] = ["l", "c", "r"];
const ANCHORS_Y: readonly AnchorY[] = ["t", "m", "b"];
const UNITS: readonly Units[] = ["metric", "imperial"];
const SHOW_WHEN: readonly GeneralConfig["showWhen"][] = ["connected", "onTrack"];
const THEME_IDS: readonly Theme[] = ["dark", "light", "f1"];

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);
const oneOf = <T extends string>(v: unknown, allowed: readonly T[]): v is T => allowed.includes(v as T);
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** A saved widget over its defaults. Anything of the wrong type falls back to the default. */
function normalizeWidget(raw: unknown, base: WidgetConfig): WidgetConfig {
  if (!isObject(raw)) return base;
  const w: WidgetConfig = {
    enabled: typeof raw.enabled === "boolean" ? raw.enabled : base.enabled,
    x: isNum(raw.x) ? raw.x : base.x,
    y: isNum(raw.y) ? raw.y : base.y,
    scale: isNum(raw.scale) ? clamp(raw.scale, SCALE_MIN, SCALE_MAX) : base.scale,
    opacity: isNum(raw.opacity) ? clamp(raw.opacity, OPACITY_MIN, OPACITY_MAX) : base.opacity,
    options: { ...base.options },
  };
  // An anchor only means something with both halves; otherwise x/y are absolute.
  if (oneOf(raw.ax, ANCHORS_X) && oneOf(raw.ay, ANCHORS_Y)) {
    w.ax = raw.ax;
    w.ay = raw.ay;
  } else if (raw.ax !== undefined || raw.ay !== undefined) {
    // A broken anchor makes the offsets meaningless too.
    w.x = base.x;
    w.y = base.y;
  }
  if (typeof raw.display === "string") w.display = raw.display;
  if (isObject(raw.options)) {
    for (const [k, v] of Object.entries(raw.options)) {
      if (typeof v === "boolean" || typeof v === "string" || isNum(v)) w.options[k] = v;
    }
  }
  return w;
}

/** Fills in anything missing from a saved config (new widgets, new options) and drops anything of the wrong type. */
export function normalizeConfig(raw: unknown): AppConfig {
  const base = defaultConfig();
  if (!isObject(raw)) return base;
  const savedWidgets = isObject(raw.widgets) ? raw.widgets : {};
  const widgets = { ...base.widgets };
  for (const meta of WIDGETS) {
    if (savedWidgets[meta.id] !== undefined) widgets[meta.id] = normalizeWidget(savedWidgets[meta.id], base.widgets[meta.id]);
  }

  const g = isObject(raw.general) ? raw.general : {};
  const general: GeneralConfig = {
    demo: typeof g.demo === "boolean" ? g.demo : base.general.demo,
    units: oneOf(g.units, UNITS) ? g.units : base.general.units,
    showWhen: oneOf(g.showWhen, SHOW_WHEN) ? g.showWhen : base.general.showWhen,
    theme: oneOf(g.theme, THEME_IDS) ? g.theme : base.general.theme,
    colors: { ...base.general.colors },
    activeProfile: typeof g.activeProfile === "string" ? g.activeProfile : base.general.activeProfile,
    controls: { ...base.general.controls },
    checkUpdates: typeof g.checkUpdates === "boolean" ? g.checkUpdates : base.general.checkUpdates,
  };
  if (isObject(g.colors)) {
    for (const t of THEME_IDS) {
      const saved = g.colors[t];
      if (!isObject(saved)) continue;
      // Only strings; the keys themselves are left alone so unknown ones are just ignored.
      general.colors[t] = Object.fromEntries(Object.entries(saved).filter(([, v]) => typeof v === "string"));
    }
  }
  if (isObject(g.controls)) {
    for (const id of Object.keys(base.general.controls) as ActionId[]) {
      const c = g.controls[id];
      if (isObject(c) && (c.key === null || isObject(c.key)) && (c.button === null || isObject(c.button))) {
        general.controls[id] = c as unknown as ControlBinding;
      }
    }
  }
  return { version: 1, general, widgets };
}

export function opt<T extends number | boolean | string>(w: WidgetConfig, key: string, fallback: T): T {
  const v = w.options[key];
  return (v === undefined ? fallback : v) as T;
}
