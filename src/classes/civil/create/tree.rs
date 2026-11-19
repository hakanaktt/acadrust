//! The style, class and settings nodes a point needs when the drawing has
//! no civil settings and style tree (a drawing the reference application
//! has never opened). The application completes a partial tree when it
//! opens the drawing: it adds the missing keys, children and settings and
//! keeps the records written here (docs/civil-write.md, §1).

use super::support::*;
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Color, Handle};
use crate::CadDocument;

/// Root keys of the style tree, the class nodes and the settings.
const KEY_STYLES: i32 = 31;
const KEY_CLASSES: i32 = 5;
const KEY_SETTINGS: i32 = 45;
/// Style tree key of the point style collection.
const KEY_POINT_STYLES: i32 = 35;
/// Id of a collection's unnamed default style.
const DEFAULT_STYLE_ID: i32 = 30;
/// Key of the attribute classification under a class node.
const KEY_CLASSIFICATION: i32 = 0xEA6;
/// Text ids of the written class and settings nodes (their names are the
/// texts). Kept apart from the ids the point and group writers give.
const TEXT_POINT_CLASS: u32 = 0x8002_0001;
const TEXT_POINTS: u32 = 0x8002_0002;
const TEXT_CREATE_POINTS: u32 = 0x8002_0003;

/// How a node's body ends after its child keys.
enum Tail {
    /// Tree and class nodes: `BL 0`, three bits `010`.
    Tree,
    /// Settings nodes: `BL 0`, `B 0` (no settings of its own: the
    /// application fills in its defaults), `BL 0` (no style items).
    Settings,
}

/// A node: `key`, its children `(key, record)` in stored order (each a
/// soft ownership), `extra` references after them.
#[allow(clippy::too_many_arguments)]
fn node(doc: &mut CadDocument, dxf: &str, handle: Handle, owner: Handle, key: u32, children: &[(u32, Handle)], tail: Tail, extra: Vec<Ref>, strings: Vec<String>) -> Result<(), DxfError> {
    let mut refs: Vec<Ref> = children.iter().map(|c| soft_owner(c.1.value())).collect();
    refs.extend(extra);
    put_object(
        doc,
        dxf,
        handle,
        owner,
        Vec::new(),
        |w| {
            w.write_bit_long(key as i32);
            w.write_bit_long(children.len() as i32);
            for c in children {
                w.write_bit_long(c.0 as i32);
            }
            w.write_bit_long(0);
            match tail {
                Tail::Tree => bits(w, "010"),
                Tail::Settings => {
                    w.write_bit(false);
                    w.write_bit_long(0);
                }
            }
        },
        Body { refs, strings },
    )
}

/// A node's texts: its own name, an empty text when its key is a text id,
/// then the names of its children whose keys are text ids, then `tail`.
fn texts(own: &str, text_key: bool, children: &[&str], tail: &[&str]) -> Vec<String> {
    let mut v = vec![own.to_string()];
    if text_key {
        v.push(String::new());
    }
    v.extend(children.iter().chain(tail).map(|s| s.to_string()));
    v
}

/// One display component of a style.
pub(super) struct Component {
    /// The component's index in its class.
    pub index: i32,
    pub visible: bool,
    pub color: Color,
    /// Lineweight: -1 ByLayer, -2 ByBlock.
    pub lineweight: i16,
    /// Linetype handle.
    pub linetype: u64,
}

/// The value of a style setting, by its key's type (bits 24..29 of the key).
pub(super) enum Value {
    /// Types 4 (none) and 0x10 (a reference, in the reference list).
    None,
    /// Type 0x0D.
    Long(i32),
    /// Type 2.
    Double(f64),
    /// Type 3.
    Byte(u8),
    /// Type 0x16: a FILETIME, written as the time of writing.
    Time,
}

/// Current time as a FILETIME (100 ns since 1601-01-01).
fn file_time() -> u64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() / 100).unwrap_or(0);
    (unix + 116_444_736_000_000_000) as u64
}

/// A style record:
///
/// ```text
/// BS 0x04BE, BL id, BL n, n x BL child id, BL 0,
/// BL 0, B 1, RC key count, keys (BL key, value by its type)
/// BL 32, 3 bits 001
/// 8 component lists: plan, model, profile, section, four empty hatch
///   lists. A component: BL index, BS 80, BS 80, BS 0, BS 80, B visible,
///   CMC color (BS 0, BL method and value, RC 0), BS lineweight,
///   BD linetype scale 1, BL 1, BL 0, BL 0x01000000, BL 0
/// class data
/// references: n children (soft ownership), one per reference key (hard
///   pointer), `own` (soft pointer), a (layer, linetype, plot style)
///   triple per component (hard pointers), the class data's references
/// texts: as for nodes, then created by, modified by, two empty texts per
///   component
/// ```
pub(super) struct Style<'a> {
    pub dxf: &'a str,
    pub id: i32,
    pub children: &'a [(i32, Handle)],
    pub keys: &'a [(u32, Value)],
    /// One per key of type 0x10, in key order.
    pub key_refs: Vec<u64>,
    pub own: u64,
    pub lists: [Vec<Component>; 4],
    pub class_refs: Vec<Ref>,
}

