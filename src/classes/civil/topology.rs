//! Site topology: the graph Civil 3D keeps for a site (in its container,
//! an `AECC_FEATURE_STYLE` record) and for each siteless alignment (its
//! companion). Nodes are points; edges run through nodes, each vertex
//! naming the faces on both sides of the segment that starts there. Face 1
//! is the site's outside; the parcels are the other faces.
//!
//! ```text
//! nodes: BL next free id, BL n, BL first id,
//!        n x { BD x, BD y, BD z, 3 bits 010, BL next id (0 last) }
//! edges: BL next free id, BL slots, then per slot either BL 2 (a free
//!        slot: its member left) or BL 0, BL edge id, BL flags, BL n, BL 0,
//!        n x { BL kind (0 line, 1 arc, 2 last), BL flags, BL node,
//!        BL face a, BL face b, [arc: BD bulge, BL 0] },
//!        BL m, m x BL, BL 0 x 3
//! ```
//!
//! A feature line is an edge of its site's topology with its elevations in
//! the nodes' z; alignments keep z 0.

use super::bits::Bits;
use crate::types::{Vector2, Vector3};

/// Points of the segment from `a` to `b` with `bulge` (`tan(sweep / 4)`,
/// positive to the left), `a` included, `b` not.
fn segment_points(a: Vector2, b: Vector2, bulge: f64) -> Vec<Vector2> {
    if bulge.abs() < 1e-12 {
        return vec![a];
    }
    let sweep = 4.0 * bulge.atan();
    let chord = b - a;
    let radius = chord.length() / (2.0 * (sweep / 2.0).sin().abs());
    // The centre is to the left of the chord for a left (positive) turn.
    let mid = (a + b) * 0.5;
    let normal = Vector2::new(-chord.y, chord.x) * (1.0 / chord.length());
    let rise = radius * (sweep / 2.0).cos().abs();
    let side = if sweep.abs() < std::f64::consts::PI { 1.0 } else { -1.0 } * bulge.signum();
    let center = mid + normal * (rise * side);
    let a0 = (a.y - center.y).atan2(a.x - center.x);
    let n = ((sweep.abs() * radius).ceil() as usize).clamp(4, 2048);
    (0..n)
        .map(|i| {
            let t = a0 + sweep * i as f64 / n as f64;
            center + Vector2::new(t.cos(), t.sin()) * radius
        })
        .collect()
}

/// One vertex of a topology edge.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TopologyVertex {
    pub point: Vector2,
    /// Elevation of the node (0 on alignments).
    pub z: f64,
    /// The vertex's flags; see [`Self::is_elevation_point`].
    pub flags: i32,
    /// Faces on the two sides of the segment starting here (0 at the last
    /// vertex, or where the record keeps no faces).
    pub faces: (i32, i32),
    /// `tan(sweep / 4)` of an arc starting here, positive to the left.
    pub bulge: f64,
    /// The node's id in the container.
    pub node: i32,
    /// An elevation point's key in the container's table of elevation
    /// points (0 for other vertices).
    pub key: i32,
}

impl TopologyVertex {
    /// A feature line's elevation point (a vertex that only sets an
    /// elevation between two PIs) rather than a PI.
    pub fn is_elevation_point(&self) -> bool {
        self.flags & 0x40_0000 != 0
    }
}

/// A site's or alignment's topology: its edges as vertex runs.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Topology {
    pub edges: Vec<Vec<TopologyVertex>>,
    /// The next free node id and elevation point key the container
    /// records (the application does not reuse ids of removed nodes).
    pub next_node: i32,
    pub next_key: i32,
    /// The flags of the container's first (outside) face, 0 without faces.
    pub outside_face_flags: i32,
    /// Each edge's flags (bit 0 a closed edge).
    pub edge_flags: Vec<i32>,
}

impl Topology {
    /// The boundary of `face`: its segments chained into loops, each a run
    /// of `(point, bulge)` with the bulge of the segment that starts there,
    /// turned so the face is on the left.
    /// Points of edge `index` in 3D: arcs tessellated, each segment's
    /// elevation interpolated between its two nodes.
    pub fn edge_points(&self, index: usize) -> Vec<Vector3> {
        let Some(edge) = self.edges.get(index) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for w in edge.windows(2) {
            let (a, b) = (w[0], w[1]);
            let points = segment_points(a.point, b.point, a.bulge);
            let n = points.len() as f64;
            out.extend(points.iter().enumerate().map(|(i, p)| Vector3::new(p.x, p.y, a.z + (b.z - a.z) * i as f64 / n)));
        }
        out.extend(edge.last().map(|v| Vector3::new(v.point.x, v.point.y, v.z)));
        out
    }

