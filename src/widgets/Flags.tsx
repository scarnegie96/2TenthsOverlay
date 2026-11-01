import type React from "react";
import { useEffect, useMemo, useRef } from "react";
import { opt, type WidgetConfig } from "../lib/config";
import { useLive } from "../lib/store";

/** irsdk_Flags bits in SessionFlags. */
const F = {
  checkered: 0x1, white: 0x2, green: 0x4, yellow: 0x8, red: 0x10, blue: 0x20, debris: 0x40,
  yellowWaving: 0x100, oneLapToGreen: 0x200, caution: 0x4000, cautionWaving: 0x8000,
  black: 0x10000, disqualify: 0x20000, furled: 0x80000, repair: 0x100000, startGo: 0x80000000,
} as const;
const NOT_GREEN = F.caution | F.cautionWaving | F.yellow | F.yellowWaving | F.red | F.checkered;

/** Real flag colors, not the theme's: the board is a physical sign. */
const LED = {
  yellow: "#ffd21a", blue: "#2e86ff", green: "#22d36b", red: "#ff2b2b", white: "#f4f4ef", orange: "#ff8a00",
};

type Pattern =
  | { kind: "fill"; color: string }
  | { kind: "text"; text: string; color: string; border: string }
  | { kind: "checker" | "meatball" | "debris" | "warning" | "off" };

interface Board {
  pattern: Pattern;
  /** Waved flags flash. */
  flash: boolean;
  /** Option key for the caption under the board. */
  textKey: string;
  color: string;
}

function boardFor(flags: number, showGreen: boolean, scText: string, race: boolean): Board | null {
  const has = (m: number) => (flags & m) !== 0;
  if (has(F.disqualify)) return { pattern: { kind: "text", text: "DSQ", color: LED.white, border: LED.red }, flash: false, textKey: "textDisqualified", color: LED.red };
  if (has(F.black)) return { pattern: { kind: "text", text: "BLK", color: LED.white, border: LED.white }, flash: false, textKey: "textBlack", color: LED.white };
  if (has(F.repair)) return { pattern: { kind: "meatball" }, flash: false, textKey: "textRepair", color: LED.orange };
  if (has(F.furled)) return { pattern: { kind: "warning" }, flash: false, textKey: "textWarning", color: LED.white };
  if (has(F.red)) return { pattern: { kind: "fill", color: LED.red }, flash: false, textKey: "textRed", color: LED.red };
  if (has(F.checkered)) return { pattern: { kind: "checker" }, flash: false, textKey: "textCheckered", color: LED.white };
  if (has(F.caution | F.cautionWaving))
    return { pattern: { kind: "text", text: scText, color: LED.white, border: LED.yellow }, flash: has(F.cautionWaving), textKey: "textCaution", color: LED.yellow };
  if (has(F.yellow | F.yellowWaving)) return { pattern: { kind: "fill", color: LED.yellow }, flash: has(F.yellowWaving), textKey: "textYellow", color: LED.yellow };
  if (has(F.debris)) return { pattern: { kind: "debris" }, flash: false, textKey: "textDebris", color: LED.yellow };
  // iRacing also sets this in practice and test sessions, where there's no start to wait for.
  if (race && has(F.oneLapToGreen)) return { pattern: { kind: "text", text: "1", color: LED.white, border: LED.green }, flash: false, textKey: "textOneToGreen", color: LED.green };
  if (has(F.blue)) return { pattern: { kind: "fill", color: LED.blue }, flash: true, textKey: "textBlue", color: LED.blue };
  if (has(F.white)) return { pattern: { kind: "fill", color: LED.white }, flash: false, textKey: "textWhite", color: LED.white };
  if (showGreen) return { pattern: { kind: "fill", color: LED.green }, flash: false, textKey: "textGreen", color: LED.green };
  return null;
}

const CAPTION_DEFAULTS: Record<string, string> = {
  textYellow: "Accident ahead", textCaution: "Safety car", textDebris: "Debris on track", textBlue: "Faster car behind",
  textWhite: "Last lap", textCheckered: "Finish", textGreen: "Green flag", textOneToGreen: "Green next lap",
  textRed: "Session stopped", textBlack: "Penalty: pit now", textRepair: "Damage: pit for repairs", textWarning: "Warning",
  textDisqualified: "Disqualified",
};

