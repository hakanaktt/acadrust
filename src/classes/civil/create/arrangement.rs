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
//! member split is void: a split face then keeps the part on the
//! segment's right with the segment as its entry, and a face that is not
//! split takes the half-edge that followed the old entry. When a segment
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
    pub faces: Vec<Face>,
}

type Half = (usize, usize);

#[derive(Clone, Copy)]
enum Entry {
    Live(Half),
    /// Split by the member being added: the half-edge that followed it.
    Void(Half),
}

impl Entry {
    fn half(self) -> Half {
        match self {
            Entry::Live(h) | Entry::Void(h) => h,
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
    /// Segments in the graph (all but for the member being added).
    present: usize,
    loose_start: bool,
}

struct FaceState {
    dirty: bool,
    entries: Vec<Entry>,
}

struct State {
    nodes: Vec<(Vector2, f64)>,
    edges: Vec<EdgeState>,
    faces: Vec<FaceState>,
}

fn segment_count(e: &EdgeState) -> usize {
    if e.closed { e.vertices.len() } else { e.vertices.len() - 1 }
}

fn near(a: Vector2, b: Vector2) -> bool {
    (a - b).length() <= 1e-8 * (1.0 + a.x.abs().max(a.y.abs()))
}

impl State {
    /// Every segment in the graph: (from, to, bulge).
    fn segments(&self) -> Vec<(usize, usize, f64)> {
        let mut out = Vec::new();
        for e in &self.edges {
            let n = e.vertices.len();
            for i in 0..e.present {
                out.push((e.vertices[i].node, e.vertices[(i + 1) % n].node, e.vertices[i].bulge.unwrap_or(0.0)));
            }
        }
        out
    }

    fn angle(&self, from: usize, to: usize, bulge: f64) -> f64 {
        let d = self.nodes[to].0 - self.nodes[from].0;
        (d.y.atan2(d.x) - 2.0 * bulge.atan()).rem_euclid(TAU)
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

    /// Twice the signed area a cycle encloses (chords): negative for the
    /// outline of a bounded face.
    fn area(&self, cycle: &[Half]) -> f64 {
        cycle.iter().map(|h| self.nodes[h.0].0.cross(&self.nodes[h.1].0)).sum()
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
                let area = self.area(&cycle);
                if area >= 0.0 {
                    continue;
                }
                let inside = cycle.iter().fold(false, |acc, h| {
                    let (a, b) = (self.nodes[h.0].0, self.nodes[h.1].0);
                    if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) { !acc } else { acc }
                });
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

/// A new node `x` on segment `a`-`b`: its parameter along the segment.
fn param(a: Vector2, b: Vector2, x: Vector2) -> f64 {
    let d = b - a;
    (x - a).dot(&d) / d.dot(&d)
}

/// Builds the arrangement of `members` in order. Only straight segments
/// may meet; a member must not cross itself or run along another.
pub(super) fn build(members: &[Input]) -> Result<Arrangement, &'static str> {
    let mut s = State { nodes: Vec::new(), edges: Vec::new(), faces: Vec::new() };
    for m in members {
        let old_faces = s.faces.len();
        let old_nodes = s.nodes.len();
        let old_segments = s.segments();
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
        let mut crossings: Vec<(usize, f64, Vector2)> = Vec::new();
        for i in 0..segs {
            let (p, q) = (m.path[i].0, m.path[(i + 1) % n].0);
            let straight = m.path[i].1.is_none();
            for &(a, b, g) in &old_segments {
                let (pa, pb) = (s.nodes[a].0, s.nodes[b].0);
                if !straight || g != 0.0 {
                    continue;
                }
                let (d, e) = (q - p, pb - pa);
                let den = d.cross(&e);
                let scale = d.length() * e.length();
                if den.abs() <= 1e-12 * scale {
                    // Parallel: refuse a shared stretch.
                    if (pa - p).cross(&d).abs() <= 1e-9 * scale.max(1.0) {
                        let (t0, t1) = (param(p, q, pa), param(p, q, pb));
                        if t0.max(t1) > 1e-9 && t0.min(t1) < 1.0 - 1e-9 {
                            return Err("a member runs along another");
                        }
                    }
                    continue;
                }
                let t = (pa - p).cross(&e) / den;
                let u = (pa - p).cross(&d) / den;
                let tol = 1e-9;
                if !(-tol..=1.0 + tol).contains(&t) || !(-tol..=1.0 + tol).contains(&u) {
                    continue;
                }
                let x = p + d * t;
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
                    (true, true) => crossings.push((i, t, x)),
                }
            }
        }
        // Crossing nodes take the next ids in the member's order.
        crossings.sort_by(|a, b| (a.0, a.1).partial_cmp(&(b.0, b.1)).unwrap());
        let mut placed: Vec<(Vector2, usize)> = Vec::new();
        for &(i, t, x) in &crossings {
            let node = match placed.iter().find(|p| near(p.0, x)) {
                Some(p) => p.1,
                None => {
                    s.nodes.push((x, 0.0));
                    placed.push((x, s.nodes.len() - 1));
                    s.nodes.len() - 1
                }
            };
            if !on_new[i].iter().any(|v| v.1 == node) {
                on_new[i].push((t, node));
            }
        }
        for &(_, _, x) in &crossings {
            let node = placed.iter().find(|p| near(p.0, x)).map(|p| p.1).unwrap();
            for &(a, b, g) in &old_segments {
                if g == 0.0 && a != node && b != node {
                    let (pa, pb) = (s.nodes[a].0, s.nodes[b].0);
                    let u = param(pa, pb, x);
                    if u > 1e-9 && u < 1.0 - 1e-9 && (pb - pa).cross(&(x - pa)).abs() <= 1e-9 * (pb - pa).length() * (1.0 + (x - pa).length()) && !on_old.iter().any(|o| o.0 == a && o.1 == b && o.3 == node) {
                        on_old.push((a, b, u, node));
                    }
                }
            }
        }
        // Entries the splits void take the half-edge after them.
        let split: Vec<(usize, usize)> = {
            let mut v: Vec<(usize, usize)> = on_old.iter().map(|o| (o.0, o.1)).collect();
            v.sort();
            v.dedup();
            v
        };
        let is_split = |h: Half| split.iter().any(|&(a, b)| (a, b) == h || (b, a) == h);
        let first_piece = |h: Half| -> Half {
            let mut best: Option<(f64, usize)> = None;
            for o in &on_old {
                let u = if (o.0, o.1) == h { o.2 } else if (o.1, o.0) == h { 1.0 - o.2 } else { continue };
                if best.is_none_or(|b| u < b.0) {
                    best = Some((u, o.3));
                }
            }
            best.map_or(h, |b| (h.0, b.1))
        };
        for f in 0..s.faces.len() {
            for k in 0..s.faces[f].entries.len() {
                if let Entry::Live(h) = s.faces[f].entries[k] {
                    if is_split(h) {
                        let after = s.next(&old_segments, h);
                        s.faces[f].entries[k] = Entry::Void(first_piece(after));
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
                for (_, node) in add {
                    out.push(V { node, bulge: None, inserted: true, shared: false });
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
                for (_, node) in add {
                    vertices.push(V { node, bulge: None, inserted: true, shared: node < old_nodes });
                }
            }
        }
        let on_earlier = |node: usize| node < old_nodes || on_old.iter().any(|o| o.3 == node);
        let loose_start = !s.faces.is_empty() && !on_earlier(own[0]);
        s.edges.push(EdgeState { vertices, closed: m.closed, present: 0, loose_start });
        let me = s.edges.len() - 1;
        let total = segment_count(&s.edges[me]);
        for j in 0..total {
            let segments = s.segments();
            let len = s.edges[me].vertices.len();
            let a = s.edges[me].vertices[j].node;
            let b = s.edges[me].vertices[(j + 1) % len].node;
            let g = s.edges[me].vertices[j].bulge.unwrap_or(0.0);
            let a_free = s.outgoing(&segments, a).is_empty();
            let b_free = s.outgoing(&segments, b).is_empty();
            let face = if s.faces.is_empty() {
                s.faces.push(FaceState { dirty: false, entries: vec![Entry::Live((a, b))] });
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
            let (ar, al) = (s.area(&right), s.area(&left));
            let new_entry;
            if ar < 0.0 && al < 0.0 {
                let k = s.faces[face].entries.iter().position(|e| right.contains(&e.half()) || left.contains(&e.half()));
                match k.map(|k| (k, s.faces[face].entries[k])) {
                    Some((_, Entry::Live(h))) if left.contains(&h) => new_entry = (a, b),
                    Some((_, Entry::Live(_))) => new_entry = (b, a),
                    Some((k, Entry::Void(_))) => {
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
                        let from = e.half().0;
                        let h = *outer.iter().find(|h| h.0 == from).ok_or("a loop has no outer side")?;
                        *e = Entry::Live(h);
                    }
                }
                new_entry = inner_half;
            }
            if earlier {
                s.faces[face].dirty = true;
            }
            s.faces.push(FaceState { dirty: false, entries: vec![Entry::Live(new_entry)] });
        }
        for f in s.faces.iter_mut() {
            for e in f.entries.iter_mut() {
                *e = Entry::Live(e.half());
            }
        }
    }
    finish(&s)
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
    let mut edges = Vec::new();
    for e in &s.edges {
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
            vertices.push(Vertex { node: v.node as i32 + 1, bulge: v.bulge, inserted: v.inserted, shared: v.shared, faces });
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
    Ok(Arrangement { nodes: s.nodes.clone(), edges, faces })
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
    if !face_holds(&pieces, c) {
        if ends.len() == 3 && ends.iter().all(|e| e.2 == 0.0) {
            return (ends[0].0 + ends[1].0 + ends[2].0) * (1.0 / 3.0);
        }
        let halves: Vec<(Vector2, Vector2)> = pieces.iter().flatten().copied().collect();
        let mut xs: Vec<f64> = halves.iter().filter(|(a, b)| (a.y <= c.y) != (b.y <= c.y)).map(|(a, b)| a.x + (c.y - a.y) * (b.x - a.x) / (b.y - a.y)).collect();
        xs.sort_by(f64::total_cmp);
        let runs: Vec<(f64, f64)> = xs.chunks_exact(2).map(|r| (r[0], r[1])).collect();
        return runs.iter().max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0))).map_or(c, |r| Vector2::new((r.0 + r.1) / 2.0, c.y));
    }
    let depth = |p: Vector2| if face_holds(&pieces, p) { distance(&pieces, p) } else { -1.0 };
    let tol = 1e-12 * scale;
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
            return c;
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
    best.map_or(c, |b| b.1)
}
