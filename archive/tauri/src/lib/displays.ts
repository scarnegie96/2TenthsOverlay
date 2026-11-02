// Which display each widget is on. The app opens one overlay window per
// display (src-tauri/src/displays.rs); each window shows the widgets placed on
// its display. A widget's position is anchored within its display, as before.

import type { WidgetConfig } from "./config";
import type { Display } from "./types";

/** "Display 2 · 2560×1440", or "Display 1 (main) · …" for the primary. */
export function displayName(d: Display): string {
  return `Display ${d.number}${d.primary ? " (main)" : ""} · ${d.width}×${d.height}`;
}

export function primaryKey(displays: Display[]): string | null {
  return displays.find((d) => d.primary)?.key ?? displays[0]?.key ?? null;
}

/**
 * The display a widget shows on: the one it was placed on, or the primary
 * display when that one isn't connected (or the widget was never placed).
 */
export function widgetDisplay(w: Pick<WidgetConfig, "display">, displays: Display[]): string | null {
  if (w.display && displays.some((d) => d.key === w.display)) return w.display;
  return primaryKey(displays);
}

/** The display after this one, wrapping round: for the edit label's display button. */
export function nextDisplay(key: string | null, displays: Display[]): Display | null {
  if (displays.length < 2) return null;
  const i = displays.findIndex((d) => d.key === key);
  return displays[(i + 1) % displays.length];
}

/**
 * Which display a point on the desktop (physical pixels) is on, if any.
 */
export function displayAt(px: number, py: number, displays: Display[]): Display | null {
  return displays.find((d) => px >= d.x && px < d.x + d.width && py >= d.y && py < d.y + d.height) ?? null;
}

/**
 * Where a widget dropped on another display goes, in that display's CSS
 * pixels. `from` is the display being dragged on; `clientX/Y` is the pointer
 * in its window (outside it, past the edge); `grab` is where the widget was
 * held, from its top-left corner, in CSS px. The widget keeps its CSS size,
 * so the same offset applies on the other side.
 */
export function dropOnto(
  from: Display,
  clientX: number,
  clientY: number,
  grab: { x: number; y: number },
  displays: Display[],
): { display: Display; left: number; top: number } | null {
  const px = from.x + clientX * from.scale;
  const py = from.y + clientY * from.scale;
  const to = displayAt(px, py, displays);
  if (!to || to.key === from.key) return null;
  return {
    display: to,
    left: (px - to.x) / to.scale - grab.x,
    top: (py - to.y) / to.scale - grab.y,
  };
}

/** A display's size in CSS px (what its overlay window's innerWidth/innerHeight would be). */
export const cssSize = (d: Display) => ({ w: d.width / d.scale, h: d.height / d.scale });
