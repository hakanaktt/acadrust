//! Sample line groups, sample lines and their sections.

use super::alignment::{rewritable, rewrite_with};
use super::default_styles::{ensure_section_styles, KEY_SECTION_STYLES};
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
/// The class word after the class head (both records).
const WORD_GROUP: i32 = 0x4000_0353;
const WORD_LINE: i32 = 0x4000_8353;
/// The geometry stamp key (`BL 2`, two values; written as 0).
const STAMP: i32 = 0x0B00_1081;

/// The runs of a group record before its line count and after its section
/// sources (the same in WL10 to WL13 and WX12: no to three lines, no or
/// one source).
const GROUP_HEAD: &str = "01000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000101010011010100110101001";
const GROUP_TAIL: &str = "10101110101000000000000000000000000000000000000000000000000000100100101000000000000000000000000000000000000000000000000000000000100100101000000000000000000000000000000000000000000000000000000000100100101000000000000000000000000000000000000000000000000000000000101100101000000000000000000000000000000000000000000000000000000000101100101000000000000000000011110101010100001011111000000100";
/// After `BL 1`, a section source of a group (a TIN surface; WX12).
const SOURCE_RUN: &str = "10000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001001";
/// The run that closes a sample line record after its section and section
/// view counts.
const LINE_TAIL: &str = "110101000111111111111111111111111111111110011111111111111111111111111111111100001011111000000100";
/// The group's texts after its name (the same in every sample).
const GROUP_TEXTS: [&str; 7] = ["", "", "0", "0", "", "", ""];
/// A group plot's constant values (the same in every sample, WX21 to
/// WX23 and WG01 to WG12): eight `BD 0`, `B 0`, `BD -30`, `BD 30`, `B 0`,
/// `BD 40`; then after its count (3) 16 zero bytes and `PLOT_TAIL`.
const PLOT_VALUES: [f64; 3] = [-30.0, 30.0, 40.0];
/// The count a group plot stores after `PLOT_VALUES`: 3 whatever its views
/// and sources (WG01 to WG12, SM02, SM10a to SM13a).
pub(super) const PLOT_COUNT: i32 = 3;
const PLOT_TAIL: &str = "0100001001010101010100100000001011010101010101010101";

/// A group plot of a group's section views: its index (1 up), the group
/// plot style, the point it was placed at and its count (3 in every
/// sample, whatever its views).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Plot {
    pub index: i32,
    pub style: u64,
    pub origin: crate::types::Vector3,
    pub count: i32,
}

