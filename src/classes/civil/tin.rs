//! TIN surface triangulation.
//!
//! The surface body stores its triangulation as one block:
//!
//! ```text
//! BL  vertex count
//! BL  x4  indices of the four bounding corner vertices
//! vertex count x { RL flags, RL one incident triangle, RD x, RD y, RD z }
//! BL  triangle count
//! triangle count x { RL v0, v1, v2, RL n0, n1, n2, RL flags, RL reserved }
//! ```
//!
//! Vertices run counter-clockwise; `n[i]` is the triangle across the edge
//! that starts at `v[i]`, or -1 on the hull. The four corner vertices (flag
//! 0x40) span the surface extents, and the triangles that use them have
//! flag bit 0 set and are not drawn. A second copy of the block follows the
//! first; the first is the built surface.

use super::bits::Bits;
use crate::types::{Vector2, Vector3};

const CORNER_FLAG: i32 = 0x40;
const HIDDEN_FLAG: i32 = 0x1;
const DELETED_FLAG: i32 = 0x4;
const VERTEX_BITS: usize = 2 * 32 + 3 * 64;
const TRIANGLE_BITS: usize = 8 * 32;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinVertex {
    pub position: Vector3,
    /// 0x80 for a surface point, 0x40 for a bounding corner; bit 0 marks a
    /// deleted vertex (it keeps its place, with no triangle).
    pub flags: i32,
    /// One triangle that uses this vertex.
    pub triangle: i32,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinTriangle {
    /// Vertex indices, counter-clockwise; `u32::MAX` in a deleted triangle.
    pub vertices: [u32; 3],
    /// Neighbor across the edge starting at the same-index vertex, -1 on the hull.
    pub neighbors: [i32; 3],
    /// Bit 0: hidden, bit 2: deleted (an edit removed it). Higher bits are
    /// kept as stored.
    pub flags: i32,
}

impl TinTriangle {
    pub fn is_visible(&self) -> bool {
        self.flags & (HIDDEN_FLAG | DELETED_FLAG) == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinSurface {
    pub vertices: Vec<TinVertex>,
    pub triangles: Vec<TinTriangle>,
    /// Indices of the four bounding corner vertices.
    pub corners: [u32; 4],
}

impl TinSurface {
    /// Corner positions of every drawn triangle.
    pub fn visible_triangles(&self) -> impl Iterator<Item = [Vector3; 3]> + '_ {
        self.triangles
            .iter()
            .filter(|triangle| triangle.is_visible())
            .map(|triangle| triangle.vertices.map(|v| self.vertices[v as usize].position))
    }
}

/// The triangulation block of a surface body.
pub(super) fn find(data: &[u8], start: usize, end: usize) -> Option<TinSurface> {
    if let Some((surface, _)) = triangulation_at(data, start, end).and_then(|pos| read_at(data, pos, end)) {
        return Some(surface);
    }
    // debt: surfaces whose first definition is none of the forms below or
    // that hold an operation kind not mapped in [`operation`] are found by
    // probing (none in the samples).
    (start..end).find_map(|pos| read_at(data, pos, end)).map(|(s, _)| s)
}

/// The triangulation a snapshot keeps: after the surface's triangulation
/// `BL 0, BL n` (1 with a snapshot), then "MMS01" and the snapshot's
/// triangulation (TS01 to TS04); `None` without a snapshot.
pub(super) fn snapshot(data: &[u8], start: usize, end: usize) -> Option<TinSurface> {
    let (_, pos) = read_at(data, triangulation_at(data, start, end)?, end)?;
    let mut bits = Bits::new(data, pos, end);
    bits.bit_long()?;
    (bits.bit_long()? == 1).then_some(())?;
    b"MMS01".iter().all(|c| bits.raw_short_byte() == Some(*c)).then_some(())?;
    read_at(data, bits.pos, end).map(|(s, _)| s)
}

/// Lines of a breakline, contour or boundary definition.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinLines {
    /// Drawing objects the lines are taken from (the definition's sources).
    pub objects: u32,
    /// Lines held by the definition itself (made from points).
    pub polylines: Vec<Vec<Vector3>>,
    /// Arcs become chords no farther than this from the arc.
    pub mid_ordinate: f64,
    /// Supplementing: no segment longer than this (0: off).
    pub maximum_distance: f64,
    /// Weeding: vertices closer than this are dropped (0: off) ...
    pub weeding_distance: f64,
    /// ... unless the line turns more than this (degrees).
    pub weeding_angle: f64,
}

/// Boundary kinds, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TinBoundaryType {
    Show = 1,
    Hide = 2,
    Outer = 4,
    DataClip = 8,
}

