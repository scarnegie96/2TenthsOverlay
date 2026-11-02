// Words for the coaching cues. The engine decides *what* to say (and sends
// the numbers); this decides how to say it, in the user's units.

import { signed, speed as toSpeed, speedUnit, type Units } from "./format";
import type { Cue, CueIssue } from "./types";

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

/** A few words for why a corner cost time, e.g. in the focus line. */
export function issueText(issue: CueIssue | null): string {
  switch (issue) {
    case "gained": return "faster than the reference";
    case "brakedEarly": return "braking early";
    case "brakedLate": return "braking late, slow apex";
    case "brakeHarder": return "braking too lightly";
    case "slowApex": return "slow at the apex";
    case "fastEntry": return "too fast in, slow out";
    case "latePickup": return "late on the throttle";
    case "liftedEarly": return "lifting early";
    case "liftedTooMuch": return "lifting too much";
    case "wrongGear": return "gear at the apex";
    case "entry": return "entry";
    case "exit": return "exit";
    default: return "";
  }
}

/** The corner summary given on the way out: the fix first, then how much it cost. */
function verdictText(cue: Cue, units: Units): { label: string; detail: string } {
  const unit = speedUnit(units);
  const spd = (ms: number | null) => `${toSpeed(ms ?? 0, units)}`;
  const t = cue.timeS ?? 0;
  const where = `Corner ${cue.corner ?? "?"}`;
  const cost = `${where}: ${t >= 0 ? "lost" : "gained"} ${Math.abs(t).toFixed(2)}s`;
  const late = (cue.distanceM ?? 0) >= 0;
  switch (cue.issue) {
    case "gained":
      return { label: `Gained ${Math.abs(t).toFixed(2)}s`, detail: `${where}, faster than the reference` };
    case "brakedEarly":
      return { label: `Braked ${metres(cue.distanceM)} early`, detail: cost };
    case "brakedLate":
      return { label: `Braked ${metres(cue.distanceM)} late`, detail: `${cost}, ${spd(cue.ours)} vs ${spd(cue.reference)} ${unit} at the apex` };
    case "brakeHarder":
      return { label: "Brake harder", detail: `${cost}, peak ${pct(cue.ours)} vs ${pct(cue.reference)}` };
    case "slowApex":
      return { label: "Carry more speed", detail: `${cost}, ${spd(cue.ours)} vs ${spd(cue.reference)} ${unit} at the apex` };
    case "fastEntry":
      return { label: "Slower in, faster out", detail: `${cost}, ${spd(cue.ours)} vs ${spd(cue.reference)} ${unit} at the apex` };
    case "latePickup":
      return { label: "Throttle earlier", detail: `${cost}, ${metres(cue.distanceM)} ${late ? "later" : "earlier"} than the reference` };
    case "liftedEarly":
      return { label: `Lifted ${metres(cue.distanceM)} early`, detail: cost };
    case "liftedTooMuch":
      return { label: "Lift less", detail: `${cost}, down to ${pct(cue.ours)} vs ${pct(cue.reference)} throttle` };
    case "wrongGear":
      return { label: `Use ${ordinal(cue.gear)} at the apex`, detail: `${cost}, you were in ${ordinal(cue.ours)}` };
    case "entry":
      return { label: `${signed(t)}s on entry`, detail: `${where}, up to the apex` };
    case "exit":
      return { label: `${signed(t)}s on exit`, detail: `${where}, from the apex on` };
    default:
      return { label: `${signed(t)}s`, detail: where };
  }
}

export function cueText(cue: Cue, units: Units): { label: string; detail: string } {
  const spd = (ms: number | null) => `${toSpeed(ms ?? 0, units)}`;
  const diff = () => {
    const d = toSpeed(Math.abs((cue.ours ?? 0) - (cue.reference ?? 0)), units);
    return `${d} ${speedUnit(units)}`;
  };
  switch (cue.kind) {
    case "brakeSoon":
      return { label: `Brake in ${metres(cue.distanceM)}`, detail: "Reference brake point ahead" };
    case "liftSoon":
      return { label: `Lift in ${metres(cue.distanceM)}`, detail: `Reference lifts to ${pct(cue.reference)}, no brake` };
    case "brakeNow":
      return { label: "Brake now", detail: `Reference braked ${metres(cue.distanceM)} ago` };
    case "brakeEarly":
      return { label: "Braking early", detail: `Reference brakes ${metres(cue.distanceM)} later` };
    case "noBrakeHere":
      return { label: "No braking here", detail: "Reference stays off the brakes" };
    case "brakeHarder":
      return { label: "Brake harder", detail: `You ${pct(cue.ours)}, reference ${pct(cue.reference)} from its brake point` };
    case "brakeLighter":
      return { label: "Brake lighter", detail: `You ${pct(cue.ours)}, reference ${pct(cue.reference)} from its brake point` };
    case "goodBraking":
      return { label: "Good braking", detail: `Matching the reference at ${pct(cue.reference)}` };
    case "holdBrake":
      return { label: "Trail the brake", detail: `Reference still at ${pct(cue.reference)} here` };
    case "releaseBrake":
      return { label: "Release the brake", detail: "Reference is off the brake here" };
    case "liftLess":
      return { label: "Lift less", detail: `You ${pct(cue.ours)}, reference ${pct(cue.reference)}` };
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
      return { label: "Turn in more", detail: "Reference turns tighter here" };
    case "turnLess":
      return { label: "Open the wheel", detail: "You're turning tighter than the reference" };
    case "cornerVerdict":
      return verdictText(cue, units);
    case "gaining":
      return { label: "Gaining", detail: `${(cue.ours ?? 0).toFixed(2)}s in the last 2 seconds` };
    case "losing":
      return { label: "Losing time", detail: `${(cue.ours ?? 0).toFixed(2)}s in the last 2 seconds` };
    case "onPace":
      return { label: "On reference pace", detail: "Matching the reference" };
  }
}
