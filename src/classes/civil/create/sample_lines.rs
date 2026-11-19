//! Sample line groups and sample lines.

use super::alignment::{rewritable, rewrite_with};
use super::profile::identifier;
use super::site::default_style_of;
use super::support::*;
use super::tree::{ensure_point_tree, style, Component, Style, DEFAULT_STYLE_ID, KEY_STYLES, POINT_STYLE_KEYS};
use crate::classes::arch::ArchEntityData;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Color, Handle};
use crate::CadDocument;

/// Style tree key of the sample line style collection.
const KEY_SAMPLE_LINE_STYLES: i32 = 0xCD5;
/// Path from the style tree to the default sample line label style: the
/// label node, the sample line labels, their collection, the default.
const LABEL_PATH: [i32; 4] = [3, 0x112, 0x1002_A00F, DEFAULT_STYLE_ID];
/// The class word after the class head (both records).
const WORD_GROUP: i32 = 0x4000_0353;
const WORD_LINE: i32 = 0x4000_8353;
/// The geometry stamp key (`BL 2`, two values; written as 0).
const STAMP: i32 = 0x0B00_1081;

/// The runs of a group record before and after its line count (the same in
/// WL10 to WL13: no to three lines).
const GROUP_HEAD: &str = "01000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000101010011010100110101001";
const GROUP_TAIL: &str = "1010101110101000000000000000000000000000000000000000000000000000100100101000000000000000000000000000000000000000000000000000000000100100101000000000000000000000000000000000000000000000000000000000100100101000000000000000000000000000000000000000000000000000000000101100101000000000000000000000000000000000000000000000000000000000101100101000000000000000000011110101010100001011111000000100";
/// The run that closes a sample line record.
const LINE_TAIL: &str = "1010110101000111111111111111111111111111111110011111111111111111111111111111111100001011111000000100";
/// The group's texts after its name (the same in every sample).
const GROUP_TEXTS: [&str; 7] = ["", "", "0", "0", "", "", ""];

fn arch(doc: &CadDocument, h: Handle) -> Option<&ArchEntityData> {
    match doc.get_entity(h)? {
        EntityType::Extended(x) => match &x.data {
            ExtendedEntityData::Arch(a) => Some(a),
            _ => None,
        },
        _ => None,
    }
}

fn head(w: &mut DwgMergedWriter, word: i32) {
    for v in [80, 0, 80, 80, 0, 0x1002_5F04, word] {
        w.write_bit_long(v);
    }
}

/// A sample line group (`AECC_SAMPLE_LINE_GROUP`, an entity in model
/// space):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40000353, 4 bits 0101,
/// RC 1, the geometry stamp (BL 2, BL 0, BL 0), GROUP_HEAD, BL n lines,
/// GROUP_TAIL
/// references: soft 0, hard owner 0, alignment (hard), hard 0, the lines
///   (soft, by station), soft 0 x 3, sample line style, label style (hard)
/// texts: "AeccImpSampleLineGroup", "", name, "", "", "0", "0", "", "", ""
/// ```
///
/// The groups of WL10 to WL13 are reproduced bit for bit apart from the
/// stamp.
#[allow(clippy::too_many_arguments)]
fn write_group(doc: &mut CadDocument, common: EntityCommon, name: &str, alignment: u64, lines: &[u64], line_style: u64, label_style: u64) -> Result<Handle, DxfError> {
    let mut refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(alignment), hard_pointer(0)];
    refs.extend(lines.iter().map(|l| soft_pointer(*l)));
    refs.extend([soft_pointer(0), soft_pointer(0), soft_pointer(0), hard_pointer(line_style), hard_pointer(label_style)]);
    let mut strings = vec!["AeccImpSampleLineGroup".to_string(), String::new(), name.to_string()];
    strings.extend(GROUP_TEXTS.iter().map(|s| s.to_string()));
    put_entity(
        doc,
        "AECC_SAMPLE_LINE_GROUP",
        common,
        |w| {
            head(w, WORD_GROUP);
            bits(w, "0101");
            bits(w, "00000001");
            for v in [STAMP, 2, 0, 0] {
                w.write_bit_long(v);
            }
            bits(w, GROUP_HEAD);
            w.write_bit_long(lines.len() as i32);
            bits(w, GROUP_TAIL);
        },
        Body { refs, strings },
    )
}

