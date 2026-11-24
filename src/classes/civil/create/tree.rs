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
pub(super) const KEY_STYLES: i32 = 31;
const KEY_CLASSES: i32 = 5;
const KEY_SETTINGS: i32 = 45;
/// Style tree key of the point style collection.
const KEY_POINT_STYLES: i32 = 35;
/// Id of a collection's unnamed default style.
pub(super) const DEFAULT_STYLE_ID: i32 = 30;
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
    /// Linetype scale.
    pub scale: f64,
}

/// A table record a style setting refers to.
#[derive(Clone, Copy)]
pub(super) enum Shared {
    /// No record.
    None,
    /// The linetype "Continuous".
    Continuous,
    /// The text style "Standard".
    Standard,
    /// A record of the drawing (a type 0x12 key: one the style owns).
    Record(u64),
}

/// The value of a style setting, by its key's type (bits 24..29 of the key).
pub(super) enum Value {
    /// Type 4: no value in the fields, the text in the text stream.
    Text(&'static str),
    /// Types 0x0F (a soft pointer) and 0x10 (a hard pointer): no value in
    /// the fields, the record in the reference list.
    Reference(Shared),
    /// Type 0x0E: a color, `BS 0, BL method and value, RC 0`.
    Color(u32),
    /// Types 1 and 0x0D.
    Long(i32),
    /// Type 2.
    Double(f64),
    /// Type 3.
    Byte(u8),
    /// Type 9: `3BD`.
    Point([f64; 3]),
    /// Type 0x18: `BL` byte count, UTF-16 with its terminator.
    Wide(&'static str),
    /// [`Value::Wide`] with a text of the caller's.
    WideString(String),
    /// Type 0x16: a FILETIME, written as the time of writing.
    Time,
}

/// Current time as a FILETIME (100 ns since 1601-01-01).
pub(super) fn file_time() -> u64 {
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
/// references: n children (soft ownership), one per reference key (a soft
///   pointer for type 0x0F, hard for 0x10), `own` (soft pointer), a
///   (layer, linetype, plot style)
///   triple per component (hard pointers), the class data's references
/// texts: as for nodes, then the text of every key of type 4 in key order
///   (0x04001008 and 0x04001010 name the users who made and changed the
///   style; written empty), two empty texts per component
/// ```
pub(super) struct Style<'a> {
    /// Its name (with a text id, followed by an empty text).
    pub name: &'a str,
    pub dxf: &'a str,
    pub id: i32,
    pub children: &'a [(i32, Handle)],
    pub keys: &'a [(u32, Value)],
    pub own: u64,
    /// An `AeccAttributeTable<int, hard pointer>` after the keys: its
    /// keys (`BL 0, B 1, RC n, n x BL key, BL 1, B 1`), each with an empty
    /// hard pointer after the keys' references, and its class name after
    /// the keys' texts. Empty: no table.
    pub pointer_table: &'a [u32],
    pub lists: [Vec<Component>; 4],
    pub class_refs: Vec<Ref>,
}

pub(super) fn style(doc: &mut CadDocument, handle: Handle, owner: Handle, st: Style, layer: u64, class: impl FnOnce(&mut DwgMergedWriter)) -> Result<(), DxfError> {
    let count: usize = st.lists.iter().map(Vec::len).sum();
    let table = |s: Shared| -> u64 {
        match s {
            Shared::None => 0,
            Shared::Continuous => doc.line_types.get("Continuous").map_or(0, |l| l.handle.value()),
            Shared::Standard => doc.text_styles.get("Standard").map_or(0, |t| t.handle.value()),
            Shared::Record(h) => h,
        }
    };
    let mut refs: Vec<Ref> = st.children.iter().map(|c| soft_owner(c.1.value())).collect();
    for (key, value) in st.keys {
        if let Value::Reference(s) = value {
            let h = table(*s);
            refs.push(match key >> 24 & 0x3F {
                0x0F => soft_pointer(h),
                0x12 => Ref(DwgReferenceType::HardOwnership, h),
                _ => hard_pointer(h),
            });
        }
    }
    refs.extend(st.pointer_table.iter().map(|_| hard_pointer(0)));
    refs.push(soft_pointer(st.own));
    for c in st.lists.iter().flatten() {
        refs.extend([hard_pointer(layer), hard_pointer(c.linetype), hard_pointer(0)]);
    }
    refs.extend(st.class_refs);
    let mut strings = vec![st.name.to_string()];
    if st.id as u32 & 0x8000_0000 != 0 {
        strings.push(String::new());
    }
    strings.extend(st.keys.iter().filter_map(|(_, v)| match v {
        Value::Text(t) => Some(t.to_string()),
        _ => None,
    }));
    if !st.pointer_table.is_empty() {
        strings.push("class AeccAttributeTable<int,class AcDbHardPointerId>".into());
    }
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
                    Value::Text(_) | Value::Reference(_) => {}
                    Value::Color(c) => {
                        w.write_bit_short(0);
                        w.write_bit_long(*c as i32);
                        byte(w, 0);
                    }
                    Value::Long(v) => w.write_bit_long(*v),
                    Value::Double(v) => w.write_bit_double(*v),
                    Value::Byte(v) => byte(w, *v),
                    Value::Point(p) => {
                        for v in p {
                            w.write_bit_double(*v);
                        }
                    }
                    Value::Wide(_) | Value::WideString(_) => {
                        let t = match value {
                            Value::WideString(t) => t.as_str(),
                            Value::Wide(t) => t,
                            _ => unreachable!(),
                        };
                        let mut bytes: Vec<u8> = t.encode_utf16().flat_map(u16::to_le_bytes).collect();
                        bytes.extend([0, 0]);
                        w.write_bit_long(bytes.len() as i32);
                        w.write_bytes(&bytes);
                    }
                    Value::Time => w.write_bytes(&now),
                }
            }
            if !st.pointer_table.is_empty() {
                w.write_bit_long(0);
                w.write_bit(true);
                byte(w, st.pointer_table.len() as u8);
                for key in st.pointer_table {
                    w.write_bit_long(*key as i32);
                }
                // ponytail: `BL 1, B 1` closes the table in the only style
                // that has one (the profile style); not named.
                w.write_bit_long(1);
                w.write_bit(true);
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
                    w.write_bit_double(c.scale);
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
pub(super) const POINT_STYLE_KEYS: [(u32, Value); 4] = [(0x0400_1008, Value::Text("")), (0x0400_1010, Value::Text("")), (0x1600_101F, Value::Time), (0x1600_101E, Value::Time)];

/// A point style (`AECC_POINT_STYLE`): the components of the
/// application's default style (marker and label in plan and model,
/// one in profile and section) and its marker:
///
/// ```text
/// BS 3 (cross), 2 bits 10, BD size (plotted, in drawing units: 0.1 in
/// as feet, 2.5 mm as meters), RC 0 (plotted units),
/// 44 bits the same in every point style of the samples
/// references after the triples: one empty hard pointer
/// ```
#[allow(clippy::too_many_arguments)]
fn point_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], layer: u64, by_block: u64, by_layer: u64, size: f64) -> Result<(), DxfError> {
    let plan = |index, visible| Component { index, visible, color: Color::ByBlock, lineweight: -2, linetype: by_block, scale: 1.0 };
    let other = Component { index: 0, visible: true, color: Color::ByBlock, lineweight: -1, linetype: by_layer, scale: 1.0 };
    let lists = [vec![plan(0, true), plan(1, true)], vec![plan(0, false), plan(1, false)], vec![Component { ..other }], vec![other]];
    let st = Style { name: "", dxf: "AECC_POINT_STYLE", id, children, keys: &POINT_STYLE_KEYS, own: 0, pointer_table: &[], lists, class_refs: vec![hard_pointer(0)] };
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    style(doc, handle, owner, st, layer, |w| {
        w.write_bit_short(3);
        bits(w, "10");
        w.write_bit_double(size);
        byte(w, 0);
        // ponytail: the 44 bits after the marker's size mode are the same
        // in every point style of the samples; name them once a style
        // varies them.
        bits(w, "00000001010101000010010100000001000000001001");
    })
}