/** A trackside LED flag panel, with a line of text under it. */
export function Flags({ config, editing }: { config: WidgetConfig; editing: boolean }) {
  const raw = useLive((s) => s.field?.clock.flags);
  const race = useLive((s) => /race/i.test(s.field?.clock.sessionType ?? ""));
  const flags = (raw ?? 0) >>> 0;
  const greenSeconds = opt(config, "greenSeconds", 5);
  const showCaption = opt(config, "showCaption", true);
  const always = opt(config, "alwaysShow", false);
  const scText = (opt(config, "scBoard", "SC") || "SC").toUpperCase().slice(0, 3);

  // Green only shows for a moment after a start or restart, then the board goes dark.
  const prev = useRef<number | null>(null);
  const greenUntil = useRef(0);
  const isGreen = (f: number) => (f & (F.green | F.startGo)) !== 0 && (f & NOT_GREEN) === 0;
  if (raw != null) {
    // The first flags we see are the state of play, not a start.
    if (prev.current !== null && isGreen(flags) && !isGreen(prev.current)) greenUntil.current = performance.now() + greenSeconds * 1000;
    prev.current = flags;
  }

  // Green is for starts and restarts, so races only (practice and test sessions flick it on when you leave the pits).
  const board = boardFor(flags, race && performance.now() < greenUntil.current, scText, race);
  const visible = Boolean(board) || editing || always;
  const caption = board ? String(config.options[board.textKey] ?? CAPTION_DEFAULTS[board.textKey] ?? "") : editing ? "No flag" : "";

  return (
    <div className={`flags ${visible ? "" : "is-hidden"}`} style={{ "--flag": board?.color ?? "var(--ink-3)" } as React.CSSProperties}>
      <div className="flags-housing">
        <LedBoard pattern={board?.pattern ?? { kind: "off" }} flash={board?.flash ?? false} />
        {showCaption && caption && <div className="flags-caption">{caption}</div>}
      </div>
    </div>
  );
}

// --- The LED matrix ------------------------------------------------------------

const DOTS = 28;
const PITCH = 5.5;
const SIZE = DOTS * PITCH;
const UNLIT = "#1b1f25";

