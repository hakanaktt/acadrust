//! Alignments of fixed lines with free arcs and spiral-curve-spirals
//! between them (`AECC_ALIGNMENT`).

use super::support::*;
use super::tree::{ensure_alignment_tree, KEY_ALIGNMENT_STYLES, KEY_DEFAULT_FEATURE, KEY_SITELESS};
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::classes::civil::alignment::{CriticalStationData, SuperelevationCurveData, MANUAL_SETTINGS, MANUAL_STATION, SUPERELEVATION_SEGMENTS, pieces_with_curvature, AlignmentAnchor, AlignmentElement, AlignmentElementKind, AlignmentPiece};
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// Element kind of a line, and the constraint of a fixed line.
const LINE: i32 = 257;
const FIXED: i16 = 0;
/// Element kind of an arc, and the constraint of an arc fitted between
/// two lines.
const ARC: i32 = 258;
const BETWEEN_LINES: i32 = 29;
/// Constraint of a fixed arc (an offset alignment's arcs).
const FIXED_ARC: i32 = 2;
/// Element kind and constraint of a widening transition of an offset
/// alignment (its points).
const TRANSITION: i32 = 267;
const POINT_SETS: i32 = 3;
/// Element kind of a spiral-curve-spiral, its constraint between two
/// lines, and the spiral definition of WA04's group.
const SPIRAL_CURVE_SPIRAL: i32 = 260;
const SCS_BETWEEN_LINES: i32 = 0;
const SPIRAL_DEFINITION: i32 = 256;
/// Element kind of a spiral.
const SPIRAL: i32 = 259;
/// Flags of an arc edge in the companion topology, besides the line flags.
pub(super) const ARC_EDGE: i32 = 0x010F_0000;

/// The type of a written alignment: a centerline, a utility alignment or a
/// rail alignment with its track gauge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlignmentKind {
    Centerline,
    Utility,
    Rail { gauge: f64 },
}

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
/// Key of a curb return's curve parameters (a text, CG02).
const KEY_CURB_CURVE: i32 = 0x0400_D783;

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
    for piece in &pieces {
        let (kind, _, points) = piece;
        if *kind == AlignmentPiece::Line {
            let (first, last) = (*points.first()?, *points.last()?);
            let along = *vertices.get(line + 1)? - vertices[line];
            (along.dot(&(last - first)) >= -1e-9 * scale * along.length()).then_some(())?;
            line += 1;
        }
        push_piece(&mut path, piece, scale);
    }
    (pieces.len() == vertices.len() - 1 + curve_pieces && line == vertices.len() - 1).then_some(())?;
    path.push((*vertices.last()?, None));
    Some(path)
}

