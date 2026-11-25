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

/// The topology in a site container or alignment companion body:
///
/// The elevation-point table after the shared class head: the next free
/// key, each point's `(key, vertex index in its feature line)`, and the
/// position after the table. A point the container records in the form
/// `k2` 21 keeps its elevation in the table; one in the form `k2` 22
/// stores none here — the key and its point's index only, the elevation
/// living elsewhere in the record (in the nodes of the section that
/// follows the table on a container the application extended). `None`
/// when the head or the table does not follow the layout.
///
/// ```text
/// BS 0x04BE, BL key (0 for a site container, 0x80000000 | n for a
/// companion), BL 0, BL 0, BL 32, 7 bits 0111010,
/// BL next free key, BL k, k x { BL key, BL 21 or 22, BD the point's
///   index in its feature line, 17 bits, BL key, BL 0, BL 0,
///   21: BD elevation, B 0, BL 0 }
/// ```
fn elevation_keys(data: &[u8], start: usize, end: usize) -> Option<(i32, Vec<(i32, i32)>, usize)> {
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
    let mut keys = Vec::new();
    for _ in 0..count {
        let key = bits.bit_long()?;
        let form = bits.bit_long()?;
        let index = bits.bit_double()?;
        if !(index.is_finite() && index >= 0.0 && index < 100_000.0 && index.fract() == 0.0) {
            return None;
        }
        bits.skip(17)?;
        (bits.bit_long()? == key).then_some(())?;
        (bits.bit_long()? == 0).then_some(())?;
        (bits.bit_long()? == 0).then_some(())?;
        keys.push((key, index as i32));
        if form == 21 {
            bits.bit_double()?;
            bits.bit()?;
            bits.bit_long()?;
        } else if form != 22 {
            return None;
        }
    }
    Some((next_key, keys, bits.pos))
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
    if let Some((next_key, _, after)) = elevation_keys(data, start, end) {
        let mut bits = Bits::new(data, after, end);
        let head = (|| {
            bits.bit_long()?;
            bits.bit_long()?;
            (bits.bit_long()? as u32 == 0x8000_0001).then_some(())
        })();
        if head.is_some() {
            return read(bits.pos, next_key);
        }
    }
    // debt: a container whose head does not follow the layout above is
    // read by trying positions for its node list.
    (start..end.saturating_sub(40)).find_map(|pos| read(pos, 0))
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

    /// The two elevation-point forms the application writes: `k2` 21 with
    /// the elevation in the table (the form every observed container kept
    /// before an application extended it) and `k2` 22 without an
    /// elevation, as recorded bit for bit on the grading tutorial drawing
    /// (D008 capture, live-API-verified: keys 1, 3-12 with elevations and
    /// keys 20, 21, 27-32 without, each key's index its point's vertex
    /// index in its feature line).
    #[test]
    fn elevation_table_reads_both_point_forms() {
        let mut w = W::new();
        w.head_and_table(&[(8, 21, 1.0), (20, 22, 0.0)]);
        let (data, bits) = w.take();
        let (next_key, keys, after) = elevation_keys(&data, 0, data.len() * 8).expect("the table reads");
        assert_eq!(next_key, 33);
        assert_eq!(keys, vec![(8, 1), (20, 0)]);
        // both entries are consumed: the position after the table is the
        // end of what was written
        assert_eq!(after, bits);
    }

    /// A table in another form refuses: the head path returns `None` and
    /// `topology` falls to the documented scan fallback instead of
    /// misreading the entries as the k2 21 layout.
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
}
