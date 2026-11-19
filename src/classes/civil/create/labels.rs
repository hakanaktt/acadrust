//! The default label styles of a new drawing.

use super::support::*;
use super::templates::{write_templates, Piece::{Children, Run, Time}, Target, Template};
use super::tree::{ensure_point_tree, KEY_STYLES};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType::{HardOwnership, HardPointer, SoftPointer};
use crate::types::Handle;
use crate::CadDocument;

/// Style tree key of the label node.
pub(super) const KEY_LABELS: i32 = 3;
/// Style tree key of the profile label set collection.
const KEY_PROFILE_LABEL_SETS: i32 = 96;

/// Writes the default label styles a new drawing lacks: the label node
/// (style tree key 3) with the point (key 0x113), sample line (0x112)
/// and profile (0x49) label roots, their collections, default styles and
/// components, and the profile label set collection (style tree key 96)
/// with its default set (the line, PVI, sag and crest curve label styles).
/// The records are the application's default label styles (users empty,
/// dates the time of writing); a drawing that has a label node is left as
/// it is.
pub(super) fn ensure_label_styles(doc: &mut CadDocument) -> Result<(), DxfError> {
    if root_node(doc, KEY_STYLES).is_none() && !ensure_point_tree(doc)? {
        return Err(invalid("drawing has a civil tree without a style tree"));
    }
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    if tree.children.iter().any(|c| c.0 == KEY_LABELS) {
        return Ok(());
    }
    let handles = write_templates(doc, styles, TEMPLATES)?;
    add_node_child(doc, styles, KEY_LABELS, handles[0], "")?;
    add_node_child(doc, styles, KEY_PROFILE_LABEL_SETS, handles[PROFILE_LABEL_SETS], "")
}

/// Label style tree keys under the label node: the root, the collection
/// and the default style of point labels and sample line labels.
pub(super) const POINT_LABELS: [i32; 3] = [0x113, 0x1000_B40C, 30];
pub(super) const SAMPLE_LINE_LABELS: [i32; 3] = [0x112, 0x1002_A00F, 30];

/// The default label style at `path` under the label node.
pub(super) fn default_label_style(doc: &CadDocument, path: [i32; 3]) -> Option<u64> {
    let mut node = root_node(doc, KEY_STYLES);
    for key in std::iter::once(KEY_LABELS).chain(path) {
        node = node.and_then(|n| read_node(doc, n)).and_then(|n| n.children.iter().find(|c| c.0 == key).map(|c| Handle::new(c.1)));
    }
    node.map(|h| h.value()).filter(|h| *h != 0)
}

/// The template of the profile label set collection.
const PROFILE_LABEL_SETS: usize = 36;

