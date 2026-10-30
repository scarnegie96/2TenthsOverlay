import { opt, type WidgetConfig } from "../lib/config";
import { useLive } from "../lib/store";
import { CarLeftRight, type NearbyCar } from "../lib/types";

const SIZE = 236;
const C = SIZE / 2;
const RADIUS = 100;
const CAR_LEN = 4.7;
const CAR_WID = 2.0;
/** Lateral distance between lane centres, metres. */
const LANE = 3.8;

/**
 * Top-down view of the cars around you. Distance along the track is exact
 * (from lap position). iRacing doesn't report lateral position, so cars that
 * are alongside are placed in the lane it says they're in (CarLeftRight).
 */
export function Radar({ config, editing }: { config: WidgetConfig; editing: boolean }) {
  const nearby = useLive((s) => s.tick?.nearby);
  const clr = useLive((s) => s.tick?.carLeftRight ?? CarLeftRight.Off);
  const range = opt(config, "range", 20);
  const always = opt(config, "alwaysShow", false);
  const classColors = opt(config, "classColors", false);

  const scale = RADIUS / range;
  const cars = (nearby ?? []).filter((c) => Math.abs(c.dist) <= range + CAR_LEN);
  const left = clr === CarLeftRight.CarLeft || clr === CarLeftRight.CarLeftRight || clr === CarLeftRight.TwoCarsLeft;
  const right = clr === CarLeftRight.CarRight || clr === CarLeftRight.CarLeftRight || clr === CarLeftRight.TwoCarsRight;
  const visible = editing || always || cars.length > 0;

  // Distance readouts are for the cars in line with us, not the ones alongside.
  const inLine = cars.filter((c) => c.side === 0);
  const nearestAhead = inLine.filter((c) => c.dist >= 0).sort((a, b) => a.dist - b.dist)[0];
  const nearestBehind = inLine.filter((c) => c.dist < 0).sort((a, b) => b.dist - a.dist)[0];

  return (
    <div className={`radar ${visible ? "" : "is-hidden"}`}>
      <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} aria-hidden>
        <defs>
          <radialGradient id="radar-bg">
            <stop offset="0%" stopColor="rgba(13,19,27,0.88)" />
            <stop offset="78%" stopColor="rgba(13,19,27,0.78)" />
            <stop offset="100%" stopColor="rgba(13,19,27,0)" />
          </radialGradient>
          <clipPath id="radar-clip">
            <circle cx={C} cy={C} r={RADIUS + 4} />
          </clipPath>
        </defs>
        <circle cx={C} cy={C} r={RADIUS + 8} fill="url(#radar-bg)" />
        <circle className="radar-ring" cx={C} cy={C} r={RADIUS} />
        <circle className="radar-ring is-inner" cx={C} cy={C} r={RADIUS / 2} />
        <line className="radar-axis" x1={C} y1={C - RADIUS} x2={C} y2={C + RADIUS} />

        <SideWarning side="left" on={left} />
        <SideWarning side="right" on={right} />

        <g clipPath="url(#radar-clip)">
          {cars.map((c) => (
            <Car key={c.carIdx} car={c} scale={scale} classColors={classColors} />
          ))}
          <rect
            className="radar-me"
            x={C - (CAR_WID * scale) / 2}
            y={C - (CAR_LEN * scale) / 2}
            width={CAR_WID * scale}
            height={CAR_LEN * scale}
            rx={Math.min(4, CAR_WID * scale * 0.3)}
          />
        </g>

        {nearestAhead && <DistLabel y={C - RADIUS - 5} dist={nearestAhead.dist} />}
        {nearestBehind && <DistLabel y={C + RADIUS + 13} dist={nearestBehind.dist} />}
      </svg>
    </div>
  );
}

function Car({ car, scale, classColors }: { car: NearbyCar; scale: number; classColors: boolean }) {
  const d = Math.abs(car.dist);
  const tone = d < CAR_LEN ? "is-overlap" : d < CAR_LEN * 2.2 ? "is-close" : "is-far";
  const x = C + car.side * LANE * scale - (CAR_WID * scale) / 2;
  const y = C - car.dist * scale - (CAR_LEN * scale) / 2;
  return (
    <rect
      className={`radar-car ${tone}`}
      x={x}
      y={y}
      width={CAR_WID * scale}
      height={CAR_LEN * scale}
      rx={Math.min(4, CAR_WID * scale * 0.3)}
      style={classColors && tone === "is-far" ? { fill: car.classColor } : undefined}
    />
  );
}

function SideWarning({ side, on }: { side: "left" | "right"; on: boolean }) {
  const r = RADIUS - 6;
  // An arc hugging the left or right edge of the radar, ±35° from horizontal.
  const a = (35 * Math.PI) / 180;
  const sx = side === "left" ? -1 : 1;
  const x1 = C + sx * r * Math.cos(a);
  const y1 = C - r * Math.sin(a);
  const y2 = C + r * Math.sin(a);
  const sweep = side === "left" ? 0 : 1;
  return (
    <path
      className={`radar-side ${on ? "is-on" : ""}`}
      d={`M ${x1} ${y1} A ${r} ${r} 0 0 ${sweep} ${x1} ${y2}`}
    />
  );
}

function DistLabel({ y, dist }: { y: number; dist: number }) {
  return (
    <text className="radar-dist" x={C} y={y} textAnchor="middle">
      {Math.abs(dist).toFixed(1)} m
    </text>
  );
}
