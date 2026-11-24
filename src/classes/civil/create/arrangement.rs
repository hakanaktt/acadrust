//! The planar arrangement a site's members make, built the way the
//! application builds it: member by member, each split where it crosses or
//! touches the lines already there, its segments added one at a time.
//!
//! A half-edge runs from a node to the next one with its face on the
//! right. A face keeps one entry half-edge per boundary loop; the record
//! writes an entry as its start node and the half-edge's index among the
//! node's half-edges sorted by angle (counterclockwise from +x).
//!
//! Face numbering (WP03 to WP17): the outside face is 1 and every split
//! makes the next face. When a segment splits a face, the part that keeps
//! the face's entry keeps the face and the other part becomes the new
//! face, its entry the segment's half-edge on that side. An entry the
//! member split is void: a split face then keeps the part its old entry
//! started in with the segment as its entry, and a face that is not
//! split takes the half-edge that followed the old entry. A loose end's
//! entry (the half-edge that follows is its own twin) keeps its first
//! piece when the member's own point split it, else takes the way back
//! past the member's own points at its start. When a segment
//! closes a loop that bounds a face of its own inside a face, the new face
//! is that bounded part and the outer part stays with the old face.

use crate::types::Vector2;
use std::f64::consts::TAU;

/// One member as it is added: its points (with the bulge of the segment
/// that starts there) and whether its last point joins the first.
pub(super) struct Input<'a> {
    pub path: &'a [(Vector2, Option<f64>)],
    pub elevations: &'a [f64],
    pub closed: bool,
    /// The member is a feature line, a lot line.
    pub feature_line: bool,
    pub lot_line: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Vertex {
    /// Node id (from 1).
    pub node: i32,
    pub bulge: Option<f64>,
    /// A node another member's crossing added to this edge.
    pub inserted: bool,
    /// The point lies on a node an earlier member made.
    pub shared: bool,
    /// Faces (left, right) of the segment that starts here.
    pub faces: (i32, i32),
    /// The segment that starts here runs along another member's: 0x1000
    /// when that member is an earlier one, 0x2000 a later one (WC64).
    pub along: i32,
}

pub(super) struct Edge {
    pub vertices: Vec<Vertex>,
    /// The edge starts at a new node off the earlier members.
    pub loose_start: bool,
    /// Another member's crossing added a node to the edge.
    pub crossed: bool,
}

pub(super) struct Face {
    /// 14, or 6 once a later member changed the face.
    pub flags: i32,
    pub entries: Vec<(i32, i32)>,
    /// A point inside the face (for a parcel's label).
    pub label: Vector2,
    /// The face's boundary loops as straight pieces.
    loops: Loops,
}

pub(super) struct Arrangement {
    /// Node points and elevations, id i + 1.
    pub nodes: Vec<(Vector2, f64)>,
    pub edges: Vec<Edge>,
    /// Faces by number (i + 1, the outside first); the record writes each
    /// under its id in `face_ids` and `next_face` as the next free id.
    pub faces: Vec<Face>,
    pub face_ids: Vec<i32>,
    pub next_face: i32,
    /// Each node's half-edges in angle order (the nodes they lead to), and
    /// each half-edge's face (on its right).
    pub outgoing: Vec<Vec<i32>>,
    pub halves: Vec<((i32, i32), i32)>,
}

type Half = (usize, usize);

#[derive(Clone, Copy)]
enum Entry {
    Live(Half),
    /// Split by the member being added: the half-edge that followed it,
    /// and the old entry's first piece.
    Void(Half, Half),
}

impl Entry {
    fn half(self) -> Half {
        match self {
            Entry::Live(h) | Entry::Void(h, _) => h,
        }
    }
}

struct V {
    node: usize,
    bulge: Option<f64>,
    inserted: bool,
    shared: bool,
}

struct EdgeState {
    vertices: Vec<V>,
    closed: bool,
    feature_line: bool,
    lot_line: bool,
    /// Segments in the graph (all but for the member being added).
    present: usize,
    loose_start: bool,
}

struct FaceState {
    dirty: bool,
    entries: Vec<Entry>,
    id: i32,
}

struct State {
    nodes: Vec<(Vector2, f64)>,
    edges: Vec<EdgeState>,
    faces: Vec<FaceState>,
    next_face: i32,
}

fn segment_count(e: &EdgeState) -> usize {
    if e.closed { e.vertices.len() } else { e.vertices.len() - 1 }
}

fn near(a: Vector2, b: Vector2) -> bool {
    (a - b).length() <= 1e-8 * (1.0 + a.x.abs().max(a.y.abs()))
}

impl State {
    /// Every segment in the graph: (from, to, bulge).
    /// A stretch two members share is one segment.
    fn segments(&self) -> Vec<(usize, usize, f64)> {
        let mut out: Vec<(usize, usize, f64)> = Vec::new();
        for (_, (a, b, g)) in self.member_segments() {
            if !out.iter().any(|&(x, y, _)| (x, y) == (a, b) || (x, y) == (b, a)) {
                out.push((a, b, g));
            }
        }
        out
    }

    /// The segments with the index of the member they belong to.
    fn member_segments(&self) -> Vec<(usize, (usize, usize, f64))> {
        let mut out = Vec::new();
        for (k, e) in self.edges.iter().enumerate() {
            let n = e.vertices.len();
            for i in 0..e.present {
                out.push((k, (e.vertices[i].node, e.vertices[(i + 1) % n].node, e.vertices[i].bulge.unwrap_or(0.0))));
            }
        }
        out
    }

    /// The direction a half-edge leaves `from` in. An arc leaving along
    /// another line's direction (a curb return's arc at its tangent point,
    /// WC64) turns off it: its curvature tips the angle by a hair, to the
    /// left for a left turn.
    fn angle(&self, from: usize, to: usize, bulge: f64) -> f64 {
        let d = self.nodes[to].0 - self.nodes[from].0;
        let curvature = 2.0 * (2.0 * bulge.atan()).sin() / d.length();
        (d.y.atan2(d.x) - 2.0 * bulge.atan() + 1e-9 * curvature.atan()).rem_euclid(TAU)
    }

    /// The half-edges leaving `node`, sorted by angle: (to, angle).
    fn outgoing(&self, segments: &[(usize, usize, f64)], node: usize) -> Vec<(usize, f64)> {
        let mut out: Vec<(usize, f64)> = Vec::new();
        for &(a, b, g) in segments {
            if a == node {
                out.push((b, self.angle(a, b, g)));
            }
            if b == node {
                out.push((a, self.angle(b, a, -g)));
            }
        }
        out.sort_by(|x, y| x.1.total_cmp(&y.1));
        out
    }

    fn next(&self, segments: &[(usize, usize, f64)], h: Half) -> Half {
        let out = self.outgoing(segments, h.1);
        let r = out.iter().position(|o| o.0 == h.0).unwrap_or(0);
        (h.1, out[(r + 1) % out.len()].0)
    }

