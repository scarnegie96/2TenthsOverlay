// Every action a hotkey or controller button can run, and how bindings are
// shown. The overlay sends the bindings to Rust (src-tauri/src/controls.rs),
// which owns the global shortcuts and reads wheel and controller buttons.

/** A keyboard shortcut. `code` is the physical key, as in KeyboardEvent.code ("KeyF", "ArrowUp", "F13"). */
export interface KeyBinding {
  code: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
}

/** A button on a wheel, button box or gamepad. */
export interface ButtonBinding {
  /** Stable id from the device's hardware id, like "VID_0EB7&PID_0E04&MI_00". */
  device: string;
  /** The device's name when it was bound, for showing while it's unplugged. */
  name: string;
  /** Button number from 1, or a hat direction (see HAT_BUTTONS). */
  button: number;
}

export interface ControlBinding {
  key: KeyBinding | null;
  button: ButtonBinding | null;
}

export type ActionId =
  | "overlay.edit" | "overlay.toggle" | "layout.next"
  | "layout.1" | "layout.2" | "layout.3" | "layout.4" | "layout.5" | "layout.6" | "layout.7" | "layout.8" | "layout.9"
  | "pit.fuel" | "pit.fuelUp" | "pit.fuelDown" | "pit.tires" | "pit.fastRepair" | "pit.tearoff" | "pit.clear";

export interface ActionMeta {
  id: ActionId;
  label: string;
  group: "Overlay" | "Layouts" | "Pit stop";
  /** Ctrl+Shift + this key by default. */
  key: string;
}

export const ACTIONS: ActionMeta[] = [
  { id: "overlay.edit", label: "Edit layout", group: "Overlay", key: "KeyE" },
  { id: "overlay.toggle", label: "Show or hide overlays", group: "Overlay", key: "KeyH" },
  { id: "layout.next", label: "Next layout", group: "Layouts", key: "KeyL" },
  ...([1, 2, 3, 4, 5, 6, 7, 8, 9] as const).map((n): ActionMeta => ({
    id: `layout.${n}`, label: `Layout ${n}`, group: "Layouts", key: `Digit${n}`,
  })),
  { id: "pit.fuel", label: "Fuel: set the plan, or untick", group: "Pit stop", key: "KeyF" },
  { id: "pit.fuelUp", label: "More fuel", group: "Pit stop", key: "ArrowUp" },
  { id: "pit.fuelDown", label: "Less fuel", group: "Pit stop", key: "ArrowDown" },
  { id: "pit.tires", label: "Four tires, or none", group: "Pit stop", key: "KeyT" },
  { id: "pit.fastRepair", label: "Fast repair", group: "Pit stop", key: "KeyR" },
  { id: "pit.tearoff", label: "Tear-off", group: "Pit stop", key: "KeyW" },
  { id: "pit.clear", label: "Untick everything", group: "Pit stop", key: "KeyX" },
];

/** Button numbers past real buttons stand for the four directions of a hat (the d-pad on most wheels). */
export const HAT_BUTTONS: Record<number, string> = { 1001: "Hat up", 1002: "Hat right", 1003: "Hat down", 1004: "Hat left" };
const HAT_ARROWS: Record<number, string> = { 1001: "Hat ↑", 1002: "Hat →", 1003: "Hat ↓", 1004: "Hat ←" };

export function defaultControls(): Record<ActionId, ControlBinding> {
  return Object.fromEntries(
    ACTIONS.map((a) => [a.id, { key: { code: a.key, ctrl: true, shift: true, alt: false, meta: false }, button: null }]),
  ) as Record<ActionId, ControlBinding>;
}

const KEY_NAMES: Record<string, string> = {
  ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→", Space: "Space", Enter: "Enter", Backspace: "Backspace",
  Tab: "Tab", Escape: "Esc", Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Semicolon: ";", Quote: "'",
  Backquote: "`", Backslash: "\\", Comma: ",", Period: ".", Slash: "/", Insert: "Insert", Delete: "Delete", Home: "Home",
  End: "End", PageUp: "Page up", PageDown: "Page down", NumpadAdd: "Num +", NumpadSubtract: "Num -",
  NumpadMultiply: "Num *", NumpadDivide: "Num /", NumpadEnter: "Num Enter", NumpadDecimal: "Num .",
};

/** "KeyF" -> "F", "Digit1" -> "1", "Numpad4" -> "Num 4", "ArrowUp" -> "↑". */
export function keyName(code: string): string {
  if (KEY_NAMES[code]) return KEY_NAMES[code];
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Numpad")) return `Num ${code.slice(6)}`;
  return code;
}

/** "Ctrl+Shift+F" */
export function keyLabel(k: KeyBinding): string {
  return [k.ctrl && "Ctrl", k.alt && "Alt", k.shift && "Shift", k.meta && "Win", keyName(k.code)].filter(Boolean).join("+");
}

/** "Button 12" or "Hat up" */
export function buttonLabel(b: ButtonBinding): string {
  return HAT_BUTTONS[b.button] ?? `Button ${b.button}`;
}

/**
 * A short hint for a widget: the controller button if there is one (that's
 * what you'd press while driving), otherwise the key without its modifiers.
 */
export function shortLabel(b: ControlBinding | undefined): string | null {
  if (b?.button) return HAT_ARROWS[b.button.button] ?? `B${b.button.button}`;
  if (b?.key) return keyName(b.key.code);
  return null;
}

/** The full name of whatever runs an action, for sentences like "Ctrl+Shift+X unticks everything". */
export function fullLabel(b: ControlBinding | undefined): string | null {
  if (b?.button) return buttonLabel(b.button);
  if (b?.key) return keyLabel(b.key);
  return null;
}

export function sameKey(a: KeyBinding | null, b: KeyBinding | null): boolean {
  return !!a && !!b && a.code === b.code && a.ctrl === b.ctrl && a.shift === b.shift && a.alt === b.alt && a.meta === b.meta;
}

export function sameButton(a: ButtonBinding | null, b: ButtonBinding | null): boolean {
  return !!a && !!b && a.device === b.device && a.button === b.button;
}
