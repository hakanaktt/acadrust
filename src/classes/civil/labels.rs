//! Labels and label groups, read by their fields.
//!
//! A single label and a label group share one record layout; a single
//! label is a group of one:
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL, 6-bit class tag    0b010010; 0b010110 adds 3 bits
//! 16 raw bytes                                 (a GUID; zero in most records)
//! BL 80, BL 80, BL 0x04BE0000, BL, 3 bits
//! BL n, n x label
//! settings                                     (see [`settings`])
//! B, BS 0x04BE                                 closes the record
//! ```
//!
//! A label is a property set of the labeled feature with the overrides
//! the user gave the label:
//!
//! ```text
//! BL k, k x key                     (see alignment::read_key)
//! 2 bits, RC, BL
//! BL k, k x BL                      ids of the keys the label carries
//! BL n, n x nested label
//! BL m, m x override
//! nested label: the label's fields up to its nested labels, without
//!            overrides (BL k, k x key, 2 bits, RC, BL, BL k, k x BL,
//!            BL n, n x nested label)
//! override:  BL b, b x block
//! block:     BL, BL, 2 bits, RC (8 x the property count in all but one
//!            sample), properties up to the one marked last
//! property:  RC id, 16 bits (the property set), 7-bit type, value by
//!            type, 3 bits (010 after the last property, else 000)
//! values:    2 BD, 3 RC, 8 and 9 three BD (a point, an offset),
//!            13 BL, 16 none (the value is a reference), 24 BL byte
//!            count and UTF-16 text (a text override), 25 BL, 26 RC
//! ```
//!
//! The label text itself is generated from the label style; only a text
//! override (value type 24) is stored.

use super::alignment::{read_key, KeyValue};
use super::bits::Bits;

/// A label's station key.
const STATION: u32 = 0x0200_D020;

/// Keys a profile label names the labelled profile entity by: a tangent
/// (line labels), the tangent before and after a PVI (PVI labels, 0 at
/// the profile's ends) and a vertical curve (sag and crest curve labels).
const PROFILE_ENTITY_KEYS: [u32; 4] = [0x0100_FC31, 0x0100_FC32, 0x0100_FC33, 0x0100_FC34];

/// The label override property set (bytes C0 0C) and, in it, the dragged
/// offset property: a type 9 value whose X and Y are the offset from the
/// labelled point to the dragged label.
const OVERRIDE_SET: u16 = 0x0CC0;
const DRAGGED_OFFSET: u8 = 0x02;

/// One label of a label record.
struct Label {
    /// Keys with their values and the bit where each value starts.
    keys: Vec<(u32, KeyValue, usize)>,
    /// The overrides' properties: `(set, id, value)`, the value kept
    /// when it is a point or an offset.
    properties: Vec<(u16, u8, Option<[f64; 3]>)>,
}

/// How deep labels nest at most (a superelevation band's label holds the
/// critical point it labels, the point its alignment element); a deeper
/// record reads as None.
const NESTING: usize = 4;

/// A label record read up to its settings.
struct Record {
    labels: Vec<Label>,
    /// Where the settings start.
    settings: usize,
}

/// The record header (see the module docs).
fn header(bits: &mut Bits) -> Option<()> {
    match super::alignment::class_header(bits)? {
        0b01_0010 => {}
        0b01_0110 => {
            bits.bits_value(3)?;
        }
        _ => return None,
    }
    bits.skip(128)?;
    for want in [80, 80, 0x04BE_0000] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bits_value(3)?;
    Some(())
}

/// One property of an override block (see the module docs), and whether
/// it is the block's last.
fn property(bits: &mut Bits) -> Option<((u16, u8, Option<[f64; 3]>), bool)> {
    let id = bits.raw_short_byte()?;
    let set = u16::from_le_bytes([bits.raw_short_byte()?, bits.raw_short_byte()?]);
    let mut value = None;
    match bits.bits_value(7)? {
        2 => {
            bits.bit_double()?;
        }
        3 | 26 => {
            bits.raw_short_byte()?;
        }
        8 | 9 => value = Some([bits.bit_double()?, bits.bit_double()?, bits.bit_double()?]),
        13 | 25 => {
            bits.bit_long()?;
        }
        16 => {}
        24 => {
            let bytes = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 8 <= bits.remaining())?;
            bits.skip(bytes * 8)?;
        }
        _ => return None,
    }
    let last = match bits.bits_value(3)? {
        0b010 => true,
        0b000 => false,
        _ => return None,
    };
    Some(((set, id, value), last))
}

/// A label's fields up to its overrides; nested labels end there.
fn node(bits: &mut Bits, depth: usize) -> Option<Label> {
    (depth < NESTING).then_some(())?;
    let mut keys = Vec::new();
    for _ in 0..bits.bit_long()? {
        // The value starts after the key's own BL.
        let key_at = bits.pos;
        bits.bit_long()?;
        let at = bits.pos;
        bits.pos = key_at;
        let (k, v) = read_key(bits)?;
        keys.push((k, v, at));
    }
    bits.bits_value(2)?;
    bits.raw_short_byte()?;
    bits.bit_long()?;
    for _ in 0..bits.bit_long()? {
        bits.bit_long()?;
    }
    for _ in 0..bits.bit_long()? {
        node(bits, depth + 1)?;
    }
    Some(Label { keys, properties: Vec::new() })
}

fn label(bits: &mut Bits) -> Option<Label> {
    let mut label = node(bits, 0)?;
    for _ in 0..bits.bit_long()? {
        for _ in 0..bits.bit_long()? {
            bits.bit_long()?;
            bits.bit_long()?;
            bits.bits_value(2)?;
            (bits.raw_short_byte()? != 0).then_some(())?;
            loop {
                let (property, last) = property(bits)?;
                label.properties.push(property);
                if last {
                    break;
                }
            }
        }
    }
    Some(label)
}

fn record(data: &[u8], start: usize, end: usize) -> Option<Record> {
    let mut bits = Bits::new(data, start, end);
    header(&mut bits)?;
    let n = bits.bit_long()?;
    let labels = (0..n).map(|_| label(&mut bits)).collect::<Option<Vec<_>>>()?;
    Some(Record { labels, settings: bits.pos })
}

/// The settings every label record holds after its labels, up to the
/// class's own:
///
/// ```text
/// BL k, k x key    (keys that take references)
/// BL
/// BL               default dimension anchor option (see [`LabelDimensionAnchor`])
/// BD               default dimension anchor value
/// BD               weeding
/// BL               label stagger: 0 none, 1 both sides, 2 right, 3 left
/// BD, BD           the second is the stagger line height
/// ```
fn settings(bits: &mut Bits) -> Option<LabelDimensionAnchor> {
    for _ in 0..bits.bit_long()? {
        read_key(bits)?;
    }
    bits.bit_long()?;
    let option = u32::try_from(bits.bit_long()?).ok()?;
    let value = bits.bit_double()?;
    bits.bit_double()?;
    bits.bit_long()?;
    bits.bit_double()?;
    bits.bit_double()?;
    Some(LabelDimensionAnchor { option, value })
}

/// Where a label record anchors its labels' dimension by default (profile
/// and section view labels): the option code (1 above the labelled point,
/// 2 below it) and the value (a plotted distance).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelDimensionAnchor {
    pub option: u32,
    pub value: f64,
}

/// The default dimension anchor a label record's settings hold (see
/// [`settings`]).
pub(super) fn dimension_anchor(data: &[u8], start: usize, end: usize) -> Option<LabelDimensionAnchor> {
    let r = record(data, start, end)?;
    settings(&mut Bits::new(data, r.settings, end))
}

/// The settings of a label group along an alignment, after [`settings`]:
/// 3 bits, BD range start, BD range end.
fn range(bits: &mut Bits) -> Option<()> {
    bits.bits_value(3)?;
    bits.bit_double()?;
    bits.bit_double()?;
    Some(())
}

/// `B, BS 0x04BE` closing the record at its end.
fn close(bits: &mut Bits) -> Option<()> {
    bits.bit()?;
    (bits.bit_short()? == 0x04BE && bits.remaining() == 0).then_some(())
}

/// The record's labels, when its settings open with their key block.
fn labels(data: &[u8], start: usize, end: usize) -> Option<Vec<Label>> {
    let r = record(data, start, end)?;
    let mut bits = Bits::new(data, r.settings, end);
    for _ in 0..bits.bit_long()? {
        read_key(&mut bits)?;
    }
    Some(r.labels)
}

fn station(label: &Label) -> Option<f64> {
    label.keys.iter().find_map(|(k, v, _)| match (*k, v) {
        (STATION, KeyValue::Double(s)) => Some(*s),
        _ => None,
    })
}

/// Stations of the labels that carry one, in record order.
pub(super) fn stations(data: &[u8], start: usize, end: usize) -> Option<Vec<f64>> {
    Some(labels(data, start, end)?.iter().filter_map(station).collect())
}

/// Stations of a station or minor station label group: its labels'
/// stations (the first label, at the range start, carries none); the
/// group's settings are [`settings`], [`range`], BD increment.
pub(super) fn group_stations(data: &[u8], start: usize, end: usize) -> Option<Vec<f64>> {
    let r = record(data, start, end)?;
    let mut bits = Bits::new(data, r.settings, end);
    settings(&mut bits)?;
    range(&mut bits)?;
    bits.bit_double()?;
    close(&mut bits)?;
    Some(r.labels.iter().filter_map(station).collect())
}

