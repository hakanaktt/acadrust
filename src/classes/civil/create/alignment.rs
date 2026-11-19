//! Alignments of fixed lines with free arcs and spiral-curve-spirals
//! between them (`AECC_ALIGNMENT`).

use super::support::*;
use super::tree::{ensure_alignment_tree, KEY_ALIGNMENT_STYLES, KEY_DEFAULT_FEATURE, KEY_SITELESS};
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::classes::civil::alignment::{pieces_with_curvature, AlignmentElement, AlignmentElementKind, AlignmentPiece};
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// Element kind of a line, and the constraint of a fixed line.
const LINE: i32 = 257;
const FIXED: i16 = 0;
/// Element kind of an arc, and the constraint of an arc fitted between
/// two lines.
const ARC: i32 = 258;
const BETWEEN_LINES: i32 = 29;
/// Element kind of a spiral-curve-spiral, its constraint between two
/// lines, and the spiral definition of WA04's group.
const SPIRAL_CURVE_SPIRAL: i32 = 260;
const SCS_BETWEEN_LINES: i32 = 0;
const SPIRAL_DEFINITION: i32 = 256;
/// Flags of an arc edge in the companion topology, besides the line flags.
pub(super) const ARC_EDGE: i32 = 0x010F_0000;

/// What an alignment does at an inner vertex.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VertexCurve {
    /// The lines meet at the vertex.
    None,
    /// A free arc of this radius, tangent to the lines before and after
    /// the vertex.
    Arc(f64),
    /// A free spiral-curve-spiral between the lines: spirals of these
    /// lengths and an arc of this radius.
    SpiralCurveSpiral { spiral_in: f64, radius: f64, spiral_out: f64 },
}

impl VertexCurve {
    fn is_some(self) -> bool {
        self != VertexCurve::None
    }

    fn valid(self) -> bool {
        let positive = |v: f64| v.is_finite() && v > 0.0;
        match self {
            VertexCurve::None => true,
            VertexCurve::Arc(r) => positive(r),
            VertexCurve::SpiralCurveSpiral { spiral_in, radius, spiral_out } => positive(spiral_in) && positive(radius) && positive(spiral_out),
        }
    }
}

/// The design elements of an alignment through `vertices`: the lines (ids
/// 1..n in station order, from vertex to vertex) then a curve element per
/// curved inner vertex (ids after the lines, in station order), linked
/// along the path.
pub(super) fn elements(vertices: &[Vector2], curves: &[VertexCurve]) -> Vec<AlignmentElement> {
    let n = vertices.len() - 1;
    let mut next = n as i16;
    let ids: Vec<Option<i16>> = curves
        .iter()
        .map(|c| {
            c.is_some().then(|| {
                next += 1;
                next
            })
        })
        .collect();
    let mut out: Vec<AlignmentElement> = (0..n)
        .map(|i| {
            let id = i as i16 + 1;
            AlignmentElement {
                id,
                previous: if i == 0 { 0 } else { ids[i - 1].unwrap_or(id - 1) },
                next: if i + 1 == n { 0 } else { ids[i].unwrap_or(id + 1) },
                kind: AlignmentElementKind::FixedLine { start: vertices[i], end: vertices[i + 1] },
            }
        })
        .collect();
    for (j, c) in curves.iter().enumerate() {
        let Some(id) = ids[j] else { continue };
        let kind = match *c {
            VertexCurve::Arc(radius) => AlignmentElementKind::FreeArc { radius },
            VertexCurve::SpiralCurveSpiral { spiral_in, radius, spiral_out } => AlignmentElementKind::FreeSpiralCurveSpiral { spiral_in, radius, spiral_out },
            VertexCurve::None => continue,
        };
        out.push(AlignmentElement { id, previous: j as i16 + 1, next: j as i16 + 2, kind });
    }
    out
}
/// Key of the geometry stamp setting.
const STAMP: i32 = 0x0B00_1081;