pub(super) fn style(doc: &mut CadDocument, handle: Handle, owner: Handle, st: Style, layer: u64, class: impl FnOnce(&mut DwgMergedWriter)) -> Result<(), DxfError> {
    let count: usize = st.lists.iter().map(Vec::len).sum();
    let mut refs: Vec<Ref> = st.children.iter().map(|c| soft_owner(c.1.value())).collect();
    refs.extend(st.key_refs.iter().map(|h| hard_pointer(*h)));
    refs.push(soft_pointer(st.own));
    for c in st.lists.iter().flatten() {
        refs.extend([hard_pointer(layer), hard_pointer(c.linetype), hard_pointer(0)]);
    }
    refs.extend(st.class_refs);
    let mut strings = vec![String::new(), String::new(), String::new()];
    strings.extend((0..2 * count).map(|_| String::new()));
    let now = file_time().to_le_bytes();
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    put_object(
        doc,
        st.dxf,
        handle,
        owner,
        Vec::new(),
        |w| {
            w.write_bit_long(st.id);
            w.write_bit_long(st.children.len() as i32);
            for c in st.children {
                w.write_bit_long(c.0);
            }
            w.write_bit_long(0);
            w.write_bit_long(0);
            w.write_bit(true);
            byte(w, st.keys.len() as u8);
            for (key, value) in st.keys {
                w.write_bit_long(*key as i32);
                match value {
                    Value::None => {}
                    Value::Long(v) => w.write_bit_long(*v),
                    Value::Double(v) => w.write_bit_double(*v),
                    Value::Byte(v) => byte(w, *v),
                    Value::Time => w.write_bytes(&now),
                }
            }
            w.write_bit_long(32);
            bits(w, "001");
            for list in &st.lists {
                w.write_bit_long(list.len() as i32);
                for c in list {
                    w.write_bit_long(c.index);
                    for v in [80, 80, 0, 80] {
                        w.write_bit_short(v);
                    }
                    w.write_bit(c.visible);
                    w.write_bit_short(0);
                    w.write_bit_long(color_word(c.color) as i32);
                    byte(w, 0);
                    w.write_bit_short(c.lineweight);
                    w.write_bit_double(1.0);
                    for v in [1, 0, 0x0100_0000, 0] {
                        w.write_bit_long(v);
                    }
                }
            }
            for _ in 0..4 {
                w.write_bit_long(0);
            }
            class(w);
        },
        Body { refs, strings },
    )
}

/// The settings every style of the samples starts its keys with or ends
/// them with: two keys without value and the dates.
const POINT_STYLE_KEYS: [(u32, Value); 4] = [(0x0400_1008, Value::None), (0x0400_1010, Value::None), (0x1600_101F, Value::Time), (0x1600_101E, Value::Time)];

/// A point style (`AECC_POINT_STYLE`): the components of the
/// application's default style (marker and label in plan and model,
/// one in profile and section) and its marker:
///
/// ```text
/// BS 3 (cross), 2 bits 10, BD 0.1/12 (size), RC 0 (plotted units),
/// 44 bits the same in every point style of the samples
/// references after the triples: one empty hard pointer
/// ```
#[allow(clippy::too_many_arguments)]
fn point_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], layer: u64, by_block: u64, by_layer: u64) -> Result<(), DxfError> {
    let plan = |index, visible| Component { index, visible, color: Color::ByBlock, lineweight: -2, linetype: by_block };
    let other = Component { index: 0, visible: true, color: Color::ByBlock, lineweight: -1, linetype: by_layer };
    let lists = [vec![plan(0, true), plan(1, true)], vec![plan(0, false), plan(1, false)], vec![Component { ..other }], vec![other]];
    let st = Style { dxf: "AECC_POINT_STYLE", id, children, keys: &POINT_STYLE_KEYS, key_refs: Vec::new(), own: 0, lists, class_refs: vec![hard_pointer(0)] };
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    style(doc, handle, owner, st, layer, |w| {
        w.write_bit_short(3);
        bits(w, "10");
        w.write_bit_double(0.1 / 12.0);
        byte(w, 0);
        // ponytail: the 44 bits after the marker's size mode are the same
        // in every point style of the samples; name them once a style
        // varies them.
        bits(w, "00000001010101000010010100000001000000001001");
    })
}