/// Geometry point kinds a geometry point label group lists, from its
/// settings: [`settings`], [`range`], `BL count`, `count` x `BL` (the low
/// byte is the kind: 1 the alignment's beginning, 2 its end, 3 and up the
/// points between).
pub(super) fn group_point_kinds(data: &[u8], start: usize, end: usize) -> Option<Vec<u8>> {
    let r = record(data, start, end)?;
    let mut bits = Bits::new(data, r.settings, end);
    settings(&mut bits)?;
    range(&mut bits)?;
    let count = bits.bit_long()?;
    let kinds = (0..count).map(|_| bits.bit_long().map(|v| (v & 0xFF) as u8)).collect::<Option<Vec<_>>>()?;
    close(&mut bits)?;
    Some(kinds)
}

/// Where a single station elevation or profile projection label is
/// anchored, from its settings: [`settings`], BD x, BD y, 3 bits, then
/// (profile projection labels) BL 80, BL 80, BL 0x04BE0000.
pub(super) fn anchor(data: &[u8], start: usize, end: usize, projection: bool) -> Option<crate::types::Vector2> {
    let r = record(data, start, end)?;
    (r.labels.len() == 1).then_some(())?;
    let mut bits = Bits::new(data, r.settings, end);
    settings(&mut bits)?;
    let x = bits.bit_double()?;
    let y = bits.bit_double()?;
    bits.bits_value(3)?;
    if projection {
        for want in [80, 80, 0x04BE_0000] {
            (bits.bit_long()? == want).then_some(())?;
        }
    }
    close(&mut bits)?;
    Some(crate::types::Vector2::new(x, y))
}

/// Where the first dragged label of a record sits, from the point it
/// labels: its dragged offset override (a zero offset leaves the label in
/// place). `Some(None)` when no label is dragged.
pub(super) fn dragged_offset(data: &[u8], start: usize, end: usize) -> Option<Option<crate::types::Vector2>> {
    Some(dragged_offsets(data, start, end)?.into_iter().flatten().next())
}

/// Each label's dragged offset, in record order (the order the
/// application numbers a group's labels by); `None` for a label left in
/// place.
pub(super) fn dragged_offsets(data: &[u8], start: usize, end: usize) -> Option<Vec<Option<crate::types::Vector2>>> {
    Some(labels(data, start, end)?.iter().map(label_drag).collect())
}

fn label_drag(label: &Label) -> Option<crate::types::Vector2> {
    label.properties.iter().find_map(|(set, id, value)| match (*set, *id, value) {
        (OVERRIDE_SET, DRAGGED_OFFSET, Some([x, y, _])) if (*x, *y) != (0.0, 0.0) => Some(crate::types::Vector2::new(*x, *y)),
        _ => None,
    })
}

/// Every profile entity id a profile label group's labels name (see
/// [`PROFILE_ENTITY_KEYS`]), with the bit offset of its value from
/// `start`, in record order; 0 (no entity) is left out.
pub(super) fn entity_refs(data: &[u8], start: usize, end: usize) -> Option<Vec<(usize, i32)>> {
    let labels = labels(data, start, end)?;
    Some(
        labels
            .iter()
            .flat_map(|l| &l.keys)
            .filter_map(|(k, v, at)| match v {
                KeyValue::Long(id) if PROFILE_ENTITY_KEYS.contains(k) && *id != 0 => Some((at - start, *id)),
                _ => None,
            })
            .collect(),
    )
}

/// Ids of the profile entities a profile label group labels, in record
/// order (see [`entity_refs`]).
pub(super) fn labelled_entity_ids(data: &[u8], start: usize, end: usize) -> Option<Vec<i32>> {
    Some(entity_refs(data, start, end)?.into_iter().map(|(_, id)| id).collect())
}

/// One piece of a label text component's content.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LabelPart {
    Text(String),
    /// `<[property(option|option|...)]>`: a property of the labeled object
    /// with its formatting options.
    Field { property: u32, options: Vec<u32> },
    /// `\P`
    NewLine,
}

/// Known label properties.
pub mod property {
    pub const STATION: u32 = 33_607_712;
    pub const GEOMETRY_POINT: u32 = 67_162_193;
    pub const TANGENT_GRADE: u32 = 33_616_640;
    pub const CURVE_LENGTH: u32 = 33_616_397;
    pub const BVC_STATION: u32 = 33_616_398;
    pub const EVC_STATION: u32 = 33_616_399;
    pub const BVC_ELEVATION: u32 = 33_616_400;
    pub const EVC_ELEVATION: u32 = 33_616_401;
    pub const HIGH_POINT_STATION: u32 = 33_616_905;
    pub const HIGH_POINT_ELEVATION: u32 = 33_616_906;
    pub const LOW_POINT_STATION: u32 = 33_616_907;
    pub const LOW_POINT_ELEVATION: u32 = 33_616_908;
    pub const PVI_STATION: u32 = 33_616_909;
    pub const PVI_ELEVATION: u32 = 33_616_910;
    pub const K_VALUE: u32 = 33_617_665;
    pub const SAMPLE_LINE_NAME: u32 = 67_280_896;
    pub const SAMPLE_LINE_STATION: u32 = 33_726_491;
    pub const POINT_ELEVATION: u32 = 33_574_914;
    pub const PROJECTION_STATION: u32 = 34_045_971;
    pub const PROJECTION_ELEVATION: u32 = 34_045_970;
    pub const ALIGNMENT_NAME: u32 = 167_825_414;
}

/// Formatting option codes.
mod option {
    /// Precision 0..=8 decimals: `PRECISION_0 + n`.
    pub const PRECISION_0: u32 = 2385;
    pub const PRECISION_MAX: u32 = 2393;
    /// Station format (`12+345.67`).
    pub const STATION: u32 = 2474;
    /// Grade shown as a percentage.
    pub const PERCENT: u32 = 2467;
    /// Station `+` after the hundreds (`12+34.56`); otherwise after the
    /// thousands.
    pub const STATION_PLUS_HUNDREDS: u32 = 2674;
    /// Station shown without its decimals.
    pub const STATION_WHOLE: u32 = 2705;
}

impl LabelPart {
    /// Text for a field's value with the field's options.
    pub fn format_value(options: &[u32], value: f64) -> String {
        let precision = options
            .iter()
            .find(|o| (option::PRECISION_0..=option::PRECISION_MAX).contains(*o))
            .map_or(2, |o| (o - option::PRECISION_0) as usize);
        if options.contains(&option::STATION) {
            let precision = if options.contains(&option::STATION_WHOLE) { 0 } else { precision };
            let digits = if options.contains(&option::STATION_PLUS_HUNDREDS) { 2 } else { 3 };
            let unit = 10f64.powi(digits as i32);
            // Round first so the minor part never shows as a whole unit.
            let scale = 10f64.powi(precision as i32);
            let rounded = (value.abs() * scale).round() / scale;
            let major = (rounded / unit).floor();
            let minor = rounded - major * unit;
            let sign = if value < 0.0 { "-" } else { "" };
            let width = if precision == 0 { digits } else { digits + 1 + precision };
            return format!("{sign}{}+{:0width$.precision$}", major as i64, minor, width = width, precision = precision);
        }
        if options.contains(&option::PERCENT) {
            return format!("{:.precision$}%", value * 100.0, precision = precision);
        }
        format!("{value:.precision$}")
    }
}

/// Split a label format string into text, fields and line breaks.
pub fn parse_format(format: &str) -> Vec<LabelPart> {
    let mut parts = Vec::new();
    let mut text = String::new();
    let mut rest = format;
    let flush = |text: &mut String, parts: &mut Vec<LabelPart>| {
        if !text.is_empty() {
            parts.push(LabelPart::Text(std::mem::take(text)));
        }
    };
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("\\P") {
            flush(&mut text, &mut parts);
            parts.push(LabelPart::NewLine);
            rest = after;
        } else if let Some(after) = rest.strip_prefix("<[") {
            if let Some(close) = after.find("]>") {
                let body = &after[..close];
                let (id, options) = match body.find('(') {
                    Some(open) => (&body[..open], body[open + 1..].trim_end_matches(')')),
                    None => (body, ""),
                };
                if let Ok(property) = id.trim().parse() {
                    flush(&mut text, &mut parts);
                    let options = options.split('|').filter_map(|o| o.trim().parse().ok()).collect();
                    parts.push(LabelPart::Field { property, options });
                    rest = &after[close + 2..];
                    continue;
                }
            }
            text.push_str("<[");
            rest = after;
        } else {
            let ch = rest.chars().next().unwrap();
            text.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    flush(&mut text, &mut parts);
    parts
}

/// A label text component as plotted sizes in drawing units at 1:1
/// (multiply by the drawing scale).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelText {
    pub content: Vec<LabelPart>,
    pub height: Option<f64>,
    /// Offset of the text from its anchor, along and across the anchor's
    /// direction.
    pub x_offset: f64,
    pub y_offset: f64,
    /// Border gap: the clearance kept between the attachment point and the
    /// text.
    pub gap: f64,
    /// Angle of the text to the anchor's direction, radians.
    pub rotation: f64,
    /// Which point of the text sits at the anchor: 0 top left, 1 top
    /// centre, 2 top right, 3..5 middle, 6..8 bottom, left to right.
    pub attachment: u8,
    /// Which point of its anchor the text hangs from, for anchors with
    /// several (a data band: 0 top, 1 middle, 2 bottom).
    pub anchor_point: Option<u8>,
    /// `false` when the label style hides the component.
    pub visible: bool,
    /// Whether the x and y offsets are taken from the component this one
    /// copies (a style's dragged-state copy whose own x or y offset was
    /// never set); the offsets read above are then not the ones shown.
    pub inherits_offset: [bool; 2],
}