/// The drawing's default alignment style: the first style of the style
/// tree's alignment style collection.
pub(super) fn default_style(doc: &CadDocument) -> Option<u64> {
    let tree = root_node(doc, 31)?;
    let o = object(doc, tree)?;
    let (data, start, end) = fields_of(o)?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    (b.bit_long()? == 31).then_some(())?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let keys = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let head = *o.decode().handles.get(keys.iter().position(|k| *k == KEY_ALIGNMENT_STYLES)?)?;
    object(doc, Handle::new(head))?.decode().handles.first().copied().filter(|h| *h != 0)
}

/// First text id given to a written companion (kept apart from the other
/// writers' ids).
const FIRST_COMPANION_TAG: u32 = 0x8003_0001;

/// The default feature style of the siteless alignments.
fn companion_node(doc: &CadDocument) -> Option<Handle> {
    let node = read_node(doc, root_node(doc, KEY_SITELESS)?)?;
    node.children.iter().find(|c| c.0 == KEY_DEFAULT_FEATURE).map(|c| Handle::new(c.1))
}

/// A 32-digit hexadecimal name, unique per call.
fn new_name(seed: u64) -> String {
    use std::hash::{BuildHasher, Hasher};
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let half = |salt: u64| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(now);
        h.write_u64(seed ^ salt);
        h.finish()
    };
    format!("{:016x}{:016x}", half(1), half(2))
}

/// A point of the solved alignment path and the segment that starts there:
/// `None` a line, `Some(bulge)` an arc (`tan` of a quarter of its signed
/// angle, positive to the left).
pub(super) type PathPoint = (Vector2, Option<f64>);

/// The solved path of `elements` (the crate's own solver): each line from
/// its start, each arc from its start with its bulge, each spiral as
/// chords between its solved points, then the end. `None` when a curved
/// vertex has no turn or a curve is not positive, or the curves do not fit
/// on the lines (a line would run backwards or a piece is missing).
pub(super) fn solve(vertices: &[Vector2], curves: &[VertexCurve], elements: &[AlignmentElement]) -> Option<Vec<PathPoint>> {
    for (j, c) in curves.iter().enumerate() {
        let (u, v) = (vertices[j + 1] - vertices[j], vertices[j + 2] - vertices[j + 1]);
        let turn = u.cross(&v).atan2(u.dot(&v));
        (c.valid() && (!c.is_some() || (turn.abs() > 1e-9 && turn.abs() < std::f64::consts::PI - 1e-9))).then_some(())?;
    }
    let pieces = pieces_with_curvature(elements);
    let curve_pieces: usize = curves
        .iter()
        .map(|c| match c {
            VertexCurve::None => 0,
            VertexCurve::Arc(_) => 1,
            VertexCurve::SpiralCurveSpiral { .. } => 3,
        })
        .sum();
    let scale = vertices.iter().map(|p| p.x.abs().max(p.y.abs())).fold(1.0, f64::max);
    let mut path: Vec<PathPoint> = Vec::new();
    let mut line = 0;
    for (kind, k, points) in &pieces {
        let (first, last) = (*points.first()?, *points.last()?);
        match kind {
            AlignmentPiece::Line => {
                let along = *vertices.get(line + 1)? - vertices[line];
                (along.dot(&(last - first)) >= -1e-9 * scale * along.length()).then_some(())?;
                line += 1;
                if first.distance(&last) > 1e-12 * scale {
                    path.push((first, None));
                }
            }
            AlignmentPiece::Curve => {
                let sweep = 2.0 * (first.distance(&last) * k.abs() / 2.0).min(1.0).asin() * k.signum();
                path.push((first, Some((sweep / 4.0).tan())));
            }
            _ => path.extend(points[..points.len() - 1].iter().map(|p| (*p, None))),
        }
    }
    (pieces.len() == vertices.len() - 1 + curve_pieces && line == vertices.len() - 1).then_some(())?;
    path.push((*vertices.last()?, None));
    Some(path)
}