    fn cycle(&self, segments: &[(usize, usize, f64)], h: Half) -> Vec<Half> {
        let mut out = vec![h];
        let mut c = self.next(segments, h);
        while c != h && out.len() <= 2 * segments.len() + 2 {
            out.push(c);
            c = self.next(segments, c);
        }
        out
    }

    /// A cycle's outline with its arcs divided.
    fn outline(&self, segments: &[(usize, usize, f64)], cycle: &[Half]) -> Vec<(Vector2, Vector2)> {
        face_loops(self, segments, &[cycle.to_vec()]).pop().unwrap_or_default()
    }

    /// Twice the signed area a cycle encloses (arcs included): negative
    /// for the outline of a bounded face.
    fn area(&self, segments: &[(usize, usize, f64)], cycle: &[Half]) -> f64 {
        loop_area(&self.outline(segments, cycle))
    }

    /// The face whose loop holds `h`.
    fn face_of(&self, segments: &[(usize, usize, f64)], h: Half) -> Option<usize> {
        let cycle = self.cycle(segments, h);
        self.faces.iter().position(|f| f.entries.iter().any(|e| cycle.contains(&e.half())))
    }

    /// The face around a point off every line: inside the smallest bounded
    /// loop holding it, else the outside face.
    fn face_at(&self, segments: &[(usize, usize, f64)], p: Vector2) -> usize {
        let mut best: Option<(f64, usize)> = None;
        for (i, f) in self.faces.iter().enumerate() {
            for e in &f.entries {
                let cycle = self.cycle(segments, e.half());
                let pieces = self.outline(segments, &cycle);
                let area = loop_area(&pieces);
                if area >= 0.0 {
                    continue;
                }
                let inside = inside_loop(&pieces, p);
                if inside && best.is_none_or(|b| -area < b.0) {
                    best = Some((-area, i));
                }
            }
        }
        best.map_or(0, |b| b.1)
    }

    /// The face at `node` in the wedge a new half-edge at `angle` enters.
    fn wedge(&self, segments: &[(usize, usize, f64)], node: usize, angle: f64) -> Option<usize> {
        let out = self.outgoing(segments, node);
        let w = out.iter().find(|o| o.1 > angle).or(out.first())?;
        self.face_of(segments, (node, w.0))
    }
}

/// A straight or arc piece from `p` to `q` (`g` its bulge, 0 straight).
#[derive(Clone, Copy)]
struct Piece {
    p: Vector2,
    q: Vector2,
    g: f64,
}

impl Piece {
    /// An arc's centre, radius, start angle and signed sweep.
    fn arc(&self) -> Option<(Vector2, f64, f64, f64)> {
        if self.g == 0.0 {
            return None;
        }
        let sweep = 4.0 * self.g.atan();
        let d = self.q - self.p;
        let normal = Vector2::new(-d.y, d.x) * (1.0 / d.length());
        let centre = (self.p + self.q) * 0.5 + normal * (d.length() / 2.0 / (sweep / 2.0).tan());
        let r = (self.p - centre).length();
        Some((centre, r, (self.p.y - centre.y).atan2(self.p.x - centre.x), sweep))
    }

    /// The parameter (0 at `p`, 1 at `q`; by angle along an arc) of a point
    /// on the piece's line or circle, `None` off it.
    fn param_of(&self, x: Vector2) -> Option<f64> {
        let scale = 1.0 + (self.q - self.p).length() + (x - self.p).length();
        match self.arc() {
            None => {
                let d = self.q - self.p;
                (d.cross(&(x - self.p)).abs() <= 1e-9 * d.length() * scale).then(|| param(self.p, self.q, x))
            }
            Some((c, r, a0, sweep)) => {
                if ((x - c).length() - r).abs() > 1e-9 * scale {
                    return None;
                }
                let a = (x.y - c.y).atan2(x.x - c.x);
                let along = if sweep > 0.0 { (a - a0).rem_euclid(TAU) } else { -(a0 - a).rem_euclid(TAU) };
                let t = along / sweep;
                // Just before the start a full turn reads as past the end.
                Some(if t > 1.0 && t - TAU / sweep.abs() > -1e-6 { t - TAU / sweep.abs() } else { t })
            }
        }
    }
}

/// Where two pieces meet: (parameter along `a`, along `b`, point); a
/// stretch they share is refused.
fn meet(a: Piece, b: Piece) -> Result<Vec<(f64, f64, Vector2)>, &'static str> {
    let tol = 1e-9;
    let within = |t: f64| (-tol..=1.0 + tol).contains(&t);
    let mut points: Vec<Vector2> = Vec::new();
    match (a.arc(), b.arc()) {
        (None, None) => {
            let (p, q, pa, pb) = (a.p, a.q, b.p, b.q);
            let (d, e) = (q - p, pb - pa);
            let den = d.cross(&e);
            let scale = d.length() * e.length();
            if den.abs() <= 1e-12 * scale {
                // Lines along each other meet at the ends of the stretch
                // they share (each end of one that lies on the other): both
                // are split there and the stretch is one segment of the
                // graph that both edges list (WC64).
                let mut out: Vec<(f64, f64, Vector2)> = Vec::new();
                if (pa - p).cross(&d).abs() <= 1e-9 * scale.max(1.0) {
                    for (t, u, x) in [(0.0, param(pa, pb, p), p), (1.0, param(pa, pb, q), q), (param(p, q, pa), 0.0, pa), (param(p, q, pb), 1.0, pb)] {
                        if within(t) && within(u) && !out.iter().any(|o| near(o.2, x)) {
                            out.push((t, u, x));
                        }
                    }
                }
                return Ok(out);
            }
            let t = (pa - p).cross(&e) / den;
            let u = (pa - p).cross(&d) / den;
            return Ok(if within(t) && within(u) { vec![(t, u, p + d * t)] } else { Vec::new() });
        }
        (Some((c, r, ..)), None) | (None, Some((c, r, ..))) => {
            let line = if a.g == 0.0 { a } else { b };
            let d = line.q - line.p;
            let f = line.p - c;
            let (qa, qb, qc) = (d.dot(&d), 2.0 * f.dot(&d), f.dot(&f) - r * r);
            let disc = qb * qb - 4.0 * qa * qc;
            if disc >= -1e-12 * qa * r * r {
                // A line touching the circle (a curb return's arc at its
                // tangent point) meets it once: rounding splits the double
                // root by up to about a millionth of the radius.
                let root = disc.max(0.0).sqrt();
                let roots = if root / (2.0 * qa.sqrt()) <= 1e-5 * r { vec![-qb / (2.0 * qa)] } else { vec![(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)] };
                for k in roots {
                    points.push(line.p + d * k);
                }
            }
        }
        (Some((c1, r1, ..)), Some((c2, r2, ..))) => {
            let d = c2 - c1;
            let dist = d.length();
            if dist <= 1e-9 * (r1 + r2) {
                if (r1 - r2).abs() <= 1e-9 * r1 && [b.p, b.q].iter().any(|x| a.param_of(*x).is_some_and(|t| t > tol && t < 1.0 - tol)) {
                    return Err("a member runs along another");
                }
                return Ok(Vec::new());
            }
            let along = (r1 * r1 - r2 * r2 + dist * dist) / (2.0 * dist);
            let h2 = r1 * r1 - along * along;
            if h2 >= -1e-9 * r1 * r1 {
                let h = h2.max(0.0).sqrt();
                let m = c1 + d * (along / dist);
                let n = Vector2::new(-d.y, d.x) * (1.0 / dist);
                if h <= 1e-5 * r1.min(r2) {
                    points.push(m);
                } else {
                    points.push(m + n * h);
                    points.push(m - n * h);
                }
            }
        }
    }
    let mut out: Vec<(f64, f64, Vector2)> = Vec::new();
    for x in points {
        // An end within tolerance is the end itself.
        let x = [a.p, a.q, b.p, b.q].into_iter().find(|e| near(*e, x)).unwrap_or(x);
        if let (Some(t), Some(u)) = (a.param_of(x), b.param_of(x)) {
            if within(t) && within(u) && !out.iter().any(|o| near(o.2, x)) {
                out.push((t, u, x));
            }
        }
    }
    Ok(out)
}

