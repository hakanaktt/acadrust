//! TIN surfaces built from points (`AECC_TIN_SURFACE`).

use super::support::*;
use super::tree::{ensure_surface_tree, KEY_SURFACE_STYLES};
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::classes::civil::tin::AUTO_REBUILD;
use crate::classes::civil::{TinBreaklineType, TinDefinition, TinOperation};
use crate::types::{Handle, Vector2, Vector3};
use crate::CadDocument;

/// How far the four bounding corners lie outside the points' extents.
const CORNER_MARGIN: f64 = 100.0;
/// Flags of a bounding corner and of a surface point.
const CORNER: u32 = 0x8440;
const POINT: u32 = 0x80;

/// A triangulation as the surface stores it: the four bounding corners
/// first, then the points; counter-clockwise triangles with the triangle
/// across the edge that starts at each vertex (-1 on the outside), the
/// triangles that use a corner hidden.
pub(super) struct Triangulation {
    pub vertices: Vec<Vector3>,
    pub triangles: Vec<[usize; 3]>,
}

/// `true` when `p` is inside the circle through the counter-clockwise
/// triangle `a, b, c`.
fn in_circle(a: Vector3, b: Vector3, c: Vector3, p: Vector3) -> bool {
    let (ax, ay, bx, by, cx, cy) = (a.x - p.x, a.y - p.y, b.x - p.x, b.y - p.y, c.x - p.x, c.y - p.y);
    let det = (ax * ax + ay * ay) * (bx * cy - cx * by) - (bx * bx + by * by) * (ax * cy - cx * ay) + (cx * cx + cy * cy) * (ax * by - bx * ay);
    det > 0.0
}

/// Delaunay triangulation of `points` (distinct in plan) inside a rectangle
/// of four corners `CORNER_MARGIN` outside the extents of `extent`, which
/// holds `points` (Bowyer-Watson).
// ponytail: quadratic insertion (every triangle is tested per point); a
// spatial index when surfaces of many thousand points are written.
pub(super) fn triangulate_in(extent: &[Vector3], points: &[Vector3]) -> Triangulation {
    let (mut lo, mut hi) = (extent[0], extent[0]);
    for p in extent {
        lo = Vector3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vector3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let m = CORNER_MARGIN;
    let corner = |x: f64, y: f64| {
        // A corner's elevation is the nearest point's.
        let near = extent.iter().min_by(|a, b| ((a.x - x).powi(2) + (a.y - y).powi(2)).total_cmp(&((b.x - x).powi(2) + (b.y - y).powi(2)))).unwrap();
        Vector3::new(x, y, near.z)
    };
    let mut vertices = vec![corner(lo.x - m, lo.y - m), corner(hi.x + m, lo.y - m), corner(hi.x + m, hi.y + m), corner(lo.x - m, hi.y + m)];
    let mut triangles: Vec<[usize; 3]> = vec![[0, 1, 2], [0, 2, 3]];
    for p in points {
        let i = vertices.len();
        vertices.push(*p);
        let bad: Vec<usize> = (0..triangles.len()).filter(|t| {
            let [a, b, c] = triangles[*t];
            in_circle(vertices[a], vertices[b], vertices[c], *p)
        }).collect();
        // The cavity's boundary: edges of exactly one bad triangle.
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for t in &bad {
            let [a, b, c] = triangles[*t];
            for e in [(a, b), (b, c), (c, a)] {
                if let Some(k) = edges.iter().position(|f| *f == (e.1, e.0)) {
                    edges.remove(k);
                } else {
                    edges.push(e);
                }
            }
        }
        for t in bad.iter().rev() {
            triangles.swap_remove(*t);
        }
        triangles.extend(edges.into_iter().map(|(a, b)| [a, b, i]));
    }
    Triangulation { vertices, triangles }
}

impl Triangulation {
    /// The triangulation of a surface with no data: the four bounding
    /// corners at the drawing extremes, each with no incident triangle, and
    /// no triangles — the record the application writes for a surface it
    /// has not built (the "EG" shell of the tutorials' `Surface-2.dwg`).
    fn empty() -> Triangulation {
        let m = f64::MAX;
        Triangulation {
            vertices: vec![
                Vector3::new(m, m, m),
                Vector3::new(-m, m, m),
                Vector3::new(-m, -m, -m),
                Vector3::new(m, -m, -m),
            ],
            triangles: Vec::new(),
        }
    }

    /// The triangle across the edge that starts at each vertex.
    fn neighbors(&self) -> Vec<[i32; 3]> {
        let mut by_edge = std::collections::HashMap::new();
        for (t, tri) in self.triangles.iter().enumerate() {
            for k in 0..3 {
                by_edge.insert((tri[k], tri[(k + 1) % 3]), t as i32);
            }
        }
        self.triangles.iter().map(|tri| [0, 1, 2].map(|k| *by_edge.get(&(tri[(k + 1) % 3], tri[k])).unwrap_or(&-1))).collect()
    }
}

/// The drawing's default surface style: the first style of the style
/// tree's surface style collection.
pub(super) fn default_style(doc: &CadDocument) -> Option<u64> {
    let tree = read_node(doc, root_node(doc, 31)?)?;
    let head = tree.children.iter().find(|c| c.0 == KEY_SURFACE_STYLES)?.1;
    object(doc, Handle::new(head))?.decode().handles.first().copied().filter(|h| *h != 0)
}

/// A 16-byte identifier, unique per call.
fn new_guid(seed: u64) -> [u8; 16] {
    use std::hash::{BuildHasher, Hasher};
    let half = |salt: u64| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(seed ^ salt);
        h.finish().to_le_bytes()
    };
    let (a, b) = (half(1), half(2));
    let mut g = [0; 16];
    g[..8].copy_from_slice(&a);
    g[8..].copy_from_slice(&b);
    g
}

