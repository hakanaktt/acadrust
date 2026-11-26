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
    /// The header's references (n + m), before the keys' references.
    header: usize,
    components: Vec<(bool, Color, i16, f64)>,
    /// The bit after the component lists.
    end: usize,
}

const COMPONENT_LISTS: usize = 8;
/// Lists whose entries carry no hatch block.
const PLAIN_LISTS: usize = 4;

fn layout(data: &[u8], start: usize, end: usize) -> Option<Layout> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    let mut refs = 1;
    let mut header = 0;
    for _ in 0..2 {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
        for _ in 0..n {
            bits.bit_long()?;
        }
        refs += n;
        header += n;
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
    let (components, end) = component_lists(data, bits.pos, end).or_else(|| {
        let mut bits = Bits::new(data, bits.pos, end);
        let more = extra_keys(&mut bits)?;
        let found = component_lists(data, bits.pos, end)?;
        refs += more;
        Some(found)
    })?;
    Some(Layout {
        keys,
        refs,
        header,
        components,
        end,
    })
}

/// The block some styles hold between their keys and the component lists
/// (styles made new and their copies, not the ones a template brings):
/// `B 1, B 1`, or `B 1, BL n, n x key, BL, B 1`. Returns the number of
/// references its keys add.
fn extra_keys(bits: &mut Bits) -> Option<usize> {
    bits.bit()?.then_some(())?;
    let mut refs = 0;
    if !bits.bit()? {
        bits.pos -= 1;
        let n = bits.bit_long()?;
        (0..=64).contains(&n).then_some(())?;
        for _ in 0..n {
            refs += usize::from(KeyValue::has_reference(read_key(bits)?.0));
        }
        bits.bit_long()?;
        bits.bit()?.then_some(())?;
    }
    Some(refs)
}

/// The bit after a style's component lists, where its class data starts.
pub(super) fn class_data(data: &[u8], start: usize, end: usize) -> Option<usize> {
    Some(layout(data, start, end)?.end)
}

/// Where a style's class data starts, the index of its own reference
/// (the first after the header's lists and the keys' references) and how
/// many keys hold a text (their texts follow the record's own ones).
pub(super) fn class_layout(data: &[u8], start: usize, end: usize) -> Option<(usize, usize, usize)> {
    let l = layout(data, start, end)?;
    Some((l.end, l.refs - 1, l.keys.iter().filter(|(key, _)| key >> 24 == 0x04).count()))
}

/// Default and code entries of a code set style
/// (`AECC_ROADWAY_STYLE_SET`), from its class data:
///
/// ```text
/// 6 x { BL kind, BL 0, BL 0 }                 defaults: uncoded markers,
///                                             links, shapes, then markers,
///                                             links, shapes of codes the
///                                             set does not list
/// 3 x { BL n, n x { BL kind, BL 0, BL 0 } }   the listed point, link and
///                                             area codes
/// ```
///
/// `kind` is 1 for a point, 2 for a link and 3 for an area. Every entry
/// has a group of five references (material, style, label style, feature
/// line style, blank) after the record's owned children, in entry order,
/// and every listed code four texts (code, name, description, blank) at
/// the end of the text stream.
pub(super) fn code_set_entries(data: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
    let mut bits = Bits::new(data, class_data(data, start, end)?, end);
    let mut entry = |bits: &mut Bits| -> Option<()> {
        (1..=3).contains(&bits.bit_long()?).then_some(())?;
        bits.bit_long()?;
        bits.bit_long()?;
        Some(())
    };
    for _ in 0..6 {
        entry(&mut bits)?;
    }
    let mut codes = 0;
    for _ in 0..3 {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 6 <= bits.remaining())?;
        for _ in 0..n {
            entry(&mut bits)?;
        }
        codes += n;
    }
    Some((6, codes))
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
) -> Vec<DisplayComponent> {
    let Some(layout) = layout(data, start, end) else {
        return Vec::new();
    };
    let count = layout.components.len();
    let triples: Vec<&[u64]> = handles.get(layout.refs..).unwrap_or_default().chunks_exact(3).take(count).collect();
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

/// The title of a profile view style's graph.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewTitle {
    /// The title (a label format string).
    pub text: String,
    /// Text style handle (written: 0 for the drawing's "Standard").
    pub text_style: u64,
    pub text_height: f64,
    /// X and Y offset.
    pub offset: [f64; 2],
    /// 0 top, 1 bottom, 2 left, 3 right.
    pub location: i32,
    /// 0 top or left, 1 middle or center, 2 bottom or right.
    pub justification: i32,
    pub border: bool,
    pub border_gap: f64,
}

/// One axis's title of a profile view style.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewAxisTitle {
    pub text: String,
    /// Text style handle (written: 0 for the drawing's "Standard").
    pub text_style: u64,
    pub text_height: f64,
    /// X and Y offset.
    pub offset: [f64; 2],
    /// Radians.
    pub rotation: f64,
    /// 0 bottom or right, 1 center, 2 top or left.
    pub location: i32,
}

