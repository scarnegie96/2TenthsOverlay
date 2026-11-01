import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, isTauri } from "../lib/bridge";
import { lapTime } from "../lib/format";
import { gapsByLap, lapTimes, positionsByLap, stats, type RaceReport, type ReportCar, type ReportSummary } from "../lib/report";
import { compoundKind } from "../widgets/Stints";

// Settings → Races: every saved race report, and one race in full. Only
// recorded data is shown: crossing times, lap times between them, the order
// across the line, pit stops and stints as timed, and your laps as logged.

export function RacesPane() {
  const [list, setList] = useState<ReportSummary[] | null>(null);
  const [open, setOpen] = useState<string | null>(() => (!isTauri && new URLSearchParams(location.search).get("report")) || null);
  const [error, setError] = useState<string | null>(null);

  const load = () => void api.listReports().then(setList).catch((e) => setError(String(e)));
  useEffect(load, []);

  if (open) return <ReportView id={open} onBack={() => setOpen(null)} />;
  return (
    <>
      <h1>Races</h1>
      <p className="lede">
        A report of every race you drive is saved when you take the flag (or when the session ends if you don't): everyone's
        laps, gaps and positions, tyre stints and pit stops, and your own fuel and laps. Your inputs aren't kept.
      </p>
      {error && <p className="notice"><span>{error}</span></p>}
      {list == null ? null : list.length === 0 ? (
        <p className="note">No races yet. Finish a race and it shows up here.</p>
      ) : (
        <div className="rp-list">
          {list.map((r) => (
            <div key={r.id} className="rp-item">
              <button className="rp-open" onClick={() => setOpen(r.id)}>
                <span className="rp-when">{new Date(r.startedAt * 1000).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}</span>
                <b>{r.track}{r.trackConfig ? ` · ${r.trackConfig}` : ""}</b>
                <span className="rp-car">{r.car}</span>
                <span className="rp-result">
                  {r.finished ? `P${r.classPosition} of ${r.classCars} in class` : "Didn't finish"} · {r.laps} laps
                  {r.official ? "" : " · unofficial"}
                </span>
              </button>
              <button className="btn btn-danger" onClick={() => void api.deleteReport(r.id).then(load)} title="Delete this report">
                Delete
              </button>
            </div>
          ))}
        </div>
      )}
    </>
  );
}

const PLAYER = "#ffb547";
const colorFor = (k: number, n: number) => `hsl(${Math.round((k * 360) / Math.max(n, 1) + 200) % 360} 70% 62%)`;

function ReportView({ id, onBack }: { id: string; onBack: () => void }) {
  const [r, setR] = useState<RaceReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [classId, setClassId] = useState<number | null>(null);
  const [focus, setFocus] = useState<number | null>(null);
  useEffect(() => {
    void api.getReport(id).then((rep) => {
      setR(rep);
      setClassId(rep.playerClassId);
      setFocus(rep.playerCarIdx);
    }).catch((e) => setError(String(e)));
  }, [id]);

  const cars = useMemo(
    () => (r ? r.cars.filter((c) => c.classId === classId).sort((a, b) => (a.finishClassPosition || 999) - (b.finishClassPosition || 999)) : []),
    [r, classId],
  );
  const colors = useMemo(() => new Map(cars.map((c, k) => [c.carIdx, c.isPlayer ? PLAYER : colorFor(k, cars.length)])), [cars]);

  if (error) return <><BackBar onBack={onBack} /><p className="notice"><span>{error}</span></p></>;
  if (!r) return <BackBar onBack={onBack} />;

  const me = r.cars.find((c) => c.isPlayer);
  const cls = r.classes.find((c) => c.classId === r.playerClassId);
  const series = (data: Map<number, Map<number, number>>): Series[] =>
    cars.map((c) => ({ id: c.carIdx, label: `#${c.number} ${c.name}`, color: colors.get(c.carIdx)!, points: data.get(c.carIdx) ?? new Map() }));
  const pitMarks = cars.flatMap((c) => c.pitStops.map((p) => ({ id: c.carIdx, lap: p.lap + 1, text: `Pit lane ${p.laneS.toFixed(1)} s, stopped ${p.stallS.toFixed(1)} s` })));
  const pick = { focus, setFocus };

  return (
    <>
      <BackBar onBack={onBack} />
      <h1 className="rp-title">{r.track}{r.trackConfig ? <small> · {r.trackConfig}</small> : null}</h1>
      <p className="lede">
        {new Date(r.startedAt * 1000).toLocaleString([], { dateStyle: "full", timeStyle: "short" })} · {r.car} · {r.sessionType}
        {r.official ? " · official" : " · unofficial"}
      </p>

      <div className="rp-facts">
        <Fact label="Finish" value={me && r.finished ? `P${me.finishClassPosition}` : "Didn't finish"}
          sub={me && r.finished ? `of ${r.cars.filter((c) => c.classId === r.playerClassId).length} in ${cls?.name ?? "class"} · P${me.finishPosition} overall` : undefined} />
        <Fact label="Start" value={me?.startPosition != null ? `P${me.startPosition}` : "–"} sub="in class" />
        <Fact label="Laps" value={String(me?.lapsCompleted ?? 0)} sub={r.totalLaps ? `of ${r.totalLaps}` : undefined} />
        <Fact label="Best lap" value={lapTime(me ? stats([...lapTimes(me).values()]).best : null)} />
        <Fact label="Incidents" value={`${r.incidents}x`} sub={r.incidentLimit ? `limit ${r.incidentLimit}x` : undefined} />
        <Fact label="Stops" value={String(me?.stops ?? 0)} />
        {cls?.strengthOfField ? <Fact label="SoF" value={String(cls.strengthOfField)} sub={cls.name} /> : null}
      </div>

      {r.classes.length > 1 && (
        <div className="rp-classes" role="group" aria-label="Class">
          {r.classes.map((c) => (
            <button key={c.classId} className={`rp-class ${c.classId === classId ? "is-on" : ""}`}
              style={{ "--cls": c.color } as React.CSSProperties} onClick={() => setClassId(c.classId)}>
              {c.name || "Class"} <small>{r.cars.filter((x) => x.classId === c.classId).length}</small>
            </button>
          ))}
        </div>
      )}

      <Results cars={cars} colors={colors} {...pick} />

      <ChartSection title="Position" help="Order across the line at the end of each lap, in this class. Click a name to pick out a driver.">
        <LineChart series={series(positionsByLap(cars))} invert integer format={(v) => `P${v}`} marks={pitMarks} {...pick} />
      </ChartSection>

      <GapSection cars={cars} series={series(gapsByLap(cars))} marks={pitMarks} pick={pick} />
      <LapTimeSection cars={cars} series={series(new Map(cars.map((c) => [c.carIdx, lapTimes(c)])))} marks={pitMarks} pick={pick} />

      <ChartSection title="Tyre stints" help="Each stint from the pit lane to the pit lane, coloured by compound, with each car's pit stops. Hover for times.">
        <StintChart cars={cars} compounds={r.tireCompounds} colors={colors} {...pick} />
      </ChartSection>

      {r.playerLaps.length > 0 && <YourLaps report={r} />}
    </>
  );
}

function BackBar({ onBack }: { onBack: () => void }) {
  return (
    <div className="rp-back">
      <button className="btn" onClick={onBack}>← All races</button>
    </div>
  );
}

function Fact({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="rp-fact">
      <span className="rp-fact-label">{label}</span>
      <b>{value}</b>
      {sub && <small>{sub}</small>}
    </div>
  );
}

function ChartSection({ title, help, tools, children }: { title: string; help?: string; tools?: ReactNode; children: ReactNode }) {
  return (
    <section className="section rp-section">
      <div className="rp-section-head">
        <h2>{title}</h2>
        {tools}
      </div>
      {help && <p className="note rp-help">{help}</p>}
      {children}
    </section>
  );
}

interface Pick {
  focus: number | null;
  setFocus: (id: number | null) => void;
}

/** The class's results: what was recorded for each car. */
function Results({ cars, colors, focus, setFocus }: { cars: ReportCar[]; colors: Map<number, string> } & Pick) {
  return (
    <section className="section rp-section">
      <h2>Results</h2>
      <div className="rp-table" role="table">
        <div className="rp-row rp-head" role="row">
          <span>Pos</span><span>Start</span><span>Driver</span><span>iR</span><span>Laps</span><span>Best</span>
          <span>Median</span><span>Stops</span><span>Pit lane</span><span />
        </div>
        {cars.map((c) => {
          const s = stats([...lapTimes(c).values()]);
          const lane = c.pitStops.reduce((t, p) => t + p.laneS, 0);
          return (
            <button key={c.carIdx} role="row" className={`rp-row ${c.isPlayer ? "is-player" : ""} ${focus === c.carIdx ? "is-focus" : ""}`}
              onClick={() => setFocus(focus === c.carIdx ? null : c.carIdx)}>
              <span><i className="rp-swatch" style={{ background: colors.get(c.carIdx) }} />{c.finishClassPosition || "–"}</span>
              <span>{c.startPosition ?? "–"}</span>
              <span className="rp-driver">#{c.number} {c.name}</span>
              <span>{c.irating ? (c.irating / 1000).toFixed(1) + "k" : "–"}</span>
              <span>{c.lapsCompleted}</span>
              <span>{lapTime(s.best)}</span>
              <span>{lapTime(s.median)}</span>
              <span>{c.stops}</span>
              <span>{c.pitStops.length ? `${lane.toFixed(1)} s` : "–"}</span>
              <span className="rp-status">{c.finished ? "Finished" : c.out ? "Out" : ""}</span>
            </button>
          );
        })}
      </div>
      <p className="note rp-help">Best and median are of the laps with both ends recorded. Pit lane is the total time between the pit lane lines.</p>
    </section>
  );
}

const ZOOMS_GAP = [
  { value: 0, label: "Everything" },
  { value: 60, label: "Up to 60 s" },
  { value: 30, label: "Up to 30 s" },
  { value: 10, label: "Up to 10 s" },
];

function GapSection({ series, marks, pick }: { cars: ReportCar[]; series: Series[]; marks: Mark[]; pick: Pick }) {
  const [zoom, setZoom] = useState(0);
  return (
    <ChartSection title="Gap to the class leader" help="Seconds behind the first car in this class to finish each lap, at the line."
      tools={<ZoomSelect value={zoom} choices={ZOOMS_GAP} onChange={setZoom} />}>
      <LineChart series={series} invert format={(v) => (v === 0 ? "Leader" : `+${v.toFixed(1)} s`)} yMax={zoom || undefined} yMin={0} marks={marks} {...pick} />
    </ChartSection>
  );
}

const ZOOMS_LAP = [
  { value: 0, label: "Every lap" },
  { value: 10, label: "Best + 10 s" },
  { value: 5, label: "Best + 5 s" },
  { value: 3, label: "Best + 3 s" },
];

function LapTimeSection({ series, marks, pick }: { cars: ReportCar[]; series: Series[]; marks: Mark[]; pick: Pick }) {
  const [zoom, setZoom] = useState(5);
  const best = Math.min(...series.flatMap((s) => [...s.points.values()]));
  return (
    <ChartSection title="Lap times" help="Time between crossing the line on one lap and the next. Quicker is higher. Laps cut off by the zoom sit on the bottom edge, ringed."
      tools={<ZoomSelect value={zoom} choices={ZOOMS_LAP} onChange={setZoom} />}>
      <LineChart series={series} format={lapTime} yMin={Number.isFinite(best) ? best - 0.3 : undefined}
        yMax={zoom && Number.isFinite(best) ? best + zoom : undefined} invert marks={marks} {...pick} />
    </ChartSection>
  );
}

function ZoomSelect({ value, choices, onChange }: { value: number; choices: { value: number; label: string }[]; onChange: (v: number) => void }) {
  return (
    <select className="rp-zoom" value={value} onChange={(e) => onChange(Number(e.target.value))} aria-label="Zoom">
      {choices.map((c) => <option key={c.value} value={c.value}>{c.label}</option>)}
    </select>
  );
}

// --- Line chart --------------------------------------------------------------

interface Series {
  id: number;
  label: string;
  color: string;
  /** lap -> value */
  points: Map<number, number>;
}

interface Mark {
  id: number;
  lap: number;
  text: string;
}

const W = 760;
const H = 300;
const PAD = { l: 58, r: 12, t: 10, b: 26 };

/**
 * Laps across, values up the side (smaller at the top with `invert`). Hover
 * shows every car's value on that lap; clicking a line or a name in the
 * legend picks that car out on every chart.
 */
function LineChart({ series, invert = false, integer = false, format, yMin, yMax, marks = [], focus, setFocus }: {
  series: Series[];
  invert?: boolean;
  integer?: boolean;
  format: (v: number) => string;
  yMin?: number;
  yMax?: number;
  marks?: Mark[];
} & Pick) {
  const [hover, setHover] = useState<number | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  const all = series.flatMap((s) => [...s.points.entries()]);
  if (all.length === 0) return <p className="note">Nothing recorded for this yet.</p>;

  const laps = all.map(([l]) => l);
  const x0 = Math.min(...laps);
  const x1 = Math.max(...laps, x0 + 1);
  const vals = all.map(([, v]) => v);
  const lo = yMin ?? Math.min(...vals);
  const hi = yMax ?? Math.max(...vals);
  const span = hi - lo || 1;
  const x = (l: number) => PAD.l + ((l - x0) / (x1 - x0)) * (W - PAD.l - PAD.r);
  const yRaw = (v: number) => {
    const f = (Math.min(Math.max(v, lo), hi) - lo) / span;
    return invert ? PAD.t + f * (H - PAD.t - PAD.b) : H - PAD.b - f * (H - PAD.t - PAD.b);
  };
  const clipped = (v: number) => v > hi || v < lo;

  const yTicks = integer ? intTicks(lo, hi) : niceTicks(lo, hi);
  const xTicks = intTicks(x0, x1, 12);

  const onMove = (e: React.MouseEvent) => {
    const r = svg.current!.getBoundingClientRect();
    const px = ((e.clientX - r.left) / r.width) * W;
    const lap = Math.round(x0 + ((px - PAD.l) / (W - PAD.l - PAD.r)) * (x1 - x0));
    setHover(lap >= x0 && lap <= x1 ? lap : null);
  };

  const ordered = [...series].sort((a, b) => Number(a.id === focus) - Number(b.id === focus));
  const at = hover == null ? [] : series
    .filter((s) => s.points.has(hover))
    .map((s) => ({ s, v: s.points.get(hover)! }))
    .sort((a, b) => a.v - b.v);

  return (
    <div className="rp-chart">
      <svg ref={svg} viewBox={`0 0 ${W} ${H}`} className="rp-svg" onMouseMove={onMove} onMouseLeave={() => setHover(null)}>
        {yTicks.map((v) => (
          <g key={`y${v}`}>
            <line className="rp-grid" x1={PAD.l} x2={W - PAD.r} y1={yRaw(v)} y2={yRaw(v)} />
            <text className="rp-axis" x={PAD.l - 6} y={yRaw(v) + 4} textAnchor="end">{format(v)}</text>
          </g>
        ))}
        {xTicks.map((l) => (
          <text key={`x${l}`} className="rp-axis" x={x(l)} y={H - 8} textAnchor="middle">{l}</text>
        ))}
        {ordered.map((s) => {
          const pts = [...s.points.entries()].sort((a, b) => a[0] - b[0]);
          const dim = focus != null && focus !== s.id;
          // Break the line where a lap is missing, so nothing is drawn that wasn't recorded.
          let d = "";
          let prev: number | null = null;
          for (const [l, v] of pts) {
            d += `${prev != null && l === prev + 1 ? "L" : "M"}${x(l).toFixed(1)},${yRaw(v).toFixed(1)} `;
            prev = l;
          }
          return (
            <g key={s.id} className={`rp-series ${dim ? "is-dim" : ""} ${focus === s.id ? "is-focus" : ""}`}
              onClick={() => setFocus(focus === s.id ? null : s.id)}>
              <path d={d} stroke={s.color} />
              {pts.map(([l, v]) => (
                <circle key={l} cx={x(l)} cy={yRaw(v)} r={clipped(v) ? 2.5 : 1.8} fill={s.color} className={clipped(v) ? "is-clipped" : ""} />
              ))}
            </g>
          );
        })}
        {marks.filter((m) => focus == null || m.id === focus).map((m, k) => {
          const s = series.find((x) => x.id === m.id);
          const v = s?.points.get(m.lap) ?? s?.points.get(m.lap - 1);
          if (!s || v == null) return null;
          return (
            <g key={k} className="rp-pit">
              <circle cx={x(m.lap)} cy={yRaw(v)} r={6} stroke={s.color} />
              <text x={x(m.lap)} y={yRaw(v) + 3.5} textAnchor="middle">P</text>
              <title>{`${s.label}: ${m.text}`}</title>
            </g>
          );
        })}
        {hover != null && <line className="rp-cursor" x1={x(hover)} x2={x(hover)} y1={PAD.t} y2={H - PAD.b} />}
      </svg>
      {hover != null && at.length > 0 && (
        <div className={`rp-tip ${x(hover) > W * 0.6 ? "is-left" : ""}`} style={{ left: `${(x(hover) / W) * 100}%` }}>
          <b>Lap {hover}</b>
          {at.slice(0, 14).map(({ s, v }) => (
            <span key={s.id} className={focus === s.id ? "is-focus" : ""}>
              <i style={{ background: s.color }} />
              {s.label}
              <em>{format(v)}</em>
            </span>
          ))}
          {at.length > 14 && <small>+{at.length - 14} more</small>}
        </div>
      )}
      <div className="rp-legend">
        {series.map((s) => (
          <button key={s.id} className={`rp-key ${focus === s.id ? "is-focus" : ""} ${focus != null && focus !== s.id ? "is-dim" : ""}`}
            onClick={() => setFocus(focus === s.id ? null : s.id)}>
            <i style={{ background: s.color }} />
            {s.label}
          </button>
        ))}
      </div>
    </div>
  );
}

function intTicks(lo: number, hi: number, max = 10): number[] {
  const step = Math.max(1, Math.ceil((hi - lo) / max));
  const out: number[] = [];
  for (let v = Math.ceil(lo); v <= hi; v += step) out.push(v);
  return out;
}

function niceTicks(lo: number, hi: number): number[] {
  const span = hi - lo || 1;
  const raw = span / 5;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 5, 10].map((m) => m * mag).find((s) => span / s <= 6) ?? raw;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-9; v += step) out.push(Number(v.toFixed(6)));
  return out;
}