/// The definitions block (see `tin::read_definitions`): head (10 bits
/// 8 + 32 x count, RC 2 + 8 x count, RC 4 with a boundary else 2, RC 0x81
/// with contours else 0x80), 2 bits (00 with contours), BL 0, BS 0x800C
/// and B holding the breakline count + 1, the definitions, BL 0 (four-byte
/// form), BL options, 2 bits.
fn write_definitions(w: &mut DwgMergedWriter, definitions: &[TinDefinition], auto_rebuild: bool, seed: u64) {
    let n = definitions.len() as u32;
    let contours = definitions.iter().any(|d| matches!(d.operation, TinOperation::Contours(_)));
    let boundary = definitions.iter().any(|d| matches!(d.operation, TinOperation::Boundary { .. }));
    let breaklines = definitions.iter().filter(|d| matches!(d.operation, TinOperation::Breaklines(..))).count() as u16 + 1;
    bits(w, &format!("{:010b}", 8 + 32 * n));
    w.write_bytes(&[(2 + 8 * n) as u8, if boundary { 4 } else { 2 }, if contours { 0x81 } else { 0x80 }]);
    bits(w, if contours { "00" } else { "10" });
    w.write_bit_long(0);
    // The BS in its 16-bit form.
    w.write_2bits(0);
    w.write_bytes(&(0x800C_u16 | ((breaklines >> 1) << 8)).to_le_bytes());
    w.write_bit(breaklines & 1 == 1);
    for (k, d) in definitions.iter().enumerate() {
        write_operation(w, &d.operation, new_guid(seed ^ ((k as u64 + 1) << 32)));
    }
    w.write_2bits(0);
    w.write_raw_long(0);
    w.write_bit_long(0xAC15_0A00_u32 as i32 | if auto_rebuild { AUTO_REBUILD } else { 0 });
    bits(w, "00");
}

fn write_points(w: &mut DwgMergedWriter, points: &[Vector3]) {
    w.write_bit_long(points.len() as i32);
    for p in points {
        w.write_bit_double(p.x);
        w.write_bit_double(p.y);
        w.write_bit_double(p.z);
    }
}

