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


// ---- The lossless label-record model ----
//
// The accessors above read the grammar's fields they name and skip the
// rest; the model here keeps every stored field so a record decodes into
// [`LabelGroupRecord`] and encodes back bit for bit. Everything is
// structural: a field the grammar does not map (an unknown property type,
// an unknown class tag, text that is not UTF-16, a color key whose index
// word is not the codec's 0) refuses the record — `None`, never a guess
// (the #518 ground).

/// The stored value of a label override's property, by the property's
/// 7-bit type. Each variant maps back to the one type it was read from.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LabelValue {
    /// Type 2: a BD.
    Double(f64),
    /// Type 3: one byte.
    Byte(u8),
    /// Type 8: three BD (a point).
    Point([f64; 3]),
    /// Type 9: three BD (a dragged offset).
    Offset([f64; 3]),
    /// Type 13: a BL.
    Long(u32),
    /// Type 24: a BL byte count and UTF-16 text.
    Text(String),
    /// Type 16: the property names a reference (in the handle stream); no
    /// stored value.
    Ref,
    /// Type 25: a BL (the long form of type 13).
    Long25(u32),
    /// Type 26: one byte (the long form of type 3).
    Byte26(u8),
}

impl LabelValue {
    /// The property's 7-bit type code.
    fn type_bits(&self) -> u32 {
        match self {
            Self::Double(_) => 2,
            Self::Byte(_) => 3,
            Self::Point(_) => 8,
            Self::Offset(_) => 9,
            Self::Long(_) => 13,
            Self::Text(_) => 24,
            Self::Ref => 16,
            Self::Long25(_) => 25,
            Self::Byte26(_) => 26,
        }
    }

    fn write(&self, w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter) {
        match self {
            Self::Double(v) => w.write_bit_double(*v),
            Self::Byte(v) | Self::Byte26(v) => w.write_byte(*v),
            Self::Point(v) | Self::Offset(v) => {
                for c in v {
                    w.write_bit_double(*c);
                }
            }
            Self::Long(v) | Self::Long25(v) => w.write_bit_long(*v as i32),
            Self::Text(text) => {
                let units: Vec<u16> = text.encode_utf16().collect();
                w.write_bit_long((units.len() * 2) as i32);
                for u in units {
                    w.write_byte(u as u8);
                    w.write_byte((u >> 8) as u8);
                }
            }
            Self::Ref => {}
        }
    }
}

/// One property of a label override block: the property's `id` in its
/// property `set` (bytes C0 0C for the label's own overrides), and the
/// typed value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelProperty {
    pub id: u8,
    pub set: u16,
    pub value: LabelValue,
}

/// One block of a label override: the block's two words, its 2 bits, the
/// RC byte (8 x the property count in all but one sample — carried, not
/// recomputed), and the block's properties in stored order (the last one
/// is marked by its trailing 3 bits `010`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelOverrideBlock {
    pub words: (u32, u32),
    pub bits: u8,
    pub rc: u8,
    pub properties: Vec<LabelProperty>,
}

/// One override of a label: `BL b, b x block`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelOverride {
    pub blocks: Vec<LabelOverrideBlock>,
}

/// The fields a label shares with a nested label: its keys with their
/// values, in stored order, the node head (2 bits, the RC byte, the
/// label flag), the key ids it lists, and its nested labels.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelNode {
    pub keys: Vec<(u32, KeyValue)>,
    /// The 2 bits, the RC byte and the flag long of the node head.
    pub head: (u8, u8, u32),
    pub ids: Vec<u32>,
    pub nested: Vec<LabelNode>,
}

/// One label of a label record: its [`LabelNode`] and, for a top-level
/// label, the overrides (one per dragged or flipped state; a nested label
/// carries none — the grammar ends it at its nested labels).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelGroupLabel {
    pub node: LabelNode,
    pub overrides: Vec<LabelOverride>,
}

/// The settings a label record holds after its labels (see
/// [`settings`]): the keys that take references, two words (the second is
/// the default dimension anchor option), four doubles (the anchor value,
/// the weeding, and the two stagger line heights) and the label stagger.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelGroupSettings {
    pub keys: Vec<(u32, KeyValue)>,
    pub words: (u32, u32),
    pub doubles: [f64; 4],
    pub stagger: u32,
}

