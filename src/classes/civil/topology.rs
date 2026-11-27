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
    /// Each edge's id in the container (its member's number), in the
    /// order of `edges` (the container's hash order: see
    /// `create::site::write_container`).
    pub edge_ids: Vec<i32>,
    /// The ids of the container's faces in the order it lists them (1 the
    /// outside face).
    pub face_ids: Vec<i32>,
    /// The outside face's entries (node id, index of the half-edge among
    /// the node's).
    pub outside_entries: Vec<(i32, i32)>,
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

#[allow(clippy::type_complexity)]
fn edges(data: &[u8], pos: usize, end: usize, nodes: &[(i32, Vector2, f64)]) -> Option<(Vec<Vec<TopologyVertex>>, Vec<i32>, Vec<i32>, usize)> {
    let mut bits = Bits::new(data, pos, end);
    let next_free = bits.bit_long()?;
    let slots = bits.bit_long()?;
    // A face's own graph may hold no edges (`BL 1, BL 0`).
    if !(0..=10_000).contains(&slots) || slots >= next_free {
        return None;
    }
    let mut out = Vec::new();
    let mut all_flags = Vec::new();
    let mut ids = Vec::new();
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
        // feature line has an elevation point), then three more lists.
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
        // Three lists `BL n, n x BL`, empty on alignments and in the
        // containers this crate writes; in a graded site's graph they name
        // other edges (Grading-3A: `(), (22), ()`; Grading-4: `(2), (), ()`
        // and `(), (1, 3), ()`).
        for _ in 0..3 {
            let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 2 <= bits.remaining())?;
            for _ in 0..n {
                bits.bit_long()?;
            }
        }
        out.push(run);
        all_flags.push(edge_flags);
        ids.push(id);
    }
    Some((out, all_flags, ids, bits.pos))
}

/// The face list at `pos`: `BL next free id, BL count, per face {BL id,
/// BL id, BL flags, BL loops, per loop {BL entry node, BL entry index}}`;
/// the ids in list order, the outside face's (id 1) flags and entries, and
/// the position after the list.
#[allow(clippy::type_complexity)]
fn face_list(data: &[u8], pos: usize, end: usize) -> Option<(Vec<i32>, i32, Vec<(i32, i32)>, usize)> {
    let mut bits = Bits::new(data, pos, end);
    bits.bit_long()?;
    let count = bits.bit_long()?;
    (0..=100_000).contains(&count).then_some(())?;
    let (mut ids, mut outside, mut entries) = (Vec::new(), 0, Vec::new());
    for _ in 0..count {
        let id = bits.bit_long()?;
        bits.bit_long()?;
        let flags = bits.bit_long()?;
        let loops = bits.bit_long()?;
        (0..=100_000).contains(&loops).then_some(())?;
        for _ in 0..loops {
            let entry = (bits.bit_long()?, bits.bit_long()?);
            if id == 1 {
                entries.push(entry);
            }
        }
        if id == 1 {
            outside = flags;
        }
        ids.push(id);
    }
    Some((ids, outside, entries, bits.pos))
}

/// A graph's nodes, edges and faces at `pos`; the position after the
/// faces.
fn graph_end(data: &[u8], pos: usize, end: usize) -> Option<usize> {
    let (nodes, after, _) = node_list(data, pos, end)?;
    let (_, _, _, faces) = edges(data, after, end, &nodes)?;
    face_list(data, faces, end).map(|f| f.3)
}

/// The elements a container keeps between its elevation-point table and
/// its graph: `BL next free id, BL n`, then `n` elements (see
/// [`element`]); the position after them. A site without gradings and an
/// alignment companion keep none (`BL 1, BL 0`). A graded site keeps
/// groups and faces (Grading-4: 3 elements; Grading-7: 11, the
/// application's ids 6 to 16), in no particular order.
///
/// A parcel-site container keeps interaction records (kind 0) instead —
/// the application's record of how the site's lot lines meet, whose
/// frames are unmapped (Labels-Parcels `Site`: 25, Parcel-2C
/// `Subdivision A`: 18, Parcel-1C: 4, Parcel-1D: 16, Parcel-2A/2B: 18
/// each; no modeler stream in any of them). They refuse here, and the
/// site graph they replace does not exist in the record: the
/// `BL 0x80000001` gate word never occurs in those six containers, so
/// the topology reads `None` with the table decoded (see
/// `elevation_keys`). The parcel-site containers that DO carry the
/// standard graph (Parcel-1B: 13 edges, Parcel-1E: 82, Parcel-3A: 80)
/// keep no elements and read through.
fn elements(data: &[u8], pos: usize, end: usize) -> Option<usize> {
    let mut bits = Bits::new(data, pos, end);
    bits.bit_long()?;
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 2 <= bits.remaining())?;
    let mut pos = bits.pos;
    for _ in 0..n {
        pos = element(data, pos, end)?;
    }
    Some(pos)
}

