// Mirrors crates/telemetry/src/model.rs. Keep the two in sync.

export type SourceKind = "none" | "iracing" | "demo";

export interface StatusPayload {
  source: SourceKind;
  connected: boolean;
  message: string;
}

export interface SessionPayload {
  trackName: string;
  trackConfig: string;
  trackLengthKm: number;
  sessionType: string;
  playerCarIdx: number;
  playerClassId: number;
  carName: string;
  fuelMaxLtr: number;
  estLapTime: number;
  shiftRpm: number;
  redlineRpm: number;
  multiclass: boolean;
  tireCompounds: string[];
  /** Where each timing sector starts, as a fraction of the lap. */
  sectors: number[];
}

/** Seconds versus each comparison lap at this point of the lap. Positive = slower. */
export interface Deltas {
  best: number | null;
  optimal: number | null;
  sessionBest: number | null;
  sessionOptimal: number | null;
  last: number | null;
  reference: number | null;
}

export type DeltaMode = keyof Deltas;

export interface ReferencePayload {
  loaded: boolean;
  trackName: string;
  trackConfig: string;
  carName: string;
  driver: string;
  lapTime: number;
  source: string;
  matchesTrack: boolean;
  matchesCar: boolean;
  kind: "ibt" | "blap";
  hasSteering: boolean;
}

export interface IbtLap {
  lap: number;
  lapTime: number;
  clean: boolean;
}

export interface IbtSummary {
  trackName: string;
  trackConfig: string;
  carName: string;
  driver: string;
  laps: IbtLap[];
  kind: "ibt" | "blap";
}

export interface TickPayload {
  sessionTime: number;
  isOnTrack: boolean;
  onPitRoad: boolean;
  speed: number; // m/s
  rpm: number;
  /** Shift up at this RPM in the current gear (where the car's lights blink). */
  shiftRpm: number | null;
  gear: number; // -1 reverse, 0 neutral
  throttle: number;
  brake: number;
  clutch: number;
  steering: number;
  lap: number;
  lapDistPct: number;
  lapCurrent: number;
  lapLast: number | null;
  lapBest: number | null;
  /** Your best clean sectors this session added up. */
  lapOptimal: number | null;
  deltas: Deltas;
  fuelLevel: number;
  carLeftRight: number;
  incidents: number;
  nearby: NearbyCar[];
  /** Ghost inputs and the live coaching cue, while a reference lap for this track is loaded. */
  coach: CoachPayload | null;
  /** g-forces, attitude and balance for the Dynamics widget. */
  dynamics: DynamicsPayload;
  /** Warning lights, engine gauges and brake bias. */
  instruments: InstrumentsPayload;
  /** Car's compass heading, radians clockwise from north. Null when iRacing doesn't publish it. */
  heading: number | null;
}

export interface Warnings {
  waterTemp: boolean;
  fuelPressure: boolean;
  oilPressure: boolean;
  oilTemp: boolean;
  stalled: boolean;
  pitLimiter: boolean;
  revLimiter: boolean;
  repairRequired: boolean;
  repairOptional: boolean;
  /** Worked out by Tenths from fuel per lap, not an iRacing flag. */
  lowFuel: boolean;
}

export interface InstrumentsPayload {
  warnings: Warnings;
  /** °C */
  oilTemp: number | null;
  /** bar */
  oilPress: number | null;
  /** °C */
  waterTemp: number | null;
  /** bar */
  fuelPress: number | null;
  voltage: number | null;
  /** % front; null when the car has no adjustable bias. */
  brakeBias: number | null;
  brakeBiasFine: number | null;
  peakBrakeBias: number | null;
}

export interface DynamicsPayload {
  /** Sideways g, positive = accelerating left (turning left). */
  latG: number;
  /** Fore-aft g, positive = accelerating, negative = braking. */
  longG: number;
  /** Vertical g; 1.0 sitting still. */
  vertG: number;
  /** Degrees, positive = nose up. */
  pitchDeg: number;
  /** Degrees, positive = leaning right. */
  rollDeg: number;
  /** Degrees between heading and direction of travel, positive = sliding left. */
  slipDeg: number;
  yawRateDeg: number;
  /** -1 understeer .. +1 oversteer; null when not cornering hard enough or still learning. */
  balance: number | null;
}

export type CueKind =
  | "brakeSoon" | "liftSoon" | "brakeNow" | "brakeEarly" | "noBrakeHere" | "brakeHarder" | "brakeLighter" | "goodBraking"
  | "holdBrake" | "releaseBrake" | "liftLess"
  | "carrySpeed" | "fastApex" | "throttleNow" | "moreThrottle" | "shiftDown" | "shiftUp" | "betterExit"
  | "turnMore" | "turnLess" | "cornerVerdict" | "gaining" | "losing" | "onPace";

/** The main reason a corner gained or lost time (corner verdicts and the focus corner). */
export type CueIssue =
  | "gained" | "brakedEarly" | "brakedLate" | "brakeHarder" | "slowApex" | "fastEntry" | "latePickup"
  | "liftedEarly" | "liftedTooMuch" | "wrongGear" | "entry" | "exit";