/// A solved piece added to a path: a line from its start (unless it has no
/// length), an arc from its start with its bulge, a spiral as chords
/// between its points.
fn push_piece(path: &mut Vec<PathPoint>, (kind, k, points): &(AlignmentPiece, f64, Vec<Vector2>), scale: f64) {
    let (first, last) = (points[0], points[points.len() - 1]);
    match kind {
        AlignmentPiece::Line => {
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

/// The path of solved `elements` without its end point (see `solve`).
fn path_of(elements: &[AlignmentElement]) -> Vec<PathPoint> {
    let mut path = Vec::new();
    for piece in &pieces_with_curvature(elements) {
        push_piece(&mut path, piece, 1.0);
    }
    path
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
///
/// An offset alignment's companion (`offset`) numbers its nodes from 2n + 1
/// (n nodes), gives every segment flags 10 and no faces (`BL 16` in place of
/// `BL 48`, faces `BL 2, BL 0, BL 0`), and refers to the alignment and one
/// empty soft pointer (AO14, AO21, AO22).
///
/// An offset profile's companion has no path (`BL 1, BL 0, BL 0` for the
/// nodes, segment flags 48, faces `BL 2, BL 0, BL 0`) and refers to the
/// profile and one empty soft pointer (OP01, OB01).
fn write_companion(doc: &mut CadDocument, handle: Handle, owner: Handle, tag: u32, alignment: Handle, path: &[PathPoint], name: &str, offset: bool) -> Result<(), DxfError> {
    let n = path.len();
    let first = if n == 0 { 0 } else if offset { 2 * n as i32 + 1 } else { 1 };
    let node_of = |i: usize| -> i32 {
        if i == 0 {
            first
        } else if i + 1 == n {
            first + 1
        } else {
            first + 1 + i as i32
        }
    };
    let mut order: Vec<usize> = if n == 0 { Vec::new() } else { vec![0, n - 1] };
    order.extend(1..n.saturating_sub(1));
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
            w.write_bit_long(if n == 0 { 1 } else { first + n as i32 });
            w.write_bit_long(n as i32);
            w.write_bit_long(first);
            for (k, i) in order.iter().enumerate() {
                for v in [path[*i].0.x, path[*i].0.y, 0.0] {
                    w.write_bit_double(v);
                }
                bits(w, "010");
                w.write_bit_long(if k + 1 == n { 0 } else { first + k as i32 + 1 });
            }
            for v in [2, 1, 0, 1, if offset { 16 } else { 48 }, n as i32, 0] {
                w.write_bit_long(v);
            }
            for (i, (_, arc)) in path.iter().enumerate() {
                let last = i + 1 == n;
                let arc = arc.filter(|_| !last);
                let flags = if last || offset { 10 } else if i + 2 < n { 0x32 } else { 2 };
                let face = if last || offset { 0 } else { 1 };
                let kind = if last { 2 } else if arc.is_some() { 1 } else { 0 };
                for v in [kind, flags | if arc.is_some() && !offset { ARC_EDGE } else { 0 }, node_of(i), face, face] {
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
            let faces: &[i32] = if offset || n == 0 { &[2, 0, 0] } else { &[2, 1, 1, 1, 7, 1, 1, 0, 0] };
            for v in faces {
                w.write_bit_long(*v);
            }
            for _ in 0..6 {
                w.write_bit_long(1);
                w.write_bit_long(100);
            }
            for _ in 0..6 {
                w.write_bit_long(0);
            }
        },
        Body { refs: [alignment.value(), 0, 0, 0][..if offset || n == 0 { 2 } else { 4 }].iter().map(|h| soft_pointer(*h)).collect(), strings: vec![name.to_string(), String::new()] },
    )
}

/// Adds the companion feature style an offset profile refers to (no path)
/// to the node of the siteless companions and returns it.
pub(super) fn add_profile_companion(doc: &mut CadDocument, profile: Handle) -> Result<Handle, DxfError> {
    let node = companion_node(doc).ok_or_else(|| invalid("drawing has no node for siteless alignments"))?;
    let used = read_node(doc, node).ok_or_else(|| invalid("feature style node cannot be read"))?;
    let tag = (FIRST_COMPANION_TAG..).find(|t| used.children.iter().all(|c| c.0 as u32 != *t)).unwrap_or(FIRST_COMPANION_TAG);
    let companion = doc.allocate_handle();
    let label = new_name(companion.value());
    write_companion(doc, companion, node, tag, profile, &[], &label, false)?;
    add_node_child(doc, node, tag as i32, companion, &label)?;
    Ok(companion)
}

/// A region of an offset alignment: a stretch at one offset (the nominal
/// one or a widening's) between two of the parent's stations.
pub(super) struct OffsetRegionRecord {
    pub offset: f64,
    /// The parent's stations, and its own.
    pub parent: (f64, f64),
    pub own: (f64, f64),
    pub widening: bool,
    /// Its first and last elements.
    pub first: i32,
    pub last: i32,
}

/// What an offset alignment's record holds besides its elements: its
/// parent, its design speed's anchor and its offset block (see
/// [`write_offset_block`]).
pub(super) struct OffsetRecord {
    pub parent: u64,
    /// Nominal offset, negative to the left of the parent.
    pub offset: f64,
    /// Its own stations.
    pub range: (f64, f64),
    pub regions: Vec<OffsetRegionRecord>,
    /// The transitions (linear): length along the parent and element.
    pub transitions: Vec<(f64, i32)>,
    /// For each parent element (by id) the elements that follow it.
    pub map: Vec<(i32, Vec<i32>)>,
    /// The parent's anchors at the ends of the regions and transitions.
    pub anchors: Vec<(f64, [u8; 16])>,
    /// Its own anchor at its start, which its design speed refers to.
    pub own: [u8; 16],
    /// Its own design speed (0 for none).
    pub speed: f64,
    /// Kept by an associative action (an intersection's offset
    /// alignments): the record's word `0x10025F04` in place of
    /// `0x00025F04`.
    pub associative: bool,
}

/// Writes the alignment record. `feature` is the companion the alignment
/// refers to (twice).
///
/// ```text
/// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04 (an offset alignment
///   no action keeps: 0x00025F04), 6 bits 010101
/// RC 1, key 0x0B001081: BL 2, BL 0, BL 0 (the geometry stamp; the
///   application keeps a value derived from the geometry and accepts 0)
/// BL 0 (byte form), 115 zero bits, 8 bits 00000010, RC 0x40 | edge id >>
///   2, 2 bits edge id & 3, 5 bits 11010 (the edge id: the alignment's edge
///   in its companion, 1, or its site's container, its member number; read
///   before as BD 2.25 and 7 bits 0111010, edge 1)
/// BL first element id, BD 0, BD start station
/// 7 bits 0000011, BD 0 (track gauge), B 0, BL first element id
/// BL n, the elements in id order:
///   line: BL 257, BS 2 (0 in an offset alignment), B 0, BS 0 (fixed),
///         BS id, BS previous id, BS next id, 2RD start, 2RD end
///   arc:  BL 258, BS 2, B 0, B 0, BD -1 (no design speed), BD -1, BS 1,
///         BD -1, BL 29 (fitted between two lines), B 0, BS id,
///         BS previous id, BS next id, BD radius
///   fixed arc (offset alignments): BL 258, BS 0, B 0, B 0, BD -1, BD -1,
///         BS 1, BD -1, BL 2, B 0, BS id, BS previous id, BS next id,
///         2RD start, 2RD middle, 2RD end
///   spiral-curve-spiral: BL 260, the arc's head with BL 0 (fitted
///         between two lines) and no B, BS id, BS previous id, BS next id,
///         BL 256 (spiral definition), BD spiral in, BD radius, BD spiral
///         out
///   widening transition (offset alignments): BL 267, the fixed arc's head
///         with BL 3 and no B, BS id, BS previous id, BS next id, BL 1
///         (point sets), BL 4096, BL n, n x 2RD
/// BL profiles, BL profile views, BL sample line groups, BL k, the k
///   station anchors in identifier order (see `AlignmentAnchor`)
/// BL e, e x { 2 bits 00, RS 0x04BE, BD raw station, the identifier of
///   its anchor, BD station back, BD station ahead, B increasing }
///   (station equations, one anchor of kind 3 each)
/// BL 1, BL s, s x { BL 1, BD station, BD speed, the identifier of the
///   anchor at the station } (design speeds, one anchor of kind 2 each)
/// BD 100, BL 0 (no superelevation), BL 0,
///   centerline: BL 1 (4 utility), BL 2, 2, 2, 0, 3
///   rail:       BL 5, BL 2, 2, 2, 0, 0, 3 (its track gauge in the BD after
///               the criteria bits)
///   offset:     BL 2, the offset block, BL 0, BL 0, BL 3
/// B 0, BS 0x04BE
/// references: soft pointer 0, companion, companion, soft pointer 0,
///   style, hard pointer 0, the parent (an offset alignment, soft
///   pointer), the profiles (soft pointers, oldest first), the profile
///   views, the sample line groups (soft pointers, oldest first)
/// texts: "AeccImpAlignment", "", "", name, five empty texts, three more
///   per curve element, two more per anchor, one more per design speed,
///   two more per station equation
/// ```
///
/// Written for WA03's and WA04's polylines and curves, the record equals
/// the samples' bit for bit apart from the geometry stamp; written for
/// AO14's and AO21's offset alignments it equals theirs apart from the
/// stamp and the identifiers.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_alignment(doc: &mut CadDocument, common: EntityCommon, name: &str, elements: &[AlignmentElement], style: u64, feature: u64, edge: i32, keys: Option<&str>, profiles: &[u64], anchors: &[AlignmentAnchor], views: &[u64], groups: &[u64], offset: Option<&OffsetRecord>) -> Result<Handle, DxfError> {
    write_alignment_of(doc, common, name, elements, style, feature, edge, keys, profiles, anchors, views, groups, offset, AlignmentKind::Centerline, &Timing::default())
}

/// A station equation as written: raw station, its anchor's identifier,
/// station back, station ahead, increasing.
pub(super) type Equation = (f64, [u8; 16], f64, f64, bool);

/// Adds a curb return (a connected alignment of type 3) `name` from
/// `incoming` to `outgoing`, two siteless alignments this crate writes:
/// an arc of `radius` tangent to the line `offset_in` from the incoming
/// alignment and the line `offset_out` from the outgoing one (negative to
/// the left of each), with a straight overlap of `overlap_in` before it
/// and `overlap_out` after it along those lines. It runs along the
/// incoming alignment's stationing into the outgoing one along its
/// stationing (`forward`) or against it, turning left or right as the
/// lines meet; the lines must meet on line elements of both alignments
/// with the overlaps on them. The application keeps it (follows its
/// roads); the roads get no anchors.
///
/// The record is an alignment of a line, a free arc fitted between the
/// lines and a line, followed by the connection block: for each side the
/// signed offset toward the turn's inside, the overlap and a region list
/// like an offset alignment's (the side's line over its stations on the
/// road, kept by the overlap less a thousandth of it at each end, two
/// empty regions, the line again); the arc's start and end stations and
/// radius; the point half a unit from where the roads meet into the
/// corner between the arms it joins; and the stations on each road where
/// the two lines meet (CR01 to CR05, CR07 to CR10).
#[allow(clippy::too_many_arguments)]
pub fn add_curb_return(doc: &mut CadDocument, name: &str, incoming: Handle, offset_in: f64, outgoing: Handle, offset_out: f64, forward: bool, radius: f64, overlaps: (f64, f64), layer: Option<&str>) -> Result<Handle, DxfError> {
    add_curb_return_with(doc, name, incoming, offset_in, outgoing, offset_out, forward, CurbReturnCurve::Arc(radius), overlaps, layer)
}

/// The curve group of a curb return between its two overlaps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CurbReturnCurve {
    /// One arc of this radius.
    Arc(f64),
    /// Three arcs turning the same way: the first and last by radius and
    /// length, the middle one by radius (its length takes the rest of the
    /// turn).
    ThreeCentered { first: (f64, f64), middle: f64, last: (f64, f64) },
    /// A spiral of `spiral_in` into an arc of `radius`, then a spiral of
    /// `spiral_out` out of it, both of `shape`.
    SpiralCurveSpiral { spiral_in: f64, radius: f64, spiral_out: f64, shape: crate::classes::civil::SpiralShape },
}

/// A curb return's curve from its start at the origin heading along +x and
/// turning left by `sweep`: the points where its parts meet (start first,
/// end last), each part's turn, and its length; `None` when the parts do
/// not fit the turn.
fn curb_curve(curve: CurbReturnCurve, sweep: f64) -> Option<(Vec<Vector2>, Vec<f64>, f64)> {
    // An arc of radius r turning by t from p heading h.
    let arc = |p: Vector2, h: f64, r: f64, t: f64| p + Vector2::new((h + t).sin() - h.sin(), h.cos() - (h + t).cos()) * r;
    let origin = Vector2::new(0.0, 0.0);
    match curve {
        CurbReturnCurve::Arc(r) => Some((vec![origin, arc(origin, 0.0, r, sweep)], vec![sweep], r * sweep)),
        CurbReturnCurve::ThreeCentered { first, middle, last } => {
            let turns = [first.1 / first.0, sweep - first.1 / first.0 - last.1 / last.0, last.1 / last.0];
            (turns[1] > 1e-9).then_some(())?;
            let mut points = vec![origin];
            let mut h = 0.0;
            for (r, t) in [first.0, middle, last.0].into_iter().zip(turns) {
                points.push(arc(*points.last()?, h, r, t));
                h += t;
            }
            Some((points, turns.to_vec(), first.1 + middle * turns[1] + last.1))
        }
        CurbReturnCurve::SpiralCurveSpiral { spiral_in, radius, spiral_out, shape } => {
            let ((p1, t1), (q, t3)) = (shape.end(spiral_in, radius), shape.end(spiral_out, radius));
            let t2 = sweep - t1 - t3;
            (t2 > 1e-9).then_some(())?;
            let p2 = arc(p1, t1, radius, t2);
            // The spiral out seen backwards from its end is the same spiral.
            let (u, n) = (Vector2::new(sweep.cos(), sweep.sin()), Vector2::new(-sweep.sin(), sweep.cos()));
            Some((vec![origin, p1, p2, p2 + u * q.x - n * q.y], vec![t1, t2, t3], spiral_in + radius * t2 + spiral_out))
        }
    }
}

/// [`add_curb_return`] with any curve group: an arc, three arcs (CG01,
/// CG07) or a spiral-curve-spiral of any spiral shape (CG02, CG06, CG08; CG09,
/// CS01, CS02). The three arcs are
/// one element; the spiral-curve-spiral is a spiral in, a fixed arc and a
/// spiral out between the lines (ids 4 to 6, the lines keeping 1 and 3)
/// and keeps its parameters in a text of the record's second key.
#[allow(clippy::too_many_arguments)]
pub fn add_curb_return_with(doc: &mut CadDocument, name: &str, incoming: Handle, offset_in: f64, outgoing: Handle, offset_out: f64, forward: bool, curve: CurbReturnCurve, overlaps: (f64, f64), layer: Option<&str>) -> Result<Handle, DxfError> {
    let positive = |v: f64| v.is_finite() && v > 0.0;
    let curve_ok = match curve {
        CurbReturnCurve::Arc(r) => positive(r),
        CurbReturnCurve::ThreeCentered { first, middle, last } => [first.0, first.1, middle, last.0, last.1].into_iter().all(positive),
        CurbReturnCurve::SpiralCurveSpiral { spiral_in, radius, spiral_out, shape } => [spiral_in, radius, spiral_out].into_iter().all(positive) && shape.length_ok(spiral_in, radius) && shape.length_ok(spiral_out, radius),
    };
    if name.is_empty() || !curve_ok || !positive(overlaps.0) || !positive(overlaps.1) || !offset_in.is_finite() || !offset_out.is_finite() || incoming == outgoing {
        return Err(invalid("a curb return needs a name, two roads, finite offsets, a positive curve whose spirals have their shape's geometry and positive overlaps"));
    }
    let lines = |h| super::intersection::lines(doc, h).ok_or_else(|| invalid("a road is not an alignment this crate solves"));
    let (la, lb) = (lines(incoming)?, lines(outgoing)?);
    let unit = |v: Vector2| v * (1.0 / v.length());
    let left = |u: Vector2| Vector2::new(-u.y, u.x);
    let cross = |a: Vector2, b: Vector2| a.x * b.y - a.y * b.x;
    // The point where two lines (point, direction) meet, as the distance
    // along each.
    let meet = |p: Vector2, d: Vector2, q: Vector2, e: Vector2| -> Option<(f64, f64)> {
        let den = cross(d, e);
        (den.abs() > 1e-12).then(|| (cross(q - p, e) / den, cross(q - p, d) / den))
    };
    let mut found = None;
    for &(sa, pa, qa) in &la {
        for &(sb, pb, qb) in &lb {
            let (d1, d2) = (unit(qa - pa), unit(qb - pb));
            let travel = if forward { d2 } else { d2 * -1.0 };
            let (o1, o2) = (pa + left(d1) * -offset_in, pb + left(d2) * -offset_out);
            let Some((t1, t2)) = meet(o1, d1, o2, d2) else { continue };
            let x = o1 + d1 * t1;
            let turn = cross(d1, travel);
            let sweep = travel.dot(&d1).clamp(-1.0, 1.0).acos();
            let Some(shape) = curb_curve(curve, sweep) else { continue };
            // How far before and after the lines' meeting point the curve
            // starts and ends.
            let e = shape.0[shape.0.len() - 1];
            let (tangent, tangent_out) = (e.x - e.y * sweep.cos() / sweep.sin(), e.y / sweep.sin());
            let (len_a, len_b) = (pa.distance(&qa), pb.distance(&qb));
            // The overlaps lie on the line elements.
            let along_in = t1 - tangent - overlaps.0;
            let along_out = t2 + if forward { tangent_out + overlaps.1 } else { -(tangent_out + overlaps.1) };
            let inside = |t: f64, len: f64| t >= -1e-9 && t <= len + 1e-9;
            if sa.is_nan() || sb.is_nan() || turn.abs() < 1e-12 || !inside(along_in, len_a) || !inside(t1 - tangent, len_a) || !inside(t1, len_a) || !inside(along_out, len_b) || !inside(t2 + if forward { tangent_out } else { -tangent_out }, len_b) {
                continue;
            }
            found = Some((sa, pa, d1, sb, pb, d2, travel, x, t1, turn, sweep, tangent, tangent_out, shape));
            break;
        }
        if found.is_some() {
            break;
        }
    }
    let (sa, pa, d1, sb, pb, d2, travel, x, t1, turn, sweep, tangent, tangent_out, (joins, turns, curve_length)) = found.ok_or_else(|| invalid("the offset lines do not meet on line elements of both roads with room for the curve and the overlaps"))?;
    let right = turn < 0.0;
    let turn_sign = if right { -1.0 } else { 1.0 };
    let dir = if forward { 1.0 } else { -1.0 };
    let t_in = x - d1 * tangent;
    let t_out = x + travel * tangent_out;
    let start = t_in - d1 * overlaps.0;
    let end = t_out + travel * overlaps.1;
    let arc_end = overlaps.0 + curve_length;
    // A point of the curve's own frame (a left turn along +x) in the drawing.
    let world = |p: Vector2| t_in + d1 * p.x + left(d1) * (p.y * turn_sign);
    let total = arc_end + overlaps.1;
    // The roads' stations: the incoming line's from its start to the
    // tangent point, the outgoing one's in station order.
    let (in_end, in_start) = (sa + t1 - tangent, sa + t1 - tangent - overlaps.0);
    let s_out = |p: Vector2| sb + (p - pb).dot(&d2);
    let (out_a, out_b) = (s_out(t_out), s_out(end));
    let (out_start, out_end) = (out_a.min(out_b), out_a.max(out_b));
    let eps_in = (in_start + overlaps.0 / 1000.0) - in_start;
    let eps_out = (out_start + overlaps.1 / 1000.0) - out_start;
    let region_in = [offset_in, in_start, in_end, eps_in, overlaps.0 - eps_in];
    let region_out = if forward { [offset_out, out_start, out_end, arc_end + eps_out, total - eps_out] } else { [offset_out, out_start, out_end, total - eps_out, arc_end + eps_out] };
    // Where the roads themselves meet, and the point half a unit into the
    // corner between the incoming road's arm behind and the outgoing arm.
    let (u1, _) = meet(pa, d1, pb, d2).ok_or_else(|| invalid("the roads are parallel"))?;
    let roads_meet = pa + d1 * u1;
    let pick = roads_meet + d1 * -0.5 + travel * 0.5;
    let mut values = [0.0; 31];
    values[0] = -offset_in * turn_sign;
    values[1] = overlaps.0;
    values[2..7].copy_from_slice(&region_in);
    values[7..12].copy_from_slice(&region_in);
    values[12] = -offset_out * turn_sign * dir;
    values[13] = overlaps.1;
    values[14..19].copy_from_slice(&region_out);
    values[19..24].copy_from_slice(&region_out);
    values[24] = overlaps.0;
    values[25] = arc_end;
    values[26] = match curve {
        CurbReturnCurve::Arc(r) => r,
        CurbReturnCurve::ThreeCentered { first, .. } => first.0,
        CurbReturnCurve::SpiralCurveSpiral { radius, .. } => radius,
    };
    values[27] = pick.x;
    values[28] = pick.y;
    values[29] = sa + t1;
    values[30] = s_out(x);
    // A value of 0 or 1 would change the block's length.
    if values.iter().any(|v| *v == 0.0 || *v == 1.0) {
        return Err(invalid("a curb return value of 0 or 1 is not written"));
    }
    let line = |id, previous, next, start, end| AlignmentElement { id, previous, next, kind: AlignmentElementKind::FixedLine { start, end } };
    let bulge = |t: f64| Some(turn_sign * (t / 4.0).tan());
    let (elements, path, spiral_out, text): (Vec<AlignmentElement>, Vec<PathPoint>, _, _) = match curve {
        CurbReturnCurve::Arc(radius) => (
            vec![line(1, 0, 2, start, t_in), AlignmentElement { id: 2, previous: 1, next: 3, kind: AlignmentElementKind::FreeArc { radius } }, line(3, 2, 0, t_out, end)],
            vec![(start, None), (t_in, bulge(sweep)), (t_out, None), (end, None)],
            None,
            None,
        ),
        CurbReturnCurve::ThreeCentered { first, middle, last } => {
            let mut path = vec![(start, None)];
            path.extend(joins[..3].iter().zip(&turns).map(|(p, t)| (world(*p), bulge(*t))));
            path.extend([(t_out, None), (end, None)]);
            let arcs = [first, (middle, 0.0), last];
            (vec![line(1, 0, 2, start, t_in), AlignmentElement { id: 2, previous: 1, next: 3, kind: AlignmentElementKind::ThreeCenteredArc { arcs } }, line(3, 2, 0, t_out, end)], path, None, None)
        }
        CurbReturnCurve::SpiralCurveSpiral { spiral_in, radius, spiral_out, shape } => {
            let (sc, cs) = (world(joins[1]), world(joins[2]));
            // The arc's middle, and the direction where the spiral out starts.
            let h = turns[0] + turns[1] / 2.0;
            let mid = world(joins[1] + Vector2::new(h.sin() - turns[0].sin(), turns[0].cos() - h.cos()) * radius);
            let a = turn_sign * (turns[0] + turns[1]);
            let out_dir = Vector2::new(d1.x * a.cos() - d1.y * a.sin(), d1.x * a.sin() + d1.y * a.cos());
            let elements = vec![
                line(1, 0, 4, start, t_in),
                line(3, 6, 0, t_out, end),
                AlignmentElement { id: 4, previous: 1, next: 5, kind: AlignmentElementKind::SpiralIn { start: t_in, direction: d1, length: spiral_in, radius, clockwise: right, shape } },
                AlignmentElement { id: 5, previous: 4, next: 6, kind: AlignmentElementKind::FixedArc { start: sc, mid, end: cs } },
                AlignmentElement { id: 6, previous: 5, next: 3, kind: AlignmentElementKind::SpiralOut { radius, length: spiral_out, clockwise: right, shape } },
            ];
            let mut path = path_of(&elements);
            path.push((end, None));
            let text = format!("{{\r\n\t\"CurveType\": 4,\r\n\t\"CurveParameters\": {{\r\n\t\t\"CurveRadius\": {radius},\r\n\t\t\"SpiralInLength\": {spiral_in},\r\n\t\t\"SpiralOutLength\": {spiral_out},\r\n\t\t\"Greater180\": false,\r\n\t\t\"SpiralType\": {}\r\n\t}}\r\n}}", shape.code());
            (elements, path, Some((cs, out_dir)), Some(text))
        }
    };
    let connected = Connected { values, right, forward, outside_in: values[0] < 0.0, roads: [incoming.value(), outgoing.value()], curve, spiral_out, text, intersection: None };
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    if default_style(doc).is_none() || companion_node(doc).is_none() {
        ensure_alignment_tree(doc)?;
    }
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no alignment styles"))?;
    let node = companion_node(doc).ok_or_else(|| invalid("drawing has no node for siteless alignments"))?;
    let used = read_node(doc, node).ok_or_else(|| invalid("feature style node cannot be read"))?;
    let tag = (FIRST_COMPANION_TAG..).find(|t| used.children.iter().all(|c| c.0 as u32 != *t)).unwrap_or(FIRST_COMPANION_TAG);
    let common = EntityCommon { handle: doc.allocate_handle(), owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let companion = doc.allocate_handle();
    let handle = common.handle;
    let label = new_name(handle.value());
    write_companion(doc, companion, node, tag, handle, &path, &label, false)?;
    add_node_child(doc, node, tag as i32, companion, &label)?;
    let timing = Timing { connected: Some(connected), ..Timing::default() };
    write_alignment_of(doc, common, name, &elements, style, companion.value(), 1, None, &[], &[], &[], &[], None, AlignmentKind::Centerline, &timing)
}

/// An alignment's station equations and design speeds (station, speed,
/// anchor identifier), in station order.
#[derive(Clone, Debug, Default)]
pub(super) struct Timing {
    pub equations: Vec<Equation>,
    pub speeds: Vec<(f64, f64, [u8; 16])>,
    /// Its superelevation curves (none: no superelevation block).
    pub superelevation: Vec<SuperelevationCurveData>,
    /// A curb return's connection block (type 3).
    pub connected: Option<Connected>,
}

/// A curb return's connection block (see [`add_curb_return`]): its 31
/// values and flags, and the incoming and outgoing alignments.
#[derive(Clone, Debug)]
pub(super) struct Connected {
    pub values: [f64; 31],
    /// It turns right (clockwise).
    pub right: bool,
    /// It runs along the outgoing alignment's stationing.
    pub forward: bool,
    /// Its first value is negative: the incoming offset lies outside the
    /// turn.
    pub outside_in: bool,
    pub roads: [u64; 2],
    /// Its curve group: three arcs store their six values in place of the
    /// radius, a spiral-curve-spiral stores none (see `write_connected`).
    pub curve: CurbReturnCurve,
    /// Where its spiral out starts and the direction there.
    pub spiral_out: Option<(Vector2, Vector2)>,
    /// The text of the record's second key (a spiral-curve-spiral's
    /// parameters).
    pub text: Option<String>,
    /// A curb return an intersection's command makes (see
    /// [`IntersectionCurbReturn`]).
    pub intersection: Option<IntersectionCurbReturn>,
}

/// What a curb return of an intersection's region keeps besides a curb
/// return's block: the region's index (1 up, in the intersection's region
/// order), the corner where the two offset lines meet, whether it runs
/// against the incoming road's stationing, the offset alignments it joins
/// (incoming, outgoing) and the intersection.
#[derive(Clone, Copy, Debug)]
pub(super) struct IntersectionCurbReturn {
    pub region: i32,
    pub corner: Vector2,
    pub in_backward: bool,
    pub offsets: [u64; 2],
    pub intersection: u64,
}

/// The constant runs around the connection block's 31 values (the
/// samples' bits; CR01 to CR10 equal apart from the values and three
/// flags). The last run ends with `BL 0, BL 3`.
const CONNECTED_RUNS: [(usize, &str); 32] = [
    (51, "4050140680a02"),
    (1, "8"),
    (6, "50"),
    (0, ""),
    (0, ""),
    (0, ""),
    (0, ""),
    (138, "40501406aa40eaa406aaa40eaa406aaa40c"),
    (0, ""),
    (0, ""),
    (0, ""),
    (0, ""),
    (73, "40501406aa40ca02810"),
    (1, "8"),
    (6, "50"),
    (0, ""),
    (0, ""),
    (0, ""),
    (0, ""),
    (138, "4050340eaa40eaa406aaa40eaa406aaa40c"),
    (0, ""),
    (0, ""),
    (0, ""),
    (0, ""),
    (79, "4050340eaa40ea048154"),
    (0, ""),
    (34, "406aaa40c"),
    (1, "0"),
    (0, ""),
    (12, "902"),
    (0, ""),
    (12, "903"),
];

/// The flags of the connection block: (run, bit) of the right turn, of
/// the forward run along the outgoing alignment, of the incoming offset
/// outside the turn and of the backward run (CR07, CR09).
const CONNECTED_RIGHT: (usize, usize) = (12, 51);
const CONNECTED_FORWARD: (usize, usize) = (24, 50);
const CONNECTED_OUTSIDE_IN: (usize, usize) = (24, 51);
const CONNECTED_BACKWARD: (usize, usize) = (29, 11);

/// Writes the connection block: the runs with the values between them,
/// each a `BD`, and the flags set in their runs.
///
/// The curve group's code is the BL at bits 53 to 62 of run 24 (2 an arc,
/// 3 three arcs, 4 a spiral-curve-spiral). Three arcs write the radius and
/// length of each arc (the middle length 0) in place of the radius and
/// `BL 1` before the bit after them (CG01, CG07); a spiral-curve-spiral
/// ends run 24 after its code and writes neither the overlap, the curve's
/// end station nor the radius (CG02, CG06, CG08).
fn write_connected(w: &mut DwgMergedWriter, c: &Connected) {
    if let Some(x) = &c.intersection {
        return write_intersection_connected(w, c, x);
    }
    let flags = [(CONNECTED_RIGHT, c.right), (CONNECTED_FORWARD, c.forward), (CONNECTED_OUTSIDE_IN, c.outside_in), (CONNECTED_BACKWARD, !c.forward)];
    let code = format!("01{:08b}", match c.curve {
        CurbReturnCurve::Arc(_) => 2,
        CurbReturnCurve::ThreeCentered { .. } => 3,
        CurbReturnCurve::SpiralCurveSpiral { .. } => 4,
    });
    let spiral = matches!(c.curve, CurbReturnCurve::SpiralCurveSpiral { .. });
    for (i, (len, hex)) in CONNECTED_RUNS.iter().enumerate() {
        if spiral && (25..=26).contains(&i) {
            continue;
        }
        if i == 27 {
            if let CurbReturnCurve::ThreeCentered { .. } = c.curve {
                w.write_bit_long(1);
            }
        }
        let len = if spiral && i == 24 { 63 } else { *len };
        for bit in 0..len {
            let digit = hex.as_bytes()[bit / 4] as char;
            let mut v = digit.to_digit(16).unwrap_or(0) >> (3 - bit % 4) & 1 == 1;
            for ((run, at), on) in flags {
                if run == i && at == bit {
                    v = on;
                }
            }
            if i == 24 && (53..63).contains(&bit) {
                v = code.as_bytes()[bit - 53] == b'1';
            }
            w.write_bit(v);
        }
        match (c.values.get(i), c.curve) {
            (Some(_), CurbReturnCurve::SpiralCurveSpiral { .. }) if (24..=26).contains(&i) => {}
            (Some(_), CurbReturnCurve::ThreeCentered { first, middle, last }) if i == 26 => {
                for v in [first.0, first.1, middle, 0.0, last.0, last.1] {
                    w.write_bit_double(v);
                }
            }
            (Some(v), _) => w.write_bit_double(*v),
            (None, _) => {}
        }
    }
}

/// The connection block of an intersection's curb return (WC40: the four
/// regions of a crossing): the runs and values of a curb return's block up
/// to the radius, with these bits set apart from the samples' runs: run 0
/// bits 39 and 40 `10`, runs 1 and 13 `0`, run 12 bit 50 the incoming run
/// against its road, bit 51 the right turn unless it is, bits 61 and 62
/// `10`, run 24 bits 50 and 51 the outgoing run along its road; then
/// `B 1, BL region, 2 x BD corner` and the last run (`BL 0, BL 3`) in place
/// of the roads' meeting point and stations.
fn write_intersection_connected(w: &mut DwgMergedWriter, c: &Connected, x: &IntersectionCurbReturn) {
    let set: [((usize, usize), bool); 11] = [
        ((0, 39), true),
        ((0, 40), false),
        ((1, 0), false),
        ((13, 0), false),
        ((12, 50), x.in_backward),
        ((12, 51), c.right != x.in_backward),
        ((12, 61), true),
        ((12, 62), false),
        ((24, 50), c.forward),
        ((24, 51), c.forward),
        ((27, 0), true),
    ];
    for (i, (len, hex)) in CONNECTED_RUNS.iter().enumerate().take(28) {
        for bit in 0..*len {
            let digit = hex.as_bytes()[bit / 4] as char;
            let mut v = digit.to_digit(16).unwrap_or(0) >> (3 - bit % 4) & 1 == 1;
            for ((run, at), on) in set {
                if run == i && at == bit {
                    v = on;
                }
            }
            if i == 24 && (53..63).contains(&bit) {
                v = format!("01{:08b}", 2).as_bytes()[bit - 53] == b'1';
            }
            w.write_bit(v);
        }
        if i < 27 {
            w.write_bit_double(c.values[i]);
        }
    }
    w.write_bit_long(x.region);
    w.write_bit_double(x.corner.x);
    w.write_bit_double(x.corner.y);
    let (len, hex) = CONNECTED_RUNS[31];
    for bit in 0..len {
        let digit = hex.as_bytes()[bit / 4] as char;
        w.write_bit(digit.to_digit(16).unwrap_or(0) >> (3 - bit % 4) & 1 == 1);
    }
}

/// Adds the curb return of an intersection's region `region` (1 up):
/// along the incoming road's offset line `offsets[0]` (run against the
/// road's stationing when `in_backward`) into the outgoing road's offset
/// line `offsets[1]` (run along its stationing when `forward`), a line of
/// `lines[0]` units before the free arc of `radius` and `lines[1]` after
/// it; with `profile` (handle and identifier) it lists that curb return
/// profile. `corner` is
/// where the offset lines meet, `arms` the in and out arms' directions
/// (away from the crossing), `stations` maps a point to its station on
/// each road. Offsets and overlaps in the block are 0 (the lines are the
/// offset alignments themselves); its region lists name the roads'
/// stations.
#[allow(clippy::too_many_arguments)]
pub(super) fn add_intersection_curb_return(
    doc: &mut CadDocument,
    handle: Handle,
    name: &str,
    roads: [Handle; 2],
    offsets: [Handle; 2],
    intersection: Handle,
    region: i32,
    corner: Vector2,
    arms: [Vector2; 2],
    stations: &dyn Fn(usize, Vector2) -> f64,
    in_backward: bool,
    forward: bool,
    radius: f64,
    lines: [f64; 2],
    profile: Option<(Handle, [u8; 16])>,
    style: Option<u64>,
    layer: Option<&str>,
) -> Result<Handle, DxfError> {
    let unit = |v: Vector2| v * (1.0 / v.length());
    let (a_in, a_out) = (unit(arms[0]), unit(arms[1]));
    let (d1, travel) = (a_in * -1.0, a_out);
    let sweep = travel.dot(&d1).clamp(-1.0, 1.0).acos();
    let turn = d1.cross(&travel);
    if !(radius.is_finite() && radius > 0.0) || turn.abs() < 1e-12 {
        return Err(invalid("a curb return needs a positive radius between roads that are not parallel"));
    }
    let right = turn < 0.0;
    let tangent = radius * (sweep / 2.0).tan();
    let (t_in, t_out) = (corner + a_in * tangent, corner + a_out * tangent);
    let (start, end) = (corner + a_in * (tangent + lines[0]), corner + a_out * (tangent + lines[1]));
    let arc_end = lines[0] + radius * sweep;
    let total = arc_end + lines[1];
    // A side's region: the roads' stations of its line, and the own
    // stations there kept a thousandth of the line in from its ends.
    let side = |road: usize, (p, own_p): (Vector2, f64), (q, own_q): (Vector2, f64)| {
        let (sp, sq) = (stations(road, p), stations(road, q));
        if sp <= sq { [0.0, sp, sq, own_p, own_q] } else { [0.0, sq, sp, own_q, own_p] }
    };
    let region_in = side(0, (start, 0.001), (t_in, lines[0] - 0.001));
    let region_out = side(1, (t_out, arc_end + 0.001), (end, total - 0.001));
    let mut values = [0.0; 31];
    values[2..7].copy_from_slice(&region_in);
    values[7..12].copy_from_slice(&region_in);
    values[14..19].copy_from_slice(&region_out);
    values[19..24].copy_from_slice(&region_out);
    values[24] = 1.0;
    values[25] = arc_end;
    values[26] = radius;
    let turn_sign = if right { -1.0 } else { 1.0 };
    let line = |id, previous, next, start, end| AlignmentElement { id, previous, next, kind: AlignmentElementKind::FixedLine { start, end } };
    let elements = vec![line(1, 0, 2, start, t_in), AlignmentElement { id: 2, previous: 1, next: 3, kind: AlignmentElementKind::FreeArc { radius } }, line(3, 2, 0, t_out, end)];
    let path = vec![(start, None), (t_in, Some(turn_sign * (sweep / 4.0).tan())), (t_out, None), (end, None)];
    let connected = Connected {
        values,
        right,
        forward,
        outside_in: false,
        roads: [roads[0].value(), roads[1].value()],
        curve: CurbReturnCurve::Arc(radius),
        spiral_out: None,
        text: None,
        intersection: Some(IntersectionCurbReturn { region, corner, in_backward, offsets: [offsets[0].value(), offsets[1].value()], intersection: intersection.value() }),
    };
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    if default_style(doc).is_none() || companion_node(doc).is_none() {
        ensure_alignment_tree(doc)?;
    }
    let style = match style {
        Some(s) => s,
        None => default_style(doc).ok_or_else(|| invalid("drawing has no alignment styles"))?,
    };
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let timing = Timing { connected: Some(connected), ..Timing::default() };
    // Its curb return profile, tied to its start by its identifier.
    let (profiles, anchors) = match profile {
        Some((h, id)) => (vec![h.value()], vec![anchor_at(&elements, 0.0, PROFILE_ANCHOR, id, 0.0).ok_or_else(|| invalid("the curb return's start cannot be placed"))?]),
        None => (Vec::new(), Vec::new()),
    };
    // On the roads' site it joins the site (WC64).
    if let Some(site) = on_site(doc, roads[0]) {
        let elevations = vec![0.0; path.len()];
        let (h, _) = super::site::join_site_as(doc, site, common, path, &elevations, super::site::MemberKind::Alignment, true, |doc, common, edge, container| {
            write_alignment_of(doc, common, name, &elements, style, container.value(), edge, None, &profiles, &anchors, &[], &[], None, AlignmentKind::Centerline, &timing)
        })?;
        return Ok(h);
    }
    let node = companion_node(doc).ok_or_else(|| invalid("drawing has no node for siteless alignments"))?;
    let used = read_node(doc, node).ok_or_else(|| invalid("feature style node cannot be read"))?;
    let tag = (FIRST_COMPANION_TAG..).find(|t| used.children.iter().all(|c| c.0 as u32 != *t)).unwrap_or(FIRST_COMPANION_TAG);
    let companion = doc.allocate_handle();
    let label = new_name(handle.value());
    write_companion(doc, companion, node, tag, handle, &path, &label, false)?;
    add_node_child(doc, node, tag as i32, companion, &label)?;
    write_alignment_of(doc, common, name, &elements, style, companion.value(), 1, None, &profiles, &anchors, &[], &[], None, AlignmentKind::Centerline, &timing)
}

/// The site `alignment` lies on (the owner of its container), `None` for
/// a siteless one.
fn on_site(doc: &CadDocument, alignment: Handle) -> Option<Handle> {
    super::site::site_container_of(doc, alignment).and_then(|c| object(doc, c).map(|o| o.owner))
}

/// The class key of the superelevation type (2 superelevation).
const KEY_SUPERELEVATION: i32 = 0x0D00_D773;

/// The bits of `v` written as a BL.
fn bl_bits(v: i32) -> String {
    match v {
        0 => "10".into(),
        1..=255 => format!("01{v:08b}"),
        _ => std::iter::once("00".to_string()).chain(v.to_le_bytes().iter().map(|b| format!("{b:08b}"))).collect(),
    }
}

/// What a superelevation critical station marks (the kinds a critical
/// station can be added as at a station of its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CriticalStationKind {
    BeginFullSuper,
    EndFullSuper,
    LevelCrown,
    LowShoulderMatch,
    ReverseCrown,
    /// A manual station: its curve gets the application's settings block
    /// and each of its units the description "Manual station" (numbered
    /// "Manual station(2)" on within its curve).
    Manual,
}

impl CriticalStationKind {
    fn code(self) -> i32 {
        match self {
            CriticalStationKind::BeginFullSuper => 3,
            CriticalStationKind::EndFullSuper => 4,
            CriticalStationKind::LevelCrown => 7,
            CriticalStationKind::LowShoulderMatch => 8,
            CriticalStationKind::ReverseCrown => 9,
            CriticalStationKind::Manual => MANUAL_STATION,
        }
    }
}

/// A critical station of a superelevation curve: its kind, its station,
/// whether it lies in the curve's ending attainment region (else the
/// beginning one) and the cross slopes (ratios) of the left outside lane,
/// left inside lane, right outside lane, right inside lane, then the same
/// four shoulders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CriticalStation {
    pub kind: CriticalStationKind,
    pub station: f64,
    pub ending: bool,
    pub slopes: [f64; 8],
}

/// A superelevation curve over the curve element `element` (its id) with
/// its critical stations in station order.
#[derive(Clone, Debug, PartialEq)]
pub struct SuperelevationCurve {
    pub element: i16,
    pub stations: Vec<CriticalStation>,
}

/// The superelevation block after `BL 1` (SE02 to SE16, SB01, SB02; the
/// reader's `plain_superelevation`):
///
/// ```text
/// B 0, BL n curves, B 0,
/// n x { node, BL 0, B 0, [last: BL n], B 0 }
/// n x { node, B 0, node, B 0, B settings (with a manual station: then
///       the 582-bit settings block), 10 bits 0001010000,
///       BL 0, BL k, k x station, BL 0, [not last: B 0] }
///   node: BL element, BL -1, BL element, BL -1
///   station: BL 3, fields, node, BL 8 (no slope), then per cross segment
///            BL side, BL inside, BL lane, BL 3, fields, node, BL 0, BD
///            slope
///   fields: BL kind, BD station from the curve's start (from its end in
///           the ending region), BD 0 (1 ending), B 0
/// lane table: 4 x { BL 2, two segments }, per segment { BL 8 (the first)
///   or BL 0 + BD 0, BL side, BL inside, BL lane, BL 1, tail, BL 0, BD 0,
///   BL 2, tail }, BL 0, BL 0, BL 8, the segments in name order
///   tail: BL 0, BL 0, BD 0.5, BD 1, B 0, BL -1, B 1, B 0, BL -1
/// ```
fn write_superelevation(w: &mut DwgMergedWriter, curves: &[SuperelevationCurveData]) {
    let node = |w: &mut DwgMergedWriter, e: i16| {
        for v in [i32::from(e), -1, i32::from(e), -1] {
            w.write_bit_long(v);
        }
    };
    let n = curves.len() as i32;
    w.write_bit(false);
    w.write_bit_long(n);
    w.write_bit(false);
    for (i, c) in curves.iter().enumerate() {
        node(w, c.element);
        w.write_bit_long(0);
        w.write_bit(false);
        if i + 1 == curves.len() {
            w.write_bit_long(n);
        }
        w.write_bit(false);
    }
    for (i, c) in curves.iter().enumerate() {
        node(w, c.element);
        w.write_bit(false);
        node(w, c.element);
        w.write_bit(false);
        let manual = c.stations.iter().any(|s| s.kind == MANUAL_STATION);
        w.write_bit(manual);
        if manual {
            super::templates::run(w, MANUAL_SETTINGS.0, MANUAL_SETTINGS.1);
        }
        bits(w, "0001010000");
        w.write_bit_long(0);
        w.write_bit_long(c.stations.len() as i32);
        for s in &c.stations {
            let fields = |w: &mut DwgMergedWriter| {
                w.write_bit_long(s.kind);
                w.write_bit_double(s.offset);
                w.write_bit_double(if s.ending { 1.0 } else { 0.0 });
                w.write_bit(false);
            };
            w.write_bit_long(3);
            fields(w);
            node(w, c.element);
            w.write_bit_long(8);
            for ((side, inside, lane), slope) in SUPERELEVATION_SEGMENTS.iter().zip(s.slopes) {
                for v in [*side, *inside, *lane, 3] {
                    w.write_bit_long(v);
                }
                fields(w);
                node(w, c.element);
                w.write_bit_long(0);
                w.write_bit_double(slope);
            }
        }
        w.write_bit_long(0);
        if i + 1 < curves.len() {
            w.write_bit(false);
        }
    }
    for pair in SUPERELEVATION_SEGMENTS.chunks(2) {
        w.write_bit_long(2);
        for (side, inside, lane) in pair {
            for v in [*side, *inside, *lane] {
                w.write_bit_long(v);
            }
        }
    }
    let tail = |w: &mut DwgMergedWriter| {
        w.write_bit_long(0);
        w.write_bit_long(0);
        w.write_bit_double(0.5);
        w.write_bit_double(1.0);
        w.write_bit(false);
        w.write_bit_long(-1);
        w.write_bit(true);
        w.write_bit(false);
        w.write_bit_long(-1);
    };
    for (i, (side, inside, lane)) in SUPERELEVATION_SEGMENTS.iter().enumerate() {
        if i == 0 {
            w.write_bit_long(8);
        } else {
            w.write_bit_long(0);
            w.write_bit_double(0.0);
        }
        for v in [*side, *inside, *lane, 1] {
            w.write_bit_long(v);
        }
        tail(w);
        w.write_bit_long(0);
        w.write_bit_double(0.0);
        w.write_bit_long(2);
        tail(w);
    }
    for v in [0, 0, 8] {
        w.write_bit_long(v);
    }
    for i in by_name() {
        let (side, inside, lane) = SUPERELEVATION_SEGMENTS[i];
        for v in [side, inside, lane] {
            w.write_bit_long(v);
        }
    }
}

/// The cross segments' names in record order.
const SEGMENT_NAMES: [&str; 8] = ["Left Outside Lane", "Left Inside Lane", "Right Outside Lane", "Right Inside Lane", "Left Outside Shoulder", "Left Inside Shoulder", "Right Outside Shoulder", "Right Inside Shoulder"];

/// The cross segments in name order.
fn by_name() -> Vec<usize> {
    let mut order: Vec<usize> = (0..8).collect();
    order.sort_by_key(|i| SEGMENT_NAMES[*i]);
    order
}

/// The texts of a superelevation block (after every other text): the
/// curve names, one description per unit (nine per critical station:
/// empty, or "Manual station", "Manual station(2)", ... for the manual
/// stations of a curve in order: SM2, SM6), the lane table's names with
/// two empty texts each, the names in name order.
fn superelevation_strings(curves: &[SuperelevationCurveData]) -> Vec<String> {
    if curves.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = (1..=curves.len()).map(|i| format!("Curve.{i}")).collect();
    for c in curves {
        let mut manual = 0;
        for s in &c.stations {
            let text = if s.kind == MANUAL_STATION {
                manual += 1;
                if manual == 1 { "Manual station".to_string() } else { format!("Manual station({manual})") }
            } else {
                String::new()
            };
            out.extend((0..9).map(|_| text.clone()));
        }
    }
    for name in SEGMENT_NAMES {
        out.extend([name.to_string(), String::new(), String::new()]);
    }
    out.extend(by_name().into_iter().map(|i| SEGMENT_NAMES[i].to_string()));
    out
}

/// The references of a superelevation block (after every other one): the
/// alignment three times per curve and twice per unit, then the
/// alignment and a null reference per lane table entry.
fn superelevation_refs(curves: &[SuperelevationCurveData], alignment: u64) -> Vec<Ref> {
    if curves.is_empty() {
        return Vec::new();
    }
    let own: usize = curves.iter().map(|c| 3 + 18 * c.stations.len()).sum();
    let mut out: Vec<Ref> = (0..own).map(|_| soft_pointer(alignment)).collect();
    for _ in 0..16 {
        out.extend([soft_pointer(alignment), soft_pointer(0)]);
    }
    out
}

/// [`write_alignment`] of type `kind` (offset alignments take `offset`)
/// with `timing`.
#[allow(clippy::too_many_arguments)]
pub(super) fn write_alignment_of(doc: &mut CadDocument, common: EntityCommon, name: &str, elements: &[AlignmentElement], style: u64, feature: u64, edge: i32, keys: Option<&str>, profiles: &[u64], anchors: &[AlignmentAnchor], views: &[u64], groups: &[u64], offset: Option<&OffsetRecord>, kind: AlignmentKind, timing: &Timing) -> Result<Handle, DxfError> {
    if !(1..256).contains(&edge) {
        return Err(invalid("the alignment's edge id is out of range"));
    }
    let curves = elements.iter().filter(|e| !matches!(e.kind, AlignmentElementKind::FixedLine { .. })).count();
    let first_id = elements.iter().map(|e| i32::from(e.id)).min().unwrap_or(1);
    let start = offset.map_or(0.0, |o| o.range.0);
    let mut speeds: Vec<(f64, f64, [u8; 16])> = offset.iter().map(|o| (o.range.0, o.speed, o.own)).collect();
    speeds.extend(timing.speeds.iter().copied());
    let equations = &timing.equations;
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let mut refs = vec![soft_pointer(0), hard_pointer(feature), hard_pointer(feature), soft_pointer(0), hard_pointer(style), hard_pointer(0)];
    refs.extend(profiles.iter().map(|p| soft_pointer(*p)));
    refs.extend(views.iter().chain(groups).map(|v| soft_pointer(*v)));
    // An offset alignment's parent is its last reference (after its
    // profiles; OP01, OB01); an offset alignment has no superelevation
    // (see `add_superelevation`).
    refs.extend(superelevation_refs(&timing.superelevation, common.handle.value()));
    refs.extend(offset.map(|o| soft_pointer(o.parent)));
    // A curb return names its roads last: in, in, out, out, in, out; an
    // intersection's: in road, in offset, out road, out offset, the
    // intersection (WC40).
    if let Some(c) = &timing.connected {
        let [a, b] = c.roads;
        match &c.intersection {
            Some(x) => refs.extend([a, x.offsets[0], b, x.offsets[1], x.intersection].map(soft_pointer)),
            None => refs.extend([a, a, b, b, a, b].map(soft_pointer)),
        }
    }
    let mut strings = vec!["AeccImpAlignment".to_string(), String::new(), String::new()];
    strings.extend(timing.connected.as_ref().and_then(|c| c.text.clone()));
    strings.push(name.to_string());
    strings.extend((0..5 + 3 * curves + 2 * anchors.len() + speeds.len() + 2 * equations.len()).map(|_| String::new()));
    strings.extend(superelevation_strings(&timing.superelevation));
    let line_flags = if offset.is_some() { 0 } else { 2 };
    put_entity(
        doc,
        "AECC_ALIGNMENT",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, if offset.is_some_and(|o| !o.associative) { 0x0002_5F04 } else { 0x1002_5F04 }] {
                w.write_bit_long(v);
            }
            bits(w, "010101");
            match keys {
                // An alignment written again keeps its keys as stored.
                Some(k) => bits(w, k),
                None => {
                    let text = timing.connected.as_ref().is_some_and(|c| c.text.is_some());
                    byte(w, if text { 2 } else { 1 });
                    w.write_bit_long(STAMP);
                    w.write_bit_long(2);
                    w.write_bit_long(0);
                    w.write_bit_long(0);
                    // A curb return's curve parameters (a text key).
                    if text {
                        w.write_bit_long(KEY_CURB_CURVE);
                    }
                }
            }
            bl_byte(w, 0);
            // The edge id ends two bits after the run's byte 0x40 (its low
            // bits carry the id's high bits).
            bits(w, &"0".repeat(115));
            bits(w, &format!("00000010{:08b}{:02b}11010", 0x40 | (edge >> 2), edge & 3));
            w.write_bit_long(first_id);
            w.write_bit_double(0.0);
            w.write_bit_double(start);
            bits(w, "0000011");
            w.write_bit_double(match kind {
                AlignmentKind::Rail { gauge } => gauge,
                _ => 0.0,
            });
            w.write_bit(false);
            w.write_bit_long(first_id);
            w.write_bit_long(elements.len() as i32);
            let point = |w: &mut DwgMergedWriter, p: Vector2| {
                w.write_raw_double(p.x);
                w.write_raw_double(p.y);
            };
            for e in elements {
                let ids = |w: &mut DwgMergedWriter| {
                    w.write_bit_short(e.id);
                    w.write_bit_short(e.previous);
                    w.write_bit_short(e.next);
                };
                // A curve element's head: no design speed (BD -1), the
                // parameter block, the constraint.
                let curve = |w: &mut DwgMergedWriter, kind: i32, flags: i16, constraint: i32| {
                    w.write_bit_long(kind);
                    w.write_bit_short(flags);
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
                        w.write_bit_short(line_flags);
                        w.write_bit(false);
                        w.write_bit_short(FIXED);
                        ids(w);
                        point(w, start);
                        point(w, end);
                    }
                    AlignmentElementKind::FreeArc { radius } => {
                        curve(w, ARC, 2, BETWEEN_LINES);
                        w.write_bit(false);
                        ids(w);
                        w.write_bit_double(radius);
                    }
                    AlignmentElementKind::FixedArc { start, mid, end } => {
                        curve(w, ARC, line_flags, FIXED_ARC);
                        w.write_bit(false);
                        ids(w);
                        for p in [start, mid, end] {
                            point(w, p);
                        }
                    }
                    AlignmentElementKind::Points { ref points } => {
                        curve(w, TRANSITION, 0, POINT_SETS);
                        ids(w);
                        w.write_bit_long(1);
                        w.write_bit_long(4096);
                        w.write_bit_long(points.len() as i32);
                        for p in points {
                            point(w, *p);
                        }
                    }
                    AlignmentElementKind::FreeSpiralCurveSpiral { spiral_in, radius, spiral_out } => {
                        curve(w, SPIRAL_CURVE_SPIRAL, 2, SCS_BETWEEN_LINES);
                        ids(w);
                        w.write_bit_long(SPIRAL_DEFINITION);
                        for v in [spiral_in, radius, spiral_out] {
                            w.write_bit_double(v);
                        }
                    }
                    AlignmentElementKind::ThreeCenteredArc { arcs } => {
                        curve(w, TRANSITION, 2, 1);
                        ids(w);
                        for v in [0, 1, 3] {
                            w.write_bit_long(v);
                        }
                        for (r, l) in arcs {
                            w.write_bit_double(r);
                            w.write_bit_double(l);
                        }
                    }
                    // A curb return's spirals (CG02): the spiral in from its
                    // start and direction, the spiral out from the start and
                    // direction its connection block keeps; both end with
                    // BD 0, BD 0, BD 1.
                    AlignmentElementKind::SpiralIn { start, direction, length, radius, clockwise, shape } => {
                        curve(w, SPIRAL, 2, 0);
                        ids(w);
                        w.write_bit_long(shape.code());
                        w.write_bit(clockwise);
                        point(w, start);
                        point(w, direction);
                        for v in [length, radius, 0.0, 0.0, 1.0] {
                            w.write_bit_double(v);
                        }
                    }
                    AlignmentElementKind::SpiralOut { radius, length, clockwise, shape } => {
                        let (from, along) = timing.connected.as_ref().and_then(|c| c.spiral_out).unwrap_or_default();
                        curve(w, SPIRAL, 2, 1);
                        ids(w);
                        w.write_bit_long(shape.code());
                        w.write_bit(clockwise);
                        w.write_bit_double(radius);
                        w.write_bit_double(length);
                        point(w, from);
                        point(w, along);
                        for v in [0.0, 0.0, 1.0] {
                            w.write_bit_double(v);
                        }
                    }
                    _ => {}
                }
            }
            for v in [profiles.len(), views.len(), groups.len(), anchors.len()] {
                w.write_bit_long(v as i32);
            }
            for a in anchors {
                w.write_bytes(&a.id);
                w.write_bit(a.flag);
                w.write_bit_long(a.kind);
                for v in [a.value, a.station, a.along, a.remaining] {
                    w.write_bit_double(v);
                }
                for v in [a.element, a.previous, a.next] {
                    w.write_bit_long(v);
                }
                for p in [a.element_start, a.element_end, a.point] {
                    point(w, p);
                }
            }
            w.write_bit_long(equations.len() as i32);
            for (raw, id, back, ahead, increasing) in equations {
                bits(w, "00");
                w.write_byte(0xBE);
                w.write_byte(0x04);
                w.write_bit_double(*raw);
                w.write_bytes(id);
                w.write_bit_double(*back);
                w.write_bit_double(*ahead);
                w.write_bit(*increasing);
            }
            w.write_bit_long(1);
            w.write_bit_long(speeds.len() as i32);
            for (station, speed, id) in &speeds {
                w.write_bit_long(1);
                w.write_bit_double(*station);
                w.write_bit_double(*speed);
                w.write_bytes(id);
            }
            w.write_bit_double(100.0);
            if timing.superelevation.is_empty() {
                w.write_bit_long(0);
            } else {
                w.write_bit_long(1);
                write_superelevation(w, &timing.superelevation);
            }
            w.write_bit_long(0);
            match offset {
                None if timing.connected.is_some() => {
                    w.write_bit_long(3);
                    if let Some(c) = &timing.connected {
                        write_connected(w, c);
                    }
                }
                None => {
                    let (code, extra): (i32, &[i32]) = match kind {
                        AlignmentKind::Centerline => (1, &[]),
                        AlignmentKind::Utility => (4, &[]),
                        AlignmentKind::Rail { .. } => (5, &[0]),
                    };
                    for v in [code, 2, 2, 2, 0].iter().chain(extra).chain(&[3]) {
                        w.write_bit_long(*v);
                    }
                }
                Some(o) => {
                    w.write_bit_long(2);
                    write_offset_block(w, o);
                    for v in [0, 0, 3] {
                        w.write_bit_long(v);
                    }
                }
            }
            w.write_bit(false);
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// The offset block of an offset alignment (the reader's
/// `offset_parameters`):
///
/// ```text
/// BL 1 (follows its parent), BL 2, BL 1, BD offset, BL side (1 left,
/// 2 right), B 0, B 0 (start and end not locked to the parent's), BL 1
/// (geometry locking), BL r, r x region { BD offset, BD parent start,
/// BD parent end, BD start, BD end, BL kind (2 nominal, 1 widening),
/// BL first element, BL last element, BL 0 x 5, BL 3 }, BL t, t x
/// transition { BL 1 (linear), BD length, BL element, BD 0, BD 0, BL 2,
/// BD 0 }, BL k, k x { BL parent element, BL -1, BL n, n x BL element },
/// BL a, a x { BD parent station, the parent anchor's identifier }
/// ```
fn write_offset_block(w: &mut DwgMergedWriter, o: &OffsetRecord) {
    for v in [1, 2, 1] {
        w.write_bit_long(v);
    }
    w.write_bit_double(o.offset);
    w.write_bit_long(if o.offset < 0.0 { 1 } else { 2 });
    w.write_bit(false);
    w.write_bit(false);
    w.write_bit_long(1);
    w.write_bit_long(o.regions.len() as i32);
    for r in &o.regions {
        for v in [r.offset, r.parent.0, r.parent.1, r.own.0, r.own.1] {
            w.write_bit_double(v);
        }
        for v in [if r.widening { 1 } else { 2 }, r.first, r.last, 0, 0, 0, 0, 0, 3] {
            w.write_bit_long(v);
        }
    }
    w.write_bit_long(o.transitions.len() as i32);
    for (length, element) in &o.transitions {
        w.write_bit_long(1);
        w.write_bit_double(*length);
        w.write_bit_long(*element);
        w.write_bit_double(0.0);
        w.write_bit_double(0.0);
        w.write_bit_long(2);
        w.write_bit_double(0.0);
    }
    w.write_bit_long(o.map.len() as i32);
    for (parent, elements) in &o.map {
        for v in [*parent, -1, elements.len() as i32] {
            w.write_bit_long(v);
        }
        for e in elements {
            w.write_bit_long(*e);
        }
    }
    w.write_bit_long(o.anchors.len() as i32);
    for (station, id) in &o.anchors {
        w.write_bit_double(*station);
        w.write_bytes(id);
    }
}

/// Anchor kind of a profile's start, of an offset alignment's range ends on
/// its parent, and of an offset alignment's own start.
pub(super) const PROFILE_ANCHOR: i32 = 32763;
const OFFSET_END_ANCHOR: i32 = 6;
const OWN_START_ANCHOR: i32 = 2;

/// A solved piece: its kind, signed curvature (curves) and points.
type Piece = (AlignmentPiece, f64, Vec<Vector2>);

/// Length of a piece: a curve's arc, a line's or a spiral's points.
fn piece_length((kind, k, points): &Piece) -> f64 {
    let (first, last) = (points[0], points[points.len() - 1]);
    match kind {
        AlignmentPiece::Curve if *k != 0.0 => 2.0 * (first.distance(&last) * k.abs() / 2.0).min(1.0).asin() / k.abs(),
        _ => points.windows(2).map(|w| w[0].distance(&w[1])).sum(),
    }
}

/// The elements of an alignment in station order, each a curve or not,
/// as pieces of kind (0 line, 1 arc, 2 spiral), length and signed radius
/// (right of travel positive; a spiral's that of its arc, 0 on lines):
/// lines and arcs one piece, spiral-curve-spirals three (spirals of their
/// stored lengths), with each element's first and last point. `None`
/// for other elements.
#[allow(clippy::type_complexity)]
pub(super) fn element_pieces(elements: &[AlignmentElement]) -> Option<Vec<(bool, Vec<(u8, f64, f64)>, [Vector2; 2])>> {
    let mut out = Vec::new();
    for (e, _, pieces) in spans(elements)? {
        let radius = |p: &Piece| if p.1 != 0.0 { -1.0 / p.1 } else { 0.0 };
        // An arc's length, also past half a circle (its middle point then
        // lies farther than the radius from its chord's middle).
        let arc = |(_, k, points): &Piece| {
            let (a, b) = (points[0], points[points.len() - 1]);
            let r = 1.0 / k.abs();
            let half = (a.distance(&b) / (2.0 * r)).clamp(-1.0, 1.0).asin();
            let past = points[points.len() / 2].distance(&Vector2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)) > r;
            2.0 * r * if past { std::f64::consts::PI - half } else { half }
        };
        let ends = [*pieces.first()?.2.first()?, *pieces.last()?.2.last()?];
        let (curve, parts) = match (&e.kind, pieces.as_slice()) {
            (_, [p]) if p.0 == AlignmentPiece::Line => (false, vec![(0, piece_length(p), 0.0)]),
            (_, [p]) if p.0 == AlignmentPiece::Curve && p.1 != 0.0 => (true, vec![(1, arc(p), radius(p))]),
            (AlignmentElementKind::FreeSpiralCurveSpiral { spiral_in, spiral_out, .. }, [_, c, _]) if c.0 == AlignmentPiece::Curve && c.1 != 0.0 => {
                let r = radius(c);
                (true, vec![(2, *spiral_in, r), (1, arc(c), r), (2, *spiral_out, r)])
            }
            _ => return None,
        };
        out.push((curve, parts, ends));
    }
    Some(out)
}

