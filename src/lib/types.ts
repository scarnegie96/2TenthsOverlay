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
}

export interface TickPayload {
  sessionTime: number;
  isOnTrack: boolean;
  onPitRoad: boolean;
  speed: number; // m/s
  rpm: number;
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
  deltaBest: number | null;
  fuelLevel: number;
  carLeftRight: number;
  incidents: number;
  nearby: NearbyCar[];
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

export interface RelativeRow extends DriverCell, LapInfo {
  position: number;
  classPosition: number;
  gap: number;
  lapStatus: -1 | 0 | 1;
  onPitRoad: boolean;
  isPlayer: boolean;
}

export interface StandingRow extends DriverCell, LapInfo {
  position: number;
  classPosition: number;
  lap: number;
  gap: number | null;
  lapsDown: number;
  interval: number | null;
  onPitRoad: boolean;
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

export interface FieldPayload {
  clock: SessionClock;
  relative: RelativeRow[];
  standings: StandingRow[];
  fuel: FuelPayload;
  tires: TiresPayload;
}

export interface Snapshot {
  status: StatusPayload | null;
  session: SessionPayload | null;
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
