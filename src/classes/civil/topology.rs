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
    if !(1..=10_000).contains(&slots) || slots >= next_free {
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
        ids.push(id);
    }
    (!out.is_empty()).then_some((out, all_flags, ids, bits.pos))
}

/// The face list at `pos`: `BL next free id, BL count, per face {BL id,
/// BL id, BL flags, BL loops, per loop {BL entry node, BL entry index}}`;
/// the ids in list order and the outside face's (id 1) flags.
fn face_list(data: &[u8], pos: usize, end: usize) -> Option<(Vec<i32>, i32, Vec<(i32, i32)>)> {
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
    Some((ids, outside, entries))
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
/// `k2` 21 keeps its elevation in the table; one in the forms `k2` 22 or
/// 24 stores none here — the key and its point's index only, the
/// elevation living elsewhere in the record (in the nodes of the section
/// that follows the table on a container the application extended; the
/// two forms' frames are layout-identical, the form word the only
/// difference — 8/8 verified across the tutorial grading containers).
/// A grading container also carries one form-25 marker: a field-less
/// entry `BL key, BL 25` (12 bits, key 0) with no point. The walk stops
/// exactly at the head's count — what follows is not table grammar (on a
/// graded site an opaque ShapeManager solid stream intervenes before the
/// topology section). `None` when the head or the table does not follow
/// the layout.
///
/// ```text
/// BS 0x04BE, BL key (0 for a site container, 0x80000000 | n for a
/// companion), BL 0, BL 0, BL 32, 7 bits 0111010,
/// BL next free key, BL k, k x { BL key,
///   21: BD the point's index in its feature line, 17 bits, BL key,
///       BL 0, BL 0, BD elevation, B 0, BL 0,
///   22 or 24: the same frame without the elevation,
///   25: nothing }
/// ```
fn elevation_keys(data: &[u8], start: usize, end: usize) -> Option<ElevationKeys> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    for want in [0, 0, 32] {
        (bits.bit_long()? == want).then_some(())?;
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
        } else if form != 22 && form != 24 {
            return None;
        }
        table.points.push((key, index as i32, form));
    }
    table.after = bits.pos;
    Some(table)
}

