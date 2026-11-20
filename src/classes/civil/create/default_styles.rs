//! The default section, section view, intersection, roadway marker,
//! parcel and feature line styles of a new drawing.

use super::support::*;
use super::templates::{write_templates, Piece::{Children, Run, Time}, Target, Template};
use super::tree::{ensure_point_tree, KEY_STYLES};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType::{HardOwnership, HardPointer, SoftPointer};
use crate::types::Handle;
use crate::CadDocument;

/// Style tree keys: section styles, section view styles, section view band
/// sets, intersection styles and roadway marker styles.
pub(super) const KEY_SECTION_STYLES: i32 = 0xC18;
pub(super) const KEY_SECTION_VIEW_STYLES: i32 = 0x33;
const KEY_SECTION_VIEW_BAND_SETS: i32 = 0xC0E;
pub(super) const KEY_INTERSECTION_STYLES: i32 = 0x1031;
/// Label node keys of the intersection label root, its collection and
/// its default style ("Basic").
pub(super) const INTERSECTION_LABELS: [i32; 3] = [0x1032, 0x0F00_B579, 0x8000_01D5_u32 as i32];
const KEY_MARKER_STYLES: i32 = 0x11B;
/// Style tree key of the group plot style collection (section view group
/// plots; class `AECC_SAMPLE_LINE_GROUP_STYLE`).
pub(super) const KEY_GROUP_PLOT_STYLES: i32 = 0x96;
/// Style tree keys of the parcel style and feature line style collections.
pub(super) const KEY_PARCEL_STYLES: i32 = 2;
pub(super) const KEY_FEATURE_LINE_STYLES: i32 = 128;
/// Style tree key of the subassembly style collection.
pub(super) const KEY_SUBASSEMBLY_STYLES: i32 = 0x11A;
/// Style tree keys of the pipe and structure style collections.
pub(super) const KEY_PIPE_STYLES: i32 = 0x116;
pub(super) const KEY_STRUCTURE_STYLES: i32 = 0x117;
/// Style tree keys of the assembly, code set, link, shape and corridor
/// style collections.
pub(super) const KEY_ASSEMBLY_STYLES: i32 = 0x118;
pub(super) const KEY_CODE_SET_STYLES: i32 = 0xEA0;
const KEY_LINK_STYLES: i32 = 0x11C;
const KEY_SHAPE_STYLES: i32 = 0x11D;
pub(super) const KEY_CORRIDOR_STYLES: i32 = 0xEB2;
/// Style tree keys of the pressure pipe, fitting and appurtenance style
/// collections and of the pressure part lists.
const KEY_PRESSURE_PIPE_STYLES: i32 = 0x17E;
const KEY_FITTING_STYLES: i32 = 0x17F;
const KEY_APPURTENANCE_STYLES: i32 = 0x180;
pub(super) const KEY_PRESSURE_PART_LISTS: i32 = 0x188;

/// Writes `templates` into the style tree when it lacks all of `keys`,
/// listing the templates at `roots` under those keys. A tree that has some
/// but not all of them is refused (the written records refer to each
/// other).
fn ensure(doc: &mut CadDocument, templates: &[Template], keys: &[(i32, usize)]) -> Result<(), DxfError> {
    if root_node(doc, KEY_STYLES).is_none() && !ensure_point_tree(doc)? {
        return Err(invalid("drawing has a civil tree without a style tree"));
    }
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    let present = keys.iter().filter(|(k, _)| tree.children.iter().any(|c| c.0 == *k)).count();
    if present == keys.len() {
        return Ok(());
    }
    if present != 0 {
        return Err(invalid("drawing has some of the style collections these styles need"));
    }
    let handles = write_templates(doc, styles, templates)?;
    for (key, i) in keys {
        add_node_child(doc, styles, *key, handles[*i], "")?;
    }
    Ok(())
}

/// The section style collection (style tree key 0xC18) and its default
/// style, when the drawing lacks them.
pub(super) fn ensure_section_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, SECTION_STYLES, &[(KEY_SECTION_STYLES, 0)])
}

/// The section view style collection (style tree key 0x33) and the
/// section view band set collection (0xC0E) with their defaults, when the
/// drawing lacks them.
pub(super) fn ensure_section_view_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, SECTION_VIEW_STYLES, &[(KEY_SECTION_VIEW_STYLES, 0), (KEY_SECTION_VIEW_BAND_SETS, 2)])
}

/// The group plot style collection (style tree key 0x96) and its default
/// style, when the drawing lacks them (WG00a).
pub(super) fn ensure_group_plot_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, GROUP_PLOT_STYLES, &[(KEY_GROUP_PLOT_STYLES, 0)])
}

/// The roadway marker style collection (style tree key 0x11B), its
/// default and twelve marker styles named by GUID, when the drawing lacks
/// it.
fn ensure_marker_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, MARKER_STYLES, &[(KEY_MARKER_STYLES, 0)])
}

/// The intersection style collection (style tree key 0x1031) and its
/// default, which draws one of the roadway markers, when the drawing lacks
/// them (with the markers when it lacks those too).
pub(super) fn ensure_intersection_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure_marker_styles(doc)?;
    ensure(doc, INTERSECTION_STYLES, &[(KEY_INTERSECTION_STYLES, 0)])
}

/// The default intersection label style, written with its root,
/// collection and components (the label node with the default label
/// styles when the drawing has none) when the drawing lacks it.
pub(super) fn ensure_intersection_label_style(doc: &mut CadDocument) -> Result<u64, DxfError> {
    if let Some(h) = super::labels::default_label_style(doc, INTERSECTION_LABELS) {
        return Ok(h);
    }
    super::labels::ensure_label_styles(doc)?;
    let styles = root_node(doc, super::tree::KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    let labels = tree.children.iter().find(|c| c.0 == super::labels::KEY_LABELS).map(|c| Handle::new(c.1)).ok_or_else(|| invalid("drawing has no label styles"))?;
    if read_node(doc, labels).ok_or_else(|| invalid("label node cannot be read"))?.children.iter().any(|c| c.0 == INTERSECTION_LABELS[0]) {
        return Err(invalid("the drawing's intersection labels have no default style"));
    }
    let handles = write_templates(doc, labels, INTERSECTION_LABEL_STYLES)?;
    add_node_child(doc, labels, INTERSECTION_LABELS[0], handles[0], "")?;
    super::labels::default_label_style(doc, INTERSECTION_LABELS).ok_or_else(|| invalid("the intersection label style was not written"))
}

/// The intersection label root, its collection and default style "Basic"
/// with its two components (the application's records, users and its own
/// name empty, dates the time of writing; the style's layer is layer 0).
#[rustfmt::skip]
const INTERSECTION_LABEL_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Null,
        children: &[(0xf00b579, 1)],
        fields: &[Run(34, "0c8400000"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Standard), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(0),
        children: &[(0x800001d5, 2)],
        fields: &[Run(34, "1e6d4003c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(2)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Standard), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "Basic", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(1),
        children: &[],
        fields: &[Run(34, "354040200"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(782, "480d555530f36a001e71401683a4b5900164001080d2034aa0222222222222c27f5400000060001ffffffff000000180007fffffffd000000180007fffffffa05543aa00000030000ffffffff40904a001fffffe0151014128007fffff8054409028")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::Null), (HardPointer, Target::Standard), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(4)), (HardPointer, Target::Record(3)), (HardOwnership, Target::Record(4)), (HardOwnership, Target::Record(3)), (SoftPointer, Target::Record(4)), (SoftPointer, Target::Record(3)), (HardPointer, Target::Null)],
        strings: &["Basic", "", "", "", "AeccDbIntersection"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_ITERATOR_STYLE",
        owner: Target::Record(2),
        children: &[],
        fields: &[Run(34, "0c4240200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(2404, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fcf1208036003480338037003680328037003a001e801e002d8018801b001c0019001b8018801c001b801980140019001a001a001c8014802e801f001600100029803a0030803a003480378037001e801e002d80198019801b0018001b801b80188019001400190018001c801b803e0019001a001b801a003e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001b001b801a003e0019001b001c001c803e0019001b8018001b003e0019001b80190018803e0019001b8018001b8014802e801f000000520282a800a00719")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(2)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Standard), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["Text For Each Intersecting Road", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(2),
        children: &[],
        fields: &[Run(34, "0c0240200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(882, "480d555531fffffe0070400683a4b5b00164001152034aa0222222222222c27f9400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fc941e002d8018801b001b801b801b801b0019001a801c80140019001a001a001c8014802e801f000000520e82a8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(2)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Standard), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["Intersection Text", "", "", ""],
    },
];

/// The parcel style collection (style tree key 2) and the feature line
/// style collection (128), each with its default style (the feature line
/// style draws the default roadway marker), when the drawing lacks them.
pub(super) fn ensure_site_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, PARCEL_STYLES, &[(KEY_PARCEL_STYLES, 0)])?;
    ensure_marker_styles(doc)?;
    ensure(doc, FEATURE_LINE_STYLES, &[(KEY_FEATURE_LINE_STYLES, 0)])
}

/// The subassembly style collection (style tree key 0x11A) and its
/// default style, when the drawing lacks them.
pub(super) fn ensure_subassembly_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, SUBASSEMBLY_STYLES, &[(KEY_SUBASSEMBLY_STYLES, 0)])
}