/// Whether two paths (points with the bulge of the segment that starts
/// there) cross, touch or run along each other.
pub(super) fn paths_meet(a: &[(Vector2, Option<f64>)], b: &[(Vector2, Option<f64>)]) -> bool {
    let pieces = |p: &[(Vector2, Option<f64>)]| -> Vec<Piece> { p.windows(2).map(|w| Piece { p: w[0].0, q: w[1].0, g: w[0].1.unwrap_or(0.0) }).collect() };
    let (pa, pb) = (pieces(a), pieces(b));
    pa.iter().any(|x| pb.iter().any(|y| meet(*x, *y).map_or(true, |m| !m.is_empty())))
}

/// The bulge of the part of an arc of bulge `g` from parameter `from` to
/// `to` (fractions of its sweep).
fn part_bulge(g: f64, from: f64, to: f64) -> f64 {
    if from == 0.0 && to == 1.0 {
        return g;
    }
    (g.atan() * (to - from)).tan()
}

/// A new node `x` on segment `a`-`b`: its parameter along the segment.
fn param(a: Vector2, b: Vector2, x: Vector2) -> f64 {
    let d = b - a;
    (x - a).dot(&d) / d.dot(&d)
}

/// Builds the arrangement of `members` in order. Only straight segments
/// may meet; a member must not cross itself or run along another.
pub(super) fn build(members: &[Input]) -> Result<Arrangement, &'static str> {
    let mut s = State { nodes: Vec::new(), edges: Vec::new(), faces: Vec::new(), next_face: 1 };
    for m in members {
        let old_faces = s.faces.len();
        let old_nodes = s.nodes.len();
        let crossed_segments = s.member_segments();
        let old_segments: Vec<(usize, usize, f64)> = crossed_segments.iter().map(|x| x.1).collect();
        let n = m.path.len();
        // Own nodes: an open member's ends, then its inner points; a closed
        // one's in path order. A point on an earlier node takes that node.
        let mut own = vec![usize::MAX; n];
        let mut shared = vec![false; n];
        let order: Vec<usize> = if m.closed { (0..n).collect() } else { [0, n - 1].into_iter().chain(1..n - 1).collect() };
        for i in order {
            let p = m.path[i].0;
            if let Some(k) = (0..old_nodes).find(|k| near(s.nodes[*k].0, p)) {
                own[i] = k;
                shared[i] = true;
            } else {
                own[i] = s.nodes.len();
                s.nodes.push((p, m.elevations[i]));
            }
        }
        // Where the member meets the earlier segments.
        let segs = if m.closed { n } else { n - 1 };
        let mut on_new: Vec<Vec<(f64, usize)>> = vec![Vec::new(); segs];
        let mut on_old: Vec<(usize, usize, f64, usize)> = Vec::new();
        let mut crossings: Vec<(usize, usize, f64, Vector2)> = Vec::new();
        for i in 0..segs {
            let piece = Piece { p: m.path[i].0, q: m.path[(i + 1) % n].0, g: m.path[i].1.unwrap_or(0.0) };
            for &(member, (a, b, g)) in &crossed_segments {
                let old = Piece { p: s.nodes[a].0, q: s.nodes[b].0, g };
                for (t, u, x) in meet(piece, old)? {
                    let tol = 1e-9;
                    let inner_t = t > tol && t < 1.0 - tol;
                    let inner_u = u > tol && u < 1.0 - tol;
                    match (inner_t, inner_u) {
                        (false, false) => {}
                        (false, true) => {
                            // An own point on an earlier segment.
                            let node = own[if t < 0.5 { i } else { (i + 1) % n }];
                            on_old.push((a, b, u, node));
                        }
                        (true, false) => {
                            let node = if u < 0.5 { a } else { b };
                            if !on_new[i].iter().any(|v| v.1 == node) {
                                on_new[i].push((t, node));
                            }
                        }
                        (true, true) => crossings.push((member, i, t, x)),
                    }
                }
            }
        }
        // Crossing nodes take the next ids by the member crossed, then in
        // the new member's order (FX04: a feature line crossing a later
        // one first gives the earlier one's crossing the lower id).
        crossings.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)));
        let mut placed: Vec<(Vector2, usize)> = Vec::new();
        for &(_, i, t, x) in &crossings {
            let node = match placed.iter().find(|p| near(p.0, x)) {
                Some(p) => p.1,
                None => {
                    // A crossing takes the new member's elevation there.
                    let (z0, z1) = (m.elevations[i], m.elevations[(i + 1) % n]);
                    s.nodes.push((x, z0 + (z1 - z0) * t));
                    placed.push((x, s.nodes.len() - 1));
                    s.nodes.len() - 1
                }
            };
            if !on_new[i].iter().any(|v| v.1 == node) {
                on_new[i].push((t, node));
            }
        }
        for &(_, _, _, x) in &crossings {
            let node = placed.iter().find(|p| near(p.0, x)).map(|p| p.1).ok_or("a crossing has no node")?;
            for &(a, b, g) in &old_segments {
                if a != node && b != node {
                    let old = Piece { p: s.nodes[a].0, q: s.nodes[b].0, g };
                    if let Some(u) = old.param_of(x) {
                        if u > 1e-9 && u < 1.0 - 1e-9 && !on_old.iter().any(|o| o.0 == a && o.1 == b && o.3 == node) {
                            on_old.push((a, b, u, node));
                        }
                    }
                }
            }
        }
        // A node a stretch of two old edges takes once.
        {
            let mut seen: Vec<(usize, usize, usize)> = Vec::new();
            on_old.retain(|o| {
                let key = (o.0.min(o.1), o.0.max(o.1), o.3);
                let fresh = !seen.contains(&key);
                seen.push(key);
                fresh
            });
        }
        // Entries the splits void take the half-edge after them.
        let split: Vec<(usize, usize)> = {
            let mut v: Vec<(usize, usize)> = on_old.iter().map(|o| (o.0, o.1)).collect();
            v.sort();
            v.dedup();
            v
        };
        let is_split = |h: Half| split.iter().any(|&(a, b)| (a, b) == h || (b, a) == h);
        // The nodes the splits put on `h`, from its start.
        let along = |h: Half| -> Vec<usize> {
            let mut v: Vec<(f64, usize)> = on_old.iter().filter_map(|o| if (o.0, o.1) == h { Some((o.2, o.3)) } else if (o.1, o.0) == h { Some((1.0 - o.2, o.3)) } else { None }).collect();
            v.sort_by(|x, y| x.0.total_cmp(&y.0));
            v.into_iter().map(|x| x.1).collect()
        };
        let first_piece = |h: Half| -> Half { along(h).first().map_or(h, |&x| (h.0, x)) };
        for f in 0..s.faces.len() {
            for k in 0..s.faces[f].entries.len() {
                if let Entry::Live(h) = s.faces[f].entries[k] {
                    if is_split(h) {
                        let after = s.next(&old_segments, h);
                        let nodes = along(h);
                        s.faces[f].entries[k] = if after != (h.1, h.0) {
                            Entry::Void(first_piece(after), first_piece(h))
                        } else if own.contains(&nodes[0]) {
                            // A loose end the member's own point splits
                            // keeps its first piece (PE01, PC11, PC20, PC23).
                            Entry::Live((h.0, nodes[0]))
                        } else {
                            // Else the way back, past the member's own points
                            // at its start (PA01, PC21, PC24).
                            let back = along(after);
                            let skip = back.iter().take_while(|x| own.contains(x)).count();
                            let from = if skip == 0 { after.0 } else { back[skip - 1] };
                            Entry::Void((from, back.get(skip).copied().unwrap_or(after.1)), first_piece(h))
                        };
                    }
                }
            }
        }
        // Split the earlier edges.
        for e in s.edges.iter_mut() {
            let mut out: Vec<V> = Vec::new();
            let len = e.vertices.len();
            for i in 0..len {
                let (a, b) = (e.vertices[i].node, e.vertices[(i + 1) % len].node);
                out.push(V { ..e.vertices[i] });
                if i + 1 == len && !e.closed {
                    break;
                }
                let mut add: Vec<(f64, usize)> = on_old.iter().filter_map(|o| if (o.0, o.1) == (a, b) { Some((o.2, o.3)) } else if (o.1, o.0) == (a, b) { Some((1.0 - o.2, o.3)) } else { None }).collect();
                add.sort_by(|x, y| x.0.total_cmp(&y.0));
                add.dedup_by_key(|x| x.1);
                // An arc's pieces keep its centre: each its share of the sweep.
                let g = e.vertices[i].bulge;
                let ends: Vec<f64> = add.iter().map(|x| x.0).chain([1.0]).collect();
                if let Some(last) = out.last_mut() {
                    last.bulge = g.map(|g| part_bulge(g, 0.0, ends[0]));
                }
                for (k, (u, node)) in add.into_iter().enumerate() {
                    out.push(V { node, bulge: g.map(|g| part_bulge(g, u, ends[k + 1])), inserted: true, shared: false });
                }
            }
            e.vertices = out;
            e.present = segment_count(e);
        }
        // The member's own edge with the nodes it meets.
        let mut vertices = Vec::new();
        for i in 0..n {
            vertices.push(V { node: own[i], bulge: m.path[i].1, inserted: false, shared: shared[i] });
            if i < segs {
                let mut add = on_new[i].clone();
                add.sort_by(|x, y| x.0.total_cmp(&y.0));
                let g = m.path[i].1;
                let ends: Vec<f64> = add.iter().map(|x| x.0).chain([1.0]).collect();
                if let Some(last) = vertices.last_mut() {
                    last.bulge = g.map(|g| part_bulge(g, 0.0, ends[0]));
                }
                for (k, (t, node)) in add.into_iter().enumerate() {
                    vertices.push(V { node, bulge: g.map(|g| part_bulge(g, t, ends[k + 1])), inserted: true, shared: node < old_nodes });
                }
            }
        }
        let on_earlier = |node: usize| node < old_nodes || on_old.iter().any(|o| o.3 == node);
        let loose_start = !s.faces.is_empty() && !on_earlier(own[0]);
        // Two pieces between the same two nodes (a line crossing an arc
        // twice) are not written; a stretch two lines share is.
        {
            let len = vertices.len();
            let count = if m.closed { len } else { len - 1 };
            let mut pairs: Vec<(usize, usize, bool)> = s.member_segments().iter().map(|&(_, (a, b, g))| (a.min(b), a.max(b), g == 0.0)).collect();
            pairs.extend((0..count).map(|i| {
                let (a, b) = (vertices[i].node, vertices[(i + 1) % len].node);
                (a.min(b), a.max(b), vertices[i].bulge.unwrap_or(0.0) == 0.0)
            }));
            pairs.sort();
            if pairs.windows(2).any(|w| (w[0].0, w[0].1) == (w[1].0, w[1].1) && !(w[0].2 && w[1].2)) {
                return Err("two lines meet twice between the same points");
            }
        }
        s.edges.push(EdgeState { vertices, closed: m.closed, feature_line: m.feature_line, lot_line: m.lot_line, present: 0, loose_start });
        let me = s.edges.len() - 1;
        let total = segment_count(&s.edges[me]);
        // The faces the member's segments go into or make (not the outside).
        let mut touched: Vec<usize> = Vec::new();
        for j in 0..total {
            let segments = s.segments();
            let len = s.edges[me].vertices.len();
            let a = s.edges[me].vertices[j].node;
            let b = s.edges[me].vertices[(j + 1) % len].node;
            let g = s.edges[me].vertices[j].bulge.unwrap_or(0.0);
            // A stretch along an earlier line is in the graph already.
            if segments.iter().any(|&(x, y, _)| (x, y) == (a, b) || (x, y) == (b, a)) {
                s.edges[me].present = j + 1;
                continue;
            }
            let a_free = s.outgoing(&segments, a).is_empty();
            let b_free = s.outgoing(&segments, b).is_empty();
            let face = if s.faces.is_empty() {
                s.faces.push(FaceState { dirty: false, entries: vec![Entry::Live((a, b))], id: s.next_face });
                s.next_face += 1;
                0
            } else if a_free && b_free {
                let f = s.face_at(&segments, s.nodes[a].0);
                s.faces[f].entries.push(Entry::Live((a, b)));
                f
            } else if !a_free {
                s.wedge(&segments, a, s.angle(a, b, g)).ok_or("a face cannot be found")?
            } else {
                s.wedge(&segments, b, s.angle(b, a, -g)).ok_or("a face cannot be found")?
            };
            let earlier = face < old_faces;
            if face > 0 && !touched.contains(&face) {
                touched.push(face);
            }
            s.edges[me].present = j + 1;
            let segments = s.segments();
            let right = s.cycle(&segments, (a, b));
            if right.contains(&(b, a)) {
                // No split: a loose end, or a link between two loops.
                if !a_free && !b_free {
                    let entries = &mut s.faces[face].entries;
                    let mut seen = false;
                    entries.retain(|e| {
                        let here = right.contains(&e.half());
                        let keep = !(here && seen);
                        seen |= here;
                        keep
                    });
                }
                if earlier && ((a_free != b_free && (on_earlier(a) || on_earlier(b))) || (!a_free && !b_free)) {
                    s.faces[face].dirty = true;
                }
                continue;
            }
            let left = s.cycle(&segments, (b, a));
            let (ar, al) = (s.area(&segments, &right), s.area(&segments, &left));
            let new_entry;
            if ar < 0.0 && al < 0.0 {
                let k = s.faces[face].entries.iter().position(|e| right.contains(&e.half()) || left.contains(&e.half()));
                // Where the face's first segment in member order is an
                // alignment's, the part along it keeps the face (PC12,
                // PC12r, PC12l, PG1 to PG3: the alignment's piece west of
                // the split, whichever part the entry is in); else the
                // part with the entry (PB03, WP19, PD20).
                let first_left = s
                    .member_segments()
                    .iter()
                    .find_map(|&(k, (p, q, _))| {
                        let (r, l) = (right.contains(&(p, q)) || right.contains(&(q, p)), left.contains(&(p, q)) || left.contains(&(q, p)));
                        (r != l).then_some((k, l))
                    })
                    .filter(|(k, _)| !s.edges[*k].lot_line && !s.edges[*k].feature_line)
                    .map(|(_, l)| l);
                match k.map(|k| (k, s.faces[face].entries[k])) {
                    Some((_, Entry::Live(h))) if first_left.is_none_or(|l| l == left.contains(&h)) => new_entry = if left.contains(&h) { (a, b) } else { (b, a) },
                    // The keeper takes the segment as its entry.
                    Some((k, Entry::Live(_))) => {
                        let keeper_left = first_left == Some(true);
                        s.faces[face].entries[k] = Entry::Live(if keeper_left { (b, a) } else { (a, b) });
                        new_entry = if keeper_left { (a, b) } else { (b, a) };
                    }
                    // The face keeps the part its old entry started in,
                    // with the segment as its entry (PB01, PB01r, PB02).
                    Some((k, Entry::Void(_, first))) if left.contains(&first) => {
                        s.faces[face].entries[k] = Entry::Live((b, a));
                        new_entry = (a, b);
                    }
                    Some((k, Entry::Void(..))) => {
                        s.faces[face].entries[k] = Entry::Live((a, b));
                        new_entry = (b, a);
                    }
                    None => return Err("a split face has no entry"),
                }
            } else {
                // A loop closed inside the face: its inside is the new face.
                let (inner, outer, inner_half) = if ar < 0.0 { (&right, &left, (a, b)) } else { (&left, &right, (b, a)) };
                for e in s.faces[face].entries.iter_mut() {
                    if inner.contains(&e.half()) {
                        // From the entry's node, or the first node after it
                        // along the loop that the outer side passes (a line
                        // hanging inside the loop).
                        let at = inner.iter().position(|h| *h == e.half()).unwrap_or(0);
                        let h = (0..inner.len())
                            .find_map(|k| {
                                let from = inner[(at + k) % inner.len()].0;
                                outer.iter().find(|h| h.0 == from).copied()
                            })
                            .ok_or("a loop has no outer side")?;
                        *e = Entry::Live(h);
                    }
                }
                new_entry = inner_half;
            }
            if earlier {
                s.faces[face].dirty = true;
            }
            s.faces.push(FaceState { dirty: false, entries: vec![Entry::Live(new_entry)], id: s.next_face });
            s.next_face += 1;
            touched.push(s.faces.len() - 1);
        }
        for f in s.faces.iter_mut() {
            for e in f.entries.iter_mut() {
                *e = Entry::Live(e.half());
            }
        }
        if m.feature_line {
            renumber_faces(&mut s, me, &touched);
        }
    }
    finish(&s)
}

