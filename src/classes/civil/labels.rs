//! Label groups.
//!
//! A label group stores one fixed-size record per label, each opening with
//! the label's station as a BD; the label text itself is generated from the
//! label style and is not stored.

use super::bits::Bits;

/// Stations of the longest run of equally spaced records with rising values.
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
        if value >= 0.01 && value < 1e7 && (centi - centi.round()).abs() < 1e-6 {
            found.push((pos, value));
        }
    }
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
            while let Some(&(_, v)) = found.iter().find(|(p, _)| *p == at) {
                if v <= *run.last().unwrap() {
                    break;
                }
                run.push(v);
                at += stride;
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
}

impl LabelPart {
    /// Text for a field's value with the field's options.
    pub fn format_value(options: &[u32], value: f64) -> String {
        let precision = options
            .iter()
            .find(|o| (option::PRECISION_0..=option::PRECISION_MAX).contains(*o))
            .map_or(2, |o| (o - option::PRECISION_0) as usize);
        if options.contains(&option::STATION) {
            let km = (value / 1000.0).floor();
            let metres = value - km * 1000.0;
            let width = if precision == 0 { 3 } else { 4 + precision };
            return format!("{}+{:0width$.precision$}", km as i64, metres, width = width, precision = precision);
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

/// A label text component: its content, text height and vertical offset, as
/// plotted sizes in drawing units at 1:1 (multiply by the drawing scale).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelText {
    pub content: Vec<LabelPart>,
    pub height: Option<f64>,
    pub y_offset: Option<f64>,
}

/// BD at `pos` written in full.
fn full_double(data: &[u8], pos: usize, end: usize) -> Option<f64> {
    let mut bits = Bits::new(data, pos, end);
    let value = bits.bit_double()?;
    (bits.pos == pos + 66 && value.is_finite()).then_some(value)
}

/// Every field of a text component sits at a fixed distance from this BD.
const COMPONENT_MARKER: f64 = 156_679.000_488_281_22;

pub(super) fn text_component(data: &[u8], start: usize, end: usize) -> Option<LabelText> {
    let content = inline_strings(data, start, end)
        .into_iter()
        .max_by_key(|s| (s.contains("<["), s.len()))?;
    let marker = (start..end)
        .find(|&pos| full_double(data, pos, end).is_some_and(|v| (v - COMPONENT_MARKER).abs() < 1e-6));
    let plotted = |v: f64| (v.abs() > 1e-6 && v.abs() < 1.0).then_some(v);
    let height = marker
        .and_then(|m| m.checked_sub(68))
        .and_then(|p| full_double(data, p, end))
        .and_then(plotted)
        .filter(|v| *v > 0.0);
    let y_offset = marker
        .and_then(|m| full_double(data, m + 147, end))
        .and_then(plotted);
    Some(LabelText {
        content: parse_format(&content),
        height,
        y_offset,
    })
}

/// UTF-16 strings stored inline as `BL byte length` + code units with a
/// null terminator, in record order.
pub(super) fn inline_strings(data: &[u8], start: usize, end: usize) -> Vec<String> {
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
    kept.into_iter().map(|k| k.2).collect()
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