/// Breakline kinds; the value is the definition kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TinBreaklineType {
    Standard = 14,
    Proximity = 15,
    NonDestructive = 24,
}

/// One step of a surface definition.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TinOperation {
    /// Points added to the surface.
    AddPoints(Vec<Vector3>),
    /// A point group (the definition's source).
    PointGroup,
    Breaklines(TinBreaklineType, TinLines),
    Contours(TinLines),
    Boundary {
        boundary_type: TinBoundaryType,
        /// The boundary cuts the triangles it crosses instead of dropping them.
        non_destructive: bool,
        lines: TinLines,
    },
    AddVertex(Vector3),
    /// The vertex at this plan position.
    DeleteVertex(Vector2),
    MoveVertex { from: Vector2, to: Vector2 },
    /// The vertex at `x, y` gets elevation `z`.
    VertexElevation(Vector3),
    /// The edge through this plan position.
    SwapEdge(Vector2),
    AddLine(Vector2, Vector2),
    /// The edge through this plan position.
    DeleteLine(Vector2),
    Raise(f64),
    /// Points read from a file (its path is in the texts), with the file's
    /// date as a FILETIME.
    Import { file_time: u64, value: i32 },
    /// The vertices inside this plan polygon.
    Polygon(Vec<Vector2>),
    /// A 4x4 matrix, row by row.
    Transform([f64; 16]),
    /// Another surface pasted in (the definition's source).
    PasteSurface,
    /// Points or lines copied from drawing objects (`object_type` 0 points,
    /// 1 lines); `maintain_edges` keeps the lines as triangle edges.
    DrawingObjects { object_type: i32, maintain_edges: bool, points: Vec<Vector3> },
    /// Points read from a point file (its path is the definition's text, its
    /// format the source), with the file's date as a FILETIME and three
    /// option bits as stored (use adjusted elevations, transform the
    /// coordinates, expand the coordinate data; all off in the samples).
    PointFile { file_time: u64, options: u8 },
    /// A corridor surface (the corridor is the source; the identifier is
    /// the corridor surface's).
    CorridorSurface([u8; 16]),
    /// Kind 40, a snapshot: the triangulation the definitions before it
    /// build is kept after the surface's (`ArchEntityData::tin_snapshot`); the
    /// value is 0 (1 in drawings of older releases).
    Kind40(i32),
}

/// One definition: its operation and identifier, and its texts and
/// source objects (from the record's texts and references).
///
/// Texts: a name (point group, lines, boundary, pasted surface, corridor
/// surface), a description (drawing objects), the file (point file), or the
/// name and the file (import). Sources: the point group, the objects lines
/// come from, the pasted surface, the point file format, the corridor.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinDefinition {
    pub operation: TinOperation,
    pub id: [u8; 16],
    pub texts: Vec<String>,
    pub sources: Vec<u64>,
}

impl TinDefinition {
    /// Texts the definition takes from the record's text list.
    pub fn text_count(&self) -> usize {
        match &self.operation {
            TinOperation::Import { .. } => 2,
            TinOperation::PointGroup | TinOperation::Breaklines(..) | TinOperation::Contours(_) | TinOperation::Boundary { .. } | TinOperation::PasteSurface | TinOperation::DrawingObjects { .. } | TinOperation::PointFile { .. } | TinOperation::CorridorSurface(_) => 1,
            _ => 0,
        }
    }

    /// References the definition takes after the record's hard owner.
    pub fn source_count(&self) -> usize {
        match &self.operation {
            TinOperation::PointGroup | TinOperation::PasteSurface | TinOperation::PointFile { .. } | TinOperation::CorridorSurface(_) => 1,
            TinOperation::Breaklines(_, l) | TinOperation::Contours(l) | TinOperation::Boundary { lines: l, .. } => l.objects as usize,
            _ => 0,
        }
    }
}

/// The definitions of a surface record.
pub(super) struct Definitions {
    pub list: Vec<TinDefinition>,
    pub auto_rebuild: bool,
}

