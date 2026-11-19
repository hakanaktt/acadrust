//! The default section, section view, intersection, roadway marker,
//! parcel and feature line styles of a new drawing.

use super::support::*;
use super::templates::{write_templates, Piece::{Children, Run, Time}, Target, Template};
use super::tree::{ensure_point_tree, KEY_STYLES};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType::{HardPointer, SoftPointer};
use crate::CadDocument;

/// Style tree keys: section styles, section view styles, section view band
/// sets, intersection styles and roadway marker styles.
pub(super) const KEY_SECTION_STYLES: i32 = 0xC18;
pub(super) const KEY_SECTION_VIEW_STYLES: i32 = 0x33;
const KEY_SECTION_VIEW_BAND_SETS: i32 = 0xC0E;
pub(super) const KEY_INTERSECTION_STYLES: i32 = 0x1031;
const KEY_MARKER_STYLES: i32 = 0x11B;
/// Style tree keys of the parcel style and feature line style collections.
pub(super) const KEY_PARCEL_STYLES: i32 = 2;
pub(super) const KEY_FEATURE_LINE_STYLES: i32 = 128;

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

/// The parcel style collection (style tree key 2) and the feature line
/// style collection (128), each with its default style (the feature line
/// style draws the default roadway marker), when the drawing lacks them.
pub(super) fn ensure_site_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    ensure(doc, PARCEL_STYLES, &[(KEY_PARCEL_STYLES, 0)])?;
    ensure_marker_styles(doc)?;
    ensure(doc, FEATURE_LINE_STYLES, &[(KEY_FEATURE_LINE_STYLES, 0)])
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