/// The companion of a siteless alignment (`AECC_FEATURE_STYLE` under the
/// default feature style): the alignment's site topology, its solved
/// path. Nodes are the path's ends (ids 1, 2) then its inner points (ids
/// 3...); the one edge runs through them with the outside face (1) on
/// both sides.
///
/// ```text
/// BS 0x04BE, BL text id, BL 0 (no children), BL 0, BL 32, BL 210,
/// 21 bits 000000110010000000110, BL 0x80000001,
/// nodes: BL next free id, BL n, BL first id, n x {3BD, 3 bits 010, BL next id}
/// edges: BL 2 (next free id), BL 1 slot, BL 0, BL 1 (id), BL 48, BL count, BL 0,
///        count x {BL kind (0 line, 1 arc, 2 last), BL flags, BL node, BL face,
///                 BL face, an arc: BD bulge, BL 0},
///        BL 0, BL 0 x 3
/// faces: BL 2, BL 1, the outside face {BL 1, BL 1, BL 7, BL 1, BL 1, BL 0}, BL 0,
///        6 x {BL 1, BL 100}, 6 x BL 0
/// references: the alignment (soft pointer), three empty soft pointers
/// texts: its name (32 hexadecimal digits), ""
/// ```
///
/// A segment's flags are 2, with 0x30 added when another segment follows
/// it and 0x010F0000 for an arc; the last point's are 10 (WA02, WA03).
fn write_companion(doc: &mut CadDocument, handle: Handle, owner: Handle, tag: u32, alignment: Handle, path: &[PathPoint], name: &str) -> Result<(), DxfError> {
    let n = path.len();
    let node_of = |i: usize| -> i32 {
        if i == 0 {
            1
        } else if i + 1 == n {
            2
        } else {
            i as i32 + 2
        }
    };
    let mut order: Vec<usize> = vec![0, n - 1];
    order.extend(1..n - 1);
    put_object(
        doc,
        "AECC_FEATURE_STYLE",
        handle,
        owner,
        Vec::new(),
        |w| {
            for v in [tag as i32, 0, 0, 32, 210] {
                w.write_bit_long(v);
            }
            bits(w, "000000110010000000110");
            w.write_bit_long(0x8000_0001_u32 as i32);
            w.write_bit_long(n as i32 + 1);
            w.write_bit_long(n as i32);
            w.write_bit_long(1);
            for (k, i) in order.iter().enumerate() {
                for v in [path[*i].0.x, path[*i].0.y, 0.0] {
                    w.write_bit_double(v);
                }
                bits(w, "010");
                w.write_bit_long(if k + 1 == n { 0 } else { k as i32 + 2 });
            }
            for v in [2, 1, 0, 1, 48, n as i32, 0] {
                w.write_bit_long(v);
            }
            for (i, (_, arc)) in path.iter().enumerate() {
                let last = i + 1 == n;
                let arc = arc.filter(|_| !last);
                let flags = if last { 10 } else if i + 2 < n { 0x32 } else { 2 };
                let face = if last { 0 } else { 1 };
                let kind = if last { 2 } else if arc.is_some() { 1 } else { 0 };
                for v in [kind, flags | if arc.is_some() { ARC_EDGE } else { 0 }, node_of(i), face, face] {
                    w.write_bit_long(v);
                }
                if let Some(bulge) = arc {
                    w.write_bit_double(bulge);
                    w.write_bit_long(0);
                }
            }
            for _ in 0..4 {
                w.write_bit_long(0);
            }
            for v in [2, 1, 1, 1, 7, 1, 1, 0, 0] {
                w.write_bit_long(v);
            }
            for _ in 0..6 {
                w.write_bit_long(1);
                w.write_bit_long(100);
            }
            for _ in 0..6 {
                w.write_bit_long(0);
            }
        },
        Body { refs: vec![soft_pointer(alignment.value()), soft_pointer(0), soft_pointer(0), soft_pointer(0)], strings: vec![name.to_string(), String::new()] },
    )
}