/// The pipe style collection (style tree key 0x116) and the structure
/// style collection (0x117), each with its default style, when the drawing
/// lacks them.
pub(super) fn ensure_pipe_network_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, PIPE_STYLES, &[(KEY_PIPE_STYLES, 0)])?;
    ensure(doc, STRUCTURE_STYLES, &[(KEY_STRUCTURE_STYLES, 0)])
}

/// The pressure pipe (style tree key 0x17E), fitting (0x17F) and
/// appurtenance (0x180) style collections with their default styles (the
/// fitting and appurtenance styles draw the default roadway marker), and
/// the pressure part list collection (0x188) with the default part list:
/// bound to the default catalog, listing a 4 inch ductile iron pipe, a 4
/// inch 11.25° elbow and a 6 inch gate valve, each with the default style
/// of its kind; when the drawing lacks them.
pub(super) fn ensure_pressure_network_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure_marker_styles(doc)?;
    ensure(doc, PRESSURE_PIPE_STYLES, &[(KEY_PRESSURE_PIPE_STYLES, 0)])?;
    ensure(doc, FITTING_STYLES, &[(KEY_FITTING_STYLES, 0)])?;
    ensure(doc, APPURTENANCE_STYLES, &[(KEY_APPURTENANCE_STYLES, 0)])?;
    ensure(doc, PRESSURE_PART_LISTS, &[(KEY_PRESSURE_PART_LISTS, 0)])
}

fn has_styles(doc: &CadDocument, key: i32) -> bool {
    root_node(doc, KEY_STYLES).and_then(|s| read_node(doc, s)).is_some_and(|t| t.children.iter().any(|c| c.0 == key))
}

/// The assembly style collection (style tree key 0x118) and the code set
/// style collection (0xEA0), each with its default style, when the drawing
/// lacks them. The default code set draws every point, link and shape
/// code with the default roadway marker, feature line, link and shape
/// styles; the link (0x11C) and shape (0x11D) style collections come with
/// it.
pub(super) fn ensure_assembly_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    if !has_styles(doc, KEY_ASSEMBLY_STYLES) {
        ensure_marker_styles(doc)?;
        ensure(doc, ASSEMBLY_STYLES, &[(KEY_ASSEMBLY_STYLES, 0)])?;
    }
    if !has_styles(doc, KEY_CODE_SET_STYLES) {
        ensure_marker_styles(doc)?;
        ensure(doc, FEATURE_LINE_STYLES, &[(KEY_FEATURE_LINE_STYLES, 0)])?;
        ensure(doc, LINK_STYLES, &[(KEY_LINK_STYLES, 0)])?;
        ensure(doc, SHAPE_STYLES, &[(KEY_SHAPE_STYLES, 0)])?;
        ensure(doc, CODE_SET_STYLES, &[(KEY_CODE_SET_STYLES, 0)])?;
    }
    Ok(())
}

/// The corridor style collection (style tree key 0xEB2) and its default
/// style, when the drawing lacks them.
pub(super) fn ensure_corridor_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, CORRIDOR_STYLES, &[(KEY_CORRIDOR_STYLES, 0)])
}

// The tables: the application's default records with the users empty and
// the dates written as fields (see `templates`).
// ponytail: the settings are constant runs; name them once a style other
// than the defaults is written.

#[rustfmt::skip]
const SECTION_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_SECTION_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "060300000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(339, "480d4814a82a12a18000000c0003fffd40600000000640550542543000000180007fffa80c00000000d54")],
        refs: &[(SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_SECTION_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(339, "480d4814a82a12a18000000c0003fffd40600000000640550542543000000180007fffa80c00000000d54")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", ""],
    },
];

