//! Civil 3D object styles, read by their fields:
//!
//! ```text
//! BS 0x04BE, BL id, BL n, n x BL, BL m, m x BL     n + m references
//! [BL 0, B 1, RC count, count x key]              settings (alignment::read_key)
//! BL 32, 3 bits
//! 4 x { BL count, count x { BL index, component, 4 x BL } }
//! 4 x { BL count, count x { BL index, component, 4 x BL, hatch } }
//! ...                                             class data
//! ```
//!
//! The first four component lists are the view directions (plan, model,
//! profile, section); an empty one is a direction the class does not draw
//! in. The last four hold hatch components, each closed by its hatch
//! block. A component:
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
//! The style's references run: the header's n + m, one per key that takes
//! a reference (types 0x0F, 0x10, 0x12), one more, then a `(layer,
//! linetype, plot style)` triple per component in record order.

use super::alignment::{read_key, KeyValue};
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

/// A style record read up to the end of its component lists.
struct Layout {
    keys: Vec<(u32, KeyValue)>,
    /// References before the components' triples.
    refs: usize,
    components: Vec<(bool, Color, i16, f64)>,
    /// The bit after the component lists.
    end: usize,
    /// Bits skipped before the component lists (see [`layout`]).
    skipped: bool,
}

const COMPONENT_LISTS: usize = 8;
/// Lists whose entries carry no hatch block.
const PLAIN_LISTS: usize = 4;

fn layout(data: &[u8], start: usize, end: usize) -> Option<Layout> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    let mut refs = 1;
    for _ in 0..2 {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
        for _ in 0..n {
            bits.bit_long()?;
        }
        refs += n;
    }
    // A style without settings (feature styles) goes on with the BL below.
    let mut keys = Vec::new();
    let mut probe = Bits::new(data, bits.pos, end);
    if probe.bit_long()? == 0 && probe.bit()? {
        let count = probe.bits_value(8)?;
        for _ in 0..count {
            keys.push(read_key(&mut probe)?);
        }
        bits = probe;
    }
    refs += keys.iter().filter(|(key, _)| KeyValue::has_reference(*key)).count();
    let mut skipped = false;
    let (components, end) = component_lists(data, bits.pos, end).or_else(|| {
        // debt: some profile styles (S24's, one in Kemer) have 2 or 56 more
        // bits before the BL 32 (the 56 with one more reference), not
        // mapped; the lists are found past them.
        skipped = true;
        (bits.pos + 1..(bits.pos + 512).min(end)).find_map(|pos| component_lists(data, pos, end))
    })?;
    Some(Layout {
        keys,
        refs,
        components,
        end,
        skipped,
    })
}

/// The bit after a style's component lists, where its class data starts.
pub(super) fn class_data(data: &[u8], start: usize, end: usize) -> Option<usize> {
    Some(layout(data, start, end)?.end)
}

/// `BL 32`, 3 bits, then the component lists; the components and the bit
/// after them.
fn component_lists(data: &[u8], pos: usize, end: usize) -> Option<(Vec<(bool, Color, i16, f64)>, usize)> {
    let mut bits = Bits::new(data, pos, end);
    (bits.bit_long()? == 32).then_some(())?;
    bits.pos += 3;
    let mut components = Vec::new();
    for list in 0..COMPONENT_LISTS {
        let n = bits.bit_long()?;
        for _ in 0..n {
            bits.bit_long()?;
            components.push(record(&mut bits)?);
            for _ in 0..4 {
                bits.bit_long()?;
            }
            if list >= PLAIN_LISTS {
                hatch(&mut bits)?;
            }
        }
    }
    Some((components, bits.pos))
}

