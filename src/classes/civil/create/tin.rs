//! TIN surfaces built from points (`AECC_TIN_SURFACE`).

use super::support::*;
use super::tree::{ensure_surface_tree, KEY_SURFACE_STYLES};
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector3};
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

/// Delaunay triangulation of `points` inside a rectangle of four corners
/// `CORNER_MARGIN` outside their extents (Bowyer-Watson).
// ponytail: quadratic insertion (every triangle is tested per point); a
// spatial index when surfaces of many thousand points are written.
pub(super) fn triangulate(points: &[Vector3]) -> Triangulation {
    let (mut lo, mut hi) = (points[0], points[0]);
    for p in points {
        lo = Vector3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = Vector3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let m = CORNER_MARGIN;
    let corner = |x: f64, y: f64| {
        // A corner's elevation is the nearest point's.
        let near = points.iter().min_by(|a, b| ((a.x - x).powi(2) + (a.y - y).powi(2)).total_cmp(&((b.x - x).powi(2) + (b.y - y).powi(2)))).unwrap();
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
fn default_style(doc: &CadDocument) -> Option<u64> {
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

/// The surface record:
///
/// ```text
/// BL 80, 2 bits 11, BL 0x1554C738, BL 0x087C1120, BL 5 (version)
/// BL 8 keys: 0x03011E29 RC 1, 0x0D011E25 BL 0, 0x02011E26 BD DBL_MAX,
///   0x0B001081 BL 2, BL 0, BL 0 (geometry stamp), 0x03001093 RC 0,
///   0x0201230C BD 0, 0x02011E27 BD DBL_MAX, 0x01011E28 BL 8
/// BL 0 (byte form), 121 zero bits, 3BD origin 0, 9 BD identity, BD 1
/// RC 1 item, 5 bits 01000
/// definitions: 2 bits 00, RL 0x80020A28 (one definition), BL 0, BL 0,
///   BL 0xE8A5800C, BL 40, BL 0, 3 bits 000, GUID, BL 0,
///   added vertices: BL n, n x 3BD
///   BL 0 (four-byte form), BL 0xAC150A00, 2 bits 00
/// RL -1, RS 0x04A2, RC 2, RS 0, RS 0x2A3C, "MMS01"
/// triangulation: BL vertex count, BL 0, 1, 2, 3 (the corners),
///   per vertex RL flags (corner 0x8440, point 0x80), RL a triangle using
///   it, 3 RD; BL triangle count, per triangle 3 RL vertices, 3 RL
///   neighbours, RL flags (1 hidden), RL -1
/// 7 bits 1010100, BS 0x04BE
/// references: soft pointer 0, hard owner 0, hard pointer 0, style
/// texts: "", "", name, "", ""
/// ```
pub(super) fn write_surface(doc: &mut CadDocument, common: EntityCommon, name: &str, points: &[Vector3], tin: &Triangulation, style: u64) -> Result<Handle, DxfError> {
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let guid = new_guid(common.handle.value() ^ points.len() as u64);
    let neighbors = tin.neighbors();
    let mut incident = vec![0i32; tin.vertices.len()];
    for (t, tri) in tin.triangles.iter().enumerate() {
        for v in tri {
            incident[*v] = t as i32;
        }
    }
    let refs = vec![soft_pointer(0), Ref(crate::io::dwg::dwg_reference_type::DwgReferenceType::HardOwnership, 0), hard_pointer(0), hard_pointer(style)];
    let strings = ["", "", name, "", ""].map(String::from).to_vec();
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
            bits(w, "00");
            w.write_raw_long(0x8002_0A28_u32 as i32);
            for v in [0, 0, 0xE8A5_800C_u32 as i32, 40, 0] {
                w.write_bit_long(v);
            }
            bits(w, "000");
            w.write_bytes(&guid);
            w.write_bit_long(0);
            w.write_bit_long(points.len() as i32);
            for p in points {
                w.write_bit_double(p.x);
                w.write_bit_double(p.y);
                w.write_bit_double(p.z);
            }
            // BL 0 in its four-byte form.
            w.write_2bits(0);
            w.write_raw_long(0);
            w.write_bit_long(0xAC15_0A00_u32 as i32);
            bits(w, "00");
            w.write_raw_long(-1);
            w.write_bytes(&[0xA2, 0x04, 2, 0, 0, 0x3C, 0x2A]);
            w.write_bytes(b"MMS01");
            w.write_bit_long(tin.vertices.len() as i32);
            for c in 0..4 {
                w.write_bit_long(c);
            }
            for (i, v) in tin.vertices.iter().enumerate() {
                w.write_raw_long((if i < 4 { CORNER } else { POINT }) as i32);
                w.write_raw_long(incident[i]);
                w.write_raw_double(v.x);
                w.write_raw_double(v.y);
                w.write_raw_double(v.z);
            }
            w.write_bit_long(tin.triangles.len() as i32);
            for (tri, n) in tin.triangles.iter().zip(&neighbors) {
                for v in tri {
                    w.write_raw_long(*v as i32);
                }
                for t in n {
                    w.write_raw_long(*t);
                }
                w.write_raw_long(i32::from(tri.iter().any(|v| *v < 4)));
                w.write_raw_long(-1);
            }
            bits(w, "1010100");
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// Adds a TIN surface triangulated from `points` (three or more, distinct
/// in plan) with the drawing's default surface style, on `layer` (layer
/// "0" when `None`). A drawing without a surface style collection gets
/// one (`tree.rs`).
pub fn add_tin_surface(doc: &mut CadDocument, name: &str, points: &[Vector3], layer: Option<&str>) -> Result<Handle, DxfError> {
    let finite = points.iter().all(|p| p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
    let distinct = points.iter().enumerate().all(|(i, p)| points[..i].iter().all(|q| q.x != p.x || q.y != p.y));
    if points.len() < 3 || !finite || !distinct {
        return Err(invalid("a surface needs three or more finite points, distinct in plan"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    if default_style(doc).is_none() {
        ensure_surface_tree(doc)?;
    }
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no surface styles"))?;
    let tin = triangulate(points);
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    write_surface(doc, common, name, points, &tin, style)
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
        let t = triangulate(&pts);
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
}