/// The point at distance `s` from a piece's start and the direction
/// there.
fn piece_point((kind, k, points): &Piece, s: f64) -> (Vector2, Vector2) {
    let (first, last) = (points[0], points[points.len() - 1]);
    let unit = |v: Vector2| v * (1.0 / v.length());
    let left = |u: Vector2| Vector2::new(-u.y, u.x);
    if *kind == AlignmentPiece::Curve && *k != 0.0 && first != last {
        // The chord turns half the sweep from the start's direction.
        let sweep = 2.0 * (first.distance(&last) * k.abs() / 2.0).min(1.0).asin() * k.signum();
        let chord = unit(last - first);
        let (c, d) = ((-sweep / 2.0).cos(), (-sweep / 2.0).sin());
        let t0 = Vector2::new(c * chord.x - d * chord.y, d * chord.x + c * chord.y);
        let center = first + left(t0) * (1.0 / k);
        let (c, d) = ((s * k).cos(), (s * k).sin());
        let r = first - center;
        let p = center + Vector2::new(c * r.x - d * r.y, d * r.x + c * r.y);
        return (p, Vector2::new(c * t0.x - d * t0.y, d * t0.x + c * t0.y));
    }
    let mut left_over = s;
    for w in points.windows(2) {
        let l = w[0].distance(&w[1]);
        if l > 0.0 && (left_over <= l || std::ptr::eq(w[1..].as_ptr(), &points[points.len() - 1])) {
            return (w[0] + (w[1] - w[0]) * (left_over / l), unit(w[1] - w[0]));
        }
        left_over -= l;
    }
    (last, if first == last { Vector2::new(1.0, 0.0) } else { unit(last - first) })
}