/// One element:
///
/// ```text
/// BL id, BL kind (1 a face, 2 a group, 0 a parcel-site interaction
///   record — its frame unmapped, it refuses), BL id, BL flags, BD, 6 x BL,
/// BL (a group: the id of one of its faces),
/// a face: B 0, B 0, BS 2, the face's body (a modeler binary stream,
///   read to its end record), BL 3, BL 0, BL 0, BL 1, BL 0, the face's
///   own elements and graph (`BL 1, BL 0` and an empty graph in the
///   samples, or nodes and edges), BL 0, BL parent (the group's id),
///   BL previous face, BL next face (0: none), BD, BD
/// ```
///
/// Proved on the six graded tutorial containers (Grading-3A to Grading-8):
/// each walks its count of elements and ends exactly at the site graph's
/// `BL 0x80000001`, and the site graph that follows holds the site's
/// feature lines (Grading-7: edges of 90, 19 and 17 vertices, as the
/// application reports for its three feature lines).
// ponytail: the flags, the BD, the six BLs and the two closing BDs are
// read past unnamed; name them from a drawing that varies one grading
// property at a time.
fn element(data: &[u8], pos: usize, end: usize) -> Option<usize> {
    let mut bits = Bits::new(data, pos, end);
    let id = bits.bit_long()?;
    let kind = bits.bit_long()?;
    (bits.bit_long()? == id).then_some(())?;
    bits.bit_long()?;
    bits.bit_double()?;
    for _ in 0..7 {
        bits.bit_long()?;
    }
    match kind {
        2 => return Some(bits.pos),
        1 => {}
        _ => return None,
    }
    (!bits.bit()? && !bits.bit()? && bits.bit_short()? == 2).then_some(())?;
    let body: Vec<u8> = {
        let mut copy = Bits::new(data, bits.pos, end);
        (0..copy.remaining() / 8).map(|_| copy.raw_short_byte()).collect::<Option<_>>()?
    };
    let (_, used) = crate::entities::acis::SabReader::read_with_consumed(&body).ok()?;
    bits.skip(used.checked_mul(8)?)?;
    for want in [3, 0, 0, 1, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    let mut bits = Bits::new(data, elements(data, bits.pos, end)?, end);
    (bits.bit_long()? as u32 == 0x8000_0001).then_some(())?;
    let mut bits = Bits::new(data, graph_end(data, bits.pos, end)?, end);
    (bits.bit_long()? == 0).then_some(())?;
    for _ in 0..3 {
        bits.bit_long()?;
    }
    bits.bit_double()?;
    bits.bit_double()?;
    Some(bits.pos)
}

/// The elevation-point table a site container or alignment companion
/// records after the shared class head (see [`elevation_keys`]).
#[derive(Debug, Clone, PartialEq, Default)]
struct ElevationKeys {
    /// The table's next free key.
    next_key: i32,
    /// The elevation points: `(key, the point's vertex index in its
    /// feature line, form)`, in table order.
    points: Vec<(i32, i32, i32)>,
    /// The number of form-25 markers (see [`elevation_keys`]); they carry
    /// no point.
    markers: usize,
    /// The bit position after the table's last counted entry.
    after: usize,
}

/// The elevation-point table of a site container or alignment companion
/// body: the next free key, each point's key and vertex index, and the
/// position after the table. A point the container records in the form
/// `k2` 21 keeps its elevation in the table; one in the forms `k2` 22,
/// 24 or 15 stores none here — the key and its point's index only, the
/// elevation living elsewhere in the record (in the nodes of the section
/// that follows the table on a container the application extended; the
/// forms' frames are layout-identical, the form word the only
/// difference — 8/8 verified across the tutorial grading containers, and
/// form 15 on the parcel-site containers: Labels-Parcels `Site` 0x4676
/// (13 entries), Parcel-2C `Subdivision A` 0xC38 (9), and the
/// Parcel-1C/1D/2A/2B sites (2/8/9/9), the parcel-site form word the
/// only difference from 22/24 — 41/41 entries). A grading container also
/// carries one form-25 marker: a field-less entry `BL key, BL 25` (12
/// bits, key 0) with no point. The walk stops exactly at the head's
/// count — what follows is not table grammar (the elements, see
/// [`elements`]). `None` when the head or the table does not follow the
/// layout.
///
/// ```text
/// BS 0x04BE, BL key (0 for a site container, 0x80000000 | n for a
/// companion), BL 0, BL 0, [BL 0, B 1, RC n, n x key,] BL 32,
/// 7 bits 0111010,
/// BL next free key, BL k, k x { BL key,
///   21: BD the point's index in its feature line, 17 bits, BL key,
///       BL 0, BL 0, BD elevation, B 0, BL 0,
///   22, 24 or 15: the same frame without the elevation,
///   25: nothing }
/// ```
///
/// The bracketed keys (the shared key grammar, `alignment::key`) are
/// written on companions only: one key, 0x0300D012 with byte 1, in all
/// 68 such records of the samples (8 with a non-empty topology: the
/// alignments the application made for corridor feature-line baselines,
/// and E-K4's companion once the application rebuilt that corridor).
/// The rest of the record is the plain form's bit for bit (KA07 "DCE"
/// against the same companion in E-K4-rb).
fn elevation_keys(data: &[u8], start: usize, end: usize) -> Option<ElevationKeys> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    for _ in 0..2 {
        (bits.bit_long()? == 0).then_some(())?;
    }
    match bits.bit_long()? {
        32 => {}
        // The keyed form: `BL 0, B 1, RC n, n x key, BL 32`.
        0 => {
            bits.bit()?.then_some(())?;
            for _ in 0..bits.bits_value(8)? {
                super::alignment::key(&mut bits)?;
            }
            (bits.bit_long()? == 32).then_some(())?;
        }
        _ => return None,
    }
    bits.skip(7)?;
    let next_key = bits.bit_long()?;
    let count = bits.bit_long()?;
    (0..=100_000).contains(&count).then_some(())?;
    let mut table = ElevationKeys { next_key, ..Default::default() };
    for _ in 0..count {
        let key = bits.bit_long()?;
        let form = bits.bit_long()?;
        if form == 25 {
            // A marker, not a point: no index, no flags, no second key.
            (key == 0).then_some(())?;
            table.markers += 1;
            continue;
        }
        let index = bits.bit_double()?;
        if !(index.is_finite() && index >= 0.0 && index < 100_000.0 && index.fract() == 0.0) {
            return None;
        }
        bits.skip(17)?;
        (bits.bit_long()? == key).then_some(())?;
        (bits.bit_long()? == 0).then_some(())?;
        (bits.bit_long()? == 0).then_some(())?;
        if form == 21 {
            bits.bit_double()?;
            bits.bit()?;
            bits.bit_long()?;
        } else if form != 22 && form != 24 && form != 15 {
            return None;
        }
        table.points.push((key, index as i32, form));
    }
    table.after = bits.pos;
    Some(table)
}

