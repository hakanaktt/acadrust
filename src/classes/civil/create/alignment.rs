//! Alignments made of fixed lines (`AECC_ALIGNMENT`).

use super::support::*;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// Style tree key of the alignment style collection.
const KEY_ALIGNMENT_STYLES: i32 = 93;
/// Element kind of a line, and the constraint of a fixed line.
const LINE: i32 = 257;
const FIXED: i16 = 0;
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

/// Root key of the node that holds siteless alignments.
const KEY_SITELESS: i32 = 336;
/// Key of that node's default feature style, which lists the companions.
const KEY_DEFAULT_FEATURE: i32 = 30;
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

/// The companion of a siteless alignment (`AECC_FEATURE_STYLE` under the
/// default feature style): the alignment's site topology. Nodes are the
/// alignment's ends (ids 1, 2) then its inner vertices (ids 3...); the one
/// edge runs through them with the outside face (1) on both sides.
///
/// ```text
/// BS 0x04BE, BL text id, BL 0 (no children), BL 0, BL 32, BL 210,
/// 21 bits 000000110010000000110, BL 0x80000001,
/// nodes: BL next free id, BL n, BL first id, n x {3BD, 3 bits 010, BL next id}
/// edges: BL 2 (next free id), BL 1 slot, BL 0, BL 1 (id), BL 48, BL count, BL 0,
///        count x {BL kind (0 line, 2 last), BL flags, BL node, BL face, BL face},
///        BL 0, BL 0 x 3
/// faces: BL 2, BL 1, the outside face {BL 1, BL 1, BL 7, BL 1, BL 1, BL 0}, BL 0,
///        6 x {BL 1, BL 100}, 6 x BL 0
/// references: the alignment (soft pointer), three empty soft pointers
/// texts: its name (32 hexadecimal digits), ""
/// ```
///
/// A vertex's flags are 2, with 0x30 added when another segment follows
/// its segment; the last vertex's are 10.
fn write_companion(doc: &mut CadDocument, handle: Handle, owner: Handle, tag: u32, alignment: Handle, vertices: &[Vector2], name: &str) -> Result<(), DxfError> {
    let n = vertices.len();
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
                for v in [vertices[*i].x, vertices[*i].y, 0.0] {
                    w.write_bit_double(v);
                }
                bits(w, "010");
                w.write_bit_long(if k + 1 == n { 0 } else { k as i32 + 2 });
            }
            for v in [2, 1, 0, 1, 48, n as i32, 0] {
                w.write_bit_long(v);
            }
            for i in 0..n {
                let last = i + 1 == n;
                let flags = if last { 10 } else if i + 2 < n { 0x32 } else { 2 };
                let face = if last { 0 } else { 1 };
                for v in [if last { 2 } else { 0 }, flags, node_of(i), face, face] {
                    w.write_bit_long(v);
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
/// BL n, n x {BL 257, BS 2, B 0, BS 0 (fixed), BS id, BS previous id,
///            BS next id, 2RD start, 2RD end}
/// BL 0 x 4 (intervals), BL 0 (equations), BL 1, BL 0 (speeds)
/// BD 100, BL 0 (no superelevation), BL 0, BL 1 (centerline),
/// BL 2, 2, 2, 0, 3, B 0, BS 0x04BE
/// references: soft pointer 0, companion, companion, soft pointer 0,
///   style, hard pointer 0
/// texts: "AeccImpAlignment", "", "", name, five empty texts
/// ```
pub(super) fn write_alignment(doc: &mut CadDocument, common: EntityCommon, name: &str, vertices: &[Vector2], style: u64, feature: u64) -> Result<Handle, DxfError> {
    let n = vertices.len() - 1;
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let refs = vec![soft_pointer(0), hard_pointer(feature), hard_pointer(feature), soft_pointer(0), hard_pointer(style), hard_pointer(0)];
    let mut strings = vec!["AeccImpAlignment".to_string(), String::new(), String::new(), name.to_string()];
    strings.extend((0..5).map(|_| String::new()));
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
            w.write_bit_long(n as i32);
            for i in 0..n {
                let id = i as i16 + 1;
                w.write_bit_long(LINE);
                w.write_bit_short(2);
                w.write_bit(false);
                w.write_bit_short(FIXED);
                w.write_bit_short(id);
                w.write_bit_short(if i == 0 { 0 } else { id - 1 });
                w.write_bit_short(if i + 1 == n { 0 } else { id + 1 });
                for p in [vertices[i], vertices[i + 1]] {
                    w.write_raw_double(p.x);
                    w.write_raw_double(p.y);
                }
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
/// `layer` (layer "0" when `None`).
pub fn add_alignment(doc: &mut CadDocument, name: &str, vertices: &[Vector2], layer: Option<&str>) -> Result<Handle, DxfError> {
    if vertices.len() < 2 || vertices.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || vertices.windows(2).any(|w| w[0] == w[1]) {
        return Err(invalid("an alignment needs two or more distinct finite vertices"));
    }
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no alignment styles"))?;
    let node = companion_node(doc).ok_or_else(|| invalid("drawing has no node for siteless alignments"))?;
    let used = read_node(doc, node).ok_or_else(|| invalid("feature style node cannot be read"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
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
    write_companion(doc, companion, node, tag, handle, vertices, &label)?;
    add_node_child(doc, node, tag as i32, companion, &label)?;
    write_alignment(doc, common, name, vertices, style, companion.value())
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
}