/// The lengths of an element's pieces: a spiral-curve-spiral's spirals
/// their stored lengths (their solved points run a little short), the
/// others [`piece_length`].
fn piece_lengths(e: &AlignmentElement, pieces: &[Piece]) -> Vec<f64> {
    let mut out: Vec<f64> = pieces.iter().map(piece_length).collect();
    if let (AlignmentElementKind::FreeSpiralCurveSpiral { spiral_in, spiral_out, .. }, [a, _, b]) = (&e.kind, out.as_mut_slice()) {
        (*a, *b) = (*spiral_in, *spiral_out);
    }
    out
}

/// The elements in station order with their pieces and where each starts
/// (distance from the path's start): lines and arcs make one piece,
/// spiral-curve-spirals three.
fn spans(elements: &[AlignmentElement]) -> Option<Vec<(&AlignmentElement, f64, Vec<Piece>)>> {
    let pieces = pieces_with_curvature(elements);
    let mut out = Vec::with_capacity(elements.len());
    let mut at = elements.iter().find(|e| e.previous == 0);
    let (mut used, mut from) = (0, 0.0);
    while let Some(e) = at {
        let n = match e.kind {
            AlignmentElementKind::FreeSpiralCurveSpiral { .. } => 3,
            _ => 1,
        };
        let own = pieces.get(used..used + n)?.to_vec();
        used += n;
        let length: f64 = piece_lengths(e, &own).iter().sum();
        out.push((e, from, own));
        from += length;
        (out.len() <= elements.len()).then_some(())?;
        at = (e.next != 0).then(|| elements.iter().find(|x| x.id == e.next)).flatten();
    }
    (out.len() == elements.len() && used == pieces.len()).then_some(out)
}