/// A whole label record, lossless (see the module docs for the layout).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelGroupRecord {
    /// The shared class header's 6-bit class tag (`0b010010` in every
    /// corpus record so far).
    pub tag: u8,
    /// The 3 extra bits of the `0b010110` tag form, `None` for the plain
    /// `0b010010` form.
    pub tag_bits: Option<u8>,
    /// The 16 bytes the record carries after its class tag (the corpus's
    /// VALIGNMENT curve records carry UTF-16 string fragments and heap
    /// addresses — residual bytes, like the alignment groups').
    pub guid: [u8; 16],
    /// The long between the `0x04BE0000` word and the trailing 3 bits
    /// (`0x00FFFFFF` in the writers' form).
    pub word: u32,
    /// The 3 trailing bits of the header (`0b010` in the writers' form).
    pub bits: u8,
    /// The shared class header's own long, between the fixed
    /// `80, 0, 80, 80, 0` words and the class tag (`0x10025F04` in the
    /// writers' form).
    pub class_word: u32,
    /// The labels, in stored order.
    pub labels: Vec<LabelGroupLabel>,
    pub settings: LabelGroupSettings,
    /// The class's own run: the bits between the settings' end and the
    /// closing bit — a class-specific section this crate does not map
    /// (the VALIGNMENT curve groups hold their cached per-curve label
    /// data here). Carried verbatim, byte-aligned from the section's
    /// start, so a record rebuilt from a decoded one writes it back
    /// exactly (the #688 guid precedent); `bits` may be fewer than
    /// `bytes.len() * 8` — the last byte holds the run's final bits.
    pub residual: Vec<u8>,
    pub residual_bits: usize,
    /// The record's closing bit (false in every record so far), before
    /// the closing `BS 0x04BE`.
    pub close: bool,
}

impl LabelGroupRecord {
    /// The record's own bits, re-encoded (the writer pads the stream to
    /// whole bytes; compare bit for bit against the source's class body).
    pub fn encoded(&self) -> Vec<u8> {
        let mut w = crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter::new(
            crate::io::dwg::DwgVersion::AC24,
            crate::types::DxfVersion::AC1032,
        );
        write_record(&mut w, self);
        w.main().to_bytes_snapshot()
    }
}

/// One class key read exactly: [`super::alignment::read_key`] with the
/// text value required to be UTF-16 and the color key's index word
/// required to be 0 (the codec writes 0; the value word carries the
/// color).
fn key_exact(bits: &mut Bits) -> Option<(u32, KeyValue)> {
    let key = bits.bit_long()? as u32;
    let count = |bits: &mut Bits| usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2);
    let value = match key >> 24 {
        0x01 | 0x0A | 0x0D => KeyValue::Long(bits.bit_long()?),
        0x02 => KeyValue::Double(bits.bit_double()?),
        0x03 | 0x1A => KeyValue::Byte(bits.bits_value(8)?),
        0x04 | 0x0F | 0x10 | 0x12 => KeyValue::None,
        0x06 => KeyValue::Point([bits.raw_double()?, bits.raw_double()?]),
        0x09 => KeyValue::Vector([bits.bit_double()?, bits.bit_double()?, bits.bit_double()?]),
        0x0B | 0x19 => {
            let n = count(bits)?;
            KeyValue::Longs((0..n).map(|_| bits.bit_long()).collect::<Option<_>>()?)
        }
        0x0C => {
            let n = count(bits)?;
            KeyValue::Doubles((0..n).map(|_| bits.bit_double()).collect::<Option<_>>()?)
        }
        0x0E => {
            (bits.bit_short()? == 0).then_some(())?;
            let color = bits.bit_long()? as u32;
            bits.bits_value(8)?;
            KeyValue::Color(color)
        }
        0x13 | 0x16 => {
            let low = bits.raw_long()? as u32;
            let high = bits.raw_long()? as u32;
            KeyValue::Raw(u64::from(high) << 32 | u64::from(low))
        }
        0x15 => {
            let mut value = 0u128;
            for i in 0..16 {
                value |= u128::from(bits.bits_value(8)?) << (8 * i);
            }
            KeyValue::Raw128(value)
        }
        0x18 => {
            let bytes = usize::try_from(bits.bit_long()?).ok().filter(|n| n % 2 == 0 && n * 8 <= bits.remaining())?;
            let units = (0..bytes / 2)
                .map(|_| {
                    let lo = bits.bits_value(8)?;
                    let hi = bits.bits_value(8)?;
                    String::from_utf16(&[u16::from_le_bytes([lo, hi])]).ok()
                })
                .collect::<Option<Vec<_>>>()?;
            KeyValue::Text(units.concat())
        }
        _ => return None,
    };
    Some((key, value))
}

