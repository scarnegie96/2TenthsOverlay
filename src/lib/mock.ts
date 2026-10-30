// Browser-only stand-in for the Rust side. Generates a small fake race and
// keeps config in localStorage, syncing between tabs with BroadcastChannel
// (open /overlay.html and /settings.html side by side).
//
// The real maths lives in crates/telemetry; this only needs to look plausible.

import type {
  FieldPayload, RelativeRow, SessionPayload, Snapshot, StandingRow, StatusPayload, TickPayload,
} from "./types";

type Handler = (payload: unknown) => void;

const CONFIG_KEY = "tenths.config";
const LAP = 92;
const N = 18;
const PLAYER = 6;
const NAMES = [
  "Lena Hoffmann", "Marco Bellini", "Tom Okafor", "Sakura Ito", "Jörg Lindqvist", "Diego Alvarez", "You",
  "Priya Raman", "Callum Reid", "Noah Fischer", "Elif Demir", "Mateus Costa", "Rafael Duarte", "Ines Moreau",
  "Kenji Sato", "Liam O'Brien", "Zoe Clarke", "Amara Nwosu",
];
const CORNERS = [0.06, 0.17, 0.31, 0.43, 0.52, 0.66, 0.79, 0.9];

const rand = (i: number) => {
  const x = Math.sin(i * 12.9898) * 43758.5453;
  return x - Math.floor(x);
};

class Mock {
  private handlers = new Map<string, Set<Handler>>();
  private channel = new BroadcastChannel("tenths");
  private editMode = false;
  private t = 0;
  private progress = Array.from({ length: N }, (_, i) => 7.6 - i * 0.03 - rand(i) * 0.01);
  private pace = Array.from({ length: N }, (_, i) => LAP + (i >= 12 ? 7 : 0) + rand(i + 3) * 2);
  private best = this.pace.map((p, i) => p - 0.4 + rand(i + 9) * 0.3);
  private fuel = 31.4;
  private started = false;