/// Writes the alignment record. `feature` is the companion the alignment
/// refers to (twice).
///
/// ```text
/// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010101
/// RC 1, key 0x0B001081: BL 2, BL 0, BL 0 (the geometry stamp; the
///   application keeps a value derived from the geometry and accepts 0)
/// BL 0 (byte form), 115 zero bits, 8 bits 00000010, RC 0x40 | edge id >>
///   2, 2 bits edge id & 3, 5 bits 11010 (the edge id: the alignment's edge
///   in its companion, 1, or its site's container, its member number; read
///   before as BD 2.25 and 7 bits 0111010, edge 1)
/// BL 1 (first element id), BD 0, BD 0 (start station)
/// 7 bits 0000011, BD 0 (track gauge), B 0, BL 1
/// BL n, the lines (ids 1.. in station order) then the arcs (ids after
///   them, in station order):
///   line: BL 257, BS 2, B 0, BS 0 (fixed), BS id, BS previous id,
///         BS next id, 2RD start, 2RD end (the vertices before and after)
///   arc:  BL 258, BS 2, B 0, B 0, BD -1 (no design speed), BD -1, BS 1,
///         BD -1, BL 29 (fitted between two lines), B 0, BS id,
///         BS previous id, BS next id, BD radius
///   spiral-curve-spiral: BL 260, the arc's head with BL 0 (fitted
///         between two lines) and no B, BS id, BS previous id, BS next id,
///         BL 256 (spiral definition), BD spiral in, BD radius, BD spiral
///         out
/// BL n, BL 0, BL 0, BL n (the profiles), the newest first:
///   the profile's identifier (a version-4 GUID, 16 bytes), B 1, BL 32763, BL 0 x 3, BD end station of the first element, BL 1, BL 0,
///   BL element count (0 for one element), 6 RD (0, 0, that end station,
///   0, 0, 0)
/// BL 0 (equations), BL 1, BL 0 (speeds)
/// BD 100, BL 0 (no superelevation), BL 0, BL 1 (centerline),
/// BL 2, 2, 2, 0, 3, B 0, BS 0x04BE
/// references: soft pointer 0, companion, companion, soft pointer 0,
///   style, hard pointer 0, the profiles (soft pointers, oldest first)
/// texts: "AeccImpAlignment", "", "", name, five empty texts, three more
///   per curve element, two more per profile
/// ```
///
/// Written for WA03's and WA04's polylines and curves, the record equals
/// the samples' bit for bit apart from the geometry stamp.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_alignment(doc: &mut CadDocument, common: EntityCommon, name: &str, elements: &[AlignmentElement], style: u64, feature: u64, edge: i32, keys: Option<&str>, profiles: &[ProfileRef]) -> Result<Handle, DxfError> {
    if !(1..256).contains(&edge) {
        return Err(invalid("the alignment's edge id is out of range"));
    }
    let curves = elements.iter().filter(|e| !matches!(e.kind, AlignmentElementKind::FixedLine { .. })).count();
    // The profiles' entries hold the end station of the alignment's first
    // element and its element count (0 for one).
    let first_end = pieces_with_curvature(elements).first().map_or(0.0, |(_, _, p)| p.windows(2).map(|w| w[0].distance(&w[1])).sum());
    let count = if elements.len() > 1 { elements.len() as i32 } else { 0 };
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let mut refs = vec![soft_pointer(0), hard_pointer(feature), hard_pointer(feature), soft_pointer(0), hard_pointer(style), hard_pointer(0)];
    refs.extend(profiles.iter().map(|p| soft_pointer(p.handle)));
    let mut strings = vec!["AeccImpAlignment".to_string(), String::new(), String::new(), name.to_string()];
    strings.extend((0..5 + 3 * curves + 2 * profiles.len()).map(|_| String::new()));
    put_entity(
        doc,
        "AECC_ALIGNMENT",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
                w.write_bit_long(v);
            }
            bits(w, "010101");
            match keys {
                // An alignment written again keeps its keys as stored.
                Some(k) => bits(w, k),
                None => {
                    byte(w, 1);
                    w.write_bit_long(STAMP);
                    w.write_bit_long(2);
                    w.write_bit_long(0);
                    w.write_bit_long(0);
                }
            }
            bl_byte(w, 0);
            // The edge id ends two bits after the run's byte 0x40 (its low
            // bits carry the id's high bits).
            bits(w, &"0".repeat(115));
            bits(w, &format!("00000010{:08b}{:02b}11010", 0x40 | (edge >> 2), edge & 3));
            w.write_bit_long(1);
            w.write_bit_double(0.0);
            w.write_bit_double(0.0);
            bits(w, "0000011");
            w.write_bit_double(0.0);
            w.write_bit(false);
            w.write_bit_long(1);
            w.write_bit_long(elements.len() as i32);
            for e in elements {
                let ids = |w: &mut DwgMergedWriter| {
                    w.write_bit_short(e.id);
                    w.write_bit_short(e.previous);
                    w.write_bit_short(e.next);
                };
                // A curve element's head: no design speed (BD -1), the
                // parameter block, the constraint.
                let curve = |w: &mut DwgMergedWriter, kind: i32, constraint: i32| {
                    w.write_bit_long(kind);
                    w.write_bit_short(2);
                    w.write_bit(false);
                    w.write_bit(false);
                    w.write_bit_double(-1.0);
                    w.write_bit_double(-1.0);
                    w.write_bit_short(1);
                    w.write_bit_double(-1.0);
                    w.write_bit_long(constraint);
                };
                match e.kind {
                    AlignmentElementKind::FixedLine { start, end } => {
                        w.write_bit_long(LINE);
                        w.write_bit_short(2);
                        w.write_bit(false);
                        w.write_bit_short(FIXED);
                        ids(w);
                        for p in [start, end] {
                            w.write_raw_double(p.x);
                            w.write_raw_double(p.y);
                        }
                    }
                    AlignmentElementKind::FreeArc { radius } => {
                        curve(w, ARC, BETWEEN_LINES);
                        w.write_bit(false);
                        ids(w);
                        w.write_bit_double(radius);
                    }
                    AlignmentElementKind::FreeSpiralCurveSpiral { spiral_in, radius, spiral_out } => {
                        curve(w, SPIRAL_CURVE_SPIRAL, SCS_BETWEEN_LINES);
                        ids(w);
                        w.write_bit_long(SPIRAL_DEFINITION);
                        for v in [spiral_in, radius, spiral_out] {
                            w.write_bit_double(v);
                        }
                    }
                    _ => {}
                }
            }
            let n = profiles.len() as i32;
            for v in [n, 0, 0, n] {
                w.write_bit_long(v);
            }
            for p in profiles.iter().rev() {
                w.write_bytes(&p.id);
                w.write_bit(true);
                for v in [32763, 0, 0, 0] {
                    w.write_bit_long(v);
                }
                w.write_bit_double(first_end);
                for v in [1, 0, count] {
                    w.write_bit_long(v);
                }
                for v in [0.0, 0.0, first_end, 0.0, 0.0, 0.0] {
                    w.write_raw_double(v);
                }
            }
            for v in [0, 1, 0] {
                w.write_bit_long(v);
            }
            w.write_bit_double(100.0);
            for v in [0, 0, 1, 2, 2, 2, 0, 3] {
                w.write_bit_long(v);
            }
            w.write_bit(false);
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// Adds an alignment through `vertices` (fixed lines, no curves) on
/// `layer` (layer "0" when `None`). A drawing without an alignment style
/// collection or a node for siteless alignments gets them (`tree.rs`).
pub fn add_alignment(doc: &mut CadDocument, name: &str, vertices: &[Vector2], layer: Option<&str>) -> Result<Handle, DxfError> {
    add_alignment_with_curves(doc, name, vertices, &vec![VertexCurve::None; vertices.len().saturating_sub(2)], layer)
}

