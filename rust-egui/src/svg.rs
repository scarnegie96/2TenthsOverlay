//! Just enough SVG path to draw the web app's drawings (car silhouettes,
//! dashboard icons) natively: parse `d` (M L H V C S Q T A Z, absolute and
//! relative), flatten curves, and triangulate each subpath once (ear
//! clipping), so drawing is a transform of cached vertices.

use egui::epaint::Mesh;
use egui::{Color32, Painter, Pos2, Shape, Stroke};

#[derive(Debug, Clone)]
pub struct Sub {
    pub pts: Vec<[f32; 2]>,
    pub closed: bool,
    /// Triangles over `pts` (empty for open or degenerate subpaths).
    pub tris: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Path {
    pub subs: Vec<Sub>,
}

/// Where the drawing goes: SVG units scaled by `k`, turned by `angle`
/// (radians, clockwise on screen) about the SVG point `pivot`, which lands on `at`.
#[derive(Debug, Clone, Copy)]
pub struct Xf {
    pub at: Pos2,
    pub pivot: [f32; 2],
    pub k: f32,
    pub angle: f32,
}

impl Xf {
    pub fn new(at: Pos2, pivot: [f32; 2], k: f32) -> Self {
        Self { at, pivot, k, angle: 0.0 }
    }
    pub fn turned(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }
    pub fn map(&self, p: [f32; 2]) -> Pos2 {
        let (dx, dy) = ((p[0] - self.pivot[0]) * self.k, (p[1] - self.pivot[1]) * self.k);
        let (s, c) = self.angle.sin_cos();
        Pos2::new(self.at.x + dx * c - dy * s, self.at.y + dx * s + dy * c)
    }
}

impl Path {
    pub fn parse(d: &str) -> Self {
        let mut subs = parse(d);
        for s in &mut subs {
            // Fill as SVG would: an open subpath is filled as if closed.
            s.tris = triangulate(&s.pts);
        }
        Self { subs }
    }

    pub fn fill(&self, p: &Painter, xf: Xf, color: Color32) {
        let mut mesh = Mesh::default();
        for s in &self.subs {
            let base = mesh.vertices.len() as u32;
            for q in &s.pts {
                mesh.colored_vertex(xf.map(*q), color);
            }
            for t in s.tris.chunks(3) {
                mesh.add_triangle(base + t[0], base + t[1], base + t[2]);
            }
        }
        p.add(Shape::mesh(mesh));
    }

    pub fn stroke(&self, p: &Painter, xf: Xf, width: f32, color: Color32) {
        let stroke = Stroke::new(width * xf.k, color);
        for s in &self.subs {
            let pts: Vec<Pos2> = s.pts.iter().map(|q| xf.map(*q)).collect();
            if s.closed {
                p.add(Shape::closed_line(pts, stroke));
            } else {
                p.add(Shape::line(pts, stroke));
            }
        }
    }

    /// Round line ends (`stroke-linecap: round`).
    pub fn stroke_round(&self, p: &Painter, xf: Xf, width: f32, color: Color32) {
        self.stroke(p, xf, width, color);
        for s in self.subs.iter().filter(|s| !s.closed) {
            for q in [s.pts.first(), s.pts.last()].into_iter().flatten() {
                p.circle_filled(xf.map(*q), width * xf.k / 2.0, color);
            }
        }
    }
}

/// A rounded rectangle as a path (`<rect rx>`).
pub fn rect(x: f32, y: f32, w: f32, h: f32, rx: f32) -> Path {
    let rx = rx.min(w / 2.0).min(h / 2.0);
    if rx <= 0.0 {
        return Path::parse(&format!("M {x} {y} H {} V {} H {x} Z", x + w, y + h));
    }
    Path::parse(&format!(
        "M {} {y} H {} A {rx} {rx} 0 0 1 {} {} V {} A {rx} {rx} 0 0 1 {} {} H {} A {rx} {rx} 0 0 1 {x} {} V {} A {rx} {rx} 0 0 1 {} {y} Z",
        x + rx,
        x + w - rx,
        x + w,
        y + rx,
        y + h - rx,
        x + w - rx,
        y + h,
        x + rx,
        y + h - rx,
        y + rx,
        x + rx
    ))
}

/// A circle as a path.
pub fn circle(cx: f32, cy: f32, r: f32) -> Path {
    Path::parse(&format!("M {} {cy} A {r} {r} 0 1 1 {} {cy} A {r} {r} 0 1 1 {} {cy} Z", cx - r, cx + r, cx - r))
}

// --- Parsing --------------------------------------------------------------------

struct Tokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl Tokens<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len() && (self.s[self.i].is_ascii_whitespace() || self.s[self.i] == b',') {
            self.i += 1;
        }
    }
    fn command(&mut self) -> Option<u8> {
        self.skip();
        let c = *self.s.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }
    fn at_number(&mut self) -> bool {
        self.skip();
        self.s.get(self.i).is_some_and(|c| c.is_ascii_digit() || *c == b'-' || *c == b'+' || *c == b'.')
    }
    fn number(&mut self) -> f32 {
        self.skip();
        let start = self.i;
        let mut seen_dot = false;
        let mut seen_e = false;
        if self.i < self.s.len() && (self.s[self.i] == b'-' || self.s[self.i] == b'+') {
            self.i += 1;
        }
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_digit() {
                self.i += 1;
            } else if c == b'.' && !seen_dot && !seen_e {
                seen_dot = true;
                self.i += 1;
            } else if (c == b'e' || c == b'E') && !seen_e {
                seen_e = true;
                self.i += 1;
                if self.i < self.s.len() && (self.s[self.i] == b'-' || self.s[self.i] == b'+') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        std::str::from_utf8(&self.s[start..self.i]).ok().and_then(|t| t.parse().ok()).unwrap_or(0.0)
    }
    /// Arc flags may be written without separators ("0 01 1").
    fn flag(&mut self) -> bool {
        self.skip();
        let c = self.s.get(self.i).copied().unwrap_or(b'0');
        self.i += 1;
        c == b'1'
    }
}