/// The critical stations of the superelevation `curves` of the alignment
/// `elements` (start station `start`): station, curve element, kind code,
/// ending and cross slopes, in station order; and the alignment's end
/// station.
#[allow(clippy::type_complexity)]
pub(super) fn superelevation_stations(elements: &[AlignmentElement], start: f64, curves: &[SuperelevationCurveData]) -> Option<(Vec<(f64, i16, i32, bool, [f64; 8])>, f64)> {
    let spans = spans(elements)?;
    let mut out = Vec::new();
    for c in curves {
        let (e, from, pieces) = spans.iter().find(|(e, _, _)| e.id == c.element)?;
        let to = from + piece_lengths(e, pieces).iter().sum::<f64>();
        out.extend(c.stations.iter().map(|s| (start + if s.ending { to } else { *from } + s.offset, c.element, s.kind, s.ending, s.slopes)));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    let length: f64 = spans.iter().flat_map(|(e, _, p)| piece_lengths(e, p)).sum();
    Some((out, start + length))
}

/// The station of the point of the alignment `elements` (start station
/// `start`) nearest to `p`: lines and arcs exactly, spirals along their
/// points.
pub(super) fn station_of_point(elements: &[AlignmentElement], start: f64, p: Vector2) -> Option<f64> {
    let mut best: Option<(f64, f64)> = None;
    let mut take = |d: f64, s: f64| {
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, s));
        }
    };
    for (e, from, pieces) in spans(elements)? {
        let mut at = from;
        // A spiral's points run a little short of its length: their
        // distances are scaled to it.
        for (piece, length) in pieces.iter().zip(piece_lengths(e, &pieces)) {
            let scale = length / piece_length(piece).max(1e-12);
            let (kind, k, points) = piece;
            let (first, last) = (points[0], points[points.len() - 1]);
            if *kind == AlignmentPiece::Curve && *k != 0.0 && first != last {
                let r = 1.0 / k.abs();
                let sweep = 2.0 * (first.distance(&last) * k.abs() / 2.0).min(1.0).asin();
                let (_, tangent) = piece_point(piece, 0.0);
                let left = Vector2::new(-tangent.y, tangent.x);
                let center = first + left * (r * k.signum());
                let angle = |q: Vector2| (q.y - center.y).atan2(q.x - center.x);
                // Turned from the start in the direction of travel.
                let turned = ((angle(p) - angle(first)) * k.signum()).rem_euclid(std::f64::consts::TAU);
                let a = if turned <= sweep { turned } else if turned - sweep < std::f64::consts::TAU - turned { sweep } else { 0.0 };
                let (q, _) = piece_point(piece, a * r);
                take(q.distance(&p), start + at + a * r);
            } else {
                let mut run = 0.0;
                for w in points.windows(2) {
                    let (d, l) = (w[1] - w[0], w[0].distance(&w[1]));
                    let t = if l > 0.0 { ((p - w[0]).dot(&d) / (l * l)).clamp(0.0, 1.0) } else { 0.0 };
                    take((w[0] + d * t).distance(&p), start + at + (run + t * l) * scale);
                    run += l;
                }
            }
            at += length;
        }
    }
    best.map(|b| b.1)
}

/// The point at `station` (start station `start`) of the solved path and
/// its direction.
fn station_point(spans: &[(&AlignmentElement, f64, Vec<Piece>)], start: f64, station: f64) -> Option<(Vector2, Vector2)> {
    let (e, from, pieces) = spans.iter().rev().find(|(_, from, _)| station - start >= *from - 1e-9)?;
    let mut s = station - start - from;
    for (i, (p, l)) in pieces.iter().zip(piece_lengths(e, pieces)).enumerate() {
        if s <= l || i + 1 == pieces.len() {
            // Along a spiral's points, scaled from its length to theirs.
            return Some(piece_point(p, s.min(l) * piece_length(p) / l.max(1e-12)));
        }
        s -= l;
    }
    None
}

/// An anchor of `kind` at `station` of the alignment `elements` with start
/// station `start`: the element the station lies on (the one that starts
/// there at a join, the last one at the end), the distances to its ends,
/// its ends and the point at the station.
pub(super) fn anchor_at(elements: &[AlignmentElement], start: f64, kind: i32, id: [u8; 16], station: f64) -> Option<AlignmentAnchor> {
    let spans = spans(elements)?;
    let total: f64 = spans.iter().map(|(e, _, p)| piece_lengths(e, p).iter().sum::<f64>()).sum();
    let tol = 1e-9 * total.max(1.0);
    let s = station - start;
    (s >= -tol && s <= total + tol).then_some(())?;
    let i = spans.iter().rposition(|(_, from, _)| s >= *from - tol)?;
    let (e, from, pieces) = &spans[i];
    let length: f64 = piece_lengths(e, pieces).iter().sum();
    let along = (s - from).clamp(0.0, length);
    let (first, last) = (pieces.first()?.2[0], *pieces.last()?.2.last()?);
    Some(AlignmentAnchor {
        id,
        flag: true,
        kind,
        value: 0.0,
        station,
        along,
        remaining: length - along,
        element: i32::from(e.id),
        previous: i32::from(e.previous),
        next: i32::from(e.next),
        element_start: first,
        element_end: last,
        point: station_point(&spans, start, station)?.0,
    })
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
    add_alignment_of_kind(doc, name, vertices, curves, AlignmentKind::Centerline, layer)
}

/// [`add_alignment_with_curves`] of type `kind`: a utility alignment, or a
/// rail alignment with a positive track gauge.
pub fn add_alignment_of_kind(doc: &mut CadDocument, name: &str, vertices: &[Vector2], curves: &[VertexCurve], kind: AlignmentKind, layer: Option<&str>) -> Result<Handle, DxfError> {
    if matches!(kind, AlignmentKind::Rail { gauge } if !(gauge.is_finite() && gauge > 0.0)) {
        return Err(invalid("a rail alignment needs a positive track gauge"));
    }
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
    write_companion(doc, companion, node, tag, handle, &path, &label, false)?;
    add_node_child(doc, node, tag as i32, companion, &label)?;
    write_alignment_of(doc, common, name, &elements, style, companion.value(), 1, None, &[], &[], &[], &[], None, kind, &Timing::default())
}

/// A widening of an offset alignment: from the parent's station `start` it
/// moves to `offset` (from the parent, negative to the left) over `entry`
/// along the parent, keeps it to `end`, and comes back to the nominal
/// offset over `exit` after `end`, by linear transitions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Widening {
    pub start: f64,
    pub end: f64,
    pub offset: f64,
    pub entry: f64,
    pub exit: f64,
}

/// Adds an offset alignment `name` at `offset` from `parent` (negative to
/// the left), between the parent's stations `range` (its whole length when
/// `None`), on `layer` (layer "0" when `None`). It follows its parent as
/// the application keeps it: every parent element in the range gives one
/// element (a line its parallel, an arc the concentric arc), the parent is
/// written again with an anchor at each end of the range and the offset
/// alignment keeps their identifiers. The parent must be a siteless
/// alignment of lines and arcs this crate writes.
pub fn add_offset_alignment(doc: &mut CadDocument, parent: Handle, name: &str, offset: f64, range: Option<(f64, f64)>, layer: Option<&str>) -> Result<Handle, DxfError> {
    add_offset_alignment_with_widenings(doc, parent, name, offset, range, &[], layer)
}

/// One stretch of an offset alignment along its parent: at one offset
/// (`ramp` none) or moving from one offset to another.
struct Stretch {
    from: f64,
    to: f64,
    offset: f64,
    ramp: Option<f64>,
    widening: bool,
}

/// [`add_offset_alignment`] with `widenings` (in station order, apart, with
/// their transitions within the range; a transition longer than 0 must lie
/// on one line of the parent). Each widening is a region of its own with an
/// element per parent element it covers; each transition is one element
/// from where it starts to where it ends.
pub fn add_offset_alignment_with_widenings(doc: &mut CadDocument, parent: Handle, name: &str, offset: f64, range: Option<(f64, f64)>, widenings: &[Widening], layer: Option<&str>) -> Result<Handle, DxfError> {
    write_offset_alignment(doc, parent, name, offset, range, widenings, layer, (0.0, false), None)
}

/// [`add_offset_alignment`] over the parent's whole length (or `range`)
/// with its own design speed `speed`, kept by an associative action (an intersection's
/// offset alignments; the caller adds the action).
/// It takes `style` (else the drawing's default alignment style).
#[allow(clippy::too_many_arguments)]
pub(super) fn add_kept_offset_alignment(doc: &mut CadDocument, parent: Handle, name: &str, offset: f64, range: Option<(f64, f64)>, speed: f64, style: Option<u64>, layer: Option<&str>) -> Result<Handle, DxfError> {
    write_offset_alignment(doc, parent, name, offset, range, &[], layer, (speed, true), style)
}