pub(super) fn arch(doc: &CadDocument, h: Handle) -> Option<&ArchEntityData> {
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
/// BL k section sources, k x {BL 1, SOURCE_RUN}, 2 bits 10, BL p group
/// plots, p x {BL index, 3BD origin, PLOT_VALUES, BL 3, 16 zero bytes,
/// PLOT_TAIL}, GROUP_TAIL after its first four bits
/// references: soft 0, hard owner 0, alignment (hard), hard 0, the lines
///   (soft, by station), per source {section style, surface} (soft), per
///   plot {group plot style (hard), soft 0, hard 0}, soft 0 x 3, sample
///   line style, label style (hard)
/// texts: "AeccImpSampleLineGroup", "", name, "", per source {"0", "",
///   ""}, per plot {"", "", ""}, "", "0", "0", "", "", ""
/// ```
///
/// The groups of WL10 to WL13 are reproduced bit for bit apart from the
/// stamp, and so are the groups with plots of WX21 to WX23 and WG01.
pub(super) fn write_group(doc: &mut CadDocument, g: &Group) -> Result<Handle, DxfError> {
    let mut refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(g.alignment), hard_pointer(0)];
    refs.extend(g.lines.iter().map(|l| soft_pointer(*l)));
    for (style, surface) in &g.sources {
        refs.extend([soft_pointer(*style), soft_pointer(*surface)]);
    }
    for p in &g.plots {
        refs.extend([hard_pointer(p.style), soft_pointer(0), hard_pointer(0)]);
    }
    refs.extend([soft_pointer(0), soft_pointer(0), soft_pointer(0), hard_pointer(g.line_style), hard_pointer(g.label_style)]);
    let mut strings = vec!["AeccImpSampleLineGroup".to_string(), String::new(), g.name.clone(), String::new()];
    for _ in &g.sources {
        strings.extend(["0".to_string(), String::new(), String::new()]);
    }
    for _ in &g.plots {
        strings.extend([String::new(), String::new(), String::new()]);
    }
    strings.extend(GROUP_TEXTS[1..].iter().map(|s| s.to_string()));
    put_entity(
        doc,
        "AECC_SAMPLE_LINE_GROUP",
        g.common.clone(),
        |w| {
            head(w, WORD_GROUP);
            bits(w, "0101");
            bits(w, "00000001");
            for v in [STAMP, 2, 0, 0] {
                w.write_bit_long(v);
            }
            bits(w, GROUP_HEAD);
            w.write_bit_long(g.lines.len() as i32);
            w.write_bit_long(g.sources.len() as i32);
            for _ in &g.sources {
                w.write_bit_long(1);
                bits(w, SOURCE_RUN);
            }
            bits(w, &GROUP_TAIL[..2]);
            w.write_bit_long(g.plots.len() as i32);
            for p in &g.plots {
                w.write_bit_long(p.index);
                for v in [p.origin.x, p.origin.y, p.origin.z] {
                    w.write_bit_double(v);
                }
                for _ in 0..8 {
                    w.write_bit_double(0.0);
                }
                w.write_bit(false);
                w.write_bit_double(PLOT_VALUES[0]);
                w.write_bit_double(PLOT_VALUES[1]);
                w.write_bit(false);
                w.write_bit_double(PLOT_VALUES[2]);
                w.write_bit_long(p.count);
                w.write_bytes(&[0; 16]);
                bits(w, PLOT_TAIL);
            }
            bits(w, &GROUP_TAIL[4..]);
        },
        Body { refs, strings },
    )
}

