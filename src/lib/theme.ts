import type { GeneralConfig } from "./config";

export type Theme = "dark" | "light";

/** Every color the user can change. Each one is a CSS variable of the same name (see base.css). */
export type ColorKey =
  | "ink" | "ink-2" | "ink-3"
  | "panel-solid" | "panel-raised" | "edge" | "wheel"
  | "fastest" | "gain" | "loss" | "attention" | "lapped" | "lapping"
  | "throttle" | "brake" | "clutch";

export interface ColorSpec {
  key: ColorKey;
  label: string;
  help?: string;
  /** Follows another color until changed itself. */
  follows?: ColorKey;
}

export const COLOR_GROUPS: { title: string; colors: ColorSpec[] }[] = [
  {
    title: "Text",
    colors: [
      { key: "ink", label: "Text" },
      { key: "ink-2", label: "Secondary text" },
      { key: "ink-3", label: "Muted text", help: "Labels, units and scales" },
    ],
  },
  {
    title: "Panels",
    colors: [
      { key: "panel-solid", label: "Panel background", help: "Panels stay slightly see-through; set each widget's opacity in its own settings" },
      { key: "panel-raised", label: "Raised surfaces", help: "Buttons and highlighted rows" },
      { key: "edge", label: "Borders and dividers" },
      { key: "wheel", label: "Steering wheel and car", help: "In the Inputs and Car dynamics widgets" },
    ],
  },
  {
    title: "Timing",
    colors: [
      { key: "fastest", label: "Fastest in class" },
      { key: "gain", label: "Gaining and personal best" },
      { key: "loss", label: "Losing time" },
      { key: "attention", label: "Warnings and highlights", help: "Also the cars alongside on the radar and spotter" },
      { key: "lapped", label: "Cars you're lapping" },
      { key: "lapping", label: "Cars lapping you" },
    ],
  },
  {
    title: "Pedals",
    colors: [
      { key: "throttle", label: "Throttle", follows: "gain" },
      { key: "brake", label: "Brake", follows: "loss" },
      { key: "clutch", label: "Clutch" },
    ],
  },
];

const ALL_COLORS = COLOR_GROUPS.flatMap((g) => g.colors);

/** Must match the defaults in base.css. */
export const THEME_COLORS: Record<Theme, Record<ColorKey, string>> = {
  dark: {
    "ink": "#e9eef3", "ink-2": "#a7b4c2", "ink-3": "#6d7c8d",
    "panel-solid": "#0d131b", "panel-raised": "#151e29", "edge": "#92aac4", "wheel": "#c9d1da",
    "fastest": "#b98cff", "gain": "#4be08f", "loss": "#ff5468", "attention": "#ffb547", "lapped": "#6fb7ff", "lapping": "#ff9a6b",
    "throttle": "#4be08f", "brake": "#ff5468", "clutch": "#6fb7ff",
  },
  light: {
    "ink": "#141c25", "ink-2": "#46525f", "ink-3": "#76828f",
    "panel-solid": "#f4f6f9", "panel-raised": "#ffffff", "edge": "#40556d", "wheel": "#3a4654",
    "fastest": "#8248e0", "gain": "#11a058", "loss": "#df2f47", "attention": "#d48300", "lapped": "#1c78d0", "lapping": "#dc6128",
    "throttle": "#11a058", "brake": "#df2f47", "clutch": "#1c78d0",
  },
};

/** The color in use: the user's own, else the one it follows, else the theme's. */
export function effectiveColor(g: GeneralConfig, key: ColorKey): string {
  const own = g.colors[g.theme]?.[key];
  if (own) return own;
  const follows = ALL_COLORS.find((c) => c.key === key)?.follows;
  return follows ? effectiveColor(g, follows) : THEME_COLORS[g.theme][key];
}

/** Puts the theme and the user's colors on this window's root element. */
export function applyTheme(g: GeneralConfig) {
  const root = document.documentElement;
  root.dataset.theme = g.theme;
  const own = g.colors[g.theme] ?? {};
  for (const { key } of ALL_COLORS) {
    if (own[key]) root.style.setProperty(`--${key}`, own[key]);
    else root.style.removeProperty(`--${key}`);
  }
}

/** Changes whenever the colors do, for redrawing canvases. */
export const paletteKey = (g: GeneralConfig) => `${g.theme}:${JSON.stringify(g.colors[g.theme] ?? {})}`;

// --- Canvas helpers ----------------------------------------------------------

let pixel: CanvasRenderingContext2D | null = null;

/**
 * Resolves any CSS color (variables, color-mix) as seen from `el` to an
 * rgba() string a canvas can use.
 */
export function cssColor(el: Element, value: string): string {
  const host = el.parentElement ?? el;
  const probe = document.createElement("span");
  probe.style.display = "none";
  probe.style.color = value;
  host.appendChild(probe);
  const computed = getComputedStyle(probe).color;
  probe.remove();
  pixel ??= document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  if (!pixel) return computed;
  pixel.clearRect(0, 0, 1, 1);
  pixel.fillStyle = computed;
  pixel.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = pixel.getImageData(0, 0, 1, 1).data;
  return `rgba(${r}, ${g}, ${b}, ${+(a / 255).toFixed(3)})`;
}

/** The same rgba() color at a different opacity. */
export function withAlpha(rgba: string, alpha: number): string {
  const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(rgba);
  return m ? `rgba(${m[1]}, ${m[2]}, ${m[3]}, ${alpha})` : rgba;
}
