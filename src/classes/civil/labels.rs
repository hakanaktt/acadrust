//! Label groups.
//!
//! A label group stores one fixed-size record per label, each opening with
//! the label's station as a BD; the label text itself is generated from the
//! label style and is not stored.

use super::bits::Bits;

/// Bits that open a label record in groups whose records vary in size
/// (geometry points between interval stations); the station follows as a
/// `BD`.
const RECORD_MARKER: &[u8] = b"00100000110100000000000000000010";

/// Stations of the records opened by [`RECORD_MARKER`], in record order.
/// Other fields open with the same bits; only a full-width double is taken,
/// and a leading zero station (stored in two bits) when the first record
/// holds one.
pub(super) fn marked_stations(data: &[u8], start: usize, end: usize) -> Vec<f64> {
    let bit = |p: usize| (data[p / 8] >> (7 - p % 8)) & 1;
    let mut stations = Vec::new();
    let mut first = true;
    for pos in start..end.saturating_sub(RECORD_MARKER.len() + 66) {
        if !RECORD_MARKER.iter().enumerate().all(|(i, c)| bit(pos + i) == c - b'0') {
            continue;
        }
        let mut bits = Bits::new(data, pos + RECORD_MARKER.len(), end);
        let at = bits.pos;
        let Some(value) = bits.bit_double() else { continue };
        if first && value == 0.0 {
            stations.push(0.0);
        }
        first = false;
        if bits.pos - at == 66 && value.is_finite() && value.abs() < 1e7 && value.abs() > 1e-9 {
            stations.push(value);
        }
    }
    stations
}

/// The labels of an alignment label group read by their fields: a 293-bit
/// header, `BL n`, `n` labels. A label is a key block (`BL k`, `k` keys; a
/// station label's station is key 0x0200D020, the first label holds the
/// group's settings), 44 bits, a key list (`BL k`, `k` keys without values),
/// `BL`, `BL m` and `m` items: `BL x3, BL mask`, then for mask 0x0CC04E08
/// 18 bits, for mask 0x0CC00208 (a dragged label) 7 bits, `BD`, `BD`, 5
/// bits. Returns the station keys' values and the bit after the labels.
// ponytail: the header, the 44 bits and the item bits around the BDs are
// skipped by length (constant in every sample); name them once a drawing
// varies them.
fn group_labels(data: &[u8], start: usize, end: usize) -> Option<(Vec<f64>, usize)> {
    const HEADER: usize = 293;
    const STATION: u32 = 0x0200_D020;
    let mut bits = Bits::new(data, start + HEADER, end);
    let n = bits.bit_long()?;
    let mut stations = Vec::new();
    for _ in 0..n {
        for _ in 0..bits.bit_long()? {
            if let (STATION, super::alignment::KeyValue::Double(station)) = super::alignment::read_key(&mut bits)? {
                stations.push(station);
            }
        }
        bits.skip(44)?;
        for _ in 0..bits.bit_long()? {
            bits.bit_long()?;
        }
        bits.bit_long()?;
        for _ in 0..bits.bit_long()? {
            for _ in 0..3 {
                bits.bit_long()?;
            }
            match bits.bit_long()? as u32 {
                0x0CC0_4E08 => bits.skip(18)?,
                0x0CC0_0208 => {
                    bits.skip(7)?;
                    bits.bit_double()?;
                    bits.bit_double()?;
                    bits.skip(5)?;
                }
                _ => return None,
            }
        }
    }
    Some((stations, bits.pos))
}

/// Stations of a station or minor station label group: its labels (see
/// [`group_labels`]), then a 294-bit trailer.
pub(super) fn group_stations(data: &[u8], start: usize, end: usize) -> Option<Vec<f64>> {
    let (stations, at) = group_labels(data, start, end)?;
    (end - at == 294).then_some(stations)
}

/// Geometry point kinds a geometry point label group lists, read after its
/// labels (see [`group_labels`]): a key block, `BL` x5, `BD, BD`, 7
/// bits, `BL count`, `count` x `BL` (the low byte is the kind: 1 the
/// alignment's beginning, 2 its end, 3 and up the points between), then 19
/// bits.
// ponytail: the 7 and 19 bits are skipped by length (constant in every
// sample).
pub(super) fn group_point_kinds(data: &[u8], start: usize, end: usize) -> Option<Vec<u8>> {
    let (_, at) = group_labels(data, start, end)?;
    let mut bits = Bits::new(data, at, end);
    for _ in 0..bits.bit_long()? {
        super::alignment::read_key(&mut bits)?;
    }
    for _ in 0..5 {
        bits.bit_long()?;
    }
    bits.bit_double()?;
    bits.bit_double()?;
    bits.skip(7)?;
    let count = bits.bit_long()?;
    let kinds = (0..count).map(|_| bits.bit_long().map(|v| (v & 0xFF) as u8)).collect::<Option<Vec<_>>>()?;
    (bits.remaining() == 19).then_some(kinds)
}