/// A feature line `me` that meets lot lines makes the application number
/// the faces it went into or made again (the outside keeps 1): walking each
/// lot line from its first point on a feature line, the faces its segments
/// border in that order, then the others, take the next ids after those
/// its splits took (PD01: split 3, then 4 and 5; PD03: a line ending inside
/// the face, 2 becomes 3; PD08: splits 6 and 7, then 8 to 11 round the
/// rectangle from (50, 0); PD09: the triangle 4, the rest 5).
fn renumber_faces(s: &mut State, me: usize, touched: &[usize]) {
    let mine: Vec<usize> = s.edges[me].vertices.iter().map(|v| v.node).collect();
    let on_lot_lines = |s: &State, node: usize| s.edges.iter().any(|e| e.lot_line && e.vertices.iter().any(|v| v.node == node));
    if touched.is_empty() || !mine.iter().any(|n| on_lot_lines(s, *n)) {
        return;
    }
    let segments = s.segments();
    let mut face_of: Vec<(Half, usize)> = Vec::new();
    for (i, f) in s.faces.iter().enumerate() {
        for e in &f.entries {
            for h in s.cycle(&segments, e.half()) {
                face_of.push((h, i));
            }
        }
    }
    let lookup = |h: Half| face_of.iter().find(|x| x.0 == h).map(|x| x.1);
    let feature_nodes: Vec<usize> = s.edges.iter().filter(|e| e.feature_line).flat_map(|e| e.vertices.iter().map(|v| v.node)).collect();
    let mut order: Vec<usize> = Vec::new();
    for e in s.edges.iter().filter(|e| e.lot_line) {
        let n = e.vertices.len();
        let Some(start) = e.vertices.iter().position(|v| feature_nodes.contains(&v.node)) else { continue };
        for k in 0..n {
            let i = (start + k) % n;
            if !e.closed && i + 1 >= n {
                continue;
            }
            let (a, b) = (e.vertices[i].node, e.vertices[(i + 1) % n].node);
            for f in [lookup((b, a)), lookup((a, b))].into_iter().flatten() {
                if touched.contains(&f) && !order.contains(&f) {
                    order.push(f);
                }
            }
        }
    }
    let rest: Vec<usize> = touched.iter().filter(|f| !order.contains(f)).copied().collect();
    order.extend(rest);
    // Each takes as entry the half-edge on its side of the last lot line
    // segment along it, and loses the mark of a changed face (PD01: (0,
    // 25) to (0, 50) and (0, 0) to (0, 25); PD08).
    let mut last: Vec<Option<Half>> = vec![None; s.faces.len()];
    for e in s.edges.iter().filter(|e| e.lot_line) {
        let n = e.vertices.len();
        for i in 0..segment_count(e) {
            let (a, b) = (e.vertices[i].node, e.vertices[(i + 1) % n].node);
            for h in [(b, a), (a, b)] {
                if let Some(f) = lookup(h) {
                    last[f] = Some(h);
                }
            }
        }
    }
    for f in order {
        s.faces[f].id = s.next_face;
        s.next_face += 1;
        s.faces[f].dirty = false;
        if let (Some(h), 1) = (last[f], s.faces[f].entries.len()) {
            s.faces[f].entries[0] = Entry::Live(h);
        }
    }
}

