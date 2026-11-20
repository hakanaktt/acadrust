//! A surface's triangulation built from its definitions, in order: points
//! go into a constrained Delaunay triangulation, lines become fixed edges,
//! non-destructive lines and boundaries split the triangles they cross,
//! boundaries hide triangles, edits change vertices and edges.

use super::tin::{triangulate_in, Triangulation};
use crate::classes::civil::{TinBoundaryType, TinBreaklineType, TinDefinition, TinLines, TinOperation};
use crate::types::{Vector2, Vector3};
use std::collections::{HashMap, HashSet};

const EPS: f64 = 1e-9;

fn orient(a: Vector3, b: Vector3, c: Vector3) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn in_circle(a: Vector3, b: Vector3, c: Vector3, p: Vector3) -> bool {
    let (ax, ay, bx, by, cx, cy) = (a.x - p.x, a.y - p.y, b.x - p.x, b.y - p.y, c.x - p.x, c.y - p.y);
    (ax * ax + ay * ay) * (bx * cy - cx * by) - (bx * bx + by * by) * (ax * cy - cx * ay) + (cx * cx + cy * cy) * (ax * by - bx * ay) > EPS
}

fn same_plan(a: Vector3, b: Vector3) -> bool {
    (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS
}

/// `p` strictly between `a` and `b` on the segment `a b`.
fn on_segment(a: Vector3, b: Vector3, p: Vector3) -> bool {
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    if orient(a, b, p).abs() > EPS * len.max(1.0) {
        return false;
    }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / (len * len);
    t > EPS && t < 1.0 - EPS
}

/// Segments `a b` and `c d` cross at a point inside both.
fn crosses(a: Vector3, b: Vector3, c: Vector3, d: Vector3) -> bool {
    let (d1, d2, d3, d4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    d1 * d2 < -EPS && d3 * d4 < -EPS
}

fn inside(polygon: &[Vector3], p: Vector3) -> bool {
    let mut inside = false;
    let n = polygon.len();
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
    }
    inside
}

fn key(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

struct Mesh {
    vertices: Vec<Vector3>,
    triangles: Vec<[usize; 3]>,
    hidden: Vec<bool>,
    fixed: HashSet<(usize, usize)>,
    clip: Option<Vec<Vector3>>,
}

// ponytail: every step scans all triangles or edges (quadratic on the
// whole build); a spatial index and kept adjacency when surfaces of many
// thousand points are written.
impl Mesh {
    fn edges(&self) -> HashMap<(usize, usize), usize> {
        let mut m = HashMap::new();
        for (t, tri) in self.triangles.iter().enumerate() {
            for k in 0..3 {
                m.insert((tri[k], tri[(k + 1) % 3]), t);
            }
        }
        m
    }

    fn locate(&self, p: Vector3) -> Option<usize> {
        self.triangles.iter().position(|&[a, b, c]| {
            let v = &self.vertices;
            orient(v[a], v[b], p) >= -EPS && orient(v[b], v[c], p) >= -EPS && orient(v[c], v[a], p) >= -EPS
        })
    }

    fn vertex_at(&self, p: Vector3) -> Option<usize> {
        let used: HashSet<usize> = self.triangles.iter().flatten().copied().collect();
        (0..self.vertices.len()).find(|i| used.contains(i) && same_plan(self.vertices[*i], p))
    }

    /// Inserts `p`; a vertex already at its plan position takes its
    /// elevation. Delaunay insertion grows the cavity across free edges;
    /// `split` only splits the triangle (or the two at an edge) holding it.
    fn insert(&mut self, p: Vector3, split: bool) -> Option<usize> {
        if let Some(i) = self.vertex_at(p) {
            self.vertices[i].z = p.z;
            return Some(i);
        }
        let start = self.locate(p)?;
        let i = self.vertices.len();
        self.vertices.push(p);
        // A fixed edge through `p` is split.
        let cut: Vec<(usize, usize)> = self.fixed.iter().copied().filter(|&(a, b)| on_segment(self.vertices[a], self.vertices[b], p)).collect();
        for e in &cut {
            self.fixed.remove(e);
        }
        let edges = self.edges();
        let mut cavity = vec![start];
        let mut seen: HashSet<usize> = cavity.iter().copied().collect();
        let mut k = 0;
        while k < cavity.len() {
            let tri = self.triangles[cavity[k]];
            k += 1;
            for e in 0..3 {
                let (a, b) = (tri[e], tri[(e + 1) % 3]);
                let Some(&n) = edges.get(&(b, a)) else { continue };
                if seen.contains(&n) {
                    continue;
                }
                let v = &self.vertices;
                let [x, y, z] = self.triangles[n];
                let on_edge = orient(v[a], v[b], p).abs() < EPS;
                let grow = if split { on_edge } else { cut.contains(&key(a, b)) || on_edge || (!self.fixed.contains(&key(a, b)) && in_circle(v[x], v[y], v[z], p)) };
                if grow {
                    seen.insert(n);
                    cavity.push(n);
                }
            }
        }
        let hidden = cavity.iter().all(|t| self.hidden[*t]);
        let mut border = Vec::new();
        for t in &cavity {
            let tri = self.triangles[*t];
            for e in 0..3 {
                let (a, b) = (tri[e], tri[(e + 1) % 3]);
                if !edges.get(&(b, a)).is_some_and(|n| seen.contains(n)) {
                    border.push((a, b));
                }
            }
        }
        cavity.sort_unstable();
        for t in cavity.iter().rev() {
            self.triangles.swap_remove(*t);
            self.hidden.swap_remove(*t);
        }
        for (a, b) in border {
            if orient(self.vertices[a], self.vertices[b], p).abs() > EPS {
                self.triangles.push([a, b, i]);
                self.hidden.push(hidden);
            }
        }
        for (a, b) in cut {
            self.fixed.insert(key(a, i));
            self.fixed.insert(key(i, b));
        }
        Some(i)
    }

    /// Flips the edge `a b` of triangles `[a, b, x]` and `[b, a, y]` to `x y`.
    fn flip(&mut self, edges: &HashMap<(usize, usize), usize>, a: usize, b: usize) -> Option<(usize, usize)> {
        let (t1, t2) = (*edges.get(&(a, b))?, *edges.get(&(b, a))?);
        let x = *self.triangles[t1].iter().find(|v| **v != a && **v != b)?;
        let y = *self.triangles[t2].iter().find(|v| **v != a && **v != b)?;
        let v = &self.vertices;
        // The quad x a y b must be convex.
        if orient(v[x], v[a], v[y]) <= EPS || orient(v[y], v[b], v[x]) <= EPS {
            return None;
        }
        let hidden = self.hidden[t1] && self.hidden[t2];
        self.triangles[t1] = [x, a, y];
        self.triangles[t2] = [y, b, x];
        self.hidden[t1] = hidden;
        self.hidden[t2] = hidden;
        Some((x, y))
    }

    /// Lawson flips from `queue` until those edges and the ones the flips
    /// make are Delaunay.
    fn legalize(&mut self, mut queue: Vec<(usize, usize)>) {
        let mut budget = 100_000;
        while let Some((a, b)) = queue.pop() {
            budget -= 1;
            if budget == 0 || self.fixed.contains(&key(a, b)) {
                continue;
            }
            let edges = self.edges();
            let (Some(&t), Some(&n)) = (edges.get(&(a, b)), edges.get(&(b, a))) else { continue };
            let Some(&y) = self.triangles[n].iter().find(|v| **v != a && **v != b) else { continue };
            let [p, q, r] = self.triangles[t];
            let v = &self.vertices;
            if !in_circle(v[p], v[q], v[r], v[y]) {
                continue;
            }
            if let Some((x, y)) = self.flip(&edges, a, b) {
                queue.extend([(x, a), (a, y), (y, b), (b, x)]);
            }
        }
    }

    /// Makes `a b` a fixed edge by flipping the edges that cross it. A
    /// line through another vertex is left as the triangulation has it,
    /// as the application does.
    fn force(&mut self, a: usize, b: usize) {
        if a == b {
            return;
        }
        let used: HashSet<usize> = self.triangles.iter().flatten().copied().collect();
        if used.iter().any(|m| *m != a && *m != b && on_segment(self.vertices[a], self.vertices[b], self.vertices[*m])) {
            return;
        }
        let mut made = Vec::new();
        for _ in 0..10_000 {
            let edges = self.edges();
            if edges.contains_key(&(a, b)) || edges.contains_key(&(b, a)) {
                self.fixed.insert(key(a, b));
                self.legalize(made);
                return;
            }
            let (pa, pb) = (self.vertices[a], self.vertices[b]);
            let crossing: Vec<(usize, usize)> = edges.keys().filter(|(u, v)| u < v && crosses(pa, pb, self.vertices[*u], self.vertices[*v])).copied().collect();
            let flipped = crossing.iter().find_map(|(u, v)| if self.fixed.contains(&key(*u, *v)) { None } else { self.flip(&edges, *u, *v) });
            match flipped {
                Some(e) => made.push(e),
                // Only fixed or unflippable edges cross: the line is not forced.
                None => return,
            }
        }
    }

    /// Elevation of the drawn surface at `p` (plan), from the triangle holding it.
    fn elevation(&self, p: Vector2) -> Option<f64> {
        let q = Vector3::new(p.x, p.y, 0.0);
        let [a, b, c] = self.triangles[self.locate(q)?];
        let (a, b, c) = (self.vertices[a], self.vertices[b], self.vertices[c]);
        let area = orient(a, b, c);
        let (wa, wb) = (orient(b, c, q) / area, orient(c, a, q) / area);
        Some(wa * a.z + wb * b.z + (1.0 - wa - wb) * c.z)
    }

    /// Lays `line` (closed when `closed`) on the drawn surface without
    /// moving its edges: from each vertex to the next, the line's points
    /// and its crossings with the edges as they are when it gets there
    /// split the triangles, each at the elevation of the surface there.
    fn drape(&mut self, line: &[Vector3], closed: bool) {
        let n = if closed { line.len() } else { line.len().saturating_sub(1) };
        let mut last: Option<usize> = None;
        for k in 0..n {
            let (a, b) = (line[k], line[(k + 1) % line.len()]);
            let mut at = a;
            for _ in 0..10_000 {
                let Some(z) = self.elevation(Vector2::new(at.x, at.y)) else { break };
                let i = self.insert(Vector3::new(at.x, at.y, z), true);
                if let (Some(p), Some(i)) = (last, i) {
                    if p != i {
                        self.fixed.insert(key(p, i));
                    }
                }
                last = i.or(last);
                let edges = self.edges();
                let next = edges.keys().filter(|(u, v)| u < v).filter_map(|&(u, v)| {
                    let (c, d) = (self.vertices[u], self.vertices[v]);
                    crosses(at, b, c, d).then(|| {
                        let t = orient(c, d, at) / (orient(c, d, at) - orient(c, d, b));
                        (t, Vector3::new(at.x + (b.x - at.x) * t, at.y + (b.y - at.y) * t, 0.0))
                    })
                }).min_by(|x, y| x.0.total_cmp(&y.0));
                match next {
                    Some((_, p)) => at = p,
                    None if !same_plan(at, b) && (k + 1 < n || !closed) => at = b,
                    None => break,
                }
            }
        }
    }

    /// Breaklines and contours (proximity: between the nearest vertices;
    /// non-destructive: laid on the surface).
    fn lines(&mut self, lines: &TinLines, kind: Option<TinBreaklineType>) {
        for line in &lines.polylines {
            if self.clip.as_ref().is_some_and(|c| line.iter().any(|p| !inside(c, *p))) {
                continue;
            }
            let mut ids = Vec::new();
            match kind {
                Some(TinBreaklineType::NonDestructive) => {
                    self.drape(line, false);
                    continue;
                }
                Some(TinBreaklineType::Proximity) => {
                    let used: HashSet<usize> = self.triangles.iter().flatten().copied().filter(|v| *v >= 4).collect();
                    for p in line {
                        let d = |i: &usize| (self.vertices[*i].x - p.x).powi(2) + (self.vertices[*i].y - p.y).powi(2);
                        ids.extend(used.iter().min_by(|m, n| d(m).total_cmp(&d(n))).copied());
                    }
                }
                _ => {
                    for (k, p) in line.iter().enumerate() {
                        if k > 0 && lines.maximum_distance > 0.0 {
                            let q = line[k - 1];
                            let n = ((((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt() / lines.maximum_distance).ceil() as usize).max(1);
                            for s in 1..n {
                                let t = s as f64 / n as f64;
                                let at = Vector3::new(q.x + (p.x - q.x) * t, q.y + (p.y - q.y) * t, q.z + (p.z - q.z) * t);
                                // A supplementing point on a vertex leaves it as it is.
                                ids.extend(self.vertex_at(at).or_else(|| self.insert(at, false)));
                            }
                        }
                        ids.extend(self.insert(*p, false));
                    }
                }
            }
            for w in ids.windows(2) {
                self.force(w[0], w[1]);
            }
        }
    }

    fn boundary(&mut self, boundary_type: TinBoundaryType, non_destructive: bool, lines: &TinLines) {
        for line in &lines.polylines {
            if line.len() < 3 {
                continue;
            }
            let polygon = if same_plan(line[0], line[line.len() - 1]) { line[..line.len() - 1].to_vec() } else { line.clone() };
            if boundary_type == TinBoundaryType::DataClip {
                self.clip = Some(polygon);
                continue;
            }
            if non_destructive {
                self.drape(&polygon, true);
            }
            for t in 0..self.triangles.len() {
                let tri = self.triangles[t].map(|v| self.vertices[v]);
                let centre = Vector3::new((tri[0].x + tri[1].x + tri[2].x) / 3.0, (tri[0].y + tri[1].y + tri[2].y) / 3.0, 0.0);
                let all_in = tri.iter().all(|p| inside(&polygon, *p) || polygon.iter().any(|q| same_plan(*q, *p)));
                let any_in = inside(&polygon, centre);
                match boundary_type {
                    TinBoundaryType::Outer => self.hidden[t] |= if non_destructive { !any_in } else { !all_in },
                    TinBoundaryType::Hide => self.hidden[t] |= if non_destructive { any_in } else { all_in },
                    TinBoundaryType::Show => {
                        if any_in {
                            self.hidden[t] = false;
                        }
                    }
                    TinBoundaryType::DataClip => {}
                }
            }
        }
    }

    /// Removes vertex `i` and fills its hole.
    fn remove(&mut self, i: usize) {
        let around: Vec<usize> = (0..self.triangles.len()).filter(|t| self.triangles[*t].contains(&i)).collect();
        if around.is_empty() {
            return;
        }
        // The hole's outline: the far edge of every triangle around `i`, chained.
        let mut next = HashMap::new();
        let hidden = around.iter().all(|t| self.hidden[*t]);
        for t in &around {
            let tri = self.triangles[*t];
            let Some(k) = tri.iter().position(|v| *v == i) else { continue };
            next.insert(tri[(k + 1) % 3], tri[(k + 2) % 3]);
        }
        let Some(&first) = next.keys().min() else { return };
        let mut ring = vec![first];
        while let Some(&n) = next.get(ring.last().unwrap()) {
            if n == first || ring.len() > next.len() {
                break;
            }
            ring.push(n);
        }
        for t in around.iter().rev() {
            self.triangles.swap_remove(*t);
            self.hidden.swap_remove(*t);
        }
        self.fixed.retain(|(a, b)| *a != i && *b != i);
        let mut made = Vec::new();
        // Ears in order of the power of the removed point to their circle
        // (the highest is a Delaunay ear), the first in the outline on a tie.
        let removed = self.vertices[i];
        while ring.len() > 3 {
            let n = ring.len();
            let v = &self.vertices;
            let power = |k: usize| -> Option<f64> {
                let (a, b, c) = (v[ring[(k + n - 1) % n]], v[ring[k]], v[ring[(k + 1) % n]]);
                if orient(a, b, c) <= EPS || ring.iter().any(|p| ![ring[(k + n - 1) % n], ring[k], ring[(k + 1) % n]].contains(p) && orient(a, b, v[*p]) > 0.0 && orient(b, c, v[*p]) > 0.0 && orient(c, a, v[*p]) > 0.0) {
                    return None;
                }
                let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
                let (a2, b2, c2) = (a.x * a.x + a.y * a.y, b.x * b.x + b.y * b.y, c.x * c.x + c.y * c.y);
                let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
                let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
                let r2 = (a.x - ux).powi(2) + (a.y - uy).powi(2);
                Some((removed.x - ux).powi(2) + (removed.y - uy).powi(2) - r2)
            };
            let Some(k) = (0..n).filter_map(|k| power(k).map(|p| (k, p))).fold(None, |best: Option<(usize, f64)>, (k, p)| match best {
                Some((_, b)) if b >= p - 1e-9 => best,
                _ => Some((k, p)),
            }).map(|b| b.0) else {
                break;
            };
            let (a, c) = (ring[(k + n - 1) % n], ring[(k + 1) % n]);
            self.triangles.push([a, ring[k], c]);
            self.hidden.push(hidden);
            made.push((a, c));
            ring.remove(k);
        }
        if ring.len() == 3 {
            self.triangles.push([ring[0], ring[1], ring[2]]);
            self.hidden.push(hidden);
        }
        self.legalize(made);
    }

    /// The edge nearest plan point `p`.
    fn edge_at(&self, p: Vector2) -> Option<(usize, usize)> {
        let q = Vector3::new(p.x, p.y, 0.0);
        let dist = |u: usize, v: usize| {
            let (s, e) = (self.vertices[u], self.vertices[v]);
            let len2 = (e.x - s.x).powi(2) + (e.y - s.y).powi(2);
            let t = (((q.x - s.x) * (e.x - s.x) + (q.y - s.y) * (e.y - s.y)) / len2).clamp(0.0, 1.0);
            (s.x + (e.x - s.x) * t - q.x).hypot(s.y + (e.y - s.y) * t - q.y)
        };
        self.edges().keys().copied().filter(|(a, b)| a < b).min_by(|(a, b), (c, d)| dist(*a, *b).total_cmp(&dist(*c, *d)))
    }

    /// Inserts data points, unless a data clip leaves them out.
    fn add(&mut self, p: Vector3) -> Option<usize> {
        if self.clip.as_ref().is_some_and(|c| !inside(c, p)) {
            return None;
        }
        self.insert(p, false)
    }

    fn apply(&mut self, operation: &TinOperation) {
        match operation {
            TinOperation::AddPoints(points) | TinOperation::DrawingObjects { object_type: 0, points, .. } => {
                for p in points {
                    self.add(*p);
                }
            }
            TinOperation::DrawingObjects { points, maintain_edges, .. } => {
                let ids: Vec<Option<usize>> = points.iter().map(|p| self.add(*p)).collect();
                if *maintain_edges {
                    for w in ids.chunks(2) {
                        if let [Some(a), Some(b)] = w {
                            self.force(*a, *b);
                        }
                    }
                }
            }
            // An edit, not data: a data clip does not apply.
            TinOperation::AddVertex(p) => {
                self.insert(*p, false);
            }
            TinOperation::Breaklines(kind, lines) => self.lines(lines, Some(*kind)),
            TinOperation::Contours(lines) => self.lines(lines, None),
            TinOperation::Boundary { boundary_type, non_destructive, lines } => self.boundary(*boundary_type, *non_destructive, lines),
            TinOperation::DeleteVertex(p) => {
                if let Some(i) = self.vertex_at(Vector3::new(p.x, p.y, 0.0)) {
                    self.remove(i);
                }
            }
            TinOperation::MoveVertex { from, to } => {
                if let Some(i) = self.vertex_at(Vector3::new(from.x, from.y, 0.0)) {
                    let z = self.vertices[i].z;
                    self.remove(i);
                    self.insert(Vector3::new(to.x, to.y, z), false);
                }
            }
            TinOperation::VertexElevation(p) => {
                if let Some(i) = self.vertex_at(*p) {
                    self.vertices[i].z = p.z;
                }
            }
            TinOperation::SwapEdge(p) => {
                if let Some((a, b)) = self.edge_at(*p) {
                    let edges = self.edges();
                    if let Some((x, y)) = self.flip(&edges, a, b).or_else(|| self.flip(&edges, b, a)) {
                        self.fixed.insert(key(x, y));
                    }
                }
            }
            TinOperation::AddLine(a, b) => {
                if let (Some(a), Some(b)) = (self.vertex_at(Vector3::new(a.x, a.y, 0.0)), self.vertex_at(Vector3::new(b.x, b.y, 0.0))) {
                    self.force(a, b);
                }
            }
            TinOperation::DeleteLine(p) => {
                if let Some((a, b)) = self.edge_at(*p) {
                    for t in 0..self.triangles.len() {
                        let tri = self.triangles[t];
                        if tri.contains(&a) && tri.contains(&b) {
                            self.hidden[t] = true;
                        }
                    }
                }
            }
            TinOperation::Raise(d) => {
                for v in self.vertices.iter_mut().skip(4) {
                    v.z += d;
                }
            }
            _ => {}
        }
    }
}

/// The triangulation of `definitions` (the four bounding corners first,
/// then the vertices in use) and the hidden flag of each triangle.
pub(super) fn build(definitions: &[TinDefinition]) -> Option<(Triangulation, Vec<bool>)> {
    // Every position a definition brings in spans the corners.
    let mut all: Vec<Vector3> = Vec::new();
    for d in definitions {
        match &d.operation {
            TinOperation::AddPoints(p) | TinOperation::DrawingObjects { points: p, .. } => all.extend(p),
            TinOperation::AddVertex(p) => all.push(*p),
            TinOperation::Breaklines(_, l) | TinOperation::Contours(l) => all.extend(l.polylines.iter().flatten()),
            _ => {}
        }
    }
    if all.is_empty() {
        return None;
    }
    // A first list of points distinct in plan is triangulated in one pass
    // (the same Delaunay triangulation the insertions below make, without
    // their per-point scans of the whole mesh).
    let mut seen = HashSet::new();
    let seed: &[Vector3] = match definitions.first().map(|d| &d.operation) {
        Some(TinOperation::AddPoints(p)) if p.iter().all(|q| seen.insert((q.x.to_bits(), q.y.to_bits()))) => p,
        _ => &[],
    };
    // The corners 100 outside the extents; the lower two at the lowest
    // elevation, the upper two at the highest.
    let start = triangulate_in(&all, seed);
    let (low, high) = all.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(p.z), h.max(p.z)));
    let mut vertices = start.vertices;
    for (k, c) in vertices[..4].iter_mut().enumerate() {
        c.z = if k < 2 { low } else { high };
    }
    let hidden = vec![false; start.triangles.len()];
    let mut mesh = Mesh { vertices, triangles: start.triangles, hidden, fixed: HashSet::new(), clip: None };
    for d in &definitions[usize::from(!seed.is_empty())..] {
        mesh.apply(&d.operation);
    }
    // Keep the corners and the vertices in use, in order.
    let used: HashSet<usize> = mesh.triangles.iter().flatten().copied().collect();
    let mut index = vec![usize::MAX; mesh.vertices.len()];
    let mut vertices = Vec::new();
    for (i, v) in mesh.vertices.iter().enumerate() {
        if i < 4 || used.contains(&i) {
            index[i] = vertices.len();
            vertices.push(*v);
        }
    }
    let triangles: Vec<[usize; 3]> = mesh.triangles.iter().map(|t| t.map(|v| index[v])).collect();
    let hidden = mesh.triangles.iter().zip(&mesh.hidden).map(|(t, h)| *h || t.iter().any(|v| *v < 4)).collect();
    (vertices.len() >= 7).then_some((Triangulation { vertices, triangles }, hidden))
}