/// `BL kind, BL flags (9; 4 from objects, 2 proximity), BL 16, GUID, BL k`
/// and the kind's data (see `tin::read_operation`).
fn write_operation(w: &mut DwgMergedWriter, operation: &TinOperation, guid: [u8; 16]) {
    let lines = match operation {
        TinOperation::Breaklines(_, l) | TinOperation::Contours(l) | TinOperation::Boundary { lines: l, .. } => Some(l),
        _ => None,
    };
    let kind = match operation {
        TinOperation::AddPoints(_) => 47,
        TinOperation::PointGroup => 0,
        TinOperation::Breaklines(t, _) => *t as i32,
        TinOperation::Contours(_) => 27,
        TinOperation::Boundary { .. } => 13,
        TinOperation::AddVertex(_) => 1,
        TinOperation::DeleteVertex(_) => 2,
        TinOperation::MoveVertex { .. } => 8,
        TinOperation::SwapEdge(_) => 9,
        TinOperation::Raise(_) => 17,
        TinOperation::VertexElevation(_) => 20,
        TinOperation::AddLine(..) => 22,
        TinOperation::DeleteLine(_) => 23,
        TinOperation::Import { .. } => 33,
        TinOperation::Transform(_) => 34,
        TinOperation::Polygon(_) => 35,
        TinOperation::Kind40(_) => 40,
        TinOperation::PasteSurface => 12,
        TinOperation::DrawingObjects { .. } => 31,
        TinOperation::PointFile { .. } => 30,
        TinOperation::CorridorSurface(_) => 41,
        TinOperation::TinFile { .. } => 5,
        TinOperation::MinimizeFlatAreas { .. } => 25,
        TinOperation::GradingGroup => 29,
        TinOperation::Simplify { .. } => 43,
    };
    let k = lines.map_or(0, |l| l.objects as i32 + l.polylines.len() as i32);
    let from_objects = lines.is_some_and(|l| l.objects > 0);
    let proximity = matches!(operation, TinOperation::Breaklines(TinBreaklineType::Proximity, _));
    w.write_bit_long(kind);
    w.write_bit_long(9 | if from_objects { 4 } else { 0 } | if proximity { 2 } else { 0 });
    w.write_bit_long(16);
    w.write_bytes(&guid);
    w.write_bit_long(k);
    if let Some(l) = lines {
        w.write_bit_long(l.objects as i32);
        w.write_bit_double(l.mid_ordinate);
        w.write_bit_long(l.polylines.len() as i32);
        for p in &l.polylines {
            write_points(w, p);
        }
        w.write_bit_long(k);
        for e in 1..=k {
            w.write_bit_long(e);
        }
        w.write_bit_double(l.maximum_distance);
        w.write_bit_double(l.weeding_distance);
        w.write_bit_double(l.weeding_angle);
    }
    let v2 = |w: &mut DwgMergedWriter, v: &Vector2| {
        w.write_bit_double(v.x);
        w.write_bit_double(v.y);
    };
    match operation {
        TinOperation::AddPoints(p) => write_points(w, p),
        TinOperation::Contours(_) => w.write_bit_long(0),
        TinOperation::Boundary { boundary_type, non_destructive, .. } => {
            w.write_bit_long(*boundary_type as i32);
            w.write_bit_long(i32::from(*non_destructive));
        }
        TinOperation::AddVertex(p) | TinOperation::VertexElevation(p) => {
            w.write_bit_double(p.x);
            w.write_bit_double(p.y);
            w.write_bit_double(p.z);
        }
        TinOperation::DeleteVertex(p) | TinOperation::SwapEdge(p) | TinOperation::DeleteLine(p) => v2(w, p),
        TinOperation::MoveVertex { from, to } => {
            v2(w, from);
            v2(w, to);
        }
        TinOperation::AddLine(a, b) => {
            v2(w, a);
            v2(w, b);
        }
        TinOperation::Raise(d) => w.write_bit_double(*d),
        TinOperation::Import { file_time, value } => {
            w.write_bit_long((file_time >> 32) as i32);
            w.write_bit_long(*file_time as i32);
            w.write_bit_long(*value);
        }
        TinOperation::PointFile { file_time, options } => {
            w.write_bit_long((file_time >> 32) as i32);
            w.write_bit_long(*file_time as i32);
            bits(w, &format!("{:03b}", options & 7));
        }
        TinOperation::Polygon(p) => {
            w.write_bit_long(p.len() as i32);
            for v in p {
                w.write_raw_double(v.x);
                w.write_raw_double(v.y);
            }
        }
        TinOperation::Transform(m) => {
            for v in m {
                w.write_raw_double(*v);
            }
            w.write_bit_long(0);
        }
        TinOperation::Kind40(v) => w.write_bit_long(*v),
        TinOperation::DrawingObjects { object_type, maintain_edges, points } => {
            w.write_bit_long(*object_type);
            w.write_bit_long(i32::from(*maintain_edges));
            write_points(w, points);
        }
        TinOperation::CorridorSurface(id) => w.write_bytes(id),
        TinOperation::TinFile { file_time } => {
            w.write_bit_long((file_time >> 32) as i32);
            w.write_bit_long(*file_time as i32);
        }
        TinOperation::MinimizeFlatAreas { counts, options } => {
            w.write_bit_long(counts[0]);
            w.write_bit_long(counts[1]);
            w.write_bit_long(*options);
        }
        TinOperation::Simplify { method, options, percentage, elevation_change, region, removed } => {
            w.write_bit_long(*method);
            w.write_bit_long(*options);
            w.write_bit_double(*percentage);
            w.write_bit_double(*elevation_change);
            w.write_bit_long(*region);
            w.write_bit_long(*removed);
        }
        TinOperation::PointGroup | TinOperation::PasteSurface | TinOperation::GradingGroup | TinOperation::Breaklines(..) => {}
    }
}

