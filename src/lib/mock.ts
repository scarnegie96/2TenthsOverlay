// Browser-only stand-in for the Rust side. Generates a small fake race and
// keeps config in localStorage, syncing between tabs with BroadcastChannel
// (open /overlay.html and /settings.html side by side).
//
// The real maths lives in crates/telemetry; this only needs to look plausible.

import type {
  CoachPayload, Cue, Deltas, FieldPayload, LapFlag, ReferencePayload, NearbyCar, RelativeRow, SessionPayload, Snapshot, StandingRow, StatusPayload,
  TickPayload, TiresPayload,
} from "./types";

type Handler = (payload: unknown) => void;

const CONFIG_KEY = "tenths.config";
const TRACK_M = 5100;
const PLAYER = 6;
const NAMES = [
  "Lena Hoffmann", "Marco Bellini", "Tom Okafor", "Sakura Ito", "Jörg Lindqvist", "Diego Alvarez", "You",
  "Priya Raman", "Callum Reid", "Noah Fischer", "Elif Demir", "Mateus Costa", "Owen Walsh", "Hannah Kowalski",
  "Rafael Duarte", "Ines Moreau", "Kenji Sato", "Liam O'Brien", "Zoe Clarke", "Pieter de Vries", "Amara Nwosu",
];
const N = NAMES.length;
const CLASSES = [
  { id: 4074, name: "LMP2", color: "#f04e7a", lap: 86.3 },
  { id: 4029, name: "GT3", color: "#ffda59", lap: 92.1 },
  { id: 4048, name: "GT4", color: "#33ceff", lap: 99.4 },
];
const classOf = (i: number) => (i <= 4 ? 0 : i <= 14 ? 1 : 2);
const CORNERS = [0.06, 0.17, 0.31, 0.43, 0.52, 0.66, 0.79, 0.9];
/** Cars that dice with the player: [car, mean offset m, swing m, period s] */
const BATTLE: [number, number, number, number][] = [
  [7, -4, 9, 22],
  [8, 14, 11, 31],
];

/** iRacing's SoF formula (the real one is in the engine). */
function strengthOfField(iratings: number[]): number | null {
  const irs = iratings.filter((ir) => ir > 0);
  if (!irs.length) return null;
  const br = 1600 / Math.LN2;
  const sum = irs.reduce((acc, ir) => acc + Math.exp(-ir / br), 0);
  return Math.round(br * Math.log(irs.length / sum));
}

const rand = (i: number) => {
  const x = Math.sin(i * 12.9898) * 43758.5453;
  return x - Math.floor(x);
};

class Mock {
  private handlers = new Map<string, Set<Handler>>();
  private channel = new BroadcastChannel("tenths");
  private editMode = false;
  private t = 0;
  private progress = Array.from({ length: N }, (_, i) => 7.6 - (i + (classOf(i) === 2 ? 1.5 : 0)) * 0.028 - rand(i) * 0.01);
  private pace = Array.from({ length: N }, (_, i) => CLASSES[classOf(i)].lap + rand(i + 3) * 2.2);
  private last = this.pace.map((p, i) => p + 0.2 + rand(i + 5) * 0.8);
  private best = this.pace.map((p, i) => p + rand(i + 9) * 0.4);
  private lapStart = Array.from({ length: N }, () => 0);
  private sides = new Map<number, number>();
  private fuel = 31.4;
  private dyn = { speed: 0, time: 0, longG: 0, balance: null as number | null };
  private started = false;
  private reference: ReferencePayload = {
    loaded: false, trackName: "", trackConfig: "", carName: "", driver: "", lapTime: 0, source: "", matchesTrack: false,
    matchesCar: true, kind: "blap", hasSteering: false,
  };
  private cue: { cue: Cue; since: number; priority: number } | null = null;
  private refDeltas: [number, number][] = [];

  constructor() {
    this.channel.onmessage = (e) => {
      const { event, payload } = e.data as { event: string; payload: unknown };
      if (event === "edit-mode") this.editMode = payload as boolean;
      if (event === "reference") this.reference = payload as ReferencePayload;
      this.dispatch(event, payload);
    };
    for (const [i, base] of BATTLE) this.progress[i] = this.progress[PLAYER] + base / TRACK_M;
    // Give a few drivers a fresh personal best / class best so the colors show.
    this.last[2] = this.best[2];
    this.last[PLAYER] = this.best[PLAYER];
    this.best[9] = Math.min(...this.best.slice(5, 15)) - 0.1;
    this.last[9] = this.best[9];
  }