/// The record header, lossless (see the module docs): the shared class
/// header, the 16 residual bytes, the `80, 80, 0x04BE0000` words, the
/// long between them and the trailing 3 bits.
fn header_full(bits: &mut Bits) -> Option<(u8, Option<u8>, [u8; 16], u32, u8, u32)> {
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    let class_word = bits.bit_long()? as u32;
    let (tag, tag_bits) = match bits.bits_value(6)? {
        0b01_0010 => (0b01_0010, None),
        0b01_0110 => (0b01_0110, Some(bits.bits_value(3)? as u8)),
        _ => return None,
    };
    let mut guid = [0u8; 16];
    for byte in &mut guid {
        *byte = bits.bits_value(8)?;
    }
    for want in [80, 80, 0x04BE_0000] {
        (bits.bit_long()? == want).then_some(())?;
    }
    let word = bits.bit_long()? as u32;
    let bits3 = bits.bits_value(3)? as u8;
    Some((tag, tag_bits, guid, word, bits3, class_word))
}

/// A label node, lossless (see [`LabelNode`]).
fn node_full(bits: &mut Bits, depth: usize) -> Option<LabelNode> {
    (depth < NESTING).then_some(())?;
    let mut keys = Vec::new();
    for _ in 0..bits.bit_long()? {
        keys.push(key_exact(bits)?);
    }
    let b2 = bits.bits_value(2)? as u8;
    let rc = bits.raw_short_byte()?;
    let flag = bits.bit_long()? as u32;
    let ids = (0..bits.bit_long()?).map(|_| bits.bit_long().map(|v| v as u32)).collect::<Option<Vec<_>>>()?;
    let nested = (0..bits.bit_long()?).map(|_| node_full(bits, depth + 1)).collect::<Option<Vec<_>>>()?;
    Some(LabelNode { keys, head: (b2, rc, flag), ids, nested })
}

/// One override block's properties, lossless (see [`LabelOverrideBlock`]).
fn property_full(bits: &mut Bits) -> Option<(LabelProperty, bool)> {
    let id = bits.raw_short_byte()?;
    let set = u16::from_le_bytes([bits.raw_short_byte()?, bits.raw_short_byte()?]);
    let value = match bits.bits_value(7)? {
        2 => LabelValue::Double(bits.bit_double()?),
        3 => LabelValue::Byte(bits.raw_short_byte()?),
        26 => LabelValue::Byte26(bits.raw_short_byte()?),
        8 => LabelValue::Point([bits.bit_double()?, bits.bit_double()?, bits.bit_double()?]),
        9 => LabelValue::Offset([bits.bit_double()?, bits.bit_double()?, bits.bit_double()?]),
        13 => LabelValue::Long(bits.bit_long()? as u32),
        25 => LabelValue::Long25(bits.bit_long()? as u32),
        16 => LabelValue::Ref,
        24 => {
            let bytes = usize::try_from(bits.bit_long()?).ok().filter(|n| n % 2 == 0 && n * 8 <= bits.remaining())?;
            let units = (0..bytes / 2)
                .map(|_| {
                    let lo = bits.bits_value(8)?;
                    let hi = bits.bits_value(8)?;
                    String::from_utf16(&[u16::from_le_bytes([lo, hi])]).ok()
                })
                .collect::<Option<Vec<_>>>()?;
            LabelValue::Text(units.concat())
        }
        _ => return None,
    };
    let last = match bits.bits_value(3)? {
        0b010 => true,
        0b000 => false,
        _ => return None,
    };
    Some((LabelProperty { id, set, value }, last))
}