/// Bits before each geometry point kind a label group labels (groups
/// written from Civil 3D 2018 on).
const POINT_KIND_MARKER: &[u8] = b"0011001101010010011111111110100100000001";

/// Geometry point kinds a geometry point label group labels: `1` the
/// alignment's beginning, `2` its end. Each is the low byte of the `BL`
/// after [`POINT_KIND_MARKER`].
pub(super) fn geometry_point_kinds(data: &[u8], start: usize, end: usize) -> Vec<u8> {
    let bit = |p: usize| (data[p / 8] >> (7 - p % 8)) & 1;
    (start..end.saturating_sub(POINT_KIND_MARKER.len() + 34))
        .filter(|&pos| POINT_KIND_MARKER.iter().enumerate().all(|(i, c)| bit(pos + i) == c - b'0'))
        .filter_map(|pos| Bits::new(data, pos + POINT_KIND_MARKER.len(), end).bit_long())
        .map(|v| (v & 0xFF) as u8)
        .collect()
}

/// Bits of the `BD` that opens a label's placement block (groups written
/// from Civil 3D 2018 on); read as a double it is about -26.515625.
const PLACEMENT_MARKER: f64 = -26.515625;

/// Where a dragged label sits, from the point it labels: the two `BD`s
/// 178 and 244 bits past [`PLACEMENT_MARKER`]. A label left in place has
/// no full doubles there.
pub(super) fn dragged_offset(data: &[u8], start: usize, end: usize) -> Option<crate::types::Vector2> {
    let at = (start..end).find(|&pos| full_double(data, pos, end).is_some_and(|v| (v - PLACEMENT_MARKER).abs() < 1e-6))?;
    let x = full_double(data, at + 178, end)?;
    let y = full_double(data, at + 244, end)?;
    (x.abs() < 1e5 && y.abs() < 1e5).then(|| crate::types::Vector2::new(x, y))
}

/// Bits before the profile entity id of each label in a profile label
/// group; 56 bits on the record holds a `BL` of the form `0x00FC____`
/// (`0x00FC3108` for tangent labels, `0x00FC3408` for curve labels,
/// `0x00FC320C` for PVI labels).
const ENTITY_LABEL_PREFIX: &[u8] = b"00000000000001";

fn is_entity_label_check(v: i32) -> bool {
    v >> 16 == 0xFC && matches!(v & 0xFF, 0x08 | 0x0C)
}

/// Every value read as an entity id after [`ENTITY_LABEL_PREFIX`], with
/// its bit offset from `start`, checked or not. A PVI label's record names
/// the tangents on both sides of its PVI (one at the profile's ends).
pub(super) fn entity_refs(data: &[u8], start: usize, end: usize) -> Vec<(usize, i32)> {
    let bit = |p: usize| (data[p / 8] >> (7 - p % 8)) & 1;
    let n = ENTITY_LABEL_PREFIX.len();
    (start + n..end.saturating_sub(34))
        .filter(|&pos| ENTITY_LABEL_PREFIX.iter().enumerate().all(|(i, c)| bit(pos - n + i) == c - b'0'))
        .filter_map(|pos| Some((pos - start, Bits::new(data, pos, end).bit_long().filter(|v| *v > 0)?)))
        .collect()
}

/// Ids of the profile entities a profile label group labels, in record
/// order.
pub(super) fn labelled_entity_ids(data: &[u8], start: usize, end: usize) -> Vec<i32> {
    let bit = |p: usize| (data[p / 8] >> (7 - p % 8)) & 1;
    let n = ENTITY_LABEL_PREFIX.len();
    (start + n..end.saturating_sub(90))
        .filter(|&pos| ENTITY_LABEL_PREFIX.iter().enumerate().all(|(i, c)| bit(pos - n + i) == c - b'0'))
        .filter(|&pos| Bits::new(data, pos + 56, end).bit_long().is_some_and(is_entity_label_check))
        .filter_map(|pos| Bits::new(data, pos, end).bit_long())
        .collect()
}