/// A sample line (`AECC_SAMPLE_LINE`):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008353, 4 bits 0010, RC 0,
/// 120 zero bits, 3BD 0 (origin), 9 BD (identity), BD station,
/// two vertices {BL 1, BD 0, BD offset (right positive), BD 0},
/// LINE_TAIL
/// references: soft 0, hard owner 0, hard 0, soft 0, soft 0, style, group
///   (hard)
/// texts: "AeccImpSampleLine", "", name, ""
/// ```
///
/// The lines of WL11 to WL13 (stations 50, 80 and 20) equal the samples'
/// records bit for bit.
fn write_line(doc: &mut CadDocument, common: EntityCommon, name: &str, station: f64, offsets: [f64; 2], line_style: u64, group: u64) -> Result<Handle, DxfError> {
    let refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(0), soft_pointer(0), soft_pointer(0), hard_pointer(line_style), hard_pointer(group)];
    let strings = ["AeccImpSampleLine", "", name, ""].iter().map(|s| s.to_string()).collect();
    put_entity(
        doc,
        "AECC_SAMPLE_LINE",
        common,
        |w| {
            head(w, WORD_LINE);
            bits(w, "0010");
            bits(w, "00000000");
            bits(w, &"0".repeat(120));
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, station] {
                w.write_bit_double(v);
            }
            for o in offsets {
                w.write_bit_long(1);
                w.write_bit_double(0.0);
                w.write_bit_double(o);
                w.write_bit_double(0.0);
            }
            bits(w, LINE_TAIL);
        },
        Body { refs, strings },
    )
}

/// The default sample line style: the users and dates, plan components 0
/// (visible) and 1, model components 0 and 1 (hidden), ByLayer color and
/// lineweight, linetype ByBlock; no class data. With these the collection
/// (style tree key 0xCD5) and its default style (id 30) equal WL13's
/// records apart from the dates and the users.
fn line_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], own: u64, layer: u64, by_block: u64) -> Result<(), DxfError> {
    let c = |index: i32, visible: bool| Component { index, visible, color: Color::ByLayer, lineweight: -1, linetype: by_block, scale: 1.0 };
    let lists = [vec![c(0, true), c(1, false)], vec![c(0, false), c(1, false)], Vec::new(), Vec::new()];
    let st = Style { dxf: "AECC_SAMPLE_LINE_STYLE", id, children, keys: &POINT_STYLE_KEYS, own, pointer_table: &[], lists, class_refs: Vec::new() };
    style(doc, handle, owner, st, layer, |_| {})
}

/// Writes the sample line style collection and its default style when the
/// drawing lacks them.
fn ensure_line_style_tree(doc: &mut CadDocument) -> Result<(), DxfError> {
    if root_node(doc, KEY_STYLES).is_none() && !ensure_point_tree(doc)? {
        return Err(invalid("drawing has a civil tree without a style tree"));
    }
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    if tree.children.iter().any(|c| c.0 == KEY_SAMPLE_LINE_STYLES) {
        return Ok(());
    }
    let layer = doc.layers.get("0").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let by_block = doc.line_types.get("ByBlock").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing lacks the ByBlock linetype"))?;
    let [head, default] = [(); 2].map(|_| doc.allocate_handle());
    line_style(doc, head, styles, KEY_SAMPLE_LINE_STYLES, &[(DEFAULT_STYLE_ID, default)], default.value(), layer, by_block)?;
    line_style(doc, default, head, DEFAULT_STYLE_ID, &[], 0, layer, by_block)?;
    add_node_child(doc, styles, KEY_SAMPLE_LINE_STYLES, head, "")
}