/// The CMC color word: method byte high, value low.
pub(super) fn color_word(c: Color) -> u32 {
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
    // The application's default marker: 0.1 in on imperial drawings, 2.5 mm
    // on metric ones (measured: a new metric drawing's style holds 0.0025).
    let imperial = matches!(doc.header.insertion_units, 1 | 2 | 3 | 10 | 21) || (doc.header.insertion_units == 0 && doc.header.measurement == 0);
    let size = if imperial { 0.1 / 12.0 } else { 0.0025 };
    point_style(doc, head, styles, KEY_POINT_STYLES, &[(DEFAULT_STYLE_ID, style)], layer, by_block, by_layer, size)?;
    point_style(doc, style, head, DEFAULT_STYLE_ID, &[], layer, by_block, by_layer, size)?;

    let name = "AeccDbCogoPoint";
    node(doc, "AECC_CLASS_NODE", classes, root, KEY_CLASSES as u32, &[(TEXT_POINT_CLASS, class)], Tail::Tree, vec![Ref(DwgReferenceType::HardOwnership, 0)], texts("", false, &[name], &[]))?;
    node(doc, "AECC_CLASS_NODE", class, classes, TEXT_POINT_CLASS, &[(KEY_CLASSIFICATION as u32, classification)], Tail::Tree, vec![Ref(DwgReferenceType::HardOwnership, 0)], texts(name, true, &[], &[name]))?;
    node(doc, "AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION", classification, class, KEY_CLASSIFICATION as u32, &[], Tail::Tree, Vec::new(), texts("", false, &[], &[]))?;

    node(doc, "AECC_ROOT_SETTINGS_NODE", settings, root, KEY_SETTINGS as u32, &[(TEXT_POINTS, points)], Tail::Settings, vec![soft_pointer(0)], texts("", false, &["AeccPoints"], &[]))?;
    // The points settings end with the default label style (written
    // below, with the label styles) and point style.
    node(doc, "AECC_SETTINGS_NODE", points, settings, TEXT_POINTS, &[(TEXT_CREATE_POINTS, create)], Tail::Settings, vec![soft_pointer(0), soft_pointer(style.value())], texts("AeccPoints", true, &["AeccCreatePoints"], &[]))?;
    node(doc, "AECC_SETTINGS_NODE", create, points, TEXT_CREATE_POINTS, &[], Tail::Settings, Vec::new(), texts("AeccCreatePoints", true, &[], &[]))?;

    set_root_entry(doc, KEY_STYLES, styles)?;
    set_root_entry(doc, KEY_CLASSES, classes)?;
    set_root_entry(doc, KEY_SETTINGS, settings)?;
    // With the default label styles the points settings name the default
    // point label style, which new points take.
    super::labels::ensure_label_styles(doc)?;
    let label = super::labels::default_label_style(doc, super::labels::POINT_LABELS).ok_or_else(|| invalid("the point label styles were not written"))?;
    node(doc, "AECC_SETTINGS_NODE", points, settings, TEXT_POINTS, &[(TEXT_CREATE_POINTS, create)], Tail::Settings, vec![soft_pointer(label), soft_pointer(style.value())], texts("AeccPoints", true, &["AeccCreatePoints"], &[]))?;
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
    (0x1000_E409, Value::Reference(Shared::None)),
    (0x0D00_E300, Value::Long(4096)),
    (0x1000_E406, Value::Reference(Shared::None)),
    (0x0400_1008, Value::Text("")),
    (0x1000_E403, Value::Reference(Shared::None)),
    (0x0200_E122, Value::Double(1.0)),
    (0x1000_E400, Value::Reference(Shared::None)),
    (0x0300_E11F, Value::Byte(0)),
    (0x1000_E40E, Value::Reference(Shared::None)),
    (0x0400_1010, Value::Text("")),
    (0x1000_E40B, Value::Reference(Shared::None)),
    (0x1000_E408, Value::Reference(Shared::None)),
    (0x1000_E405, Value::Reference(Shared::None)),
    (0x0200_E124, Value::Double(1.0)),
    (0x1000_E402, Value::Reference(Shared::None)),
    (0x0200_E121, Value::Double(0.18)),
    (0x1000_B052, Value::Reference(Shared::None)),
    (0x1000_E40D, Value::Reference(Shared::None)),
    (0x1600_101F, Value::Time),
    (0x1000_E40A, Value::Reference(Shared::None)),
    (0x0D00_E301, Value::Long(256)),
    (0x1000_E407, Value::Reference(Shared::None)),
    (0x1000_E404, Value::Reference(Shared::None)),
    (0x0200_E123, Value::Double(1.0)),
    (0x1000_E401, Value::Reference(Shared::None)),
    (0x0200_E120, Value::Double(0.001)),
    (0x1000_E40F, Value::Reference(Shared::None)),
    (0x1000_E40C, Value::Reference(Shared::None)),
    (0x1600_101E, Value::Time),
];

