//! Label groups.
//!
//! A label group stores one fixed-size record per label, each opening with
//! the label's station as a BD; the label text itself is generated from the
//! label style and is not stored.

use super::bits::Bits;

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
}

/// BD at `pos` written in full.
fn full_double(data: &[u8], pos: usize, end: usize) -> Option<f64> {
    let mut bits = Bits::new(data, pos, end);
    let value = bits.bit_double()?;
    (bits.pos == pos + 66 && value.is_finite()).then_some(value)
}

/// Every field of a text component sits at a fixed distance from this BD.
const COMPONENT_MARKER: f64 = 156_679.000_488_281_22;

/// A plotted size: zero, or a short decimal well under a drawing unit.
fn plotted_size(v: f64) -> bool {
    v == 0.0 || ((1e-6..0.1).contains(&v.abs()) && ((v * 1e6).round() - v * 1e6).abs() < 1e-3)
}

/// `BD x offset, BD, BD y offset, BD, B, BD gap`, the first run of them
/// after the component's header that reads as plotted sizes.
// ponytail: found by probing; the fields before it vary with the style's
// overrides. Read them in order once those are decoded.
fn offsets(data: &[u8], from: usize, end: usize) -> Option<(f64, f64, f64)> {
    offsets_within(data, from, end.min(from + 600))
}

fn offsets_within(data: &[u8], from: usize, end: usize) -> Option<(f64, f64, f64)> {
    (from..end).find_map(|pos| {
        let mut bits = Bits::new(data, pos, end);
        let mut values = [0.0; 4];
        for v in &mut values {
            *v = bits.bit_double().filter(|v| plotted_size(*v))?;
        }
        bits.bit()?;
        let at = bits.pos;
        let gap = bits.bit_double()?;
        (bits.pos == at + 66 && gap > 0.0 && plotted_size(gap)).then_some((values[0], values[2], gap))
    })
}

/// A line component closes with its length and then its angle, both
/// written in full.
pub(super) fn line_component(data: &[u8], start: usize, end: usize) -> Option<(f64, f64)> {
    let angle_at = (start..end)
        .rev()
        .find(|&p| full_double(data, p, end).is_some_and(|v| (1e-3..=std::f64::consts::TAU).contains(&v.abs())))?;
    let length_at = (start..angle_at.checked_sub(66)?).rev().find(|&p| {
        full_double(data, p, end).is_some_and(|v| (1e-6..1.0).contains(&v) && ((v * 1e6).round() - v * 1e6).abs() < 1e-3)
    })?;
    Some((full_double(data, length_at, end)?, full_double(data, angle_at, end)?))
}

pub(super) fn text_component(data: &[u8], start: usize, end: usize) -> Option<LabelText> {
    // The content is the longest format string; rotation and attachment
    // follow it.
    let (content, after) = inline_strings_at(data, start, end)
        .into_iter()
        .max_by_key(|(s, _)| (s.contains("<["), s.len()))?;
    let mut bits = Bits::new(data, after, end);
    let rotation = bits.bit_double().filter(|v| v.abs() <= 10.0).unwrap_or(0.0);
    bits.bit_double();
    let attachment = bits.bit_short().and_then(|v| u8::try_from(v).ok()).filter(|v| *v <= 8).unwrap_or(6);
    let marker = (start..end)
        .find(|&pos| full_double(data, pos, end).is_some_and(|v| (v - COMPONENT_MARKER).abs() < 1e-6));
    let plotted = |v: f64| (v.abs() > 1e-6 && v.abs() < 1.0).then_some(v);
    let height = marker
        .and_then(|m| m.checked_sub(68))
        .and_then(|p| full_double(data, p, end))
        .and_then(plotted)
        .filter(|v| *v > 0.0);
    // Components written without the marker keep the same block; it is then
    // the first one in the record.
    let (x_offset, y_offset, gap) = match marker {
        Some(m) => offsets(data, m + 82, end),
        None => offsets_within(data, start, end),
    }
    .unwrap_or((0.0, 0.0, 0.0));
    // A 16-bit header after the marker ends with the anchor point, when the
    // component's settings block (opening 1011) follows it directly.
    // ponytail: only that short header form is read; longer ones leave it
    // unknown until their fields are decoded.
    let anchor_point = marker.and_then(|m| {
        let mut bits = Bits::new(data, m + 82, end);
        let mut read = |n: usize| -> Option<u8> {
            let mut v = 0u8;
            for _ in 0..n {
                v = (v << 1) | u8::from(bits.bit()?);
            }
            Some(v)
        };
        read(12)?;
        let point = read(4)?;
        (read(4)? == 0b1011).then_some(point)
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
    // Label text is Latin script; anything else is a misaligned read.
    let latin = |u: u16| (0x20..=0x24F).contains(&u) || (0x2000..=0x206F).contains(&u) || u == 0x09;
    if units.is_empty() || !units.iter().all(|&u| latin(u)) {
        return None;
    }
    Some((String::from_utf16(&units).ok()?, bits.pos))
}