fn finish(s: &State) -> Result<Arrangement, &'static str> {
    let segments = s.segments();
    // Every loop's face.
    let mut face_of: Vec<(Half, i32)> = Vec::new();
    for (i, f) in s.faces.iter().enumerate() {
        for e in &f.entries {
            for h in s.cycle(&segments, e.half()) {
                face_of.push((h, i as i32 + 1));
            }
        }
    }
    let lookup = |h: Half| face_of.iter().find(|x| x.0 == h).map(|x| x.1);
    let all = s.member_segments();
    let mut edges = Vec::new();
    for (k, e) in s.edges.iter().enumerate() {
        let len = e.vertices.len();
        let mut vertices = Vec::new();
        for i in 0..len {
            let v = &e.vertices[i];
            let faces = if i + 1 == len && !e.closed {
                (0, 0)
            } else {
                let next = e.vertices[(i + 1) % len].node;
                (lookup((next, v.node)).ok_or("a loop has no face")?, lookup((v.node, next)).ok_or("a loop has no face")?)
            };
            let along = if i + 1 == len && !e.closed {
                0
            } else {
                let next = e.vertices[(i + 1) % len].node;
                all.iter().filter(|&&(j, (a, b, _))| j != k && ((a, b) == (v.node, next) || (b, a) == (v.node, next))).map(|&(j, _)| if j < k { 0x1000 } else { 0x2000 }).fold(0, |x, y| x | y)
            };
            vertices.push(Vertex { node: v.node as i32 + 1, bulge: v.bulge, inserted: v.inserted, shared: v.shared, faces, along });
        }
        edges.push(Edge { vertices, loose_start: e.loose_start, crossed: e.vertices.iter().any(|v| v.inserted) });
    }
    let faces = s
        .faces
        .iter()
        .map(|f| {
            let entries = f
                .entries
                .iter()
                .map(|e| {
                    let (from, to) = e.half();
                    let k = s.outgoing(&segments, from).iter().position(|o| o.0 == to).unwrap_or(0);
                    (from as i32 + 1, k as i32)
                })
                .collect();
            let loops: Vec<Vec<Half>> = f.entries.iter().map(|e| s.cycle(&segments, e.half())).collect();
            Face { flags: if f.dirty { 6 } else { 14 }, entries, label: label_point(s, &segments, &loops), loops: face_loops(s, &segments, &loops) }
        })
        .collect();
    let outgoing = (0..s.nodes.len()).map(|n| s.outgoing(&segments, n).iter().map(|o| o.0 as i32 + 1).collect()).collect();
    let halves = face_of.iter().map(|((a, b), f)| ((*a as i32 + 1, *b as i32 + 1), *f)).collect();
    Ok(Arrangement { nodes: s.nodes.clone(), edges, faces, face_ids: s.faces.iter().map(|f| f.id).collect(), next_face: s.next_face, outgoing, halves })
}