function LedBoard({ pattern: given, flash }: { pattern: Pattern; flash: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  // The pattern is a new object on every render; only redraw when its content changes.
  const key = JSON.stringify(given);
  const pattern = useMemo(() => JSON.parse(key) as Pattern, [key]);

  useEffect(() => {
    const el = canvas.current!;
    const ctx = el.getContext("2d")!;
    const dpr = Math.max(1, window.devicePixelRatio || 1) * 2;
    el.width = SIZE * dpr;
    el.height = SIZE * dpr;
    ctx.scale(dpr, dpr);
    const lit = litDots(pattern);

    const draw = (on: boolean) => {
      ctx.clearRect(0, 0, SIZE, SIZE);
      for (let y = 0; y < DOTS; y++) {
        for (let x = 0; x < DOTS; x++) {
          const color = on ? lit[y * DOTS + x] : null;
          const cx = (x + 0.5) * PITCH;
          const cy = (y + 0.5) * PITCH;
          if (color) {
            // A soft halo, then the bright core.
            ctx.globalAlpha = 0.28;
            ctx.fillStyle = color;
            ctx.beginPath();
            ctx.arc(cx, cy, PITCH * 0.62, 0, Math.PI * 2);
            ctx.fill();
            ctx.globalAlpha = 1;
          }
          ctx.fillStyle = color ?? UNLIT;
          ctx.beginPath();
          ctx.arc(cx, cy, PITCH * 0.36, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    };

    draw(true);
    if (!flash) return;
    let on = true;
    const timer = window.setInterval(() => draw((on = !on)), 300);
    return () => window.clearInterval(timer);
  }, [pattern, flash]);

  return <canvas ref={canvas} className="flags-board" style={{ width: SIZE, height: SIZE }} />;
}

/** The color of every dot, row by row; null for unlit. */
function litDots(p: Pattern): (string | null)[] {
  const dots: (string | null)[] = new Array(DOTS * DOTS).fill(null);
  const set = (x: number, y: number, c: string) => {
    if (x >= 0 && y >= 0 && x < DOTS && y < DOTS) dots[y * DOTS + x] = c;
  };
  const each = (fn: (x: number, y: number) => string | null) => {
    for (let y = 0; y < DOTS; y++) for (let x = 0; x < DOTS; x++) dots[y * DOTS + x] = fn(x, y);
  };
  const mid = (DOTS - 1) / 2;

  switch (p.kind) {
    case "fill":
      each(() => p.color);
      break;
    case "checker":
      each((x, y) => ((Math.floor(x / 4) + Math.floor(y / 4)) % 2 === 0 ? LED.white : null));
      break;
    case "meatball":
      each((x, y) => (Math.hypot(x - mid, y - mid) <= 9.5 ? LED.orange : null));
      break;
    case "debris":
      // The surface flag: yellow with red stripes.
      each((x) => (Math.floor(x / 4) % 2 === 0 ? LED.yellow : LED.red));
      break;
    case "warning":
      // Black and white, split on the diagonal.
      each((x, y) => (x + y < DOTS - 1 ? LED.white : null));
      break;
    case "text": {
      each((x, y) => (Math.min(x, y, DOTS - 1 - x, DOTS - 1 - y) < 2 ? p.border : null));
      const chars = [...p.text].map((c) => GLYPHS[c] ?? GLYPHS[" "]);
      const scale = chars.length <= 2 ? 2 : 1;
      const w = chars.length * 5 * scale + (chars.length - 1) * scale;
      const h = 7 * scale;
      const x0 = Math.round((DOTS - w) / 2);
      const y0 = Math.round((DOTS - h) / 2);
      chars.forEach((g, i) => {
        g.forEach((row, ry) =>
          [...row].forEach((bit, rx) => {
            if (bit !== "1") return;
            for (let sy = 0; sy < scale; sy++)
              for (let sx = 0; sx < scale; sx++) set(x0 + i * 6 * scale + rx * scale + sx, y0 + ry * scale + sy, p.color);
          }),
        );
      });
      break;
    }
    case "off":
      break;
  }
  return dots;
}

/** 5x7 dot-matrix font. */
const GLYPHS: Record<string, string[]> = Object.fromEntries(
  Object.entries({
    A: "01110 10001 10001 11111 10001 10001 10001", B: "11110 10001 10001 11110 10001 10001 11110",
    C: "01110 10001 10000 10000 10000 10001 01110", D: "11110 10001 10001 10001 10001 10001 11110",
    E: "11111 10000 10000 11110 10000 10000 11111", F: "11111 10000 10000 11110 10000 10000 10000",
    G: "01110 10001 10000 10111 10001 10001 01111", H: "10001 10001 10001 11111 10001 10001 10001",
    I: "01110 00100 00100 00100 00100 00100 01110", J: "00111 00010 00010 00010 00010 10010 01100",
    K: "10001 10010 10100 11000 10100 10010 10001", L: "10000 10000 10000 10000 10000 10000 11111",
    M: "10001 11011 10101 10101 10001 10001 10001", N: "10001 10001 11001 10101 10011 10001 10001",
    O: "01110 10001 10001 10001 10001 10001 01110", P: "11110 10001 10001 11110 10000 10000 10000",
    Q: "01110 10001 10001 10001 10101 10010 01101", R: "11110 10001 10001 11110 10100 10010 10001",
    S: "01111 10000 10000 01110 00001 00001 11110", T: "11111 00100 00100 00100 00100 00100 00100",
    U: "10001 10001 10001 10001 10001 10001 01110", V: "10001 10001 10001 10001 10001 01010 00100",
    W: "10001 10001 10001 10101 10101 10101 01010", X: "10001 10001 01010 00100 01010 10001 10001",
    Y: "10001 10001 01010 00100 00100 00100 00100", Z: "11111 00001 00010 00100 01000 10000 11111",
    0: "01110 10001 10011 10101 11001 10001 01110", 1: "00100 01100 00100 00100 00100 00100 01110",
    2: "01110 10001 00001 00010 00100 01000 11111", 3: "11111 00010 00100 00010 00001 10001 01110",
    4: "00010 00110 01010 10010 11111 00010 00010", 5: "11111 10000 11110 00001 00001 10001 01110",
    6: "00110 01000 10000 11110 10001 10001 01110", 7: "11111 00001 00010 00100 01000 01000 01000",
    8: "01110 10001 10001 01110 10001 10001 01110", 9: "01110 10001 10001 01111 00001 00010 01100",
    "-": "00000 00000 00000 11111 00000 00000 00000", " ": "00000 00000 00000 00000 00000 00000 00000",
  }).map(([k, v]) => [k, v.split(" ")]),
);
