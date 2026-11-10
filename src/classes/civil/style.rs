//! Display components of Civil 3D object styles.
//!
//! A style lists one record per display component, per view direction
//! (plan first, then model, ...), in the order the class defines:
//!
//! ```text
//! BS v, BS v, BS 0, BS v      component tag; v is 70 in Civil 3D 2013
//!                             drawings (AC1027) and 80 from 2018 (AC1032)
//! B    visible
//! CMC  color
//! BS   lineweight
//! BD   linetype scale
//! ```
//!
//! Each record's layer and linetype come from the style's handles, one
//! `(layer, linetype, plot style)` triple per record, in record order.

use super::bits::Bits;
use crate::types::Color;

/// How a style draws one component of its object.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisplayComponent {
    pub visible: bool,
    pub color: Color,
    /// Layer handle.
    pub layer: u64,
    /// Linetype handle.
    pub linetype: u64,
    /// Lineweight in 0.01 mm; -1 ByLayer, -2 ByBlock, -3 default.
    pub lineweight: i16,
    pub linetype_scale: f64,
}

/// `(visible, color, lineweight, linetype scale)` of each component record.
fn records(data: &[u8], start: usize, end: usize) -> Vec<(bool, Color, i16, f64)> {
    let mut out = Vec::new();
    let mut pos = start;
    while pos < end {
        let mut bits = Bits::new(data, pos, end);
        match record(&mut bits) {
            Some(r) => {
                out.push(r);
                pos = bits.pos;
            }
            None => pos += 1,
        }
    }
    out
}

fn record(bits: &mut Bits) -> Option<(bool, Color, i16, f64)> {
    let tag = [bits.bit_short()?, bits.bit_short()?, bits.bit_short()?, bits.bit_short()?];
    (matches!(tag[0], 70 | 80) && tag == [tag[0], tag[0], 0, tag[0]]).then_some(())?;
    let visible = bits.bit()?;
    bits.bit_short()?;
    let rgb = bits.bit_long()? as u32;
    bits.raw_short_byte()?; // name flags; the names sit in the text stream
    let [b, g, r, method] = rgb.to_le_bytes();
    let color = match method {
        0xC0 => Color::ByLayer,
        0xC1 => Color::ByBlock,
        0xC2 => Color::from_rgb(r, g, b),
        0xC3 => Color::from_index(i16::from(b)),
        0xC8 => Color::None,
        _ => return None,
    };
    let lineweight = bits.bit_short()?;
    let linetype_scale = bits.bit_double()?;
    Some((visible, color, lineweight, linetype_scale))
}

/// Records paired with their `(layer, linetype, plot style)` triples. A
/// class writes its own references before the triples and some after them,
/// so the triples start where the most of them name a layer.
pub(super) fn components(
    data: &[u8],
    start: usize,
    end: usize,
    handles: &[u64],
    is_layer: impl Fn(u64) -> bool,
) -> Vec<DisplayComponent> {
    let records = records(data, start, end);
    let Some(last) = handles.len().checked_sub(3 * records.len()) else {
        return Vec::new();
    };
    let layers_at = |first: usize| {
        handles[first..]
            .chunks(3)
            .take(records.len())
            .filter(|refs| is_layer(refs[0]))
            .count()
    };
    let Some(first) = (0..=last).rev().max_by_key(|&first| layers_at(first)) else {
        return Vec::new();
    };
    records
        .into_iter()
        .zip(handles[first..].chunks(3))
        .map(|((visible, color, lineweight, linetype_scale), refs)| DisplayComponent {
            visible,
            color,
            layer: refs[0],
            linetype: refs[1],
            lineweight,
            linetype_scale,
        })
        .collect()
}

/// Settings a style keeps as `key (10 bits), group (10 bits), BS type,
/// value` records; type 2 is a `BD`. Returns `(key, group, value)` of every
/// `BD` record, in record order.
pub(super) fn keyed_doubles(data: &[u8], start: usize, end: usize) -> Vec<(u16, u16, f64)> {
    let mut out = Vec::new();
    let read = |pos: usize| -> Option<(u16, u16, f64)> {
        let mut bits = Bits::new(data, pos, end);
        let mut field = || -> Option<u16> {
            let mut v = 0u16;
            for _ in 0..10 {
                v = (v << 1) | u16::from(bits.bit()?);
            }
            Some(v)
        };
        let key = field()?;
        let group = field()?;
        (bits.compact_short()? == 2).then_some(())?;
        let value = bits.bit_double().filter(|v| v.is_finite())?;
        Some((key, group, value))
    };
    for pos in start..end.saturating_sub(32) {
        if let Some(record) = read(pos) {
            out.push(record);
        }
    }
    out
}

/// First value of `key` in `group`.
pub(super) fn keyed(records: &[(u16, u16, f64)], key: u16, group: u16) -> Option<f64> {
    records.iter().find(|r| r.0 == key && r.1 == group).map(|r| r.2)
}

/// Data band style height: key, group.
pub(super) const BAND_HEIGHT: (u16, u16) = (0b00_0001_0111, 0);

/// Surface style contour intervals: key, group.
const MINOR_INTERVAL: (u16, u16) = (0b00_1100_0100, 0b01_1000_0000);
const MAJOR_INTERVAL: (u16, u16) = (0b10_0001_0100, 0b01_1000_0000);

/// `(minor, major)` contour intervals of a surface style.
pub(super) fn contour_intervals(data: &[u8], start: usize, end: usize) -> Option<(f64, f64)> {
    let records = keyed_doubles(data, start, end);
    let value = |(key, group): (u16, u16)| keyed(&records, key, group).filter(|v| *v > 0.0);
    Some((value(MINOR_INTERVAL)?, value(MAJOR_INTERVAL)?))
}

