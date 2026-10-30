// Words for the coaching cues. The engine decides *what* to say (and sends
// the numbers); this decides how to say it, in the user's units.

import { speed as toSpeed, speedUnit, type Units } from "./format";
import type { Cue } from "./types";

const pct = (v: number | null) => `${Math.round((v ?? 0) * 100)}%`;
const metres = (v: number | null) => {
  const m = Math.abs(v ?? 0);
  return `${m >= 30 ? Math.round(m / 10) * 10 : Math.round(m)} m`;
};
const ordinal = (n: number | null) => {
  const g = n ?? 0;
  const s = g === 1 ? "st" : g === 2 ? "nd" : g === 3 ? "rd" : "th";
  return `${g}${s}`;
};

export function cueText(cue: Cue, units: Units): { label: string; detail: string } {
  const spd = (ms: number | null) => `${toSpeed(ms ?? 0, units)}`;
  const diff = () => {
    const d = toSpeed(Math.abs((cue.ours ?? 0) - (cue.reference ?? 0)), units);
    return `${d} ${speedUnit(units)}`;
  };
  switch (cue.kind) {
    case "brakeSoon":
      return { label: `Brake in ${metres(cue.distanceM)}`, detail: "Reference brake point ahead" };
    case "brakeNow":
      return { label: "Brake now", detail: `Reference braked ${metres(cue.distanceM)} ago` };
    case "brakeEarly":
      return { label: "Braking early", detail: `Reference brakes ${metres(cue.distanceM)} later` };
    case "noBrakeHere":
      return { label: "No braking here", detail: "Reference stays off the brakes" };
    case "brakeHarder":
      return { label: "Brake harder", detail: `You ${pct(cue.ours)}, reference ${pct(cue.reference)}` };
    case "brakeLighter":
      return { label: "Brake lighter", detail: `You ${pct(cue.ours)}, reference ${pct(cue.reference)}` };
    case "goodBraking":
      return { label: "Good braking", detail: `Matching the reference at ${pct(cue.reference)}` };
    case "carrySpeed":
      return { label: "Carry more speed", detail: `${spd(cue.ours)} vs ${spd(cue.reference)} ${speedUnit(units)} at the apex` };
    case "fastApex":
      return { label: "Faster at the apex", detail: `${diff()} up on the reference` };
    case "throttleNow":
      return { label: "Throttle now", detail: `Reference is already at ${pct(cue.reference)}` };
    case "moreThrottle":
      return { label: "More throttle", detail: `You ${pct(cue.ours)}, reference ${pct(cue.reference)}` };
    case "shiftDown":
      return { label: `Shift down to ${cue.gear}`, detail: `Reference uses ${ordinal(cue.gear)} here` };
    case "shiftUp":
      return { label: `Shift up to ${cue.gear}`, detail: `Reference uses ${ordinal(cue.gear)} here` };
    case "betterExit":
      return { label: "Better exit needed", detail: `${diff()} down on the straight` };
    case "turnMore":
      return { label: "Turn in more", detail: "Reference takes a tighter line here" };
    case "turnLess":
      return { label: "Open the wheel", detail: "You're turning tighter than the reference" };
    case "gaining":
      return { label: "Gaining", detail: `${(cue.ours ?? 0).toFixed(2)}s in the last 2 seconds` };
    case "losing":
      return { label: "Losing time", detail: `${(cue.ours ?? 0).toFixed(2)}s in the last 2 seconds` };
    case "onPace":
      return { label: "On reference pace", detail: "Matching the reference" };
  }
}