/// A hatch block: `BL 80, BL pattern type, BD scale, BD angle`, 13 bits.
// ponytail: the 13 bits are `0010101000101` in every sample (about 600
// blocks), so they are skipped by length; name their fields once a drawing
// varies them.
fn hatch(bits: &mut Bits) -> Option<()> {
    (bits.bit_long()? == 80).then_some(())?;
    bits.bit_long()?;
    bits.bit_double()?;
    bits.bit_double()?;
    (bits.remaining() >= 13).then_some(())?;
    bits.pos += 13;
    Some(())
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

/// The components with their layer and linetype references.
pub(super) fn components(
    data: &[u8],
    start: usize,
    end: usize,
    handles: &[u64],
    is_layer: impl Fn(u64) -> bool,
) -> Vec<DisplayComponent> {
    let Some(layout) = layout(data, start, end) else {
        return Vec::new();
    };
    let count = layout.components.len();
    let triples_at = |first: usize| -> Vec<&[u64]> { handles.get(first..).unwrap_or_default().chunks_exact(3).take(count).collect() };
    let first = if layout.skipped {
        // debt: the skipped bits hold references too; the triples start
        // where the most of them name a layer.
        (layout.refs..=handles.len().saturating_sub(3 * count))
            .max_by_key(|&first| (triples_at(first).iter().filter(|t| is_layer(t[0])).count(), std::cmp::Reverse(first)))
            .unwrap_or(layout.refs)
    } else {
        layout.refs
    };
    let triples = triples_at(first);
    if triples.len() < count {
        return Vec::new();
    }
    layout
        .components
        .into_iter()
        .zip(triples)
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

/// The `BD` value of setting `key`.
pub(super) fn setting(data: &[u8], start: usize, end: usize, key: u32) -> Option<f64> {
    layout(data, start, end)?.keys.into_iter().find_map(|(k, value)| match value {
        KeyValue::Double(v) if k == key => Some(v),
        _ => None,
    })
}

/// Data band style height.
pub(super) const BAND_HEIGHT: u32 = 0x0201_7001;
/// Surface style contour intervals.
const MINOR_INTERVAL: u32 = 0x0201_460C;
const MAJOR_INTERVAL: u32 = 0x0201_4621;

/// `(minor, major)` contour intervals of a surface style.
pub(super) fn contour_intervals(data: &[u8], start: usize, end: usize) -> Option<(f64, f64)> {
    let value = |key: u32| setting(data, start, end, key).filter(|v| *v > 0.0);
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

/// Axis settings: `0x0201_6g_kk`, g the axis (1 left, 2 right, 3 top,
/// 4 bottom), kk the setting.
const AXIS_MAJOR_INTERVAL: u32 = 0x0C;
const AXIS_MINOR_INTERVAL: u32 = 0x0E;
const AXIS_MAJOR_TICK: u32 = 0x0D;
const AXIS_MINOR_TICK: u32 = 0x0F;
const AXIS_MAJOR_TEXT: u32 = 0x08;
const AXIS_MINOR_TEXT: u32 = 0x09;

pub(super) fn view_axes(data: &[u8], start: usize, end: usize) -> Option<ViewAxes> {
    let keys = layout(data, start, end)?.keys;
    let axis = |group: u32| -> Option<ViewAxis> {
        let value = |item: u32| {
            keys.iter().find_map(|(k, v)| match v {
                KeyValue::Double(v) if *k == 0x0201_6000 | group << 8 | item => Some(*v),
                _ => None,
            })
        };
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
        left: axis(1)?,
        right: axis(2)?,
        top: axis(3)?,
        bottom: axis(4)?,
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

/// After the component lists: `BS kind`, two bits, `BD size`, then a byte
/// whose value 2 marks an absolute size.
pub(super) fn marker(data: &[u8], start: usize, end: usize) -> Option<Marker> {
    let mut bits = Bits::new(data, layout(data, start, end)?.end, end);
    let kind = bits.bit_short()?;
    bits.pos += 2;
    let size = bits.bit_double()?;
    let mode = bits.raw_short_byte()?;
    Some(Marker {
        kind: u32::try_from(kind).ok()?,
        size,
        absolute: mode == 2,
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
            assert_eq!(record(&mut Bits::new(&data, 0, data.len() * 8)), Some((true, Color::ByLayer, -1, 1.0)), "tag {v}");
        }
        let data = record_bits(60);
        assert_eq!(record(&mut Bits::new(&data, 0, data.len() * 8)), None);
    }
}
