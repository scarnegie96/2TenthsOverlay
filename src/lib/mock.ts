// Browser-only stand-in for the Rust side. Generates a small fake race and
// keeps config in localStorage, syncing between tabs with BroadcastChannel
// (open /overlay.html and /settings.html side by side).
//
// The real maths lives in crates/telemetry; this only needs to look plausible.

import type { RaceReport } from "./report";
import type {
  CarStatus, CarStints, CoachPayload, Display, Stint, Cue, Deltas, FieldPayload, LapFlag, LapRecord, PitCommand, PitPayload, Profile, ProfileSummary, ReferencePayload, NearbyCar, RelativeRow, SessionPayload, Snapshot, StandingRow,
  StatusPayload, TickPayload, TiresPayload, UpdateInfo,
} from "./types";

type Handler = (payload: unknown) => void;

const CONFIG_KEY = "tenths.config";
/** ?sample: a few saved layouts, with Sprint on screen, for the README screenshots. */
const SAMPLE = typeof location !== "undefined" && new URLSearchParams(location.search).has("sample");
const FINISHED = typeof location !== "undefined" && new URLSearchParams(location.search).has("finished");
const RATINGS_KEY = "tenths.driverRatings";
const SHOWCASE = typeof location !== "undefined" && new URLSearchParams(location.search).has("all");
const SHOW_PITS = typeof location !== "undefined" && new URLSearchParams(location.search).has("pits");

/**
 * Every widget on at once, laid out by hand for a 2560x1440 screen at 100%,
 * for the README's overview pictures. Positions are from the top-left corner.
 */
const SHOWCASE_LAYOUT: Record<string, [number, number]> = {
  standings: [24, 24], relative: [24, 604], dynamics: [24, 846], inputs: [24, 1302],
  delta: [568, 24], racepace: [568, 258], session: [1132, 258],
  coach: [568, 612], flags: [990, 612], fuel: [1170, 612],
  laplog: [568, 836], radar: [892, 836], instruments: [760, 1213],
  stints: [1468, 24], tires: [1468, 716], pitstop: [1772, 716], weather: [2132, 24],
  battle: [2132, 476], twitch: [2132, 740],
};
/** Options the showcase needs: a channel for Twitch chat (the overview adds ?chat, so it's made up, not live). */
const SHOWCASE_OPTIONS: Record<string, Record<string, string | number | boolean>> = {
  twitch: { channel: "tenths_racing", fadeAfter: 0, rows: 8 },
};