/// A label with its overrides, lossless (see [`LabelGroupLabel`]).
fn label_full(bits: &mut Bits) -> Option<LabelGroupLabel> {
    let node = node_full(bits, 0)?;
    let mut overrides = Vec::new();
    for _ in 0..bits.bit_long()? {
        let mut blocks = Vec::new();
        for _ in 0..bits.bit_long()? {
            let words = (bits.bit_long()? as u32, bits.bit_long()? as u32);
            let b2 = bits.bits_value(2)? as u8;
            let rc = bits.raw_short_byte()?;
            (rc != 0).then_some(())?;
            let mut properties = Vec::new();
            loop {
                let (property, last) = property_full(bits)?;
                properties.push(property);
                if last {
                    break;
                }
            }
            blocks.push(LabelOverrideBlock { words, bits: b2, rc, properties });
        }
        overrides.push(LabelOverride { blocks });
    }
    Some(LabelGroupLabel { node, overrides })
}

/// The settings, lossless (see [`LabelGroupSettings`]).
fn settings_full(bits: &mut Bits) -> Option<LabelGroupSettings> {
    let mut keys = Vec::new();
    for _ in 0..bits.bit_long()? {
        keys.push(key_exact(bits)?);
    }
    let words = (bits.bit_long()? as u32, bits.bit_long()? as u32);
    let d0 = bits.bit_double()?;
    let d1 = bits.bit_double()?;
    let stagger = bits.bit_long()? as u32;
    let d2 = bits.bit_double()?;
    let d3 = bits.bit_double()?;
    Some(LabelGroupSettings { keys, words, doubles: [d0, d1, d2, d3], stagger })
}

/// A whole label record, lossless; `None` for any form the grammar does
/// not map (an unknown class tag or property type, non-UTF-16 text, a
/// color key with an index word, an over-deep nesting, trailing bits).
pub(super) fn record_full(data: &[u8], start: usize, end: usize) -> Option<LabelGroupRecord> {
    let mut bits = Bits::new(data, start, end);
    let (tag, tag_bits, guid, word, bits3, class_word) = header_full(&mut bits)?;
    let labels = (0..bits.bit_long()?).map(|_| label_full(&mut bits)).collect::<Option<Vec<_>>>()?;
    let settings = settings_full(&mut bits)?;
    (end >= bits.pos + 19).then_some(())?; // the close (B + BS 0x04BE) must fit
    let residual_bits = end - bits.pos - 19;
    let mut residual = Vec::new();
    for _ in 0..residual_bits / 8 {
        residual.push(bits.bits_value(8)?);
    }
    let rem = residual_bits % 8;
    if rem > 0 {
        residual.push(bits.bits_value(rem)? << (8 - rem));
    }
    let close = bits.bit()?;
    (bits.bit_short()? == 0x04BE && bits.remaining() == 0).then_some(())?;
    Some(LabelGroupRecord { tag, tag_bits, guid, word, bits: bits3, class_word, labels, settings, residual, residual_bits, close })
}

// ---- The encoder side of the shared label grammar ----

/// Writes the constant bit run `bits` ("0"/"1" characters).
fn bit_run(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, bits: &str) {
    for b in bits.bytes() {
        w.write_bit(b == b'1');
    }
}

/// Writes one class key and its value (see [`key_exact`]).
pub(super) fn write_key(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, key: u32, value: &KeyValue) {
    w.write_bit_long(key as i32);
    match value {
        KeyValue::None => {}
        KeyValue::Long(v) => w.write_bit_long(*v),
        KeyValue::Double(v) => w.write_bit_double(*v),
        KeyValue::Byte(v) => w.write_byte(*v),
        KeyValue::Point(p) => {
            w.write_raw_double(p[0]);
            w.write_raw_double(p[1]);
        }
        KeyValue::Vector(v) => {
            for c in v {
                w.write_bit_double(*c);
            }
        }
        KeyValue::Longs(list) => {
            w.write_bit_long(list.len() as i32);
            for v in list {
                w.write_bit_long(*v);
            }
        }
        KeyValue::Doubles(list) => {
            w.write_bit_long(list.len() as i32);
            for v in list {
                w.write_bit_double(*v);
            }
        }
        KeyValue::Color(color) => {
            w.write_bit_short(0);
            w.write_bit_long(*color as i32);
            bit_run(w, "00000000");
        }
        KeyValue::Raw(raw) => {
            w.write_raw_long(*raw as i32);
            w.write_raw_long((*raw >> 32) as i32);
        }
        KeyValue::Raw128(raw) => {
            for i in 0..16 {
                w.write_byte((raw >> (8 * i)) as u8);
            }
        }
        KeyValue::Text(text) => {
            let units: Vec<u16> = text.encode_utf16().collect();
            w.write_bit_long((units.len() * 2) as i32);
            for u in units {
                w.write_byte(u as u8);
                w.write_byte((u >> 8) as u8);
            }
        }
    }
}