#[rustfmt::skip]
const GROUP_PLOT_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_SAMPLE_LINE_GROUP_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(10, "658"), Children, Run(781, "a8800d2006042aaaaaaaaaaada9f80520061b01020000808c801818081d200604088888888888c09f82020000803c80186a0205c801810aaaaaaaaaaab6a7e02480186a02084801810222222222223027e06c801810aaaaaaaaaaab4a7e0f88000b0"), Time, Run(270, "00e400c3603e400c08555555555555a53f032400c08555555555555b53f078400058"), Time, Run(959, "480a0d2a0a84a8202400030c00feff50180000000190154150950404000061801fdfea03000000003204a82a12a08090000c3003fbfd40600000000640d50542541010000186007f7fa80c00000000c822a0a84a8600800030c00feff50180000000190554150950c01800061801fdfea030000000035554")],
        refs: &[(SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_SAMPLE_LINE_GROUP_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(781, "a8800d2006042aaaaaaaaaaada9f80520061b01020000808c801818081d200604088888888888c09f82020000803c80186a0205c801810aaaaaaaaaaab6a7e02480186a02084801810222222222223027e06c801810aaaaaaaaaaab4a7e0f88000b0"), Time, Run(270, "00e400c3603e400c08555555555555a53f032400c08555555555555b53f078400058"), Time, Run(959, "480a0d2a0a84a8202400030c00feff50180000000190154150950404000061801fdfea03000000003204a82a12a08090000c3003fbfd40600000000640d50542541010000186007f7fa80c00000000c822a0a84a8600800030c00feff50180000000190554150950c01800061801fdfea030000000035554")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
];

#[rustfmt::skip]
const SECTION_VIEW_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_SECTION_VIEW_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(10, "4cc"), Children, Run(16081, "ae1028c2022000b18088025c2021a82814ca02060014b20086a0410318084d4063180810000000000000488006b100810222222222223027e073080810000000000000088007b00080a0c0cf08088014c80230b61e002d80198019801b001b801980190019001a801400190018001c801b803e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f00000002dc202135027c20205050c4021a8104ec6021a8100eca02060213320086a0a043200810222222222222e27e0d318081405b1808c2d87800b600660066006c006e006600640064006a0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000004b180810222222222222e27e06300080a0404080002014c20230b61e002d80198019801b001b801980190019001b001400190018001c801b803e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f000000048220404088888888888c09f800ca021a84024c8021a82836c40213500ec40204088888888888c09f806c4021a80810c20204088888888888b89f812c00204088888888888c09f832c8022006f080810222222222223027e00b2808180048c80204088888888888c09f826c8020502cc802135020c6021a8282ac40205022c4021a82824c2021a8282ec002040000000000000020032c2022014708086a040131808c21c9e00cc00cc00e600ca00e8000000023100848222222222223027f409b080811bc3d0900f9b224800b308084d408300086a0a0a30008140808000202e20000299828c602200df08084d403f080810222222222223027e01f08086a02063280818081ac80204088888888888c09f83ec6020501cc60204000000000000022001ec40204088888888888b89f806c002130aaaaaaaaaaab2a7f022c002028103c22042000b1008802bc20205023c2021a82812ca02060214320086a0402b28086c14b18086a04063100810000000000000488006b080810222222222223027e07300080a080531808c2d87800b600660066006c006e006600640064006a0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c0000000f320088009c20212088888888888c0dfd04ec4021a81050c2021a8100cca02060201328086a400db20084d403b200810222222222223027e01b20086a0213318086a0a043180810222222222222e27e0d310081405b1008c2d87800b600660066006c006e006600640064006c0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000004b100810222222222222e27e05b000810222222222223027e03b00086a021288810c2d87800b600660066006c006800600068006a006e0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000007f080810222222222222e27e14888106a04118881010222222222222e27e01b280818082ac80205022c8021a82824c6021a82836c20213500ec20204088888888888c09f806c2021a80814c00230d41e002d80198019801b001b801980190018801b801400190018001c801b803e0019001a001b801a003e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001b001b801a003e0019001b001c001c803e0019001b8018001b003e0019001b80190018803e0019001b8018001b803e0019001c00180019803e0019001c0018801b8014802e801f000000010c00206020cb000810000000000000008000f0808804ac80230b61e002d80198019801b001a0018001a001a801b801400190018001c801b803e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f000000032c60220067080810000000000000488013088106a0a11b200810222222222222e27e02320084c088888888888c0dfc123180810222222222223027e09b1808140b318084d408310086a0a0ab08081408b08086a0a0b300081413f08086a040d708081405f0808c2d87800b600660066006c006e006600640064006c0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000004f080810222222222222e27e10088104d407b200810222222222222e27e0131008c2288a00d800ca00ec00c200e800d200de00dc000000023080848222222222223037f409b0008140f88000b0"), Time, Run(8738, "0518804400164011002d94040c0420640109a80c640102000000000000091000d630102044444444444604fc0e620102000000000000011000f6101020444444444445c4fc0161011000980044010e1010d414086501030009d900435020a18c0435020119404350106990040a02d9004616c3c005b0033003300360037003300320032003500280032003000390037007c0032003300380037007c0032003400330033007c0032003500390033007c0032003600300039007c0032003600350037007c0032003800300033007c00320038003100370029005d003e000000025900408111111111111713f03184040800000000000024400358004050206840001002e101184511401b0019401d8018401d001a401bc01b80000000a6201185b0f0016c00cc00cc00d800dc00cc00c800c800d800a000c800c000e400dc01f000c800cc00e000dc01f000c800d000cc00cc01f000c800d400e400cc01f000c800d800c000e401f000c800d800d400dc01f000c800e000c000cc01f000c800e000c400dc00a4017400f80000001e63011000780040c002761010d4081b630109a807630102044444444444604fc0363010d404086201020444444444445c4fc1a61010280b6101185b0f0016c00cc00cc00d800dc00cc00c800c800d800a000c800c000e400dc01f000c800cc00e000dc01f000c800d000cc00cc01f000c800d400e400cc01f000c800d800c000e401f000c800d800d400dc01f000c800e000c000cc01f000c800e000c400dc00a4017400f8000000096101020444444444445c4fc0660010d80a6001020000000000000010014e10110069800461304400720061006600740069006e006700200042007500660066006500720020004f00750074006c0069006e006500000003b8404080000000000000440041900435050558c040a0458c0435050498804350506180040800000000000000400958c04616c3c005b0033003300360034003000340035003700280032003000390037007c0032003300380037007c0032003400330033007c0032003500390033007c0032003600300039007c0032003600350037007c0032003800300033007c00320038003100370029005d003e0000000658804401f1102028026401184393c0198019801cc019401d0000000236301020444444444445c4fc046301098111111111111813f81362010212b604937ec87e4fc16620109a81061010d4141560010280ae101185b0f0016c00cc00cc00d800dc00cc00c800c800d800a000c800c000e400dc01f000c800cc00e000dc01f000c800d000cc00cc01f000c800d400e400cc01f000c800d800c000e401f000c800d800d400dc01f000c800e000c000cc01f000c800e000c400dc00a4017400f800000014640110023840408111111111111713f07d90040a039900408000000000000044003d8c0408111111111111713f0098404611445006c00650076006100740069006f006e00000004980040502005800409078400058"), Time, Run(5919, "480a4d2a0a84a8601000030c00feff50180000000190154150950c01800061801fdfea03000000003204a82a12a08030000c3003fbfd40600000000640d50542543006000186007f7fa80c00000000c822a0a84a8200c00030c00feff50180000000190554150950401800061801fdfea0300000000320ca82a12a08030000c3003fbfd40600000000641d50542543006000186007f7fa80c00000000c842a0a84a8200c00030c00feff50180000000190954150950c01800061801fdfea03000000003214a82a12a08030000c3003fbfd40600000000642d50542541006000186007f7fa80c00000000c862a0a84a8200c00030c00feff50180000000190d54150950c01000061801fdfea0300000000321ca82a12a08020000c3003fbfd40600000000643d50542541004000186007f7fa80c00000000c882a0a84a8200800030c00feff50180000000191154150950401000061801fdfea03000000003224a82a12a08020000c3003fbfd40600000000644d50542543004000186007f7fa80c00000000c8a2a0a84a8200800030c00feff50180000000191554150950c01000061801fdfea0300000000322ca82a12a08020000c3003fbfd40600000000645d50542541004000186007f7fa80c00000000c8c2a0a84a8200800030c00feff50180000000191f54150950400800061801fdfea03000000003240a82a12a08010000c3003fbfd40600000000648550542541006000186007f7fa80c00000000c912a0a84a8200c00030c00feff50180000000192354150950401800061801fdfea03000000003248a82a12a08030000c3003fbfd40600000000649550542543012000186007f7fa80c00000000c932a0a84a8202000030c00feff50180000000192754150950c04800061801fdfea03000000003250a82a12a08080000c3003fbfd4060000000064a550542543006000186007f7fa80c00000000c95aa0a84a8600c00030c00feff50180000000194954150950401800061801fdfea030000000035554")],
        refs: &[(HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbGraphCrossSection", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_SECTION_VIEW_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(16081, "ae1028c2022000b18088025c2021a82814ca02060014b20086a0410318084d4063180810000000000000488006b100810222222222223027e073080810000000000000088007b00080a0c0cf08088014c80230b61e002d80198019801b001b801980190019001a801400190018001c801b803e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f00000002dc202135027c20205050c4021a8104ec6021a8100eca02060213320086a0a043200810222222222222e27e0d318081405b1808c2d87800b600660066006c006e006600640064006a0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000004b180810222222222222e27e06300080a0404080002014c20230b61e002d80198019801b001b801980190019001b001400190018001c801b803e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f000000048220404088888888888c09f800ca021a84024c8021a82836c40213500ec40204088888888888c09f806c4021a80810c20204088888888888b89f812c00204088888888888c09f832c8022006f080810222222222223027e00b2808180048c80204088888888888c09f826c8020502cc802135020c6021a8282ac40205022c4021a82824c2021a8282ec002040000000000000020032c2022014708086a040131808c21c9e00cc00cc00e600ca00e8000000023100848222222222223027f409b080811bc3d0900f9b224800b308084d408300086a0a0a30008140808000202e20000299828c602200df08084d403f080810222222222223027e01f08086a02063280818081ac80204088888888888c09f83ec6020501cc60204000000000000022001ec40204088888888888b89f806c002130aaaaaaaaaaab2a7f022c002028103c22042000b1008802bc20205023c2021a82812ca02060214320086a0402b28086c14b18086a04063100810000000000000488006b080810222222222223027e07300080a080531808c2d87800b600660066006c006e006600640064006a0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c0000000f320088009c20212088888888888c0dfd04ec4021a81050c2021a8100cca02060201328086a400db20084d403b200810222222222223027e01b20086a0213318086a0a043180810222222222222e27e0d310081405b1008c2d87800b600660066006c006e006600640064006c0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000004b100810222222222222e27e05b000810222222222223027e03b00086a021288810c2d87800b600660066006c006800600068006a006e0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000007f080810222222222222e27e14888106a04118881010222222222222e27e01b280818082ac80205022c8021a82824c6021a82836c20213500ec20204088888888888c09f806c2021a80814c00230d41e002d80198019801b001b801980190018801b801400190018001c801b803e0019001a001b801a003e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001b001b801a003e0019001b001c001c803e0019001b8018001b003e0019001b80190018803e0019001b8018001b803e0019001c00180019803e0019001c0018801b8014802e801f000000010c00206020cb000810000000000000008000f0808804ac80230b61e002d80198019801b001a0018001a001a801b801400190018001c801b803e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f000000032c60220067080810000000000000488013088106a0a11b200810222222222222e27e02320084c088888888888c0dfc123180810222222222223027e09b1808140b318084d408310086a0a0ab08081408b08086a0a0b300081413f08086a040d708081405f0808c2d87800b600660066006c006e006600640064006c0050006400600072006e00f8006400660070006e00f8006400680066006600f80064006a0072006600f80064006c0060007200f80064006c006a006e00f8006400700060006600f8006400700062006e005200ba007c00000004f080810222222222222e27e10088104d407b200810222222222222e27e0131008c2288a00d800ca00ec00c200e800d200de00dc000000023080848222222222223037f409b0008140f88000b0"), Time, Run(8738, "0518804400164011002d94040c0420640109a80c640102000000000000091000d630102044444444444604fc0e620102000000000000011000f6101020444444444445c4fc0161011000980044010e1010d414086501030009d900435020a18c0435020119404350106990040a02d9004616c3c005b0033003300360037003300320032003500280032003000390037007c0032003300380037007c0032003400330033007c0032003500390033007c0032003600300039007c0032003600350037007c0032003800300033007c00320038003100370029005d003e000000025900408111111111111713f03184040800000000000024400358004050206840001002e101184511401b0019401d8018401d001a401bc01b80000000a6201185b0f0016c00cc00cc00d800dc00cc00c800c800d800a000c800c000e400dc01f000c800cc00e000dc01f000c800d000cc00cc01f000c800d400e400cc01f000c800d800c000e401f000c800d800d400dc01f000c800e000c000cc01f000c800e000c400dc00a4017400f80000001e63011000780040c002761010d4081b630109a807630102044444444444604fc0363010d404086201020444444444445c4fc1a61010280b6101185b0f0016c00cc00cc00d800dc00cc00c800c800d800a000c800c000e400dc01f000c800cc00e000dc01f000c800d000cc00cc01f000c800d400e400cc01f000c800d800c000e401f000c800d800d400dc01f000c800e000c000cc01f000c800e000c400dc00a4017400f8000000096101020444444444445c4fc0660010d80a6001020000000000000010014e10110069800461304400720061006600740069006e006700200042007500660066006500720020004f00750074006c0069006e006500000003b8404080000000000000440041900435050558c040a0458c0435050498804350506180040800000000000000400958c04616c3c005b0033003300360034003000340035003700280032003000390037007c0032003300380037007c0032003400330033007c0032003500390033007c0032003600300039007c0032003600350037007c0032003800300033007c00320038003100370029005d003e0000000658804401f1102028026401184393c0198019801cc019401d0000000236301020444444444445c4fc046301098111111111111813f81362010212b604937ec87e4fc16620109a81061010d4141560010280ae101185b0f0016c00cc00cc00d800dc00cc00c800c800d800a000c800c000e400dc01f000c800cc00e000dc01f000c800d000cc00cc01f000c800d400e400cc01f000c800d800c000e401f000c800d800d400dc01f000c800e000c000cc01f000c800e000c400dc00a4017400f800000014640110023840408111111111111713f07d90040a039900408000000000000044003d8c0408111111111111713f0098404611445006c00650076006100740069006f006e00000004980040502005800409078400058"), Time, Run(5919, "480a4d2a0a84a8601000030c00feff50180000000190154150950c01800061801fdfea03000000003204a82a12a08030000c3003fbfd40600000000640d50542543006000186007f7fa80c00000000c822a0a84a8200c00030c00feff50180000000190554150950401800061801fdfea0300000000320ca82a12a08030000c3003fbfd40600000000641d50542543006000186007f7fa80c00000000c842a0a84a8200c00030c00feff50180000000190954150950c01800061801fdfea03000000003214a82a12a08030000c3003fbfd40600000000642d50542541006000186007f7fa80c00000000c862a0a84a8200c00030c00feff50180000000190d54150950c01000061801fdfea0300000000321ca82a12a08020000c3003fbfd40600000000643d50542541004000186007f7fa80c00000000c882a0a84a8200800030c00feff50180000000191154150950401000061801fdfea03000000003224a82a12a08020000c3003fbfd40600000000644d50542543004000186007f7fa80c00000000c8a2a0a84a8200800030c00feff50180000000191554150950c01000061801fdfea0300000000322ca82a12a08020000c3003fbfd40600000000645d50542541004000186007f7fa80c00000000c8c2a0a84a8200800030c00feff50180000000191f54150950400800061801fdfea03000000003240a82a12a08010000c3003fbfd40600000000648550542541006000186007f7fa80c00000000c912a0a84a8200c00030c00feff50180000000192354150950401800061801fdfea03000000003248a82a12a08030000c3003fbfd40600000000649550542543012000186007f7fa80c00000000c932a0a84a8202000030c00feff50180000000192754150950c04800061801fdfea03000000003250a82a12a08080000c3003fbfd4060000000064a550542543006000186007f7fa80c00000000c95aa0a84a8600c00030c00feff50180000000194954150950401800061801fdfea030000000035554")],
        refs: &[(HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (HardPointer, Target::Standard), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbGraphCrossSection", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_SECTION_VIEW_BAND_STYLE_SET",
        owner: Target::Null,
        children: &[(0x1e, 3)],
        fields: &[Run(34, "038300000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(32, "480d5555")],
        refs: &[(SoftPointer, Target::Record(3))],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_SECTION_VIEW_BAND_STYLE_SET",
        owner: Target::Record(2),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(32, "480d5555")],
        refs: &[(SoftPointer, Target::Null)],
        strings: &["", "", ""],
    },
];

#[rustfmt::skip]
const MARKER_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Null,
        children: &[(0x800001e8, 13), (0x800001e9, 12), (0x800001ea, 11), (0x800001e3, 10), (0x800001eb, 9), (0x800001e4, 8), (0x800001ec, 7), (0x800001e5, 6), (0x800001ed, 5), (0x800001e6, 4), (0x1e, 3), (0x800001ee, 2), (0x800001e7, 1)],
        fields: &[Run(34, "06c040000"), Children, Run(157, "a828102000080808000201ea40006000f88000b0"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601800030c00ffff50180000000190195054254300c000186007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Record(3)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "af999ab1-14da-4d64-9cae-432463a0f8a9", "0b00cab8-d6fd-417d-b94a-6bba0e7e9c77", "06d063c2-a4fe-46b4-b3fe-8374593cd1cb", "19ef784e-d7df-4138-9938-f586c0532493", "0a2a4d2d-9338-4516-a90a-1722698ef99d", "09d6aa73-fc1d-4658-81a8-19f45d6f891b", "1d009f3c-397f-453a-bd25-f718e1113aef", "3760b186-c385-49a6-9bfc-10a5807bf9ee", "22fc4507-0d76-4346-9aad-6300c8eaa99c", "44ef4c7c-5993-411f-bcc4-35281ac38136", "f82937d8-e0d8-4f5e-bcd5-b8195ba2d10d", "336ef678-ebcd-418f-8013-164afd92c67b", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "39c040200"), Children, Run(267, "a838102000080808000202e20000200c0020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601800030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["336ef678-ebcd-418f-8013-164afd92c67b", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "3b8040200"), Children, Run(267, "a838102000080808000202e20000201c0020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601400030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["f82937d8-e0d8-4f5e-bcd5-b8195ba2d10d", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(157, "a828102000080808000201ea40006000f88000b0"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601800030c00ffff50180000000190195054254300c000186007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "398040200"), Children, Run(267, "a838102000080808000202e2000020240020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601000030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea0300000000355243000000000000048800406a88a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["44ef4c7c-5993-411f-bcc4-35281ac38136", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "3b4040200"), Children, Run(267, "a838102000080808000202e20000202c0020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8600400030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["22fc4507-0d76-4346-9aad-6300c8eaa99c", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "394040200"), Children, Run(267, "a838102000080808000202e20000200d8038801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601000030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea0300000000355247000000000000068800402a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["3760b186-c385-49a6-9bfc-10a5807bf9ee", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "3b0040200"), Children, Run(267, "a838102000080808000202e2000020140020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8600c00030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea0300000000355243000000000000048800406a88a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["1d009f3c-397f-453a-bd25-f718e1113aef", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "390040200"), Children, Run(267, "a838102000080808000202e20000201c0020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601400030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["09d6aa73-fc1d-4658-81a8-19f45d6f891b", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "3ac040200"), Children, Run(267, "a838102000080808000202e2000020140020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8600c00030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea0300000000355243000000000000048800406a88a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["0a2a4d2d-9338-4516-a90a-1722698ef99d", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "38c040200"), Children, Run(267, "a838102000080808000202e20000200d8038801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601000030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea0300000000355247000000000000068800402a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["19ef784e-d7df-4138-9938-f586c0532493", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "3a8040200"), Children, Run(267, "a838102000080808000202e20000200c0020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601800030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["06d063c2-a4fe-46b4-b3fe-8374593cd1cb", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "3a4040200"), Children, Run(267, "a838102000080808000202e2000020240020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8601000030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea0300000000355243000000000000048800406a88a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["0b00cab8-d6fd-417d-b94a-6bba0e7e9c77", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYMARKER_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(34, "3a0040200"), Children, Run(267, "a838102000080808000202e20000202c0020801ea4000600150800018083e20002c"), Time, Run(34, "078400058"), Time, Run(767, "480a032a0a84a8600400030c00ffff501800000001901950542543000000180007fffa80c00000000c80ca82a12a18060000c3003fffd40600000000640654150950c03000061801fffea03000000003552430222222222223027e0002a80a02")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["af999ab1-14da-4d64-9cae-432463a0f8a9", "", "", "", "", "", "", "", "", "", "", ""],
    },
];

#[rustfmt::skip]
const INTERSECTION_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_INTERSECTION_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "0c4400000"), Children, Run(149, "a8281020000808080002003600e200f88000b0"), Time, Run(34, "078400058"), Time, Run(29, "480d5550")],
        refs: &[(HardPointer, Target::Style(0x11B, -2147483165)), (SoftPointer, Target::Record(1))],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_INTERSECTION_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(149, "a8281020000808080002003600e200f88000b0"), Time, Run(34, "078400058"), Time, Run(29, "480d5550")],
        refs: &[(HardPointer, Target::Style(0x11B, -2147483163)), (SoftPointer, Target::Null)],
        strings: &["", "", ""],
    },
];

#[rustfmt::skip]
const PARCEL_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_PARCEL_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(10, "408"), Children, Run(325, "a8401020000806c800018081520000807480001000000000000028800808000201f2000200f88000b0"), Time, Run(34, "078400058"), Time, Run(1308, "480a052a0a84a8600000030000ffff50180000000190154150950400000060001fffea030000000032052a0a84a8600000030000ffff50180000000190154150950400000060001fffea03000000003480a04a82a12a18000000c1003fffd40600000000640654150950c00000060801fdfea030000000032a10000000000001f87f151501950542543000000182007f7fa80c00000000ca840000000000007e1fc545a")],
        refs: &[(HardPointer, Target::Null), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "<[167804933(2449)]> : <[16809985(2593|2803|2817)]>", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31"],
    },
    Template {
        dxf: "AECC_PARCEL_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(325, "a8401020000806c800018081520000807480001000000000000028800808000201f2000200f88000b0"), Time, Run(34, "078400058"), Time, Run(1308, "480a052a0a84a8600000030000ffff50180000000190154150950400000060001fffea030000000032052a0a84a8600000030000ffff50180000000190154150950400000060001fffea03000000003480a04a82a12a18000000c1003fffd40600000000640654150950c00000060801fdfea030000000032a10000000000001f87f151501950542543000000182007f7fa80c00000000ca840000000000007e1fc545a")],
        refs: &[(HardPointer, Target::Null), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "<[167804933(2449)]> : <[16809985(2593|2803|2817)]>", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31"],
    },
];

#[rustfmt::skip]
const FEATURE_LINE_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_FEATURELINE_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(10, "600"), Children, Run(183, "a840102000080140018782020000800c0018783e20002c"), Time, Run(102, "00e000c3c0080030f078400058"), Time, Run(1122, "480a032a0a84a8600000030000ffff501800000001901950542543000000180007fffa80c00000000c824a82a12a18000000c0003fffd40600000000640950542543000000180007fffa80c00000000c81aa0a84a8600000030000ffff50180000000190454150950c00000060001fffea0300000000320280aa0a84a8600000030400ffff501800000001aa8")],
        refs: &[(SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_FEATURELINE_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(183, "a840102000080140018782020000800c0018783e20002c"), Time, Run(102, "00e000c3c0080030f078400058"), Time, Run(1122, "480a032a0a84a8600000030000ffff501800000001901950542543000000180007fffa80c00000000c824a82a12a18000000c0003fffd40600000000640950542543000000180007fffa80c00000000c81aa0a84a8600000030000ffff50180000000190454150950c00000060001fffea0300000000320280aa0a84a8600000030400ffff501800000001aa8")],
        refs: &[(SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Style(0x11B, 30)), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
];

const SUBASSEMBLY_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_SUBASSEMBLY_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "068040000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1889, "480a0d2a0a84a8601800030c00ffff50180000000190154150950c01800061801fffea03000000003204a82a12a18060000c3003fffd40600000000640d50542543006000186007fffa80c00000000c822a0a84a8601800030c00ffff50180000000190554150950c01800061801fffea0300000000320d2a0a84a8601800030c00ffff50180000000190154150950c01800061801fffea03000000003204a82a12a18060000c3003fffd40600000000640d50542543006000186007fffa80c00000000c822a0a84a8601800030c00ffff50180000000190554150950c01800061801fffea030000000035550")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_SUBASSEMBLY_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1889, "480a0d2a0a84a8601800030c00ffff50180000000190154150950c01800061801fffea03000000003204a82a12a18060000c3003fffd40600000000640d50542543006000186007fffa80c00000000c822a0a84a8601800030c00ffff50180000000190554150950c01800061801fffea0300000000320d2a0a84a8601800030c00ffff50180000000190154150950c01800061801fffea03000000003204a82a12a18060000c3003fffd40600000000640d50542543006000186007fffa80c00000000c822a0a84a8601800030c00ffff50180000000190554150950c01800061801fffea030000000035550")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
];

#[rustfmt::skip]
const PIPE_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_PIPE_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "058040000"), Children, Run(1237, "a92834a2081a80806a208044d4cccccccccd49f8102000080ba88206c002882011353333333333527e14288206c162882018003ea20804828a2081a81038a2080484ca208060011288206a0208080002054a2080480aa208044d4cccccccccd49f832a2081b004a208044d4cccccccccd49f82ca2081a81026a2081a8103ca20804848a2081a80836a2080484aa208060010a88206a020f88000b0"), Time, Run(666, "0114410089a9999999999a93f061441035010054410089a9999999999a93f0a5441035030b544100c001551040d4082051040241d5104024495104030008d441035010ad4410089a9999999999a93f078400058"), Time, Run(4703, "480a0d2a0a84a8200000030000ffff50180000000190154150950c00000060001fffea03000000003204a82a12a18000000c0003fffd40600000000640d50542543000000180007fffa80c00000000c822a0a84a8200000030000ffff50180000000190554150950400000060001fffea0300000000320282aa0a84a8600000030000ffff50180000000190a950542541000000180007fffa80c00000000c80aa0a84a8600000030000ffff50180000000190254150950c00000060001fffea03000000003206a82a12a18000000c0003fffd40600000000641150542541000000180007fffa80c00000000c832a0a84a8200000030000ffff50180000000190754150950400000060001fffea03000000003210a82a12a08000000c0003fffd40600000000642550542541000000180007fffa80c00000000c852a0a84a8200000030000ffff50180000000190541950542543000000180007fffa80c00000000c83aa0a84a8600000030000ffff50180000000190854150950400000060001fffea03000000003218a82a12a18000000c0003fffd40600000000643550542541000000180007fffa80c00000000c80ca82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a2c814a82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a2a02a82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a2a0480aa0a84a8600000030400feff5018000000019509060b51153ec87a4fca8a812a0a84a8600000030400feff5018000000019509060b51153ec87a4fca8a")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
    Template {
        dxf: "AECC_PIPE_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(1237, "a92834a2081a80806a208044d4cccccccccd49f8102000080ba88206c002882011353333333333527e14288206c162882018003ea20804828a2081a81038a2080484ca208060011288206a0208080002054a2080480aa208044d4cccccccccd49f832a2081b004a208044d4cccccccccd49f82ca2081a81026a2081a8103ca20804848a2081a80836a2080484aa208060010a88206a020f88000b0"), Time, Run(666, "0114410089a9999999999a93f061441035010054410089a9999999999a93f0a5441035030b544100c001551040d4082051040241d5104024495104030008d441035010ad4410089a9999999999a93f078400058"), Time, Run(4703, "480a0d2a0a84a8200000030000ffff50180000000190154150950c00000060001fffea03000000003204a82a12a18000000c0003fffd40600000000640d50542543000000180007fffa80c00000000c822a0a84a8200000030000ffff50180000000190554150950400000060001fffea0300000000320282aa0a84a8600000030000ffff50180000000190a950542541000000180007fffa80c00000000c80aa0a84a8600000030000ffff50180000000190254150950c00000060001fffea03000000003206a82a12a18000000c0003fffd40600000000641150542541000000180007fffa80c00000000c832a0a84a8200000030000ffff50180000000190754150950400000060001fffea03000000003210a82a12a08000000c0003fffd40600000000642550542541000000180007fffa80c00000000c852a0a84a8200000030000ffff50180000000190541950542543000000180007fffa80c00000000c83aa0a84a8600000030000ffff50180000000190854150950400000060001fffea03000000003218a82a12a18000000c0003fffd40600000000643550542541000000180007fffa80c00000000c80ca82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a2c814a82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a2a02a82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a2a0480aa0a84a8600000030400feff5018000000019509060b51153ec87a4fca8a812a0a84a8600000030400feff5018000000019509060b51153ec87a4fca8a")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
];

#[rustfmt::skip]
const STRUCTURE_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_STUCTURE_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "05c040000"), Children, Run(1187, "a9586ec208044d4cccccccccd49f81020000803b08206c05b0820180874c2081b060c2082018b08201209308206a040eb08201208080002012c2080481ec2081b034c208044d4cccccccccd49f82ec20804872c208044d4cccccccccd49f828c208044d4cccccccccd49f86cc2080483cc208060019b0820120030820740100006180016c20804822c2081b038c2080483e20002c"), Time, Run(616, "0398410360658410090d184100c04156104100e18410090218410350503184100c041361040243b61040d40c1661040d4083261040243561040d4080a61040241b61040241061040d81e100016"), Time, Run(2501, "0198410400d61040d81861040226a6666666666a4fd20281ca82a12a18000000c0003fbfd40600000000640550542541000000180007f7fa80c00000000c812a0a84a8200000030000feff50180000000190140950542543000000180007f7fa80c00000000c81ca82a12a18000000c0003fbfd40600000000640550542541000000180007f7fa80c00000000c81aa0a84a8200000030000feff501800000001903950542543000000180007f7fa80c00000000c80aa0a84a8200000030000feff50180000000190354150950400000060001fdfea030000000032032a0a84a8600000030400feff5018000000019509060b51153ec87a4fca8b2032a0a84a8600000030400feff5018000000019509060b51153ec87a4fca8a80ca82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a28")],
        refs: &[(HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
    Template {
        dxf: "AECC_STUCTURE_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(1187, "a9586ec208044d4cccccccccd49f81020000803b08206c05b0820180874c2081b060c2082018b08201209308206a040eb08201208080002012c2080481ec2081b034c208044d4cccccccccd49f82ec20804872c208044d4cccccccccd49f828c208044d4cccccccccd49f86cc2080483cc208060019b0820120030820740100006180016c20804822c2081b038c2080483e20002c"), Time, Run(616, "0398410360658410090d184100c04156104100e18410090218410350503184100c041361040243b61040d40c1661040d4083261040243561040d4080a61040241b61040241061040d81e100016"), Time, Run(2501, "0198410400d61040d81861040226a6666666666a4fd20281ca82a12a18000000c0003fbfd40600000000640550542541000000180007f7fa80c00000000c812a0a84a8200000030000feff50180000000190140950542543000000180007f7fa80c00000000c81ca82a12a18000000c0003fbfd40600000000640550542541000000180007f7fa80c00000000c81aa0a84a8200000030000feff501800000001903950542543000000180007f7fa80c00000000c80aa0a84a8200000030000feff50180000000190354150950400000060001fdfea030000000032032a0a84a8600000030400feff5018000000019509060b51153ec87a4fca8b2032a0a84a8600000030400feff5018000000019509060b51153ec87a4fca8a80ca82a12a18000000c1003fbfd4060000000065424182d4454fb21e93f2a28")],
        refs: &[(HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
];

#[rustfmt::skip]
const ASSEMBLY_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_ASSEMBLY_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "060040000"), Children, Run(217, "a8501020000801c0020802020000802c002080050008200f88000b0"), Time, Run(138, "00200103601200104001800410078400058"), Time, Run(29, "480d5550")],
        refs: &[(HardPointer, Target::Style(0x11B, -2147483161)), (HardPointer, Target::Style(0x11B, -2147483154)), (HardPointer, Target::Style(0x11B, -2147483162)), (HardPointer, Target::Style(0x11B, -2147483157)), (HardPointer, Target::Style(0x11B, -2147483155)), (SoftPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_ASSEMBLY_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(217, "a8501020000801c0020802020000802c002080050008200f88000b0"), Time, Run(138, "00200103601200104001800410078400058"), Time, Run(29, "480d5550")],
        refs: &[(HardPointer, Target::Style(0x11B, -2147483156)), (HardPointer, Target::Style(0x11B, -2147483158)), (HardPointer, Target::Style(0x11B, -2147483160)), (HardPointer, Target::Style(0x11B, -2147483165)), (HardPointer, Target::Style(0x11B, -2147483164)), (SoftPointer, Target::Null)],
        strings: &["", "", ""],
    },
];

#[rustfmt::skip]
const LINK_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_ROADWAYLINK_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "070040000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(494, "480a032a0a84a8626400030c00ffff501800000001901950542543132000186007fffa80c00000000d2032a0a84a8626400030c00ffff501800000001aa8")],
        refs: &[(SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAYLINK_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(494, "480a032a0a84a8626400030c00ffff501800000001901950542543132000186007fffa80c00000000d2032a0a84a8626400030c00ffff501800000001aa8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", ""],
    },
];

#[rustfmt::skip]
const SHAPE_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_ROADWAYSHAPE_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "074040000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(2037, "480a052a0a84a860d400030c00ffff50180000000190154150950c7e000061801fffea030000000032052a0a84a860d400030c00ffff50180000000190154150950c7e000061801fffea030000000032052a0a84a820d400030c00ffff50180000000190154150950c7e000061801fffea030000000032052a0a84a860d400030c00ffff50180000000190154150950c7e000061801fffea030000000032032a0a84a8600000030400feff50180000000195040d8a8a80ca82a12a18000000c1003fbfd4060000000065410362a2a032a0a84a8600000030400feff50180000000195040d8a8a80ca82a12a18000000c1003fbfd4060000000065410362a28")],
        refs: &[(SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
    Template {
        dxf: "AECC_ROADWAYSHAPE_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(2037, "480a052a0a84a860d400030c00ffff50180000000190154150950c7e000061801fffea030000000032052a0a84a860d400030c00ffff50180000000190154150950c7e000061801fffea030000000032052a0a84a820d400030c00ffff50180000000190154150950c7e000061801fffea030000000032052a0a84a860d400030c00ffff50180000000190154150950c7e000061801fffea030000000032032a0a84a8600000030400feff50180000000195040d8a8a80ca82a12a18000000c1003fbfd4060000000065410362a2a032a0a84a8600000030400feff50180000000195040d8a8a80ca82a12a18000000c1003fbfd4060000000065410362a28")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
];

#[rustfmt::skip]
const CODE_SET_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_ROADWAY_STYLE_SET",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "280380000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(119, "480d5552034815207480d205481d54")],
        refs: &[(SoftPointer, Target::Record(1)), (HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (HardPointer, Target::Style(0x80, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11C, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11D, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (HardPointer, Target::Style(0x80, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11C, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11D, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_ROADWAY_STYLE_SET",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(119, "480d5552034815207480d205481d54")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (HardPointer, Target::Style(0x80, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11C, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11D, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (HardPointer, Target::Style(0x80, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11C, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11D, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
];

#[rustfmt::skip]
const CORRIDOR_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_CORRIDOR_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "2c8380000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1269, "480a092a0a84a8200000030000ffff50180000000190154150950400000060001fffea03000000003204a82a12a08000000c0003fffd40600000000640d50542541000000180007fffa80c00000000c824a82a12a08000000c0003fffd40600000000640550542541000000180007fffa80c00000000c812a0a84a8200000030000ffff50180000000190354150950400000060001fffea030000000035550")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
    Template {
        dxf: "AECC_CORRIDOR_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1269, "480a092a0a84a8200000030000ffff50180000000190154150950400000060001fffea03000000003204a82a12a08000000c0003fffd40600000000640d50542541000000180007fffa80c00000000c824a82a12a08000000c0003fffd40600000000640550542541000000180007fffa80c00000000c812a0a84a8200000030000ffff50180000000190354150950400000060001fffea030000000035550")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
    },
];

#[rustfmt::skip]
const PRESSURE_PIPE_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_PRESSURE_PIPE_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "1f8040000"), Children, Run(547, "a89834a2081a808102000080ba88206c0fa8820120a288206a040e288201213288201800202000080ca88206c09a88206a040f28820120da88201212a882018003e20002c"), Time, Run(194, "0614410350105544103502081441009075441009078400058"), Time, Run(3672, "480a0d2a0a84a8200000030000ffff50180000000190154150950c00000060001fffea03000000003204a82a12a18000000c0003fffd40600000000640d50542543000000180007fffa80c00000000c822a0a84a8200000030000ffff50180000000190554150950400000060001fffea0300000000320282aa0a84a8600000030000ffff501800000001908950542541000000180007fffa80c00000000c80aa0a84a8600000030000ffff50180000000190254150950c00000060001fffea03000000003206a82a12a18000000c0003fffd40600000000641150542541000000180007fffa80c00000000c832a0a84a8200000030000ffff50180000000190754150950400000060001fffea03000000003210a82a12a08000000c0003fffd4060000000064090454150950400000060001fffea03000000003216a82a12a18000000c0003fffd40600000000640654150950c00000060801fdfea030000000032a120c16a22a7d90f49f951640a54150950c00000060801fdfea030000000032a120c16a22a7d90f49f95150154150950c00000060801fdfea030000000032a120c16a22a7d90f49f951501950542543000000182007f7fa80c00000000ca848305a88a9f643d27e545")],
        refs: &[(SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
    Template {
        dxf: "AECC_PRESSURE_PIPE_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(547, "a89834a2081a808102000080ba88206c0fa8820120a288206a040e288201213288201800202000080ca88206c09a88206a040f28820120da88201212a882018003e20002c"), Time, Run(194, "0614410350105544103502081441009075441009078400058"), Time, Run(3672, "480a0d2a0a84a8200000030000ffff50180000000190154150950c00000060001fffea03000000003204a82a12a18000000c0003fffd40600000000640d50542543000000180007fffa80c00000000c822a0a84a8200000030000ffff50180000000190554150950400000060001fffea0300000000320282aa0a84a8600000030000ffff501800000001908950542541000000180007fffa80c00000000c80aa0a84a8600000030000ffff50180000000190254150950c00000060001fffea03000000003206a82a12a18000000c0003fffd40600000000641150542541000000180007fffa80c00000000c832a0a84a8200000030000ffff50180000000190754150950400000060001fffea03000000003210a82a12a08000000c0003fffd4060000000064090454150950400000060001fffea03000000003216a82a12a18000000c0003fffd40600000000640654150950c00000060801fdfea030000000032a120c16a22a7d90f49f951640a54150950c00000060801fdfea030000000032a120c16a22a7d90f49f95150154150950c00000060801fdfea030000000032a120c16a22a7d90f49f951501950542543000000182007f7fa80c00000000ca848305a88a9f643d27e545")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
];

#[rustfmt::skip]
const FITTING_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_FITTING_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "1fc040000"), Children, Run(705, "a8e01f22120481020000800c8848801922120480b22121b02722121a810202000080a488481201c8848120bc8848800d22122008c884818081d22120480f22121a818172212044d4cccccccccd49f82d221206000f88000b0"), Time, Run(500, "00a4424090224424401091090240991090d4080d91090226a6666666666a4fc0a91090241591090226a6666666666a4fc0491090240091090d4041e100016"), Time, Run(1780, "04a44240c01202814a82a12a18000000c0003fbfd40600000000640950542541000000180007f7fa80c00000000c80ca82a12a18000000c0003fbfd40600000000640a54150950c00000060001fdfea03000000003204a82a12a08000000c0003fbfd4060000000064090154150950c00000060001fffea03000000003204a82a12a08000000c0003fffd40600000000640654150950c00000060801fdfea030000000032a13151640654150950c00000060801fdfea030000000032a13151501950542543000000182007f7fa80c00000000ca848305a88a9f643d27e545")],
        refs: &[(HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
    Template {
        dxf: "AECC_FITTING_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(705, "a8e01f22120481020000800c8848801922120480b22121b02722121a810202000080a488481201c8848120bc8848800d22122008c884818081d22120480f22121a818172212044d4cccccccccd49f82d221206000f88000b0"), Time, Run(500, "00a4424090224424401091090240991090d4080d91090226a6666666666a4fc0a91090241591090226a6666666666a4fc0491090240091090d4041e100016"), Time, Run(1780, "04a44240c01202814a82a12a18000000c0003fbfd40600000000640950542541000000180007f7fa80c00000000c80ca82a12a18000000c0003fbfd40600000000640a54150950c00000060001fdfea03000000003204a82a12a08000000c0003fbfd4060000000064090154150950c00000060001fffea03000000003204a82a12a08000000c0003fffd40600000000640654150950c00000060801fdfea030000000032a13151640654150950c00000060801fdfea030000000032a13151501950542543000000182007f7fa80c00000000ca848305a88a9f643d27e545")],
        refs: &[(HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
];

#[rustfmt::skip]
const APPURTENANCE_STYLES: &[Template] = &[
    Template {
        dxf: "AECC_APPURTENANCE_STYLE",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "200040000"), Children, Run(657, "a8d00d62122008d884818081020000807588481203d88486a0605d884811353333333333527e0808000200562120481162122008588481204d88486a0406d884811353333333333527e0558848120f88000b0"), Time, Run(442, "056c424089a9999999999a93f012c42409002c424350104ac4240c000fb10902401b10910032c42409016c4243604ec4243502078400058"), Time, Run(1810, "052c4240900ec42409480a052a0a84a8600000030000feff50180000000190254150950400000060001fdfea030000000032032a0a84a8600000030000feff501800000001902950542543000000180007f7fa80c00000000c812a0a84a8200000030000feff50180000000190240550542543000000180007fffa80c00000000c812a0a84a8200000030000ffff501800000001901950542543000000182007f7fa80c00000000ca84c545901950542543000000182007f7fa80c00000000ca84c54540654150950c00000060801fdfea030000000032a120c16a22a7d90f49f9514")],
        refs: &[(HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (SoftPointer, Target::Record(1)), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
    Template {
        dxf: "AECC_APPURTENANCE_STYLE",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(657, "a8d00d62122008d884818081020000807588481203d88486a0605d884811353333333333527e0808000200562120481162122008588481204d88486a0406d884811353333333333527e0558848120f88000b0"), Time, Run(442, "056c424089a9999999999a93f012c42409002c424350104ac4240c000fb10902401b10910032c42409016c4243604ec4243502078400058"), Time, Run(1810, "052c4240900ec42409480a052a0a84a8600000030000feff50180000000190254150950400000060001fdfea030000000032032a0a84a8600000030000feff501800000001902950542543000000180007f7fa80c00000000c812a0a84a8200000030000feff50180000000190240550542543000000180007fffa80c00000000c812a0a84a8200000030000ffff501800000001901950542543000000182007f7fa80c00000000ca84c545901950542543000000182007f7fa80c00000000ca84c54540654150950c00000060801fdfea030000000032a120c16a22a7d90f49f9514")],
        refs: &[(HardPointer, Target::Style(0x11B, 30)), (HardPointer, Target::Null), (HardPointer, Target::Null), (SoftPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "", "ANSI31", "", "", "ANSI31", "", "", "ANSI31"],
    },
];

#[rustfmt::skip]
const PRESSURE_PART_LISTS: &[Template] = &[
    Template {
        dxf: "AECC_PRESSURE_PART_LIST",
        owner: Target::Null,
        children: &[(0x1e, 1)],
        fields: &[Run(34, "220040000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(37, "480d555550")],
        refs: &[(SoftPointer, Target::Null)],
        strings: &["", "", "", ""],
    },
    Template {
        dxf: "AECC_PRESSURE_PART_LIST",
        owner: Target::Record(0),
        children: &[],
        fields: &[Run(2649, "47aa8303884123281836841233036841233036841233010200008080800020308412328302e84123302e84123302e84123302e84123302e84123302e84123303e20002ca1c84890e2efba020f08000b2872122438bbee80a406aaa9014390154150416540be049cca088122c814a817c092d1411010000000000000c901029502f8125a2822032052a05f024b5504404000000000000144040a540be04ac8220281b2c80aa0a82092a05f024e65044091640a540be0496aa088080000000001004c80814a817c092d54110100000000000005101029502f812b20880b2f333202e4202a82a0824a817c0905a034110440a540be0496aa088080000000000002880814a817c095904406540be049cca088122c8120c872033994110245902659049cca088122c8132e824e65044091640999412732822048b204e420b05a0341104409c84160b40682208810")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Style(0x17E, 30)), (HardPointer, Target::ByLayerMaterial), (HardPointer, Target::Style(0x17F, 30)), (HardPointer, Target::ByLayerMaterial), (HardPointer, Target::Style(0x180, 30)), (HardPointer, Target::ByLayerMaterial)],
        strings: &["", "", "", "611746E2-B19E-4F8E-ACF7-F12491879819", "7ea43c7c-83bb-48f4-9b18-8b9c1a5f250e", "7ea43c7c-83bb-48f4-9b18-8b9c1a5f250e", "pipe-4 in-push on-ductile iron-350 psi-AWWA C149", "", "AeccNetworkPartDataField", "", "ID_MATERIAL", "Id Material", "ID_MATERIAL", "", "AeccNetworkPartDataField", "", "CUT_LENGTH", "Cut Length", "CUT_LENGTH", "ft", "AeccNetworkPartDataField", "", "MIN_FLEX_RADIUS", "Minimum Flex Radius", "MIN_FLEX_RADIUS", "ft", "AeccNetworkPartDataField", "", "DEFLECTION", "Deflection", "DEFLECTION", "deg", "AeccNetworkPartDataField", "", "", "", "PRESSURE_PART_CACHED_NDS", "", "4.000000,4.000000", "4ec98468-cb87-435a-89bc-cebf9c6392bb", "05a4ab2d-c9d0-428a-a506-0f8a58ee8da1", "elbow-4 in-11.25 degree-push on-ductile iron-350 psi-AWWA C111/C152", "", "AeccNetworkPartDataField", "", "ID_MATERIAL", "Id Material", "ID_MATERIAL", "", "AeccNetworkPartDataField", "", "ANGLE", "Angle", "ANGLE", "deg", "AeccNetworkPartDataField", "", "DEFLECTION", "Deflection", "DEFLECTION", "deg", "AeccNetworkPartDataField", "", "", "", "PRESSURE_PART_CACHED_NDS", "", "4.000000,4.000000", "2821bd9a-296d-4bc4-8fef-e0b93833dd38", "c0cbab87-d7f1-458f-b4df-d1052bb936eb", "gate valve-6 inch-push on-ductile iron-200 psi-AWWA C111/C500", "", "AeccNetworkPartDataField", "", "PART_FAMILY_NAME", "Part Family Name", "PART_FAMILY_NAME", "", "gate valve-push on-ductile iron-200 psi", "AeccNetworkPartDataField", "", "DEFLECTION", "Deflection", "DEFLECTION", "deg", "AeccNetworkPartDataField", "", "", "", "PRESSURE_PART_CACHED_NDS", "", "6.000000,6.000000", "AeccNetworkPartDataField", "", "ID_MATERIAL", "Id Material", "ID_MATERIAL", "", "ID_MATERIAL", "Id Material", "ID_MATERIAL", "", "ductile iron", "ID_MATERIAL", "Id Material", "ID_MATERIAL", "", "ductile iron", "ID_MATERIAL", "Id Material", "ID_MATERIAL", "", "ductile iron", "ID_MATERIAL", "Id Material", "ID_MATERIAL", "", "ductile iron", "PART_FAMILY_NAME", "Part Family Name", "PART_FAMILY_NAME", "", "gate valve-push on-ductile iron-200 psi", "", "PART_FAMILY_NAME", "Part Family Name", "PART_FAMILY_NAME", "", "gate valve-push on-ductile iron-150 psi", ""],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::objects::ObjectType;
    use crate::types::{DxfVersion, Handle};

    /// Handles and owners of a family's records, in object order.
    fn family(doc: &CadDocument, dxf: &str) -> Vec<(Handle, Handle)> {
        doc.objects
            .iter()
            .filter_map(|(h, o)| match o {
                ObjectType::Arch(a) if a.dxf_name == dxf => Some((*h, a.owner)),
                _ => None,
            })
            .collect()
    }

    /// Every style family's first record is its collection root and
    /// carries the family's own DXF class, so a class-instance census
    /// counts the root plus the styles, while Civil 3D's style
    /// collections enumerate only a root's children (see
    /// `write_templates`). Pins that shape on the round-trip of the
    /// parcel, feature line, marker and corridor defaults.
    #[test]
    fn style_collection_roots_share_their_family_s_style_class() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        ensure_site_styles(&mut doc).unwrap();
        ensure_corridor_styles(&mut doc).unwrap();
        let path = std::env::temp_dir().join(format!("civil-style-roots-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);

        let styles_node = root_node(&back, KEY_STYLES).unwrap();
        for (dxf, table) in [
            ("AECC_PARCEL_STYLE", PARCEL_STYLES),
            ("AECC_FEATURELINE_STYLE", FEATURE_LINE_STYLES),
            ("AECC_ROADWAYMARKER_STYLE", MARKER_STYLES),
            ("AECC_CORRIDOR_STYLE", CORRIDOR_STYLES),
        ] {
            let records = family(&back, dxf);
            assert_eq!(records.len(), table.len(), "{dxf}: census");
            let roots: Vec<Handle> = records.iter().filter(|(_, owner)| *owner == styles_node).map(|(h, _)| *h).collect();
            assert_eq!(roots.len(), 1, "{dxf}: one collection root");
            for (h, owner) in &records {
                if *h != roots[0] {
                    assert_eq!(owner, &roots[0], "{dxf}: {h:?} is owned by the root");
                }
            }
        }
    }
}