/// A sample line (`AECC_SAMPLE_LINE`):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008353, 4 bits 0010, RC 0,
/// 120 zero bits, 3BD 0 (origin), 9 BD (identity), BD station,
/// the vertices left of the alignment, then those right of it, each side
/// `BL n, n x {BD 0, BD offset (left negative), BD station less the
/// line's}` from the alignment outwards,
/// BL sections, BL section views, LINE_TAIL (a line with all its vertices
///   on one side: its first bit, BL 1, its bits after the first three)
/// references: soft 0, hard owner 0, hard 0, soft 0 per vertex, style,
///   group (hard), the sections, the section views (soft)
/// texts: "AeccImpSampleLine", "", name, ""
/// ```
///
/// The line's station is where it crosses the alignment. The lines of WL11
/// to WL13 (stations 50, 80 and 20) and of LV00 to LV02 (three and four
/// vertices, stations away from the line's) equal the samples' records bit
/// for bit, and so do the one-sided lines of LV03 and LW01 to LW04 (left
/// or right, two or three vertices, square or skewed to the alignment).
pub(super) fn write_line(doc: &mut CadDocument, l: &Line) -> Result<Handle, DxfError> {
    let (name, station) = (l.name.as_str(), l.station);
    let mut refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(0)];
    refs.extend(l.sides.iter().flatten().map(|_| soft_pointer(0)));
    refs.extend([hard_pointer(l.style), hard_pointer(l.group)]);
    refs.extend(l.sections.iter().chain(&l.views).map(|h| soft_pointer(*h)));
    let strings = ["AeccImpSampleLine", "", name, ""].iter().map(|s| s.to_string()).collect();
    put_entity(
        doc,
        "AECC_SAMPLE_LINE",
        l.common.clone(),
        |w| {
            head(w, l.word);
            bits(w, "0010");
            bits(w, "00000000");
            bits(w, &"0".repeat(120));
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, station] {
                w.write_bit_double(v);
            }
            for side in &l.sides {
                w.write_bit_long(side.len() as i32);
                for (offset, delta) in side {
                    w.write_bit_double(0.0);
                    w.write_bit_double(*offset);
                    w.write_bit_double(*delta);
                }
            }
            w.write_bit_long(l.sections.len() as i32);
            w.write_bit_long(l.views.len() as i32);
            if l.sides.iter().any(|s| s.is_empty()) {
                bits(w, &LINE_TAIL[..1]);
                w.write_bit_long(1);
                bits(w, &LINE_TAIL[3..]);
            } else {
                bits(w, LINE_TAIL);
            }
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
    let st = Style { name: "", dxf: "AECC_SAMPLE_LINE_STYLE", id, children, keys: &POINT_STYLE_KEYS, own, pointer_table: &[], lists, class_refs: Vec::new() };
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
/// style), so a drawing without the label node gets the default label
/// styles.
fn label_style(doc: &CadDocument) -> Option<u64> {
    super::labels::default_label_style(doc, super::labels::SAMPLE_LINE_LABELS)
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
    super::labels::ensure_label_styles(doc)?;
    let label = label_style(doc).ok_or_else(|| invalid("drawing has no sample line label style"))?;
    ensure_line_style_tree(doc)?;
    let line_style = default_style_of(doc, KEY_SAMPLE_LINE_STYLES).ok_or_else(|| invalid("drawing has no sample line styles"))?;
    let handle = doc.allocate_handle();
    let c = common(doc, handle, layer)?;
    let mut groups = written.groups.clone();
    groups.push(handle.value());
    rewrite_with(doc, alignment, written, &profiles, &profiles, None, &groups)?;
    write_group(doc, &Group { common: c, name: name.to_string(), alignment: alignment.value(), lines: Vec::new(), sources: Vec::new(), plots: Vec::new(), line_style, label_style: label })
}

/// A sample line as written.
pub(super) struct Line {
    pub(super) common: EntityCommon,
    /// The class word (kept when the line is written again).
    pub(super) word: i32,
    pub(super) name: String,
    pub(super) station: f64,
    /// The vertices left and right of the alignment as (offset, left
    /// negative; station less the line's), from the alignment outwards.
    pub(super) sides: [Vec<(f64, f64)>; 2],
    pub(super) style: u64,
    pub(super) group: u64,
    pub(super) sections: Vec<u64>,
    pub(super) views: Vec<u64>,
}

/// A group as written: its record's alignment, lines, section sources
/// `(section style, surface)`, group plots, styles and name.
pub(super) struct Group {
    pub common: EntityCommon,
    pub name: String,
    pub alignment: u64,
    pub lines: Vec<u64>,
    pub sources: Vec<(u64, u64)>,
    pub plots: Vec<Plot>,
    pub line_style: u64,
    pub label_style: u64,
}

/// The group plots of a group record in the layout [`write_group`]
/// writes (its constant values as in every sample), `None` otherwise.
fn plots_of(a: &ArchEntityData) -> Option<Vec<(i32, crate::types::Vector3, i32)>> {
    let tail = u64::from_str_radix(PLOT_TAIL, 2).ok()?;
    a.section_view_group_plots()?
        .into_iter()
        .map(|p| (p.extra == [0.0; 8] && p.values == PLOT_VALUES && p.flags == [false; 2] && p.id == [0; 16] && p.tail == tail).then_some((p.index, p.origin, p.count)))
        .collect()
}

pub(super) fn read_group(doc: &CadDocument, group: Handle) -> Option<Group> {
    let Some(EntityType::Extended(x)) = doc.get_entity(group) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    (a.dxf_name == "AECC_SAMPLE_LINE_GROUP").then_some(())?;
    let plots = plots_of(a)?;
    let refs = a.typed_handles();
    (refs.len() >= 9 + 3 * plots.len()).then_some(())?;
    let middle: Vec<u64> = refs[4..refs.len() - 5 - 3 * plots.len()].iter().map(|r| r.0).collect();
    let plot_refs: Vec<u64> = refs[refs.len() - 5 - 3 * plots.len()..refs.len() - 5].iter().map(|r| r.0).collect();
    let is_line = |h: u64| arch(doc, Handle::new(h)).is_some_and(|l| l.dxf_name == "AECC_SAMPLE_LINE");
    let n = middle.iter().take_while(|h| is_line(**h)).count();
    let rest = &middle[n..];
    (rest.len() % 2 == 0).then_some(())?;
    let sources: Vec<(u64, u64)> = rest.chunks(2).map(|c| (c[0], c[1])).collect();
    let strings = a.decode().strings;
    let mut want = vec![String::new()];
    for _ in &sources {
        want.extend(["0".to_string(), String::new(), String::new()]);
    }
    for _ in &plots {
        want.extend([String::new(), String::new(), String::new()]);
    }
    want.extend(GROUP_TEXTS[1..].iter().map(|s| s.to_string()));
    (strings.len() == 3 + want.len() && strings[3..] == want[..]).then_some(())?;
    Some(Group {
        common: x.common.clone(),
        name: strings[2].clone(),
        alignment: refs[2].0,
        lines: middle[..n].to_vec(),
        sources,
        plots: plots.iter().zip(plot_refs.chunks(3)).map(|((index, origin, count), r)| Plot { index: *index, style: r[0], origin: *origin, count: *count }).collect(),
        line_style: refs[refs.len() - 2].0,
        label_style: refs[refs.len() - 1].0,
    })
}

/// A sample line record read back (see [`write_line`]), `None` for
/// another record.
pub(super) fn read_line(doc: &CadDocument, line: Handle) -> Option<Line> {
    let Some(EntityType::Extended(x)) = doc.get_entity(line) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    (a.dxf_name == "AECC_SAMPLE_LINE").then_some(())?;
    let (data, start, end) = a.main_stream()?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    for _ in 0..6 {
        b.bit_long()?;
    }
    let word = b.bit_long()?;
    b.skip(4 + 8 + 120)?;
    for _ in 0..12 {
        b.bit_double()?;
    }
    let station = b.bit_double()?;
    let mut sides = [Vec::new(), Vec::new()];
    for side in &mut sides {
        for _ in 0..b.bit_long()? {
            (b.bit_double()? == 0.0).then_some(())?;
            side.push((b.bit_double()?, b.bit_double()?));
        }
    }
    let (n, m) = (usize::try_from(b.bit_long()?).ok()?, usize::try_from(b.bit_long()?).ok()?);
    let refs = a.typed_handles();
    let at = 3 + sides.iter().map(Vec::len).sum::<usize>();
    (refs.len() == at + 2 + n + m).then_some(())?;
    let name = a.decode().strings.get(2)?.clone();
    Some(Line {
        common: x.common.clone(),
        word,
        name,
        station,
        sides,
        style: refs[at].0,
        group: refs[at + 1].0,
        sections: refs[at + 2..at + 2 + n].iter().map(|r| r.0).collect(),
        views: refs[at + 2 + n..].iter().map(|r| r.0).collect(),
    })
}

pub(super) fn station(doc: &CadDocument, line: u64) -> Option<f64> {
    read_line(doc, Handle::new(line)).map(|l| l.station)
}

/// Adds a sample line `name` to `group` at `station`, reaching `left` to
/// the left of the alignment and `right` to its right (both distances, at
/// least one positive), on `layer` (layer "0" when `None`). The group is
/// written again with the line among its lines (kept in station order).
pub fn add_sample_line(doc: &mut CadDocument, group: Handle, name: &str, station_value: f64, left: f64, right: f64, layer: Option<&str>) -> Result<Handle, DxfError> {
    if !(left >= 0.0 && right >= 0.0 && left + right > 0.0) {
        return Err(invalid("a sample line needs non-negative offsets"));
    }
    add_sample_line_through(doc, group, name, station_value, &[(station_value, -left), (station_value, right)], layer)
}

/// Adds a sample line `name` to `group` through `vertices` (station,
/// offset; the offset negative left of the alignment), crossing the
/// alignment at `station`, on `layer` (layer "0" when `None`). A line
/// with all its vertices on one side (two or more) stands at the station
/// of its vertex nearest the alignment, as the application places one.
/// The group is written again with the line among its lines (kept in
/// station order).
pub fn add_sample_line_through(doc: &mut CadDocument, group: Handle, name: &str, station_value: f64, vertices: &[(f64, f64)], layer: Option<&str>) -> Result<Handle, DxfError> {
    let finite = vertices.iter().all(|v| v.0.is_finite() && v.1.is_finite());
    // The side is the offset's sign (-0 is left).
    let left = |v: &&(f64, f64)| v.1.is_sign_negative();
    if name.is_empty() || !station_value.is_finite() || !finite || vertices.len() < 2 || vertices.iter().any(|v| v.1 == 0.0) {
        return Err(invalid("a sample line needs a name, a station and two or more vertices off the alignment"));
    }
    let one_side = vertices.iter().all(|v| left(&v)) || !vertices.iter().any(|v| left(&v));
    if one_side && vertices.iter().min_by(|a, b| a.1.abs().total_cmp(&b.1.abs())).is_some_and(|v| v.0 != station_value) {
        return Err(invalid("a one-sided sample line stands at the station of its vertex nearest the alignment"));
    }
    let side = |left_side: bool| {
        let mut v: Vec<(f64, f64)> = vertices.iter().filter(|v| v.1.is_sign_negative() == left_side).map(|v| (v.1, v.0 - station_value)).collect();
        v.sort_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
        v
    };
    let sides = [side(true), side(false)];
    let g = read_group(doc, group).ok_or_else(|| invalid("the group is not one this crate writes"))?;
    let al = arch(doc, Handle::new(g.alignment)).ok_or_else(|| invalid("the group's alignment is missing"))?;
    let start = al.alignment_start_station().ok_or_else(|| invalid("the alignment has no start station"))?;
    let length: f64 = al.alignment_pieces().iter().map(|(_, p)| p.windows(2).map(|w| w[0].distance(&w[1])).sum::<f64>()).sum();
    if station_value < start - 1e-9 || station_value > start + length + 1e-9 {
        return Err(invalid("the station is off the alignment"));
    }
    if !g.sources.is_empty() {
        return Err(invalid("the group samples sources; adding lines to it is not written"));
    }
    let mut lines: Vec<(f64, u64)> = g.lines.iter().map(|l| Some((station(doc, *l)?, *l))).collect::<Option<Vec<_>>>().ok_or_else(|| invalid("a line of the group cannot be read"))?;
    let handle = doc.allocate_handle();
    let c = common(doc, handle, layer)?;
    let line = Line { common: c, word: WORD_LINE, name: name.to_string(), station: station_value, sides, style: g.line_style, group: group.value(), sections: Vec::new(), views: Vec::new() };
    write_line(doc, &line)?;
    let at = lines.partition_point(|l| l.0 <= station_value);
    lines.insert(at, (station_value, handle.value()));
    let ordered: Vec<u64> = lines.iter().map(|l| l.1).collect();
    write_group(doc, &Group { lines: ordered, ..g })?;
    Ok(handle)
}

/// The class word of a section record.
const WORD_SECTION: i32 = 0x4000_8354;
/// The run that closes a section record after its counts (WX12).
const SECTION_TAIL: &str = "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000001011111000000100";

/// A section (`AECC_SECTION`, an entity in model space) as the
/// application writes it before it samples the source:
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008354, 4 bits 0010, RC 0,
/// 120 zero bits, 3BD 0, 9 BD identity, BL 0, BL 0, BL 1, BL 0, BL 0,
/// SECTION_TAIL
/// references: soft 0, hard owner 0, section style, sample line (hard),
///   source (soft)
/// texts: "AeccImpSection", "", name, ""
/// ```
///
/// WX12's section equals this record bit for bit.
fn write_section(doc: &mut CadDocument, common: EntityCommon, name: &str, style: u64, line: u64, source: u64) -> Result<Handle, DxfError> {
    let refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(style), hard_pointer(line), soft_pointer(source)];
    let strings = ["AeccImpSection", "", name, ""].iter().map(|s| s.to_string()).collect();
    put_entity(
        doc,
        "AECC_SECTION",
        common,
        |w| {
            head(w, WORD_SECTION);
            bits(w, "0010");
            bits(w, "00000000");
            bits(w, &"0".repeat(120));
            for v in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
                w.write_bit_double(v);
            }
            for v in [0, 0, 1, 0, 0] {
                w.write_bit_long(v);
            }
            bits(w, SECTION_TAIL);
        },
        Body { refs, strings },
    )
}