/// One kind of ticks of a profile view style's axis: major, minor, or the
/// horizontal geometry ticks of the top and bottom axes (which have no
/// interval: 0).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewTicks {
    pub interval: f64,
    pub size: f64,
    pub text_height: f64,
    /// The label (a label format string).
    pub label: String,
    /// Text style handle (written: 0 for the drawing's "Standard").
    pub text_style: u64,
    /// X and Y offset.
    pub offset: [f64; 2],
    /// Radians.
    pub rotation: f64,
    /// 0 top or left, 1 bottom or right, 2 center.
    pub justification: i32,
}

/// One axis of a profile view style.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewAxisStyle {
    /// Whether the axis shows its ticks and labels (the one setting the
    /// API's "show ticks and labels" and "ticks and labels at the start
    /// elevation" both write); the top axis has none and reads `true`.
    pub show_ticks_and_labels: bool,
    pub title: ProfileViewAxisTitle,
    pub major: ProfileViewTicks,
    pub minor: ProfileViewTicks,
    /// Top and bottom axes only.
    pub geometry: Option<ProfileViewTicks>,
}

/// Grid lines of one direction of a profile view style.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewGridLines {
    pub omit_in_padding: bool,
    pub clip_to_highest_profile: bool,
    pub clip: bool,
}

/// The grid of a profile view style; sides are left, right, bottom, top.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewGrid {
    pub padding: [f64; 4],
    pub axis_offset: [f64; 4],
    pub horizontal: ProfileViewGridLines,
    pub vertical: ProfileViewGridLines,
}

/// The settings of a profile view style (`AECC_PROFILE_VIEW_STYLE`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewStyleSettings {
    pub vertical_exaggeration: f64,
    /// Stations run right to left.
    pub right_to_left: bool,
    pub title: ProfileViewTitle,
    pub grid: ProfileViewGrid,
    pub left: ProfileViewAxisStyle,
    pub right: ProfileViewAxisStyle,
    pub top: ProfileViewAxisStyle,
    pub bottom: ProfileViewAxisStyle,
}

/// Profile view style setting keys: `type << 24 | 0x01_6000 | g << 8 |
/// item`, g 0 for the graph and the grid, 1 to 4 for the left, right, top
/// and bottom axes, 5 for the grid lines and the axes' tick switches.
pub(super) mod view_key {
    pub const EXAGGERATION: u32 = 0x0201_6001;
    pub const RIGHT_TO_LEFT: u32 = 0x0301_9001;
    pub const TITLE_TEXT: u32 = 0x1801_600A;
    pub const TITLE_STYLE: u32 = 0x1001_6002;
    pub const TITLE_OFFSET: u32 = 0x0901_6003;
    pub const TITLE_LOCATION: u32 = 0x0D01_6006;
    pub const TITLE_JUSTIFICATION: u32 = 0x0D01_6007;
    pub const TITLE_BORDER: u32 = 0x0301_6008;
    pub const TITLE_BORDER_GAP: u32 = 0x0201_6009;
    pub const TITLE_HEIGHT: u32 = 0x0201_600B;
    /// Left, right, bottom, top.
    pub const PADDING: [u32; 4] = [0x0201_6019, 0x0201_600A, 0x0201_6018, 0x0201_6017];
    pub const AXIS_OFFSET: [u32; 4] = [0x0201_6015, 0x0201_6016, 0x0201_6014, 0x0201_6013];
    /// Horizontal then vertical: omit in padding, clip to the highest
    /// profile, clip.
    pub const GRID_LINES: [[u32; 3]; 2] = [[0x0301_6509, 0x0301_6508, 0x0301_6501], [0x0301_650B, 0x0301_650A, 0x0301_6503]];
    /// The axes' tick switches: left, right, top (none), bottom.
    pub const SHOW: [Option<u32>; 4] = [Some(0x0301_6507), Some(0x0301_650C), None, Some(0x0301_6506)];
    /// An axis setting: `ty` the key type, `axis` 1 to 4, `item` the
    /// setting.
    pub const fn axis(ty: u32, axis: u32, item: u32) -> u32 {
        ty << 24 | 0x01_6000 | axis << 8 | item
    }
    /// Axis title items: text, style, location, offset, height, rotation.
    pub const TITLE: [u32; 6] = [0x02, 0x01, 0x03, 0x04, 0x07, 0x13];
    /// Tick items (major, minor, geometry): interval, size, height, label,
    /// style, offset, rotation, justification; 0 for none.
    pub const TICKS: [[u32; 8]; 3] = [[0x0C, 0x0D, 0x08, 0x0A, 0x14, 0x16, 0x15, 0x27], [0x0E, 0x0F, 0x09, 0x0B, 0x19, 0x1B, 0x1A, 0x28], [0, 0x24, 0x23, 0x25, 0x1E, 0x20, 0x1F, 0x29]];
}

/// The settings of a profile view style record; `refs` are its
/// references (the keys' references follow the header's, in key order).
pub(super) fn view_style_settings(data: &[u8], start: usize, end: usize, refs: &[u64]) -> Option<ProfileViewStyleSettings> {
    let layout = layout(data, start, end)?;
    let keys = &layout.keys;
    view_settings_from(&|key| keys.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()), &|key| {
        let k = keys.iter().filter(|(k, _)| KeyValue::has_reference(*k)).position(|(k, _)| *k == key)?;
        refs.get(layout.header + k).copied()
    })
}