/// The default sample line label style. A group without one opens with
/// AUDIT errors (E-L2: the application reports the group's null label
/// style), so a drawing without label styles (a new drawing) is refused.
fn label_style(doc: &CadDocument) -> Option<u64> {
    let mut node = root_node(doc, KEY_STYLES);
    for key in LABEL_PATH {
        node = node.and_then(|n| read_node(doc, n)).and_then(|n| n.children.iter().find(|c| c.0 == key).map(|c| Handle::new(c.1)));
    }
    node.map(|h| h.value()).filter(|h| *h != 0)
}

fn common(doc: &CadDocument, handle: Handle, layer: Option<&str>) -> Result<EntityCommon, DxfError> {
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    Ok(EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() })
}

/// Adds an empty sample line group `name` to `alignment` on `layer` (layer
/// "0" when `None`), with the default sample line style and label style.
/// The alignment must be one this crate writes (it is written again with
/// the group in its list) and the drawing must have the default sample
/// line label style; a drawing without a sample line style gets the
/// default one.
pub fn add_sample_line_group(doc: &mut CadDocument, alignment: Handle, name: &str, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a sample line group needs a name"));
    }
    let (written, profiles) = rewritable(doc, alignment, |h| identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    let label = label_style(doc).ok_or_else(|| invalid("drawing has no sample line label style"))?;
    ensure_line_style_tree(doc)?;
    let line_style = default_style_of(doc, KEY_SAMPLE_LINE_STYLES).ok_or_else(|| invalid("drawing has no sample line styles"))?;
    let handle = doc.allocate_handle();
    let c = common(doc, handle, layer)?;
    let mut groups = written.groups.clone();
    groups.push(handle.value());
    rewrite_with(doc, alignment, written, &profiles, &profiles, None, &groups)?;
    write_group(doc, c, name, alignment.value(), &[], line_style, label)
}

/// A group as written: its record's alignment, lines, styles and name.
struct Group {
    common: EntityCommon,
    name: String,
    alignment: u64,
    lines: Vec<u64>,
    line_style: u64,
    label_style: u64,
}

fn read_group(doc: &CadDocument, group: Handle) -> Option<Group> {
    let Some(EntityType::Extended(x)) = doc.get_entity(group) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    (a.dxf_name == "AECC_SAMPLE_LINE_GROUP").then_some(())?;
    let refs = a.typed_handles();
    (refs.len() >= 9).then_some(())?;
    let strings = a.decode().strings;
    (strings.len() == 3 + GROUP_TEXTS.len() && strings[3..].iter().zip(GROUP_TEXTS).all(|(s, t)| s == t)).then_some(())?;
    Some(Group {
        common: x.common.clone(),
        name: strings[2].clone(),
        alignment: refs[2].0,
        lines: refs[4..refs.len() - 5].iter().map(|r| r.0).collect(),
        line_style: refs[refs.len() - 2].0,
        label_style: refs[refs.len() - 1].0,
    })
}

/// The station a sample line record stores (its 13th `BD` after the
/// 120-bit run), `None` for another record.
fn station(a: &ArchEntityData) -> Option<f64> {
    (a.dxf_name == "AECC_SAMPLE_LINE").then_some(())?;
    let (data, start, end) = a.main_stream()?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    for _ in 0..7 {
        b.bit_long()?;
    }
    b.skip(4 + 8 + 120)?;
    for _ in 0..12 {
        b.bit_double()?;
    }
    b.bit_double()
}