/// The surface record:
///
/// ```text
/// BL 80, 2 bits 11, BL 0x1554C738, BL 0x087C1120, BL 5 (version)
/// BL 8 keys: 0x03011E29 RC 1, 0x0D011E25 BL 0, 0x02011E26 BD DBL_MAX,
///   0x0B001081 BL 2, BL 0, BL 0 (geometry stamp), 0x03001093 RC 0,
///   0x0201230C BD 0, 0x02011E27 BD DBL_MAX, 0x01011E28 BL 8
/// BL 0 (byte form), 121 zero bits, 3BD origin 0, 9 BD identity, BD 1
/// RC 1 item, 5 bits 01000
/// the definitions (`write_definitions`)
/// RL -1, RS 0x04A2, RC 2, RS 0, RS 0x2A3C, "MMS01"
/// triangulation: BL vertex count, BL 0, 1, 2, 3 (the corners),
///   per vertex RL flags (corner 0x8440, point 0x80), RL a triangle using
///   it, 3 RD; BL triangle count, per triangle 3 RL vertices, 3 RL
///   neighbours, RL flags (1 hidden), RL -1
/// BL 0, BL 0 (1 with a snapshot: "MMS01", the snapshot's triangulation
///   as above, BL 0), BL 0, B 0, BS 0x04BE
/// references: soft pointer 0, hard owner 0, the definitions' sources,
///   hard pointer 0, style
/// texts: "", "", the definitions' names, name, "", ""
/// ```
#[allow(clippy::too_many_arguments)]
pub(super) fn write_surface(doc: &mut CadDocument, common: EntityCommon, name: &str, definitions: &[TinDefinition], auto_rebuild: bool, tin: &Triangulation, hidden: &[bool], snapshot: Option<(&Triangulation, &[bool])>, style: u64) -> Result<Handle, DxfError> {
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let seed = common.handle.value() ^ tin.vertices.len() as u64;
    let triangulation = |w: &mut DwgMergedWriter, tin: &Triangulation, hidden: &[bool]| {
        let neighbors = tin.neighbors();
        // A vertex of a shell surface has no incident triangle (-1); on a
        // built surface every vertex lies on one.
        let mut incident = vec![-1i32; tin.vertices.len()];
        for (t, tri) in tin.triangles.iter().enumerate() {
            for v in tri {
                incident[*v] = t as i32;
            }
        }
        // A built surface's corners carry the hidden-triangle bits (0x8440);
        // the shell stores the bare corner flag (0x40), the app's form.
        let corner = if tin.triangles.is_empty() { 0x40 } else { CORNER };
        w.write_bit_long(tin.vertices.len() as i32);
        for c in 0..4 {
            w.write_bit_long(c);
        }
        for (i, v) in tin.vertices.iter().enumerate() {
            w.write_raw_long((if i < 4 { corner } else { POINT }) as i32);
            w.write_raw_long(incident[i]);
            w.write_raw_double(v.x);
            w.write_raw_double(v.y);
            w.write_raw_double(v.z);
        }
        w.write_bit_long(tin.triangles.len() as i32);
        for ((tri, n), h) in tin.triangles.iter().zip(&neighbors).zip(hidden) {
            for v in tri {
                w.write_raw_long(*v as i32);
            }
            for t in n {
                w.write_raw_long(*t);
            }
            w.write_raw_long(i32::from(*h));
            w.write_raw_long(-1);
        }
    };
    let mut refs = vec![soft_pointer(0), Ref(crate::io::dwg::dwg_reference_type::DwgReferenceType::HardOwnership, 0)];
    refs.extend(definitions.iter().flat_map(|d| d.sources.iter().map(|h| hard_pointer(*h))));
    refs.extend([hard_pointer(0), hard_pointer(style)]);
    let mut strings = vec![String::new(), String::new()];
    strings.extend(definitions.iter().flat_map(|d| d.texts.iter().cloned()));
    strings.extend([name.to_string(), String::new(), String::new()]);
    put_entity(
        doc,
        "AECC_TIN_SURFACE",
        common,
        |w| {
            w.write_bit_long(80);
            bits(w, "11");
            for v in [0x1554_C738, 0x087C_1120, 5, 8] {
                w.write_bit_long(v);
            }
            w.write_bit_long(0x0301_1E29);
            byte(w, 1);
            w.write_bit_long(0x0D01_1E25);
            w.write_bit_long(0);
            w.write_bit_long(0x0201_1E26);
            w.write_bit_double(f64::MAX);
            for v in [0x0B00_1081, 2, 0, 0, 0x0300_1093] {
                w.write_bit_long(v);
            }
            byte(w, 0);
            w.write_bit_long(0x0201_230C);
            w.write_bit_double(0.0);
            w.write_bit_long(0x0201_1E27);
            w.write_bit_double(f64::MAX);
            w.write_bit_long(0x0101_1E28);
            w.write_bit_long(8);
            bl_byte(w, 0);
            bits(w, &"0".repeat(121));
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0] {
                w.write_bit_double(v);
            }
            byte(w, 1);
            bits(w, "01000");
            write_definitions(w, definitions, auto_rebuild, seed);
            w.write_raw_long(-1);
            w.write_bytes(&[0xA2, 0x04, 2, 0, 0, 0x3C, 0x2A]);
            w.write_bytes(b"MMS01");
            triangulation(w, tin, hidden);
            w.write_bit_long(0);
            match snapshot {
                Some((s, h)) => {
                    w.write_bit_long(1);
                    w.write_bytes(b"MMS01");
                    triangulation(w, s, h);
                    w.write_bit_long(0);
                }
                None => w.write_bit_long(0),
            }
            bits(w, "100");
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// The geometry a definition's sources bring in, for the triangulation:
/// a point group's points (from the drawing's COGO points), the vertices of
/// the lines, 2D and 3D polylines lines come from. Pasted surfaces,
/// point files and corridor surfaces bring none (the application adds
/// them when it rebuilds the surface).
fn with_source_geometry(doc: &CadDocument, definitions: &[TinDefinition]) -> Result<Vec<TinDefinition>, DxfError> {
    use crate::entities::EntityType;
    let shape = |h: u64| -> Result<Vec<Vector3>, DxfError> {
        match doc.get_entity(Handle::new(h)) {
            Some(EntityType::Line(l)) => Ok(vec![l.start, l.end]),
            Some(EntityType::LwPolyline(p)) => {
                let mut v: Vec<Vector3> = p.vertices.iter().map(|v| Vector3::new(v.location.x, v.location.y, p.elevation)).collect();
                if p.is_closed {
                    v.extend(v.first().copied());
                }
                Ok(v)
            }
            Some(EntityType::Polyline3D(p)) => {
                let mut v: Vec<Vector3> = p.vertices.iter().map(|v| v.position).collect();
                if p.is_closed() {
                    v.extend(v.first().copied());
                }
                Ok(v)
            }
            _ => Err(invalid("a line source is not a line, a polyline or a 3D polyline")),
        }
    };
    let points = super::points::cogo_points(doc);
    definitions
        .iter()
        .map(|d| {
            let mut d = d.clone();
            match &mut d.operation {
                TinOperation::PointGroup => {
                    let numbers = object(doc, Handle::new(d.sources[0])).map(|g| g.point_group_numbers()).ok_or_else(|| invalid("a point group source is not a point group"))?;
                    d.operation = TinOperation::AddPoints(numbers.iter().filter_map(|n| points.iter().find(|p| p.number == *n)).map(|p| p.position).collect());
                }
                TinOperation::Breaklines(_, l) | TinOperation::Contours(l) | TinOperation::Boundary { lines: l, .. } => {
                    for h in &d.sources {
                        l.polylines.push(shape(*h)?);
                    }
                }
                _ => {}
            }
            Ok(d)
        })
        .collect()
}

/// Adds a TIN surface triangulated from `points` (three or more, distinct
/// in plan) with the drawing's default surface style, on `layer` (layer
/// "0" when `None`). An empty `points` slice adds the empty surface's
/// shell (see [`add_empty_tin_surface`]). A drawing without a surface
/// style collection gets one (`tree.rs`).
pub fn add_tin_surface(doc: &mut CadDocument, name: &str, points: &[Vector3], layer: Option<&str>) -> Result<Handle, DxfError> {
    if points.is_empty() {
        return add_empty_tin_surface(doc, name, layer);
    }
    let points = TinDefinition { operation: TinOperation::AddPoints(points.to_vec()), id: [0; 16], texts: Vec::new(), sources: Vec::new() };
    add_tin_surface_with(doc, name, &[points], false, layer)
}

/// Adds an empty TIN surface: the shell the application stores for a
/// surface it has not built (the four bounding corners at the drawing
/// extremes, no triangles), with the drawing's default surface style, on
/// `layer` (layer "0" when `None`). Reading it back yields a
/// [`TinSurface`][crate::classes::civil::TinSurface] with four vertices and
/// no triangles.
pub fn add_empty_tin_surface(doc: &mut CadDocument, name: &str, layer: Option<&str>) -> Result<Handle, DxfError> {
    add_tin_surface_with(doc, name, &[], false, layer)
}

/// Adds a TIN surface built from `definitions` in order (each one's `id`
/// is replaced by a new one; texts and sources are written as given),
/// rebuilding itself on a source change when `auto_rebuild`. The stored
/// triangulation applies the definitions in order (`tin_build`), with the
/// geometry of their sources read from the drawing.
pub fn add_tin_surface_with(doc: &mut CadDocument, name: &str, definitions: &[TinDefinition], auto_rebuild: bool, layer: Option<&str>) -> Result<Handle, DxfError> {
    if definitions.len() > 31 {
        return Err(invalid("a surface holds at most 31 definitions"));
    }
    for d in definitions {
        if d.sources.len() != d.source_count() || d.texts.len() != d.text_count() {
            return Err(invalid("a definition's texts or sources do not match its kind"));
        }
    }
    let resolved = with_source_geometry(doc, definitions)?;
    // Whether the definitions bring any geometry in: a surface without any
    // stores the empty shell (the four bounding corners, no triangles), as
    // the application does for a surface it has not built.
    let geometry = |ds: &[TinDefinition]| {
        ds.iter().any(|d| match &d.operation {
            TinOperation::AddPoints(p) | TinOperation::DrawingObjects { points: p, .. } => !p.is_empty(),
            TinOperation::Breaklines(_, l) | TinOperation::Contours(l) | TinOperation::Boundary { lines: l, .. } => l.polylines.iter().flatten().next().is_some(),
            TinOperation::AddVertex(_) | TinOperation::VertexElevation(_) => true,
            _ => false,
        })
    };
    let finite = |p: &Vector3| p.x.is_finite() && p.y.is_finite() && p.z.is_finite();
    let built = |ds: &[TinDefinition]| -> Result<(Triangulation, Vec<bool>), DxfError> {
        let all_finite = ds.iter().all(|d| match &d.operation {
            TinOperation::AddPoints(p) | TinOperation::DrawingObjects { points: p, .. } => p.iter().all(finite),
            TinOperation::Breaklines(_, l) | TinOperation::Contours(l) | TinOperation::Boundary { lines: l, .. } => l.polylines.iter().flatten().all(finite),
            TinOperation::AddVertex(p) | TinOperation::VertexElevation(p) => finite(p),
            _ => true,
        });
        if geometry(ds) {
            if !all_finite {
                return Err(invalid("a surface needs three or more finite points, distinct in plan"));
            }
            super::tin_build::build(ds).ok_or_else(|| invalid("a surface needs three or more finite points, distinct in plan"))
        } else {
            Ok((Triangulation::empty(), Vec::new()))
        }
    };
    let (tin, hidden) = built(&resolved)?;
    // A snapshot keeps what the definitions before it build.
    let snapshots: Vec<usize> = definitions.iter().enumerate().filter(|(_, d)| matches!(d.operation, TinOperation::Kind40(_))).map(|(i, _)| i).collect();
    let snapshot = match snapshots.as_slice() {
        [] => None,
        [k] => Some(built(&resolved[..*k]).map_err(|_| invalid("a snapshot needs a surface of three or more points before it"))?),
        _ => return Err(invalid("a surface holds one snapshot")),
    };
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    if default_style(doc).is_none() {
        ensure_surface_tree(doc)?;
    }
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no surface styles"))?;
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    write_surface(doc, common, name, definitions, auto_rebuild, &tin, &hidden, snapshot.as_ref().map(|(t, h)| (t, h.as_slice())), style)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    fn orient(a: Vector3, b: Vector3, c: Vector3) -> f64 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }

    /// A drawing with a stand-in style tree (only the keys and references
    /// the surface writer looks up).
    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let root = ensure_civil(&mut doc).unwrap();
        let [tree, head, style] = [(); 3].map(|_| doc.allocate_handle());
        for (h, owner, key, children) in [(tree, root, 31, vec![(KEY_SURFACE_STYLES, head)]), (head, tree, KEY_SURFACE_STYLES, vec![(30, style)]), (style, head, 30, vec![])] {
            let refs = children.iter().map(|c: &(i32, Handle)| soft_owner(c.1.value())).collect();
            put_object(&mut doc, "AECC_TREE_NODE", h, owner, Vec::new(), |w| {
                w.write_bit_long(key);
                w.write_bit_long(children.len() as i32);
                for c in &children {
                    w.write_bit_long(c.0);
                }
                w.write_bit_long(0);
                bits(w, "010");
            }, Body { refs, strings: vec![String::new()] })
            .unwrap();
        }
        set_root_entry(&mut doc, 31, tree).unwrap();
        doc
    }

    #[test]
    fn the_triangulation_is_delaunay_with_hidden_corner_triangles() {
        let pts: Vec<Vector3> = [(0.0, 0.0, 0.0), (10.0, 0.0, 1.0), (0.0, 10.0, 2.0), (10.0, 10.0, 3.0), (20.0, 0.0, 1.5), (20.0, 10.0, 2.5), (5.0, 5.0, 4.0), (15.0, 5.0, 3.5), (0.0, 20.0, 2.0), (20.0, 20.0, 5.0)].iter().map(|p| Vector3::new(p.0, p.1, p.2)).collect();
        let t = triangulate_in(&pts, &pts);
        assert_eq!(t.vertices.len(), 14);
        // Euler: 2n - 2 - (hull vertices: the four corners).
        assert_eq!(t.triangles.len(), 2 * 14 - 2 - 4);
        for [a, b, c] in &t.triangles {
            assert!(orient(t.vertices[*a], t.vertices[*b], t.vertices[*c]) > 0.0);
            for p in 0..t.vertices.len() {
                assert!(!in_circle(t.vertices[*a], t.vertices[*b], t.vertices[*c], t.vertices[p]) || [*a, *b, *c].contains(&p));
            }
        }
        // The points' own triangles, the ones that touch no corner.
        assert_eq!(t.triangles.iter().filter(|tr| tr.iter().all(|v| *v >= 4)).count(), 11);
        let n = t.neighbors();
        assert!(n.iter().flatten().filter(|x| **x == -1).count() == 4);
    }

    #[test]
    fn surfaces_round_trip() {
        let mut doc = drawing();
        let pts = [Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 1.0), Vector3::new(0.0, 10.0, 2.0)];
        assert!(add_tin_surface(&mut doc, "S", &pts[..2], None).is_err());
        assert!(add_tin_surface(&mut doc, "S", &[pts[0], pts[1], Vector3::new(0.0, 0.0, 5.0)], None).is_err());
        let h = add_tin_surface(&mut doc, "S", &pts, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-tin-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let Some(crate::entities::EntityType::Extended(x)) = back.get_entity(h) else { panic!() };
        let crate::entities::ExtendedEntityData::Arch(a) = &x.data else { panic!() };
        let tin = a.tin_surface().unwrap();
        assert_eq!(tin.vertices.len(), 7);
        let drawn: Vec<_> = tin.visible_triangles().collect();
        assert_eq!(drawn.len(), 1);
        assert!(pts.iter().all(|p| drawn[0].contains(p)));
    }

    #[test]
    fn a_new_drawing_gets_its_surface_style() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let pts = [Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 1.0), Vector3::new(0.0, 10.0, 2.0)];
        add_tin_surface(&mut doc, "S", &pts, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-tin-new-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let style = default_style(&back).unwrap();
        let o = object(&back, Handle::new(style)).unwrap();
        assert_eq!(o.dxf_name, "AECC_SURFACE_STYLE");
        // The name, the 17 key texts and two texts per component.
        let d = o.decode();
        assert_eq!(d.strings.len(), 1 + 17 + 2 * 25);
        assert!(d.strings.iter().any(|t| t.ends_with(";ANSI31")));
    }

    #[test]
    fn definitions_round_trip_and_shape_the_triangulation() {
        use crate::classes::civil::{TinBoundaryType, TinLines};
        let mut doc = drawing();
        let grid: Vec<Vector3> = (0..5).flat_map(|i| (0..5).map(move |j| Vector3::new(i as f64 * 25.0, j as f64 * 25.0, 10.0 + i as f64 + j as f64))).collect();
        let lines = |pl: Vec<Vector3>| TinLines { objects: 0, polylines: vec![pl], mid_ordinate: 1.0, maximum_distance: 0.0, weeding_distance: 0.0, weeding_angle: 0.0 };
        let def = |operation, texts: &[&str]| TinDefinition { operation, id: [0; 16], texts: texts.iter().map(|t| t.to_string()).collect(), sources: Vec::new() };
        let (a, b) = (Vector3::new(10.0, 40.0, 30.0), Vector3::new(90.0, 55.0, 30.0));
        let defs = vec![
            def(TinOperation::AddPoints(grid), &[]),
            def(TinOperation::Breaklines(TinBreaklineType::Standard, lines(vec![a, b])), &["B1"]),
            def(TinOperation::Boundary { boundary_type: TinBoundaryType::Hide, non_destructive: false, lines: lines(vec![Vector3::new(70.0, -5.0, 0.0), Vector3::new(105.0, -5.0, 0.0), Vector3::new(105.0, 30.0, 0.0), Vector3::new(70.0, 30.0, 0.0), Vector3::new(70.0, -5.0, 0.0)]) }, &["H1"]),
            def(TinOperation::DeleteVertex(Vector2::new(25.0, 75.0)), &[]),
            def(TinOperation::Raise(2.0), &[]),
        ];
        // Texts must match the kinds.
        assert!(add_tin_surface_with(&mut doc, "S", &defs[1..2].iter().map(|d| TinDefinition { texts: Vec::new(), ..d.clone() }).collect::<Vec<_>>(), false, None).is_err());
        let h = add_tin_surface_with(&mut doc, "S", &defs, true, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-tin-defs-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let Some(crate::entities::EntityType::Extended(x)) = back.get_entity(h) else { panic!() };
        let crate::entities::ExtendedEntityData::Arch(r) = &x.data else { panic!() };
        let (read, auto) = r.tin_definitions().unwrap();
        assert!(auto);
        assert_eq!(read.iter().map(|d| (&d.operation, &d.texts)).collect::<Vec<_>>(), defs.iter().map(|d| (&d.operation, &d.texts)).collect::<Vec<_>>());
        let tin = r.tin_surface().unwrap();
        // The breakline is an edge, raised with everything; the deleted
        // vertex is gone; the hide boundary hides the cell it holds.
        let at = |p: Vector3| tin.vertices.iter().position(|v| v.position.x == p.x && v.position.y == p.y).unwrap() as u32;
        let (ia, ib) = (at(a), at(b));
        assert_eq!(tin.vertices[ia as usize].position.z, 32.0);
        assert!(tin.triangles.iter().any(|t| t.is_visible() && t.vertices.contains(&ia) && t.vertices.contains(&ib)));
        assert!(!tin.vertices.iter().any(|v| v.position.x == 25.0 && v.position.y == 75.0));
        assert!(!tin.visible_triangles().any(|t| t.iter().all(|p| p.x >= 75.0 && p.y <= 25.0)));
    }

    /// A snapshot keeps what the definitions before it build; the surface
    /// applies the later ones (TS01, TS02); a second snapshot is refused.
    #[test]
    fn a_snapshot_keeps_the_triangulation_before_it() {
        let mut doc = drawing();
        let def = |operation| TinDefinition { operation, id: [0; 16], texts: Vec::new(), sources: Vec::new() };
        let grid: Vec<Vector3> = (0..3).flat_map(|i| (0..3).map(move |j| Vector3::new(i as f64 * 10.0, j as f64 * 10.0, 5.0))).collect();
        let defs = [def(TinOperation::AddPoints(grid)), def(TinOperation::Kind40(0)), def(TinOperation::Raise(2.0))];
        assert!(add_tin_surface_with(&mut doc, "T", &[defs[0].clone(), defs[1].clone(), defs[1].clone()], false, None).is_err());
        let h = add_tin_surface_with(&mut doc, "S", &defs, false, None).unwrap();
        let Some(crate::entities::EntityType::Extended(x)) = doc.get_entity(h) else { panic!() };
        let crate::entities::ExtendedEntityData::Arch(a) = &x.data else { panic!() };
        let (now, kept) = (a.tin_surface().unwrap(), a.tin_snapshot().unwrap());
        assert_eq!((now.vertices.len(), kept.vertices.len(), now.vertices[4].position.z, kept.vertices[4].position.z), (13, 13, 7.0, 5.0));
        assert_eq!(a.tin_definitions().unwrap().0.len(), 3);
    }

    /// An empty surface stores the shell (four corners at the drawing
    /// extremes, no triangles) and decodes back as empty, not `None`; so
    /// does the snapshot of a snapshot-only surface (Pipe Networks-3B).
    #[test]
    fn an_empty_surface_round_trips_as_a_shell() {
        let mut doc = drawing();
        let h = crate::classes::civil::add_empty_tin_surface(&mut doc, "EG", None).unwrap();
        let snapshot_only = [TinDefinition { operation: TinOperation::Kind40(0), id: [0; 16], texts: Vec::new(), sources: Vec::new() }];
        let hs = add_tin_surface_with(&mut doc, "SNAP", &snapshot_only, false, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-tin-empty-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        for (handle, snapshot) in [(h, false), (hs, true)] {
            let Some(crate::entities::EntityType::Extended(x)) = back.get_entity(handle) else { panic!() };
            let crate::entities::ExtendedEntityData::Arch(a) = &x.data else { panic!() };
            let shell = a.tin_surface().unwrap();
            assert_eq!(shell.vertices.len(), 4);
            assert!(shell.triangles.is_empty());
            let m = f64::MAX;
            assert_eq!(shell.vertices[0].position, Vector3::new(m, m, m));
            assert_eq!(shell.vertices[3].position, Vector3::new(m, -m, -m));
            assert_eq!(a.tin_snapshot().is_some(), snapshot);
            if snapshot {
                assert_eq!(a.tin_snapshot().unwrap().triangles.len(), 0);
            }
        }
    }
}