fn parse(d: &str) -> Vec<Sub> {
    let mut t = Tokens { s: d.as_bytes(), i: 0 };
    let mut subs: Vec<Sub> = Vec::new();
    let mut cur: Vec<[f32; 2]> = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    // Last control point, for S and T.
    let mut last_c: Option<[f32; 2]> = None;
    let mut last_q: Option<[f32; 2]> = None;
    let mut cmd = b'M';
    let flush = |cur: &mut Vec<[f32; 2]>, subs: &mut Vec<Sub>, closed: bool| {
        if cur.len() > 1 {
            subs.push(Sub { pts: std::mem::take(cur), closed, tris: Vec::new() });
        } else {
            cur.clear();
        }
    };
    loop {
        if let Some(c) = t.command() {
            cmd = c;
        } else if !t.at_number() {
            break;
        }
        let rel = cmd.is_ascii_lowercase();
        let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
        match cmd.to_ascii_uppercase() {
            b'M' => {
                flush(&mut cur, &mut subs, false);
                x = ox + t.number();
                y = oy + t.number();
                sx = x;
                sy = y;
                cur.push([x, y]);
                // Further pairs are line-tos.
                cmd = if rel { b'l' } else { b'L' };
                last_c = None;
                last_q = None;
            }
            b'L' => {
                x = ox + t.number();
                y = oy + t.number();
                cur.push([x, y]);
                last_c = None;
                last_q = None;
            }
            b'H' => {
                x = ox + t.number();
                cur.push([x, y]);
                last_c = None;
                last_q = None;
            }
            b'V' => {
                y = oy + t.number();
                cur.push([x, y]);
                last_c = None;
                last_q = None;
            }
            b'C' | b'S' => {
                let c1 = if cmd.eq_ignore_ascii_case(&b'C') {
                    [ox + t.number(), oy + t.number()]
                } else {
                    last_c.map(|c| [2.0 * x - c[0], 2.0 * y - c[1]]).unwrap_or([x, y])
                };
                let c2 = [ox + t.number(), oy + t.number()];
                let e = [ox + t.number(), oy + t.number()];
                for i in 1..=12 {
                    let s = i as f32 / 12.0;
                    let u = 1.0 - s;
                    let b = |a: f32, b: f32, c: f32, d: f32| u * u * u * a + 3.0 * u * u * s * b + 3.0 * u * s * s * c + s * s * s * d;
                    cur.push([b(x, c1[0], c2[0], e[0]), b(y, c1[1], c2[1], e[1])]);
                }
                last_c = Some(c2);
                last_q = None;
                x = e[0];
                y = e[1];
            }
            b'Q' | b'T' => {
                let c = if cmd.eq_ignore_ascii_case(&b'Q') {
                    [ox + t.number(), oy + t.number()]
                } else {
                    last_q.map(|c| [2.0 * x - c[0], 2.0 * y - c[1]]).unwrap_or([x, y])
                };
                let e = [ox + t.number(), oy + t.number()];
                for i in 1..=10 {
                    let s = i as f32 / 10.0;
                    let u = 1.0 - s;
                    cur.push([u * u * x + 2.0 * u * s * c[0] + s * s * e[0], u * u * y + 2.0 * u * s * c[1] + s * s * e[1]]);
                }
                last_q = Some(c);
                last_c = None;
                x = e[0];
                y = e[1];
            }
            b'A' => {
                let rx = t.number().abs();
                let ry = t.number().abs();
                let rot = t.number().to_radians();
                let large = t.flag();
                let sweep = t.flag();
                let e = [ox + t.number(), oy + t.number()];
                arc(&mut cur, [x, y], e, rx, ry, rot, large, sweep);
                x = e[0];
                y = e[1];
                last_c = None;
                last_q = None;
            }
            b'Z' => {
                flush(&mut cur, &mut subs, true);
                x = sx;
                y = sy;
                cur.push([x, y]);
                last_c = None;
                last_q = None;
                // A number straight after Z isn't valid; stop rather than loop.
                if t.at_number() {
                    break;
                }
                continue;
            }
            _ => break,
        }
    }
    flush(&mut cur, &mut subs, false);
    // Drop the stray start point a trailing Z leaves.
    subs
}