  on(event: string, handler: Handler): () => void {
    if (!this.handlers.has(event)) this.handlers.set(event, new Set());
    this.handlers.get(event)!.add(handler);
    this.start();
    return () => this.handlers.get(event)?.delete(handler);
  }

  private dispatch(event: string, payload: unknown) {
    this.handlers.get(event)?.forEach((h) => h(payload));
  }

  /** Emit to this page and every other open page. */
  private broadcast(event: string, payload: unknown) {
    this.dispatch(event, payload);
    this.channel.postMessage({ event, payload });
  }

  async call(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
    switch (cmd) {
      case "load_config": {
        try {
          const raw = localStorage.getItem(CONFIG_KEY);
          return raw ? JSON.parse(raw) : null;
        } catch {
          return null;
        }
      }
      case "save_config":
        try {
          localStorage.setItem(CONFIG_KEY, JSON.stringify(args.config));
        } catch {
          /* storage unavailable; keep in memory only */
        }
        this.broadcast("config-changed", args.config);
        return;
      case "get_snapshot":
        return {
          status: this.status(), session: this.session(), reference: this.reference, editMode: this.editMode,
          overlayVisible: true, demo: true,
        } satisfies Snapshot;
      case "inspect_lap_file":
        if (String(args.path).endsWith(".blap")) {
          return {
            trackName: "Demo Ring", trackConfig: "Grand Prix", carName: "GT3 Evo", driver: "Coach", kind: "blap",
            laps: [{ lap: 1, lapTime: 92.412, clean: true }],
          };
        }
        return {
          trackName: "Demo Ring", trackConfig: "Grand Prix", carName: "GT3 Evo", driver: "Coach", kind: "ibt",
          laps: [
            { lap: 3, lapTime: 92.884, clean: true },
            { lap: 4, lapTime: 92.412, clean: true },
            { lap: 5, lapTime: 95.106, clean: false },
            { lap: 6, lapTime: 92.655, clean: true },
          ],
        };
      case "load_reference":
        this.reference = {
          loaded: true, trackName: "Demo Ring", trackConfig: "Grand Prix", carName: "GT3 Evo", driver: "Coach",
          lapTime: [0, 92.412, 0, 92.884, 92.412, 95.106, 92.655][Number(args.lap)] ?? 92.4, source: String(args.path),
          matchesTrack: true, matchesCar: true, kind: String(args.path).endsWith(".ibt") ? "ibt" : "blap",
          hasSteering: String(args.path).endsWith(".ibt"),
        };
        this.broadcast("reference", this.reference);
        return;
      case "clear_reference":
        this.reference = { ...this.reference, loaded: false, matchesTrack: false };
        this.broadcast("reference", this.reference);
        return;
      case "set_edit_mode":
        this.editMode = Boolean(args.enabled);
        this.broadcast("edit-mode", this.editMode);
        return;
      case "set_overlay_visible":
        this.broadcast("overlay-visible", args.visible);
        return;
      case "set_demo":
      case "open_settings":
        return;
      default:
        throw new Error(`mock: unknown command ${cmd}`);
    }
  }

  private status(): StatusPayload {
    return { source: "demo", connected: true, message: "Browser preview" };
  }

  private session(): SessionPayload {
    return {
      trackName: "Demo Ring", trackConfig: "Grand Prix", trackLengthKm: TRACK_M / 1000, sessionType: "Race",
      playerCarIdx: PLAYER, playerClassId: CLASSES[1].id, carName: "GT3 Evo", fuelMaxLtr: 120, estLapTime: 92.1,
      shiftRpm: 8100, redlineRpm: 8600, multiclass: true, tireCompounds: ["Hard", "Wet"],
      sectors: [0, 0.12, 0.25, 0.4, 0.58, 0.72, 0.86],
    };
  }