/// An alignment style (`AECC_ALIGNMENT_STYLE`): the default style's
/// components (indices 0-5, 11 and 20 in plan, visible but 11, and hidden
/// in model; index 9 in section), no class data.
fn alignment_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], own: u64, layer: u64, by_block: u64) -> Result<(), DxfError> {
    let indices = [0, 1, 2, 3, 4, 5, 11, 20];
    let view = |shown: bool| -> Vec<Component> { indices.iter().map(|i| Component { index: *i, visible: shown && *i != 11, color: Color::ByLayer, lineweight: -1, linetype: by_block, scale: 1.0 }).collect() };
    let lists = [view(true), view(false), Vec::new(), vec![Component { index: 9, visible: true, color: Color::ByBlock, lineweight: -1, linetype: by_block, scale: 1.0 }]];
    let st = Style { name: "", dxf: "AECC_ALIGNMENT_STYLE", id, children, keys: &ALIGNMENT_STYLE_KEYS, own, pointer_table: &[], lists, class_refs: Vec::new() };
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

/// Style tree key of the surface style collection.
pub(super) const KEY_SURFACE_STYLES: i32 = 77;

/// The settings of the application's default surface style, in stored
/// order. Its legend table and label styles (eight keys of type 0x0F) are
/// written empty; the other references are the linetype "Continuous" and
/// the text style "Standard". A surface style with only the dates opens
/// but draws nothing.
const SURFACE_STYLE_KEYS: [(u32, Value); 216] = [
    (0x0201_410C, Value::Double(0.0)),
    (0x0201_430A, Value::Double(std::f64::consts::FRAC_PI_2)),
    (0x0401_4409, Value::Text(" ")),
    (0x0301_4508, Value::Byte(0)),
    (0x0301_4607, Value::Byte(0)),
    (0x0D01_4625, Value::Long(3)),
    (0x0101_461F, Value::Long(4)),
    (0x0201_4706, Value::Double(1.0)),
    (0x0D01_4900, Value::Long(1)),
    (0x0201_4904, Value::Double(1.0)),
    (0x0201_4A03, Value::Double(0.0)),
    (0x0201_4B02, Value::Double(0.0)),
    (0x0E01_4B0F, Value::Color(0xC3000006)),
    (0x0400_1008, Value::Text("")),
    (0x0301_4109, Value::Byte(1)),
    (0x0201_4307, Value::Double(25.0)),
    (0x0101_440D, Value::Long(66)),
    (0x0301_4505, Value::Byte(1)),
    (0x0D01_4600, Value::Long(1)),
    (0x0301_4604, Value::Byte(0)),
    (0x0201_4615, Value::Double(10.0)),
    (0x0301_4703, Value::Byte(0)),
    (0x0101_470A, Value::Long(1)),
    (0x0E01_4B1D, Value::Color(0xC3000002)),
    (0x0301_4B21, Value::Byte(0)),
    (0x0101_4F79, Value::Long(0)),
    (0x0F01_4D03, Value::Reference(Shared::None)),
    (0x0F01_461B, Value::Reference(Shared::None)),
    (0x0301_4106, Value::Byte(0)),
    (0x0D01_4300, Value::Long(1)),
    (0x0201_4403, Value::Double(0.0)),
    (0x0E01_4410, Value::Color(0xC0000000)),
    (0x0401_4711, Value::Text(" ")),
    (0x0101_4905, Value::Long(8)),
    (0x0E01_4B09, Value::Color(0xC3000003)),
    (0x0401_4B0D, Value::Text("1;0;0;100.0;0.0;2.0;ANSI31")),
    (0x0E01_4B2B, Value::Color(0xC3000003)),
    (0x0301_4B2F, Value::Byte(0)),
    (0x0D01_4F05, Value::Long(0)),
    (0x0D01_4C08, Value::Long(1)),
    (0x0301_4103, Value::Byte(0)),
    (0x0201_4202, Value::Double(1.0)),
    (0x0301_4301, Value::Byte(1)),
    (0x0401_460F, Value::Text(" ")),
    (0x0301_4620, Value::Byte(0)),
    (0x0101_4A01, Value::Long(1)),
    (0x0D01_4D04, Value::Long(5)),
    (0x0201_4F7D, Value::Double(0.1)),
    (0x0D01_4C05, Value::Long(0)),
    (0x0F01_4B10, Value::Reference(Shared::Continuous)),
    (0x0F01_4B32, Value::Reference(Shared::None)),
    (0x0400_1010, Value::Text("")),
    (0x0301_4100, Value::Byte(1)),
    (0x0101_4602, Value::Long(1)),
    (0x0201_460C, Value::Double(2.0)),
    (0x0D01_4619, Value::Long(0)),
    (0x0201_461D, Value::Double(0.0)),
    (0x0101_4701, Value::Long(1)),
    (0x0401_470B, Value::Text(" ")),
    (0x0301_4B07, Value::Byte(0)),
    (0x0E01_4B03, Value::Color(0xC3000001)),
    (0x0D01_4D01, Value::Long(0)),
    (0x0D01_4E00, Value::Long(1)),
    (0x0D01_4F76, Value::Long(0)),
    (0x0201_4F7A, Value::Double(1.0)),
    (0x0F01_4612, Value::Reference(Shared::Standard)),
    (0x0F01_4B1E, Value::Reference(Shared::Continuous)),
    (0x0201_410E, Value::Double(0.0)),
    (0x0101_4401, Value::Long(1)),
    (0x0301_4609, Value::Byte(0)),
    (0x0101_4610, Value::Long(0)),
    (0x0201_4708, Value::Double(0.0)),
    (0x0101_470F, Value::Long(1)),
    (0x0D01_4902, Value::Long(1)),
    (0x0D01_4B00, Value::Long(1)),
    (0x0E01_4B11, Value::Color(0xC3000006)),
    (0x0301_4B15, Value::Byte(0)),
    (0x0E01_4B22, Value::Color(0xC3000001)),
    (0x0301_4B26, Value::Byte(1)),
    (0x0D01_4B33, Value::Long(2)),
    (0x0301_4F77, Value::Byte(0)),
    (0x0F01_4B2C, Value::Reference(Shared::Continuous)),
    (0x0301_410B, Value::Byte(1)),
    (0x0201_4309, Value::Double(25.0)),
    (0x0401_4408, Value::Text(" ")),
    (0x0301_4507, Value::Byte(0)),
    (0x0301_4606, Value::Byte(0)),
    (0x0101_460D, Value::Long(4)),
    (0x0D01_4624, Value::Long(5)),
    (0x0201_4705, Value::Double(0.0)),
    (0x0D01_4800, Value::Long(1)),
    (0x0201_4804, Value::Double(1.0)),
    (0x0201_4903, Value::Double(0.0)),
    (0x0201_4B01, Value::Double(0.0)),
    (0x0101_4B08, Value::Long(35)),
    (0x0E01_4B30, Value::Color(0xC3000005)),
    (0x0201_4B34, Value::Double(10.0)),
    (0x0F01_4F03, Value::Reference(Shared::None)),
    (0x0301_4108, Value::Byte(1)),
    (0x0201_4306, Value::Double(1.0)),
    (0x0401_4405, Value::Text("+")),
    (0x0101_440C, Value::Long(34)),
    (0x0E01_4412, Value::Color(0xC0000000)),
    (0x0D01_4500, Value::Long(1)),
    (0x0201_4504, Value::Double(1.0)),
    (0x0201_4614, Value::Double(100.0)),
    (0x0101_4709, Value::Long(2)),
    (0x0401_4B20, Value::Text("1;0;0;100.0;0.0;2.0;ANSI31")),
    (0x0F01_4B04, Value::Reference(Shared::Continuous)),
    (0x0F01_4C03, Value::Reference(Shared::None)),
    (0x0F00_B44D, Value::Reference(Shared::None)),
    (0x0301_4105, Value::Byte(0)),
    (0x0D01_4200, Value::Long(1)),
    (0x0301_4402, Value::Byte(0)),
    (0x0201_4611, Value::Double(100.0)),
    (0x0D01_461E, Value::Long(0)),
    (0x0401_4710, Value::Text(" ")),
    (0x0101_4805, Value::Long(8)),
    (0x0301_4B0C, Value::Byte(1)),
    (0x0E01_4B19, Value::Color(0xC3000001)),
    (0x0E01_4B2A, Value::Color(0xC3000001)),
    (0x0401_4B2E, Value::Text("1;0;0;100.0;0.0;2.0;ESCHER")),
    (0x0F01_4B12, Value::Reference(Shared::Continuous)),
    (0x0F01_4B23, Value::Reference(Shared::Continuous)),
    (0x0301_4102, Value::Byte(1)),
    (0x0201_4201, Value::Double(0.0)),
    (0x0101_4406, Value::Long(2)),
    (0x0401_460E, Value::Text(" ")),
    (0x0101_4901, Value::Long(1)),
    (0x0301_4B1A, Value::Byte(0)),
    (0x0D01_4C04, Value::Long(0)),
    (0x0D01_4F01, Value::Long(0)),
    (0x0301_4F7C, Value::Byte(0)),
    (0x0F01_4B31, Value::Reference(Shared::Continuous)),
    (0x1600_101F, Value::Time),
    (0x0101_4601, Value::Long(1)),
    (0x0201_460B, Value::Double(1.0)),
    (0x0D01_4618, Value::Long(1)),
    (0x0101_4623, Value::Long(2)),
    (0x0401_4B06, Value::Text("1;0;0;100.0;0.0;2.0;ANSI37")),
    (0x0301_4B17, Value::Byte(0)),
    (0x0E01_4B13, Value::Color(0xC3000003)),
    (0x0E01_4B24, Value::Color(0xC3000001)),
    (0x0301_4B28, Value::Byte(0)),
    (0x0D01_4C01, Value::Long(0)),
    (0x0D01_4D00, Value::Long(1)),
    (0x0F01_4622, Value::Reference(Shared::None)),
    (0x0201_410D, Value::Double(0.0)),
    (0x0201_4509, Value::Double(0.0)),
    (0x0301_4608, Value::Byte(0)),
    (0x0201_4707, Value::Double(25.0)),
    (0x0101_470E, Value::Long(0)),
    (0x0D01_4802, Value::Long(1)),
    (0x0D01_4A00, Value::Long(1)),
    (0x0201_4A04, Value::Double(1.0)),
    (0x0F01_470D, Value::Reference(Shared::Standard)),
    (0x0301_410A, Value::Byte(0)),
    (0x0201_4308, Value::Double(0.0)),
    (0x0201_4407, Value::Double(0.0)),
    (0x0301_4506, Value::Byte(1)),
    (0x0301_4605, Value::Byte(1)),
    (0x0201_4616, Value::Double(2.0)),
    (0x0D01_4700, Value::Long(1)),
    (0x0301_4704, Value::Byte(0)),
    (0x0201_4803, Value::Double(0.0)),
    (0x0101_4B18, Value::Long(35)),
    (0x0101_4B29, Value::Long(35)),
    (0x0F01_4E03, Value::Reference(Shared::None)),
    (0x0301_4107, Value::Byte(1)),
    (0x0201_4305, Value::Double(0.0)),
    (0x0D01_4400, Value::Long(1)),
    (0x0201_4404, Value::Double(1.0)),
    (0x0101_440B, Value::Long(2)),
    (0x0E01_4411, Value::Color(0xC0000000)),
    (0x0201_4503, Value::Double(0.0)),
    (0x0201_4613, Value::Double(0.0)),
    (0x0101_4A05, Value::Long(8)),
    (0x0E01_4B0A, Value::Color(0xC3000005)),
    (0x0301_4B0E, Value::Byte(0)),
    (0x0E01_4B1B, Value::Color(0xC3000005)),
    (0x0301_4B1F, Value::Byte(0)),
    (0x0D01_4F06, Value::Long(5)),
    (0x0D01_4C09, Value::Long(6)),
    (0x0F01_440A, Value::Reference(Shared::Standard)),
    (0x0F01_4B14, Value::Reference(Shared::Continuous)),
    (0x0F01_4B25, Value::Reference(Shared::Continuous)),
    (0x0301_4104, Value::Byte(0)),
    (0x0301_4302, Value::Byte(1)),
    (0x0D01_440E, Value::Long(2)),
    (0x0101_4617, Value::Long(1)),
    (0x0201_4621, Value::Double(10.0)),
    (0x0301_4B2D, Value::Byte(0)),
    (0x0D01_4E04, Value::Long(6)),
    (0x0301_4101, Value::Byte(0)),
    (0x0201_440F, Value::Double(3.0)),
    (0x0101_4603, Value::Long(1)),
    (0x0101_4702, Value::Long(1)),
    (0x0401_470C, Value::Text(" ")),
    (0x0101_4713, Value::Long(8)),
    (0x0101_4801, Value::Long(1)),
    (0x0D01_4E01, Value::Long(0)),
    (0x0D01_4F00, Value::Long(1)),
    (0x0201_4F04, Value::Double(5.0)),
    (0x0201_4F7B, Value::Double(1.0)),
    (0x0201_4C07, Value::Double(5.0)),
    (0x0F01_4712, Value::Reference(Shared::Standard)),
    (0x1600_101E, Value::Time),
    (0x0201_460A, Value::Double(0.0)),
    (0x0D01_4A02, Value::Long(1)),
    (0x0301_4B05, Value::Byte(0)),
    (0x0401_4B16, Value::Text("1;0;0;100.0;0.0;2.0;EARTH")),
    (0x0401_4B27, Value::Text("1;0;0;100.0;0.0;2.0;ANSI32")),
    (0x0D01_4C00, Value::Long(1)),
    (0x0301_4F78, Value::Byte(0)),
    (0x0F01_4B0B, Value::Reference(Shared::Continuous)),
    (0x0F01_4B1C, Value::Reference(Shared::Continuous)),
];

/// A surface style (`AECC_SURFACE_STYLE`): the default style's components
/// (twelve in plan and model, the border (2) drawn in plan and the
/// triangles (1) in model; one in section), class data `RC 0`.
fn surface_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], own: u64, layer: u64, by_block: u64) -> Result<(), DxfError> {
    let colors = [Some(1), Some(4), Some(2), Some(3), Some(42), None, Some(6), None, None, None, None, None];
    let view = |shown: i32| -> Vec<Component> {
        (0..12).map(|i| Component { index: i, visible: i == shown, color: colors[i as usize].map_or(Color::ByLayer, Color::from_index), lineweight: -2, linetype: by_block, scale: 1.0 }).collect()
    };
    let lists = [view(2), view(1), Vec::new(), vec![Component { index: 12, visible: false, color: Color::from_index(2), lineweight: -2, linetype: by_block, scale: 1.0 }]];
    let st = Style { name: "", dxf: "AECC_SURFACE_STYLE", id, children, keys: &SURFACE_STYLE_KEYS, own, pointer_table: &[], lists, class_refs: Vec::new() };
    style(doc, handle, owner, st, layer, |w| bits(w, "00000000"))
}