export type CueTone = "neutral" | "info" | "good" | "warn" | "bad";

export interface Cue {
  kind: CueKind;
  tone: CueTone;
  /** Metres to (+) or past (−) the reference's point. Verdicts: how much later (+) or earlier (−) you did it. */
  distanceM: number | null;
  /** Pedal 0..1, speed m/s or gear, depending on the cue. */
  ours: number | null;
  reference: number | null;
  gear: number | null;
  /** Counted from the start line. */
  corner: number | null;
  /** Verdicts: seconds lost in the corner (negative = gained). */
  timeS: number | null;
  issue: CueIssue | null;
}

/** The corner costing you most, averaged over your recent passes. */
export interface CoachFocus {
  corner: number;
  /** Average seconds lost per pass. */
  lossS: number;
  issue: CueIssue | null;
  passes: number;
}

export interface CoachPayload {
  refSpeed: number | null;
  refThrottle: number | null;
  refBrake: number | null;
  refGear: number | null;
  /** rad, positive = left. Only .ibt references record steering. */
  refSteering: number | null;
  /** Your speed minus the reference's, m/s. */
  speedDelta: number | null;
  /** Null while there's nothing to coach: off track, in the pits or under yellow. */
  cue: Cue | null;
  focus: CoachFocus | null;
}

export interface NearbyCar {
  carIdx: number;
  /** metres, positive = ahead */
  dist: number;
  /** -1 left, 0 in line, 1 right */
  side: -1 | 0 | 1;
  classColor: string;
}

/** green = personal best, purple = fastest in class */
export type LapFlag = "none" | "personal" | "fastest";

export interface LapInfo {
  lastLap: number | null;
  bestLap: number | null;
  lastFlag: LapFlag;
  classFastest: boolean;
}

export interface DriverCell {
  carIdx: number;
  name: string;
  abbrev: string;
  number: string;
  classId: number;
  className: string;
  classColor: string;
  irating: number;
  license: string;
  licenseColor: string;
}

/** Where a car is: on track, in the pit lane, stopped in its pit box, or out of the sim (garage, towed, quit). */
export type CarStatus = "track" | "pitLane" | "pitStall" | "out";

export interface RelativeRow extends DriverCell, LapInfo {
  position: number;
  classPosition: number;
  gap: number;
  lapStatus: -1 | 0 | 1;
  onPitRoad: boolean;
  status: CarStatus;
  isPlayer: boolean;
}

export interface StandingRow extends DriverCell, LapInfo {
  position: number;
  classPosition: number;
  lap: number;
  gap: number | null;
  lapsDown: number;
  interval: number | null;
  /** Class position at the start of the race; null outside races or when unknown. */
  startPosition: number | null;
  onPitRoad: boolean;
  status: CarStatus;
  isPlayer: boolean;
}

export interface FuelPayload {
  level: number;
  max: number;
  perLap: number | null;
  lastLapUsed: number | null;
  lapsOfFuel: number | null;
  lapsToGo: number | null;
  toFinish: number | null;
  samples: number;
}

export interface SessionClock {
  sessionType: string;
  timeRemain: number | null;
  lapsRemain: number | null;
  totalLaps: number | null;
  flags: number;
  trackTemp: number;
  airTemp: number;
  /** SoF of the player's class. */
  strengthOfField: number | null;
  /** Fastest class first. */
  classes: ClassSummary[];
  /** Seconds since midnight in the sim. */
  timeOfDay: number | null;
  /** 0 clear, 1 partly cloudy, 2 mostly cloudy, 3 overcast */
  skies: number | null;
  /** 1 dry .. 7 extremely wet, 0 unknown */
  trackWetness: number | null;
  declaredWet: boolean;
  /** m/s */
  windSpeed: number | null;
  /** radians */
  windDir: number | null;
  /** 0..1 */
  humidity: number | null;
  /** 0..1, when iRacing publishes it. */
  precipitation: number | null;
  /** 0..1 */
  fogLevel: number | null;
  /** Yours, or the team's in team events. */
  incidents: number;
  incidentLimit: number | null;
  fastRepairsUsed: number;
  fastRepairsLimit: number | null;
}

export interface ClassSummary {
  classId: number;
  name: string;
  color: string;
  cars: number;
  strengthOfField: number | null;
}

export interface TireCorner {
  /** °C, left / middle / right */
  temp: [number, number, number];
  /** tread remaining 0..1 */
  wear: [number, number, number];
  /** kPa, as set in the garage */
  coldPressure: number;
}

export interface TiresPayload {
  /** LF, RF, LR, RR */
  corners: TireCorner[];
  compound: string;
  setsUsed: number;
  setsAvailable: number | null;
  /** In the pit box, so readings are current. */
  live: boolean;
  hasData: boolean;
}

