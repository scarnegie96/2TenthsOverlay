export type Units = "metric" | "imperial";

const MINUS = "−";

/** 92.123 -> "1:32.123" */
export function lapTime(t: number | null | undefined, decimals = 3): string {
  if (t == null || !(t > 0)) return "–";
  const m = Math.floor(t / 60);
  const s = t - m * 60;
  const sec = s.toFixed(decimals).padStart(decimals + 3, "0");
  return m > 0 ? `${m}:${sec}` : s.toFixed(decimals);
}

/** Signed seconds with a real minus sign: "+0.21", "−1.40". */
export function signed(v: number, decimals = 2): string {
  const s = Math.abs(v).toFixed(decimals);
  if (Number(s) === 0) return (0).toFixed(decimals);
  return (v < 0 ? MINUS : "+") + s;
}

export function gap(v: number | null | undefined, decimals = 1): string {
  if (v == null) return "";
  return Math.abs(v).toFixed(decimals);
}

/** 1:02:03 / 12:34 */
export function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${ss}` : `${m}:${ss}`;
}

export function speed(ms: number, units: Units): number {
  return Math.round(Math.abs(ms) * (units === "imperial" ? 2.23694 : 3.6));
}

export function speedUnit(units: Units): string {
  return units === "imperial" ? "mph" : "km/h";
}

export function fuel(litres: number, units: Units): number {
  return units === "imperial" ? litres * 0.264172 : litres;
}

export function fuelUnit(units: Units): string {
  return units === "imperial" ? "gal" : "L";
}

/** Tire pressure from kPa: "165 kPa" or "23.9 psi". */
export function pressure(kpa: number, units: Units): string {
  return units === "imperial" ? `${(kpa * 0.145038).toFixed(1)} psi` : `${Math.round(kpa)} kPa`;
}

export function gear(g: number): string {
  if (g < 0) return "R";
  if (g === 0) return "N";
  return String(g);
}

/** 2431 -> "2.4k" */
export function irating(ir: number): string {
  if (!ir) return "";
  return ir >= 1000 ? `${(ir / 1000).toFixed(1)}k` : String(ir);
}

/** "A 3.45" -> { letter: "A", sr: "3.45" } */
export function license(lic: string): { letter: string; sr: string } {
  const [letter = "", sr = ""] = lic.split(" ");
  return { letter, sr };
}

/** Readable text color on top of a hex background. */
export function contrastText(hex: string): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex);
  if (!m) return "#fff";
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 255, g = (n >> 8) & 255, b = n & 255;
  return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? "#0d131b" : "#fff";
}