/// The CMC color word: method byte high, value low.
fn color_word(c: Color) -> u32 {
    match c {
        Color::ByLayer => 0xC000_0000,
        Color::ByBlock => 0xC100_0000,
        _ => c.index().map_or(0xC100_0000, |i| 0xC300_0000 | u32::from(i)),
    }
}

/// Writes the point style collection, the COGO point class node and the
/// points settings when the drawing has none of the style tree, class
/// nodes and settings. Returns `false` (and writes nothing) when one of
/// them exists: the drawing then holds a tree the application built.
pub(super) fn ensure_point_tree(doc: &mut CadDocument) -> Result<bool, DxfError> {
    let root = ensure_civil(doc)?;
    if [KEY_STYLES, KEY_CLASSES, KEY_SETTINGS].iter().any(|k| root_node(doc, *k).is_some()) {
        return Ok(false);
    }
    let layer = doc.layers.get("0").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let linetype = |name: &str| doc.line_types.get(name).map(|l| l.handle.value()).ok_or_else(|| invalid("drawing lacks the ByBlock or ByLayer linetype"));
    let (by_block, by_layer) = (linetype("ByBlock")?, linetype("ByLayer")?);
    let [styles, head, style, classes, class, classification, settings, points, create] = [(); 9].map(|_| doc.allocate_handle());

    node(doc, "AECC_TREE_NODE", styles, root, KEY_STYLES as u32, &[(KEY_POINT_STYLES as u32, head)], Tail::Tree, Vec::new(), texts("", false, &[], &[]))?;
    point_style(doc, head, styles, KEY_POINT_STYLES, &[(DEFAULT_STYLE_ID, style)], layer, by_block, by_layer)?;
    point_style(doc, style, head, DEFAULT_STYLE_ID, &[], layer, by_block, by_layer)?;

    let name = "AeccDbCogoPoint";
    node(doc, "AECC_CLASS_NODE", classes, root, KEY_CLASSES as u32, &[(TEXT_POINT_CLASS, class)], Tail::Tree, vec![Ref(DwgReferenceType::HardOwnership, 0)], texts("", false, &[name], &[]))?;
    node(doc, "AECC_CLASS_NODE", class, classes, TEXT_POINT_CLASS, &[(KEY_CLASSIFICATION as u32, classification)], Tail::Tree, vec![Ref(DwgReferenceType::HardOwnership, 0)], texts(name, true, &[], &[name]))?;
    node(doc, "AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION", classification, class, KEY_CLASSIFICATION as u32, &[], Tail::Tree, Vec::new(), texts("", false, &[], &[]))?;

    node(doc, "AECC_ROOT_SETTINGS_NODE", settings, root, KEY_SETTINGS as u32, &[(TEXT_POINTS, points)], Tail::Settings, vec![soft_pointer(0)], texts("", false, &["AeccPoints"], &[]))?;
    // The points settings end with the default label style (none: the
    // application gives points its own label styles) and point style.
    node(doc, "AECC_SETTINGS_NODE", points, settings, TEXT_POINTS, &[(TEXT_CREATE_POINTS, create)], Tail::Settings, vec![soft_pointer(0), soft_pointer(style.value())], texts("AeccPoints", true, &["AeccCreatePoints"], &[]))?;
    node(doc, "AECC_SETTINGS_NODE", create, points, TEXT_CREATE_POINTS, &[], Tail::Settings, Vec::new(), texts("AeccCreatePoints", true, &[], &[]))?;

    set_root_entry(doc, KEY_STYLES, styles)?;
    set_root_entry(doc, KEY_CLASSES, classes)?;
    set_root_entry(doc, KEY_SETTINGS, settings)?;
    Ok(true)
}

/// Style tree key of the alignment style collection, root key of the
/// siteless alignments and the key of their default feature style.
pub(super) const KEY_ALIGNMENT_STYLES: i32 = 93;
pub(super) const KEY_SITELESS: i32 = 336;
pub(super) const KEY_DEFAULT_FEATURE: i32 = 30;