  private start() {
    if (this.started) return;
    this.started = true;
    setTimeout(() => {
      this.dispatch("status", this.status());
      this.dispatch("session", this.session());
    }, 0);
    let last = performance.now();
    let fieldAt = 0;
    // ?speed=4 in the URL fast-forwards the fake race (handy for lap-based widgets).
    const speedUp = Math.max(1, Math.min(20, Number(new URLSearchParams(location.search).get("speed")) || 1));
    const loop = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000) * speedUp;
      last = now;
      this.step(dt);
      this.dispatch("tick", this.tick());
      if (now - fieldAt > 100) {
        fieldAt = now;
        this.dispatch("field", this.field());
      }
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  private speedFactor(pct: number) {
    let f = 1;
    for (const c of CORNERS) {
      let d = pct - c;
      d -= Math.round(d);
      f = Math.min(f, 0.35 + Math.min(1, Math.abs(d) * (d < 0 ? 22 : 9)) * 0.65);
    }
    return f;
  }

  private step(dt: number) {
    this.t += dt;
    for (let i = 0; i < N; i++) {
      if (BATTLE.some(([b]) => b === i)) continue;
      const before = Math.floor(this.progress[i]);
      this.progress[i] += (dt / this.pace[i]) * (this.speedFactor(this.progress[i] % 1) / 0.82);
      if (Math.floor(this.progress[i]) > before) {
        const lap = this.t - this.lapStart[i];
        if (this.lapStart[i] > 0) {
          this.last[i] = lap;
          this.best[i] = Math.min(this.best[i], lap);
        }
        this.lapStart[i] = this.t;
      }
    }
    for (const [i, base, swing, period] of BATTLE) {
      this.progress[i] = this.progress[PLAYER] + (base + swing * Math.sin((this.t * 2 * Math.PI) / period)) / TRACK_M;
    }
    this.fuel = Math.max(0, this.fuel - dt * (2.7 / 92));
  }

  private nearby(pct: number): { cars: NearbyCar[]; carLeftRight: number } {
    const cars: NearbyCar[] = [];
    for (let i = 0; i < N; i++) {
      if (i === PLAYER) continue;
      let d = (this.progress[i] % 1) - pct;
      d -= Math.round(d);
      const dist = d * TRACK_M;
      if (Math.abs(dist) <= 60) cars.push({ carIdx: i, dist, side: 0, classColor: CLASSES[classOf(i)].color });
    }
    // In the mock, even-numbered cars are on our left, odd on our right.
    let left = 0;
    let right = 0;
    for (const c of cars) {
      const wanted = c.carIdx % 2 ? 1 : -1;
      if (Math.abs(c.dist) < 5.5) {
        c.side = wanted as 1 | -1;
        if (wanted < 0) left++;
        else right++;
      } else if (Math.abs(c.dist) < 14 && this.sides.get(c.carIdx)) {
        c.side = this.sides.get(c.carIdx) as 1 | -1;
      }
    }
    this.sides = new Map(cars.filter((c) => c.side).map((c) => [c.carIdx, c.side]));
    const carLeftRight =
      left && right ? 4 : left > 1 ? 5 : right > 1 ? 6 : left ? 2 : right ? 3 : 1;
    return { cars, carLeftRight };
  }

  private tick(): TickPayload {
    const p = this.progress[PLAYER];
    const pct = p % 1;
    const f = this.speedFactor(pct);
    const ahead = this.speedFactor((pct + 0.006) % 1);
    const braking = ahead < f - 0.01;
    const speed = 78 * f;
    const g = Math.min(6, Math.max(1, Math.ceil(speed / 13)));
    const lapCurrent = pct * this.pace[PLAYER];
    const near = this.nearby(pct);
    const t: TickPayload = {
      sessionTime: this.t, isOnTrack: true, onPitRoad: false, speed,
      rpm: 4200 + ((speed % 13) / 13) * 4300, shiftRpm: 8400, gear: g,
      throttle: braking ? 0 : ahead > f + 0.005 || f > 0.98 ? 1 : 0.45 + Math.sin(this.t * 3) * 0.08,
      brake: braking ? Math.min(1, 0.6 + (f - ahead) * 8) : 0,
      clutch: 0, steering: Math.sin(pct * 40) * (1 - f) * 2,
      lap: Math.floor(p) + 1, lapDistPct: pct, lapCurrent,
      lapLast: this.last[PLAYER], lapBest: this.best[PLAYER], lapOptimal: this.best[PLAYER] - 0.412,
      deltas: this.deltas(pct, Math.floor(p)),
      fuelLevel: this.fuel, carLeftRight: near.carLeftRight, incidents: 2, nearby: near.cars,
      coach: null,
      dynamics: { latG: 0, longG: 0, vertG: 1, pitchDeg: 0, rollDeg: 0, slipDeg: 0, yawRateDeg: 0, balance: null },
      instruments: null as unknown as TickPayload["instruments"],
    };
    t.dynamics = this.dynamics(t, pct);
    t.instruments = this.instruments(t);
    t.coach = this.coach(t);
    return t;
  }