/// Adds an alignment of fixed lines through `vertices` with `curves[j]` at
/// inner vertex `j + 1`: a free arc or spiral-curve-spiral tangent to the
/// lines before and after the vertex, which end where it meets them. The
/// curves must fit on the lines between them.
pub fn add_alignment_with_curves(doc: &mut CadDocument, name: &str, vertices: &[Vector2], curves: &[VertexCurve], layer: Option<&str>) -> Result<Handle, DxfError> {
    if vertices.len() < 2 || vertices.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || vertices.windows(2).any(|w| w[0] == w[1]) {
        return Err(invalid("an alignment needs two or more distinct finite vertices"));
    }
    if curves.len() + 2 != vertices.len() {
        return Err(invalid("an alignment needs one curve per inner vertex"));
    }
    let elements = elements(vertices, curves);
    let path = solve(vertices, curves, &elements).ok_or_else(|| invalid("a curve has no turn or is not positive, or the curves do not fit on the lines"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    if default_style(doc).is_none() || companion_node(doc).is_none() {
        ensure_alignment_tree(doc)?;
    }
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no alignment styles"))?;
    let node = companion_node(doc).ok_or_else(|| invalid("drawing has no node for siteless alignments"))?;
    let used = read_node(doc, node).ok_or_else(|| invalid("feature style node cannot be read"))?;
    let tag = (FIRST_COMPANION_TAG..).find(|t| used.children.iter().all(|c| c.0 as u32 != *t)).unwrap_or(FIRST_COMPANION_TAG);
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let companion = doc.allocate_handle();
    let handle = common.handle;
    let label = new_name(handle.value());
    write_companion(doc, companion, node, tag, handle, &path, &label)?;
    add_node_child(doc, node, tag as i32, companion, &label)?;
    write_alignment(doc, common, name, &elements, style, companion.value(), 1, None, &[])
}

/// A profile of an alignment: its record and its identifier.
pub(super) struct ProfileRef {
    pub handle: u64,
    pub id: [u8; 16],
}

/// What the alignment writer needs to write an alignment again: its
/// common data, name, elements, style, companion and class keys (their
/// count byte and keys, as stored).
pub(super) struct Written {
    pub common: EntityCommon,
    pub name: String,
    pub elements: Vec<AlignmentElement>,
    pub style: u64,
    pub feature: u64,
    pub keys: String,
    pub edge: i32,
}

/// An alignment this crate can write again: its elements are the ones the
/// writer writes, its references are the writer's six and profiles, and
/// writing it from what is read gives back its record bit for bit (so
/// nothing the application stores is lost). Returns it with its profiles.
pub(super) fn rewritable(doc: &CadDocument, alignment: Handle, profile_id: impl Fn(u64) -> Option<[u8; 16]>) -> Option<(Written, Vec<ProfileRef>)> {
    let Some(crate::entities::EntityType::Extended(x)) = doc.get_entity(alignment) else { return None };
    let crate::entities::ExtendedEntityData::Arch(a) = &x.data else { return None };
    (a.dxf_name == "AECC_ALIGNMENT").then_some(())?;
    let elements = a.alignment_elements()?;
    elements.iter().all(|e| matches!(e.kind, AlignmentElementKind::FixedLine { .. } | AlignmentElementKind::FreeArc { .. } | AlignmentElementKind::FreeSpiralCurveSpiral { .. })).then_some(())?;
    let refs = a.typed_handles();
    (refs.len() >= 6).then_some(())?;
    let profiles = refs[6..].iter().map(|(h, _)| Some(ProfileRef { handle: *h, id: profile_id(*h)? })).collect::<Option<Vec<_>>>()?;
    let d = a.decode();
    let name = d.strings.get(3)?.clone();
    let (data, start, end) = a.main_stream()?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    for _ in 0..6 {
        b.bit_long()?;
    }
    b.skip(6)?;
    let from = b.pos;
    for _ in 0..b.bits_value(8)? {
        crate::classes::civil::alignment::read_key(&mut b)?;
    }
    let keys: String = (from..b.pos).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    b.skip(10 + 115 + 8)?;
    let high = b.bits_value(8)?;
    let edge = (i32::from(high & 0x3F) << 2) | i32::from(b.bits_value(2)?);
    let w = Written { common: x.common.clone(), name, elements, style: refs[4].0, feature: refs[1].0, keys, edge };
    Some((w, profiles))
}

/// Writes an alignment again with `profiles`, after checking that writing
/// it with the profiles it has reproduces its record.
pub(super) fn rewrite(doc: &mut CadDocument, alignment: Handle, w: Written, before: &[ProfileRef], profiles: &[ProfileRef]) -> Result<(), DxfError> {
    let bits_of = |doc: &CadDocument| -> Option<Vec<u8>> {
        match doc.get_entity(alignment)? {
            crate::entities::EntityType::Extended(x) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => {
                    let (data, start, end) = a.main_stream()?;
                    Some((start..end).map(|i| data[i / 8] >> (7 - i % 8) & 1).collect())
                }
                _ => None,
            },
            _ => None,
        }
    };
    let original = doc.get_entity(alignment).cloned().ok_or_else(|| invalid("alignment not found"))?;
    let old = bits_of(doc);
    write_alignment(doc, w.common.clone(), &w.name, &w.elements, w.style, w.feature, w.edge, Some(&w.keys), before)?;
    if bits_of(doc) != old {
        *doc.get_entity_mut(alignment).ok_or_else(|| invalid("alignment not found"))? = original;
        return Err(invalid("the alignment holds data this crate does not write"));
    }
    write_alignment(doc, w.common, &w.name, &w.elements, w.style, w.feature, w.edge, Some(&w.keys), profiles)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// A drawing with stand-in nodes (only the keys and references the
    /// alignment writer looks up): a style tree whose alignment styles
    /// list one style, and the node of siteless alignments with its
    /// default feature style.
    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let root = ensure_civil(&mut doc).unwrap();
        let [tree, head, style, siteless, feature] = [(); 5].map(|_| doc.allocate_handle());
        let node = |doc: &mut CadDocument, handle: Handle, owner: Handle, dxf: &str, key: i32, children: &[(i32, Handle)]| {
            let refs = children.iter().map(|c| soft_owner(c.1.value())).collect();
            put_object(doc, dxf, handle, owner, Vec::new(), |w| {
                w.write_bit_long(key);
                w.write_bit_long(children.len() as i32);
                for c in children {
                    w.write_bit_long(c.0);
                }
                w.write_bit_long(0);
                bits(w, "010");
            }, Body { refs, strings: vec![String::new()] })
            .unwrap();
        };
        node(&mut doc, tree, root, "AECC_TREE_NODE", 31, &[(KEY_ALIGNMENT_STYLES, head)]);
        node(&mut doc, head, tree, "AECC_TREE_NODE", KEY_ALIGNMENT_STYLES, &[(30, style)]);
        node(&mut doc, style, head, "AECC_TREE_NODE", 30, &[]);
        node(&mut doc, siteless, root, "AECC_TREE_NODE", KEY_SITELESS, &[(KEY_DEFAULT_FEATURE, feature)]);
        node(&mut doc, feature, siteless, "AECC_FEATURE_STYLE", KEY_DEFAULT_FEATURE, &[]);
        set_root_entry(&mut doc, 31, tree).unwrap();
        set_root_entry(&mut doc, KEY_SITELESS, siteless).unwrap();
        doc
    }

    #[test]
    fn alignments_round_trip() {
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 50.0)];
        assert!(add_alignment(&mut doc, "AL1", &v[..1], None).is_err());
        let first = add_alignment(&mut doc, "AL1", &v, None).unwrap();
        let second = add_alignment(&mut doc, "AL2", &v[1..], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-alignment-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let read = |h: Handle| match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => (a.alignment_name(), a.alignment_start_station(), a.alignment_polylines()),
                _ => panic!(),
            },
            _ => panic!(),
        };
        assert_eq!(read(first), (Some("AL1".into()), Some(0.0), vec![v.to_vec()]));
        assert_eq!(read(second).2, vec![v[1..].to_vec()]);
        let feature = companion_node(&back).unwrap();
        assert_eq!(read_node(&back, feature).unwrap().children.len(), 2);
    }

    #[test]
    fn a_new_drawing_gets_its_alignment_style_and_siteless_node() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)];
        let h = add_alignment(&mut doc, "AL1", &v, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-alignment-new-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        for key in [31, 5, 45, KEY_SITELESS] {
            assert!(root_node(&back, key).is_some(), "root key {key}");
        }
        let style = default_style(&back).unwrap();
        assert_eq!(object(&back, Handle::new(style)).unwrap().dxf_name, "AECC_ALIGNMENT_STYLE");
        assert_eq!(read_node(&back, companion_node(&back).unwrap()).unwrap().children.len(), 1);
        match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => assert_eq!(a.alignment_polylines(), vec![v.to_vec()]),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn curves_round_trip() {
        use crate::classes::civil::{AlignmentElementKind, AlignmentPiece};
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, -50.0), Vector2::new(300.0, -50.0), Vector2::new(350.0, 50.0)];
        let (arc, none) = (VertexCurve::Arc, VertexCurve::None);
        let scs = VertexCurve::SpiralCurveSpiral { spiral_in: 20.0, radius: 50.0, spiral_out: 20.0 };
        assert!(add_alignment_with_curves(&mut doc, "X", &v, &[arc(50.0)], None).is_err());
        assert!(add_alignment_with_curves(&mut doc, "X", &v, &[arc(5000.0), none, none], None).is_err());
        assert!(add_alignment_with_curves(&mut doc, "X", &v, &[arc(-5.0), none, none], None).is_err());
        assert!(add_alignment_with_curves(&mut doc, "X", &[v[0], v[1], Vector2::new(200.0, 0.0)], &[arc(10.0)], None).is_err());
        let h = add_alignment_with_curves(&mut doc, "AL1", &v, &[arc(50.0), none, arc(40.0)], None).unwrap();
        let s = add_alignment_with_curves(&mut doc, "AL2", &v[..3], &[scs], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-alignment-curves-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let a = match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let elements = a.alignment_elements().unwrap();
        let links: Vec<(i16, i16, i16)> = elements.iter().map(|e| (e.id, e.previous, e.next)).collect();
        assert_eq!(links, vec![(1, 0, 5), (2, 5, 3), (3, 2, 6), (4, 6, 0), (5, 1, 2), (6, 3, 4)]);
        assert_eq!(elements[4].kind, AlignmentElementKind::FreeArc { radius: 50.0 });
        assert_eq!(elements[0].kind, AlignmentElementKind::FixedLine { start: v[0], end: v[1] });
        let pieces = a.alignment_pieces();
        let kinds: Vec<AlignmentPiece> = pieces.iter().map(|p| p.0).collect();
        assert_eq!(kinds, vec![AlignmentPiece::Line, AlignmentPiece::Curve, AlignmentPiece::Line, AlignmentPiece::Line, AlignmentPiece::Curve, AlignmentPiece::Line]);
        // The first arc starts at its tangent point, R tan(delta / 2) before
        // the vertex.
        let t = 50.0 * (0.5f64.atan() / 2.0).tan();
        assert!((pieces[1].1[0].x - (100.0 - t)).abs() < 1e-9);
        // The companion holds the solved path: the ends, the tangent points
        // and the uncurved vertex.
        let c = read_node(&back, companion_node(&back).unwrap()).unwrap().children[0].1;
        assert_eq!(object(&back, Handle::new(c)).unwrap().decode().handles[0], h.value());

        // The spiral-curve-spiral round trips as one element between the
        // lines and solves to spiral, curve, spiral.
        let a = match back.get_entity(s) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let elements = a.alignment_elements().unwrap();
        assert_eq!(elements[2].kind, AlignmentElementKind::FreeSpiralCurveSpiral { spiral_in: 20.0, radius: 50.0, spiral_out: 20.0 });
        let kinds: Vec<AlignmentPiece> = a.alignment_pieces().iter().map(|p| p.0).collect();
        assert_eq!(kinds, vec![AlignmentPiece::Line, AlignmentPiece::Spiral, AlignmentPiece::Curve, AlignmentPiece::Spiral, AlignmentPiece::Line]);
    }
}
