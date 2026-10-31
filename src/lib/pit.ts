// Pit service: the fuel plan, what each pit hotkey sends, and the hook that
// listens for those hotkeys. The commands themselves go through the Rust side
// (telemetry/src/pit.rs) to iRacing, which never replies; the Pit stop widget
// shows what iRacing reports as ticked instead.

import { useEffect } from "react";
import { create } from "zustand";
import { api, isTauri, on } from "./bridge";
import { opt } from "./config";
import { fuel as conv, fuelUnit, type Units } from "./format";
import { useLive } from "./store";
import type { FuelPayload, PitAction, PitCommand, PitPayload } from "./types";

export type FuelMode = "finish" | "full" | "manual";

/** Litres needed to finish beyond what's in the tank, with the safety margin (negative = spare). */
export function fuelToFinish(f: FuelPayload | null | undefined, marginLaps: number): number | null {
  if (f?.toFinish == null || f.perLap == null) return null;
  return f.toFinish + marginLaps * f.perLap;
}

/**
 * Whole litres to add for a fuel plan: 0 when there's enough, null when it
 * isn't known yet or the plan is to keep the amount set in iRacing.
 */
export function fuelPlan(mode: FuelMode, f: FuelPayload | null | undefined, marginLaps: number): number | null {
  if (mode === "manual" || !f) return null;
  const need = mode === "full" ? f.max - f.level : fuelToFinish(f, marginLaps);
  return need == null ? null : Math.max(0, Math.ceil(need));
}

/** Whole litres after one press of the fuel up or down hotkey, kept between empty and a full tank. */
export function steppedFuel(from: number, delta: number, tankMax: number): number {
  const next = Math.max(0, Math.round(from) + delta);
  return tankMax > 0 ? Math.min(next, Math.ceil(tankMax)) : next;
}

/**
 * The commands for a hotkey, apart from fuel up and down. Each one toggles:
 * fuel sets the plan, or unticks fuel when that amount is already set; tires
 * tick all four, or untick them when any are ticked.
 */
export function pitCommands(action: Exclude<PitAction, "fuelUp" | "fuelDown">, pit: PitPayload, plan: number | null, mode: FuelMode): PitCommand[] {
  switch (action) {
    case "fuel":
      if (mode === "manual") return [pit.fuel ? { kind: "clearFuel" } : { kind: "fuel", litres: 0 }];
      if (plan == null) return [];
      if (plan <= 0 || (pit.fuel && Math.abs(pit.fuelAmount - plan) < 1)) return [{ kind: "clearFuel" }];
      return [{ kind: "fuel", litres: plan }];
    case "tires":
      if (pit.tires.some(Boolean)) return [{ kind: "clearTires" }];
      return [0, 1, 2, 3].map((corner) => ({ kind: "tire", corner, kpa: 0 }));
    case "fastRepair":
      return [pit.fastRepair ? { kind: "clearFastRepair" } : { kind: "fastRepair" }];
    case "tearoff":
      return [pit.tearoff ? { kind: "clearTearoff" } : { kind: "tearoff" }];
    case "clear":
      return [{ kind: "clear" }];
  }
}

/** The pit action last pressed and when, so the widget can light up its key. */
export const usePitPressed = create<{ action: PitAction | null; at: number }>(() => ({ action: null, at: 0 }));

/** What was last sent, for the widget to show briefly. */
export const usePitSent = create<{ text: string | null; at: number }>(() => ({ text: null, at: 0 }));

function describe(cmds: PitCommand[], fuel: string): string {
  const c = cmds[0];
  if (!c) return "";
  switch (c.kind) {
    case "fuel": return c.litres > 0 ? `Fuel set to ${fuel}` : "Fuel ticked";
    case "clearFuel": return "Fuel unticked";
    case "tire": return "Four tires ticked";
    case "clearTires": return "Tires unticked";
    case "fastRepair": return "Fast repair ticked";
    case "clearFastRepair": return "Fast repair unticked";
    case "tearoff": return "Tear-off ticked";
    case "clearTearoff": return "Tear-off unticked";
    case "clear": return "Everything unticked";
    case "compound": return "Compound changed";
  }
}

async function send(cmds: PitCommand[], fuelText: string) {
  if (!cmds.length) return;
  for (const c of cmds) await api.pitCommand(c);
  usePitSent.setState({ text: describe(cmds, fuelText), at: performance.now() });
}