/// Writes the surface style collection and its default style when the
/// drawing lacks them (and the point tree of a drawing without a style
/// tree).
pub(super) fn ensure_surface_tree(doc: &mut CadDocument) -> Result<(), DxfError> {
    if root_node(doc, KEY_STYLES).is_none() && !ensure_point_tree(doc)? {
        return Err(invalid("drawing has a civil tree without a style tree"));
    }
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    if tree.children.iter().any(|c| c.0 == KEY_SURFACE_STYLES) {
        return Ok(());
    }
    let layer = doc.layers.get("0").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let by_block = doc.line_types.get("ByBlock").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing lacks the ByBlock linetype"))?;
    let [head, default] = [(); 2].map(|_| doc.allocate_handle());
    surface_style(doc, head, styles, KEY_SURFACE_STYLES, &[(DEFAULT_STYLE_ID, default)], default.value(), layer, by_block)?;
    surface_style(doc, default, head, DEFAULT_STYLE_ID, &[], 0, layer, by_block)?;
    add_node_child(doc, styles, KEY_SURFACE_STYLES, head, "")
}

/// Style tree key of the profile style collection.
pub(super) const KEY_PROFILE_STYLES: i32 = 74;

/// The settings of the application's default profile style, in stored
/// order; `extension` is the style's extension record (key 0x12010131).
/// The 12 keys of type 0x10 refer to marker styles, written empty.
fn profile_style_keys(extension: u64) -> [(u32, Value); 30] {
    [
        (0x1001_0130, Value::Reference(Shared::None)),
        (0x0D00_E300, Value::Long(4097)),
        (0x0301_0108, Value::Byte(1)),
        (0x0201_0105, Value::Double(5.0)),
        (0x0400_1008, Value::Text("")),
        (0x0200_E122, Value::Double(1.0)),
        (0x1001_0127, Value::Reference(Shared::None)),
        (0x0201_0110, Value::Double(1.0)),
        (0x1001_0124, Value::Reference(Shared::None)),
        (0x0400_1010, Value::Text("")),
        (0x0200_E124, Value::Double(1.0)),
        (0x1001_0129, Value::Reference(Shared::None)),
        (0x0200_E121, Value::Double(0.2)),
        (0x1001_0126, Value::Reference(Shared::None)),
        (0x0201_010F, Value::Double(5.0)),
        (0x1001_0123, Value::Reference(Shared::None)),
        (0x1000_B052, Value::Reference(Shared::None)),
        (0x1600_101F, Value::Time),
        (0x1201_0131, Value::Reference(Shared::Record(extension))),
        (0x1001_0120, Value::Reference(Shared::None)),
        (0x0D00_E301, Value::Long(258)),
        (0x0301_0109, Value::Byte(1)),
        (0x0201_0106, Value::Double(5.0)),
        (0x0200_E123, Value::Double(1.0)),
        (0x1001_0128, Value::Reference(Shared::None)),
        (0x1000_E401, Value::Reference(Shared::None)),
        (0x1001_0125, Value::Reference(Shared::None)),
        (0x0201_010E, Value::Double(5.0)),
        (0x1001_0122, Value::Reference(Shared::None)),
        (0x1600_101E, Value::Time),
    ]
}