  /** Engine gauges that follow the revs; a water temperature warning comes on now and then. */
  private instruments(t: TickPayload): TickPayload["instruments"] {
    const load = (t.rpm - 4200) / 4300;
    const hot = this.t % 45 > 39;
    return {
      warnings: {
        waterTemp: hot, fuelPressure: false, oilPressure: false, oilTemp: false, stalled: false,
        pitLimiter: false, revLimiter: t.rpm > 8400, repairRequired: false, repairOptional: false,
        lowFuel: t.fuelLevel / 2.7 < 2,
      },
      oilTemp: 98 + 8 * load + Math.sin(this.t * 0.01) * 2,
      oilPress: 2.2 + 3 * load,
      waterTemp: hot ? 112 : 88 + 4 * load,
      fuelPress: 3.8,
      voltage: 13.8,
      brakeBias: this.t % 60 > 30 ? 54.0 : 54.5,
      brakeBiasFine: null,
      peakBrakeBias: null,
    };
  }

  /** Rough body physics so the Dynamics widget moves believably. */
  private dynamics(t: TickPayload, pct: number): TickPayload["dynamics"] {
    const G = 9.80665;
    const dt = t.sessionTime - this.dyn.time;
    const rawLong = dt > 0 && this.dyn.time > 0 ? (t.speed - this.dyn.speed) / dt / G : 0;
    this.dyn.longG += (Math.max(-2.2, Math.min(1.2, rawLong)) - this.dyn.longG) * 0.25;
    this.dyn.speed = t.speed;
    this.dyn.time = t.sessionTime;
    const balanceFactor = t.speed > 45 ? 0.82 : t.speed < 28 ? 1.12 : 1;
    const yaw = t.steering * t.speed * 0.016 * balanceFactor;
    const latG = (t.speed * yaw) / G;
    const hard = Math.abs(latG) > 0.5;
    const target = hard ? (balanceFactor - 1) / 0.4 : null;
    if (target != null) this.dyn.balance = (this.dyn.balance ?? target) + (target - (this.dyn.balance ?? target)) * 0.12;
    const kerb = ((pct * 37) % 1) < 0.02 ? 0.35 * Math.sin(t.sessionTime * 40) : 0;
    return {
      latG,
      longG: this.dyn.longG,
      vertG: 1 + 0.00006 * t.speed * t.speed + kerb,
      pitchDeg: (0.0012 * this.dyn.longG * G * 180) / Math.PI,
      rollDeg: (0.0016 * latG * G * 180) / Math.PI,
      slipDeg: (-(t.speed < 30 ? 0.035 : 0.012) * yaw * t.speed * 180) / Math.PI / Math.max(1, t.speed),
      yawRateDeg: (yaw * 180) / Math.PI,
      balance: hard ? this.dyn.balance : null,
    };
  }