fn write_node(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, node: &LabelNode) {
    w.write_bit_long(node.keys.len() as i32);
    for (k, v) in &node.keys {
        write_key(w, *k, v);
    }
    let (b2, rc, flag) = node.head;
    bit_run(w, &format!("{b2:02b}"));
    w.write_byte(rc);
    w.write_bit_long(flag as i32);
    w.write_bit_long(node.ids.len() as i32);
    for id in &node.ids {
        w.write_bit_long(*id as i32);
    }
    w.write_bit_long(node.nested.len() as i32);
    for nested in &node.nested {
        write_node(w, nested);
    }
}

fn write_label(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, label: &LabelGroupLabel) {
    write_node(w, &label.node);
    w.write_bit_long(label.overrides.len() as i32);
    for override_ in &label.overrides {
        w.write_bit_long(override_.blocks.len() as i32);
        for block in &override_.blocks {
            w.write_bit_long(block.words.0 as i32);
            w.write_bit_long(block.words.1 as i32);
            bit_run(w, &format!("{:02b}", block.bits));
            w.write_byte(block.rc);
            for (i, p) in block.properties.iter().enumerate() {
                w.write_byte(p.id);
                let set = p.set.to_le_bytes();
                w.write_byte(set[0]);
                w.write_byte(set[1]);
                bit_run(w, &format!("{:07b}", p.value.type_bits()));
                p.value.write(w);
                bit_run(w, if i + 1 == block.properties.len() { "010" } else { "000" });
            }
        }
    }
}