function fuelText(litres: number, units: Units) {
  return `${conv(litres, units).toFixed(units === "imperial" ? 1 : 0)} ${fuelUnit(units)}`;
}

function plan() {
  const s = useLive.getState();
  const mode = opt<FuelMode>(s.config.widgets.pitstop, "fuelMode", "finish");
  const litres = fuelPlan(mode, s.field?.fuel, opt(s.config.widgets.fuel, "marginLaps", 0.5));
  const text = litres == null ? "" : fuelText(litres, s.config.general.units);
  return { s, mode, litres, text };
}

/**
 * The amount the last fuel up or down press set. iRacing's readback takes a
 * moment to arrive, so quick presses step from this rather than from a stale
 * reading.
 */
let stepped: { litres: number; at: number } | null = null;
const STEP_HOLD_MS = 1500;
/** Fuel was set by hand with up or down, so auto fuel leaves the next stop alone. */
let fuelByHand = false;

export function runPitAction(action: PitAction) {
  usePitPressed.setState({ action, at: performance.now() });
  const { s, mode, litres, text } = plan();
  const pit = s.field?.pit;
  if (!pit?.available) return;
  if (action === "fuelUp" || action === "fuelDown") {
    const now = performance.now();
    const from = stepped && now - stepped.at < STEP_HOLD_MS ? stepped.litres : pit.fuel ? pit.fuelAmount : 0;
    const step = opt(s.config.widgets.pitstop, "fuelStep", 5);
    const next = steppedFuel(from, action === "fuelUp" ? step : -step, s.field?.fuel.max ?? 0);
    stepped = { litres: next, at: now };
    fuelByHand = true;
    void send([next > 0 ? { kind: "fuel", litres: next } : { kind: "clearFuel" }], fuelText(next, s.config.general.units));
    return;
  }
  stepped = null;
  if (action === "fuel" || action === "clear") fuelByHand = false;
  void send(pitCommands(action, pit, litres, mode), text);
}

/** Browser preview: Shift + F, ↑, ↓, T, R, W and X stand in for the Ctrl+Shift hotkeys. */
const PREVIEW_KEYS: Record<string, PitAction> = {
  f: "fuel", arrowup: "fuelUp", arrowdown: "fuelDown", t: "tires", r: "fastRepair", w: "tearoff", x: "clear",
};

/**
 * Pit hotkeys and auto fuel, mounted in the overlay. While the Pit stop
 * widget is on and you're in the car, takes the pit hotkeys and runs them;
 * and, if asked, sets the fuel as you enter the pit lane. Pass `owner` false
 * in all but one window: with an overlay on every display, only the primary
 * display's sends commands, so nothing is sent twice.
 */
export function usePitControl(owner = true) {
  const enabled = useLive((s) => s.config.widgets.pitstop.enabled);
  const connected = useLive((s) => s.status?.connected ?? false);
  const onTrack = useLive((s) => s.tick?.isOnTrack ?? false);
  const live = owner && enabled && connected && onTrack;

  useEffect(() => {
    if (owner) void api.setPitActive(live);
  }, [owner, live]);

  useEffect(() => {
    if (!live) return;
    const off = on<PitAction>("pit-shortcut", runPitAction);
    const key = (e: KeyboardEvent) => {
      const action = PREVIEW_KEYS[e.key.toLowerCase()];
      if (!isTauri && action && e.shiftKey && !e.ctrlKey && !e.metaKey) runPitAction(action);
    };
    window.addEventListener("keydown", key);
    return () => {
      void off.then((f) => f());
      window.removeEventListener("keydown", key);
    };
  }, [live]);

  // Auto fuel: set the planned amount as you cross into the pit lane.
  useEffect(() => {
    if (!live) return;
    return useLive.subscribe((s, prev) => {
      const entered = s.tick?.onPitRoad && prev.tick && !prev.tick.onPitRoad;
      if (!entered || !opt(s.config.widgets.pitstop, "autoFuel", false)) return;
      if (fuelByHand) {
        fuelByHand = false;
        return;
      }
      const { mode, litres, text } = plan();
      const pit = s.field?.pit;
      if (mode === "manual" || litres == null || !pit?.available) return;
      if (pit.fuel && Math.abs(pit.fuelAmount - litres) < 1) return;
      void send([litres > 0 ? { kind: "fuel", litres } : { kind: "clearFuel" }], text);
    });
  }, [live]);
}
