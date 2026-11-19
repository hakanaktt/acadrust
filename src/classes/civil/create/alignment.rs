//! Alignments of fixed lines with free arcs between them (`AECC_ALIGNMENT`).

use super::support::*;
use super::tree::{ensure_alignment_tree, KEY_ALIGNMENT_STYLES, KEY_DEFAULT_FEATURE, KEY_SITELESS};
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// Element kind of a line, and the constraint of a fixed line.
const LINE: i32 = 257;
const FIXED: i16 = 0;
/// Element kind of an arc, and the constraint of an arc fitted between
/// two lines.
const ARC: i32 = 258;
const BETWEEN_LINES: i32 = 29;
/// Flags of an arc edge in the companion topology, besides the line flags.
const ARC_EDGE: i32 = 0x010F_0000;
/// Key of the geometry stamp setting.
const STAMP: i32 = 0x0B00_1081;

/// The drawing's default alignment style: the first style of the style
/// tree's alignment style collection.
fn default_style(doc: &CadDocument) -> Option<u64> {
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
type PathPoint = (Vector2, Option<f64>);

/// The solved path through `vertices`: at a vertex with a radius the
/// lines end at the arc's tangent points. `None` when a radius is not
/// positive, a curved vertex has no turn, or the arcs do not fit on the
/// lines between them.
fn solve(vertices: &[Vector2], radii: &[f64]) -> Option<Vec<PathPoint>> {
    let n = vertices.len();
    let dir = |i: usize| (vertices[i + 1] - vertices[i]).normalize();
    // Per interior vertex: (tangent length, signed turn).
    let mut curves: Vec<Option<(f64, f64)>> = Vec::new();
    for (j, r) in radii.iter().enumerate() {
        if *r == 0.0 {
            curves.push(None);
            continue;
        }
        let (u, v) = (dir(j), dir(j + 1));
        let turn = u.cross(&v).atan2(u.dot(&v));
        (r.is_finite() && *r > 0.0 && turn.abs() > 1e-9 && turn.abs() < std::f64::consts::PI - 1e-9).then_some(())?;
        curves.push(Some((r * (turn.abs() / 2.0).tan(), turn)));
    }
    let tangent = |j: usize| curves.get(j).copied().flatten().map_or(0.0, |c| c.0);
    for i in 0..n - 1 {
        let before = if i == 0 { 0.0 } else { tangent(i - 1) };
        (before + tangent(i) <= vertices[i].distance(&vertices[i + 1]) * (1.0 + 1e-12)).then_some(())?;
    }
    let mut path = vec![(vertices[0], None)];
    for (j, c) in curves.iter().enumerate() {
        let p = vertices[j + 1];
        match c {
            None => path.push((p, None)),
            Some((t, turn)) => {
                path.push((p - dir(j) * *t, Some((turn / 4.0).tan())));
                path.push((p + dir(j + 1) * *t, None));
            }
        }
    }
    path.push((vertices[n - 1], None));
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
/// BL 0 (byte form), 65 zero bits, BD 2.25, 7 bits 0111010
/// BL 1 (first element id), BD 0, BD 0 (start station)
/// 7 bits 0000011, BD 0 (track gauge), B 0, BL 1
/// BL n, the lines (ids 1.. in station order) then the arcs (ids after
///   them, in station order):
///   line: BL 257, BS 2, B 0, BS 0 (fixed), BS id, BS previous id,
///         BS next id, 2RD start, 2RD end (the vertices before and after)
///   arc:  BL 258, BS 2, B 0, B 0, BD -1 (no design speed), BD -1, BS 1,
///         BD -1, BL 29 (fitted between two lines), B 0, BS id,
///         BS previous id, BS next id, BD radius
/// BL 0 x 4 (intervals), BL 0 (equations), BL 1, BL 0 (speeds)
/// BD 100, BL 0 (no superelevation), BL 0, BL 1 (centerline),
/// BL 2, 2, 2, 0, 3, B 0, BS 0x04BE
/// references: soft pointer 0, companion, companion, soft pointer 0,
///   style, hard pointer 0
/// texts: "AeccImpAlignment", "", "", name, five empty texts, three more
///   per arc
/// ```
///
/// Written for WA03's polyline and radius, the record equals WA03's bit
/// for bit apart from the geometry stamp.
pub(super) fn write_alignment(doc: &mut CadDocument, common: EntityCommon, name: &str, vertices: &[Vector2], radii: &[f64], style: u64, feature: u64) -> Result<Handle, DxfError> {
    let n = vertices.len() - 1;
    // Arc ids follow the lines'; `arc[j]` is the id of the arc at inner
    // vertex j.
    let mut next_arc = n as i16;
    let arc: Vec<Option<i16>> = radii.iter().map(|r| (*r > 0.0).then(|| { next_arc += 1; next_arc })).collect();
    let arcs = next_arc as usize - n;
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let refs = vec![soft_pointer(0), hard_pointer(feature), hard_pointer(feature), soft_pointer(0), hard_pointer(style), hard_pointer(0)];
    let mut strings = vec!["AeccImpAlignment".to_string(), String::new(), String::new(), name.to_string()];
    strings.extend((0..5 + 3 * arcs).map(|_| String::new()));
    put_entity(
        doc,
        "AECC_ALIGNMENT",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
                w.write_bit_long(v);
            }
            bits(w, "010101");
            byte(w, 1);
            w.write_bit_long(STAMP);
            w.write_bit_long(2);
            w.write_bit_long(0);
            w.write_bit_long(0);
            bl_byte(w, 0);
            bits(w, &"0".repeat(65));
            w.write_bit_double(2.25);
            bits(w, "0111010");
            w.write_bit_long(1);
            w.write_bit_double(0.0);
            w.write_bit_double(0.0);
            bits(w, "0000011");
            w.write_bit_double(0.0);
            w.write_bit(false);
            w.write_bit_long(1);
            w.write_bit_long((n + arcs) as i32);
            for i in 0..n {
                let id = i as i16 + 1;
                w.write_bit_long(LINE);
                w.write_bit_short(2);
                w.write_bit(false);
                w.write_bit_short(FIXED);
                w.write_bit_short(id);
                w.write_bit_short(if i == 0 { 0 } else { arc[i - 1].unwrap_or(id - 1) });
                w.write_bit_short(if i + 1 == n { 0 } else { arc[i].unwrap_or(id + 1) });
                for p in [vertices[i], vertices[i + 1]] {
                    w.write_raw_double(p.x);
                    w.write_raw_double(p.y);
                }
            }
            for (j, id) in arc.iter().enumerate() {
                let Some(id) = id else { continue };
                w.write_bit_long(ARC);
                w.write_bit_short(2);
                w.write_bit(false);
                w.write_bit(false);
                w.write_bit_double(-1.0);
                w.write_bit_double(-1.0);
                w.write_bit_short(1);
                w.write_bit_double(-1.0);
                w.write_bit_long(BETWEEN_LINES);
                w.write_bit(false);
                w.write_bit_short(*id);
                w.write_bit_short(j as i16 + 1);
                w.write_bit_short(j as i16 + 2);
                w.write_bit_double(radii[j]);
            }
            for v in [0, 0, 0, 0, 0, 1, 0] {
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
    add_alignment_with_curves(doc, name, vertices, &vec![0.0; vertices.len().saturating_sub(2)], layer)
}

/// Adds an alignment of fixed lines through `vertices` with a free arc of
/// radius `radii[j]` at inner vertex `j + 1` (0 for none): the arc is
/// tangent to the lines before and after the vertex, which end at its
/// tangent points. The arcs must fit on the lines between them.
pub fn add_alignment_with_curves(doc: &mut CadDocument, name: &str, vertices: &[Vector2], radii: &[f64], layer: Option<&str>) -> Result<Handle, DxfError> {
    if vertices.len() < 2 || vertices.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || vertices.windows(2).any(|w| w[0] == w[1]) {
        return Err(invalid("an alignment needs two or more distinct finite vertices"));
    }
    if radii.len() + 2 != vertices.len() {
        return Err(invalid("an alignment needs one radius per inner vertex"));
    }
    let path = solve(vertices, radii).ok_or_else(|| invalid("a curve has no turn, a radius is not positive, or the curves do not fit on the lines"))?;
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
    write_alignment(doc, common, name, vertices, radii, style, companion.value())
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
        assert!(add_alignment_with_curves(&mut doc, "X", &v, &[50.0], None).is_err());
        assert!(add_alignment_with_curves(&mut doc, "X", &v, &[5000.0, 0.0, 0.0], None).is_err());
        assert!(add_alignment_with_curves(&mut doc, "X", &[v[0], v[1], Vector2::new(200.0, 0.0)], &[10.0], None).is_err());
        let h = add_alignment_with_curves(&mut doc, "AL1", &v, &[50.0, 0.0, 40.0], None).unwrap();
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
    }
}