  /** A rough coach: the "reference" brakes a little later and carries a bit more speed. */
  private coach(t: TickPayload): CoachPayload | null {
    if (!this.reference.loaded) return null;
    const pct = t.lapDistPct;
    const rp = (pct + 0.004) % 1;
    const rf = this.speedFactor(rp);
    const rAhead = this.speedFactor((rp + 0.006) % 1);
    const refBraking = rAhead < rf - 0.01;
    const refBrake = refBraking ? Math.min(1, 0.72 + (rf - rAhead) * 8) : 0;
    const refThrottle = refBraking ? 0 : rAhead > rf + 0.005 || rf > 0.98 ? 1 : 0.55;
    const refSpeed = 78 * rf * 1.012;
    const refGear = Math.min(6, Math.max(1, Math.ceil(refSpeed / 13)));
    // Only .ibt references carry steering.
    const refSteering = this.reference.hasSteering ? Math.sin(rp * 40) * (1 - rf) * 1.9 : null;

    const d = t.deltas.reference ?? 0;
    this.refDeltas.push([t.sessionTime, d]);
    while (this.refDeltas.length && t.sessionTime - this.refDeltas[0][0] > 2) this.refDeltas.shift();

    const make = (kind: Cue["kind"], tone: Cue["tone"], priority: number, extra: Partial<Cue> = {}) => ({
      cue: { kind, tone, distanceM: null, ours: null, reference: null, gear: null, corner: null, ...extra } as Cue,
      priority,
    });
    const options: { cue: Cue; priority: number }[] = [make("onPace", "neutral", 0)];
    const braking = t.brake > 0.1;
    if (refBrake > 0.1 && !braking) options.push(make("brakeNow", "bad", 9, { distanceM: -12 }));
    if (braking && refBrake < 0.05) options.push(make("brakeEarly", "warn", 8, { distanceM: 20 }));
    if (braking && refBrake > 0.1) {
      const diff = t.brake - refBrake;
      if (diff < -0.15) options.push(make("brakeHarder", "warn", 6, { ours: t.brake, reference: refBrake }));
      else if (diff > 0.15) options.push(make("brakeLighter", "warn", 6, { ours: t.brake, reference: refBrake }));
      else options.push(make("goodBraking", "good", 2, { reference: refBrake }));
    }
    if (!braking) {
      for (let k = 1; k <= 20; k++) {
        const a = (rp + k * 0.001) % 1;
        if (this.speedFactor((a + 0.006) % 1) < this.speedFactor(a) - 0.01) {
          if (refBrake < 0.05) options.push(make("brakeSoon", "info", 4, { distanceM: k * 5.1 }));
          break;
        }
      }
    }
    if (refThrottle > 0.6 && t.throttle < 0.3 && !braking) {
      options.push(make("throttleNow", "bad", 7, { ours: t.throttle, reference: refThrottle }));
    }
    if (t.gear !== refGear) {
      options.push(make(t.gear > refGear ? "shiftDown" : "shiftUp", "info", 3, { gear: refGear }));
    }
    const change = d - (this.refDeltas[0]?.[1] ?? d);
    if (change < -0.04) options.push(make("gaining", "good", 1, { ours: -change }));
    else if (change > 0.04) options.push(make("losing", "warn", 1, { ours: change }));

    const best = options.reduce((a, b) => (b.priority > a.priority ? b : a));
    const cur = this.cue;
    if (!cur || cur.cue.kind === best.cue.kind || best.priority > cur.priority || t.sessionTime - cur.since > 0.9) {
      this.cue = { ...best, since: cur && cur.cue.kind === best.cue.kind ? cur.since : t.sessionTime };
    }
    return {
      refSpeed, refThrottle, refBrake, refGear, refSteering,
      speedDelta: t.speed - refSpeed, cue: this.cue!.cue,
    };
  }

  /** Plausible delta curves: start at 0 on the line, wander, finish at an offset. */
  private deltas(pct: number, lap: number): Deltas {
    const curve = (end: number, phase: number) =>
      end * pct + 0.28 * Math.sin(Math.PI * pct) * Math.sin(2 * Math.PI * 2.3 * pct + lap * 1.7 + phase);
    return {
      best: curve(0.35, 0),
      optimal: curve(0.62, 0.8),
      sessionBest: curve(0.08, 1.9),
      sessionOptimal: curve(0.27, 2.6),
      last: curve(-0.21, 3.4),
      reference: this.reference.loaded ? curve(0.44, 4.1) : null,
    };
  }

  private driver(i: number) {
    const lic = ["A", "B", "C"][Math.floor(rand(i + 13) * 3)];
    const c = CLASSES[classOf(i)];
    return {
      carIdx: i, name: NAMES[i], abbrev: NAMES[i].split(" ").pop() ?? "", number: String(3 + ((i * 7) % 97)),
      classId: c.id, className: c.name, classColor: c.color,
      irating: 1200 + Math.floor(rand(i + 11) * 3800), license: `${lic} ${(1.5 + rand(i + 17) * 2.9).toFixed(2)}`,
      licenseColor: lic === "A" ? "#0153db" : lic === "B" ? "#00c702" : "#feec04",
    };
  }

  private laps(i: number, classBest: number[]) {
    const lastFlag: LapFlag =
      Math.abs(this.last[i] - this.best[i]) < 0.0005
        ? Math.abs(this.best[i] - classBest[classOf(i)]) < 0.0005 ? "fastest" : "personal"
        : "none";
    return {
      lastLap: this.last[i], bestLap: this.best[i], lastFlag,
      classFastest: Math.abs(this.best[i] - classBest[classOf(i)]) < 0.0005,
    };
  }

