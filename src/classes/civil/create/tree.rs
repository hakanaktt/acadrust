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
struct Component {
    visible: bool,
    /// Lineweight: -1 ByLayer, -2 ByBlock.
    lineweight: i16,
    /// Linetype handle.
    linetype: u64,
}

/// Current time as a FILETIME (100 ns since 1601-01-01).
fn file_time() -> u64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() / 100).unwrap_or(0);
    (unix + 116_444_736_000_000_000) as u64
}

/// A point style (`AECC_POINT_STYLE`). `children` are the ids and records
/// of the styles a collection head lists (empty for a style).
///
/// ```text
/// BS 0x04BE, BL id, BL n, n x BL child id, BL 0,
/// BL 0, B 1, RC 4, keys 0x04001008, 0x04001010 (no value),
///                    0x1600101F, 0x1600101E (FILETIME created, modified)
/// BL 32, 3 bits 001
/// 8 component lists: plan (marker, label), model (marker, label),
///   profile (1), section (1), four empty hatch lists; a component is
///   BL index, BS 80 x 2, BS 0, BS 80, B visible, CMC color (ByBlock),
///   BS lineweight, BD linetype scale, BL 1, BL 0, BL 0x01000000, BL 0
/// marker: BS 3 (cross), 2 bits 10, BD size, RC 0 (plotted units), then
///   44 bits (the same in every point style of the samples)
/// references: n children (soft ownership), one empty soft pointer, a
///   (layer, linetype, plot style) triple per component, one empty hard
///   pointer
/// texts: as for nodes, then created by, modified by, two per component
/// ```
fn point_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], layer: u64, by_block: u64, by_layer: u64) -> Result<(), DxfError> {
    let plan = |visible| Component { visible, lineweight: -2, linetype: by_block };
    let lists: [Vec<Component>; 4] = [
        vec![plan(true), plan(true)],
        vec![plan(false), plan(false)],
        vec![Component { visible: true, lineweight: -1, linetype: by_layer }],
        vec![Component { visible: true, lineweight: -1, linetype: by_layer }],
    ];
    let count: usize = lists.iter().map(Vec::len).sum();
    let mut refs: Vec<Ref> = children.iter().map(|c| soft_owner(c.1.value())).collect();
    refs.push(soft_pointer(0));
    for c in lists.iter().flatten() {
        refs.extend([hard_pointer(layer), hard_pointer(c.linetype), hard_pointer(0)]);
    }
    refs.push(hard_pointer(0));
    let mut strings = vec![String::new()];
    strings.extend(["", ""].map(String::from));
    strings.extend((0..2 * count).map(|_| String::new()));
    let now = file_time().to_le_bytes();
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    put_object(
        doc,
        "AECC_POINT_STYLE",
        handle,
        owner,
        Vec::new(),
        |w| {
            w.write_bit_long(id);
            w.write_bit_long(children.len() as i32);
            for c in children {
                w.write_bit_long(c.0);
            }
            w.write_bit_long(0);
            w.write_bit_long(0);
            w.write_bit(true);
            byte(w, 4);
            w.write_bit_long(0x0400_1008);
            w.write_bit_long(0x0400_1010);
            for key in [0x1600_101F, 0x1600_101E] {
                w.write_bit_long(key);
                w.write_bytes(&now);
            }
            w.write_bit_long(32);
            bits(w, "001");
            for list in &lists {
                w.write_bit_long(list.len() as i32);
                for (i, c) in list.iter().enumerate() {
                    w.write_bit_long(i as i32);
                    for v in [80, 80, 0, 80] {
                        w.write_bit_short(v);
                    }
                    w.write_bit(c.visible);
                    w.write_bit_short(0);
                    w.write_bit_long(color_word(Color::ByBlock) as i32);
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
            w.write_bit_short(3);
            bits(w, "10");
            w.write_bit_double(0.1 / 12.0);
            byte(w, 0);
            // ponytail: the 44 bits after the marker's size mode are the
            // same in every point style of the samples; name them once a
            // style varies them.
            bits(w, "00000001010101000010010100000001000000001001");
        },
        Body { refs, strings },
    )
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