function showcaseWidgets() {
  return Object.fromEntries(
    Object.entries(SHOWCASE_LAYOUT).map(([id, [x, y]]) => [
      id,
      { enabled: true, x, y, ax: "l", ay: "t", scale: 1, opacity: 1, options: SHOWCASE_OPTIONS[id] ?? {} },
    ]),
  );
}
const SAMPLE_LAYOUTS: Record<string, Profile> = Object.fromEntries(
  [["sprint", "Sprint", 2], ["endurance", "Endurance", 9], ["streaming", "Streaming", 30]].map(([id, name, days]) => [
    id,
    { format: "tenths-layout", version: 1, name, updatedAt: Math.floor(Date.now() / 1000) - Number(days) * 86400, widgets: {} },
  ]),
) as Record<string, Profile>;
const LAYOUTS_KEY = "tenths.layouts";
const TRACK_M = 5100;
const PLAYER = 6;
const NAMES = [
  "Lena Hoffmann", "Marco Bellini", "Tom Okafor", "Sakura Ito", "Jörg Lindqvist", "Diego Alvarez", "You",
  "Priya Raman", "Callum Reid", "Noah Fischer", "Elif Demir", "Mateus Costa", "Owen Walsh", "Hannah Kowalski",
  "Rafael Duarte", "Ines Moreau", "Kenji Sato", "Liam O'Brien", "Zoe Clarke", "Pieter de Vries", "Amara Nwosu",
];
const N = NAMES.length;
/** Retired on lap 2: out of the sim, but still in the standings. */
const RETIRED = 13;
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
  /** What's ticked for the next stop: four tires and some fuel, like the Rust demo. */
  private pit: PitPayload = {
    available: true, tires: [true, true, true, true], pressures: [165, 165, 162, 162], compound: "Hard", fuel: true,
    fuelAmount: 30, tearoff: false, fastRepair: false, fastRepairsLeft: 1, status: 0, inPitStall: false,
  };
  private lapLog: LapRecord[] = [];
  private dyn = { speed: 0, time: 0, longG: 0, balance: null as number | null };
  private started = false;
  private reference: ReferencePayload = {
    loaded: false, trackName: "", trackConfig: "", carName: "", driver: "", lapTime: 0, source: "", matchesTrack: false,
    matchesCar: true, kind: "blap", hasSteering: false,
  };
  private cue: { cue: Cue; since: number; priority: number } | null = null;
  private refDeltas: [number, number][] = [];

  constructor() {
    // ?reference starts with an imported .ibt lap, for the Coach and the reference inputs.
    if (new URLSearchParams(location.search).has("reference")) {
      this.reference = {
        loaded: true, trackName: "Demo Ring", trackConfig: "Grand Prix", carName: "GT3 Evo", driver: "Coach", lapTime: 92.412,
        source: "demo.ibt", matchesTrack: true, matchesCar: true, kind: "ibt", hasSteering: true,
      };
    }
    this.channel.onmessage = (e) => {
      const { event, payload } = e.data as { event: string; payload: unknown };
      if (event === "edit-mode") this.editMode = payload as boolean;
      if (event === "reference") this.reference = payload as ReferencePayload;
      this.dispatch(event, payload);
    };
    // A hidden tab stands in for closing the settings window (lib.rs sends "settings-hidden").
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) this.dispatch("settings-hidden", null);
    });
    for (const [i, base] of BATTLE) this.progress[i] = this.progress[PLAYER] + base / TRACK_M;
    this.progress[RETIRED] = 1.55;
    // Give a few drivers a fresh personal best / class best so the colors show.
    this.last[2] = this.best[2];
    this.last[PLAYER] = this.best[PLAYER];
    // Some laps already driven, with an out lap and a spin. The personal best is from an earlier session.
    const b = this.best[PLAYER];
    const now = Math.floor(Date.now() / 1000);
    const seed: [number | null, Partial<LapRecord>][] = [
      [b + 21.6, { pit: true }], [b + 0.82, {}], [b + 0.31, {}], [b + 3.9, { offTrack: true, incidents: 2 }],
      [b + 0.46, {}], [b, {}], [b + 0.24, {}],
    ];
    this.lapLog = seed.map(([time, extra], i) => ({
      lap: i + 1, time, clean: !extra.pit && !extra.offTrack, pit: false, offTrack: false, incidents: 0,
      fuelUsed: extra.pit ? null : 2.7 + rand(i + 40) * 0.12, trackTemp: 31.9 - i * 0.1, airTemp: 22.8,
      personalBest: false, at: now - (seed.length - i) * 95, ...extra,
    }));
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

  private update: UpdateInfo | null = null;
  private deletedReports = new Set<string>();
  private updateChecked: number | null = null;

  async call(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
    switch (cmd) {
      case "load_config": {
        try {
          // ?all: every widget on, for the overview screenshots on a 2560x1440 screen.
          if (SHOWCASE) return { version: 1, general: {}, widgets: showcaseWidgets() };
          const raw = localStorage.getItem(CONFIG_KEY);
          if (!raw && SAMPLE) return { version: 1, general: { activeProfile: "sprint" }, widgets: {} };
          return raw ? JSON.parse(raw) : null;
        } catch {
          return null;
        }
      }
      case "config_notice":
        // ?corrupt shows the notice for a settings file that couldn't be read.
        return new URLSearchParams(location.search).has("corrupt") ? "config.corrupt-20261031-142501.json" : null;
      case "save_config":
        try {
          localStorage.setItem(CONFIG_KEY, JSON.stringify(args.config));
        } catch {
          /* storage unavailable; keep in memory only */
        }
        this.broadcast("config-changed", args.config);
        return;
      case "list_profiles":
        return this.layoutList();
      case "get_profile": {
        const p = this.layouts()[String(args.id)];
        if (!p) throw new Error("That layout no longer exists.");
        return p;
      }
      case "save_profile": {
        const all = this.layouts();
        const existing = args.id ? all[String(args.id)] : undefined;
        if (args.id && !existing) throw new Error("That layout no longer exists.");
        const name = String(args.name ?? "").trim() || existing?.name;
        if (!name) throw new Error("Give the layout a name.");
        const id = existing ? String(args.id) : this.newLayoutId(name, all);
        all[id] = { format: "tenths-layout", version: 1, name, updatedAt: Math.floor(Date.now() / 1000), widgets: args.widgets };
        this.saveLayouts(all);
        return id;
      }
      case "delete_profile": {
        const all = this.layouts();
        delete all[String(args.id)];
        this.saveLayouts(all);
        return;
      }
      case "import_profile_text": {
        let p: Profile;
        try {
          p = JSON.parse(String(args.text));
        } catch {
          throw new Error("That isn't a Tenths layout file.");
        }
        if (p?.format !== "tenths-layout" || typeof p.widgets !== "object") throw new Error("That isn't a Tenths layout file.");
        const all = this.layouts();
        const id = this.newLayoutId(p.name, all);
        all[id] = { ...p, updatedAt: Math.floor(Date.now() / 1000) };
        this.saveLayouts(all);
        return id;
      }
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
      case "pit_command":
        this.applyPit(args.action as PitCommand);
        return;
      case "set_demo":
      case "open_settings":
      case "set_pit_active":
      case "set_controls":
      case "suspend_controls":
        return;
      case "list_controllers":
        return [];
      case "pointer":
        return null;
      case "set_clickable":
        return;
      case "stream_port":
        return null;
      // A real report from the Rust demo (report_demo example), for the Races page.
      case "list_reports": {
        const r = (await import("./fixtures/demo-report.json")).default as unknown as RaceReport;
        if (this.deletedReports.has(r.id)) return [];
        const me = r.cars.find((c) => c.isPlayer);
        return [{
          id: r.id, startedAt: r.startedAt, track: r.track, trackConfig: r.trackConfig, car: r.car,
          sessionType: r.sessionType, official: r.official, finished: r.finished,
          classPosition: me?.finishClassPosition ?? 0, classCars: r.cars.filter((c) => c.classId === r.playerClassId).length,
          laps: me?.lapsCompleted ?? 0,
        }];
      }
      case "get_report":
        return (await import("./fixtures/demo-report.json")).default;
      case "delete_report":
        this.deletedReports.add(String(args.id));
        return;
      case "get_driver_ratings":
        try {
          return JSON.parse(localStorage.getItem(RATINGS_KEY) ?? "{}");
        } catch {
          return {};
        }
      case "set_driver_rating": {
        const all = (await this.call("get_driver_ratings")) as Record<string, unknown>;
        const key = String(args.userId);
        if (args.rating) all[key] = { name: args.name, rating: args.rating, at: Math.floor(Date.now() / 1000) };
        else delete all[key];
        localStorage.setItem(RATINGS_KEY, JSON.stringify(all));
        this.broadcast("driver-ratings", all);
        return;
      }
      // ?update pretends a newer version is out.
      case "update_status":
        return { current: "0.4.1", available: this.update, lastChecked: this.updateChecked, enabled: true };
      case "check_update": {
        await new Promise((r) => setTimeout(r, 700));
        this.updateChecked = Math.floor(Date.now() / 1000);
        this.update = new URLSearchParams(location.search).has("update") ? PREVIEW_UPDATE : null;
        this.dispatch("update-status", await this.call("update_status"));
        return this.update;
      }
      case "install_update": {
        if (!this.update) throw new Error("There's no update to install. Check again first.");
        const total = 9_400_000;
        for (let d = 0; d <= total; d += 470_000) {
          this.dispatch("update-progress", { downloaded: d, total });
          await new Promise((r) => setTimeout(r, 120));
        }
        // The real app closes and the installer reopens it.
        location.reload();
        return;
      }
      case "get_displays":
        return previewDisplays();
      case "my_display": {
        // ?display=2 pretends this tab is the overlay on the second display.
        const n = Number(new URLSearchParams(location.search).get("display") || 1);
        return previewDisplays()[n - 1]?.key ?? previewDisplays()[0].key;
      }
      default:
        throw new Error(`mock: unknown command ${cmd}`);
    }
  }

  /** What iRacing does with a pit command (the Rust demo does the same in pit.rs). */
  private applyPit(c: PitCommand) {
    const p = { ...this.pit, tires: [...this.pit.tires] as PitPayload["tires"], pressures: [...this.pit.pressures] as PitPayload["pressures"] };
    switch (c.kind) {
      case "clear": Object.assign(p, { tires: [false, false, false, false], fuel: false, tearoff: false, fastRepair: false }); break;
      case "fuel": p.fuel = true; if (c.litres > 0) p.fuelAmount = c.litres; break;
      case "clearFuel": p.fuel = false; break;
      case "tire": p.tires[c.corner] = true; if (c.kpa > 0) p.pressures[c.corner] = c.kpa; break;
      case "clearTires": p.tires = [false, false, false, false]; break;
      case "tearoff": p.tearoff = true; break;
      case "clearTearoff": p.tearoff = false; break;
      case "fastRepair": p.fastRepair = true; break;
      case "clearFastRepair": p.fastRepair = false; break;
      case "compound": break;
    }
    this.pit = p;
  }

  private layouts(): Record<string, Profile> {
    try {
      const saved = localStorage.getItem(LAYOUTS_KEY);
      return saved ? JSON.parse(saved) : SAMPLE ? SAMPLE_LAYOUTS : {};
    } catch {
      return {};
    }
  }

  private layoutList(): ProfileSummary[] {
    return Object.entries(this.layouts())
      .map(([id, p]) => ({ id, name: p.name, updatedAt: p.updatedAt }))
      .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  }

  private saveLayouts(all: Record<string, Profile>) {
    try {
      localStorage.setItem(LAYOUTS_KEY, JSON.stringify(all));
    } catch {
      /* storage unavailable */
    }
    this.broadcast("profiles-changed", this.layoutList());
  }

  private newLayoutId(name: string, all: Record<string, Profile>) {
    const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 40) || "layout";
    let id = slug;
    for (let n = 2; all[id]; n++) id = `${slug}-${n}`;
    return id;
  }

  private status(): StatusPayload {
    return { source: "demo", connected: true, message: "Browser preview" };
  }

  private session(): SessionPayload {
    return {
      trackName: "Demo Ring", trackConfig: "Grand Prix", trackLengthKm: TRACK_M / 1000, sessionType: "Race",
      playerCarIdx: PLAYER, playerClassId: CLASSES[1].id, carName: "GT3 Evo", fuelMaxLtr: 120, estLapTime: 92.1,
      shiftRpm: 8100, redlineRpm: 8600, multiclass: true, official: true, tireCompounds: ["Hard", "Wet"],
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
    // ?warmup=100 races that many seconds before the first frame, so graphs and traces are full (screenshots).
    const warmup = Math.min(600, Number(new URLSearchParams(location.search).get("warmup")) || 0);
    if (warmup > 0) {
      // After the widgets mount, so the canvases record it too. A timer, not a frame:
      // headless screenshots run on virtual time, where frames are rare.
      setTimeout(() => {
        for (let i = 0; i < warmup * 30; i++) {
          this.step(1 / 30);
          this.dispatch("tick", this.tick());
          if (i % 3 === 0) this.dispatch("field", this.field());
        }
      }, 500);
    }
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

  /** Share of a lap's time spent getting to each point (1000 steps), for time gaps. */
  private cumTime: Float32Array | null = null;
  private slowness = 0;
  private timeAt(progress: number) {
    if (!this.cumTime) {
      const c = new Float32Array(1001);
      for (let k = 1; k <= 1000; k++) c[k] = c[k - 1] + 1 / this.speedFactor((k - 0.5) / 1000);
      this.slowness = c[1000] / 1000;
      for (let k = 0; k <= 1000; k++) c[k] /= c[1000];
      this.cumTime = c;
    }
    return Math.floor(progress) + this.cumTime[Math.round((progress % 1) * 1000)];
  }

  /** Average of 1 / speed factor round the lap: what makes a lap last exactly its pace. */
  private meanSlowness() {
    if (!this.cumTime) this.timeAt(0);
    return this.slowness;
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

  /** One car in the pit lane, one stopped in its box, one retired. */
  private carStatus(i: number): CarStatus {
    if (i === RETIRED) return "out";
    if (i === 21) return "pitStall";
    // ?pits: one GT3 car (Owen Walsh) is always in the pit lane, so screenshots show the pit symbol.
    if (SHOW_PITS && i === 12) return "pitLane";
    return inPitWindow(i, this.progress[i]) ? "pitLane" : "track";
  }

  private step(dt: number) {
    this.t += dt;
    for (let i = 0; i < N; i++) {
      if (BATTLE.some(([b]) => b === i) || i === RETIRED || i === 21) continue;
      const before = Math.floor(this.progress[i]);
      // Speed follows the corners; scaled so a whole lap takes the car's lap time (pace).
      this.progress[i] += (dt / this.pace[i]) * this.speedFactor(this.progress[i] % 1) * this.meanSlowness();
      if (Math.floor(this.progress[i]) > before) {
        const lap = this.t - this.lapStart[i];
        if (this.lapStart[i] > 0) {
          if (i === PLAYER) {
            this.lapLog.push({
              lap: this.lapLog.length + 1, time: lap, clean: true, pit: false, offTrack: false, incidents: 0,
              fuelUsed: 2.74, trackTemp: 31.4, airTemp: 22.8, personalBest: lap < this.best[i],
              at: Math.floor(Date.now() / 1000),
            });
          }
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
      // One clockwise lap of the compass per lap, swinging through the corners.
      heading: (((pct + 0.04 * Math.sin(pct * Math.PI * 16)) * Math.PI * 2) % (Math.PI * 2) + Math.PI * 2) % (Math.PI * 2),
    };
    t.dynamics = this.dynamics(t, pct);
    t.instruments = this.instruments(t);
    t.coach = this.coach(t);
    return t;
  }

  /** Weather that changes slowly: a light shower every 10 minutes, a veering wind. */
  private weather() {
    const cycle = Math.max(0, Math.sin((this.t / 600) * Math.PI * 2));
    return {
      skies: cycle > 0.5 ? 3 : cycle > 0.1 ? 2 : 1,
      trackWetness: cycle > 0.8 ? 3 : cycle > 0.4 ? 2 : 1,
      precipitation: 0.35 * cycle * cycle,
      fogLevel: 0,
      windSpeed: 3.4 + 1.2 * Math.abs(Math.sin(this.t / 37)),
      windDir: 5.5 + 0.4 * Math.sin(this.t / 240),
      humidity: 0.56 + 0.3 * cycle,
      trackTemp: 31.4 - 4 * cycle,
      airTemp: 22.8 - 2 * cycle,
    };
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
      cue: { kind, tone, distanceM: null, ours: null, reference: null, gear: null, corner: null, timeS: null, issue: null, ...extra } as Cue,
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
      focus: { corner: 3, lossS: 0.18, issue: "brakedEarly", passes: 4 },
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
      irating: 1200 + Math.floor(rand(i + 11) * 3800), userId: 400000 + i * 7919, license: `${lic} ${(1.5 + rand(i + 17) * 2.9).toFixed(2)}`,
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
    const stints = this.stints();
    const relative: RelativeRow[] = order
      // Cars out of the sim aren't on track, so they're not in the relative.
      .filter((i) => i !== RETIRED)
      .map((i) => {
        let d = (this.progress[i] % 1) - (me % 1);
        d -= Math.round(d);
        const laps = Math.round(this.progress[i] - me - d);
        return {
          ...this.driver(i), ...this.laps(i, classBest), position: pos.get(i)!, classPosition: classPos.get(i)!,
          gap: d * 92.1, lapStatus: Math.max(-1, Math.min(1, laps)) as -1 | 0 | 1, onPitRoad: this.carStatus(i) !== "track",
          status: this.carStatus(i),
          isPlayer: i === PLAYER,
          compound: stints[i]?.stints[stints[i].stints.length - 1]?.compound ?? 0,
        };
      })
      .sort((a, b) => b.gap - a.gap);

    // A starting grid close to, but not the same as, race pace.
    const grid = new Map<number, number>();
    for (let k = 0; k < CLASSES.length; k++) {
      [...Array(N).keys()]
        .filter((i) => classOf(i) === k)
        .sort((a, b) => a + (((a * 37) % 9) - 4) * 0.6 - (b + (((b * 37) % 9) - 4) * 0.6))
        .forEach((i, n) => grid.set(i, n + 1));
    }
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
        lapPct: this.progress[i] - Math.floor(this.progress[i]),
        // Time behind the leader (how long to reach where it is now), like iRacing's.
        gap: lead === i ? null : (this.timeAt(this.progress[lead]) - this.timeAt(this.progress[i])) * this.pace[i],
        lapsDown: Math.floor(this.progress[lead] - this.progress[i]),
        interval: ahead === undefined ? null : (this.timeAt(this.progress[ahead]) - this.timeAt(this.progress[i])) * this.pace[i],
        startPosition: grid.get(i) ?? null,
        onPitRoad: this.carStatus(i).startsWith("pit"), status: this.carStatus(i), isPlayer: i === PLAYER,
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
        flags: this.flags(), strengthOfField: classes[1].strengthOfField, classes,
        timeOfDay: 14 * 3600 + (performance.now() / 1000) % 3600, ...this.weather(), declaredWet: false,
        incidents: 2, incidentLimit: 17, fastRepairsUsed: 0,
        fastRepairsLimit: 1,
        // ?finished: you've taken the flag, so the Standings asks for ratings.
        playerFinished: FINISHED,
      },
      relative,
      standings,
      fuel: {
        level: this.fuel, max: 120, perLap, lastLapUsed: 2.74, lapsOfFuel: this.fuel / perLap, lapsToGo,
        toFinish: lapsToGo * perLap - this.fuel, samples: 5,
      },
      tires,
      pit: this.pit,
      stints,
      // As if a few stops have been timed in each class.
      pitLoss: CLASSES.map((c, k) => ({ classId: c.id, lossS: 26.4 + k * 1.3, stallS: 19 + k, stops: 4 + k })),
      lapLog: {
        laps: this.lapLog.slice(-100),
        sessionBest: Math.min(...this.lapLog.filter((l) => l.clean && l.time != null).map((l) => l.time!)),
        personalBest: { time: this.best[PLAYER] - 0.183, at: Math.floor(Date.now() / 1000) - 86400 * 12 },
        saving: false,
      },
    };
  }

  /**
   * Stints for every car, from a pit schedule like the Rust demo's: each car
   * stops at the line every few laps, and now and then comes out on wets.
   */
  private stints(): CarStints[] {
    const out: CarStints[] = [];
    for (let i = 0; i < N; i++) {
      const p = this.progress[i];
      if (i === RETIRED) {
        out.push({ carIdx: i, stops: 0, inPits: false, out: true,
          stints: [{ start: 0, end: p, open: false, endedBy: "out", compound: 0, partial: false }] });
        continue;
      }
      const every = stintLaps(i);
      const stints: Stint[] = [];
      let start = 0;
      let stops = 0;
      const wet = (k: number) => (k > 0 && ((i * 7 + k) * 13) % 10 > 6 ? 1 : 0);
      if (every) {
        for (let lap = every; lap - 0.035 < p; lap += every) {
          stints.push({ start, end: Math.min(p, lap - 0.035), open: false, endedBy: "pit", compound: wet(stops), partial: false });
          stops++;
          start = lap + 0.03;
        }
      }
      const inPits = i === 21 || inPitWindow(i, p);
      if (!inPits) stints.push({ start, end: p, open: true, endedBy: null, compound: wet(stops), partial: false });
      out.push({ carIdx: i, stints, stops, inPits, out: false });
    }
    return out;
  }

  /** A local yellow and a blue flag now and then, like the Rust demo; F cycles through every flag. */
  private flags(): number {
    if (flagPreview != null) return flagPreview;
    const t = this.t % 70;
    if (t >= 20 && t < 27) return 0x4 | 0x8 | 0x100;
    if (t >= 50 && t < 55) return 0x4 | 0x20;
    return 0x4;
  }
}

/** Browser preview: press F on the overlay to step through flags, or start on one with ?flag=<bits>. */
let flagPreview: number | null =
  typeof location !== "undefined" && new URLSearchParams(location.search).get("flag")
    ? Number(new URLSearchParams(location.search).get("flag"))
    : null;
const PREVIEW_FLAGS = [0x8000 | 0x4000, 0x100 | 0x8, 0x20, 0x40, 0x2, 0x1, 0x10, 0x10000, 0x100000, 0x80000, 0x20000, 0x200, 0x4];
if (typeof window !== "undefined") {
  window.addEventListener("keydown", (e) => {
    // Shift+F is the pit stop's fuel key.
    if (e.key.toLowerCase() !== "f" || e.ctrlKey || e.metaKey || e.shiftKey) return;
    const i = flagPreview == null ? 0 : PREVIEW_FLAGS.indexOf(flagPreview) + 1;
    flagPreview = i < PREVIEW_FLAGS.length ? PREVIEW_FLAGS[i] : null;
  });
}

export const mock = new Mock();

/**
 * Browser preview displays: one, or with ?displays=2 a second screen to the
 * right at 150% scaling, to try moving widgets between screens (open
 * overlay.html?displays=2 and overlay.html?displays=2&display=2 side by side).
 */
function previewDisplays(): Display[] {
  const two = new URLSearchParams(location.search).get("displays") === "2";
  const main: Display = { key: "\\\\.\\DISPLAY1", number: 1, primary: true, x: 0, y: 0, width: 1920, height: 1080, scale: 1 };
  if (!two) return [main];
  return [main, { key: "\\\\.\\DISPLAY2", number: 2, primary: false, x: 1920, y: 0, width: 3840, height: 2160, scale: 1.5 }];
}

/** Laps between a preview car's pit stops (0: it doesn't stop), like the Rust demo. */
function stintLaps(i: number): number {
  return i === PLAYER || i === 21 || i === RETIRED || BATTLE.some(([b]) => b === i) ? 0 : 3 + Math.floor((((i * 41) % 17) / 17) * 4);
}

/** In the pit lane at this race progress: just before or after the line, at the end of a stint. */
function inPitWindow(i: number, progress: number): boolean {
  const every = stintLaps(i);
  if (!every) return false;
  const lap = Math.floor(progress);
  const pct = progress - lap;
  return (pct > 0.965 && lap > 0 && (lap + 1) % every === 0) || (pct < 0.03 && lap > 0 && lap % every === 0);
}

/** The pretend newer version for `?update`. */
const PREVIEW_UPDATE: UpdateInfo = {
  version: "0.5.0",
  notes: "- Track map widget\n- Fuel to finish confirmed for timed races\n- Fixes for identical button boxes",
  published: Math.floor(Date.now() / 1000) - 2 * 86400,
};