/// SVG endpoint arc to points (the spec's F.6.5 conversion).
#[allow(clippy::too_many_arguments)]
fn arc(out: &mut Vec<[f32; 2]>, p0: [f32; 2], p1: [f32; 2], mut rx: f32, mut ry: f32, phi: f32, large: bool, sweep: bool) {
    if rx == 0.0 || ry == 0.0 || (p0[0] == p1[0] && p0[1] == p1[1]) {
        out.push(p1);
        return;
    }
    let (s, c) = phi.sin_cos();
    let dx = (p0[0] - p1[0]) / 2.0;
    let dy = (p0[1] - p1[1]) / 2.0;
    let x1 = c * dx + s * dy;
    let y1 = -s * dx + c * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = (num / den).max(0.0).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let cx = c * cx1 - s * cy1 + (p0[0] + p1[0]) / 2.0;
    let cy = s * cx1 + c * cy1 + (p0[1] + p1[1]) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = ang(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = ang((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !sweep && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f32::consts::TAU;
    }
    let n = ((dt.abs() / (std::f32::consts::PI / 16.0)).ceil() as usize).max(2);
    for i in 1..=n {
        let a = t1 + dt * i as f32 / n as f32;
        let (sa, ca) = a.sin_cos();
        out.push([cx + c * rx * ca - s * ry * sa, cy + s * rx * ca + c * ry * sa]);
    }
}

// --- Triangulation ---------------------------------------------------------------

/// Ear clipping of a simple polygon (either winding); duplicate end point dropped.
pub fn triangulate(pts: &[[f32; 2]]) -> Vec<u32> {
    let mut n = pts.len();
    if n >= 2 && pts[0] == pts[n - 1] {
        n -= 1;
    }
    if n < 3 {
        return Vec::new();
    }
    let area: f32 = (0..n).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        a[0] * b[1] - b[0] * a[1]
    }).sum();
    let ccw = area > 0.0;
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity((n - 2) * 3);
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            let cr = cross(a, b, c);
            let convex = if ccw { cr > 1e-9 } else { cr < -1e-9 };
            if !convex {
                continue;
            }
            // No other vertex inside the ear.
            let inside = idx.iter().any(|&j| {
                if j == ia || j == ib || j == ic {
                    return false;
                }
                let q = pts[j];
                let (d1, d2, d3) = (cross(a, b, q), cross(b, c, q), cross(c, a, q));
                if ccw {
                    d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0
                } else {
                    d1 <= 0.0 && d2 <= 0.0 && d3 <= 0.0
                }
            });
            if inside {
                continue;
            }
            out.extend([ia as u32, ib as u32, ic as u32]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            // Degenerate (collinear) points left: drop one and go on.
            let drop = (0..idx.len()).find(|&i| {
                let m = idx.len();
                cross(pts[idx[(i + m - 1) % m]], pts[idx[i]], pts[idx[(i + 1) % m]]).abs() <= 1e-9
            });
            match drop {
                Some(i) => {
                    idx.remove(i);
                }
                None => break,
            }
        }
    }
    if idx.len() == 3 {
        out.extend([idx[0] as u32, idx[1] as u32, idx[2] as u32]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_commands() {
        let p = Path::parse("M 0 0 L 10 0 L 10 10 Z M 20 20 h 5 v 5 h -5 z");
        assert_eq!(p.subs.len(), 2);
        assert!(p.subs[0].closed);
        assert_eq!(p.subs[1].pts[1], [25.0, 20.0]);
        assert_eq!(p.subs[1].pts[2], [25.0, 25.0]);
        // Packed numbers and arc flags.
        let a = Path::parse("M50 9m-4.5 0a41 41 0 0 1 9 0");
        let last = *a.subs[0].pts.last().unwrap();
        assert!((last[0] - 54.5).abs() < 1e-3 && (last[1] - 9.0).abs() < 1e-3);
        assert!(a.subs[0].pts.iter().all(|q| q[1] <= 9.0 + 1e-3));
    }

    #[test]
    fn triangulates_concave() {
        // An L shape: 6 points, 4 triangles, total area 3.
        let pts = [[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0], [1.0, 2.0], [0.0, 2.0]];
        let tris = triangulate(&pts);
        assert_eq!(tris.len(), 12);
        let area: f32 = tris
            .chunks(3)
            .map(|t| {
                let (a, b, c) = (pts[t[0] as usize], pts[t[1] as usize], pts[t[2] as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum();
        assert!((area - 3.0).abs() < 1e-4);
        let c = circle(0.0, 0.0, 10.0);
        assert!(!c.subs[0].tris.is_empty());
        let r = rect(0.0, 0.0, 10.0, 5.0, 2.0);
        assert!(r.subs[0].closed && r.subs[0].tris.len() >= 6);
    }
}