/** What iRacing has ticked for your next pit stop. */
export interface PitPayload {
  /** False while spectating, when iRacing doesn't publish pit service. */
  available: boolean;
  /** Tires to change, LF, RF, LR, RR. */
  tires: [boolean, boolean, boolean, boolean];
  /** kPa each tire will be set to. */
  pressures: [number, number, number, number];
  compound: string;
  fuel: boolean;
  /** Litres to add. */
  fuelAmount: number;
  tearoff: boolean;
  fastRepair: boolean;
  /** null when unlimited or unknown. */
  fastRepairsLeft: number | null;
  /** 0 none, 1 in progress, 2 done, 100+ a problem (too far forward...). */
  status: number;
  inPitStall: boolean;
}

/** A pit command for iRacing. Fuel is in whole litres and pressure in kPa; 0 keeps iRacing's current value. */
export type PitCommand =
  | { kind: "clear" }
  | { kind: "fuel"; litres: number }
  | { kind: "clearFuel" }
  | { kind: "tire"; corner: number; kpa: number }
  | { kind: "clearTires" }
  | { kind: "tearoff" }
  | { kind: "clearTearoff" }
  | { kind: "fastRepair" }
  | { kind: "clearFastRepair" }
  | { kind: "compound"; index: number };

/** A wheel, button box or gamepad. */
export interface Controller {
  device: string;
  name: string;
}

/** A controller button press, sent to every window. */
export interface ButtonPress {
  device: string;
  name: string;
  button: number;
}

/** What a pit hotkey asks for. */
export type PitAction = "fuel" | "fuelUp" | "fuelDown" | "tires" | "fastRepair" | "tearoff" | "clear";

export interface FieldPayload {
  clock: SessionClock;
  relative: RelativeRow[];
  standings: StandingRow[];
  fuel: FuelPayload;
  tires: TiresPayload;
  pit: PitPayload;
  lapLog: LapLogPayload;
  /** Every car's stints this session. */
  stints: CarStints[];
  /** Average pit stop loss per class, once enough stops have been timed. */
  pitLoss: PitLoss[];
}

export interface PitLoss {
  classId: number;
  /** Average seconds a stop costs: pit lane time minus racing time for the same stretch. */
  lossS: number;
  /** Average time standing in the box. */
  stallS: number;
  stops: number;
}

export interface Stint {
  /** Race progress (laps completed plus distance round the lap) at the start. */
  start: number;
  /** At the end, or where the car is now for the one it's on. */
  end: number;
  open: boolean;
  endedBy: "pit" | "out" | null;
  /** Index into the session's tireCompounds, -1 if unknown. */
  compound: number;
  /** Only part of it was seen (Tenths started, or the car appeared, mid-stint). */
  partial: boolean;
}

export interface CarStints {
  carIdx: number;
  stints: Stint[];
  /** Times into the pit lane. */
  stops: number;
  inPits: boolean;
  /** Left the sim (retired, towed, quit). */
  out: boolean;
}

export interface LapRecord {
  lap: number;
  /** null when the lap couldn't be timed. */
  time: number | null;
  /** On track the whole way, off pit road, no new incidents. */
  clean: boolean;
  /** An in or out lap. */
  pit: boolean;
  offTrack: boolean;
  incidents: number;
  /** Litres. */
  fuelUsed: number | null;
  trackTemp: number;
  airTemp: number;
  /** Beat the saved personal best when it was set. */
  personalBest: boolean;
  /** Unix seconds. */
  at: number;
}

export interface LapLogPayload {
  /** This session's laps, oldest first. */
  laps: LapRecord[];
  sessionBest: number | null;
  /** All-time for this car and track. */
  personalBest: { time: number; at: number } | null;
  /** Laps are being saved to disk (not in the demo). */
  saving: boolean;
}

/** A saved layout, as listed. */
export interface ProfileSummary {
  id: string;
  name: string;
  /** Unix seconds. */
  updatedAt: number;
}

/** A saved layout file. */
export interface Profile {
  format: "tenths-layout";
  version: number;
  name: string;
  updatedAt: number;
  /** Same shape as AppConfig["widgets"]. */
  widgets: unknown;
}

export interface Snapshot {
  status: StatusPayload | null;
  session: SessionPayload | null;
  reference: ReferencePayload | null;
  editMode: boolean;
  overlayVisible: boolean;
  demo: boolean;
}

/** CarLeftRight values from the SDK. */
export const CarLeftRight = {
  Off: 0,
  Clear: 1,
  CarLeft: 2,
  CarRight: 3,
  CarLeftRight: 4,
  TwoCarsLeft: 5,
  TwoCarsRight: 6,
} as const;

/** A connected display (monitor), from the app. */
export interface Display {
  /** Stable id: Windows' device name, like "\\.\DISPLAY2". */
  key: string;
  /** 1 for the primary, then left to right. */
  number: number;
  primary: boolean;
  /** Physical pixels on the desktop. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** Windows display scaling: 1 = 100%, 1.5 = 150%. */
  scale: number;
}