    pub fn face_boundary(&self, face: i32) -> Vec<Vec<(Vector2, f64)>> {
        // Segments with the face on the left (`a`) or right (`b`) side.
        let mut segments: Vec<(Vector2, Vector2, f64)> = Vec::new();
        for edge in &self.edges {
            for w in edge.windows(2) {
                let (v, next) = (w[0], w[1]);
                if v.faces.0 == face {
                    segments.push((v.point, next.point, v.bulge));
                } else if v.faces.1 == face {
                    segments.push((next.point, v.point, -v.bulge));
                }
            }
        }
        let near = |a: Vector2, b: Vector2| a.distance(&b) <= 1e-6 * (1.0 + a.x.abs().max(a.y.abs()));
        let mut loops = Vec::new();
        while let Some((a, b, bulge)) = segments.pop() {
            let mut run = vec![(a, bulge)];
            let mut tip = b;
            while let Some(i) = segments.iter().position(|s| near(s.0, tip)) {
                let (p, q, g) = segments.swap_remove(i);
                run.push((p, g));
                tip = q;
            }
            run.push((tip, 0.0));
            loops.push(run);
        }
        loops
    }
}

impl Topology {
    /// The boundary loops of `face` as points, arcs divided.
    pub fn face_polylines(&self, face: i32) -> Vec<Vec<Vector2>> {
        self.face_boundary(face)
            .into_iter()
            .map(|run| {
                let mut points: Vec<Vector2> = run.windows(2).flat_map(|w| segment_points(w[0].0, w[1].0, w[0].1)).collect();
                points.extend(run.last().map(|p| p.0));
                points
            })
            .collect()
    }
}