/// The arrangement of members that have no points (a container whose
/// members were moved onto the site: WT04 to WT06, WP01, WP02): no nodes
/// and no edges, and the outside face with `outside` flags and no entries
/// (none for 0: the container once the application has opened it again,
/// WT04R).
pub(super) fn without_points(outside: i32) -> Arrangement {
    let faces = (outside != 0).then(|| Face { flags: outside, entries: Vec::new(), label: Vector2::new(0.0, 0.0), loops: Vec::new() });
    let faces: Vec<Face> = faces.into_iter().collect();
    let face_ids: Vec<i32> = (1..=faces.len() as i32).collect();
    Arrangement { nodes: Vec::new(), edges: Vec::new(), next_face: faces.len() as i32 + 1, faces, face_ids, outgoing: Vec::new(), halves: Vec::new() }
}

impl Edge {
    /// The edge of a member without points.
    pub(super) fn without_points() -> Self {
        Edge { vertices: Vec::new(), loose_start: false, crossed: false }
    }
}

/// The points of a segment from `p` to `q` with `bulge` other than `q`:
/// `p` and, along an arc, `n - 1` more points; with `extremes`, also where
/// the arc reaches its extents.
fn arc_points(p: Vector2, q: Vector2, bulge: f64, n: usize, extremes: bool) -> Vec<Vector2> {
    if bulge == 0.0 {
        return vec![p];
    }
    let sweep = 4.0 * bulge.atan();
    let d = q - p;
    let normal = Vector2::new(-d.y, d.x) * (1.0 / d.length());
    let center = (p + q) * 0.5 + normal * (d.length() / 2.0 / (sweep / 2.0).tan());
    let r = (p - center).length();
    let a0 = (p.y - center.y).atan2(p.x - center.x);
    let at = |a: f64| center + Vector2::new(a.cos(), a.sin()) * r;
    let mut out: Vec<Vector2> = (0..n).map(|i| at(a0 + sweep * i as f64 / n as f64)).collect();
    if extremes {
        for k in 0..4 {
            let quarter = k as f64 * std::f64::consts::FRAC_PI_2;
            let t = if sweep > 0.0 { (quarter - a0).rem_euclid(TAU) } else { (a0 - quarter).rem_euclid(TAU) };
            if t < sweep.abs() {
                out.push(at(quarter));
            }
        }
    }
    out
}

/// A face's boundary loops as straight pieces (arcs divided).
type Loops = Vec<Vec<(Vector2, Vector2)>>;