/// A profile style (`AECC_PROFILE_STYLE`) and the extension record it owns
/// (`AECC_VALIGNMENT_STYLE_EXTENSION`):
///
/// ```text
/// style: the style layout with the keys above, a pointer table of one
///   key (0x10010101), components 9 in model, 0-6 and 11 in profile
///   (ByLayer, linetype scale 2), 10 in section (ByBlock); no class data
/// extension: the style layout with id 0, the users and the dates, one
///   profile component 12 (ByLayer), no class data
/// ```
#[allow(clippy::too_many_arguments)]
fn profile_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], own: u64, layer: u64, by_block: u64) -> Result<(), DxfError> {
    let extension = doc.allocate_handle();
    let c = |index: i32, color: Color, scale: f64| Component { index, visible: true, color, lineweight: -1, linetype: by_block, scale };
    let profile = [0, 1, 2, 3, 4, 5, 6, 11].iter().map(|i| c(*i, Color::ByLayer, 2.0)).collect();
    let lists = [Vec::new(), vec![c(9, Color::ByLayer, 2.0)], profile, vec![c(10, Color::ByBlock, 1.0)]];
    let keys = profile_style_keys(extension.value());
    let st = Style { name: "", dxf: "AECC_PROFILE_STYLE", id, children, keys: &keys, own, pointer_table: &[0x1001_0101], lists, class_refs: Vec::new() };
    style(doc, handle, owner, st, layer, |_| {})?;
    let lists = [Vec::new(), Vec::new(), vec![c(12, Color::ByLayer, 1.0)], Vec::new()];
    let st = Style { name: "", dxf: "AECC_VALIGNMENT_STYLE_EXTENSION", id: 0, children: &[], keys: &POINT_STYLE_KEYS, own: 0, pointer_table: &[], lists, class_refs: Vec::new() };
    style(doc, extension, handle, st, layer, |_| {})
}

/// Writes the profile style collection and its default style when the
/// drawing lacks them (and the point tree of a drawing without a style
/// tree).
pub(super) fn ensure_profile_tree(doc: &mut CadDocument) -> Result<(), DxfError> {
    if root_node(doc, KEY_STYLES).is_none() && !ensure_point_tree(doc)? {
        return Err(invalid("drawing has a civil tree without a style tree"));
    }
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    if tree.children.iter().any(|c| c.0 == KEY_PROFILE_STYLES) {
        return Ok(());
    }
    let layer = doc.layers.get("0").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let by_block = doc.line_types.get("ByBlock").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing lacks the ByBlock linetype"))?;
    let [head, default] = [(); 2].map(|_| doc.allocate_handle());
    profile_style(doc, head, styles, KEY_PROFILE_STYLES, &[(DEFAULT_STYLE_ID, default)], default.value(), layer, by_block)?;
    profile_style(doc, default, head, DEFAULT_STYLE_ID, &[], 0, layer, by_block)?;
    add_node_child(doc, styles, KEY_PROFILE_STYLES, head, "")
}
