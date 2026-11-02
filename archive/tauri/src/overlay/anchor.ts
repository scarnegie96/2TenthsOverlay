import type { AnchorX, AnchorY, WidgetConfig } from "../lib/config";

export interface Size {
  w: number;
  h: number;
}

/** The screen the default layout (and any layout saved before anchoring) was designed on. */
export const REFERENCE_SCREEN: Size = { w: 1920, h: 1080 };

/** Keep at least this much of a widget on screen. */
const MARGIN = 0;

/**
 * Which edge or centre a widget belongs to, from where its middle sits: left,
 * centre or right third of the screen, and top, middle or bottom third.
 */
export function anchorFor(left: number, top: number, size: Size, screen: Size): { ax: AnchorX; ay: AnchorY } {
  const cx = left + size.w / 2;
  const cy = top + size.h / 2;
  return {
    ax: cx < screen.w / 3 ? "l" : cx > (screen.w * 2) / 3 ? "r" : "c",
    ay: cy < screen.h / 3 ? "t" : cy > (screen.h * 2) / 3 ? "b" : "m",
  };
}

/**
 * Offsets from the anchor: distance from the left/top edge, from the
 * right/bottom edge, or of the widget's centre from the screen's centre.
 */
export function toAnchored(left: number, top: number, size: Size, screen: Size) {
  const { ax, ay } = anchorFor(left, top, size, screen);
  const x = ax === "l" ? left : ax === "r" ? screen.w - (left + size.w) : left + size.w / 2 - screen.w / 2;
  const y = ay === "t" ? top : ay === "b" ? screen.h - (top + size.h) : top + size.h / 2 - screen.h / 2;
  return { ax, ay, x: Math.round(x), y: Math.round(y) };
}

/**
 * Where a widget goes on this screen. Widgets keep their distance from the
 * edge (or centre) they're anchored to, so a layout made at 1080p still sits
 * in the same places at 1440p or 4K. Configs without an anchor are treated as
 * absolute positions on a 1920x1080 screen and anchored from there.
 */
export function resolvePosition(
  c: Pick<WidgetConfig, "x" | "y" | "ax" | "ay">,
  size: Size,
  screen: Size,
  /** Size at 100%. Unanchored positions are for this size, so scaling grows away from the pinned edge. */
  unscaled: Size = size,
) {
  let { x, y, ax, ay } = c;
  if (!ax || !ay) {
    const a = toAnchored(c.x, c.y, unscaled, REFERENCE_SCREEN);
    ({ x, y, ax, ay } = a);
  }
  const left = ax === "l" ? x : ax === "r" ? screen.w - size.w - x : screen.w / 2 + x - size.w / 2;
  const top = ay === "t" ? y : ay === "b" ? screen.h - size.h - y : screen.h / 2 + y - size.h / 2;
  return clampOnScreen(left, top, size, screen);
}

/** Pull a widget back on screen if it's partly off (e.g. after switching to a smaller monitor). */
export function clampOnScreen(left: number, top: number, size: Size, screen: Size) {
  const maxX = Math.max(MARGIN, screen.w - size.w - MARGIN);
  const maxY = Math.max(MARGIN, screen.h - size.h - MARGIN);
  return {
    x: Math.round(Math.min(maxX, Math.max(MARGIN, left))),
    y: Math.round(Math.min(maxY, Math.max(MARGIN, top))),
  };
}