pub(super) const AUTO_REBUILD: i32 = 0x0100_0000;

/// The definitions block:
///
/// ```text
/// head: 10 bits 8 + 32 x count, RC 2 + 8 x count, RC (4 with a boundary,
///   else 2), RC (0x81 with contours, else 0x80);
/// 2 bits, BL 0, 2 bits 00, RC 0x0C (0x0A and a byte, 8 x the snapshot's
///   index, once the application rebuilt the snapshot: TS03, TO03 to TO05),
///   RC 0x80 + breakline count / 2, B,
/// count x { BL kind, BL flags, BL 16, GUID, BL named, data (`read_operation`) },
/// BL 0, BL options (0x01000000: rebuild automatically), 2 bits
/// ```
// ponytail: the head's other bits, the two bits after it, the 0x0C field
// and the bit are skipped here; the writer stores them as the samples show them.
pub(super) fn read_definitions(bits: &mut Bits) -> Option<Definitions> {
    let first = usize::from(bits.bits_value(2)?) << 8 | usize::from(bits.bits_value(8)?);
    let count = bits.raw_short_byte()?;
    bits.raw_short_byte()?;
    // The byte keeps the highest count the surface had (a removed snapshot
    // leaves it one above: TS05); the ten bits hold the current one.
    let n = (first >= 8 && (first - 8) % 32 == 0).then(|| (first - 8) / 32)?;
    if bits.raw_short_byte()? & 0x80 == 0 || usize::from(count >> 3) < n {
        return None;
    }
    bits.skip(2)?;
    bits.bit_long()?;
    bits.skip(2)?;
    if bits.bits_value(8)? == 0x0A {
        bits.skip(8)?;
    }
    bits.skip(9)?;
    let list = (0..n).map(|_| read_operation(bits)).collect::<Option<_>>()?;
    bits.bit_long()?;
    let options = bits.bit_long()?;
    bits.skip(2)?;
    Some(Definitions { list, auto_rebuild: options & AUTO_REBUILD != 0 })
}

fn vector2(bits: &mut Bits) -> Option<Vector2> {
    Some(Vector2::new(bits.bit_double()?, bits.bit_double()?))
}

fn vector3(bits: &mut Bits) -> Option<Vector3> {
    Some(Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?))
}

fn points(bits: &mut Bits) -> Option<Vec<Vector3>> {
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 6 <= bits.remaining())?;
    (0..n).map(|_| vector3(bits)).collect()
}

/// `BL objects, BD mid-ordinate, BL m, m x { BL n, n x 3BD }, BL k, k x BL
/// (1 to k, one per object or held line; k is also the definition's
/// `named` value), BD maximum distance, BD weeding distance, BD weeding
/// angle`.
fn lines(bits: &mut Bits) -> Option<TinLines> {
    let objects = u32::try_from(bits.bit_long()?).ok()?;
    let mid_ordinate = bits.bit_double()?;
    let m = usize::try_from(bits.bit_long()?).ok().filter(|m| *m <= bits.remaining())?;
    let polylines = (0..m).map(|_| points(bits)).collect::<Option<_>>()?;
    for _ in 0..bits.bit_long()? {
        bits.bit_long()?;
    }
    Some(TinLines {
        objects,
        polylines,
        mid_ordinate,
        maximum_distance: bits.bit_double()?,
        weeding_distance: bits.bit_double()?,
        weeding_angle: bits.bit_double()?,
    })
}

fn guid(bits: &mut Bits) -> Option<[u8; 16]> {
    let mut g = [0u8; 16];
    for b in &mut g {
        *b = bits.raw_short_byte()?;
    }
    Some(g)
}