fn node_list(data: &[u8], pos: usize, end: usize) -> Option<(Vec<(i32, Vector2, f64)>, usize, i32)> {
    let mut bits = Bits::new(data, pos, end);
    let next_free = bits.bit_long()?;
    let n = bits.bit_long()?;
    let mut id = bits.bit_long()?;
    // A container whose members were moved onto the site holds no nodes
    // (first id 0) until the application builds it again (WT04 to WT06,
    // WP01, WP02).
    if n == 0 && id == 0 && (1..=1_000_000).contains(&next_free) {
        return Some((Vec::new(), bits.pos, next_free));
    }
    if !(1..=100_000).contains(&n) || next_free > 1_000_000 || n >= next_free || id < 1 || id >= next_free {
        return None;
    }
    let mut nodes: Vec<(i32, Vector2, f64)> = Vec::with_capacity(n as usize);
    for i in 0..n {
        let (x, y, z) = (bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
        if ![x, y, z].iter().all(|v| v.is_finite() && v.abs() < 1e12) || bits.bits_value(3)? != 0b010 {
            return None;
        }
        let next = bits.bit_long()?;
        let last = i + 1 == n;
        if last != (next == 0) || (!last && !(1..next_free).contains(&next)) || nodes.iter().any(|m| m.0 == id) {
            return None;
        }
        nodes.push((id, Vector2::new(x, y), z));
        id = next;
    }
    Some((nodes, bits.pos, next_free))
}

fn edges(data: &[u8], pos: usize, end: usize, nodes: &[(i32, Vector2, f64)]) -> Option<(Vec<Vec<TopologyVertex>>, Vec<i32>, usize)> {
    let mut bits = Bits::new(data, pos, end);
    let next_free = bits.bit_long()?;
    let slots = bits.bit_long()?;
    if !(1..=10_000).contains(&slots) || slots >= next_free {
        return None;
    }
    let mut out = Vec::new();
    let mut all_flags = Vec::new();
    for _ in 0..slots {
        // A member that left keeps its slot as a lone 2.
        match bits.bit_long()? {
            2 => continue,
            0 => {}
            _ => return None,
        }
        // The slot's edge ID: its slot number in a container Civil 3D wrote
        // in one go, any free ID once members moved between containers.
        let id = bits.bit_long()?;
        (1..next_free).contains(&id).then_some(())?;
        // Edge flags: bit 1 marks a closed edge (a lot line around a
        // parcel), whose last vertex starts the segment back to the first.
        let edge_flags = bits.bit_long()?;
        let closed = edge_flags & 1 != 0;
        // An edge can be left with no vertices; it keeps its place.
        let n = bits.bit_long()?;
        if !(0..=100_000).contains(&n) || bits.bit_long()? != 0 {
            return None;
        }
        let mut run = Vec::with_capacity(n as usize);
        for i in 0..n {
            let kind = bits.bit_long()?;
            let flags = bits.bit_long()?;
            let node = bits.bit_long()?;
            let faces = (bits.bit_long()?, bits.bit_long()?);
            // The last vertex ends the edge (2), or on a closed edge starts
            // the segment (0) or arc (1, a circle) back to the first one.
            let last = i + 1 == n;
            if !(0..=2).contains(&kind) || (!last && kind == 2) || (last && kind == 0 && !closed) {
                return None;
            }
            let bulge = if kind == 1 {
                let b = bits.bit_double().filter(|b| b.is_finite())?;
                (bits.bit_long()? == 0).then_some(b)?
            } else {
                0.0
            };
            let &(_, point, z) = nodes.iter().find(|m| m.0 == node)?;
            run.push(TopologyVertex { point, z, flags, faces, bulge, node, key: 0 });
        }
        // A closed edge ends with a copy of its first vertex.
        if run.last().is_some_and(|v| v.bulge != 0.0 || v.faces != (0, 0)) {
            let first = run[0];
            run.push(TopologyVertex { bulge: 0.0, faces: (0, 0), ..first });
        }
        // A list after the vertices (empty on alignments; one entry once a
        // feature line has an elevation point), then three zeros.
        let listed = bits.bit_long()?;
        if !(0..=100_000).contains(&listed) {
            return None;
        }
        // The keys of the edge's elevation points, in edge order.
        let mut points = run.iter_mut().filter(|v| v.is_elevation_point());
        for _ in 0..listed {
            let key = bits.bit_long()?;
            if let Some(v) = points.next() {
                v.key = key;
            }
        }
        for _ in 0..3 {
            (bits.bit_long()? == 0).then_some(())?;
        }
        out.push(run);
        all_flags.push(edge_flags);
    }
    (!out.is_empty()).then_some((out, all_flags, bits.pos))
}

/// The flags of the first face of the face list at `pos`: `BL next free
/// id, BL count, per face {BL id, BL id, BL flags, ...}`.
fn outside_face_flags(data: &[u8], pos: usize, end: usize) -> Option<i32> {
    let mut bits = Bits::new(data, pos, end);
    bits.bit_long()?;
    if bits.bit_long()? == 0 {
        return Some(0);
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.bit_long()
}

/// The topology in a site container or alignment companion body:
///
/// ```text
/// BS 0x04BE, BL key (0 for a site container, 0x80000000 | n for a
/// companion), BL 0, BL 0, BL 32, 7 bits 0111010,
/// elevation points: BL next free key, BL k, k x {BL key, BL 21, BD the
///   point's index in its feature line, 17 bits, BL key, BL 0, BL 0,
///   BD elevation, B 0, BL 0}, BL 1, BL 0,
/// BL 0x80000001, node list, edge list
/// ```
pub(super) fn topology(data: &[u8], start: usize, end: usize) -> Option<Topology> {
    let read = |pos: usize, next_key: i32| -> Option<Topology> {
        let (nodes, after, next_node) = node_list(data, pos, end)?;
        let (edges, edge_flags, faces) = edges(data, after, end, &nodes)?;
        Some(Topology {
            edges,
            edge_flags,
            next_node,
            next_key,
            outside_face_flags: outside_face_flags(data, faces, end).unwrap_or(0),
        })
    };
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    for want in [0, 0, 32] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.skip(7)?;
    let mut next_key = 0;
    let head = (|| {
        next_key = bits.bit_long()?;
        let count = bits.bit_long()?;
        (0..=100_000).contains(&count).then_some(())?;
        for _ in 0..count {
            bits.bit_long()?;
            bits.bit_long()?;
            bits.bit_double()?;
            bits.skip(17)?;
            for _ in 0..3 {
                bits.bit_long()?;
            }
            bits.bit_double()?;
            bits.bit()?;
            bits.bit_long()?;
        }
        bits.bit_long()?;
        bits.bit_long()?;
        (bits.bit_long()? as u32 == 0x8000_0001).then_some(())
    })();
    if head.is_some() {
        return read(bits.pos, next_key);
    }
    // debt: a container whose head does not follow the layout above is
    // read by trying positions for its node list.
    (start..end.saturating_sub(40)).find_map(|pos| read(pos, 0))
}