// --- Stints ------------------------------------------------------------------

function StintChart({ cars, compounds, colors, focus, setFocus }: { cars: ReportCar[]; compounds: string[]; colors: Map<number, string> } & Pick) {
  const maxLap = Math.max(1, ...cars.flatMap((c) => c.stints.map((s) => s.end)));
  // From the first lap anything was recorded, not lap 0.
  const minLap = Math.min(maxLap - 1, Math.floor(Math.min(...cars.flatMap((c) => c.stints.map((s) => s.start)), maxLap)));
  const row = 22;
  const left = 170;
  const w = W - left - PAD.r;
  const x = (l: number) => left + ((l - minLap) / (maxLap - minLap)) * w;
  return (
    <div className="rp-chart">
      <svg viewBox={`0 0 ${W} ${cars.length * row + 22}`} className="rp-svg">
        {intTicks(minLap, maxLap, 12).map((l) => (
          <g key={l}>
            <line className="rp-grid" x1={x(l)} x2={x(l)} y1={0} y2={cars.length * row} />
            <text className="rp-axis" x={x(l)} y={cars.length * row + 15} textAnchor="middle">{l}</text>
          </g>
        ))}
        {cars.map((c, k) => (
          <g key={c.carIdx} className={`rp-stint-row ${focus != null && focus !== c.carIdx ? "is-dim" : ""}`}
            transform={`translate(0 ${k * row})`} onClick={() => setFocus(focus === c.carIdx ? null : c.carIdx)}>
            <rect x={0} y={4} width={8} height={row - 8} fill={colors.get(c.carIdx)} rx={2} />
            <text className={`rp-stint-name ${c.isPlayer ? "is-player" : ""}`} x={14} y={row / 2 + 4}>
              {`${c.finishClassPosition || "–"}  #${c.number} ${c.name}`.slice(0, 26)}
            </text>
            {c.stints.map((s, i) => {
              const name = s.compound >= 0 ? compounds[s.compound] ?? "" : "";
              return (
                <rect key={i} className={`rp-stint tyre-${name ? compoundKind(name) : "other"} ${s.partial ? "is-partial" : ""}`}
                  x={x(s.start)} y={5} width={Math.max(1.5, x(s.end) - x(s.start))} height={row - 10} rx={3}>
                  <title>{`${c.name}: ${s.partial ? "at least " : ""}${(s.end - s.start).toFixed(1)} laps${name ? ` on ${name}` : ""}, laps ${s.start.toFixed(1)}–${s.end.toFixed(1)}`}</title>
                </rect>
              );
            })}
            {c.pitStops.map((p, i) => (
              <g key={`p${i}`} className="rp-stint-stop">
                <rect x={x(p.lap + 1) - 1} y={2} width={2} height={row - 4} />
                <title>{`${c.name}: pit stop on lap ${p.lap + 1}, ${p.laneS.toFixed(1)} s in the pit lane, ${p.stallS.toFixed(1)} s stopped`}</title>
              </g>
            ))}
          </g>
        ))}
      </svg>
    </div>
  );
}