  private field(): FieldPayload {
    const order = [...Array(N).keys()].sort((a, b) => this.progress[b] - this.progress[a]);
    const classBest = CLASSES.map((_, c) => Math.min(...this.best.filter((_, i) => classOf(i) === c)));
    const pos = new Map<number, number>();
    const classPos = new Map<number, number>();
    const counts = [0, 0, 0];
    order.forEach((i, k) => {
      pos.set(i, k + 1);
      classPos.set(i, ++counts[classOf(i)]);
    });
    const me = this.progress[PLAYER];
    const relative: RelativeRow[] = order
      .map((i) => {
        let d = (this.progress[i] % 1) - (me % 1);
        d -= Math.round(d);
        const laps = Math.round(this.progress[i] - me - d);
        return {
          ...this.driver(i), ...this.laps(i, classBest), position: pos.get(i)!, classPosition: classPos.get(i)!,
          gap: d * 92.1, lapStatus: Math.max(-1, Math.min(1, laps)) as -1 | 0 | 1, onPitRoad: i === 20,
          isPlayer: i === PLAYER,
        };
      })
      .sort((a, b) => b.gap - a.gap);

    const leaders = new Map<number, number>();
    const prev = new Map<number, number>();
    const standings: StandingRow[] = order.map((i) => {
      const c = classOf(i);
      if (!leaders.has(c)) leaders.set(c, i);
      const lead = leaders.get(c)!;
      const ahead = prev.get(c);
      prev.set(c, i);
      return {
        ...this.driver(i), ...this.laps(i, classBest), position: pos.get(i)!, classPosition: classPos.get(i)!,
        lap: Math.floor(this.progress[i]),
        gap: lead === i ? null : (this.progress[lead] - this.progress[i]) * this.pace[i],
        lapsDown: Math.floor(this.progress[lead] - this.progress[i]),
        interval: ahead === undefined ? null : (this.progress[ahead] - this.progress[i]) * this.pace[i],
        onPitRoad: i === 20, isPlayer: i === PLAYER,
      };
    });

    const classes = CLASSES.map((c, k) => {
      const cars = [...Array(N).keys()].filter((i) => classOf(i) === k);
      return {
        classId: c.id, name: c.name, color: c.color, cars: cars.length,
        strengthOfField: strengthOfField(cars.map((i) => this.driver(i).irating)),
      };
    });

    const perLap = 2.7;
    const lapsToGo = 30 - Math.floor(this.progress[order[0]]) - 1 + (1 - (me % 1));
    const tires: TiresPayload = {
      corners: [
        { temp: [78, 82.5, 86], wear: [0.94, 0.93, 0.91], coldPressure: 165 },
        { temp: [91, 88.5, 84], wear: [0.86, 0.88, 0.9], coldPressure: 165 },
        { temp: [80.5, 83, 85.5], wear: [0.95, 0.94, 0.94], coldPressure: 162 },
        { temp: [89, 87, 83.5], wear: [0.89, 0.9, 0.92], coldPressure: 162 },
      ],
      compound: "Hard", setsUsed: 2, setsAvailable: 3, live: false, hasData: true,
    };
    return {
      clock: {
        sessionType: "Race", timeRemain: null, lapsRemain: 30 - Math.floor(this.progress[order[0]]), totalLaps: 30,
        flags: 0, trackTemp: 31.4, airTemp: 22.8, strengthOfField: classes[1].strengthOfField, classes,
        timeOfDay: 14 * 3600 + (performance.now() / 1000) % 3600, skies: 1, trackWetness: 1, declaredWet: false,
        windSpeed: 3.4, windDir: 5.5, humidity: 0.56, incidents: 2, incidentLimit: 17, fastRepairsUsed: 0,
        fastRepairsLimit: 1,
      },
      relative,
      standings,
      fuel: {
        level: this.fuel, max: 120, perLap, lastLapUsed: 2.74, lapsOfFuel: this.fuel / perLap, lapsToGo,
        toFinish: lapsToGo * perLap - this.fuel, samples: 5,
      },
      tires,
    };
  }
}

export const mock = new Mock();