  constructor() {
    this.channel.onmessage = (e) => {
      const { event, payload } = e.data as { event: string; payload: unknown };
      if (event === "edit-mode") this.editMode = payload as boolean;
      this.dispatch(event, payload);
    };
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
          status: this.status(),
          session: this.session(),
          editMode: this.editMode,
          overlayVisible: true,
          demo: true,
        } satisfies Snapshot;
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
      trackName: "Demo Ring", trackConfig: "Grand Prix", trackLengthKm: 5.1, sessionType: "Race",
      playerCarIdx: PLAYER, playerClassId: 1, carName: "GT3 Evo", fuelMaxLtr: 120, estLapTime: LAP,
      shiftRpm: 8100, redlineRpm: 8600, multiclass: true,
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
    const loop = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000);
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
      const pct = this.progress[i] % 1;
      this.progress[i] += (dt / this.pace[i]) * (this.speedFactor(pct) / 0.82);
    }
    this.fuel = Math.max(0, this.fuel - dt * (2.7 / LAP));
  }

  private tick(): TickPayload {
    const p = this.progress[PLAYER];
    const pct = p % 1;
    const f = this.speedFactor(pct);
    const ahead = this.speedFactor((pct + 0.006) % 1);
    const braking = ahead < f - 0.01;
    const speed = 78 * f;
    const g = Math.min(6, Math.max(1, Math.ceil(speed / 13)));
    let side: number = 1;
    for (let i = 0; i < N; i++) {
      if (i === PLAYER) continue;
      let d = (this.progress[i] % 1) - pct;
      d -= Math.round(d);
      if (Math.abs(d) * 5100 < 6) side = i % 2 ? 3 : 2;
    }
    const lapCurrent = pct * this.pace[PLAYER];
    return {
      sessionTime: this.t, isOnTrack: true, onPitRoad: false, speed,
      rpm: 4200 + ((speed % 13) / 13) * 4300, gear: g,
      throttle: braking ? 0 : ahead > f + 0.005 || f > 0.98 ? 1 : 0.45,
      brake: braking ? Math.min(1, 0.6 + (f - ahead) * 8) : 0,
      clutch: 0, steering: Math.sin(pct * 40) * (1 - f) * 2,
      lap: Math.floor(p) + 1, lapDistPct: pct, lapCurrent,
      lapLast: this.best[PLAYER] + 0.6, lapBest: this.best[PLAYER],
      deltaBest: lapCurrent - pct * this.best[PLAYER] + Math.sin(this.t / 7) * 0.25,
      fuelLevel: this.fuel, carLeftRight: side, incidents: 2,
    };
  }

  private driver(i: number) {
    const gt4 = i >= 12;
    const lic = ["A", "B", "C"][Math.floor(rand(i + 13) * 3)];
    return {
      carIdx: i, name: NAMES[i], abbrev: NAMES[i].split(" ").pop() ?? "", number: String(3 + ((i * 7) % 97)),
      classId: gt4 ? 2 : 1, className: gt4 ? "GT4" : "GT3", classColor: gt4 ? "#33ceff" : "#ffda59",
      irating: 1200 + Math.floor(rand(i + 11) * 3200), license: `${lic} ${(1.5 + rand(i + 17) * 2.9).toFixed(2)}`,
      licenseColor: lic === "A" ? "#0153db" : lic === "B" ? "#00c702" : "#feec04",
    };
  }

  private field(): FieldPayload {
    const order = [...Array(N).keys()].sort((a, b) => this.progress[b] - this.progress[a]);
    const pos = new Map<number, number>();
    const classPos = new Map<number, number>();
    const counts = [0, 0];
    order.forEach((i, k) => {
      pos.set(i, k + 1);
      const c = i >= 12 ? 1 : 0;
      classPos.set(i, ++counts[c]);
    });
    const me = this.progress[PLAYER];
    const relative: RelativeRow[] = order.map((i) => {
      let d = (this.progress[i] % 1) - (me % 1);
      d -= Math.round(d);
      const laps = Math.round(this.progress[i] - me - d);
      return {
        ...this.driver(i), position: pos.get(i)!, classPosition: classPos.get(i)!, gap: d * LAP,
        lapStatus: Math.max(-1, Math.min(1, laps)) as -1 | 0 | 1, onPitRoad: false, isPlayer: i === PLAYER,
      };
    }).sort((a, b) => b.gap - a.gap);

    const leaders = new Map<number, number>();
    const prev = new Map<number, number>();
    const standings: StandingRow[] = order.map((i) => {
      const c = i >= 12 ? 1 : 0;
      if (!leaders.has(c)) leaders.set(c, i);
      const lead = leaders.get(c)!;
      const ahead = prev.get(c);
      prev.set(c, i);
      const lapsDown = Math.floor(this.progress[lead] - this.progress[i]);
      return {
        ...this.driver(i), position: pos.get(i)!, classPosition: classPos.get(i)!, lap: Math.floor(this.progress[i]),
        gap: lead === i ? null : (this.progress[lead] - this.progress[i]) * this.pace[i], lapsDown,
        interval: ahead === undefined ? null : (this.progress[ahead] - this.progress[i]) * this.pace[i],
        lastLap: this.best[i] + 0.3 + rand(i + Math.floor(this.progress[i])) * 0.8, bestLap: this.best[i],
        classFastest: i === (c ? 13 : 3), onPitRoad: false, isPlayer: i === PLAYER,
      };
    });

    const perLap = 2.7;
    const lapsToGo = 30 - Math.floor(this.progress[order[0]]) - 1 + (1 - (me % 1));
    return {
      clock: {
        sessionType: "Race", timeRemain: null, lapsRemain: 30 - Math.floor(this.progress[order[0]]), totalLaps: 30,
        flags: 0, trackTemp: 31.4, airTemp: 22.8, strengthOfField: 2687,
      },
      relative,
      standings,
      fuel: {
        level: this.fuel, max: 120, perLap, lastLapUsed: 2.74, lapsOfFuel: this.fuel / perLap, lapsToGo,
        toFinish: lapsToGo * perLap - this.fuel, samples: 5,
      },
    };
  }
}

export const mock = new Mock();