// --- Your laps ----------------------------------------------------------------

/** Your laps as Tenths logged them: time, fuel used, incidents, track temperature. */
function YourLaps({ report }: { report: RaceReport }) {
  const laps = report.playerLaps;
  const [hover, setHover] = useState<number | null>(null);
  const fuel = laps.filter((l) => l.fuelUsed != null);
  const maxFuel = Math.max(0.1, ...fuel.map((l) => l.fuelUsed!));
  const total = fuel.reduce((t, l) => t + l.fuelUsed!, 0);
  const n = laps.length;
  const bw = (W - PAD.l - PAD.r) / Math.max(n, 1);
  const h = 160;
  const temps = laps.map((l) => l.trackTemp).filter((t) => t > 0);
  const tLo = Math.min(...temps);
  const tHi = Math.max(...temps);
  const ty = (t: number) => 14 + (1 - (t - tLo) / (tHi - tLo || 1)) * (h - 40);

  return (
    <>
      <ChartSection title="Your fuel" help={`Litres used on each lap, measured from the tank. Laps with a stop or a refuel aren't measured. ${fuel.length} laps measured, ${total.toFixed(1)} L in all.`}>
        <div className="rp-chart">
          <svg viewBox={`0 0 ${W} ${h}`} className="rp-svg" onMouseLeave={() => setHover(null)}>
            {niceTicks(0, maxFuel).map((v) => (
              <g key={v}>
                <line className="rp-grid" x1={PAD.l} x2={W - PAD.r} y1={h - PAD.b - (v / maxFuel) * (h - PAD.b - 10)} y2={h - PAD.b - (v / maxFuel) * (h - PAD.b - 10)} />
                <text className="rp-axis" x={PAD.l - 6} y={h - PAD.b - (v / maxFuel) * (h - PAD.b - 10) + 4} textAnchor="end">{`${v} L`}</text>
              </g>
            ))}
            {laps.map((l, k) => {
              const bh = l.fuelUsed != null ? (l.fuelUsed / maxFuel) * (h - PAD.b - 10) : 0;
              return (
                <g key={l.lap} onMouseEnter={() => setHover(l.lap)}>
                  <rect className={`rp-bar ${hover === l.lap ? "is-hover" : ""}`} x={PAD.l + k * bw + 1} y={h - PAD.b - bh} width={Math.max(1, bw - 2)} height={bh} />
                  <rect x={PAD.l + k * bw} y={0} width={bw} height={h} fill="transparent" />
                  {hover === l.lap && (
                    <text className="rp-bar-value" x={PAD.l + k * bw + bw / 2} y={Math.max(10, h - PAD.b - bh - 5)} textAnchor="middle">
                      {l.fuelUsed != null ? `${l.fuelUsed.toFixed(2)} L` : "not measured"}
                    </text>
                  )}
                  {(n <= 30 || k % Math.ceil(n / 30) === 0) && (
                    <text className="rp-axis" x={PAD.l + k * bw + bw / 2} y={h - 8} textAnchor="middle">{l.lap}</text>
                  )}
                </g>
              );
            })}
          </svg>
        </div>
      </ChartSection>

      {temps.length > 1 && (
        <ChartSection title="Track temperature" help="At the end of each of your laps.">
          <div className="rp-chart">
            <svg viewBox={`0 0 ${W} ${h}`} className="rp-svg">
              {[...new Set([tLo, tHi])].map((t) => (
                <text key={t} className="rp-axis" x={PAD.l - 6} y={ty(t) + 4} textAnchor="end">{`${t.toFixed(1)}°`}</text>
              ))}
              <path className="rp-temp" d={laps.filter((l) => l.trackTemp > 0).map((l, k) => `${k ? "L" : "M"}${PAD.l + laps.indexOf(l) * bw + bw / 2},${ty(l.trackTemp)}`).join(" ")} />
            </svg>
          </div>
        </ChartSection>
      )}

      <section className="section rp-section">
        <h2>Your laps</h2>
        <div className="rp-table rp-laps" role="table">
          <div className="rp-row rp-head" role="row">
            <span>Lap</span><span>Time</span><span>Fuel</span><span>Incidents</span><span>Track</span><span>Air</span><span />
          </div>
          {laps.map((l) => (
            <div key={l.lap} role="row" className={`rp-row ${hover === l.lap ? "is-focus" : ""}`} onMouseEnter={() => setHover(l.lap)}>
              <span>{l.lap}</span>
              <span className={l.personalBest ? "rp-pb" : ""}>{lapTime(l.time)}</span>
              <span>{l.fuelUsed != null ? `${l.fuelUsed.toFixed(2)} L` : "–"}</span>
              <span>{l.incidents ? `${l.incidents}x` : ""}</span>
              <span>{l.trackTemp ? `${l.trackTemp.toFixed(1)}°` : ""}</span>
              <span>{l.airTemp ? `${l.airTemp.toFixed(1)}°` : ""}</span>
              <span className="rp-flags">
                {[l.pit && "Pit", l.offTrack && "Off track", !l.clean && !l.pit && !l.offTrack && l.time != null && "Not clean"].filter(Boolean).join(" · ")}
              </span>
            </div>
          ))}
        </div>
      </section>
    </>
  );
}