/// One definition: `BL kind, BL flags, BL 16, GUID, BL named`, then by kind:
///
/// ```text
/// 47 points:                    BL n, n x 3BD
///  0 point group:               nothing (the group is a source)
/// 14, 15, 24 breaklines:        lines (`lines`)
/// 27 contours:                  lines, BL
/// 13 boundary:                  lines, BL type, BL non-destructive
///  1 add vertex, 20 elevation:  3BD
///  2 delete vertex, 9 swap edge, 23 delete line: 2BD
///  8 move vertex, 22 add line:  4BD
/// 17 raise:                     BD
/// 33 import:                    BL, BL (a FILETIME, high half first), BL
/// 35 polygon:                   BL n, n x { RD x, RD y }
/// 34 transform:                 16 RD (row by row), BL
/// 40:                           BL
/// 12 paste surface:             nothing (the surface is a source)
/// 31 drawing objects:           BL object type, BL maintain edges, BL n, n x 3BD
/// 30 point file:                BL, BL (a FILETIME, high half first), 3 bits
/// 41 corridor surface:          GUID
/// ```
pub(super) fn read_operation(bits: &mut Bits) -> Option<TinDefinition> {
    let kind = bits.bit_long()?;
    bits.bit_long()?;
    bits.bit_long()?;
    let id = guid(bits)?;
    bits.bit_long()?;
    let operation = match kind {
        47 => TinOperation::AddPoints(points(bits)?),
        0 => TinOperation::PointGroup,
        14 => TinOperation::Breaklines(TinBreaklineType::Standard, lines(bits)?),
        15 => TinOperation::Breaklines(TinBreaklineType::Proximity, lines(bits)?),
        24 => TinOperation::Breaklines(TinBreaklineType::NonDestructive, lines(bits)?),
        27 => {
            let l = lines(bits)?;
            bits.bit_long()?;
            TinOperation::Contours(l)
        }
        13 => {
            let lines = lines(bits)?;
            let boundary_type = match bits.bit_long()? {
                1 => TinBoundaryType::Show,
                2 => TinBoundaryType::Hide,
                4 => TinBoundaryType::Outer,
                8 => TinBoundaryType::DataClip,
                _ => return None,
            };
            TinOperation::Boundary { boundary_type, non_destructive: bits.bit_long()? != 0, lines }
        }
        1 => TinOperation::AddVertex(vector3(bits)?),
        20 => TinOperation::VertexElevation(vector3(bits)?),
        2 => TinOperation::DeleteVertex(vector2(bits)?),
        9 => TinOperation::SwapEdge(vector2(bits)?),
        23 => TinOperation::DeleteLine(vector2(bits)?),
        8 => TinOperation::MoveVertex { from: vector2(bits)?, to: vector2(bits)? },
        22 => TinOperation::AddLine(vector2(bits)?, vector2(bits)?),
        17 => TinOperation::Raise(bits.bit_double()?),
        33 => {
            let high = bits.bit_long()? as u32;
            let low = bits.bit_long()? as u32;
            TinOperation::Import { file_time: (u64::from(high) << 32) | u64::from(low), value: bits.bit_long()? }
        }
        35 => {
            let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 128 <= bits.remaining())?;
            TinOperation::Polygon((0..n).map(|_| Some(Vector2::new(bits.raw_double()?, bits.raw_double()?))).collect::<Option<_>>()?)
        }
        34 => {
            let mut m = [0.0; 16];
            for v in &mut m {
                *v = bits.raw_double()?;
            }
            bits.bit_long()?;
            TinOperation::Transform(m)
        }
        40 => TinOperation::Kind40(bits.bit_long()?),
        12 => TinOperation::PasteSurface,
        31 => TinOperation::DrawingObjects { object_type: bits.bit_long()?, maintain_edges: bits.bit_long()? != 0, points: points(bits)? },
        30 => {
            let high = bits.bit_long()? as u32;
            let low = bits.bit_long()? as u32;
            TinOperation::PointFile { file_time: (u64::from(high) << 32) | u64::from(low), options: bits.bits_value(3)? }
        }
        41 => TinOperation::CorridorSurface(guid(bits)?),
        _ => return None,
    };
    Some(TinDefinition { operation, id, texts: Vec::new(), sources: Vec::new() })
}

/// The count of definitions a record's head keeps, when the head is in
/// the counted form (`10 bits 8 + 32 x count`, see
/// [`read_definitions`]). `None` in the application's no-definitions
/// form — the empty shell (`Surface-2.dwg` "EG") keeps another 10-bit
/// field there and no definitions block; its marker follows the head's
/// 111-bit run (see [`triangulation_at`]).
pub(super) fn definitions_count(data: &[u8], start: usize, end: usize) -> Option<usize> {
    let at = definitions_at(data, start, end)?;
    let mut bits = Bits::new(data, at, end);
    let first = usize::from(bits.bits_value(2)?) << 8 | usize::from(bits.bits_value(8)?);
    (first >= 8 && (first - 8) % 32 == 0).then(|| (first - 8) / 32)
}

