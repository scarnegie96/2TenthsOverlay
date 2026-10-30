export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Guide {
  axis: "x" | "y";
  pos: number;
}

const THRESHOLD = 8;
const GRID = 4;

/**
 * Snaps a moving rect to the edges and centres of other widgets and of the
 * screen. Returns the adjusted position plus the guide lines to draw.
 */
export function snap(moving: Rect, others: Rect[], screen: { w: number; h: number }) {
  const xs: number[] = [0, screen.w / 2, screen.w];
  const ys: number[] = [0, screen.h / 2, screen.h];
  for (const o of others) {
    xs.push(o.x, o.x + o.w, o.x + o.w / 2);
    ys.push(o.y, o.y + o.h, o.y + o.h / 2);
  }

  const best = (start: number, size: number, lines: number[]) => {
    let hit: { delta: number; line: number } | null = null;
    for (const edge of [start, start + size / 2, start + size]) {
      for (const line of lines) {
        const d = line - edge;
        if (Math.abs(d) <= THRESHOLD && (!hit || Math.abs(d) < Math.abs(hit.delta))) hit = { delta: d, line };
      }
    }
    return hit;
  };

  const guides: Guide[] = [];
  let { x, y } = moving;
  const hx = best(x, moving.w, xs);
  if (hx) {
    x += hx.delta;
    guides.push({ axis: "x", pos: hx.line });
  } else {
    x = Math.round(x / GRID) * GRID;
  }
  const hy = best(y, moving.h, ys);
  if (hy) {
    y += hy.delta;
    guides.push({ axis: "y", pos: hy.line });
  } else {
    y = Math.round(y / GRID) * GRID;
  }

  x = Math.min(Math.max(0, x), Math.max(0, screen.w - moving.w));
  y = Math.min(Math.max(0, y), Math.max(0, screen.h - moving.h));
  return { x, y, guides };
}