/// Stations of the longest run of equally spaced records with rising values.
/// A record whose station is 0 is 64 bits shorter: the BD stores zero in
/// two bits.
pub(super) fn stations(data: &[u8], start: usize, end: usize) -> Vec<f64> {
    let mut found: Vec<(usize, f64)> = Vec::new();
    for pos in start..end {
        let mut bits = Bits::new(data, pos, end);
        let raw = bits.bit().zip(bits.bit());
        if raw != Some((false, false)) {
            continue;
        }
        let Some(value) = bits.raw_double() else { continue };
        let centi = value * 100.0;
        if value.abs() >= 0.01 && value.abs() < 1e7 && (centi - centi.round()).abs() < 1e-6 {
            found.push((pos, value));
        }
    }
    let full = |at: usize| found.iter().find(|(p, _)| *p == at).map(|&(_, v)| v);
    let zero = |at: usize| {
        let mut bits = Bits::new(data, at, end);
        bits.bit() == Some(true) && bits.bit() == Some(false)
    };
    let mut best: Vec<f64> = Vec::new();
    for (i, &(first, value)) in found.iter().enumerate() {
        for &(second, next) in &found[i + 1..] {
            let stride = second - first;
            if stride > 2000 {
                break;
            }
            if stride < 128 || next <= value {
                continue;
            }
            let mut run = vec![value, next];
            let mut at = second + stride;
            loop {
                let last = *run.last().unwrap();
                match full(at) {
                    Some(v) if v > last => {
                        run.push(v);
                        at += stride;
                    }
                    None if last < 0.0 && zero(at) => {
                        run.push(0.0);
                        at += stride - 64;
                    }
                    _ => break,
                }
            }
            // Records before the run: a zero station, then negative ones.
            let mut at = first;
            loop {
                let head = run[0];
                if head > 0.0 && at >= start + stride - 64 && zero(at - (stride - 64)) {
                    run.insert(0, 0.0);
                    at -= stride - 64;
                } else if let Some(v) = at.checked_sub(stride).and_then(full).filter(|v| *v < head) {
                    run.insert(0, v);
                    at -= stride;
                } else {
                    break;
                }
            }
            if run.len() > best.len() {
                best = run;
            }
        }
    }
    best.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    if best.len() < 2 {
        best.clear();
    }
    best
}