fn face_loops(s: &State, segments: &[(usize, usize, f64)], loops: &[Vec<Half>]) -> Loops {
    let bulge = |h: &Half| segments.iter().find_map(|&(a, b, g)| if (a, b) == *h { Some(g) } else if (b, a) == *h { Some(-g) } else { None }).unwrap_or(0.0);
    loops
        .iter()
        .map(|run| {
            run.iter()
                .flat_map(|h| {
                    let (p, q) = (s.nodes[h.0].0, s.nodes[h.1].0);
                    let mut points = arc_points(p, q, bulge(h), 64, false);
                    points.push(q);
                    points.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}

fn loop_area(pieces: &[(Vector2, Vector2)]) -> f64 {
    pieces.iter().map(|(a, b)| a.cross(b)).sum()
}

fn inside_loop(pieces: &[(Vector2, Vector2)], p: Vector2) -> bool {
    pieces.iter().fold(false, |acc, (a, b)| if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) { !acc } else { acc })
}

fn distance(pieces: &Loops, p: Vector2) -> f64 {
    pieces
        .iter()
        .flatten()
        .map(|(a, b)| {
            let d = *b - *a;
            let t = if d.dot(&d) == 0.0 { 0.0 } else { ((p - *a).dot(&d) / d.dot(&d)).clamp(0.0, 1.0) };
            (p - (*a + d * t)).length()
        })
        .fold(f64::INFINITY, f64::min)
}

/// Whether `p` lies inside a bounded face: inside its outline and outside
/// the loops of what lies within it.
fn face_holds(loops: &Loops, p: Vector2) -> bool {
    let mut outline = false;
    for l in loops {
        let area = loop_area(l);
        if area < 0.0 && inside_loop(l, p) {
            outline = true;
        } else if area > 0.0 && inside_loop(l, p) {
            return false;
        }
    }
    outline
}

fn loop_area_abs(loops: &Loops) -> f64 {
    loops.iter().map(|l| loop_area(l)).filter(|a| *a < 0.0).map(f64::abs).sum()
}

impl Arrangement {
    /// The id the record gives face number `face` (0 stays 0).
    pub(super) fn face_id(&self, face: i32) -> i32 {
        if face >= 1 { self.face_ids.get(face as usize - 1).copied().unwrap_or(face) } else { face }
    }

    /// The number of the face the record names `id`.
    pub(super) fn face_number(&self, id: i32) -> Option<i32> {
        self.face_ids.iter().position(|f| *f == id).map(|i| i as i32 + 1)
    }

    /// The bounded face holding `p`: `prefer` when `p` lies on its
    /// boundary or inside it, else the smallest face around it.
    pub(super) fn face_at(&self, p: Vector2, prefer: i32) -> Option<i32> {
        if let Some(f) = self.faces.get((prefer.max(1) - 1) as usize).filter(|_| prefer >= 2) {
            let scale = 1e-9 * (1.0 + p.x.abs().max(p.y.abs()));
            if distance(&f.loops, p) <= scale || face_holds(&f.loops, p) {
                return Some(prefer);
            }
        }
        (1..self.faces.len())
            .filter(|i| face_holds(&self.faces[*i].loops, p))
            .min_by(|a, b| loop_area_abs(&self.faces[*a].loops).total_cmp(&loop_area_abs(&self.faces[*b].loops)))
            .map(|i| i as i32 + 1)
    }
}

/// Where a parcel's area label goes (WP03 to WP21). From the middle of
/// the face's extents (arcs included) the label slides along the
/// horizontal or the vertical line through it, whichever gets farther
/// from the boundary, to the nearest point as far from the boundary as
/// that line gets (rightward or upward on a tie). The middle stays when
/// one of the two lines gets no deeper there (rectangles, WP14's
/// triangle); a loose line pushes it aside (WP10),
/// a notch up (WP18), a diagonal side to where the side stops mattering
/// (WP19). A middle off a triangle gives its centroid (WP17).
// ponytail: the slide samples 4096 points along each line, then refines;
// other faces whose middle is off the face take the middle of the widest
// run of the horizontal line through it.
fn label_point(s: &State, segments: &[(usize, usize, f64)], loops: &[Vec<Half>]) -> Vector2 {
    let bulge = |h: &Half| segments.iter().find_map(|&(a, b, g)| if (a, b) == *h { Some(g) } else if (b, a) == *h { Some(-g) } else { None }).unwrap_or(0.0);
    let ends: Vec<(Vector2, Vector2, f64)> = loops.iter().flatten().map(|h| (s.nodes[h.0].0, s.nodes[h.1].0, bulge(h))).collect();
    let Some(first) = ends.first() else { return Vector2::new(0.0, 0.0) };
    let extents: Vec<Vector2> = ends.iter().flat_map(|e| arc_points(e.0, e.1, e.2, 1, true)).collect();
    let (lo, hi) = extents.iter().fold((first.0, first.0), |(lo, hi), p| (Vector2::new(lo.x.min(p.x), lo.y.min(p.y)), Vector2::new(hi.x.max(p.x), hi.y.max(p.y))));
    let c = (lo + hi) * 0.5;
    let pieces = face_loops(s, segments, loops);
    let scale = (hi - lo).length().max(1.0);
    let triangle = ends.len() == 3;
    if triangle && ends.iter().any(|e| e.2 != 0.0) {
        return curved_triangle_label(&ends).unwrap_or(c);
    }
    if !face_holds(&pieces, c) && !triangle {
        let halves: Vec<(Vector2, Vector2)> = pieces.iter().flatten().copied().collect();
        let mut xs: Vec<f64> = halves.iter().filter(|(a, b)| (a.y <= c.y) != (b.y <= c.y)).map(|(a, b)| a.x + (c.y - a.y) * (b.x - a.x) / (b.y - a.y)).collect();
        xs.sort_by(f64::total_cmp);
        let runs: Vec<(f64, f64)> = xs.chunks_exact(2).map(|r| (r[0], r[1])).collect();
        return runs.iter().max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0))).map_or(c, |r| Vector2::new((r.0 + r.1) / 2.0, c.y));
    }
    let depth = |p: Vector2| if face_holds(&pieces, p) { distance(&pieces, p) } else { -1.0 };
    let tol = 1e-12 * scale;
    let slide = || -> (f64, Vector2) {
    let mut best: Option<(f64, Vector2)> = None;
    for axis in [Vector2::new(1.0, 0.0), Vector2::new(0.0, 1.0)] {
        let (from, to) = if axis.x != 0.0 { (lo.x - c.x, hi.x - c.x) } else { (lo.y - c.y, hi.y - c.y) };
        let n = 4096;
        let at = |t: f64| c + axis * t;
        let step = (to - from) / n as f64;
        let samples: Vec<(f64, f64)> = (0..=n).map(|i| from + step * i as f64).map(|t| (t, depth(at(t)))).collect();
        // The farthest depth along the line, refined around the best sample.
        let m = samples.iter().max_by(|x, y| x.1.total_cmp(&y.1)).map_or(0.0, |m| m.0);
        let (mut a, mut b) = (m - step, m + step);
        for _ in 0..200 {
            let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
            if depth(at(m1)) < depth(at(m2)) {
                a = m1;
            } else {
                b = m2;
            }
        }
        let peak = (a + b) / 2.0;
        let top = depth(at(peak)).max(depth(at(0.0)));
        let reaches = |t: f64| depth(at(t)) >= top - tol;
        // The nearest point reaching it, rightward or upward first.
        if reaches(0.0) {
            // The middle is as deep as its line gets: it stays (WP14).
            return (depth(c), c);
        }
        let t = {
            let near = samples
                .iter()
                .filter(|x| x.1 >= top - 1e-6 * scale)
                .min_by(|x, y| (x.0.abs(), -x.0.signum()).partial_cmp(&(y.0.abs(), -y.0.signum())).unwrap())
                .map_or(peak, |x| x.0);
            let mut outer = if reaches(near) { near } else { peak };
            let mut inner = outer - step * outer.signum();
            if reaches(inner) {
                inner = 0.0;
            }
            for _ in 0..200 {
                let mid = (inner + outer) / 2.0;
                if reaches(mid) {
                    outer = mid;
                } else {
                    inner = mid;
                }
            }
            outer
        };
        if best.is_none_or(|b| top > b.0 + 1e-9 * scale) {
            best = Some((top, at(t)));
        }
    }
    best.unwrap_or((depth(c), c))
    };
    let (slid_depth, slid) = slide();
    if !triangle {
        return slid;
    }
    // A triangle's label: the middle of its largest angle's bisector, up
    // to the opposite side, unless the slide gets nearly as deep without
    // passing it (thin triangles; WPLA, WPLB, WPLC).
    // ponytail: the 0.94 is fitted to the samples; nearly isosceles right
    // triangles (WPLC leg50 to leg65 but leg60) take another point the
    // rule misses.
    let p = [ends[0].0, ends[1].0, ends[2].0];
    let angle = |i: usize| {
        let (a, b, c) = (p[i], p[(i + 1) % 3], p[(i + 2) % 3]);
        let (u, v) = (b - a, c - a);
        (u.dot(&v) / (u.length() * v.length())).clamp(-1.0, 1.0).acos()
    };
    let i = (0..3).max_by(|x, y| angle(*x).total_cmp(&angle(*y))).unwrap_or(0);
    let (a, b, cc) = (p[i], p[(i + 1) % 3], p[(i + 2) % 3]);
    let (ab, ac) = ((b - a).length(), (cc - a).length());
    let foot = (b * ac + cc * ab) * (1.0 / (ab + ac));
    let bisector = (a + foot) * 0.5;
    // A right isosceles triangle with legs along the axes (a square cut
    // corner to corner) goes to the middle of the vertical run through the
    // bisector's middle, halfway to the extents' middle (WPLA, WPLB, WP19,
    // WPLC leg60).
    let (u, v) = (b - a, cc - a);
    let along = |w: Vector2| w.x.abs() <= 1e-9 * scale || w.y.abs() <= 1e-9 * scale;
    if (ab - ac).abs() <= 1e-9 * scale && u.dot(&v).abs() <= 1e-9 * scale * scale && along(u) && along(v) {
        return Vector2::new(bisector.x, (bisector.y + c.y) / 2.0);
    }
    let bisector_depth = depth(bisector);
    if slid_depth >= 0.94 * bisector_depth && slid_depth <= bisector_depth { slid } else { bisector }
}