/// Where the definitions start: the graph head (see
/// profile::graph_head), 3BD origin, 9 BD, BD, `RC n`, `n` x 5 bits.
pub(super) fn definitions_at(data: &[u8], start: usize, end: usize) -> Option<usize> {
    let mut bits = Bits::new(data, start, end);
    super::profile::graph_head(&mut bits)?;
    for _ in 0..13 {
        bits.bit_double()?;
    }
    let items = usize::from(bits.bits_value(8)?);
    bits.skip(5 * items)?;
    Some(bits.pos)
}

/// After the definitions: RL, `RS 0x05A2` (0x04A2 in newer drawings),
/// `RC k`, `k` x 2 bytes, "MMS01"; returns where the triangulation starts.
fn marker(bits: &mut Bits) -> Option<usize> {
    bits.raw_long()?;
    matches!(u16::from_le_bytes([bits.raw_short_byte()?, bits.raw_short_byte()?]), 0x05A2 | 0x04A2).then_some(())?;
    let k = usize::from(bits.raw_short_byte()?);
    bits.skip(16 * k)?;
    (b"MMS01".iter().all(|c| bits.raw_short_byte() == Some(*c))).then_some(bits.pos)
}

/// The definitions of a surface body, checked by the marker that follows.
pub(super) fn definitions(data: &[u8], start: usize, end: usize) -> Option<Definitions> {
    let mut bits = Bits::new(data, definitions_at(data, start, end)?, end);
    let found = read_definitions(&mut bits)?;
    marker(&mut bits)?;
    Some(found)
}

/// Where the triangulation starts: after the definitions, or (a surface
/// without definitions, and the older format) after 111 bits.
// ponytail: the 111 bits are skipped by length (the same in every sample).
fn triangulation_at(data: &[u8], start: usize, end: usize) -> Option<usize> {
    let at = definitions_at(data, start, end)?;
    let mut bits = Bits::new(data, at, end);
    if let Some(pos) = read_definitions(&mut bits).and_then(|_| marker(&mut bits)) {
        return Some(pos);
    }
    let mut bits = Bits::new(data, at, end);
    bits.skip(111)?;
    marker(&mut bits)
}

/// A triangulation at `pos` and where it ends. A surface with no
/// triangulation keeps its shell: the four bounding corners at the drawing
/// extremes (`Surface-2.dwg` "EG", the snapshot-only surface of
/// `Pipe Networks-3B.dwg`), with no incident triangle and no triangles.
fn read_at(data: &[u8], pos: usize, end: usize) -> Option<(TinSurface, usize)> {
    let mut bits = Bits::new(data, pos, end);
    let count = usize::try_from(bits.bit_long()?).ok()?;
    if count < 4 || count.checked_mul(VERTEX_BITS)? > bits.remaining() {
        return None;
    }
    let mut corners = [0u32; 4];
    for corner in &mut corners {
        *corner = u32::try_from(bits.bit_long()?).ok()?;
        if *corner as usize >= count {
            return None;
        }
    }
    let first = bits.raw_long()?;
    if first & (CORNER_FLAG | 0x80) == 0 {
        return None;
    }
    bits.pos -= 32;
    let mut vertices = Vec::with_capacity(count);
    for _ in 0..count {
        let flags = bits.raw_long()?;
        let triangle = bits.raw_long()?;
        let position = Vector3::new(bits.raw_double()?, bits.raw_double()?, bits.raw_double()?);
        if !(position.x.is_finite() && position.y.is_finite() && position.z.is_finite()) {
            return None;
        }
        vertices.push(TinVertex {
            position,
            flags,
            triangle,
        });
    }
    if corners
        .iter()
        .any(|&c| vertices[c as usize].flags & CORNER_FLAG == 0)
    {
        return None;
    }
    let triangle_count = usize::try_from(bits.bit_long()?).ok()?;
    if triangle_count.checked_mul(TRIANGLE_BITS)? > bits.remaining() {
        return None;
    }
    // A corner-only shell (the application's record for a surface it has
    // not built) has no triangles; anything else needs at least the hull.
    if triangle_count == 0 && count != 4 {
        return None;
    }
    let mut triangles = Vec::with_capacity(triangle_count);
    for _ in 0..triangle_count {
        let mut v = [0i32; 8];
        for x in &mut v {
            *x = bits.raw_long()?;
        }
        let flags = v[6];
        // A deleted triangle keeps its place (neighbors count it) with -1 vertices.
        let deleted = flags & DELETED_FLAG != 0;
        let mut vertex = [0u32; 3];
        for (out, &x) in vertex.iter_mut().zip(&v[..3]) {
            if !(deleted && x == -1) && (x < 0 || x as usize >= count) {
                return None;
            }
            *out = x as u32;
        }
        let neighbors = [v[3], v[4], v[5]];
        if neighbors.iter().any(|n| *n < -1 || *n as i64 >= triangle_count as i64) {
            return None;
        }
        triangles.push(TinTriangle {
            vertices: vertex,
            neighbors,
            flags,
        });
    }
    Some((
        TinSurface {
            vertices,
            triangles,
            corners,
        },
        bits.pos,
    ))
}