/// Adds a sample line `name` to `group` at `station`, reaching `left` to
/// the left of the alignment and `right` to its right (both distances, at
/// least one positive), on `layer` (layer "0" when `None`). The group is
/// written again with the line among its lines (kept in station order).
pub fn add_sample_line(doc: &mut CadDocument, group: Handle, name: &str, station_value: f64, left: f64, right: f64, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() || !station_value.is_finite() || !(left >= 0.0 && right >= 0.0 && left + right > 0.0) || !left.is_finite() || !right.is_finite() {
        return Err(invalid("a sample line needs a name, a station and non-negative offsets"));
    }
    let g = read_group(doc, group).ok_or_else(|| invalid("the group is not one this crate writes"))?;
    let al = arch(doc, Handle::new(g.alignment)).ok_or_else(|| invalid("the group's alignment is missing"))?;
    let start = al.alignment_start_station().ok_or_else(|| invalid("the alignment has no start station"))?;
    let length: f64 = al.alignment_pieces().iter().map(|(_, p)| p.windows(2).map(|w| w[0].distance(&w[1])).sum::<f64>()).sum();
    if station_value < start - 1e-9 || station_value > start + length + 1e-9 {
        return Err(invalid("the station is off the alignment"));
    }
    let mut lines: Vec<(f64, u64)> = g.lines.iter().map(|l| Some((station(arch(doc, Handle::new(*l))?)?, *l))).collect::<Option<Vec<_>>>().ok_or_else(|| invalid("a line of the group cannot be read"))?;
    let handle = doc.allocate_handle();
    let c = common(doc, handle, layer)?;
    write_line(doc, c, name, station_value, [-left, right], g.line_style, group.value())?;
    let at = lines.partition_point(|l| l.0 <= station_value);
    lines.insert(at, (station_value, handle.value()));
    let ordered: Vec<u64> = lines.iter().map(|l| l.1).collect();
    write_group(doc, g.common, &g.name, g.alignment, &ordered, g.line_style, g.label_style)?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::{DxfVersion, Vector2};

    fn node(doc: &mut CadDocument, handle: Handle, owner: Handle, key: i32, children: &[(i32, Handle)]) {
        put_object(doc, "AECC_TREE_NODE", handle, owner, Vec::new(), |w| {
            w.write_bit_long(key);
            w.write_bit_long(children.len() as i32);
            for c in children {
                w.write_bit_long(c.0);
            }
            w.write_bit_long(0);
            bits(w, "010");
        }, Body { refs: children.iter().map(|c| soft_owner(c.1.value())).collect(), strings: vec![String::new()] })
        .unwrap();
    }

    #[test]
    fn groups_keep_their_lines_by_station() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        // No label style in a new drawing.
        assert!(add_sample_line_group(&mut doc, al, "SLG1", None).is_err());
        let tree = root_node(&doc, KEY_STYLES).unwrap();
        let [labels, kind, head, default] = [(); 4].map(|_| doc.allocate_handle());
        node(&mut doc, labels, tree, LABEL_PATH[0], &[(LABEL_PATH[1], kind)]);
        node(&mut doc, kind, labels, LABEL_PATH[1], &[(LABEL_PATH[2], head)]);
        node(&mut doc, head, kind, LABEL_PATH[2], &[(LABEL_PATH[3], default)]);
        node(&mut doc, default, head, LABEL_PATH[3], &[]);
        add_node_child(&mut doc, tree, LABEL_PATH[0], labels, "").unwrap();
        let g = add_sample_line_group(&mut doc, al, "SLG1", None).unwrap();
        assert!(add_sample_line(&mut doc, g, "X", 150.0, 5.0, 5.0, None).is_err());
        assert!(add_sample_line(&mut doc, g, "X", 50.0, -5.0, 5.0, None).is_err());
        let a = add_sample_line(&mut doc, g, "SL1", 50.0, 10.0, 10.0, None).unwrap();
        let b = add_sample_line(&mut doc, g, "SL2", 80.0, 20.0, 15.0, None).unwrap();
        let c = add_sample_line(&mut doc, g, "SL3", 20.0, 7.0, 5.0, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-sample-lines-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let group = read_group(&back, g).unwrap();
        assert_eq!(group.lines, vec![c.value(), a.value(), b.value()]);
        assert_eq!(group.label_style, default.value());
        assert_eq!(station(arch(&back, b).unwrap()), Some(80.0));
        assert_eq!(arch(&back, a).unwrap().decode().strings[2], "SL1");
        // The alignment lists the group after its own references.
        assert_eq!(arch(&back, al).unwrap().typed_handles().last().map(|r| r.0), Some(g.value()));
        let style = default_style_of(&back, KEY_SAMPLE_LINE_STYLES).unwrap();
        assert_eq!(group.line_style, style);
    }
}