/// Anchor point of a single label: the first pair of full-width BDs that
/// reads as a drawing position (both coordinates above 1000 in size).
// ponytail: positional guess that holds for station/elevation labels; parse
// the label record properly if other single labels need it.
pub(super) fn anchor(data: &[u8], start: usize, end: usize) -> Option<crate::types::Vector2> {
    (start..end).find_map(|pos| {
        let mut bits = Bits::new(data, pos, end);
        let x = bits.bit_double()?;
        let y = (bits.pos == pos + 66).then(|| bits.bit_double()).flatten()?;
        let full = bits.pos == pos + 132;
        (full && x.is_finite() && y.is_finite() && x.abs() > 1e3 && y.abs() > 1e3 && x.abs() < 1e8 && y.abs() < 1e8)
            .then(|| crate::types::Vector2::new(x, y))
    })
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

/// Bits that open a text component's general block; its visibility is
/// the third bit after them.
const COMPONENT_GENERAL: &[u8] = b"1110100100101101";

/// Whether a text component is shown: unread (visible) without the block.
pub(super) fn component_visible(data: &[u8], start: usize, end: usize) -> bool {
    let bit = |p: usize| (data[p / 8] >> (7 - p % 8)) & 1;
    let n = COMPONENT_GENERAL.len();
    (start..end.saturating_sub(n + 3))
        .take(1200)
        .find(|&pos| COMPONENT_GENERAL.iter().enumerate().all(|(i, c)| bit(pos + i) == c - b'0'))
        .is_none_or(|pos| bit(pos + n + 2) == 1)
}

/// BD at `pos` written in full.
fn full_double(data: &[u8], pos: usize, end: usize) -> Option<f64> {
    let mut bits = Bits::new(data, pos, end);
    let value = bits.bit_double()?;
    (bits.pos == pos + 66 && value.is_finite()).then_some(value)
}

/// A plotted size: zero, or a short decimal well under a drawing unit.
fn plotted_size(v: f64) -> bool {
    v == 0.0 || ((1e-6..0.1).contains(&v.abs()) && ((v * 1e6).round() - v * 1e6).abs() < 1e-3)
}

/// `BD x offset, BD, BD y offset, BD, B, BD gap`, the first run of them
/// after the component's header that reads as plotted sizes.
// ponytail: found by probing; the fields before it vary with the style's
// overrides. Read them in order once those are decoded.
fn offsets(data: &[u8], from: usize, end: usize) -> Option<(f64, f64, f64, usize)> {
    offsets_within(data, from, end.min(from + 600))
}

/// The general block up to its fields: 12 flag bits, 32 bits more when
/// flag 4 is clear, then the anchor point (4 bits) and 20 bits. Returns
/// the anchor point.
fn general_head(bits: &mut Bits) -> Option<u8> {
    bits.bits_value(4)?;
    if bits.bits_value(8)? & 0x80 == 0 {
        bits.skip(32)?;
    }
    let point = bits.bits_value(4)?;
    bits.skip(20)?;
    Some(point)
}

/// X offset, y offset and border gap read by their fields: the general
/// head (see [`general_head`]), `1000`, 6 BL, B, BD x, BD, BD y, BD, B,
/// BD gap. Also returns the bit after the gap.
fn general_offsets(data: &[u8], at: usize, end: usize) -> Option<(f64, f64, f64, usize)> {
    let mut bits = Bits::new(data, at, end);
    general_head(&mut bits)?;
    (bits.bits_value(4)? == 0b1000).then_some(())?;
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

/// X offset, y offset and border gap, with where the block starts.
fn offsets_within(data: &[u8], from: usize, end: usize) -> Option<(f64, f64, f64, usize)> {
    (from..end).find_map(|pos| {
        let mut bits = Bits::new(data, pos, end);
        let mut values = [0.0; 4];
        for v in &mut values {
            *v = bits.bit_double().filter(|v| plotted_size(*v))?;
        }
        bits.bit()?;
        let at = bits.pos;
        let gap = bits.bit_double()?;
        (bits.pos == at + 66 && gap > 0.0 && plotted_size(gap)).then_some((values[0], values[2], gap, pos))
    })
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

pub(super) fn text_component(data: &[u8], start: usize, end: usize) -> Option<LabelText> {
    // After the style's component lists: `BL, 2 bits, BD height, BL, BL
    // color`, then the component itself: `BL type tag, BL, BL, BL`, then its
    // general block. A height of 1.0 (a drawing unit) is the unset one.
    let head = super::style::class_data(data, start, end).and_then(|at| {
        let mut bits = Bits::new(data, at, end);
        bits.bit_long()?;
        bits.pos += 2;
        let height = bits.bit_double()?;
        for _ in 0..6 {
            bits.bit_long()?;
        }
        Some(((0.0..1.0).contains(&height).then_some(height).filter(|v| *v > 0.0), bits.pos))
    });
    let height = head.and_then(|(height, _)| height);
    let general = head.map(|(_, at)| at);
    let fields = general.and_then(|at| general_offsets(data, at, end));
    // The content string, then rotation and attachment.
    let (content, after) = fields
        .and_then(|(.., pos)| content_at(data, pos, end))
        .and_then(|pos| string_at(data, pos, end))
        .or_else(|| {
            // debt: components the fields above do not read (8 of 1929 in
            // the samples, component tag 0x0022001F) take the longest
            // format string.
            inline_strings_at(data, start, end)
                .into_iter()
                .max_by_key(|(s, _)| (s.contains("<["), s.len()))
        })?;
    let mut bits = Bits::new(data, after, end);
    let rotation = bits.bit_double().filter(|v| v.abs() <= 10.0).unwrap_or(0.0);
    bits.bit_double();
    let attachment = bits.bit_short().and_then(|v| u8::try_from(v).ok()).filter(|v| *v <= 8).unwrap_or(6);
    let (x_offset, y_offset, gap) = fields
        .map(|(x, y, gap, _)| (x, y, gap))
        .or_else(|| {
            // debt: components whose general block does not read (none in
            // the samples) are found by probing.
            general.and_then(|at| offsets(data, at, end).map(|(x, y, gap, _)| (x, y, gap)))
        })
        .unwrap_or((0.0, 0.0, 0.0));
    // The anchor point, when the component's settings block (opening
    // 1011) follows it.
    let anchor_point = general.and_then(|at| {
        let mut bits = Bits::new(data, at, end);
        let point = general_head(&mut bits)?;
        bits.pos -= 20;
        (bits.bits_value(4)? == 0b1011).then_some(point)
    });
    Some(LabelText {
        content: parse_format(&content),
        height,
        x_offset,
        y_offset,
        gap,
        rotation,
        attachment,
        anchor_point,
        visible: component_visible(data, start, end),
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