/// The head every label style component's class data opens with (after
/// the style's component lists): `BL, 2 bits, BD height, BL, BL color, BL
/// tag, BL, BL, BL`. The tag's bits flag the properties set on the
/// component (in a dragged-state copy 0x40 the x offset, 0x80 the y
/// offset, 0x10000 the border gap) and its kind (see [`DRAGGED_COPY`]).
/// Returns the height, the tag and where the general block starts.
fn component_head(data: &[u8], start: usize, end: usize) -> Option<(f64, u32, usize)> {
    let mut bits = Bits::new(data, super::style::class_data(data, start, end)?, end);
    bits.bit_long()?;
    bits.bits_value(2)?;
    let height = bits.bit_double()?;
    let mut header = [0; 6];
    for v in &mut header {
        *v = bits.bit_long()?;
    }
    Some((height, header[2] as u32, bits.pos))
}

/// The general block up to its fields: 4 bits (the second is the
/// component's visibility), 8 flag bits, 32 bits more when flag 0x80 is
/// clear, then the anchor point (4 bits) and 20 bits; then the block's
/// fields open with `1000`. Returns the visibility and the anchor point.
fn general_head(bits: &mut Bits) -> Option<(bool, u8)> {
    let lead = bits.bits_value(4)?;
    if bits.bits_value(8)? & 0x80 == 0 {
        bits.skip(32)?;
    }
    let point = bits.bits_value(4)?;
    bits.skip(20)?;
    Some((lead & 0b0100 != 0, point))
}

/// Whether a label style component is shown, read from its general
/// block (see [`general_head`]); `None` when the component's class data
/// does not read that far.
pub(super) fn component_visible(data: &[u8], start: usize, end: usize) -> Option<bool> {
    let (_, _, at) = component_head(data, start, end)?;
    let mut bits = Bits::new(data, at, end);
    let (visible, _) = general_head(&mut bits)?;
    (bits.bits_value(4)? == 0b1000).then_some(visible)
}

/// The tag bit (see [`component_head`]; mask A of the label style
/// fields) of a style's dragged-state copy of a component: set in every
/// copy, in no other text, line, block or arrow component.
const DRAGGED_COPY: u32 = 0x0020_0000;

/// X offset, y offset and border gap read by their fields: the general
/// head (see [`general_head`]), `1000`, 5 BL, BD (the readable bias; 0, a
/// `BL`-sized `10`, except in dragged-state copies), B, BD x, BD, BD y,
/// BD, B, BD gap. Also returns the bit after the gap.
fn general_offsets(data: &[u8], at: usize, end: usize) -> Option<(f64, f64, f64, usize)> {
    let mut bits = Bits::new(data, at, end);
    general_head(&mut bits)?;
    (bits.bits_value(4)? == 0b1000).then_some(())?;
    for _ in 0..5 {
        bits.bit_long()?;
    }
    bits.bit_double()?;
    bits.bit()?;
    let x = bits.bit_double()?;
    bits.bit_double()?;
    let y = bits.bit_double()?;
    bits.bit_double()?;
    bits.bit()?;
    let gap = bits.bit_double()?;
    Some((x, y, gap, bits.pos))
}

/// A `CMC` color: BS, BL with the color method in its top byte, RC.
fn color(bits: &mut Bits) -> Option<()> {
    bits.bit_short()?;
    let rgb = bits.bit_long()? as u32;
    matches!(rgb >> 24, 0xC0..=0xC3 | 0xC8).then_some(())?;
    bits.raw_short_byte().map(|_| ())
}

/// Where the content string starts, read on from the gap: 4 bits, BD,
/// color, BL, color, BL, BL, color, BL, BL, BL, `BL n`, `n` BLs, BL, BL
/// (63, 0 in text components).
fn content_at(data: &[u8], pos: usize, end: usize) -> Option<usize> {
    let mut bits = Bits::new(data, pos, end);
    bits.skip(4)?;
    bits.bit_double()?;
    color(&mut bits)?;
    bits.bit_long()?;
    color(&mut bits)?;
    bits.bit_long()?;
    bits.bit_long()?;
    color(&mut bits)?;
    for _ in 0..3 {
        bits.bit_long()?;
    }
    let n = bits.bit_long()?;
    (0..=64).contains(&n).then_some(())?;
    for _ in 0..n + 2 {
        bits.bit_long()?;
    }
    Some(bits.pos)
}

/// The content string at `pos`: `BL byte length`, UTF-16 code units, the
/// last one 0. Returns the text and the bit after it.
fn content(data: &[u8], pos: usize, end: usize) -> Option<(String, usize)> {
    let mut bits = Bits::new(data, pos, end);
    let length = usize::try_from(bits.bit_long()?).ok()?;
    (length >= 2 && length % 2 == 0 && length * 8 <= bits.remaining()).then_some(())?;
    let mut units = Vec::with_capacity(length / 2);
    for _ in 0..length / 2 {
        units.push(u16::from_le_bytes([bits.raw_short_byte()?, bits.raw_short_byte()?]));
    }
    (units.pop() == Some(0)).then_some(())?;
    Some((String::from_utf16(&units).ok()?, bits.pos))
}