/// The settings of the application's default alignment style, in stored
/// order. The 17 keys of type 0x10 refer to marker styles (the default
/// style refers to one for two of them); a written style refers to none.
const ALIGNMENT_STYLE_KEYS: [(u32, Value); 29] = [
    (0x1000_E409, Value::None),
    (0x0D00_E300, Value::Long(4096)),
    (0x1000_E406, Value::None),
    (0x0400_1008, Value::None),
    (0x1000_E403, Value::None),
    (0x0200_E122, Value::Double(1.0)),
    (0x1000_E400, Value::None),
    (0x0300_E11F, Value::Byte(0)),
    (0x1000_E40E, Value::None),
    (0x0400_1010, Value::None),
    (0x1000_E40B, Value::None),
    (0x1000_E408, Value::None),
    (0x1000_E405, Value::None),
    (0x0200_E124, Value::Double(1.0)),
    (0x1000_E402, Value::None),
    (0x0200_E121, Value::Double(0.18)),
    (0x1000_B052, Value::None),
    (0x1000_E40D, Value::None),
    (0x1600_101F, Value::Time),
    (0x1000_E40A, Value::None),
    (0x0D00_E301, Value::Long(256)),
    (0x1000_E407, Value::None),
    (0x1000_E404, Value::None),
    (0x0200_E123, Value::Double(1.0)),
    (0x1000_E401, Value::None),
    (0x0200_E120, Value::Double(0.001)),
    (0x1000_E40F, Value::None),
    (0x1000_E40C, Value::None),
    (0x1600_101E, Value::Time),
];

/// An alignment style (`AECC_ALIGNMENT_STYLE`): the default style's
/// components (indices 0-5, 11 and 20 in plan, visible but 11, and hidden
/// in model; index 9 in section), no class data.
fn alignment_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], own: u64, layer: u64, by_block: u64) -> Result<(), DxfError> {
    let indices = [0, 1, 2, 3, 4, 5, 11, 20];
    let view = |shown: bool| -> Vec<Component> { indices.iter().map(|i| Component { index: *i, visible: shown && *i != 11, color: Color::ByLayer, lineweight: -1, linetype: by_block }).collect() };
    let lists = [view(true), view(false), Vec::new(), vec![Component { index: 9, visible: true, color: Color::ByBlock, lineweight: -1, linetype: by_block }]];
    let key_refs = vec![0; ALIGNMENT_STYLE_KEYS.iter().filter(|k| k.0 >> 24 & 0x3F == 0x10).count()];
    let st = Style { dxf: "AECC_ALIGNMENT_STYLE", id, children, keys: &ALIGNMENT_STYLE_KEYS, key_refs, own, lists, class_refs: Vec::new() };
    style(doc, handle, owner, st, layer, |_| {})
}

/// Writes what an alignment needs that the drawing lacks: the point tree
/// of a new drawing (for its style tree), the alignment style collection
/// and its default style, and the node of siteless alignments with its
/// default feature style (an empty site topology).
pub(super) fn ensure_alignment_tree(doc: &mut CadDocument) -> Result<(), DxfError> {
    if root_node(doc, KEY_STYLES).is_none() && !ensure_point_tree(doc)? {
        return Err(invalid("drawing has a civil tree without a style tree"));
    }
    let root = ensure_civil(doc)?;
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let layer = doc.layers.get("0").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let by_block = doc.line_types.get("ByBlock").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing lacks the ByBlock linetype"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    if !tree.children.iter().any(|c| c.0 == KEY_ALIGNMENT_STYLES) {
        let [head, default] = [(); 2].map(|_| doc.allocate_handle());
        alignment_style(doc, head, styles, KEY_ALIGNMENT_STYLES, &[(DEFAULT_STYLE_ID, default)], default.value(), layer, by_block)?;
        alignment_style(doc, default, head, DEFAULT_STYLE_ID, &[], 0, layer, by_block)?;
        add_node_child(doc, styles, KEY_ALIGNMENT_STYLES, head, "")?;
    }
    if root_node(doc, KEY_SITELESS).is_none() {
        let [siteless, feature] = [(); 2].map(|_| doc.allocate_handle());
        node(doc, "AECC_ALIGNMENT_PARCEL_NODE", siteless, root, KEY_SITELESS as u32, &[(KEY_DEFAULT_FEATURE as u32, feature)], Tail::Tree, Vec::new(), texts("", false, &[], &[]))?;
        // The default feature style is a node with an empty site topology:
        // BL 0, BL 32, BL 210, 21 bits, BL 0x80000001, no nodes (BL 1, BL 0,
        // BL 0), no edges (BL 1, BL 0), the face table.
        put_object(
            doc,
            "AECC_FEATURE_STYLE",
            feature,
            siteless,
            Vec::new(),
            |w| {
                for v in [KEY_DEFAULT_FEATURE, 0, 0, 32, 210] {
                    w.write_bit_long(v);
                }
                bits(w, "000000110010000000110");
                w.write_bit_long(0x8000_0001_u32 as i32);
                for v in [1, 0, 0, 1, 0, 2, 0, 0, 1, 100, 1, 100, 1, 100, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0] {
                    w.write_bit_long(v);
                }
            },
            Body { refs: Vec::new(), strings: vec![String::new()] },
        )?;
        set_root_entry(doc, KEY_SITELESS, siteless)?;
    }
    Ok(())
}
