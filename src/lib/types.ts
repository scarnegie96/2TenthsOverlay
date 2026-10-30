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

export interface RelativeRow extends DriverCell {
  position: number;
  classPosition: number;
  gap: number;
  lapStatus: -1 | 0 | 1;
  onPitRoad: boolean;
  isPlayer: boolean;
}

export interface StandingRow extends DriverCell {
  position: number;
  classPosition: number;
  lap: number;
  gap: number | null;
  lapsDown: number;
  interval: number | null;
  lastLap: number | null;
  bestLap: number | null;
  classFastest: boolean;
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

export interface FieldPayload {
  clock: SessionClock;
  relative: RelativeRow[];
  standings: StandingRow[];
  fuel: FuelPayload;
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