/// [`add_offset_alignment_with_widenings`]; `kept` is the own design speed
/// and whether an action keeps it, `style` the alignment style (else the
/// drawing's default).
#[allow(clippy::too_many_arguments)]
fn write_offset_alignment(doc: &mut CadDocument, parent: Handle, name: &str, offset: f64, range: Option<(f64, f64)>, widenings: &[Widening], layer: Option<&str>, kept: (f64, bool), style: Option<u64>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("an offset alignment needs a name"));
    }
    let side_ok = |v: f64| v.is_finite() && v != 0.0 && v.signum() == offset.signum();
    if !offset.is_finite() || offset == 0.0 || widenings.iter().any(|w| !side_ok(w.offset)) {
        return Err(invalid("an offset alignment needs finite offsets other than 0, its widenings on its side"));
    }
    let (written, profiles) = rewritable(doc, parent, |h| super::profile::identifier(doc, h)).ok_or_else(|| invalid("the parent is not an alignment this crate writes"))?;
    if object(doc, Handle::new(written.feature)).is_none_or(|o| o.dxf_name != "AECC_FEATURE_STYLE") {
        return Err(invalid("the parent's companion is missing"));
    }
    // An offset of an alignment on a site joins that site (WC64).
    let site = on_site(doc, parent);
    if written.elements.iter().any(|e| !matches!(e.kind, AlignmentElementKind::FixedLine { .. } | AlignmentElementKind::FreeArc { .. })) {
        return Err(invalid("the parent has elements other than lines and arcs"));
    }
    let spans = spans(&written.elements).ok_or_else(|| invalid("the parent cannot be solved"))?;
    let total: f64 = spans.iter().map(|(e, _, p)| piece_lengths(e, p).iter().sum::<f64>()).sum();
    // An intersection's offsets are cut to the parent's ends (a tee
    // junction's road ending or starting at the intersection, WC50).
    let (s0, s1) = range.map_or((0.0, total), |(a, b)| if kept.1 { (a.max(0.0), b.min(total)) } else { (a, b) });
    let tol = 1e-9 * total.max(1.0);
    if !(s0.is_finite() && s1.is_finite() && s0 >= -tol && s1 <= total + tol && s1 - s0 > tol) {
        return Err(invalid("the range must run forward within the parent's stations"));
    }
    // The stretches in station order: nominal, entry, widening, exit, ...
    let mut stretches = Vec::new();
    let mut cursor = s0;
    for w in widenings {
        let finite = [w.start, w.end, w.entry, w.exit].iter().all(|v| v.is_finite());
        if !finite || w.entry < 0.0 || w.exit < 0.0 || w.start <= cursor + tol || w.start + w.entry >= w.end - tol || w.end + w.exit >= s1 - tol {
            return Err(invalid("widenings must lie apart, in station order, with their transitions inside the range"));
        }
        stretches.push(Stretch { from: cursor, to: w.start, offset, ramp: None, widening: false });
        stretches.push(Stretch { from: w.start, to: w.start + w.entry, offset, ramp: Some(w.offset), widening: false });
        stretches.push(Stretch { from: w.start + w.entry, to: w.end, offset: w.offset, ramp: None, widening: true });
        stretches.push(Stretch { from: w.end, to: w.end + w.exit, offset: w.offset, ramp: Some(offset), widening: false });
        cursor = w.end + w.exit;
    }
    stretches.push(Stretch { from: cursor, to: s1, offset, ramp: None, widening: false });

    let right = |u: Vector2| Vector2::new(u.y, -u.x);
    let at = |s: f64, off: f64| -> Option<Vector2> {
        let (p, u) = station_point(&spans, 0.0, s)?;
        Some(p + right(u) * off)
    };
    // The pieces in station order: (stretch, parent element (none for a
    // transition), kind, bulge, own length).
    let mut pieces: Vec<(usize, Option<i32>, AlignmentElementKind, Option<f64>, f64)> = Vec::new();
    for (si, st) in stretches.iter().enumerate() {
        if let Some(to_offset) = st.ramp {
            let (a, b) = (at(st.from, st.offset).ok_or_else(|| invalid("a transition cannot be placed"))?, at(st.to, to_offset).ok_or_else(|| invalid("a transition cannot be placed"))?);
            if st.to - st.from > tol {
                let mid = (st.from + st.to) / 2.0;
                let line = spans.iter().find(|(e, from, ps)| matches!(e.kind, AlignmentElementKind::FixedLine { .. }) && mid >= *from && mid <= from + piece_length(&ps[0]));
                if line.is_none_or(|(_, from, ps)| st.from < from - tol || st.to > from + piece_length(&ps[0]) + tol) {
                    return Err(invalid("a transition must lie on one line of the parent"));
                }
            }
            pieces.push((si, None, AlignmentElementKind::Points { points: vec![a, b] }, None, a.distance(&b)));
            continue;
        }
        for (e, from, ps) in &spans {
            let piece = &ps[0];
            let length = piece_length(piece);
            let (a, b) = ((st.from - from).max(0.0), (st.to - from).min(length));
            if b - a <= tol {
                continue;
            }
            let at = |s: f64| {
                let (p, u) = piece_point(piece, s);
                p + right(u) * st.offset
            };
            let k = piece.1;
            let (kind, bulge, own) = match e.kind {
                AlignmentElementKind::FreeArc { .. } => {
                    // The concentric arc: the radius grows on the side away
                    // from the center (left of the path when k > 0).
                    let radius = 1.0 / k.abs() + if k > 0.0 { st.offset } else { -st.offset };
                    if radius <= tol {
                        return Err(invalid("the offset passes an arc's center"));
                    }
                    let sweep = (b - a) * k;
                    (AlignmentElementKind::FixedArc { start: at(a), mid: at((a + b) / 2.0), end: at(b) }, Some((sweep / 4.0).tan()), radius * sweep.abs())
                }
                _ => (AlignmentElementKind::FixedLine { start: at(a), end: at(b) }, None, b - a),
            };
            pieces.push((si, Some(i32::from(e.id)), kind, bulge, own));
        }
    }
    // Ids as the application gives them: from 2n + 1 the parent elements in
    // the range (to the first piece each keeps at the nominal offset), then
    // per widening its pieces, the nominal pieces after it, its transitions.
    let base = 2 * written.elements.iter().map(|e| i32::from(e.id)).max().unwrap_or(0) + 1;
    let mut in_range: Vec<i32> = Vec::new();
    for p in &pieces {
        if let Some(e) = p.1 {
            if !in_range.contains(&e) {
                in_range.push(e);
            }
        }
    }
    let mut ids: Vec<Option<i32>> = vec![None; pieces.len()];
    for (i, p) in pieces.iter().enumerate() {
        let nominal = p.1.is_some() && !stretches[p.0].widening;
        if nominal && !pieces[..i].iter().any(|q| q.1 == p.1 && !stretches[q.0].widening) {
            ids[i] = Some(base + in_range.iter().position(|e| Some(*e) == p.1).unwrap_or(0) as i32);
        }
    }
    let mut next = base + in_range.len() as i32;
    for w in 0..widenings.len() {
        let (entry, wide, exit, after) = (4 * w + 1, 4 * w + 2, 4 * w + 3, 4 * w + 4);
        for stretch in [wide, after] {
            for (i, p) in pieces.iter().enumerate() {
                if p.0 == stretch && ids[i].is_none() {
                    ids[i] = Some(next);
                    next += 1;
                }
            }
        }
        for stretch in [entry, exit] {
            if let Some(i) = pieces.iter().position(|p| p.0 == stretch) {
                ids[i] = Some(next);
                next += 1;
            }
        }
    }
    let ids: Vec<i32> = ids.into_iter().collect::<Option<_>>().ok_or_else(|| invalid("the range holds no element"))?;
    let count = pieces.len();
    let mut elements: Vec<AlignmentElement> = (0..count)
        .map(|i| AlignmentElement {
            id: ids[i] as i16,
            previous: if i == 0 { 0 } else { ids[i - 1] as i16 },
            next: if i + 1 == count { 0 } else { ids[i + 1] as i16 },
            kind: pieces[i].2.clone(),
        })
        .collect();
    let starts_of = |kind: &AlignmentElementKind| match kind {
        AlignmentElementKind::FixedLine { start, end } => (*start, *end),
        AlignmentElementKind::FixedArc { start, end, .. } => (*start, *end),
        AlignmentElementKind::Points { points } => (points[0], points[points.len() - 1]),
        _ => unreachable!(),
    };
    let mut path: Vec<PathPoint> = pieces.iter().map(|p| (starts_of(&p.2).0, p.3)).collect();
    path.push((starts_of(&pieces.last().ok_or_else(|| invalid("the range holds no element"))?.2).1, None));
    // Own stations of the pieces' ends. An intersection's clipped offset
    // alignment starts at 0 (WC40), any other at its parent's station.
    let clipped = kept.1 && range.is_some();
    let mut own = vec![if clipped { 0.0 } else { s0 }];
    for p in &pieces {
        own.push(own[own.len() - 1] + p.4);
    }
    let regions: Vec<OffsetRegionRecord> = stretches
        .iter()
        .enumerate()
        .filter(|(_, st)| st.ramp.is_none())
        .map(|(si, st)| {
            let held: Vec<usize> = (0..count).filter(|i| pieces[*i].0 == si).collect();
            let (first, last) = (held[0], held[held.len() - 1]);
            OffsetRegionRecord { offset: st.offset, parent: (st.from, st.to), own: (own[first], own[last + 1]), widening: st.widening, first: ids[first], last: ids[last] }
        })
        .collect();
    let transitions: Vec<(f64, i32)> = stretches
        .iter()
        .enumerate()
        .filter(|(_, st)| st.ramp.is_some())
        .map(|(si, st)| (st.to - st.from, ids[pieces.iter().position(|p| p.0 == si).unwrap_or(0)]))
        .collect();
    let mut map: Vec<(i32, Vec<i32>)> = in_range.iter().map(|e| (*e, (0..count).filter(|i| pieces[*i].1 == Some(*e)).map(|i| ids[i]).collect())).collect();
    map.sort();
    elements.sort_by_key(|e| e.id);
    let mut stations: Vec<f64> = Vec::new();
    for st in &stretches {
        for s in [st.from, st.to] {
            if stations.iter().all(|t| (t - s).abs() > tol) {
                stations.push(s);
            }
        }
    }
    stations.sort_by(f64::total_cmp);

    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let handle = common.handle;
    let id = |salt: u64| super::profile::new_identifier(handle.value() ^ salt);
    let anchors: Vec<(f64, [u8; 16])> = stations.iter().enumerate().map(|(i, s)| (*s, id(i as u64 + 16))).collect();
    let own_id = id(3);
    let extra = anchors
        .iter()
        .map(|(s, g)| anchor_at(&written.elements, 0.0, OFFSET_END_ANCHOR, *g, *s).ok_or_else(|| invalid("an anchor cannot be placed on the parent")))
        .collect::<Result<Vec<_>, _>>()?;
    // Its own anchor lies at the start of its first element; a clipped
    // intersection offset's anchor names the whole parent element's offset
    // (WC40: station 0 at the start of the uncut line, 200 remaining).
    let (mut first_start, mut first_end) = starts_of(&pieces[0].2);
    let mut remaining = pieces[0].4;
    if clipped {
        if let Some((e, from, ps)) = spans.iter().find(|(e, ..)| Some(i32::from(e.id)) == pieces[0].1) {
            let piece = &ps[0];
            let length = piece_length(piece);
            let at = |s: f64| {
                let (p, u) = piece_point(piece, s);
                p + right(u) * offset
            };
            let _ = from;
            (first_start, first_end) = (at(0.0), at(length));
            remaining = match e.kind {
                AlignmentElementKind::FreeArc { .. } => (1.0 / piece.1.abs() + if piece.1 > 0.0 { offset } else { -offset }) * (length * piece.1).abs(),
                _ => length,
            };
        }
    }
    let own_anchor = AlignmentAnchor {
        id: own_id,
        flag: true,
        kind: OWN_START_ANCHOR,
        value: 0.0,
        station: own[0],
        along: 0.0,
        remaining,
        element: ids[0],
        previous: 0,
        next: ids.get(1).copied().unwrap_or(0),
        element_start: first_start,
        element_end: first_end,
        point: first_start,
    };
    let record = OffsetRecord { parent: parent.value(), offset, range: (own[0], own[count]), regions, transitions, map, anchors, own: own_id, speed: kept.0, associative: kept.1 };
    let groups = written.groups.clone();
    rewrite_adding(doc, parent, written, &profiles, &profiles, None, &groups, &extra, &Timing::default())?;
    let style = match style {
        Some(s) => s,
        None => default_style(doc).ok_or_else(|| invalid("drawing has no alignment styles"))?,
    };
    if let Some(site) = site {
        let elevations = vec![0.0; path.len()];
        let (h, _) = super::site::join_site_as(doc, site, common, path, &elevations, super::site::MemberKind::Alignment, true, |doc, common, edge, container| {
            write_alignment(doc, common, name, &elements, style, container.value(), edge, None, &[], &[own_anchor], &[], &[], Some(&record))
        })?;
        return Ok(h);
    }
    let node = companion_node(doc).ok_or_else(|| invalid("drawing has no node for siteless alignments"))?;
    let used = read_node(doc, node).ok_or_else(|| invalid("feature style node cannot be read"))?;
    let tag = (FIRST_COMPANION_TAG..).find(|t| used.children.iter().all(|c| c.0 as u32 != *t)).unwrap_or(FIRST_COMPANION_TAG);
    let companion = doc.allocate_handle();
    let label = new_name(handle.value());
    write_companion(doc, companion, node, tag, handle, &path, &label, true)?;
    add_node_child(doc, node, tag as i32, companion, &label)?;
    write_alignment(doc, common, name, &elements, style, companion.value(), 1, None, &[], &[own_anchor], &[], &[], Some(&record))
}

/// The solved length of an alignment's elements.
pub(super) fn length_of(elements: &[AlignmentElement]) -> Option<f64> {
    Some(spans(elements)?.iter().map(|(e, _, p)| piece_lengths(e, p).iter().sum::<f64>()).sum())
}