/// One axis of a profile view style, in plotted units for sizes.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewAxis {
    pub major_interval: f64,
    pub minor_interval: f64,
    pub major_tick_size: f64,
    pub minor_tick_size: f64,
    pub major_text_height: f64,
    pub minor_text_height: f64,
}

/// Axes of a profile view style.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewAxes {
    pub left: ViewAxis,
    pub right: ViewAxis,
    pub top: ViewAxis,
    pub bottom: ViewAxis,
}

/// Axis setting keys and the group of each axis.
const AXIS_MAJOR_INTERVAL: u16 = 0b00_1100_0110;
const AXIS_MINOR_INTERVAL: u16 = 0b00_1110_0110;
const AXIS_MAJOR_TICK: u16 = 0b00_1101_0110;
const AXIS_MINOR_TICK: u16 = 0b00_1111_0110;
const AXIS_MAJOR_TEXT: u16 = 0b00_1000_0110;
const AXIS_MINOR_TEXT: u16 = 0b00_1001_0110;
const LEFT: u16 = 0b00_0100_0000;
const RIGHT: u16 = 0b00_1000_0000;
const TOP: u16 = 0b00_1100_0000;
const BOTTOM: u16 = 0b01_0000_0000;

pub(super) fn view_axes(data: &[u8], start: usize, end: usize) -> Option<ViewAxes> {
    let records = keyed_doubles(data, start, end);
    let axis = |group: u16| -> Option<ViewAxis> {
        let value = |key: u16| keyed(&records, key, group);
        Some(ViewAxis {
            major_interval: value(AXIS_MAJOR_INTERVAL).filter(|v| *v > 0.0)?,
            minor_interval: value(AXIS_MINOR_INTERVAL).filter(|v| *v > 0.0)?,
            major_tick_size: value(AXIS_MAJOR_TICK).unwrap_or(0.0),
            minor_tick_size: value(AXIS_MINOR_TICK).unwrap_or(0.0),
            major_text_height: value(AXIS_MAJOR_TEXT).unwrap_or(0.0),
            minor_text_height: value(AXIS_MINOR_TEXT).unwrap_or(0.0),
        })
    };
    Some(ViewAxes {
        left: axis(LEFT)?,
        right: axis(RIGHT)?,
        top: axis(TOP)?,
        bottom: axis(BOTTOM)?,
    })
}

/// Bits a label tick style writes after its size.
const TICK_TRAILER_BITS: usize = 25;

/// Plotted size of a label tick style: the full `BD` that closes its body
/// before a fixed 25-bit trailer.
pub(super) fn tick_size(data: &[u8], start: usize, end: usize) -> Option<f64> {
    let at = end.checked_sub(TICK_TRAILER_BITS + 66)?;
    (at >= start).then_some(())?;
    let mut bits = Bits::new(data, at, end);
    let size = bits.bit_double()?;
    (bits.pos == at + 66 && size.is_finite() && size > 0.0).then_some(size)
}

/// How a marker style draws a point: AutoCAD point-style bits (low bits:
/// 0 dot, 1 nothing, 2 plus, 3 cross, 4 tick; +32 circle, +64 square) at
/// `size`, either in drawing units or in plotted units.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Marker {
    pub kind: u32,
    pub size: f64,
    /// `true`: `size` is in drawing units; `false`: in plotted units.
    pub absolute: bool,
}

/// After the last display component: `BL kind`, two bits, `BD size`, then
/// a byte whose value 2 marks an absolute size.
pub(super) fn marker(data: &[u8], start: usize, end: usize) -> Option<Marker> {
    // The last component record, then the fields that follow it.
    let last = (start..end).rev().find(|&pos| {
        let mut bits = Bits::new(data, pos, end);
        record(&mut bits).is_some()
    })?;
    let mut bits = Bits::new(data, last, end);
    record(&mut bits)?;
    let after = bits.pos;
    (after..(after + 160).min(end)).find_map(|pos| {
        let mut bits = Bits::new(data, pos, end);
        let kind = bits.compact_short()?;
        (bits.bit()? && !bits.bit()?).then_some(())?;
        let at = bits.pos;
        let size = bits.bit_double()?;
        (bits.pos - at == 66 && size.is_finite() && size > 1e-6 && size < 1e5 && (0..128).contains(&kind)).then_some(())?;
        let mode = bits.raw_short_byte()?;
        Some(Marker {
            kind: kind as u32,
            size,
            absolute: mode == 2,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One component record with tag value `v`: visible, ByLayer, lineweight
    /// -1, linetype scale 1.
    fn record_bits(v: u8) -> Vec<u8> {
        let mut bits = String::new();
        let byte = |b: u8| format!("{b:08b}");
        for tag in [v, v, 0, v] {
            bits += &if tag == 0 { "10".to_string() } else { format!("01{}", byte(tag)) };
        }
        bits += "1"; // visible
        bits += "10"; // BS 0
        bits += "00";
        for b in 0xC000_0000u32.to_le_bytes() {
            bits += &byte(b);
        }
        bits += &byte(0); // name flags
        bits += "00";
        for b in (-1i16).to_le_bytes() {
            bits += &byte(b);
        }
        bits += "01"; // BD 1.0
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        bits.as_bytes().chunks(8).map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap()).collect()
    }

    #[test]
    fn reads_2013_and_2018_component_tags() {
        for v in [70, 80] {
            let data = record_bits(v);
            assert_eq!(records(&data, 0, data.len() * 8), vec![(true, Color::ByLayer, -1, 1.0)], "tag {v}");
        }
        let data = record_bits(60);
        assert!(records(&data, 0, data.len() * 8).is_empty());
    }
}