/// A label text component read by its fields: the component head (see
/// [`component_head`]; a height of 1.0, a drawing unit, is the unset
/// one), the general block and its offsets (see [`general_offsets`]), the
/// content (see [`content_at`]), then BD rotation, BD, BS attachment.
pub(super) fn text_component(data: &[u8], start: usize, end: usize) -> Option<LabelText> {
    let (height, tag, general) = component_head(data, start, end)?;
    let height = (0.0..1.0).contains(&height).then_some(height).filter(|v| *v > 0.0);
    let copy = tag & DRAGGED_COPY != 0;
    let (x_offset, y_offset, gap, after_gap) = general_offsets(data, general, end)?;
    let (content, after) = content(data, content_at(data, after_gap, end)?, end)?;
    let mut bits = Bits::new(data, after, end);
    let rotation = bits.bit_double()?;
    bits.bit_double()?;
    let attachment = u8::try_from(bits.bit_short()?).ok().filter(|v| *v <= 8)?;
    // The anchor point, when the component's settings block (opening
    // 1011) follows it.
    let mut bits = Bits::new(data, general, end);
    let (visible, point) = general_head(&mut bits)?;
    bits.pos -= 20;
    let anchor_point = (bits.bits_value(4)? == 0b1011).then_some(point);
    Some(LabelText {
        content: parse_format(&content),
        height,
        x_offset,
        y_offset,
        gap,
        rotation,
        attachment,
        anchor_point,
        visible,
        inherits_offset: [copy && tag & 0x40 == 0, copy && tag & 0x80 == 0],
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Bits as written, padded to whole bytes.
    fn bytes(bits: &str) -> Vec<u8> {
        let mut bits = bits.to_string();
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        bits.as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect()
    }

    fn raw(le: &[u8]) -> String {
        le.iter().map(|b| format!("{b:08b}")).collect()
    }

    fn rc(v: u8) -> String {
        format!("{v:08b}")
    }

    fn bl(v: u32) -> String {
        match v {
            0 => "10".into(),
            1..=255 => format!("01{}", rc(v as u8)),
            _ => format!("00{}", raw(&v.to_le_bytes())),
        }
    }

    fn bd(v: f64) -> String {
        format!("00{}", raw(&v.to_le_bytes()))
    }

    fn header() -> String {
        let mut s: String = [80, 0, 80, 80, 0, 7].into_iter().map(bl).collect();
        s += "010010";
        s += &"0".repeat(128);
        s + &bl(80) + &bl(80) + &bl(0x04BE_0000) + &bl(3) + "000"
    }

    /// A label at `station` naming profile entity `entity`, dragged by
    /// `drag`.
    fn label(station: f64, entity: u32, drag: Option<(f64, f64)>) -> String {
        nesting(station, entity, drag, &[])
    }

    /// A nested label: one key, the 2 bits, RC, BL, its key ids and its
    /// own nested labels; nested labels carry no overrides.
    fn nested(inner: &[String]) -> String {
        let s = bl(1) + &bl(0x0200_D61D) + &bd(1.0) + "00" + &rc(1) + &bl(0x1001) + &bl(1) + &bl(0x0200_D61D);
        s + &bl(inner.len() as u32) + &inner.concat()
    }

    /// [`label`] holding the `inner` nested labels.
    fn nesting(station: f64, entity: u32, drag: Option<(f64, f64)>, inner: &[String]) -> String {
        let mut s = bl(2) + &bl(STATION) + &bd(station) + &bl(0x0100_FC31) + &bl(entity);
        s += &("00".to_string() + &rc(0) + &bl(0));
        s += &(bl(1) + &bl(5));
        s += &(bl(inner.len() as u32) + &inner.concat());
        match drag {
            None => s += &bl(0),
            Some((x, y)) => {
                s += &(bl(1) + &bl(1) + &bl(0) + &bl(0) + "00" + &rc(8));
                s += &(rc(DRAGGED_OFFSET) + &rc(0xC0) + &rc(0x0C) + "0001001");
                s += &(bd(x) + &bd(y) + &bd(0.0) + "010");
            }
        }
        s
    }

    fn settings() -> String {
        bl(0) + &bl(69) + &bl(1) + &bd(0.0) + &bd(0.0) + &bl(0) + &bd(0.025) + &bd(0.05)
    }

    fn close() -> String {
        "0".to_string() + "00" + &raw(&0x04BEu16.to_le_bytes())
    }

    fn record(labels: &[String], tail: &str) -> (Vec<u8>, usize) {
        let s = header() + &bl(labels.len() as u32) + &labels.concat() + &settings() + tail + &close();
        (bytes(&s), s.len())
    }

    #[test]
    fn station_group_reads_its_labels_stations() {
        let tail = "000".to_string() + &bd(0.0) + &bd(60.0) + &bd(20.0);
        let (data, end) = record(&[label(20.0, 0, None), label(40.0, 0, None)], &tail);
        assert_eq!(group_stations(&data, 0, end), Some(vec![20.0, 40.0]));
        assert_eq!(stations(&data, 0, end), Some(vec![20.0, 40.0]));
        // A record that does not close where the settings say is not read.
        assert_eq!(group_stations(&data, 0, end - 1), None);
    }

    #[test]
    fn geometry_point_kinds_are_low_bytes() {
        let tail = "000".to_string() + &bd(0.0) + &bd(60.0) + &bl(2) + &bl(0x0101) + &bl(3);
        let (data, end) = record(&[label(0.0, 0, None)], &tail);
        assert_eq!(group_point_kinds(&data, 0, end), Some(vec![1, 3]));
    }

    #[test]
    fn single_labels_are_anchored_by_their_settings() {
        let tail = bd(12.5) + &bd(-4.0) + "000";
        let (data, end) = record(&[label(5.0, 0, None)], &tail);
        let a = anchor(&data, 0, end, false).unwrap();
        assert_eq!((a.x, a.y), (12.5, -4.0));
        assert_eq!(anchor(&data, 0, end, true), None);
        let tail = bd(1.0) + &bd(2.0) + "000" + &bl(80) + &bl(80) + &bl(0x04BE_0000);
        let (data, end) = record(&[label(5.0, 0, None)], &tail);
        let a = anchor(&data, 0, end, true).unwrap();
        assert_eq!((a.x, a.y), (1.0, 2.0));
    }

    #[test]
    fn dragged_offset_is_the_first_nonzero_override() {
        let (data, end) = record(&[label(0.0, 0, Some((0.0, 0.0))), label(1.0, 0, Some((3.0, -2.0)))], "");
        let d = dragged_offset(&data, 0, end).unwrap().unwrap();
        assert_eq!((d.x, d.y), (3.0, -2.0));
        let (data, end) = record(&[label(0.0, 0, None)], "");
        assert_eq!(dragged_offset(&data, 0, end), Some(None));
    }

    #[test]
    fn entity_ids_point_at_their_values() {
        let (data, end) = record(&[label(0.0, 0x1234, None), label(1.0, 0, None), label(2.0, 7, None)], "");
        let refs = entity_refs(&data, 0, end).unwrap();
        assert_eq!(refs.iter().map(|r| r.1).collect::<Vec<_>>(), vec![0x1234, 7]);
        for (at, id) in refs {
            assert_eq!(Bits::new(&data, at, end).bit_long(), Some(id));
        }
    }

    #[test]
    fn nested_labels_are_read_past() {
        // A superelevation band label: its critical point, the point's two
        // element ends.
        let point = nested(&[nested(&[]), nested(&[])]);
        let (data, end) = record(&[nesting(25.0, 0, None, &[point]), nesting(50.0, 0, Some((1.5, -2.0)), &[nested(&[])])], "");
        assert_eq!(stations(&data, 0, end), Some(vec![25.0, 50.0]));
        let drags = dragged_offsets(&data, 0, end).unwrap();
        assert_eq!(drags.len(), 2);
        assert!(drags[0].is_none());
        assert_eq!(drags[1].map(|d| (d.x, d.y)), Some((1.5, -2.0)));
        // Deeper nesting than any record holds is not read.
        let mut deep = nested(&[]);
        for _ in 0..NESTING {
            deep = nested(&[deep]);
        }
        let (data, end) = record(&[nesting(25.0, 0, None, &[deep])], "");
        assert_eq!(stations(&data, 0, end), None);
    }

    #[test]
    fn override_blocks_end_at_their_last_property() {
        let prop = |id: u8, ty: &str, value: &str, last: bool| rc(id) + &rc(0xC0) + &rc(0x0C) + ty + value + if last { "010" } else { "000" };
        let text: String = "<[1]>".encode_utf16().flat_map(|u| u.to_le_bytes()).map(rc).collect();
        let mut s = bl(1) + &bl(STATION) + &bd(10.0) + "00" + &rc(1) + &bl(0x1003) + &bl(0) + &bl(0);
        // One override of two blocks: a reset flag and the drag (RC 16
        // over three properties, as one application drawing holds), then
        // a text override.
        s += &(bl(1) + &bl(2));
        s += &(bl(0) + &bl(0) + "00" + &rc(16));
        s += &prop(0x5A, "0011001", &bl(1), false);
        s += &prop(0x04, "0000011", &rc(0), false);
        s += &prop(DRAGGED_OFFSET, "0001001", &(bd(4.0) + &bd(-1.0) + &bd(0.0)), true);
        s += &(bl(0) + &bl(0) + "00" + &rc(8));
        s += &prop(0x2C, "0011000", &(bl(10) + &text), true);
        let (data, end) = record(&[s], "");
        assert_eq!(stations(&data, 0, end), Some(vec![10.0]));
        let d = dragged_offset(&data, 0, end).unwrap().unwrap();
        assert_eq!((d.x, d.y), (4.0, -1.0));
    }

    #[test]
    fn settings_hold_the_dimension_anchor_weeding_and_stagger() {
        // BL 69, BL 2 (below), BD 0.0625, BD weeding 12.5, BL stagger 2,
        // BD, BD; then a station group's range and increment.
        let settings = bl(0) + &bl(69) + &bl(2) + &bd(0.0625) + &bd(12.5) + &bl(2) + &bd(0.025) + &bd(0.005);
        let tail = "000".to_string() + &bd(0.0) + &bd(60.0) + &bd(20.0);
        let s = header() + &bl(1) + &label(20.0, 0, None) + &settings + &tail + &close();
        let data = bytes(&s);
        assert_eq!(dimension_anchor(&data, 0, s.len()), Some(LabelDimensionAnchor { option: 2, value: 0.0625 }));
        assert_eq!(group_stations(&data, 0, s.len()), Some(vec![20.0]));
    }

    #[test]
    fn a_128_bit_key_is_read_past() {
        // Sectional data band labels open with keys of type 0x15.
        let mut s = bl(3) + &bl(0x1501_7822) + &"0".repeat(128) + &bl(0x0F01_7824) + &bl(STATION) + &bd(329.5);
        s += &("00".to_string() + &rc(1) + &bl(0x9001) + &bl(0) + &bl(0) + &bl(0));
        let (data, end) = record(&[s], "");
        assert_eq!(stations(&data, 0, end), Some(vec![329.5]));
    }

    #[test]
    fn general_head_reads_visibility_and_anchor_point() {
        let s = "0100".to_string() + "10000000" + "1011" + &"0".repeat(20) + "1000";
        let data = bytes(&s);
        let mut bits = Bits::new(&data, 0, s.len());
        assert_eq!(general_head(&mut bits), Some((true, 0b1011)));
        assert_eq!(bits.bits_value(4), Some(0b1000));
        let s = "0000".to_string() + "00000000" + &"1".repeat(32) + "0010" + &"0".repeat(20);
        let data = bytes(&s);
        assert_eq!(general_head(&mut Bits::new(&data, 0, s.len())), Some((false, 0b0010)));
    }

    #[test]
    fn offsets_follow_the_readable_bias() {
        let head = "0100".to_string() + "10000000" + "0000" + &"0".repeat(20) + "1000";
        let block = "0".to_string() + &bd(0.125) + &bd(0.0) + &bd(-0.0625) + &bd(0.0) + "0" + &bd(0.03125);
        let normal = head.clone() + &bl(0).repeat(3) + &bl(1) + &bl(0) + &bl(0) + &block;
        let data = bytes(&normal);
        assert_eq!(general_offsets(&data, 0, normal.len()), Some((0.125, -0.0625, 0.03125, normal.len())));
        // A dragged-state copy writes its readable bias in full.
        let turned = head + &bl(0).repeat(3) + &bl(1) + &bl(1) + &bd(1.919_862_177_193_762_5) + &block;
        let data = bytes(&turned);
        assert_eq!(general_offsets(&data, 0, turned.len()), Some((0.125, -0.0625, 0.03125, turned.len())));
    }

    #[test]
    fn content_is_a_terminated_utf16_string() {
        let units: String = "A\nb\0".encode_utf16().flat_map(|u| u.to_le_bytes()).map(rc).collect();
        let s = bl(8) + &units;
        let data = bytes(&s);
        assert_eq!(content(&data, 0, s.len()), Some(("A\nb".to_string(), s.len())));
        let s = bl(2) + &rc(b'x') + &rc(0);
        assert_eq!(content(&bytes(&s), 0, s.len()), None);
    }

    // ---- The station-type records (the 60 the class-table gap refused;
    // ---- the classes are registered in `mod.rs`'s ENTITY_CLASSES) ----

    /// The bytes of a hex literal.
    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
    }

/// One oracle record's capture: the class-body bytes from the byte
/// holding the body's first bit, the body's bit offset in that byte,
/// its bit length, its label count, its labels' stations, the
/// settings' dimension anchor (option, value), the class run's bit
/// count and the settings' reference key (the corpus's 60
/// station-type records: Assembly-2a/2b/2c, Mass Haul-1/2, Pipe
/// Networks-1B/1C/2/2B/3/3B/3C, Profile-3A/3B/5A/5B/5D/6A/6B/6C).
struct StationCapture {
    file: &'static str,
    handle: &'static str,
    dxf: &'static str,
    raw: &'static str,
    offset: usize,
    bits: usize,
    labels: usize,
    stations: &'static [f64],
    anchor: (u32, f64),
    run_bits: usize,
    settings_key: u32,
}

const STATION_CAPTURES: &[StationCapture] = &[
    StationCapture { file: "Assembly-2a.dwg", handle: "0x1043", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0122481845ac0006020feb00010000000000000b2800d7f800d000020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe003400106800010000000000000b2800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000000034a00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe003400106800010000000000180e4800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a000040000000000003ca00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe003400106800010000000000080fe800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000006041200020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000001c10a800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000000044a00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe00340010680001000000000004118800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000002047a00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe00340010680001000000000006122800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000006049200020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000000a128800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a000040000000000704aa00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000000e12e800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a000040000000000004ca00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe00340011eb0001808020010000020810680001045ac000611eb0001831fe001f48081b68001ea2a02000000000000601fd0aaaaaaaaaaab6a7e2aaaaaaaaaaae29ff80068304081ea99405415000002f810000000000000594017c089", offset: 5, bits: 8694, labels: 18, stations: &[100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0, 1500.0, 1600.0], anchor: (1, 0.125), run_bits: 191, settings_key: 0xf00b40d },
    StationCapture { file: "Assembly-2a.dwg", handle: "0x1044", dxf: "AECC_VALIGNMENT_MINOR_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc012228203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000000092800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000006031200020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b000100000000000009280106800010000000000080de800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a000040000000000703aa00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b000100000000000009280106800010000000000040f8800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000001840a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b0001000000000000092801068000100000000000a108800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000003843a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000012114800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000005846a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b0001000000000000092801068000100000000000d120800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000007c48a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000011126800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000000c4aa00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b0001000000000000092801068000100000000001512c800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000001c4c200020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000019132800080040000081041a000040c7f8007d202015a0007a8a80d42aaaaaaaaaaada9f8aaaaaaaaaaab8a7fe9505400000be04000000000000125005f0224", offset: 5, bits: 8780, labels: 17, stations: &[50.0, 150.0, 250.0, 350.0, 450.0, 550.0, 650.0, 750.0, 850.0, 950.0, 1050.0, 1150.0, 1250.0, 1350.0, 1450.0, 1550.0, 1650.0], anchor: (1, 0.0), run_bits: 127, settings_key: 0xf00b402 },
    StationCapture { file: "Assembly-2a.dwg", handle: "0x1045", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0120c82035fe0034000eeb00014346800008a7a000080f6b0000a0400802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb00012346800008a9a000080f6b0000a0400802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb00014346800008a7a000080f6b0000a0800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb00012346800008a9a000080f6b0000a0800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb0001434680000809a000040f6b0000a0a00802400000828d1a000020f6b0000843ac00020eeb0001035fe0035482035fe0034000eeb000123468000080ba000040f6b0000a0200802400000828d1a000020f6b0000843ac00020eeb0001035fe003548081f68001ea2a0350aaaaaaaaaaab6a7e2aaaaaaaaaaae29ff80068304081ea99405415000002f8110d0040400000201000000c040000040100000140400000601000001c040000080100000240400000a01000002c0400000c0100000340400005f022", offset: 5, bits: 3672, labels: 6, stations: &[], anchor: (1, 0.0), run_bits: 577, settings_key: 0xf00b40f },
    StationCapture { file: "Assembly-2b.dwg", handle: "0x1043", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0122481845ac0006020d7f800d0003fac00040000000000002ca00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe003400106800010000000000000b2800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a0000400000000000034a00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe003400106800010000000000180e4800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a000040000000000003ca00020010000020810680001045ac000611eb0001831fe001f481835fe0034000feb00010000000000000b280106800010000000000080fe800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000006041200020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000001c10a800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000000044a00020010000020810680001045ac000611eb0001831fe001f481835fe0034000feb00010000000000000b28010680001000000000004118800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a0000400000000002047a00020010000020810680001045ac000611eb0001831fe001f481835fe0034000feb00010000000000000b28010680001000000000006122800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a0000400000000006049200020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000000a128800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a000040000000000704aa00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000000e12e800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a000040000000000004ca00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe00340011eb0001808020010000020810680001045ac000611eb0001831fe001f48081b68001ea2a02000000000000601fd0aaaaaaaaaaab6a7e2aaaaaaaaaaae29ff80068304081ea99405415000002f810000000000000594017c089", offset: 5, bits: 8694, labels: 18, stations: &[100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0, 1500.0, 1600.0], anchor: (1, 0.125), run_bits: 191, settings_key: 0xf00b40d },
    StationCapture { file: "Assembly-2b.dwg", handle: "0x1044", dxf: "AECC_VALIGNMENT_MINOR_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0122282035fe0034000feb00010000000000000b280106b00010000000000000928010680001000000000000092800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000006031200020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b000100000000000009280106800010000000000080de800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a000040000000000703aa00020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b000100000000000009280106800010000000000040f8800080040000081041a000040c7f8007d2080d7f800d0003fac00040000000000002ca0041ac000400000000000024a0041a0000400000000001840a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b0001000000000000092801068000100000000000a108800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000003843a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000012114800080040000081041a000040c7f8007d2080d7f800d0003fac00040000000000002ca0041ac000400000000000024a0041a0000400000000005846a00020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b0001000000000000092801068000100000000000d120800080040000081041a000040c7f8007d2080d7f800d0003fac00040000000000002ca0041ac000400000000000024a0041a0000400000000007c48a00020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b00010000000000000928010680001000000000011126800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000000c4aa00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b0001000000000000092801068000100000000001512c800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000001c4c200020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000019132800080040000081041a000040c7f8007d202015a0007a8a80d42aaaaaaaaaaada9f8aaaaaaaaaaab8a7fe9505400000be04000000000000125005f0224", offset: 5, bits: 8780, labels: 17, stations: &[50.0, 150.0, 250.0, 350.0, 450.0, 550.0, 650.0, 750.0, 850.0, 950.0, 1050.0, 1150.0, 1250.0, 1350.0, 1450.0, 1550.0, 1650.0], anchor: (1, 0.0), run_bits: 127, settings_key: 0xf00b402 },
    StationCapture { file: "Assembly-2b.dwg", handle: "0x1045", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0120c82035fe0034000eeb00014346800008a7a000080f6b0000a0400802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb000120f6b0000a04346800008a9a0000800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb00014346800008a7a000080f6b0000a0800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb000120f6b0000a08346800008a9a0000800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb0001434680000809a000040f6b0000a0a00802400000828d1a000020f6b0000843ac00020eeb0001035fe0035482035fe0034000eeb000123468000080ba000040f6b0000a0200802400000828d1a000020f6b0000843ac00020eeb0001035fe003548081f68001ea2a0350aaaaaaaaaaab6a7e2aaaaaaaaaaae29ff80068304081ea99405415000002f8110d0040400000201000000c040000040100000140400000601000001c040000080100000240400000a01000002c0400000c0100000340400005f022", offset: 5, bits: 3672, labels: 6, stations: &[], anchor: (1, 0.0), run_bits: 577, settings_key: 0xf00b40f },
    StationCapture { file: "Assembly-2c.dwg", handle: "0x1043", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0122481845ac0006020d7f800d0003fac00040000000000002ca00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe003400106800010000000000000b2800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a0000400000000000034a00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe003400106800010000000000180e4800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a000040000000000003ca00020010000020810680001045ac000611eb0001831fe001f481835fe0034000feb00010000000000000b280106800010000000000080fe800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000006041200020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000001c10a800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a0000400000000000044a00020010000020810680001045ac000611eb0001831fe001f481835fe0034000feb00010000000000000b28010680001000000000004118800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a0000400000000002047a00020010000020810680001045ac000611eb0001831fe001f481835fe0034000feb00010000000000000b28010680001000000000006122800080040000082041a00004116b0001847ac00060c7f8007d2060d7f800d0003fac00040000000000002ca0041a0000400000000006049200020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000000a128800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a000040000000000704aa00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe0034001068000100000000000e12e800080040000082041a00004116b0001847ac00060c7f8007d2060feb00010000000000000b2800d7f800d00041a000040000000000004ca00020010000020810680001045ac000611eb0001831fe001f48183fac00040000000000002ca0035fe00340011eb0001808020010000020810680001045ac000611eb0001831fe001f48081b68001ea2a02000000000000601fd0aaaaaaaaaaab6a7e2aaaaaaaaaaae29ff80068304081ea99405415000002f810000000000000594017c089", offset: 5, bits: 8694, labels: 18, stations: &[100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0, 1500.0, 1600.0], anchor: (1, 0.125), run_bits: 191, settings_key: 0xf00b40d },
    StationCapture { file: "Assembly-2c.dwg", handle: "0x1044", dxf: "AECC_VALIGNMENT_MINOR_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0122282035fe0034000feb00010000000000000b280106b00010000000000000928010680001000000000000092800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000006031200020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b000100000000000009280106800010000000000080de800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a000040000000000703aa00020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b000100000000000009280106800010000000000040f8800080040000081041a000040c7f8007d2080d7f800d0003fac00040000000000002ca0041ac000400000000000024a0041a0000400000000001840a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b0001000000000000092801068000100000000000a108800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000003843a00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000012114800080040000081041a000040c7f8007d2080d7f800d0003fac00040000000000002ca0041ac000400000000000024a0041a0000400000000005846a00020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b0001000000000000092801068000100000000000d120800080040000081041a000040c7f8007d2080d7f800d0003fac00040000000000002ca0041ac000400000000000024a0041a0000400000000007c48a00020010000020410680001031fe001f482035fe0034000feb00010000000000000b280106b00010000000000000928010680001000000000011126800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000000c4aa00020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b0001000000000000092801068000100000000001512c800080040000081041a000040c7f8007d2080feb00010000000000000b2800d7f800d00041ac000400000000000024a0041a0000400000000001c4c200020010000020410680001031fe001f48203fac00040000000000002ca0035fe003400106b00010000000000000928010680001000000000019132800080040000081041a000040c7f8007d202015a0007a8a80d42aaaaaaaaaaada9f8aaaaaaaaaaab8a7fe9505400000be04000000000000125005f0224", offset: 5, bits: 8780, labels: 17, stations: &[50.0, 150.0, 250.0, 350.0, 450.0, 550.0, 650.0, 750.0, 850.0, 950.0, 1050.0, 1150.0, 1250.0, 1350.0, 1450.0, 1550.0, 1650.0], anchor: (1, 0.0), run_bits: 127, settings_key: 0xf00b402 },
    StationCapture { file: "Assembly-2c.dwg", handle: "0x1045", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082c800000000000000000000000000000001505400000be043fffffc0120c82035fe0034000eeb00014346800008a7a000080f6b0000a0400802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb000120f6b0000a04346800008a9a0000800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb00014346800008a7a000080f6b0000a0800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb000120f6b0000a08346800008a9a0000800802400000828d1a000020f6b0000843ac00020eeb0001031fe001f482035fe0034000eeb0001434680000809a000040f6b0000a0a00802400000828d1a000020f6b0000843ac00020eeb0001035fe0035482035fe0034000eeb000123468000080ba000040f6b0000a0200802400000828d1a000020f6b0000843ac00020eeb0001035fe003548081f68001ea2a0350aaaaaaaaaaab6a7e2aaaaaaaaaaae29ff80068304081ea99405415000002f8110d0040400000201000000c040000040100000140400000601000001c040000080100000240400000a01000002c0400000c0100000340400005f022", offset: 5, bits: 3672, labels: 6, stations: &[], anchor: (1, 0.0), run_bits: 577, settings_key: 0xf00b40f },
    StationCapture { file: "Mass Haul-1.dwg", handle: "0x1043", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe0091140c22d600030106bfc0068001fd6000200000000000016500010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a0008340000800000000000059400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d000020000000000001a500010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a000834000080000000000c072400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d000020000000000001e500010008000010408340000822d6000308f58000c18ff000fa40c1aff001a0007f58000800000000000059400834000080000000000407f400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000003020900010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a000834000080000000000e085400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000000022500010008000010408340000822d6000308f58000c18ff000fa40c1aff001a0007f58000800000000000059400834000080000000000208c400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d0000200000000001023d00010008000010408340000822d6000308f58000c18ff000fa40c1aff001a0007f580008000000000000594008340000800000000003091400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d0000200000000003024900010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a0008340000800000000005094400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000003825500010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a0008340000800000000007097400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000000026500010008000010408340000822d6000308f58000c18ff000fa4040db4000f51501000000000000300fe8555555555555b53f155555555555714fe40034182040f54ca02a0a8000017c080000000000002ca00be044", offset: 5, bits: 8303, labels: 17, stations: &[100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0, 1500.0, 1600.0], anchor: (1, 0.125), run_bits: 191, settings_key: 0xf00b40d },
    StationCapture { file: "Mass Haul-1.dwg", handle: "0x1044", dxf: "AECC_VALIGNMENT_MINOR_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe009114101aff001a0007f5800080000000000005940083580008000000000000494008340000800000000000049400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000003018900010008000010208340000818ff000fa4101aff001a0007f580008000000000000594008358000800000000000049400834000080000000000406f400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d000020000000000381d500010008000010208340000818ff000fa4101aff001a0007f580008000000000000594008358000800000000000049400834000080000000000207c400040020000040820d00002063fc003e90406bfc0068001fd600020000000000001650020d600020000000000001250020d0000200000000000c20500010008000010208340000818ff000fa4101fd60002000000000000165001aff001a00083580008000000000000494008340000800000000005084400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000001c21d00010008000010208340000818ff000fa4101fd60002000000000000165001aff001a0008358000800000000000049400834000080000000000908a400040020000040820d00002063fc003e90406bfc0068001fd600020000000000001650020d600020000000000001250020d0000200000000002c23500010008000010208340000818ff000fa4101aff001a0007f5800080000000000005940083580008000000000000494008340000800000000006890400040020000040820d00002063fc003e90406bfc0068001fd600020000000000001650020d600020000000000001250020d0000200000000003e24500010008000010208340000818ff000fa4101aff001a0007f5800080000000000005940083580008000000000000494008340000800000000008893400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000000625500010008000010208340000818ff000fa4101fd60002000000000000165001aff001a0008358000800000000000049400834000080000000000a896400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000000e26100010008000010208340000818ff000fa4101fd60002000000000000165001aff001a0008358000800000000000049400834000080000000000c899400040020000040820d00002063fc003e90100ad0003d45406a1555555555556d4fc555555555555c53ff4a82a000005f02000000000000092802f811", offset: 5, bits: 8777, labels: 17, stations: &[50.0, 150.0, 250.0, 350.0, 450.0, 550.0, 650.0, 750.0, 850.0, 950.0, 1050.0, 1150.0, 1250.0, 1350.0, 1450.0, 1550.0, 1650.0], anchor: (1, 0.0), run_bits: 127, settings_key: 0xf00b402 },
    StationCapture { file: "Mass Haul-1.dwg", handle: "0x1045", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d6000107758000818ff000fa4040fb4000f51501a8555555555555b53f155555555555714ffc0034182040f54ca02a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.0), run_bits: 577, settings_key: 0xf00b40f },
    StationCapture { file: "Mass Haul-2.dwg", handle: "0x1043", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe0091140c22d600030106bfc0068001fd6000200000000000016500010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a0008340000800000000000059400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d000020000000000001a500010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a000834000080000000000c072400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d000020000000000001e500010008000010408340000822d6000308f58000c18ff000fa40c1aff001a0007f58000800000000000059400834000080000000000407f400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000003020900010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a000834000080000000000e085400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000000022500010008000010408340000822d6000308f58000c18ff000fa40c1aff001a0007f58000800000000000059400834000080000000000208c400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d0000200000000001023d00010008000010408340000822d6000308f58000c18ff000fa40c1aff001a0007f580008000000000000594008340000800000000003091400040020000041020d0000208b58000c23d60003063fc003e90306bfc0068001fd600020000000000001650020d0000200000000003024900010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a0008340000800000000005094400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000003825500010008000010408340000822d6000308f58000c18ff000fa40c1fd60002000000000000165001aff001a0008340000800000000007097400040020000041020d0000208b58000c23d60003063fc003e90307f580008000000000000594006bfc00680020d0000200000000000026500010008000010408340000822d6000308f58000c18ff000fa4040db4000f51501000000000000300fe8555555555555b53f155555555555714fe40034182040f54ca02a0a8000017c080000000000002ca00be044", offset: 5, bits: 8303, labels: 17, stations: &[100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 1100.0, 1200.0, 1300.0, 1400.0, 1500.0, 1600.0], anchor: (1, 0.125), run_bits: 191, settings_key: 0xf00b40d },
    StationCapture { file: "Mass Haul-2.dwg", handle: "0x1044", dxf: "AECC_VALIGNMENT_MINOR_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe009114101aff001a0007f5800080000000000005940083580008000000000000494008340000800000000000049400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000003018900010008000010208340000818ff000fa4101aff001a0007f580008000000000000594008358000800000000000049400834000080000000000406f400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d000020000000000381d500010008000010208340000818ff000fa4101aff001a0007f580008000000000000594008358000800000000000049400834000080000000000207c400040020000040820d00002063fc003e90406bfc0068001fd600020000000000001650020d600020000000000001250020d0000200000000000c20500010008000010208340000818ff000fa4101fd60002000000000000165001aff001a00083580008000000000000494008340000800000000005084400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000001c21d00010008000010208340000818ff000fa4101fd60002000000000000165001aff001a0008358000800000000000049400834000080000000000908a400040020000040820d00002063fc003e90406bfc0068001fd600020000000000001650020d600020000000000001250020d0000200000000002c23500010008000010208340000818ff000fa4101aff001a0007f5800080000000000005940083580008000000000000494008340000800000000006890400040020000040820d00002063fc003e90406bfc0068001fd600020000000000001650020d600020000000000001250020d0000200000000003e24500010008000010208340000818ff000fa4101aff001a0007f5800080000000000005940083580008000000000000494008340000800000000008893400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000000625500010008000010208340000818ff000fa4101fd60002000000000000165001aff001a0008358000800000000000049400834000080000000000a896400040020000040820d00002063fc003e90407f580008000000000000594006bfc00680020d600020000000000001250020d0000200000000000e26100010008000010208340000818ff000fa4101fd60002000000000000165001aff001a0008358000800000000000049400834000080000000000c899400040020000040820d00002063fc003e90100ad0003d45406a1555555555556d4fc555555555555c53ff4a82a000005f02000000000000092802f811", offset: 5, bits: 8777, labels: 17, stations: &[50.0, 150.0, 250.0, 350.0, 450.0, 550.0, 650.0, 750.0, 850.0, 950.0, 1050.0, 1150.0, 1250.0, 1350.0, 1450.0, 1550.0, 1650.0], anchor: (1, 0.0), run_bits: 127, settings_key: 0xf00b402 },
    StationCapture { file: "Mass Haul-2.dwg", handle: "0x1045", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d6000107758000818ff000fa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d6000107758000818ff000fa4040fb4000f51501a8555555555555b53f155555555555714ffc0034182040f54ca02a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.0), run_bits: 577, settings_key: 0xf00b40f },
    StationCapture { file: "Pipe Networks-1B.dwg", handle: "0x79f", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082460007200f80064006c006a006e00f800a82a000005f021fffffe0092940c22d600030107f580008000000000000494006bfc006800010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000a0040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000059400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003018900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000069400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000101bd00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c072400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000381d500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000079400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d000020000000000081f100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000407f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000c20500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c082400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001421100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e085400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001c21d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000089400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002422900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000208c400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002c23500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000408f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000001a24100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000003091400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003e24500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000c092400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002224d00010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000005094400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000625500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e095400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002a25900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000007097400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000e26100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000099400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003226500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000909a400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001626d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000209c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000021e8e50e542b1e7100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e89c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c27500010008000010408340000822d6000308f58000c1aff001aa4040db4000f51501a8555555555555b53f155555555555714ff8e43114869e1bef7e763ad22ec0e84ea02a0a8000017c0800000000000024a00be044", offset: 5, bits: 18943, labels: 41, stations: &[0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0, 1300.0, 1350.0, 1400.0, 1450.0, 1500.0, 1550.0, 1600.0, 1650.0, 1700.0, 1750.0, 1800.0, 1841.760335016616, 1850.0, 1900.0], anchor: (1, 0.0), run_bits: 255, settings_key: 0xf00b40d },
    StationCapture { file: "Pipe Networks-1C.dwg", handle: "0x79f", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082460007200f80064006c006a006e00f800a82a000005f021fffffe0092940c22d600030106bfc0068001fd6000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000a0040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000059400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003018900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000069400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000101bd00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c072400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000381d500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000079400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000081f100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000407f400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000c20500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c082400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001421100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e085400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001c21d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000089400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002422900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000208c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c23500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000408f400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001a24100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000003091400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003e24500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c092400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002224d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000005094400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000625500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e095400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002a25900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000007097400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000e26100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000099400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003226500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000909a400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001626d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000209c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000021e8e50e542b1e7100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e89c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c27500010008000010408340000822d6000308f58000c1aff001aa4040db4000f51501a8555555555555b53f155555555555714ff8e43114869e1bef7e763ad22ec0e84ea02a0a8000017c0800000000000024a00be044", offset: 5, bits: 18943, labels: 41, stations: &[0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0, 1300.0, 1350.0, 1400.0, 1450.0, 1500.0, 1550.0, 1600.0, 1650.0, 1700.0, 1750.0, 1800.0, 1841.760335016616, 1850.0, 1900.0], anchor: (1, 0.0), run_bits: 255, settings_key: 0xf00b40d },
    StationCapture { file: "Pipe Networks-2.dwg", handle: "0x79f", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082460007200f80064006c006a006e00f800a82a000005f021fffffe0092940c22d600030106bfc0068001fd6000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000a0040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000059400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003018900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000069400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000101bd00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c072400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000381d500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000079400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000081f100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000407f400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000c20500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c082400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001421100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e085400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001c21d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000089400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002422900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000208c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c23500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000408f400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001a24100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000003091400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003e24500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c092400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002224d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000005094400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000625500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e095400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002a25900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000007097400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000e26100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000099400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003226500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000909a400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001626d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000209c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000021e8e50e542b1e7100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e89c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c27500010008000010408340000822d6000308f58000c1aff001aa4040db4000f51501a8555555555555b53f155555555555714ff8e43114869e1bef7e763ad22ec0e84ea02a0a8000017c0800000000000024a00be044", offset: 5, bits: 18943, labels: 41, stations: &[0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0, 1300.0, 1350.0, 1400.0, 1450.0, 1500.0, 1550.0, 1600.0, 1650.0, 1700.0, 1750.0, 1800.0, 1841.760335016616, 1850.0, 1900.0], anchor: (1, 0.0), run_bits: 255, settings_key: 0xf00b40d },
    StationCapture { file: "Pipe Networks-2B.dwg", handle: "0x79f", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082460007200f80064006c006a006e00f800a82a000005f021fffffe0092940c22d600030107f580008000000000000494006bfc006800010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000a0040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000059400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003018900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000069400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000101bd00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c072400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000381d500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000079400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d000020000000000081f100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000407f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000c20500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c082400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001421100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e085400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001c21d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000089400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002422900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000208c400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002c23500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000408f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000001a24100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000003091400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003e24500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000c092400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002224d00010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000005094400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000625500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e095400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002a25900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000007097400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000e26100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000099400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003226500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000909a400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001626d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000209c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000021e8e50e542b1e7100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e89c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c27500010008000010408340000822d6000308f58000c1aff001aa4040db4000f51501a8555555555555b53f155555555555714ff8e43114869e1bef7e763ad22ec0e84ea02a0a8000017c0800000000000024a00be044", offset: 5, bits: 18943, labels: 41, stations: &[0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0, 1300.0, 1350.0, 1400.0, 1450.0, 1500.0, 1550.0, 1600.0, 1650.0, 1700.0, 1750.0, 1800.0, 1841.760335016616, 1850.0, 1900.0], anchor: (1, 0.0), run_bits: 255, settings_key: 0xf00b40d },
    StationCapture { file: "Pipe Networks-3.dwg", handle: "0x79f", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082460007200f80064006c006a006e00f800a82a000005f021fffffe0092940c22d600030107f580008000000000000494006bfc006800010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000a0040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000059400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003018900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000069400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000101bd00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c072400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000381d500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000079400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d000020000000000081f100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000407f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000c20500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c082400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001421100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e085400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001c21d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000089400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002422900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000208c400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002c23500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000408f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000001a24100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000003091400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003e24500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000c092400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002224d00010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000005094400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000625500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e095400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002a25900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000007097400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000e26100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000099400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003226500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000909a400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001626d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000209c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000021e8e50e542b1e7100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e89c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c27500010008000010408340000822d6000308f58000c1aff001aa4040db4000f51501a8555555555555b53f155555555555714ff8e43114869e1bef7e763ad22ec0e84ea02a0a8000017c0800000000000024a00be044", offset: 5, bits: 18943, labels: 41, stations: &[0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0, 1300.0, 1350.0, 1400.0, 1450.0, 1500.0, 1550.0, 1600.0, 1650.0, 1700.0, 1750.0, 1800.0, 1841.760335016616, 1850.0, 1900.0], anchor: (1, 0.0), run_bits: 255, settings_key: 0xf00b40d },
    StationCapture { file: "Pipe Networks-3B.dwg", handle: "0x79f", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082460007200f80064006c006a006e00f800a82a000005f021fffffe0092940c22d600030107f580008000000000000494006bfc006800010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000a0040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000059400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003018900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000069400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000101bd00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c072400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000381d500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000079400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d000020000000000081f100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000407f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000c20500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c082400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001421100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e085400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001c21d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000089400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002422900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000208c400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002c23500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000408f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000001a24100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000003091400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003e24500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000c092400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002224d00010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000005094400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000625500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e095400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002a25900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000007097400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000e26100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000099400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003226500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000909a400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001626d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000209c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000021e8e50e542b1e7100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e89c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c27500010008000010408340000822d6000308f58000c1aff001aa4040db4000f51501a8555555555555b53f155555555555714ff8e43114869e1bef7e763ad22ec0e84ea02a0a8000017c0800000000000024a00be044", offset: 5, bits: 18943, labels: 41, stations: &[0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0, 1300.0, 1350.0, 1400.0, 1450.0, 1500.0, 1550.0, 1600.0, 1650.0, 1700.0, 1750.0, 1800.0, 1841.760335016616, 1850.0, 1900.0], anchor: (1, 0.0), run_bits: 255, settings_key: 0xf00b40d },
    StationCapture { file: "Pipe Networks-3C.dwg", handle: "0x79f", dxf: "AECC_VALIGNMENT_STATION_LABEL_GROUP", raw: "eaa12a0a84022f81082460007200f80064006c006a006e00f800a82a000005f021fffffe0092940c22d600030107f580008000000000000494006bfc006800010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000a0040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000012500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000059400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003018900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000069400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000101bd00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c072400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000020000000000381d500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000000079400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d000020000000000081f100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000407f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000000c20500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000c082400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001421100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e085400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001c21d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000089400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002422900010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000208c400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002c23500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000408f400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000001a24100010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000003091400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000003e24500010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a000834000080000000000c092400040020000041020d0000208b58000c23d6000306bfc006a90307f580008000000000000494006bfc00680020d0000200000000002224d00010008000010408340000822d6000308f58000c1aff001aa40c1fd60002000000000000125001aff001a0008340000800000000005094400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000625500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e095400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002a25900010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000007097400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000000e26100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f580008000000000000494008340000800000000000099400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000003226500010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000909a400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000001626d00010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000209c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d000021e8e50e542b1e7100010008000010408340000822d6000308f58000c1aff001aa40c1aff001a0007f58000800000000000049400834000080000000000e89c400040020000041020d0000208b58000c23d6000306bfc006a90306bfc0068001fd600020000000000001250020d0000200000000002c27500010008000010408340000822d6000308f58000c1aff001aa4040db4000f51501a8555555555555b53f155555555555714ff8e43114869e1bef7e763ad22ec0e84ea02a0a8000017c0800000000000024a00be044", offset: 5, bits: 18943, labels: 41, stations: &[0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 750.0, 800.0, 850.0, 900.0, 950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0, 1250.0, 1300.0, 1350.0, 1400.0, 1450.0, 1500.0, 1550.0, 1600.0, 1650.0, 1700.0, 1750.0, 1800.0, 1841.760335016616, 1850.0, 1900.0], anchor: (1, 0.0), run_bits: 255, settings_key: 0xf00b40d },
    StationCapture { file: "Profile-3A.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-3A.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-3B.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-3B.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5A.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5A.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5B.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5B.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5B.dwg", handle: "0x1e53", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240400000121fd702c000000000180a480a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5B.dwg", handle: "0x1e56", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5B.dwg", handle: "0x1e59", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082400000001c004a81a0000000000000000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5B.dwg", handle: "0x1e5c", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000000000014012a81aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5B.dwg", handle: "0x1e5f", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824c1b23c9050f6d47435503a90cc6cd474a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5D.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5D.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5D.dwg", handle: "0x1e53", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240400000121fd702c000000000180a480a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5D.dwg", handle: "0x1e56", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082400000000000000000000000000000000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5D.dwg", handle: "0x1e59", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082400000001c004a81a0000000000000000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5D.dwg", handle: "0x1e5c", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000000000014012a81aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-5D.dwg", handle: "0x1e5f", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824c1b23c9050f6d47435503a90cc6cd474a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6A.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6A.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6A.dwg", handle: "0x1f87", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082404000001613cf068000000000180a480a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6A.dwg", handle: "0x1f8a", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108255cccaa85d5ed18895a66aa84978d1a88a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6A.dwg", handle: "0x1f8d", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824840f8287a0d5767683a98087e35f7676a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6A.dwg", handle: "0x1f90", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108242c855887e35f76762c1f588662757876a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6B.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6B.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6B.dwg", handle: "0x1f87", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082404000001613cf068000000000180a480a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6B.dwg", handle: "0x1f8a", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108255cccaa85d5ed18895a66aa84978d1a88a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6B.dwg", handle: "0x1f8d", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824840f8287a0d5767683a98087e35f7676a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6B.dwg", handle: "0x1f90", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108242c855887e35f76762c1f588662757876a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6C.dwg", handle: "0x1df0", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824e600a800de00de00d800a800d200e000a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6C.dwg", handle: "0x1df3", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108240000000000000000f8baa2b8514a605aa82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6C.dwg", handle: "0x1f87", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f81082404000001613cf068000000000180a480a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6C.dwg", handle: "0x1f8a", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108255cccaa85d5ed18895a66aa84978d1a88a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6C.dwg", handle: "0x1f8d", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f810824840f8287a0d5767683a98087e35f7676a82a000005f021fffffe009064101aff001a0007758000a1a340000404d0000207b580005010040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005021a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005020040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a0007758000a07b580005041a340000453d000040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000454d0000407b580005040040120000041468d0000107b58000421d600010775800081aff001aa4101aff001a000775800091a340000405d0000207b580005050040120000041468d0000107b58000421d600010775800081aff001aa4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 3669, labels: 6, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
    StationCapture { file: "Profile-6C.dwg", handle: "0x1f90", dxf: "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP", raw: "eaa12a0a84022f8108242c855887e35f76762c1f588662757876a82a000005f021fffffe00a4040fb4000f51501000000000000300fe8555555555555b53f155555555555714ffd2a0a8000017c0886802020000010080000060200000200800000a0200000300800000e02000004008000012020000050080000160200000600800001a0200002f8112", offset: 5, bits: 1093, labels: 0, stations: &[], anchor: (1, 0.125), run_bits: 513, settings_key: 0xf00b40f },
];


    /// Every station-type record the corpus carries (the #724 sweep's 60
    /// refusals) reads by the shared label grammar: header, keyed labels
    /// with their overrides, settings, the class's own run carried
    /// verbatim, and the `B, BS 0x04BE` close; the station and minor
    /// station groups' labels carry their ascending stations.
    #[test]
    fn station_type_records_decode_by_the_shared_grammar() {
        assert_eq!(STATION_CAPTURES.len(), 60);
        for cap in STATION_CAPTURES {
            let data = hex_bytes(cap.raw);
            let (start, end) = (cap.offset, cap.offset + cap.bits);
            let r = super::record(&data, start, end)
                .unwrap_or_else(|| panic!("{} {} {}: does not read", cap.file, cap.handle, cap.dxf));
            assert_eq!(r.labels.len(), cap.labels, "{} {}", cap.file, cap.handle);
            // The settings: the pinned reference key, the (69, 1) words,
            // the dimension anchor, then the class run (carried, not
            // mapped) and the close.
            let anchor = dimension_anchor(&data, start, end)
                .unwrap_or_else(|| panic!("{} {}: no settings", cap.file, cap.handle));
            assert_eq!((anchor.option, anchor.value), cap.anchor, "{} {}", cap.file, cap.handle);
            let mut bits = Bits::new(&data, r.settings, end);
            let keys = bits.bit_long().expect("key count");
            assert_eq!(keys, 1, "{} {}", cap.file, cap.handle);
            assert_eq!(bits.bit_long().map(|k| k as u32), Some(cap.settings_key), "{} {}", cap.file, cap.handle);
            // The settings keys take their values from the handle stream
            // (type 0x0F — no payload bits).
            bits.bit_long().expect("word");
            assert_eq!(u32::try_from(bits.bit_long().expect("option")).ok(), Some(anchor.option));
            bits.bit_double().expect("value");
            bits.bit_double().expect("weeding");
            assert_eq!(bits.bit_long(), Some(0), "{} {}: stagger", cap.file, cap.handle);
            bits.bit_double().expect("stagger height a");
            bits.bit_double().expect("stagger height b");
            let run_bits = bits.remaining() - 19;
            assert_eq!(run_bits, cap.run_bits, "{} {}: class run", cap.file, cap.handle);
            bits.skip(run_bits).expect("run");
            assert_eq!(super::close(&mut bits), Some(()), "{} {}: close", cap.file, cap.handle);
            // The labels' stations: pinned for every record (empty for the
            // geometry point group, whose labels carry no station key).
            assert_eq!(
                stations(&data, start, end).as_deref(),
                Some(cap.stations),
                "{} {} {}",
                cap.file,
                cap.handle,
                cap.dxf
            );
            // The stations ascend when the group carries any.
            let mut sorted = cap.stations.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(sorted, cap.stations, "{} {}", cap.file, cap.handle);
        }
    }

    /// The geometry point group's labels name geometry points, not
    /// stations: their point-index and property-id keys read as keyed
    /// values and the entity ids (the profile element keys the curve
    /// groups carry) stay empty.
    #[test]
    fn geometry_point_records_carry_point_keys_not_entity_keys() {
        for cap in STATION_CAPTURES.iter().filter(|c| c.dxf == "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP") {
            let data = hex_bytes(cap.raw);
            let (start, end) = (cap.offset, cap.offset + cap.bits);
            let r = super::record(&data, start, end).expect("reads");
            // A group with every label at its default carries no
            // per-label records (9 of the 43).
            for label in &r.labels {
                let ids: Vec<u32> = label.keys.iter().map(|(k, _, _)| *k).collect();
                assert!(!ids.iter().any(|k| PROFILE_ENTITY_KEYS.contains(k)), "{} {}", cap.file, cap.handle);
            }
            assert_eq!(labelled_entity_ids(&data, start, end), Some(Vec::new()));
        }
    }
}