/// Adds superelevation to `alignment`: one curve per curve element (a
/// free arc or spiral-curve-spiral, by id) with its critical stations
/// and their cross slopes, replacing the alignment's own. Each station is
/// stored from the curve's start (from its end in the ending attainment
/// region). The alignment must be one this crate writes.
pub fn add_superelevation(doc: &mut CadDocument, alignment: Handle, curves: &[SuperelevationCurve]) -> Result<(), DxfError> {
    let (written, profiles) = rewritable(doc, alignment, |h| super::profile::identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    if written.offset.is_some() {
        return Err(invalid("an offset alignment takes no superelevation"));
    }
    let spans = spans(&written.elements).ok_or_else(|| invalid("the alignment cannot be solved"))?;
    let mut data = Vec::new();
    for c in curves {
        let (e, from, pieces) = spans.iter().find(|(e, _, _)| e.id == c.element).ok_or_else(|| invalid("no element with that id"))?;
        if matches!(e.kind, AlignmentElementKind::FixedLine { .. }) || c.stations.is_empty() {
            return Err(invalid("a superelevation curve needs a curve element and critical stations"));
        }
        if c.stations.iter().any(|s| !s.station.is_finite() || s.slopes.iter().any(|v| !v.is_finite())) || c.stations.windows(2).any(|w| w[1].station < w[0].station) {
            return Err(invalid("critical stations need finite values in station order"));
        }
        let to = from + piece_lengths(e, pieces).iter().sum::<f64>();
        let stations = c.stations.iter().map(|s| CriticalStationData { kind: s.kind.code(), offset: s.station - if s.ending { to } else { *from }, ending: s.ending, slopes: s.slopes }).collect();
        data.push((*from, SuperelevationCurveData { element: c.element, stations }));
    }
    data.sort_by(|a, b| a.0.total_cmp(&b.0));
    if data.windows(2).any(|w| w[0].1.element == w[1].1.element) || data.is_empty() {
        return Err(invalid("one superelevation curve per curve element"));
    }
    let timing = Timing { superelevation: data.into_iter().map(|(_, c)| c).collect(), ..Timing::default() };
    let groups = written.groups.clone();
    rewrite_adding(doc, alignment, written, &profiles, &profiles, None, &groups, &[], &timing)
}

/// Anchor kinds of a station equation and of a design speed.
const EQUATION_ANCHOR: i32 = 3;
const SPEED_ANCHOR: i32 = 2;

/// Adds design speeds `(station, speed)` to `alignment`: each ties to an
/// anchor at its station. The alignment must be one this crate writes.
pub fn add_design_speeds(doc: &mut CadDocument, alignment: Handle, speeds: &[(f64, f64)]) -> Result<(), DxfError> {
    if speeds.iter().any(|(s, v)| !s.is_finite() || !(v.is_finite() && *v >= 0.0)) {
        return Err(invalid("design speeds need finite stations and speeds"));
    }
    add_timing(doc, alignment, speeds.iter().map(|(s, v)| (*s, Some(*v), None)).collect())
}

/// Adds station equations `(raw station, station ahead, increasing)` to
/// `alignment`, after the ones it has: each ties to an anchor at its raw
/// station and keeps the station back the equations before it give. The
/// alignment must be one this crate writes.
pub fn add_station_equations(doc: &mut CadDocument, alignment: Handle, equations: &[(f64, f64, bool)]) -> Result<(), DxfError> {
    if equations.iter().any(|(r, a, _)| !r.is_finite() || !a.is_finite()) || equations.windows(2).any(|w| w[1].0 <= w[0].0) {
        return Err(invalid("station equations need finite stations in increasing order"));
    }
    add_timing(doc, alignment, equations.iter().map(|(r, a, i)| (*r, None, Some((*a, *i)))).collect())
}

/// Writes `alignment` again with design speeds (`Some` speed) or station
/// equations (`Some` (ahead, increasing)) at the given raw stations.
fn add_timing(doc: &mut CadDocument, alignment: Handle, items: Vec<(f64, Option<f64>, Option<(f64, bool)>)>) -> Result<(), DxfError> {
    let (written, profiles) = rewritable(doc, alignment, |h| super::profile::identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    let seed = written.common.handle.value() ^ (written.anchors.len() as u64) << 20;
    let mut extra = Vec::new();
    let mut timing = Timing::default();
    let mut equations: Vec<crate::classes::civil::StationEquation> = written.timing.equations.iter().map(|e| crate::classes::civil::StationEquation { raw: e.0, back: e.2, ahead: e.3, increasing: e.4 }).collect();
    for (i, (station, speed, equation)) in items.into_iter().enumerate() {
        let id = super::profile::new_identifier(seed ^ i as u64);
        let kind = if equation.is_some() { EQUATION_ANCHOR } else { SPEED_ANCHOR };
        extra.push(anchor_at(&written.elements, 0.0, kind, id, station).ok_or_else(|| invalid("a station is not on the alignment"))?);
        match (speed, equation) {
            (Some(v), _) => timing.speeds.push((station, v, id)),
            (_, Some((ahead, increasing))) => {
                if equations.last().is_some_and(|e| e.raw >= station) {
                    return Err(invalid("station equations must follow the alignment's"));
                }
                let back = crate::classes::civil::display_station(&equations, station);
                equations.push(crate::classes::civil::StationEquation { raw: station, back, ahead, increasing });
                timing.equations.push((station, id, back, ahead, increasing));
            }
            _ => {}
        }
    }
    let groups = written.groups.clone();
    rewrite_adding(doc, alignment, written, &profiles, &profiles, None, &groups, &extra, &timing)
}

/// Anchor kind of a corridor region's end locked to the alignment's
/// geometry.
pub(super) const LOCK_ANCHOR: i32 = 582;

/// Writes `alignment` again with anchors of `kind` at the given stations,
/// by the given identifiers. The alignment must be one this crate writes.
pub(super) fn add_anchors(doc: &mut CadDocument, alignment: Handle, kind: i32, anchors: &[([u8; 16], f64)]) -> Result<(), DxfError> {
    let (written, profiles) = rewritable(doc, alignment, |h| super::profile::identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    let extra = anchors
        .iter()
        .map(|(id, station)| anchor_at(&written.elements, 0.0, kind, *id, *station).ok_or_else(|| invalid("a station is not on the alignment")))
        .collect::<Result<Vec<_>, _>>()?;
    let groups = written.groups.clone();
    rewrite_adding(doc, alignment, written, &profiles, &profiles, None, &groups, &extra, &Timing::default())
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
    /// Its keys with the superelevation type key (added when missing).
    pub keys_superelevation: String,
    pub edge: i32,
    /// Its station anchors as stored.
    pub anchors: Vec<AlignmentAnchor>,
    /// Its type, station equations and design speeds.
    pub kind: AlignmentKind,
    pub timing: Timing,
    /// The profile views of the alignment, oldest first.
    pub views: Vec<u64>,
    /// Its sample line groups, oldest first.
    pub groups: Vec<u64>,
    /// An offset alignment's offset block.
    pub offset: Option<OffsetRecord>,
}

/// An alignment this crate can write again: its elements are the ones the
/// writer writes, its references are the writer's six, profiles and
/// profile views, and
/// writing it from what is read gives back its record bit for bit (so
/// nothing the application stores is lost). Returns it with its profiles.
pub(super) fn rewritable(doc: &CadDocument, alignment: Handle, profile_id: impl Fn(u64) -> Option<[u8; 16]>) -> Option<(Written, Vec<ProfileRef>)> {
    let Some(crate::entities::EntityType::Extended(x)) = doc.get_entity(alignment) else { return None };
    let crate::entities::ExtendedEntityData::Arch(a) = &x.data else { return None };
    (a.dxf_name == "AECC_ALIGNMENT").then_some(())?;
    let elements = a.alignment_elements()?;
    let offset = match a.alignment_offset_parameters() {
        Some(o) => Some(offset_record(a, o)?),
        None => None,
    };
    let fits = |e: &AlignmentElement| match e.kind {
        AlignmentElementKind::FixedLine { .. } | AlignmentElementKind::FreeArc { .. } | AlignmentElementKind::FreeSpiralCurveSpiral { .. } => true,
        AlignmentElementKind::FixedArc { .. } | AlignmentElementKind::Points { .. } => offset.is_some(),
        _ => false,
    };
    elements.iter().all(fits).then_some(())?;
    let refs = a.typed_handles();
    let parent_refs = usize::from(offset.is_some());
    (refs.len() >= 6 + parent_refs).then_some(())?;
    let is = |h: u64, dxf: &str| matches!(doc.get_entity(Handle::new(h)), Some(crate::entities::EntityType::Extended(x)) if matches!(&x.data, crate::entities::ExtendedEntityData::Arch(v) if v.dxf_name == dxf));
    let superelevation = a.alignment_superelevation()?;
    (offset.is_none() || superelevation.is_empty()).then_some(())?;
    let tail_end = refs.len().checked_sub(parent_refs + superelevation_refs(&superelevation, 0).len())?.max(6);
    let tail: Vec<u64> = refs[6..tail_end].iter().map(|(h, _)| *h).collect();
    let views_at = tail.iter().position(|h| is(*h, "AECC_PROFILE_VIEW") || is(*h, "AECC_SAMPLE_LINE_GROUP")).unwrap_or(tail.len());
    let groups_at = tail.iter().position(|h| is(*h, "AECC_SAMPLE_LINE_GROUP")).unwrap_or(tail.len()).max(views_at);
    let profiles = tail[..views_at].iter().map(|h| Some(ProfileRef { handle: *h, id: profile_id(*h)? })).collect::<Option<Vec<_>>>()?;
    let views = tail[views_at..groups_at].to_vec();
    let groups = tail[groups_at..].to_vec();
    (views.iter().all(|h| is(*h, "AECC_PROFILE_VIEW")) && groups.iter().all(|h| is(*h, "AECC_SAMPLE_LINE_GROUP"))).then_some(())?;
    let d = a.decode();
    let name = d.strings.get(3)?.clone();
    let (data, start, end) = a.main_stream()?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    for _ in 0..6 {
        b.bit_long()?;
    }
    b.skip(6)?;
    let from = b.pos;
    let mut has_superelevation_key = false;
    for _ in 0..b.bits_value(8)? {
        has_superelevation_key |= crate::classes::civil::alignment::read_key(&mut b)?.0 == KEY_SUPERELEVATION as u32;
    }
    let keys: String = (from..b.pos).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    let keys_superelevation = if has_superelevation_key {
        keys.clone()
    } else {
        let count = u8::from_str_radix(&keys[..8], 2).ok()?.checked_add(1)?;
        format!("{count:08b}{}{}{}", &keys[8..], bl_bits(KEY_SUPERELEVATION), bl_bits(2))
    };
    b.skip(10 + 115 + 8)?;
    let high = b.bits_value(8)?;
    let edge = (i32::from(high & 0x3F) << 2) | i32::from(b.bits_value(2)?);
    let anchors = a.alignment_anchors()?;
    let kind = match a.alignment_type()? {
        crate::classes::civil::AlignmentType::Centerline => AlignmentKind::Centerline,
        crate::classes::civil::AlignmentType::Miscellaneous => AlignmentKind::Utility,
        crate::classes::civil::AlignmentType::Rail => AlignmentKind::Rail { gauge: a.alignment_track_gauge()? },
        // The writer writes an offset alignment's block in place of a type.
        crate::classes::civil::AlignmentType::Offset if offset.is_some() => AlignmentKind::Centerline,
        _ => return None,
    };
    let (equations, speeds) = a.alignment_timing_with_ids()?;
    let timing = Timing {
        equations: equations.iter().map(|(e, id)| (e.raw, *id, e.back, e.ahead, e.increasing)).collect(),
        // An offset alignment's own speed is written from its block.
        speeds: speeds.iter().filter(|(_, id)| offset.as_ref().is_none_or(|o| *id != o.own)).map(|(s, id)| (s.station, s.speed, *id)).collect(),
        superelevation,
        connected: None,
    };
    let w = Written { common: x.common.clone(), name, elements, style: refs[4].0, feature: refs[1].0, keys, keys_superelevation, edge, anchors, kind, timing, views, groups, offset };
    Some((w, profiles))
}

/// An offset alignment's block as the writer takes it (linear transitions
/// only; the own anchor is the alignment's start anchor).
fn offset_record(a: &crate::classes::arch::ArchEntityData, o: crate::classes::civil::OffsetParameters) -> Option<OffsetRecord> {
    o.transitions.iter().all(|t| t.kind == 1 && t.radii.is_empty()).then_some(())?;
    let own = a.alignment_anchors()?.into_iter().find(|x| x.kind == OWN_START_ANCHOR)?.id;
    let speed = a.alignment_timing_with_ids()?.1.iter().find(|(_, id)| *id == own).map_or(0.0, |(s, _)| s.speed);
    let (data, start_bit, end_bit) = a.main_stream()?;
    let mut b = super::super::bits::Bits::new(data, start_bit, end_bit);
    for _ in 0..5 {
        b.bit_long()?;
    }
    let associative = b.bit_long()? == 0x1002_5F04;
    let start = a.alignment_start_station()?;
    let end = o.regions.iter().map(|r| r.end).fold(start, f64::max);
    Some(OffsetRecord {
        parent: a.alignment_offset_parent()?,
        offset: o.offset,
        range: (start, end),
        regions: o.regions.iter().map(|r| OffsetRegionRecord { offset: r.offset, parent: (r.parent_start, r.parent_end), own: (r.start, r.end), widening: r.kind == 1, first: r.first_element, last: r.last_element }).collect(),
        transitions: o.transitions.iter().map(|t| (t.length, t.element)).collect(),
        map: o.element_map,
        anchors: o.anchors,
        own,
        speed,
        associative,
    })
}

/// How a stretch of an offset alignment follows its parent: the parent's
/// stations, its own, its offset at both ends, whether it is at the
/// nominal offset and whether it is a widening's transition.
#[derive(Clone, Copy, Debug)]
pub(super) struct StationRow {
    pub parent: (f64, f64),
    pub own: (f64, f64),
    pub offset: (f64, f64),
    pub nominal: bool,
    pub transition: bool,
}

/// The rows of an offset alignment's stations along its parent: one per
/// parent element and region, and one per transition between regions that
/// runs along the parent (a transition of length 0 follows no parent
/// stations, OW01), in parent station order; and the parent.
pub(super) fn offset_station_map(doc: &CadDocument, alignment: Handle) -> Option<(u64, Vec<StationRow>)> {
    let arch = |h: Handle| match doc.get_entity(h)? {
        crate::entities::EntityType::Extended(x) => match &x.data {
            crate::entities::ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_ALIGNMENT" => Some(a),
            _ => None,
        },
        _ => None,
    };
    let a = arch(alignment)?;
    let o = a.alignment_offset_parameters()?;
    (!o.regions.is_empty()).then_some(())?;
    let parent = a.alignment_offset_parent()?;
    let (parent_elements, own_elements) = (arch(Handle::new(parent))?.alignment_elements()?, a.alignment_elements()?);
    let (parent_spans, own_spans) = (spans(&parent_elements)?, spans(&own_elements)?);
    let length = |pieces: &[Piece]| pieces.iter().map(piece_length).sum::<f64>();
    let start = a.alignment_start_station()?;
    let several = o.regions.len() > 1;
    let mut out = Vec::new();
    for region in &o.regions {
        let tol = 1e-9 * region.end.abs().max(1.0);
        for (pid, ids) in &o.element_map {
            let (_, from, pieces) = parent_spans.iter().find(|(e, ..)| i32::from(e.id) == *pid)?;
            let (s0, s1) = (from.max(region.parent_start), (from + length(pieces)).min(region.parent_end));
            if s1 <= s0 + tol {
                continue;
            }
            let own: Vec<(f64, f64)> = ids
                .iter()
                .map(|id| own_spans.iter().find(|(e, ..)| i32::from(e.id) == *id).map(|(_, f, p)| (start + f, start + f + length(p))))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                // With widenings each region keeps the elements on its own
                // stations.
                .filter(|(a, b)| !several || (*b > region.start + tol && *a < region.end - tol))
                .collect();
            let (first, last) = (own.first()?, own.last()?);
            out.push(StationRow { parent: (s0, s1), own: (first.0, last.1), offset: (region.offset, region.offset), nominal: region.kind != 1, transition: false });
        }
    }
    for w in o.regions.windows(2) {
        if w[1].parent_start > w[0].parent_end + 1e-9 * w[0].parent_end.abs().max(1.0) {
            out.push(StationRow { parent: (w[0].parent_end, w[1].parent_start), own: (w[0].end, w[1].start), offset: (w[0].offset, w[1].offset), nominal: false, transition: true });
        }
    }
    out.sort_by(|a, b| a.parent.0.total_cmp(&b.parent.0));
    Some((parent, out))
}

/// Writes an alignment again with `profiles` and the profile views
/// `w.views`, after checking that writing it with the profiles and views
/// it has reproduces its record.
pub(super) fn rewrite(doc: &mut CadDocument, alignment: Handle, w: Written, before: &[ProfileRef], profiles: &[ProfileRef]) -> Result<(), DxfError> {
    rewrite_with_views(doc, alignment, w, before, profiles, None)
}

/// [`rewrite`], with `views` in place of the views it has when given.
pub(super) fn rewrite_with_views(doc: &mut CadDocument, alignment: Handle, w: Written, before: &[ProfileRef], profiles: &[ProfileRef], views: Option<&[u64]>) -> Result<(), DxfError> {
    let groups = w.groups.clone();
    rewrite_with(doc, alignment, w, before, profiles, views, &groups)
}

/// [`rewrite_with_views`], with the sample line groups `groups`.
pub(super) fn rewrite_with(doc: &mut CadDocument, alignment: Handle, w: Written, before: &[ProfileRef], profiles: &[ProfileRef], views: Option<&[u64]>, groups: &[u64]) -> Result<(), DxfError> {
    rewrite_adding(doc, alignment, w, before, profiles, views, groups, &[], &Timing::default())
}

/// [`rewrite_with`], adding the anchors `extra`.
#[allow(clippy::too_many_arguments)]
fn rewrite_adding(doc: &mut CadDocument, alignment: Handle, w: Written, before: &[ProfileRef], profiles: &[ProfileRef], views: Option<&[u64]>, groups: &[u64], extra: &[AlignmentAnchor], extra_timing: &Timing) -> Result<(), DxfError> {
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
    let handles = |list: &[ProfileRef]| list.iter().map(|p| p.handle).collect::<Vec<_>>();
    write_alignment_of(doc, w.common.clone(), &w.name, &w.elements, w.style, w.feature, w.edge, Some(&w.keys), &handles(before), &w.anchors, &w.views, &w.groups, w.offset.as_ref(), w.kind, &w.timing)?;
    if bits_of(doc) != old {
        *doc.get_entity_mut(alignment).ok_or_else(|| invalid("alignment not found"))? = original;
        return Err(invalid("the alignment holds data this crate does not write"));
    }
    // A new profile ties to the alignment's start by its identifier (a
    // profile sampled from a surface has none: a zero identifier).
    let mut anchors = w.anchors.clone();
    anchors.extend_from_slice(extra);
    let start = w.offset.as_ref().map_or(0.0, |o| o.range.0);
    for p in profiles.iter().filter(|p| p.id != [0; 16] && w.anchors.iter().all(|a| a.id != p.id)) {
        anchors.push(anchor_at(&w.elements, start, PROFILE_ANCHOR, p.id, start).ok_or_else(|| invalid("the alignment's start cannot be placed"))?);
    }
    anchors.sort_by(|a, b| a.id.cmp(&b.id));
    // The speeds stay in station order.
    let mut speeds = [&w.timing.speeds[..], &extra_timing.speeds[..]].concat();
    speeds.sort_by(|a, b| a.0.total_cmp(&b.0));
    // New superelevation curves replace the alignment's.
    let superelevation = if extra_timing.superelevation.is_empty() { w.timing.superelevation.clone() } else { extra_timing.superelevation.clone() };
    let keys = if superelevation.is_empty() { &w.keys } else { &w.keys_superelevation };
    let timing = Timing { equations: [&w.timing.equations[..], &extra_timing.equations[..]].concat(), speeds, superelevation, connected: None };
    write_alignment_of(doc, w.common.clone(), &w.name, &w.elements, w.style, w.feature, w.edge, Some(keys), &handles(profiles), &anchors, views.unwrap_or(&w.views), groups, w.offset.as_ref(), w.kind, &timing)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;
    use crate::classes::civil::SpiralShape;

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

    #[test]
    fn offset_alignments_follow_their_parent() {
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 50.0)];
        let parent = add_alignment_with_curves(&mut doc, "AL1", &v, &[VertexCurve::Arc(169.4427190999918)], None).unwrap();
        assert!(add_offset_alignment(&mut doc, parent, "X", 0.0, None, None).is_err());
        assert!(add_offset_alignment(&mut doc, parent, "X", 10.0, Some((50.0, 400.0)), None).is_err());
        assert!(add_offset_alignment(&mut doc, parent, "X", -200.0, None, None).is_err());
        let off = add_offset_alignment(&mut doc, parent, "OFF1", 10.0, Some((30.0, 150.0)), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-alignment-offset-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let arch = |h: Handle| match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let (a, o) = (arch(parent), arch(off));
        assert_eq!(o.alignment_type(), Some(crate::classes::civil::AlignmentType::Offset));
        let p = o.alignment_offset_parameters().unwrap();
        assert_eq!((p.offset, p.left, p.regions[0].parent_start, p.regions[0].parent_end), (10.0, false, 30.0, 150.0));
        assert_eq!((p.regions[0].first_element, p.regions[0].last_element), (7, 9));
        assert_eq!(p.element_map, vec![(1, vec![7]), (2, vec![9]), (3, vec![8])]);
        // The parent holds an anchor at each end of the range, by the
        // identifiers the offset alignment keeps.
        let anchors = a.alignment_anchors().unwrap();
        assert_eq!(anchors.len(), 2);
        for (station, id) in &p.anchors {
            let at = anchors.iter().find(|x| x.id == *id).unwrap();
            assert_eq!((at.kind, at.station), (6, *station));
        }
        let start = anchors.iter().find(|x| x.station == 30.0).unwrap();
        assert_eq!((start.element, start.previous, start.next, start.along), (1, 0, 3, 30.0));
        let own = o.alignment_anchors().unwrap();
        assert_eq!((own.len(), own[0].kind, own[0].element), (1, 2, 7));
        match o.alignment_elements().unwrap()[0].kind {
            AlignmentElementKind::FixedLine { start, .. } => assert!(start.distance(&Vector2::new(30.0, -10.0)) < 1e-9),
            _ => panic!(),
        }
        assert_eq!(o.alignment_start_station(), Some(30.0));
    }

    #[test]
    fn offset_alignments_keep_their_widenings() {
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 50.0)];
        let parent = add_alignment_with_curves(&mut doc, "AL1", &v, &[VertexCurve::Arc(169.4427190999918)], None).unwrap();
        let w = |start: f64, end: f64, entry: f64, exit: f64| Widening { start, end, offset: 15.0, entry, exit };
        // Transitions longer than 0 must lie on one parent line; widenings
        // keep the offset's side.
        assert!(add_offset_alignment_with_widenings(&mut doc, parent, "X", 10.0, None, &[w(50.0, 90.0, 20.0, 0.0)], None).is_err());
        assert!(add_offset_alignment_with_widenings(&mut doc, parent, "X", -10.0, None, &[w(20.0, 50.0, 0.0, 0.0)], None).is_err());
        let off = add_offset_alignment_with_widenings(&mut doc, parent, "OFF1", 10.0, None, &[w(20.0, 50.0, 10.0, 10.0)], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-alignment-widening-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let o = match back.get_entity(off) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let p = o.alignment_offset_parameters().unwrap();
        let regions: Vec<(f64, f64, f64, i32)> = p.regions.iter().map(|r| (r.offset, r.parent_start, r.parent_end, r.kind)).collect();
        assert_eq!(regions[..2], [(10.0, 0.0, 20.0, 2), (15.0, 30.0, 50.0, 1)]);
        assert_eq!((regions[2].1, regions[2].3), (60.0, 2));
        let transitions: Vec<(i32, f64, i32)> = p.transitions.iter().map(|t| (t.kind, t.length, t.element)).collect();
        assert_eq!(transitions, vec![(1, 10.0, 11), (1, 10.0, 12)]);
        assert_eq!(p.element_map[0], (1, vec![7, 10]));
        assert_eq!(p.anchors.iter().map(|a| a.0).collect::<Vec<_>>()[..5], [0.0, 20.0, 30.0, 50.0, 60.0]);
        let ramp = o.alignment_elements().unwrap().into_iter().find(|e| e.id == 11).unwrap();
        match ramp.kind {
            AlignmentElementKind::Points { points } => {
                assert!(points[0].distance(&Vector2::new(20.0, -10.0)) < 1e-9 && points[1].distance(&Vector2::new(30.0, -15.0)) < 1e-9);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn utility_and_rail_alignments_keep_their_type() {
        use crate::classes::civil::AlignmentType;
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)];
        assert!(add_alignment_of_kind(&mut doc, "X", &v, &[], AlignmentKind::Rail { gauge: 0.0 }, None).is_err());
        let u = add_alignment_of_kind(&mut doc, "U", &v, &[], AlignmentKind::Utility, None).unwrap();
        let r = add_alignment_of_kind(&mut doc, "R", &v, &[], AlignmentKind::Rail { gauge: 1.435 }, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-alignment-types-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let read = |h: Handle| match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) => (a.alignment_type(), a.alignment_track_gauge()),
                _ => panic!(),
            },
            _ => panic!(),
        };
        assert_eq!(read(u), (Some(AlignmentType::Miscellaneous), Some(0.0)));
        assert_eq!(read(r), (Some(AlignmentType::Rail), Some(1.435)));
    }

    #[test]
    fn design_speeds_and_station_equations_read_back() {
        use crate::classes::civil::{DesignSpeed, StationEquation};
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(200.0, 0.0)];
        let a = add_alignment_of_kind(&mut doc, "A", &v, &[], AlignmentKind::Centerline, None).unwrap();
        add_design_speeds(&mut doc, a, &[(60.0, 30.0), (0.0, 50.0)]).unwrap();
        add_station_equations(&mut doc, a, &[(80.0, 1000.0, true)]).unwrap();
        add_station_equations(&mut doc, a, &[(150.0, 2000.0, false)]).unwrap();
        assert!(add_station_equations(&mut doc, a, &[(100.0, 0.0, true)]).is_err());
        let path = std::env::temp_dir().join(format!("civil-alignment-timing-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let Some(crate::entities::EntityType::Extended(x)) = back.get_entity(a) else { panic!() };
        let crate::entities::ExtendedEntityData::Arch(r) = &x.data else { panic!() };
        assert_eq!(r.alignment_design_speeds(), [DesignSpeed { station: 0.0, speed: 50.0 }, DesignSpeed { station: 60.0, speed: 30.0 }]);
        assert_eq!(r.alignment_station_equations(), [
            StationEquation { raw: 80.0, back: 80.0, ahead: 1000.0, increasing: true },
            StationEquation { raw: 150.0, back: 1070.0, ahead: 2000.0, increasing: false },
        ]);
        let mut kinds: Vec<i32> = r.alignment_anchors().unwrap().iter().map(|a| a.kind).collect();
        kinds.sort();
        assert_eq!(kinds, [2, 2, 3, 3]);
    }

    #[test]
    fn superelevation_curves_read_back() {
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 60.0), Vector2::new(300.0, 0.0)];
        let a = add_alignment_with_curves(&mut doc, "A", &v, &[VertexCurve::Arc(80.0), VertexCurve::Arc(80.0)], None).unwrap();
        let mut slopes = [0.0; 8];
        slopes[3] = -0.06;
        let station = |kind, station, ending| CriticalStation { kind, station, ending, slopes };
        // Element 1 is a line.
        assert!(add_superelevation(&mut doc, a, &[SuperelevationCurve { element: 1, stations: vec![station(CriticalStationKind::LevelCrown, 50.0, false)] }]).is_err());
        add_superelevation(&mut doc, a, &[
            SuperelevationCurve { element: 5, stations: vec![station(CriticalStationKind::BeginFullSuper, 180.0, false), station(CriticalStationKind::EndFullSuper, 240.0, true)] },
            SuperelevationCurve { element: 4, stations: vec![station(CriticalStationKind::ReverseCrown, 90.0, false)] },
        ])
        .unwrap();
        // The alignment is written again with them.
        add_design_speeds(&mut doc, a, &[(0.0, 50.0)]).unwrap();
        let path = std::env::temp_dir().join(format!("civil-superelevation-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let Some(crate::entities::EntityType::Extended(x)) = back.get_entity(a) else { panic!() };
        let crate::entities::ExtendedEntityData::Arch(r) = &x.data else { panic!() };
        let curves = r.alignment_superelevation().unwrap();
        assert_eq!(curves.iter().map(|c| (c.element, c.stations.len())).collect::<Vec<_>>(), [(4, 1), (5, 2)]);
        let s = curves[1].stations[1];
        assert_eq!((s.kind, s.ending, s.slopes[3]), (4, true, -0.06));
        assert!(s.offset < 0.0 && curves[1].stations[0].offset > 0.0);
        assert_eq!(r.alignment_design_speeds().len(), 1);
    }

    #[test]
    fn manual_stations_bring_the_curve_settings() {
        let mut doc = drawing();
        let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 50.0)];
        let a = add_alignment_with_curves(&mut doc, "A", &v, &[VertexCurve::Arc(169.4427190999918)], None).unwrap();
        let station = |kind, station| CriticalStation { kind, station, ending: false, slopes: [0.0; 8] };
        add_superelevation(&mut doc, a, &[SuperelevationCurve { element: 3, stations: vec![station(CriticalStationKind::BeginFullSuper, 70.0), station(CriticalStationKind::Manual, 75.0), station(CriticalStationKind::Manual, 80.0)] }]).unwrap();
        let path = std::env::temp_dir().join(format!("civil-manual-stations-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let Some(crate::entities::EntityType::Extended(x)) = back.get_entity(a) else { panic!() };
        let crate::entities::ExtendedEntityData::Arch(r) = &x.data else { panic!() };
        let kinds: Vec<i32> = r.alignment_superelevation().unwrap()[0].stations.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, [3, MANUAL_STATION, MANUAL_STATION]);
        // Each unit of a manual station is described, numbered within the
        // curve.
        let texts = r.decode().strings;
        assert_eq!(texts.iter().filter(|t| *t == "Manual station").count(), 9);
        assert_eq!(texts.iter().filter(|t| *t == "Manual station(2)").count(), 9);
    }

    #[test]
    fn curb_returns_join_their_roads() {
        let mut doc = drawing();
        let v = Vector2::new;
        let a = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        let b = add_alignment(&mut doc, "AL2", &[v(100.0, -100.0), v(100.0, 100.0)], None).unwrap();
        let c = add_alignment(&mut doc, "AL3", &[v(0.0, 50.0), v(200.0, 50.0)], None).unwrap();
        assert!(add_curb_return(&mut doc, "X", a, -10.0, c, -10.0, true, 10.0, (5.0, 5.0), None).is_err());
        // A left turn from AL1 into AL2, then a right turn into AL2 run
        // against its stationing.
        let left = add_curb_return(&mut doc, "CR1", a, -10.0, b, -10.0, true, 10.0, (5.0, 5.0), None).unwrap();
        let right = add_curb_return(&mut doc, "CR2", a, 10.0, b, 10.0, false, 10.0, (5.0, 5.0), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-curb-returns-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let read = |h: Handle| match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(r) => r.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        for (h, start, end) in [(left, v(75.0, 10.0), v(90.0, 25.0)), (right, v(95.0, -10.0), v(110.0, -25.0))] {
            let r = read(h);
            assert_eq!(r.alignment_type(), Some(crate::classes::civil::AlignmentType::CurbReturn));
            assert_eq!(r.alignment_connected_inputs(), Some([a.value(), b.value()]));
            let e = r.alignment_elements().unwrap();
            assert!(matches!(e[1].kind, AlignmentElementKind::FreeArc { radius } if radius == 10.0));
            let AlignmentElementKind::FixedLine { start: s, .. } = e[0].kind else { panic!() };
            let AlignmentElementKind::FixedLine { end: t, .. } = e[2].kind else { panic!() };
            assert!(s.distance(&start) < 1e-9 && t.distance(&end) < 1e-9, "{s:?} {t:?}");
        }
    }

    /// Three arcs and a spiral-curve-spiral between AL1 and AL2 (CG01,
    /// CG02): the lines start where the samples' do.
    #[test]
    fn curb_returns_of_other_curve_groups() {
        let mut doc = drawing();
        let v = Vector2::new;
        let a = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        let b = add_alignment(&mut doc, "AL2", &[v(100.0, -100.0), v(100.0, 100.0)], None).unwrap();
        let ccc = CurbReturnCurve::ThreeCentered { first: (30.0, 5.0), middle: 10.0, last: (30.0, 5.0) };
        let scs = |shape| CurbReturnCurve::SpiralCurveSpiral { spiral_in: 6.0, radius: 10.0, spiral_out: 6.0, shape };
        let h1 = add_curb_return_with(&mut doc, "CR1", a, -10.0, b, -10.0, true, ccc, (5.0, 5.0), None).unwrap();
        let h2 = add_curb_return_with(&mut doc, "CR2", a, -10.0, b, -10.0, true, scs(SpiralShape::Clothoid), (5.0, 5.0), None).unwrap();
        // The other shapes start their spiral in where the samples' do
        // (CG09, CS01).
        let shapes = [
            (SpiralShape::Bloss, 76.9144711223893),
            (SpiralShape::SineHalfWave, 76.78966791895277),
            (SpiralShape::JapaneseCubic, 76.72821309623592),
            (SpiralShape::CubicParabola, 76.7033871330615),
            (SpiralShape::Sinusoidal, 76.94358128539841),
            (SpiralShape::Biquadratic, 76.92846040778008),
        ];
        let others: Vec<_> = shapes.iter().enumerate().map(|(i, (shape, _))| add_curb_return_with(&mut doc, &format!("S{i}"), a, -10.0, b, -10.0, true, scs(*shape), (5.0, 5.0), None).unwrap()).collect();
        let path = std::env::temp_dir().join(format!("civil-curb-groups-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let read = |h: Handle| match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(r) => r.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let e = read(h1).alignment_elements().unwrap();
        assert!(matches!(e[1].kind, AlignmentElementKind::ThreeCenteredArc { arcs } if arcs[1] == (10.0, 0.0)));
        let AlignmentElementKind::FixedLine { end, .. } = e[0].kind else { panic!() };
        assert!(end.distance(&v(76.4049419773902, 10.0)) < 1e-9);
        let r = read(h2);
        assert_eq!(r.decode().strings[3].lines().next(), Some("{"));
        let e = r.alignment_elements().unwrap();
        assert_eq!(e.iter().map(|e| e.id).collect::<Vec<_>>(), [1, 3, 4, 5, 6]);
        let AlignmentElementKind::FixedLine { end, .. } = e[0].kind else { panic!() };
        assert!(end.distance(&v(76.85945875683416, 10.0)) < 1e-9);
        // A sine half-wave spiral past the length where its X stops growing
        // (about 3.83 R) is refused.
        assert!(add_curb_return_with(&mut doc, "SW", a, -10.0, b, -10.0, true, CurbReturnCurve::SpiralCurveSpiral { spiral_in: 40.0, radius: 10.0, spiral_out: 6.0, shape: SpiralShape::SineHalfWave }, (5.0, 5.0), None).is_err());
        for (h, (shape, x)) in others.into_iter().zip(shapes) {
            let r = read(h);
            assert!(r.decode().strings[3].contains(&format!("\"SpiralType\": {}", shape.code())));
            let e = r.alignment_elements().unwrap();
            let AlignmentElementKind::SpiralIn { start, shape: read_shape, .. } = e[2].kind else { panic!() };
            assert_eq!(read_shape, shape);
            // The samples' Japanese cubic X solves its equation only to
            // 1e-4; the writer solves it exactly.
            let tolerance = if shape == SpiralShape::JapaneseCubic { 1e-4 } else { 1e-6 };
            assert!((start.x - x).abs() < tolerance, "{shape:?} {}", start.x - x);
        }
    }
}