/// Makes the TIN surface `surface` a sampled section source of `group`
/// and adds a section of it to each of the group's lines, named "<group> -
/// <line> - <surface>(1)", with the drawing's default section style. The
/// sections are written as the application writes them before it samples
/// the surface: they hold no section points until the application
/// updates them. Returns the sections in the group's line order.
pub fn add_sections(doc: &mut CadDocument, group: Handle, surface: Handle) -> Result<Vec<Handle>, DxfError> {
    let mut g = read_group(doc, group).ok_or_else(|| invalid("the group is not one this crate writes"))?;
    let s = arch(doc, surface).filter(|s| s.dxf_name == "AECC_TIN_SURFACE").ok_or_else(|| invalid("the source is not a TIN surface"))?;
    if g.sources.iter().any(|(_, h)| *h == surface.value()) {
        return Err(invalid("the group samples the surface already"));
    }
    let surface_name = s.decode().strings.get(2).cloned().unwrap_or_default();
    ensure_section_styles(doc)?;
    let style = default_style_of(doc, KEY_SECTION_STYLES).ok_or_else(|| invalid("drawing has no section styles"))?;
    let mut lines = g.lines.iter().map(|l| read_line(doc, Handle::new(*l))).collect::<Option<Vec<_>>>().ok_or_else(|| invalid("a line of the group cannot be read"))?;
    let mut out = Vec::new();
    for line in &mut lines {
        let handle = doc.allocate_handle();
        let c = EntityCommon { handle, layer_handle: line.common.layer_handle, layer: line.common.layer.clone(), owner_handle: line.common.owner_handle, ..EntityCommon::default() };
        write_section(doc, c, &format!("{} - {} - {}(1)", g.name, line.name, surface_name), style, line.common.handle.value(), surface.value())?;
        line.sections.push(handle.value());
        write_line(doc, line)?;
        out.push(handle);
    }
    g.sources.push((style, surface.value()));
    write_group(doc, &g)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::{DxfVersion, Vector2};

    #[test]
    fn groups_keep_their_lines_by_station() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        // A new drawing has the default label styles.
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
        let label = super::super::labels::default_label_style(&back, super::super::labels::SAMPLE_LINE_LABELS).unwrap();
        assert_eq!(group.label_style, label);
        assert_eq!(object(&back, Handle::new(label)).unwrap().decode().strings.last().map(String::as_str), Some("AeccDbSampleLine"));
        assert_eq!(station(&back, b.value()), Some(80.0));
        // The public reader takes the layout the codec writes too.
        assert_eq!([a, b, c].map(|h| arch(&back, h).unwrap().sample_line_station()), [Some(50.0), Some(80.0), Some(20.0)]);
        assert_eq!(arch(&back, a).unwrap().decode().strings[2], "SL1");
        // The alignment lists the group after its own references.
        assert_eq!(arch(&back, al).unwrap().typed_handles().last().map(|r| r.0), Some(g.value()));
        let style = default_style_of(&back, KEY_SAMPLE_LINE_STYLES).unwrap();
        assert_eq!(group.line_style, style);
    }

    #[test]
    fn lines_run_through_their_vertices() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let g = add_sample_line_group(&mut doc, al, "SLG1", None).unwrap();
        // Both vertices on one side: at the nearest vertex's station.
        assert!(add_sample_line_through(&mut doc, g, "X", 40.0, &[(45.0, 5.0), (55.0, 15.0)], None).is_err());
        let one = add_sample_line_through(&mut doc, g, "SL0", 45.0, &[(45.0, 5.0), (55.0, 15.0)], None).unwrap();
        assert_eq!(read_line(&doc, one).unwrap().sides, [vec![], vec![(5.0, 0.0), (15.0, 10.0)]]);
        let line = add_sample_line_through(&mut doc, g, "SL1", 40.0, &[(38.0, -12.0), (40.0, -4.0), (42.0, 6.0), (41.0, 14.0)], None).unwrap();
        let l = read_line(&doc, line).unwrap();
        assert_eq!(l.sides, [vec![(-4.0, 0.0), (-12.0, -2.0)], vec![(6.0, 2.0), (14.0, 1.0)]]);
        // One soft 0 per vertex before the style.
        assert_eq!(arch(&doc, line).unwrap().typed_handles().len(), 3 + 4 + 2);
    }

    #[test]
    fn sections_join_their_lines_and_group() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let g = add_sample_line_group(&mut doc, al, "SLG1", None).unwrap();
        let a = add_sample_line(&mut doc, g, "SL1", 50.0, 10.0, 10.0, None).unwrap();
        let b = add_sample_line(&mut doc, g, "SL2", 20.0, 5.0, 5.0, None).unwrap();
        let p = |x: f64, y: f64| crate::types::Vector3::new(x, y, 10.0 + x / 10.0);
        let surface = crate::classes::civil::add_tin_surface(&mut doc, "S1", &[p(-10.0, -40.0), p(110.0, -40.0), p(110.0, 40.0), p(-10.0, 40.0)], None).unwrap();
        assert!(add_sections(&mut doc, g, al).is_err());
        let made = add_sections(&mut doc, g, surface).unwrap();
        assert!(add_sections(&mut doc, g, surface).is_err());
        let path = std::env::temp_dir().join(format!("civil-sections-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let group = read_group(&back, g).unwrap();
        assert_eq!(group.lines, vec![b.value(), a.value()]);
        // The drawing gets the default section style.
        let section_style = Handle::new(default_style_of(&back, KEY_SECTION_STYLES).unwrap());
        assert_eq!(object(&back, section_style).unwrap().dxf_name, "AECC_SECTION_STYLE");
        assert_eq!(group.sources, vec![(section_style.value(), surface.value())]);
        // Sections follow the group's line order: SL2 (station 20), SL1.
        assert_eq!(read_line(&back, b).unwrap().sections, vec![made[0].value()]);
        assert_eq!(read_line(&back, a).unwrap().sections, vec![made[1].value()]);
        let s = arch(&back, made[1]).unwrap();
        assert_eq!(s.decode().strings[2], "SLG1 - SL1 - S1(1)");
        assert_eq!(s.typed_handles().iter().map(|r| r.0).collect::<Vec<_>>(), vec![0, 0, section_style.value(), a.value(), surface.value()]);
        // A group that samples a source takes no more lines.
        assert!(add_sample_line(&mut doc, g, "SL3", 70.0, 5.0, 5.0, None).is_err());
    }
}