/// ```text
/// BS 0x04BE, BL key (0 for a site container, 0x80000000 | n for a
/// companion), BL 0, BL 0, BL 32, 7 bits 0111010,
/// elevation points (see [`elevation_keys`]), BL 1, BL 0,
/// BL 0x80000001, node list, edge list
/// ```
pub(super) fn topology(data: &[u8], start: usize, end: usize) -> Option<Topology> {
    let read = |pos: usize, next_key: i32| -> Option<Topology> {
        let (nodes, after, next_node) = node_list(data, pos, end)?;
        let (edges, edge_flags, edge_ids, faces) = edges(data, after, end, &nodes)?;
        let (face_ids, outside_face_flags, outside_entries) = face_list(data, faces, end).unwrap_or_default();
        Some(Topology { edges, edge_flags, edge_ids, face_ids, next_node, next_key, outside_face_flags, outside_entries })
    };
    // A graded site container keeps its gradings (with their solids)
    // between the table and the topology section; their layout is not
    // mapped, so such a container gives no topology.
    let t = elevation_keys(data, start, end)?;
    let mut bits = Bits::new(data, t.after, end);
    bits.bit_long()?;
    bits.bit_long()?;
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

    /// The application's extended-container record, bit for bit as
    /// recorded on the grading tutorial drawing (D008 capture,
    /// live-API-verified): the table with a `k2` 21 and a `k2` 22 entry,
    /// then — behind the site's grading data, whose grammar is not mapped
    /// (2026-11-25 corpus sweep) — the feature-line section, whose edges
    /// carry inter-edge words (an other-edge key reference, empty words)
    /// the strict first-section edge list refuses. The record stays the
    /// fixture for the grammar-walking reader that will reach that
    /// section; today the head path refuses before it, so `topology`
    /// reads `None`.
    #[test]
    fn the_extended_containers_feature_line_section_stays_pending_the_walk() {
        let mut w = W::new();
        w.head_and_table(&[(8, 21, 1.0), (20, 22, 0.0)]);
        // the feature-line section: marker, node list (next free 10, 3
        // nodes, the chain 5 -> 9 -> 3 -> end)
        w.bl(0x8000_0001u32 as i32);
        w.bl(10);
        w.bl(3);
        w.bl(5);
        for (x, y, z) in [(1.5, 2.5, 10.0), (4.5, 2.5, 11.0), (4.5, 5.5, 12.0)] {
            w.bd(x);
            w.bd(y);
            w.bd(z);
            for b in [0, 1, 0] {
                w.bit(b);
            }
            w.bl(match (x, y) {
                (1.5, 2.5) => 9,
                (4.5, 2.5) => 3,
                _ => 0,
            });
        }
        // edge list: next free 50, 2 slots
        w.bl(50);
        w.bl(2);
        // slot 1: edge 42, the zero word before the vertices, a key list
        // with the table's key for vertex 1, then an other-edge key
        // reference (1, 12) and an empty word
        w.bl(0);
        w.bl(42);
        w.bl(0x1_0830);
        w.bl(3);
        w.bl(0);
        for (kind, flags, node, fa, fb) in [(0i32, 50i32, 5i32, 1i32, 2i32), (0, 0x400_430, 9, 1, 2), (2, 2, 3, 0, 0)] {
            w.bl(kind);
            w.bl(flags);
            w.bl(node);
            w.bl(fa);
            w.bl(fb);
        }
        w.bl(1);
        w.bl(8);
        w.bl(1);
        w.bl(12);
        w.bl(0);
        w.bl(0);
        // slot 2: edge 43, no zero word (the first vertex's kind is its 0)
        w.bl(0);
        w.bl(43);
        w.bl(0x1_0030);
        w.bl(2);
        w.bl(0);
        w.bl(50);
        w.bl(5);
        w.bl(1);
        w.bl(2);
        w.bl(2);
        w.bl(2);
        w.bl(3);
        w.bl(0);
        w.bl(0);
        w.bl(1);
        w.bl(7);
        w.bl(0);
        let (data, _) = w.take();
        // the table reads: next key 33, both points with their feature-line
        // vertex indexes
        let table = elevation_keys(&data, 0, data.len() * 8).expect("the table reads");
        assert_eq!(table.next_key, 33);
        assert_eq!(table.points, vec![(8, 1, 21), (20, 0, 22)]);
        // the section does not read yet: the fixture has no `BL 1, BL 0`
        // between the table and the section (the extended container keeps
        // other words there), so the head path refuses, and the strict
        // first-section edge list refuses the section's inter-edge words
        assert!(topology(&data, 0, data.len() * 8).is_none());
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
    ];
    // Grading-7.dwg 3533: the counted table plus the trailing
    // 216-bit ShapeManager stream header (magic-anchored kernel data,
    // opaque); its header begins (BL 17, BL 11, ..) — reading past count
    // would misread it as a table entry (the archived "form 11").
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
        assert_eq!((census[21], census[22], census[24], census[25]), (23, 16, 8, 4));
    }

    /// Grading-7's counted table followed by the trailing 216-bit
    /// ShapeManager stream header, bit for bit as recorded: the walk
    /// stops at the head's count and never reads the header — its opening
    /// words (BL 17, BL 11, ..) would otherwise misread as a table entry
    /// (the archived "form 11", which does not exist).
    #[test]
    fn the_walk_stops_at_count_before_the_solid_stream() {
        let bytes = hex_bytes(G7_TABLE_PLUS_ASM_HEADER);
        let table = elevation_keys(&bytes, 0, bytes.len() * 8).expect("the table reads");
        assert_eq!(table.next_key, 26);
        assert_eq!(table.after, 485);
        assert_eq!(table.points, vec![(8, 17, 22), (25, 9, 24), (10, 14, 24), (7, 0, 22)]);
        assert_eq!(table.markers, 1);
    }

    /// Decodes the evidence pack's hex fixture strings.
    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
    }
}