/// Join `(a, b)` segments sharing end keys into polylines of keys.
fn chain<K: Copy + Eq + std::hash::Hash>(segments: &[(K, K)]) -> Vec<Vec<K>> {
    use std::collections::HashMap;
    let mut at: HashMap<K, Vec<usize>> = HashMap::new();
    for (i, (a, b)) in segments.iter().enumerate() {
        at.entry(*a).or_default().push(i);
        at.entry(*b).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    // Open chains start at a key one segment touches; the rest are loops.
    let mut starts: Vec<usize> = (0..segments.len())
        .filter(|&i| at[&segments[i].0].len() == 1 || at[&segments[i].1].len() == 1)
        .collect();
    starts.extend(0..segments.len());
    for first in starts {
        if used[first] {
            continue;
        }
        used[first] = true;
        let (a, b) = segments[first];
        let mut keys = if at[&a].len() == 1 { vec![a, b] } else { vec![b, a] };
        loop {
            let tail = *keys.last().unwrap();
            let Some(&next) = at[&tail].iter().find(|&&i| !used[i]) else {
                break;
            };
            used[next] = true;
            let (p, q) = segments[next];
            keys.push(if p == tail { q } else { p });
        }
        out.push(keys);
    }
    out
}

impl TinSurface {
    fn drawn(&self, triangle: i32) -> bool {
        triangle >= 0
            && self
                .triangles
                .get(triangle as usize)
                .is_some_and(TinTriangle::is_visible)
    }

    /// Outline of the drawn triangles: every edge without a drawn neighbor,
    /// joined into loops.
    pub fn boundary(&self) -> Vec<Vec<Vector3>> {
        let mut edges = Vec::new();
        for triangle in self.triangles.iter().filter(|t| t.is_visible()) {
            for edge in 0..3 {
                if !self.drawn(triangle.neighbors[edge]) {
                    edges.push((triangle.vertices[edge], triangle.vertices[(edge + 1) % 3]));
                }
            }
        }
        chain(&edges)
            .into_iter()
            .map(|keys| keys.iter().map(|&v| self.vertices[v as usize].position).collect())
            .collect()
    }

    /// Contour polylines at every multiple of `interval`, with their
    /// elevation. A vertex exactly at a contour elevation counts as above it.
    pub fn contours(&self, interval: f64) -> Vec<(f64, Vec<Vector3>)> {
        use std::collections::HashMap;
        if !(interval > 0.0) {
            return Vec::new();
        }
        type Edge = (u32, u32);
        let key = |a: u32, b: u32| (a.min(b), a.max(b));
        let mut levels: HashMap<i64, Vec<(Edge, Edge)>> = HashMap::new();
        for triangle in self.triangles.iter().filter(|t| t.is_visible()) {
            let v = triangle.vertices;
            let z = v.map(|i| self.vertices[i as usize].position.z);
            let low = (z.iter().copied().fold(f64::INFINITY, f64::min) / interval).ceil() as i64;
            let high = (z.iter().copied().fold(f64::NEG_INFINITY, f64::max) / interval).floor() as i64;
            for level in low..=high {
                let h = level as f64 * interval;
                let crossed: Vec<Edge> = (0..3)
                    .filter(|&e| (z[e] >= h) != (z[(e + 1) % 3] >= h))
                    .map(|e| key(v[e], v[(e + 1) % 3]))
                    .collect();
                if let [a, b] = crossed[..] {
                    levels.entry(level).or_default().push((a, b));
                }
            }
        }
        let mut out = Vec::new();
        let mut ordered: Vec<_> = levels.into_iter().collect();
        ordered.sort_by_key(|(level, _)| *level);
        for (level, segments) in ordered {
            let h = level as f64 * interval;
            let at = |(a, b): Edge| {
                let (p, q) = (self.vertices[a as usize].position, self.vertices[b as usize].position);
                let t = (h - p.z) / (q.z - p.z);
                Vector3::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t, h)
            };
            for keys in chain(&segments) {
                out.push((h, keys.into_iter().map(at).collect()));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    /// The empty-surface shell of the tutorials' Surface-2.dwg ("EG"): the
    /// four bounding corners at the drawing extremes, no triangles.
    fn empty_shell() -> String {
        let max = "1".repeat(48) + "11101111 01111111"; // RD f64::MAX (0x7FEF_FFFF_FFFF_FFFF)
        let corner = |i: &str| match i {
            "0" => "10".to_string(),
            i => format!("01 {:08b}", i.parse::<u8>().unwrap()),
        };
        let vertex = format!("01000000 00000000 00000000 00000000 {} {max} {max} {max}", "1".repeat(32));
        format!(
            "01 00000100 {} {} {} {} {} {} {} {} 10",
            corner("0"),
            corner("1"),
            corner("2"),
            corner("3"),
            vertex,
            vertex,
            vertex,
            vertex
        )
    }

    #[test]
    fn reads_an_empty_surface_shell() {
        let b = bits_of(&empty_shell());
        let (surface, end) = read_at(&b, 0, b.len() * 8).unwrap();
        assert_eq!(surface.vertices.len(), 4);
        assert!(surface.vertices.iter().all(|v| v.flags == 0x40 && v.triangle == -1));
        assert_eq!(surface.corners, [0, 1, 2, 3]);
        assert!(surface.triangles.is_empty());
        assert!(surface.visible_triangles().next().is_none());
        assert_eq!(end, empty_shell().chars().filter(|c| *c == '0' || *c == '1').count());
        // The shell is exactly four corners: any other vertex count with no
        // triangles is not a triangulation.
        let five = empty_shell().replacen("00000100", "00000101", 1);
        assert!(read_at(&bits_of(&five), 0, five.len() * 8).is_none());
    }

    #[test]
    fn reads_a_raise_and_a_kind_40_then_stops_at_an_unknown_kind() {
        // BL 17, BL 9, BL 16, GUID, BL 0, BD 1.5; BL 40, BL 9, BL 16, GUID,
        // BL 0, BL 1; then BL 41, BL 9, BL 16.
        let guid = "0".repeat(128);
        let raise = format!("0100010001 0100001001 0100010000 {guid} 10 00{}", "0".repeat(48) + "1111100000111111");
        let kind_40 = format!("0100101000 0100001001 0100010000 {guid} 10 0100000001");
        let b = bits_of(&format!("{raise} {kind_40} 0100101001 0100001001 0100010000 {guid} 10 {}", "0".repeat(16)));
        let mut bits = Bits::new(&b, 0, b.len() * 8);
        assert_eq!(read_operation(&mut bits).unwrap().operation, TinOperation::Raise(1.5));
        assert_eq!(bits.pos, 30 + 128 + 2 + 66);
        assert_eq!(read_operation(&mut bits).unwrap().operation, TinOperation::Kind40(1));
        assert_eq!(bits.pos, 2 * (30 + 128 + 2) + 66 + 10);
        assert!(read_operation(&mut bits).is_none());
    }

    #[test]
    fn reads_the_head_of_a_created_and_of_a_rebuilt_snapshot() {
        let guid = "0".repeat(128);
        let kind_40 = format!("0100101000 0100001001 0100010000 {guid} 10 10");
        let tail = format!("00{} 00{} 00", "0".repeat(32), "0".repeat(32));
        for field in ["00001100 10000000 1", "00001010 00001000 10000000 1"] {
            let b = bits_of(&format!("0000101000 00001010 00000010 10000000 10 10 00 {field} {kind_40} {tail}"));
            let mut bits = Bits::new(&b, 0, b.len() * 8);
            let found = read_definitions(&mut bits).unwrap();
            assert_eq!(found.list[0].operation, TinOperation::Kind40(0));
            assert_eq!(bits.pos, 40 + field.replace(' ', "").len() + 162 + 70);
        }
    }
}