/// The label of a face of three sides, some of them arcs (a curb
/// return's corner between two offset lines, WC64): the middle of the
/// largest angle's bisector, up to the opposite side.
// ponytail: one shape sampled (quarter-circle corners); the slide the
// straight triangles weigh against the bisector is not tried here.
fn curved_triangle_label(ends: &[(Vector2, Vector2, f64)]) -> Option<Vector2> {
    let unit = |a: f64| Vector2::new(a.cos(), a.sin());
    let chord = |p: Vector2, q: Vector2| (q - p).y.atan2((q - p).x);
    // At vertex i: the way out along side i and the way back along side
    // i - 1, as tangent directions.
    let angle = |i: usize| {
        let (p, q, g) = ends[i];
        let (r, s, h) = ends[(i + 2) % 3];
        let (out, back) = (unit(chord(p, q) - 2.0 * g.atan()), unit(chord(s, r) + 2.0 * h.atan()));
        (out.dot(&back).clamp(-1.0, 1.0).acos(), out, back)
    };
    let i = (0..3).max_by(|x, y| angle(*x).0.total_cmp(&angle(*y).0))?;
    let (_, out, back) = angle(i);
    let a = ends[i].0;
    let dir = out + back;
    let dir = dir * (1.0 / dir.length());
    let (p, q, g) = ends[(i + 1) % 3];
    let side = Piece { p, q, g };
    let foot = match side.arc() {
        None => {
            let e = q - p;
            let den = dir.cross(&e);
            (den.abs() > 1e-12).then(|| a + dir * ((p - a).cross(&e) / den))?
        }
        Some((centre, r, ..)) => {
            let f = a - centre;
            let (b, cc) = (f.dot(&dir), f.dot(&f) - r * r);
            let disc = b * b - cc;
            (disc >= 0.0).then_some(())?;
            [-b - disc.sqrt(), -b + disc.sqrt()]
                .into_iter()
                .filter(|t| *t > 1e-9)
                .map(|t| a + dir * t)
                .find(|x| side.param_of(*x).is_some_and(|t| (-1e-9..=1.0 + 1e-9).contains(&t)))?
        }
    };
    Some((a + foot) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(points: &[(f64, f64)]) -> Vector2 {
        let path: Vec<(Vector2, Option<f64>)> = points.iter().map(|p| (Vector2::new(p.0, p.1), None)).collect();
        let z = vec![0.0; path.len()];
        build(&[Input { path: &path, elevations: &z, closed: true, feature_line: false, lot_line: true }]).unwrap().faces[1].label
    }

    #[test]
    fn triangle_labels() {
        let near = |p: Vector2, x: f64, y: f64| (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9;
        // The middle of the right angle's bisector (WPLC base, any order).
        assert!(near(label(&[(0.0, 0.0), (60.0, 0.0), (0.0, 40.0)]), 12.0, 12.0));
        assert!(near(label(&[(0.0, 40.0), (60.0, 0.0), (0.0, 0.0)]), 12.0, 12.0));
        // A thin one slides (WPLC leg20).
        let p = label(&[(0.0, 0.0), (60.0, 0.0), (0.0, 20.0)]);
        assert!((p.y - 10.0).abs() < 1e-9 && (p.x - 7.2076).abs() < 1e-4);
        // A square cut corner to corner (WPLA, WP19).
        assert!(near(label(&[(0.0, 0.0), (0.0, 50.0), (50.0, 50.0)]), 12.5, 31.25));
        // WP14's triangle keeps its middle.
        assert!(near(label(&[(0.0, 100.0), (60.0, 100.0), (30.0, 160.0)]), 30.0, 130.0));
    }
}