/// ```text
/// BS 0x04BE, BL key (0 for a site container, 0x80000000 | n for a
/// companion), BL 0, BL 0, [keys,] BL 32, 7 bits 0111010,
/// elevation points (see [`elevation_keys`]), elements (see
/// [`elements`]; `BL 1, BL 0` without gradings), BL 0x80000001, node
/// list, edge list
/// ```
pub(super) fn topology(data: &[u8], start: usize, end: usize) -> Option<Topology> {
    let read = |pos: usize, next_key: i32| -> Option<Topology> {
        let (nodes, after, next_node) = node_list(data, pos, end)?;
        let (edges, edge_flags, edge_ids, faces) = edges(data, after, end, &nodes)?;
        (!edges.is_empty()).then_some(())?;
        let (face_ids, outside_face_flags, outside_entries) = face_list(data, faces, end).map(|f| (f.0, f.1, f.2)).unwrap_or_default();
        Some(Topology { edges, edge_flags, edge_ids, face_ids, next_node, next_key, outside_face_flags, outside_entries })
    };
    let t = elevation_keys(data, start, end)?;
    let mut bits = Bits::new(data, elements(data, t.after, end)?, end);
    (bits.bit_long()? as u32 == 0x8000_0001).then_some(())?;
    read(bits.pos, t.next_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first bit writer over a byte buffer, for building the app's
    /// recorded table forms bit for bit.
    struct W {
        bytes: Vec<u8>,
        n: usize,
    }

    impl W {
        fn new() -> Self {
            Self { bytes: vec![0; 4096], n: 0 }
        }
        fn bit(&mut self, b: u8) {
            if b != 0 {
                self.bytes[self.n / 8] |= 1 << (7 - self.n % 8);
            }
            self.n += 1;
        }
        fn raw(&mut self, bytes: &[u8]) {
            for &b in bytes {
                for i in (0..8).rev() {
                    self.bit((b >> i) & 1);
                }
            }
        }
        fn code(&mut self, c: u8) {
            self.bit((c >> 1) & 1);
            self.bit(c & 1);
        }
        fn bl(&mut self, v: i32) {
            match v {
                0 => self.code(2),
                1..=255 => {
                    self.code(1);
                    self.raw(&[v as u8]);
                }
                v => {
                    self.code(0);
                    self.raw(&v.to_le_bytes());
                }
            }
        }
        fn bd(&mut self, v: f64) {
            if v == 0.0 {
                self.code(2);
            } else if v == 1.0 {
                self.code(1);
            } else {
                self.code(0);
                self.raw(&v.to_le_bytes());
            }
        }
        /// The shared class head and an elevation-point table: `entries`
        /// as `(key, form, index)` with the 17 bits between the index and
        /// the key, and the k2 21 entries carrying `668.0`.
        fn head_and_table(&mut self, entries: &[(i32, i32, f64)]) {
            self.code(0);
            self.raw(&0x04BEu16.to_le_bytes());
            self.bl(30);
            self.bl(0);
            self.bl(0);
            self.bl(32);
            for b in [0, 1, 1, 1, 0, 1, 0] {
                self.bit(b);
            }
            self.bl(33);
            self.bl(entries.len() as i32);
            for &(key, form, index) in entries {
                self.bl(key);
                self.bl(form);
                self.bd(index);
                for b in [1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0] {
                    self.bit(b);
                }
                self.bl(key);
                self.bl(0);
                self.bl(0);
                if form == 21 {
                    self.bd(668.0);
                    self.bit(0);
                    self.bl(0);
                }
            }
        }
        fn take(self) -> (Vec<u8>, usize) {
            (self.bytes[..(self.n + 7) / 8].to_vec(), self.n)
        }
    }

    /// The point forms the application writes: `k2` 21 with the elevation
    /// in the table (the form every observed container kept before an
    /// application extended it) and `k2` 22 without an elevation, as
    /// recorded bit for bit on the grading tutorial drawing (D008 capture,
    /// live-API-verified: keys 1, 3-12 with elevations and keys 20, 21,
    /// 27-32 without, each key's index its point's vertex index in its
    /// feature line).
    #[test]
    fn elevation_table_reads_both_point_forms() {
        let mut w = W::new();
        w.head_and_table(&[(8, 21, 1.0), (20, 22, 0.0)]);
        let (data, bits) = w.take();
        let table = elevation_keys(&data, 0, data.len() * 8).expect("the table reads");
        assert_eq!(table.next_key, 33);
        assert_eq!(table.points, vec![(8, 1, 21), (20, 0, 22)]);
        assert_eq!(table.markers, 0);
        // both entries are consumed: the position after the table is the
        // end of what was written
        assert_eq!(table.after, bits);
    }

    /// A table in another form refuses: the head path returns `None` and
    /// so does `topology`, instead of misreading the entries as the k2 21
    /// layout.
    #[test]
    fn elevation_table_refuses_another_form() {
        let mut w = W::new();
        w.head_and_table(&[(8, 21, 1.0), (20, 23, 0.0)]);
        let (data, _) = w.take();
        assert!(elevation_keys(&data, 0, data.len() * 8).is_none());
    }

    /// A container whose table holds a k2 22 entry feeds the head path:
    /// `topology` reads the first section after the table with the table's
    /// next key, exactly as for a k2 21-only table.
    #[test]
    fn topology_reads_the_first_section_after_a_k2_22_table() {
        let mut w = W::new();
        w.head_and_table(&[(8, 21, 1.0), (20, 22, 0.0)]);
        // BL 1, BL 0, marker, node list (next free 4, 2 nodes, 1 -> 2 -> end)
        w.bl(1);
        w.bl(0);
        w.bl(0x8000_0001u32 as i32);
        w.bl(4);
        w.bl(2);
        w.bl(1);
        for (x, y, z) in [(1.5, 2.5, 10.0), (4.5, 5.5, 12.0)] {
            w.bd(x);
            w.bd(y);
            w.bd(z);
            for b in [0, 1, 0] {
                w.bit(b);
            }
            w.bl(if x == 1.5 { 2 } else { 0 });
        }
        // edge list: next free 3, 1 slot: edge 1, flags, 2 vertices, an
        // empty key list, three zeros
        w.bl(3);
        w.bl(1);
        w.bl(0);
        w.bl(1);
        w.bl(0x1_0830);
        w.bl(2);
        w.bl(0);
        for (kind, flags, node, fa, fb) in [(0i32, 50i32, 1i32, 1i32, 1i32), (2, 2, 2, 0, 0)] {
            w.bl(kind);
            w.bl(flags);
            w.bl(node);
            w.bl(fa);
            w.bl(fb);
        }
        w.bl(0);
        w.bl(0);
        w.bl(0);
        w.bl(0);
        let (data, _) = w.take();
        let t = topology(&data, 0, data.len() * 8).expect("the first section reads");
        assert_eq!(t.edge_ids, vec![1]);
        assert_eq!(t.next_key, 33);
        assert_eq!(t.edges.len(), 1);
        assert_eq!(t.edges[0].iter().map(|v| (v.point.x, v.point.y, v.z)).collect::<Vec<_>>(), vec![(1.5, 2.5, 10.0), (4.5, 5.5, 12.0)]);
        assert_eq!(t.next_node, 4);
    }

    /// A companion in the keyed head form, as the application writes it
    /// (KA07-KA12 "E2B"/"E41", and E-K4 "DCE" once the application
    /// rebuilt the corridor; the rest of the record is the plain form's,
    /// bit for bit): `BL 0, B 1,
    /// RC 1`, the key 0x0300D012 with byte 1, then the plain form's
    /// `BL 32`. The topology reads; a third value other than 0 or 32
    /// does not.
    #[test]
    fn topology_reads_after_the_keyed_head() {
        let record = |third: i32| {
            let mut w = W::new();
            w.code(0);
            w.raw(&0x04BEu16.to_le_bytes());
            w.bl(0x8000_06AEu32 as i32);
            w.bl(0);
            w.bl(0);
            w.bl(third);
            if third == 0 {
                w.bit(1);
                w.raw(&[1]);
                w.bl(0x0300_D012);
                w.raw(&[1]);
                w.bl(32);
            }
            for b in [0, 1, 1, 1, 0, 1, 0] {
                w.bit(b);
            }
            // next free key 1, no elevation points, BL 1, BL 0, marker
            w.bl(1);
            w.bl(0);
            w.bl(1);
            w.bl(0);
            w.bl(0x8000_0001u32 as i32);
            // nodes: next free 3, two nodes 1 -> 2
            w.bl(3);
            w.bl(2);
            w.bl(1);
            for (x, next) in [(0.0, 2), (300.0, 0)] {
                w.bd(x);
                w.bd(-10.0);
                w.bd(0.0);
                for b in [0, 1, 0] {
                    w.bit(b);
                }
                w.bl(next);
            }
            // edges: next free 2, one slot: edge 1 from node 1 to node 2
            w.bl(2);
            w.bl(1);
            w.bl(0);
            w.bl(1);
            w.bl(0x30);
            w.bl(2);
            w.bl(0);
            for (kind, flags, node, fa, fb) in [(0, 2, 1, 1, 1), (2, 10, 2, 0, 0)] {
                w.bl(kind);
                w.bl(flags);
                w.bl(node);
                w.bl(fa);
                w.bl(fb);
            }
            for _ in 0..4 {
                w.bl(0);
            }
            w.take().0
        };
        let data = record(0);
        let t = topology(&data, 0, data.len() * 8).expect("the keyed head reads");
        assert_eq!(t.edges[0].iter().map(|v| (v.point.x, v.point.y)).collect::<Vec<_>>(), vec![(0.0, -10.0), (300.0, -10.0)]);
        assert_eq!(t.next_node, 3);
        let plain = record(32);
        assert_eq!(topology(&plain, 0, plain.len() * 8), Some(t));
        let other = record(16);
        assert!(topology(&other, 0, other.len() * 8).is_none());
    }

    /// A minimal modeler binary stream: the header (no records) and the
    /// end record, as the face bodies begin and end.
    fn body() -> Vec<u8> {
        let mut b = b"ASM BinaryFile4".to_vec();
        for v in [22300u32, 0, 0, 0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for text in ["Modeler", "229.2", "Mon Feb  6 04:27:52 2023"] {
            b.push(7);
            b.push(text.len() as u8);
            b.extend_from_slice(text.as_bytes());
        }
        for v in [1e-6f64, 1e-10] {
            b.push(6);
            b.extend_from_slice(&v.to_le_bytes());
        }
        for sub in ["End", "of", "ASM"] {
            b.push(0x0E);
            b.push(sub.len() as u8);
            b.extend_from_slice(sub.as_bytes());
        }
        b.extend_from_slice(&[0x0D, 4]);
        b.extend_from_slice(b"data");
        b
    }

    /// A graded site container in the layout of Grading-4 and Grading-5:
    /// the table, then `BL 4, BL 3` and three elements — a face (id 1,
    /// flags 801 with its BD 90, its body, its own graph of two nodes and
    /// one edge, parent 2, next face 3), the group (id 2, kind 2, its face
    /// 1) and a face (id 3, an empty graph, parent 2, previous face 1) —
    /// then the site graph, whose second edge carries the second of its
    /// three closing lists (`(), (1, 3), ()`). `topology` reads the site
    /// graph; one element too few leaves the walk short of the graph's
    /// marker.
    #[test]
    fn a_graded_site_reads_its_graph_after_the_elements() {
        let nodes = |w: &mut W, points: &[(f64, f64, f64)]| {
            w.bl(points.len() as i32 + 1);
            w.bl(points.len() as i32);
            w.bl(if points.is_empty() { 0 } else { 1 });
            for (i, &(x, y, z)) in points.iter().enumerate() {
                w.bd(x);
                w.bd(y);
                w.bd(z);
                for b in [0, 1, 0] {
                    w.bit(b);
                }
                w.bl(if i + 1 == points.len() { 0 } else { i as i32 + 2 });
            }
        };
        let edge = |w: &mut W, id: i32, ends: (i32, i32), lists: [&[i32]; 3]| {
            w.bl(0);
            w.bl(id);
            w.bl(0x30);
            w.bl(2);
            w.bl(0);
            for (kind, flags, node, fa, fb) in [(0, 0x32, ends.0, 1, 1), (2, 0xA, ends.1, 0, 0)] {
                w.bl(kind);
                w.bl(flags);
                w.bl(node);
                w.bl(fa);
                w.bl(fb);
            }
            w.bl(0);
            for list in lists {
                w.bl(list.len() as i32);
                for &v in list {
                    w.bl(v);
                }
            }
        };
        let fields = |w: &mut W, id: i32, kind: i32, flags: i32, bd: f64, six: [i32; 6], link: i32| {
            w.bl(id);
            w.bl(kind);
            w.bl(id);
            w.bl(flags);
            w.bd(bd);
            for v in six {
                w.bl(v);
            }
            w.bl(link);
        };
        let face = |w: &mut W, points: &[(f64, f64, f64)], tail: [i32; 3]| {
            w.bit(0);
            w.bit(0);
            w.code(1);
            w.raw(&[2]);
            w.raw(&body());
            for v in [3, 0, 0, 1, 0, 1, 0] {
                w.bl(v);
            }
            w.bl(0x8000_0001u32 as i32);
            nodes(w, points);
            if points.is_empty() {
                w.bl(1);
                w.bl(0);
            } else {
                w.bl(2);
                w.bl(1);
                edge(w, 1, (1, 2), [&[], &[], &[]]);
            }
            w.bl(2);
            w.bl(0);
            w.bl(0);
            for v in tail {
                w.bl(v);
            }
            w.bd(103.5);
            w.bd(422.25);
        };
        let record = |count: i32| {
            let mut w = W::new();
            w.bytes.resize(16384, 0);
            w.head_and_table(&[(8, 21, 1.0), (20, 22, 0.0)]);
            w.bl(4);
            w.bl(count);
            fields(&mut w, 1, 1, 801, 90.0, [24, 4, 3, 2, 0, 0], 1);
            face(&mut w, &[(5398.5, 3347.25, 761.5), (5398.5, 3327.25, 755.0)], [2, 0, 3]);
            fields(&mut w, 2, 2, 801, 0.0, [1, 2, 0, 2, 0, 0], 1);
            fields(&mut w, 3, 1, 769, 0.0, [4, 24, 3, 2, 0, 0], 9);
            face(&mut w, &[], [2, 1, 0]);
            w.bl(0x8000_0001u32 as i32);
            nodes(&mut w, &[(5398.5, 3347.25, 761.5), (5398.5, 3327.25, 755.0), (5433.75, 3327.25, 766.75)]);
            w.bl(3);
            w.bl(2);
            edge(&mut w, 1, (1, 2), [&[2], &[], &[]]);
            edge(&mut w, 2, (2, 3), [&[], &[1, 3], &[]]);
            w.bl(2);
            w.bl(0);
            w.take().0
        };
        let data = record(3);
        let t = topology(&data, 0, data.len() * 8).expect("the site graph reads");
        assert_eq!(t.edge_ids, vec![1, 2]);
        assert_eq!(t.next_key, 33);
        assert_eq!(t.edges[1].iter().map(|v| (v.point.x, v.point.y, v.z)).collect::<Vec<_>>(), vec![(5398.5, 3327.25, 755.0), (5433.75, 3327.25, 766.75)]);
        // Counting two elements, the walk stops at the third element's id
        // instead of the graph's marker.
        let short = record(2);
        assert!(topology(&short, 0, short.len() * 8).is_none());
    }
    // Generated from the 2026-11-25/26 walk-map evidence pack
    // (walkmap/k2_entry_tables.json + walkmap/full_records.jsonl): the
    // bit-exact elevation-point-table spans of the seven tutorial site
    // containers (`AECC_FEATURE_STYLE` records; the class body starts at
    // record bit 40; each span runs from the table head to the end of the
    // head's counted entries — `walked == count` on every container).
    const TABLES: &[(&str, &str, i32, usize, &[(i32, i32, i32)])] = &[
        // Grading-3.dwg C69: next_key 13, count 11, table 1987 bits, forms 21+21+21+21+21+21+21+21+21+21+21
        ("G3", "2f8111ea481d21a85a108a80000000000003081202a4228d847c66c73b68540480a2ac80a901a000000000038211012128a80000000000003881202a426883171aabe5b5854048522a0000000000001020480a90aa07cf70b86270a15012068ab202a40e8eac7787a45798540485a2a0000000000001120480a90ba24d2c2556f70e15012088a80000000000000081202a41285dc357cca979854048622a0000000000001220480a90ca126a073cfcb0e150120a8a80000000000001081202a4168535cbb205b84854048322a0000000000000820480a906a17385b6de8a8a150120e8a80000000000002881202a41e8c5555f3a0db3854040", 13, 1987, &[(8, 6, 21), (1, 1, 21), (9, 7, 21), (10, 8, 21), (3, 1, 21), (11, 9, 21), (4, 2, 21), (12, 10, 21), (5, 3, 21), (6, 4, 21), (7, 5, 21)]),
        // Grading-3A.dwg C69: next_key 33, count 19, table 2667 bits, forms 21+22+21+21+21+21+21+22+21+21+22+22+21+22+22+21+22+21+22
        ("G3A", "2f8111ea481d24289a108a80000000000003081202a4228d847c66c73b6854049022c0000000000001320402a920a405156405480d00000000001c108809094540000000000001c4090152134418b8d55f2dac2a024291500000000000008102405485503e7b85c313850a80903455901520747563bc3d22bcc2a0242d1500000000000008902405485d1269612ab7b870a8091b45a801523748222a0000000000000020480a904a1770d5f32a5e615012188a80000000000004881202a432849a81cf3f2c3854048a22d400a914a471160000000000000010200548e520a8a80000000000001081202a4168535cbb205b84854048aa2c0000000000000020400a915a47516a01548ed20c8a80000000000002081202a41a85ce16db7a2a2854048f22c0000000000001320402a91ea41d1500000000000005102405483d18aaabe741b670a8091f45a805523f40", 33, 2667, &[(8, 6, 21), (32, 11, 22), (1, 1, 21), (9, 7, 21), (10, 8, 21), (3, 1, 21), (11, 9, 21), (27, 0, 22), (4, 2, 21), (12, 10, 21), (20, 0, 22), (28, 2, 22), (5, 3, 21), (21, 2, 22), (29, 0, 22), (6, 4, 21), (30, 11, 22), (7, 5, 21), (31, 0, 22)]),
        // Grading-4.dwg BB4: next_key 25, count 5, table 421 bits, forms 24+22+22+24+25
        ("G4", "2f8111ea481d23282a308c300aa46290145a805520348122c0000000000000c20402a902a4111800000000000004102015482548c8", 25, 421, &[(24, 1, 24), (1, 0, 22), (2, 6, 22), (4, 4, 24), (0, 0, 25)]),
        // Grading-5.dwg BB4: next_key 28, count 5, table 421 bits, forms 22+22+25+24+24
        ("G5", "2f8111ea481d23882a028b500aa4069024580000000000001840805520552328da30c02a91ba411180000000000000410201548250", 28, 421, &[(1, 0, 22), (2, 6, 22), (0, 0, 25), (27, 1, 24), (4, 4, 24)]),
        // Grading-6.dwg 3533: next_key 7, count 0, table 61 bits, forms 
        ("G6", "2f8111ea481d20f0", 7, 61, &[]),
        // Grading-7.dwg 3533: next_key 26, count 5, table 485 bits, forms 22+25+24+24+22
        ("G7", "2f8111ea481d23482a108b0000000000000628100aa422a46519460000000000000224080552334852300000000000001620402a90aa41d16a015483d0", 26, 485, &[(8, 17, 22), (0, 0, 25), (25, 9, 24), (10, 14, 24), (7, 0, 22)]),
        // Grading-8.dwg 3533: next_key 28, count 6, table 671 bits, forms 22+24+25+24+21+22
        ("G8", "2f8111ea481d238832108b0000000000000628100aa42290a4600000000000002c40805521552328da300000000000001120402a91ba4311500000000000000102005486505268845ebff6f08090745a805520f4", 28, 671, &[(8, 17, 22), (10, 14, 24), (0, 0, 25), (27, 9, 24), (12, 2, 21), (7, 0, 22)]),
        // Labels-Parcels.dwg `Site` 0x4676: next_key 52, count 13, table
        // 1582 bits, forms 15 x13 — the parcel-site point form (the same
        // frame as 22/24, no elevation), 2026-11-28 round
        ("LP4676", "2f812a481d26886a508780000000000004481002a4a293043c00000000000037408015261494a1e0000000000001320400a929a4c50f0000000000000e102005498d2548780000000000005081002a4aa93243c0000000000003a408015265495a1e0000000000001520400a92ba4b10f0000000000000c502005496524a8780000000000002081002a49692d43c0000000000003240801525b49321e0000000000000c20400a926a49d0f00000000000007102005493d25e8780000000000006c81002a4be91e", 52, 1582, &[(40, 9, 15), (48, 23, 15), (41, 11, 15), (49, 24, 15), (42, 12, 15), (50, 26, 15), (43, 13, 15), (44, 17, 15), (37, 4, 15), (45, 18, 15), (38, 6, 15), (39, 7, 15), (47, 22, 15)]),
        // Parcel-2C.dwg `Subdivision A` 0xC38: next_key 11, count 9, table 866 bits, forms 15 x9
        ("P2C38", "2f8111ea481d21684a1087b002a42290143c00000000000010408015203484a1e0000000000000820400a909a4090f00000000000000102005481521487d002a42a90343d801520748221e0000000000000020400a904a4150f6005482d20e8780000000000000081002a41e91", 11, 866, &[(8, 1, 15), (1, 4, 15), (9, 4, 15), (2, 2, 15), (10, 0, 15), (3, 1, 15), (4, 2, 15), (5, 1, 15), (7, 2, 15)]),
    ];
    // Grading-7.dwg 3533: the counted table plus the 216 bits that follow
    // it: the element list's `BL 17, BL 11` (next free id, count) and the
    // first two elements up to the first face's body (`B 0, B 0, BS 2`) —
    // reading past count would misread them as a table entry (the
    // archived "form 11").
    const G7_TABLE_PLUS_ASM_HEADER: &str = "2f8111ea481d23482a108b0000000000000628100aa422a46519460000000000000224080552334852300000000000001620402a90aa41d16a015483d22285a10812100000800055482484a2080a20000180005554808810";

    /// The seven tutorial site containers' recorded tables, bit for bit
    /// (the 2026-11-25/26 walk-map evidence pack, machine-verified by the
    /// count-checked walk): every table walks to exactly its head's count
    /// — the position after the last counted entry is the recorded span's
    /// end — with the recorded keys, vertex indexes and forms, and the
    /// form-25 markers counted without reading point fields (they have
    /// none).
    #[test]
    fn recorded_tables_walk_to_their_count() {
        let mut census = [0usize; 26];
        for (name, hex, next_key, bits, entries) in TABLES {
            let data = hex_bytes(hex);
            let table = elevation_keys(&data, 0, *bits).expect(name);
            assert_eq!(table.next_key, *next_key, "{name}");
            assert_eq!(table.after, *bits, "{name}: the walk stops at count");
            let expect_points: Vec<_> = entries.iter().copied().filter(|e| e.2 != 25).collect();
            let expect_markers = entries.iter().filter(|e| e.2 == 25).count();
            assert_eq!(table.points, expect_points, "{name}");
            assert_eq!(table.markers, expect_markers, "{name}");
            for &(_, _, form) in &table.points {
                census[form as usize] += 1;
            }
            census[25] += table.markers;
        }
        // the corpus form census: 23 x 21 + 16 x 22 + 8 x 24 + 4 x 25
        // + the parcel-site form 15 (13 on Labels-Parcels `Site`,
        // 9 on Parcel-2C `Subdivision A`)
        assert_eq!((census[21], census[22], census[24], census[25], census[15]), (23, 16, 8, 4, 22));
    }

    /// Grading-7's counted table followed by the 216 bits after it (the
    /// element list's head and first elements), bit for bit as recorded:
    /// the walk stops at the head's count — the opening words (BL 17,
    /// BL 11, ..) would otherwise misread as a table entry (the archived
    /// "form 11", which does not exist).
    #[test]
    fn the_walk_stops_at_count_before_the_solid_stream() {
        let bytes = hex_bytes(G7_TABLE_PLUS_ASM_HEADER);
        let table = elevation_keys(&bytes, 0, bytes.len() * 8).expect("the table reads");
        assert_eq!(table.next_key, 26);
        assert_eq!(table.after, 485);
        assert_eq!(table.points, vec![(8, 17, 22), (25, 9, 24), (10, 14, 24), (7, 0, 22)]);
        assert_eq!(table.markers, 1);
    }


    /// Parcel-2C.dwg `Subdivision A`'s site container (0xC38), the whole
    /// record bit for bit: the interaction-area form. Its table carries
    /// the parcel-site form 15 (see [`elevation_keys`]); after it the
    /// container lists interaction records (kind 0, frames unmapped) and
    /// no site graph follows — the `BL 0x80000001` gate word never occurs
    /// in the record, so the topology reads `None` with the table decoded
    /// (2026-11-28 parcel-site round; the coordinates the records carry
    /// agree bit-exactly with the lot lines' own).
    const PARCEL_2C_SITE: &str = "6500830e2a2f8111ea481d21684a1087b002a42290143c00000000000010408015203484a1e0000000000000820400a909a4090f00000000000000102005481521487d002a42a90343d801520748221e0000000000000020400a904a4150f6005482d20e8780000000000000081002a41e915449089080540c0002a4791444244014030000a922459019010100c00009c077b0020ecebbf40646902426424040300000d4c3ec32842f88fd0591f459119110100c0000006ba0d0c9f0e3bf4264951440a408140300002701dec0083b3aefe91a4510a90a0500c0000000000000040fe3ca47d0244a44814030000183e869b0c387aefe925409039030100c00003326a651222234be40a46d0244e44c050300001d78de40083b3aefd0a92e41d049040500c00003326a651222234bea46d15452450150300001d78de40083b3aefe92e459059050140c000240e475014364340403000090792140d069060540c0002a4751443a43814030000000000001c01252f6921459079070140c00024124790143e43c0403000090892240c010000804cd1c4200060214cd9009340003a01c4b9e46490248c000c04299b201268000287808f380c8a049280008338739b765d000050f011e70191409260004c0d4995e2cb20001e023f68c072881240400100d4cd9509340003a01c4b9ea64d02484800201a99b201268000287808f3c4caa04921003008f3e9bae55000050f011e7899540925200182ada60efcb20001913bb98327248124c400c023cfa6eb954000143c0479c06450248100141f2d2bf5d2a800074038973d4c9a0492200040353365325d0000e80712e745934092640010670e736ecba0000a1e023cf132a81240c00c023cfa6eb9540001638f2153164902491800c04299b2f1308000287808f3a2c9a04904003008f3e9bae550000782c5b79c6934092480020301185f6c9a0001d00e25cf53268124b000e8facdb5f79440003a01c4b9f864902482800c04299b2a126800074038973f0c920491d00180853365e261000050f011e7899540924a0020301185f6c9a0000a1e023cf132a8124b400e8facdb5f7944000143c0479c06450248300141f2d2bf5d2a800074038973f0c920491e00180853365e261000050f011e7019140924c00040c675afbca20001d00e25cf53268124b800585d230bed9340003a01c4b9f864902483800201a99b20126800074038973fcc9a0492700020633ad7de51000050f011e7899540925e002c2e9185f6c9a0000a1e023ce032281293147a4203008000040a93241daa4050240908aaa9280c0200002aaa404300800004129b2405a76aa0c0100001246a5aa830040000499ad6a90240902a40903412a9090c020000102a6c90969d014090241eaaa4443008000040a932421ac4050240918aaa9210c020000102a6c92c6bdab409024b6a9024350ea4a430080000aaa9020c0200001059012c8003c05036a9ae08518c97fd96f58fe40430040f014a5aa6c042ea8ee20a0ad23f9010c0103c05226a9a81eef5fdf5e23f50fe40430040f014c1aa698f680e2442accd33f90240904a40902406a91a0c020000102a4c9046b5a640902486a90240502a4883008000040a9b24b9ab6b1024092faa4090f44292a0c0200002aaa40c300800004129b2415aa6b20c01000012e6a9ab8300400004b1aa6bd0240906a4090841ea91b0c020000102a4c9226a1b04090248ea90240d04a4ac30080000aaa9040c0200001029010c8003c050769daa0cbbff265e60b68fe40902406aaa4503008000041a9324250169e0c010000125405a983004000049d016b60c010000121405a68300400004c9016a1024091da40509aa47030080000aaa9240c0200002aaa4b030080000aaa9050c0200001029010c8003c05056b1aa0cbbff265e60b68fe40902422aaa4543008000040e932475016a20c010000123405b04090247aaaa4743008000040a932491a76a50240925aa4090541a9250c020000102a449326a1a6409024c2a90244512a4183008000040a40432000f01409ad6a82ae1d1e6b55fcc3f90240904aaa9160c020000106a4c91e405b08300400004a1016ba0c010000131405af8300400004b5016ae0401000012f405ac40902462901416a91e0c020000102a489266a5ad4090249ea90241d08a4b83008000040a93240dae6bd0240931aa409134529070c020000102901048003c05036bdaa0b787479ad57f30fe4090241a90142aa91f0c020000102a449286b9b0409024a6a9024250aa49c30080000aaa6c50c6c1b0419014a1016a1a8419014c2405014190141d016a5a94190149d026a9aa4190140e6adab419014b5026b1ac419014bd026b5ad41901499016b9ae419014c501699a6419014c90269da741901495026bdaf419014190196f5c901591015910159101591015910b5a1695a96b5916c5956d5996e59eaa8080300a0a0a0a0a0a0a0a0a08418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708418708423b280842ce4808423a4808423b6808423c680842c9080842ce6808423a680842c7480842c9480842ce8808423a880842c7880842cea808423aa808423cc80842c7e80842c9c80842cec808423ac808423d680842c8080842c9e808423ae808423d880842c8480842cf0808423b080842c8880842ce280842c8a80842ca08084186e808423ce808423b480842c7680842c9680842c8680842c9280842c7a80842c8280842cf281";

    /// Parcel-1B.dwg's site container (0xC38), the whole record bit for
    /// bit: the graph form. No elements; the standard graph follows the
    /// gate and reads — the parcel site the application wrote as a graph.
    const PARCEL_1B_SITE: &str = "6500830e2a2f8111ea481d20320300200010090a1884000c04299b20126800074038973c8c920491800180853364024d000050f011e70191409202000806a66ca849a0001d00e25cf532681242400100d4cd900934000143c0479e26550248100141f2d2bf5d2a800074038973d4c9a04903003008f3e9bae55000058e3c854c592409208006011e7d375caa0000f058b6f38d268124140060214cd9509340003a01c4b9f86490248e800c04299b2f1308000287808f3c4caa0490600283e5a57eba550000e80712e7e19240923c003010a66cbc4c20000a1e023ce032281241c00100d4cd9009340003a01c4b9fe64d02523286c8406010000081526483a6680a0481211555202180400002054d92029a266812048155548486010000081536484a6880a048120f555222180400002054992109a202812048c55548106010000081480964001e0281a669a0586e7c1815f40a8120481209555206180400002054d920a99a68812048355548206010000081480864001e0283a6899865dff932f305b47f204812035552281804000020549921280a68812048ed5548286010000081480864001e0282a6899865dff932f305b47f2048121155522a1804000020549923a80a68812048f55548306010000081480864001e028126899855c3a3cd6abf987f2048120955522c1804000020549923c80a68812048c55548386010000081480824001e0281a689985bc3a3cd6abf987f2048120d5549aa0680a028320283a0299a668320281c9a2688320283203202b6a02b2202b2202b2202b2202b2204b22cb5540401804211d9404211d2404211db404211e3404211d3404211d4404211d5404211e6404211d6404211eb404211d7404211ec404211d840420c37404211da404211e740";

    /// The interaction-area parcel-site container: the table decodes
    /// (Parcel-2C's recorded form-15 entries), the interaction records
    /// refuse, and the topology is `None` — the named partial.
    #[test]
    fn a_parcel_site_interaction_container_reads_its_table_and_refuses_by_name() {
        let data = hex_bytes(PARCEL_2C_SITE);
        let table = elevation_keys(&data, 40, data.len() * 8).expect("the form-15 table reads");
        assert_eq!(table.next_key, 11);
        assert_eq!(table.points, vec![(8, 1, 15), (1, 4, 15), (9, 4, 15), (2, 2, 15), (10, 0, 15), (3, 1, 15), (4, 2, 15), (5, 1, 15), (7, 2, 15)]);
        let end = data.len() * 8;
        // the interaction records (kind-0 elements) do not follow the
        // element grammar, and no graph follows them
        assert!(elements(&data, table.after, end).is_none());
        assert!(topology(&data, 40, end).is_none());
    }

    /// The graph-form parcel-site container reads: Parcel-1B's 13 lot
    /// lines with the recorded slot order, nodes and faces.
    #[test]
    fn a_parcel_site_graph_container_reads() {
        let data = hex_bytes(PARCEL_1B_SITE);
        let t = topology(&data, 40, data.len() * 8).expect("the parcel site's graph reads");
        assert_eq!(t.edges.len(), 13);
        assert_eq!(t.edge_ids, vec![8, 1, 9, 17, 2, 3, 4, 20, 5, 21, 6, 22, 7]);
        assert_eq!(t.next_node, 33);
        assert_eq!(t.next_key, 1);
        assert_eq!(t.face_ids, vec![1, 51, 52]);
        assert_eq!(t.outside_face_flags, 0x6);
        assert_eq!(t.outside_entries, vec![(7, 1)]);
    }

    /// Decodes the evidence pack's hex fixture strings.
    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
    }
}