/// [`ProfileViewStyleSettings`] from a style's settings: `get` a key's
/// value, `reference` the record a reference key points to.
pub(super) fn view_settings_from(get: &dyn Fn(u32) -> Option<KeyValue>, reference: &dyn Fn(u32) -> Option<u64>) -> Option<ProfileViewStyleSettings> {
    use view_key::*;
    let double = |key: u32| match get(key)? {
        KeyValue::Double(v) => Some(v),
        _ => None,
    };
    let long = |key: u32| match get(key)? {
        KeyValue::Long(v) => Some(v),
        _ => None,
    };
    let flag = |key: u32| match get(key)? {
        KeyValue::Byte(v) => Some(v != 0),
        _ => None,
    };
    let point = |key: u32| match get(key)? {
        KeyValue::Vector(v) => Some([v[0], v[1]]),
        _ => None,
    };
    let text = |key: u32| match get(key)? {
        KeyValue::Text(t) => Some(t.trim_end_matches('\0').to_string()),
        _ => None,
    };
    let ticks = |g: u32, items: [u32; 8]| -> Option<ProfileViewTicks> {
        let at = |ty: u32, i: usize| axis(ty, g, items[i]);
        Some(ProfileViewTicks {
            interval: if items[0] == 0 { 0.0 } else { double(at(0x02, 0))? },
            size: double(at(0x02, 1))?,
            text_height: double(at(0x02, 2))?,
            label: text(at(0x18, 3))?,
            text_style: reference(at(0x10, 4))?,
            offset: point(at(0x09, 5))?,
            rotation: double(at(0x02, 6))?,
            justification: long(at(0x0D, 7))?,
        })
    };
    let side = |g: u32| -> Option<ProfileViewAxisStyle> {
        let at = |ty: u32, i: usize| axis(ty, g, TITLE[i]);
        Some(ProfileViewAxisStyle {
            show_ticks_and_labels: match SHOW[g as usize - 1] {
                Some(k) => flag(k)?,
                None => true,
            },
            title: ProfileViewAxisTitle {
                text: text(at(0x18, 0))?,
                text_style: reference(at(0x10, 1))?,
                location: long(at(0x0D, 2))?,
                offset: point(at(0x09, 3))?,
                text_height: double(at(0x02, 4))?,
                rotation: double(at(0x02, 5))?,
            },
            major: ticks(g, TICKS[0])?,
            minor: ticks(g, TICKS[1])?,
            geometry: if g >= 3 { Some(ticks(g, TICKS[2])?) } else { None },
        })
    };
    let lines = |k: [u32; 3]| Some(ProfileViewGridLines { omit_in_padding: flag(k[0])?, clip_to_highest_profile: flag(k[1])?, clip: flag(k[2])? });
    let sides = |k: [u32; 4]| -> Option<[f64; 4]> { Some([double(k[0])?, double(k[1])?, double(k[2])?, double(k[3])?]) };
    Some(ProfileViewStyleSettings {
        vertical_exaggeration: double(EXAGGERATION)?,
        right_to_left: flag(RIGHT_TO_LEFT)?,
        title: ProfileViewTitle {
            text: text(TITLE_TEXT)?,
            text_style: reference(TITLE_STYLE)?,
            text_height: double(TITLE_HEIGHT)?,
            offset: point(TITLE_OFFSET)?,
            location: long(TITLE_LOCATION)?,
            justification: long(TITLE_JUSTIFICATION)?,
            border: flag(TITLE_BORDER)?,
            border_gap: double(TITLE_BORDER_GAP)?,
        },
        grid: ProfileViewGrid { padding: sides(PADDING)?, axis_offset: sides(AXIS_OFFSET)?, horizontal: lines(GRID_LINES[0])?, vertical: lines(GRID_LINES[1])? },
        left: side(1)?,
        right: side(2)?,
        top: side(3)?,
        bottom: side(4)?,
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

    fn packed(s: &str) -> (Vec<u8>, usize) {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        (s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect(), s.len())
    }

    #[test]
    fn reads_both_extra_key_forms() {
        // B 1, B 1.
        let (b, len) = packed("1 1");
        let mut bits = Bits::new(&b, 0, len);
        assert_eq!(extra_keys(&mut bits), Some(0));
        assert_eq!(bits.pos, 2);
        // B 1, BL 1, key 0x10010101 (a reference, no value), BL 1, B 1.
        let (b, len) = packed("1 0100000001 00 00000001 00000001 00000001 00010000 0100000001 1");
        let mut bits = Bits::new(&b, 0, len);
        assert_eq!(extra_keys(&mut bits), Some(1));
        assert_eq!(bits.pos, len);
        // A list (BL 32 opens it) is not the block.
        let (b, len) = packed("0100100000");
        assert_eq!(extra_keys(&mut Bits::new(&b, 0, len)), None);
    }

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