/// Writes a whole label record (see [`record_full`]); the stream holds
/// exactly the record's bits when it closes (the writer pads to whole
/// bytes outside this call).
pub(super) fn write_record(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, record: &LabelGroupRecord) {
    for v in [80u32, 0, 80, 80, 0] {
        w.write_bit_long(v as i32);
    }
    w.write_bit_long(record.class_word as i32);
    bit_run(w, &format!("{:06b}", record.tag));
    if let Some(extra) = record.tag_bits {
        bit_run(w, &format!("{:03b}", extra));
    }
    w.write_bytes(&record.guid);
    for v in [80u32, 80, 0x04BE_0000] {
        w.write_bit_long(v as i32);
    }
    w.write_bit_long(record.word as i32);
    bit_run(w, &format!("{:03b}", record.bits));
    w.write_bit_long(record.labels.len() as i32);
    for label in &record.labels {
        write_label(w, label);
    }
    let settings = &record.settings;
    w.write_bit_long(settings.keys.len() as i32);
    for (k, v) in &settings.keys {
        write_key(w, *k, v);
    }
    w.write_bit_long(settings.words.0 as i32);
    w.write_bit_long(settings.words.1 as i32);
    w.write_bit_double(settings.doubles[0]);
    w.write_bit_double(settings.doubles[1]);
    w.write_bit_long(settings.stagger as i32);
    w.write_bit_double(settings.doubles[2]);
    w.write_bit_double(settings.doubles[3]);
    // The class's own run, verbatim (whole bytes, then the run's final
    // partial byte bit by bit — the byte may hold padding bits beyond
    // `residual_bits`).
    let whole = record.residual_bits / 8;
    let rem = record.residual_bits % 8;
    for byte in &record.residual[..whole] {
        w.write_byte(*byte);
    }
    if rem > 0 {
        let last = record.residual[whole];
        for i in 0..rem {
            w.write_bit((last >> (7 - i)) & 1 == 1);
        }
    }
    w.write_bit(record.close);
    w.write_bit_short(0x04BE);
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

    /// The bytes of a hex literal.
    fn hex_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex"))
            .collect()
    }

    /// The oracle record's lossless decode re-encodes bit for bit (the
    /// encoder side of the shared label grammar, pinned on the capture).
    #[test]
    fn oracle_curve_label_groups_round_trip_bit_exact() {
        for cap in CURVE_CAPTURES {
            let data = hex_bytes(cap.raw);
            let record = record_full(&data, cap.start, cap.end).expect("the capture decodes");
            let mut w = crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter::new(
                crate::io::dwg::DwgVersion::AC24,
                crate::types::DxfVersion::AC1032,
            );
            write_record(&mut w, &record);
            let out = w.main().to_bytes_snapshot();
            let nbits = cap.end - cap.start;
            assert!(out.len() * 8 >= nbits, "re-encode too short");
            for i in 0..nbits {
                let got = (out[i / 8] >> (7 - (i % 8))) & 1;
                let want = (data[(cap.start + i) / 8] >> (7 - ((cap.start + i) % 8))) & 1;
                assert_eq!(got, want, "bit {i} of the {} run for profile {:X}", cap.kind, cap.profile);
            }
            // The decoded fields: the group form (no labels; the settings
            // carry the label set reference and the recorded words and
            // doubles), and the class run's two recorded lengths.
            assert!(record.labels.is_empty(), "the group form carries no labels");
            assert_eq!(record.settings.words, (69, 1));
            assert_eq!(record.settings.doubles, [0.125, 0.0, 0.083_333_333_333_333_33, 0.166_666_666_666_666_66]);
            assert_eq!(record.settings.stagger, 0);
            assert_eq!(record.settings.keys.len(), 1);
            assert_eq!(record.settings.keys[0].0, 0x0F00_B413, "the profile label set key");
            assert_eq!(record.tag, 0b01_0010);
            assert_eq!(record.class_word, 0x1002_5F04);
            let run_bits = record.residual_bits;
            let expected = if nbits == 769 { 189 } else { 125 };
            assert_eq!(run_bits, expected, "the class run length of the {} record", cap.kind);
        }
    }

    /// The record grammar refuses a truncated class run (the close must
    /// fit) — the decoder's graceful None, never a guess.
    #[test]
    fn a_record_without_room_for_its_close_is_refused() {
        let cap = &CURVE_CAPTURES[0];
        let mut data = hex_bytes(cap.raw);
        let end = cap.end - 10;
        // Trim the stream so the close cannot fit: the reader must refuse.
        let trimmed = cap.end - 10;
        while data.len() * 8 > trimmed {
            data.pop();
        }
        assert_eq!(record_full(&data, cap.start, end), None);
    }

    /// One oracle record's capture (Sections-Project-Objects.dwg): the
    /// entity's DWG body bytes (hex), the class body's bit range, and the
    /// group's references (profile, label style, view) as recorded.
    struct CurveCapture {
        kind: &'static str,
        profile: u64,
        style: u64,
        view: u64,
        start: usize,
        end: usize,
        raw: &'static str,
    }

    /// The drawing's 8 standalone VALIGNMENT curve label groups
    /// (AeccDbVAlignmentSagCurveLabeling x4, CrestCurveLabeling x4), as
    /// captured from the drawing.
    const CURVE_CAPTURES: &[CurveCapture] = &[
        CurveCapture { kind: "sag", profile: 0x639F, style: 0x49BE, view: 0x63A1, start: 2885, end: 3654, raw: concat!(
            "80010098ed6a5c015c01000003000000c8000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074005300610067004300",
            "75007200760065004c006100620065006c0069006e0067002000280041006500",
            "630063004c0061006e0064003100330030002900000000008400000006000000",
            "0500000000000000000000000000000000000000000000000000000000000000",
            "0000f03f00000000000000000000000000000000000000000000f03f9a999999",
            "9999d93f000000000000000000000000000000009a9999999999d93f00000000",
            "0000000000000000000000000000000000000000000000000000000008000000",
            "15000000ab400010eaa12a0a84022f81082468006c005c00660070004e005800",
            "7000a82a000005f021fffffe00a40413b4000f51501000000000000300fe8555",
            "555555555b53f155555555555714ff80000000000a0867a6866e24a898dcda02",
            "a0a8000017c082f81121410065006300630049006d007000560041006c006900",
            "67006e006d0065006e007400530061006700430075007200760065004c006100",
            "620065006c0069006e006700aa2202a88820202131cfa924df2131d0ff",
        ) },
        CurveCapture { kind: "crest", profile: 0x639F, style: 0x49BE, view: 0x63A1, start: 2917, end: 3686, raw: concat!(
            "80410098edaa60016001000003000000cc000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074004300720065007300",
            "7400430075007200760065004c006100620065006c0069006e00670020002800",
            "41006500630063004c0061006e00640031003300300029000000000084000000",
            "0600000005000000000000000000000000000000000000000000000000000000",
            "000000000000f03f00000000000000000000000000000000000000000000f03f",
            "9a9999999999d93f000000000000000000000000000000009a9999999999d93f",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0800000015000000ab400010eaa12a0a84022f8108246e006e005c006c006c00",
            "4e0058007000a82a000005f021fffffe00a40413b4000f515010000000000003",
            "00fe8555555555555b53f155555555555714ff80000000000a0867a6866e24a8",
            "98dcda02a0a8000017c082f81123410065006300630049006d00700056004100",
            "6c00690067006e006d0065006e00740043007200650073007400430075007200",
            "760065004c006100620065006c0069006e006700aa4202a88820202131cfa924",
            "df2131d0ff",
        ) },
        CurveCapture { kind: "sag", profile: 0x63A0, style: 0x49BE, view: 0x63A1, start: 2885, end: 3590, raw: concat!(
            "80010098edea5c015c01000003000000c8000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074005300610067004300",
            "75007200760065004c006100620065006c0069006e0067002000280041006500",
            "630063004c0061006e0064003100330030002900000000008400000006000000",
            "0500000000000000000000000000000000000000000000000000000000000000",
            "0000f03f00000000000000000000000000000000000000000000f03f9a999999",
            "9999d93f000000000000000000000000000000009a9999999999d93f00000000",
            "0000000000000000000000000000000000000000000000000000000008000000",
            "15000000ab400010eaa12a0a84022f8108240000000000000000000000000000",
            "0000a82a000005f021fffffe00a40413b4000f51501000000000000300fe8555",
            "555555555b53f155555555555714ffc04559ade98824da02a0a8000017c082f8",
            "1121410065006300630049006d007000560041006c00690067006e006d006500",
            "6e007400530061006700430075007200760065004c006100620065006c006900",
            "6e006700aa2202a88820202131d02924df2131d0ff",
        ) },
        CurveCapture { kind: "crest", profile: 0x63A0, style: 0x49BE, view: 0x63A1, start: 2917, end: 3622, raw: concat!(
            "80410098ee2a60016001000003000000cc000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074004300720065007300",
            "7400430075007200760065004c006100620065006c0069006e00670020002800",
            "41006500630063004c0061006e00640031003300300029000000000084000000",
            "0600000005000000000000000000000000000000000000000000000000000000",
            "000000000000f03f00000000000000000000000000000000000000000000f03f",
            "9a9999999999d93f000000000000000000000000000000009a9999999999d93f",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0800000015000000ab400010eaa12a0a84022f810824000000000000e8000000",
            "000000000000a82a000005f021fffffe00a40413b4000f515010000000000003",
            "00fe8555555555555b53f155555555555714ffc04559ade98824da02a0a80000",
            "17c082f81123410065006300630049006d007000560041006c00690067006e00",
            "6d0065006e00740043007200650073007400430075007200760065004c006100",
            "620065006c0069006e006700aa4202a88820202131d02924df2131d0ff",
        ) },
        CurveCapture { kind: "sag", profile: 0x63A7, style: 0x49BE, view: 0x63A9, start: 2885, end: 3590, raw: concat!(
            "80010098ee6a5c015c01000003000000c8000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074005300610067004300",
            "75007200760065004c006100620065006c0069006e0067002000280041006500",
            "630063004c0061006e0064003100330030002900000000008400000006000000",
            "0500000000000000000000000000000000000000000000000000000000000000",
            "0000f03f00000000000000000000000000000000000000000000f03f9a999999",
            "9999d93f000000000000000000000000000000009a9999999999d93f00000000",
            "0000000000000000000000000000000000000000000000000000000008000000",
            "15000000ab400010eaa12a0a84022f81082440008800c200e800c200b8008200",
            "ea00a82a000005f021fffffe00a40413b4000f51501000000000000300fe8555",
            "555555555b53f155555555555714ffc3d94815ef6764da02a0a8000017c082f8",
            "1121410065006300630049006d007000560041006c00690067006e006d006500",
            "6e007400530061006700430075007200760065004c006100620065006c006900",
            "6e006700aa2202a88820202131d3a924df2131d4ff",
        ) },
        CurveCapture { kind: "crest", profile: 0x63A7, style: 0x49BE, view: 0x63A9, start: 2917, end: 3622, raw: concat!(
            "80410098eeaa60016001000003000000cc000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074004300720065007300",
            "7400430075007200760065004c006100620065006c0069006e00670020002800",
            "41006500630063004c0061006e00640031003300300029000000000084000000",
            "0600000005000000000000000000000000000000000000000000000000000000",
            "000000000000f03f00000000000000000000000000000000000000000000f03f",
            "9a9999999999d93f000000000000000000000000000000009a9999999999d93f",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0800000015000000ab400010eaa12a0a84022f810825f800018606000187f800",
            "01860e000186a82a000005f021fffffe00a40413b4000f515010000000000003",
            "00fe8555555555555b53f155555555555714ffc3d94815ef6764da02a0a80000",
            "17c082f81123410065006300630049006d007000560041006c00690067006e00",
            "6d0065006e00740043007200650073007400430075007200760065004c006100",
            "620065006c0069006e006700aa4202a88820202131d3a924df2131d4ff",
        ) },
        CurveCapture { kind: "sag", profile: 0x63A8, style: 0x49BE, view: 0x63A9, start: 2885, end: 3654, raw: concat!(
            "80010098eeea5c015c01000003000000c8000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074005300610067004300",
            "75007200760065004c006100620065006c0069006e0067002000280041006500",
            "630063004c0061006e0064003100330030002900000000008400000006000000",
            "0500000000000000000000000000000000000000000000000000000000000000",
            "0000f03f00000000000000000000000000000000000000000000f03f9a999999",
            "9999d93f000000000000000000000000000000009a9999999999d93f00000000",
            "0000000000000000000000000000000000000000000000000000000008000000",
            "15000000ab400010eaa12a0a84022f8108242e00000030000000320000003400",
            "0000a82a000005f021fffffe00a40413b4000f51501000000000000300fe8555",
            "555555555b53f155555555555714ff8000000000110aa7a527d00def6764da02",
            "a0a8000017c082f81121410065006300630049006d007000560041006c006900",
            "67006e006d0065006e007400530061006700430075007200760065004c006100",
            "620065006c0069006e006700aa2202a88820202131d42924df2131d4ff",
        ) },
        CurveCapture { kind: "crest", profile: 0x63A8, style: 0x49BE, view: 0x63A9, start: 2917, end: 3686, raw: concat!(
            "80410098ef2a60016001000003000000cc000000240000009a9999999999b93f",
            "9a9999999999b93f000000000000000000000000000000000000000000000000",
            "000000000000f03f000000000000f03f00000000000000000000000000000000",
            "9a9999999999c93f000000000000f03f00000000000000004100650063006300",
            "44006200560041006c00690067006e006d0065006e0074004300720065007300",
            "7400430075007200760065004c006100620065006c0069006e00670020002800",
            "41006500630063004c0061006e00640031003300300029000000000084000000",
            "0600000005000000000000000000000000000000000000000000000000000000",
            "000000000000f03f00000000000000000000000000000000000000000000f03f",
            "9a9999999999d93f000000000000000000000000000000009a9999999999d93f",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0800000015000000ab400010eaa12a0a84022f810825f800018606000187f800",
            "01860e000186a82a000005f021fffffe00a40413b4000f515010000000000003",
            "00fe8555555555555b53f155555555555714ff8000000000110aa7a527d00def",
            "6764da02a0a8000017c082f81123410065006300630049006d00700056004100",
            "6c00690067006e006d0065006e00740043007200650073007400430075007200",
            "760065004c006100620065006c0069006e006700aa4202a88820202131d42924",
            "df2131d4ff",
        ) },
    ];
}
