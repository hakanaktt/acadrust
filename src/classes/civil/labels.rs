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
//! BL 0                              nested labels (counts above zero are
//!                                   not decoded: the record reads as None)
//! BL m, m x override
//! override:  BL 1, BL, BL, 2 bits, RC 8 x p, p x property
//! property:  RC id, 16 bits (the property set), 7-bit type, value by
//!            type, 3 bits (010 after the last property, else 000)
//! values:    2 BD, 3 RC, 8 and 9 three BD (a point, an offset),
//!            13 BL, 16 none (the value is a reference), 25 BL, 26 RC
//! ```
//!
//! The label text itself is generated from the label style and is not
//! stored.

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

fn property(bits: &mut Bits) -> Option<(u16, u8, Option<[f64; 3]>)> {
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
        _ => return None,
    }
    bits.bits_value(3)?;
    Some((set, id, value))
}

fn label(bits: &mut Bits) -> Option<Label> {
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
    (bits.bit_long()? == 0).then_some(())?;
    let mut properties = Vec::new();
    for _ in 0..bits.bit_long()? {
        (bits.bit_long()? == 1).then_some(())?;
        bits.bit_long()?;
        bits.bit_long()?;
        bits.bits_value(2)?;
        let size = bits.raw_short_byte()?;
        (size % 8 == 0).then_some(())?;
        for _ in 0..size / 8 {
            properties.push(property(bits)?);
        }
    }
    Some(Label { keys, properties })
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
/// BL, BL, 5 x BD
/// ```
fn settings(bits: &mut Bits) -> Option<()> {
    for _ in 0..bits.bit_long()? {
        read_key(bits)?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    for _ in 0..5 {
        bits.bit_double()?;
    }
    Some(())
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
    let labels = labels(data, start, end)?;
    Some(labels.iter().flat_map(|l| &l.properties).find_map(|(set, id, value)| match (*set, *id, value) {
        (OVERRIDE_SET, DRAGGED_OFFSET, Some([x, y, _])) if (*x, *y) != (0.0, 0.0) => Some(crate::types::Vector2::new(*x, *y)),
        _ => None,
    }))
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
}

/// The head every label style component's class data opens with (after
/// the style's component lists): `BL, 2 bits, BD height, BL, BL color, BL
/// type tag, BL, BL, BL`. Returns the height, the type tag and where the
/// general block starts.
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

/// BD at `pos` written in full.
fn full_double(data: &[u8], pos: usize, end: usize) -> Option<f64> {
    let mut bits = Bits::new(data, pos, end);
    let value = bits.bit_double()?;
    (bits.pos == pos + 66 && value.is_finite()).then_some(value)
}

/// X offset, y offset and border gap read by their fields: the general
/// head (see [`general_head`]), `1000`, 6 BL, B, BD x, BD, BD y, BD, B,
/// BD gap; or, in components with type tag 0x0022001F, `1000`, 5 BL,
/// BD angle, BD, BD, B, BD x, BD y, B, BD gap. Also returns the bit after
/// the gap.
fn general_offsets(data: &[u8], at: usize, end: usize, turned: bool) -> Option<(f64, f64, f64, usize)> {
    let mut bits = Bits::new(data, at, end);
    general_head(&mut bits)?;
    (bits.bits_value(4)? == 0b1000).then_some(())?;
    if turned {
        for _ in 0..5 {
            bits.bit_long()?;
        }
        for _ in 0..3 {
            bits.bit_double()?;
        }
        bits.bit()?;
        let x = bits.bit_double()?;
        let y = bits.bit_double()?;
        bits.bit()?;
        let gap = bits.bit_double()?;
        return Some((x, y, gap, bits.pos));
    }
    for _ in 0..6 {
        bits.bit_long()?;
    }
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

/// A line component closes with its length and then its angle, both
/// written in full. The length starts 142 bits before the record's end;
/// a line along the label's direction has no angle and its length starts
/// 86 bits before the end.
pub(super) fn line_component(data: &[u8], start: usize, end: usize) -> Option<(f64, f64)> {
    let length_at = |back: usize| {
        end.checked_sub(back)
            .filter(|p| *p >= start)
            .and_then(|p| full_double(data, p, end))
            .filter(|v| (1e-6..1.0).contains(v))
    };
    if length_at(142).is_none() {
        if let Some(length) = length_at(86) {
            return Some((length, 0.0));
        }
    }
    let angle_at = (start..end)
        .rev()
        .find(|&p| full_double(data, p, end).is_some_and(|v| (1e-3..=std::f64::consts::TAU).contains(&v.abs())))?;
    let length_at = (start..angle_at.checked_sub(66)?).rev().find(|&p| {
        full_double(data, p, end).is_some_and(|v| (1e-6..1.0).contains(&v) && ((v * 1e6).round() - v * 1e6).abs() < 1e-3)
    })?;
    Some((full_double(data, length_at, end)?, full_double(data, angle_at, end)?))
}

/// A label text component read by its fields: the component head (see
/// [`component_head`]; a height of 1.0, a drawing unit, is the unset
/// one), the general block and its offsets (see [`general_offsets`]), the
/// content (see [`content_at`]), then BD rotation, BD, BS attachment.
pub(super) fn text_component(data: &[u8], start: usize, end: usize) -> Option<LabelText> {
    let (height, tag, general) = component_head(data, start, end)?;
    let height = (0.0..1.0).contains(&height).then_some(height).filter(|v| *v > 0.0);
    let (x_offset, y_offset, gap, after_gap) = general_offsets(data, general, end, tag == 0x0022_001F)?;
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
    })
}

/// UTF-16 strings stored inline as `BL byte length` + code units with a
/// null terminator, in record order.
pub(super) fn inline_strings(data: &[u8], start: usize, end: usize) -> Vec<String> {
    inline_strings_at(data, start, end).into_iter().map(|(s, _)| s).collect()
}

/// Inline strings with the bit position after each.
fn inline_strings_at(data: &[u8], start: usize, end: usize) -> Vec<(String, usize)> {
    // A misaligned read can look valid inside a real string; keep the
    // longest of any overlapping candidates.
    let mut found: Vec<(usize, usize, String)> = (start..end)
        .filter_map(|pos| string_at(data, pos, end).map(|(text, next)| (pos, next, text)))
        .collect();
    found.sort_by(|a, b| (b.1 - b.0).cmp(&(a.1 - a.0)));
    let mut kept: Vec<(usize, usize, String)> = Vec::new();
    for candidate in found {
        if kept.iter().all(|k| candidate.1 <= k.0 || candidate.0 >= k.1) {
            kept.push(candidate);
        }
    }
    kept.sort_by_key(|k| k.0);
    kept.into_iter().map(|k| (k.2, k.1)).collect()
}

fn string_at(data: &[u8], pos: usize, end: usize) -> Option<(String, usize)> {
    let mut bits = Bits::new(data, pos, end);
    let length = usize::try_from(bits.bit_long()?).ok()?;
    if !(4..=4096).contains(&length) || length % 2 != 0 || bits.remaining() < length * 8 {
        return None;
    }
    let mut units = Vec::with_capacity(length / 2);
    for _ in 0..length / 2 {
        units.push(u16::from_le_bytes([bits.raw_short_byte()?, bits.raw_short_byte()?]));
    }
    if units.pop() != Some(0) {
        return None;
    }
    // Label text is in a written script; control codes, surrogates and
    // private-use units mean a misaligned read.
    let script = |u: u16| {
        matches!(u, 0x09 | 0x20..=0x24F | 0x370..=0x52F | 0x590..=0x6FF | 0x900..=0x97F | 0xE00..=0xEFF | 0x2000..=0x206F | 0x2100..=0x22FF | 0x3000..=0x30FF | 0x4E00..=0x9FFF | 0xAC00..=0xD7A3)
    };
    if units.is_empty() || !units.iter().all(|&u| script(u)) {
        return None;
    }
    Some((String::from_utf16(&units).ok()?, bits.pos))
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
        let mut s = bl(2) + &bl(STATION) + &bd(station) + &bl(0x0100_FC31) + &bl(entity);
        s += &("00".to_string() + &rc(0) + &bl(0));
        s += &(bl(1) + &bl(5));
        s += &bl(0);
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
        bl(0) + &bl(0) + &bl(0) + &bd(0.0).repeat(5)
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
    fn nested_labels_are_not_read() {
        let nested = label(0.0, 0, None).replacen(&(bl(1) + &bl(5) + &bl(0)), &(bl(1) + &bl(5) + &bl(1)), 1);
        let (data, end) = record(&[nested], "");
        assert_eq!(stations(&data, 0, end), None);
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
    fn content_is_a_terminated_utf16_string() {
        let units: String = "A\nb\0".encode_utf16().flat_map(|u| u.to_le_bytes()).map(rc).collect();
        let s = bl(8) + &units;
        let data = bytes(&s);
        assert_eq!(content(&data, 0, s.len()), Some(("A\nb".to_string(), s.len())));
        let s = bl(2) + &rc(b'x') + &rc(0);
        assert_eq!(content(&bytes(&s), 0, s.len()), None);
    }
}