/// The default label style tree (label node, label roots, collections,
/// default styles and their components, profile label sets), in writing
/// order. Each record's fields are the samples' records apart from the
/// dates; the runs hold the styles' settings, which are not split into
/// fields.
// ponytail: the settings are constant runs; name them once a style other
// than the defaults is written.
#[rustfmt::skip]
const TEMPLATES: &[Template] = &[
    Template {
        dxf: "AECC_LABEL_NODE",
        owner: Target::Null,
        children: &[(0x112, 26), (0x113, 30), (0x49, 1)],
        fields: &[Run(10, "40c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(1302, "480d5551fbfe00010222222222223027f23c2a8480003662884800036628848000270298480003322984800042a84800023c298480001fffffe007ff6ff9fa7f590016400115202809aebc9413836ffc7eaa0222222222222c27f8088888888888c09fc00000060001ffffffff000000180007fffffffd000000180007fffffffa0547f83800048222222222223027e044444444444584fe600000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Standard), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Layer0), (HardPointer, Target::ByBlock), (HardPointer, Target::ByBlock), (HardPointer, Target::ByBlock), (HardPointer, Target::ByBlock), (HardPointer, Target::Null)],
        strings: &["", "", "", ".", ".", ".", ".", ".", ".", "."],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(0),
        children: &[(0xf00b412, 23), (0xf00b413, 17), (0xf00b414, 12), (0xf00b40d, 9), (0xf00b40e, 6), (0xf00b40f, 2)],
        fields: &[Run(10, "524"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(1),
        children: &[(0x1e, 3)],
        fields: &[Run(34, "03ed0003c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(3)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(2),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(774, "480d5555301f68001e70400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff40904a001fffffe0151014128007fffff8054409028")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(5)), (HardPointer, Target::Record(4)), (HardOwnership, Target::Record(5)), (HardOwnership, Target::Record(4)), (SoftPointer, Target::Record(5)), (SoftPointer, Target::Record(4)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbVAlignment"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(3),
        children: &[],
        fields: &[Run(34, "318080200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(2322, "480d555531fffffe0070400683a4b480886000000762001152034a044444444444584fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fce81e002d801b001b8018801b00190018801c801980140019001a001a001c8014802e801f001d0010001e002d80198019801b0018001b801b80188019001400190018001c801b803e0019001a001b801a003e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001b001b801a003e0019001b001c001c803e0019001b8018001b003e0019001b80190018803e0019001b8018001b803e0019001c00180019803e0019001c0018801b8014802e801f00000052108328")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(3)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["Horizontal Geometry", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_RADIAL_LINE_STYLE",
        owner: Target::Record(3),
        children: &[],
        fields: &[Run(34, "044300000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(804, "480d555531fffffe0030400683a4b4904162001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0547f0380004082c40022811111111111171bfaa8aaaaaaaaaaaaba3f8d221337f7cd91240a6")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(3)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(1),
        children: &[(0x1e, 7)],
        fields: &[Run(34, "03ad0003c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(7)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(6),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(700, "480d5555301d68001e70400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff40504a001fffffe015101406")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(8)), (HardOwnership, Target::Record(8)), (SoftPointer, Target::Record(8)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbVAlignment"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(7),
        children: &[],
        fields: &[Run(34, "130300000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(2034, "480d555531fffffe0070400683a4b4904162001152034a044444444444584fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fcd41e002d80198019801b0018001b801b80188019001400190018001c801b803e0019001a001b801a003e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001b001b801a003e0019001b001c001c803e0019001b8018001b003e0019001b80190018803e0019001b8018001b803e0019001c00180019803e0019001c0018801b8014802e801f0000001a44266fef9b2248120a8328")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(7)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(1),
        children: &[(0x1e, 10)],
        fields: &[Run(34, "036d0003c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(10)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(9),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(700, "480d5555301b68001e70400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff40504a001fffffe015101406")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(11)), (HardOwnership, Target::Record(11)), (SoftPointer, Target::Record(11)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbVAlignment"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(10),
        children: &[],
        fields: &[Run(34, "130300000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(2034, "480d555531fffffe0070400683a4b4904162001152034a044444444444584fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fcd41e002d80198019801b0018001b801b80188019001400190018001c801b803e0019001a001b801a003e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001b001b801a003e0019001b001c001c803e0019001b8018001b003e0019001b80190018803e0019001b8018001b803e0019001c00180019803e0019001c0018801b8014802e801f0000001a44266fef9b2248120a8328")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(10)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(1),
        children: &[(0x1e, 13)],
        fields: &[Run(34, "052d0003c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(13)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(12),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(856, "480d5555302968001e71400683a4b4900164001080d2034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff40d04a001fffffe0151014128007fffff805440904a001fffffe01510340e")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(16)), (HardPointer, Target::Record(15)), (HardPointer, Target::Record(14)), (HardOwnership, Target::Record(16)), (HardOwnership, Target::Record(15)), (HardOwnership, Target::Record(14)), (SoftPointer, Target::Record(16)), (SoftPointer, Target::Record(15)), (SoftPointer, Target::Record(14)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbVAlignment"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(13),
        children: &[],
        fields: &[Run(34, "16c080200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1642, "480d555531fffffe0070400683a4b482d01004001562001152034a044444444444646fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fcbe2280260022802b0010001e80100010001e002d80198019801b0018801b001c80188018001400190018001c801b803e00190019801c001c003e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f000000548228")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(13)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["PVIE", "", "", "", "PVIS"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(13),
        children: &[],
        fields: &[Run(34, "168080200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(2290, "480d555531fffffe0070400683a4b480886000000762001152034a044444444444604fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fce623802900208022002280100021002900228020802580100029802a00208010001e8010001e002d80198019801b0018801b001c8018001c801400190018001c801b803e0019001a001b801a003e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001b001b801a003e0019001b001c001c803e0019001b8018001b003e0019001b80190018803e0019001b8018001b803e0019001c00180019803e0019001c0018801b8014802e801f00000052108328")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(13)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["PVIS", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_RADIAL_LINE_STYLE",
        owner: Target::Record(13),
        children: &[],
        fields: &[Run(34, "044300000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(804, "480d555531fffffe0030400683a4b4906162001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0547f03800040c2c400228111111111111813faa8000000000000d03f8182d4454fb21f93fa6")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(13)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(1),
        children: &[(0x1e, 18)],
        fields: &[Run(34, "04ed0003c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(18)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(17),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(930, "480d5555302768001e71400683a4b4900164001080d2034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff41104a001fffffe0151014128007fffff805440904a001fffffe0151034128007fffff8054411048")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(22)), (HardPointer, Target::Record(21)), (HardPointer, Target::Record(20)), (HardPointer, Target::Record(19)), (HardOwnership, Target::Record(22)), (HardOwnership, Target::Record(21)), (HardOwnership, Target::Record(20)), (HardOwnership, Target::Record(19)), (SoftPointer, Target::Record(22)), (SoftPointer, Target::Record(21)), (SoftPointer, Target::Record(20)), (SoftPointer, Target::Record(19)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbVAlignment"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(18),
        children: &[],
        fields: &[Run(34, "003440008"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1554, "480d555531fffffe0070400683a4b482e01004000562001152034a044444444444604fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fcb81e002d80198019801b0018801b0019801c801b801400190018001c801b803e00190019801c001c003e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f002b0021800000520e82a8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(18)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", "", "Dimension Line"],
    },
    Template {
        dxf: "AECC_LABEL_RADIAL_LINE_STYLE",
        owner: Target::Record(18),
        children: &[],
        fields: &[Run(34, "178080200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(748, "480d555531fffffe0030400683a4b4906562001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0547f03800050dac400228111111111111713faa8555555555555b53fa9016")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(18)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["End Line", "", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_RADIAL_LINE_STYLE",
        owner: Target::Record(18),
        children: &[],
        fields: &[Run(34, "174080200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(748, "480d555531fffffe0030400683a4b4906362001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0547f03800050d2c400228111111111111713faa8555555555555b53fa9016")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(18)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["Start Line", "", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_RADIAL_LINE_STYLE",
        owner: Target::Record(18),
        children: &[],
        fields: &[Run(34, "170080200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(684, "480d555531fffffe0070400683a4b4906962001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0547e03800050dac40022aaa8555555555555b53fa9016")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(18)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["Dimension Line", "", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(1),
        children: &[(0x1e, 24)],
        fields: &[Run(34, "04ad0003c"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(24)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(23),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(700, "480d5555302568001e70400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff40504a001fffffe015101406")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(25)), (HardOwnership, Target::Record(25)), (SoftPointer, Target::Record(25)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbVAlignment"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(24),
        children: &[],
        fields: &[Run(34, "128300000"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1490, "480d555531fffffe0070400683a4b4907562001152034a044444444444584fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fcb61e002d80198019801b0018801b001b001a001800140019001a001b001b803e00190019801c001b803e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f000000520e82a8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(24)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(0),
        children: &[(0x1002a00f, 27)],
        fields: &[Run(34, "048040000"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(26),
        children: &[(0x1e, 28)],
        fields: &[Run(34, "03e800840"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(28)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(27),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(700, "480d5555301f40042070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff40504a001fffffe015101406")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(29)), (HardOwnership, Target::Record(29)), (SoftPointer, Target::Record(29)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbSampleLine"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(28),
        children: &[],
        fields: &[Run(34, "3680c0200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(930, "480d555531fffffe0070400683a4b4900362001152034a044444444444584fe0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fc931e002d801b001b8019001c0018001c001c801b00140019001a001a001c8014802e801f000000520c82a8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(28)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["Sample Name", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(0),
        children: &[(0x1000b40c, 31)],
        fields: &[Run(34, "04c040000"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(30),
        children: &[(0x1e, 32)],
        fields: &[Run(34, "032d00040"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(76, "046c8000c041e100016"), Time, Run(610, "480d555531fffffe0070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffffa8")],
        refs: &[(SoftPointer, Target::Record(32)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_COLLECTOR_STYLE",
        owner: Target::Record(31),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a828102000080808000203e20002c"), Time, Run(68, "0684000101e100016"), Time, Run(848, "480d5555301968002070400683a4b4900164001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa05542a200000030000ffffffff40d04a001fffffe0151014128007fffff805440904a001fffffe01510340e")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(35)), (HardPointer, Target::Record(34)), (HardPointer, Target::Record(33)), (HardOwnership, Target::Record(35)), (HardOwnership, Target::Record(34)), (HardOwnership, Target::Record(33)), (SoftPointer, Target::Record(35)), (SoftPointer, Target::Record(34)), (SoftPointer, Target::Record(33)), (HardPointer, Target::Null)],
        strings: &["", "", "", "AeccDbCogoPoint"],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(32),
        children: &[],
        fields: &[Run(34, "0700c0200"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(890, "480d555531fffffe0070400683a4b480128000101562001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fc931e002d801b001b80188019001c8019801a001c00140019001a001a001c8014802e801f000000548228")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(32)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["Point Description", "", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(32),
        children: &[],
        fields: &[Run(34, "009400008"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1490, "480d555531fffffe0070400683a4b4901362001152034044444444444584fea0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fcb61e002d80198019801a801b801a001c8018801a001400190018001c801b803e00190019801c001c003e0019001a00198019803e0019001a801c8019803e0019001b0018001c803e0019001b001a801b803e0019001c00180019803e0019001c0018801b8014802e801f00000052068228")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(32)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_LABEL_TEXT_STYLE",
        owner: Target::Record(32),
        children: &[],
        fields: &[Run(34, "005400004"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(978, "480d555531fffffe0070400683a4b480128000100962001152034aa0222222222222c27e1400000060001ffffffff000000180007fffffffd000000180007fffffffa0549fc981e002d8018801b001b801c801b801b001c801b80140019001c00180019803e0019001c0018801b8014802e801f000000520c8228")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Record(32)), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::Null), (HardPointer, Target::ByBlock), (HardPointer, Target::Null), (HardPointer, Target::Null)],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_PROFILE_LABEL_SET",
        owner: Target::Null,
        children: &[(0x1e, 37)],
        fields: &[Run(10, "580"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(31, "480d5554")],
        refs: &[(SoftPointer, Target::Record(37))],
        strings: &["", "", ""],
    },
    Template {
        dxf: "AECC_PROFILE_LABEL_SET",
        owner: Target::Record(36),
        children: &[],
        fields: &[Run(10, "478"), Children, Run(115, "a820102000080808000203e20002c"), Time, Run(34, "078400058"), Time, Run(1303, "480d55520882035640004000000000000601f8116a000503364001a8080ec00c203fffffffd070bd801808555555555555b53f06ac80008000000000000c03f022d4000800000000000059400b9801836066c80035010c1801808555555555555c53f01d8018407fffffffa060d5900010000000000001807e0cd90006a0203b003080ffffffff40c1ab20002000000000000300fc19b2000d404076006101fffffffe")],
        refs: &[(SoftPointer, Target::Null), (HardPointer, Target::Record(24)), (HardPointer, Target::Record(13)), (HardPointer, Target::Record(18)), (HardPointer, Target::Record(18))],
        strings: &["", "", "", "AeccDbVAlignmentLineLabeling", "AeccDbVAlignmentPVILabeling", "AeccDbVAlignmentSagCurveLabeling", "AeccDbVAlignmentCrestCurveLabeling"],
    },
];
