//! Label styles read field by field: the label collector style
//! (`AECC_LABEL_COLLECTOR_STYLE`, a label style's general, behavior,
//! plan readability, leader and dragged state settings) and its text
//! components (`AECC_LABEL_TEXT_STYLE`).
//!
//! Both open their class data (after the component lists, see
//! [`super::style::class_data`]) with the same base fields:
//!
//! ```text
//! BL height set, BL, BD height, BL, BL kind
//! BL mask A, BL mask B                        settings the record overrides
//! BL, BL, B label visible, B visible, B background mask, B,
//! BL anchor component id, BL anchor (low byte: 4 + anchor point)
//! BL orientation, BL insert, BL, BL, BL plan readable, BD readable bias,
//! B flip anchors, BD x offset, BD, BD y offset, BD, B border visible,
//! BD gap, B leader visible, B, B leader spline, B, BD arrowhead size,
//! CMC text color, BL text lineweight, CMC border color,
//! BL border lineweight, BL border type, CMC leader color,
//! BL leader lineweight, BL leader attachment, BL, BL, BL mask C
//! ```
//!
//! (CMC: `BS 0, BL color word, RC 0`.) A collector goes on with its
//! dragged state (mask C):
//!
//! ```text
//! B, B, B, B stacked, BD text height, B border visible, BD gap,
//! BS border type, B background mask, B leader justification,
//! CMC color, BL lineweight, BL n,
//! n x { BL, BD, BL, 9 bits, BL, BL, BL, BL, B, BL }, BL, BD max width
//! ```
//!
//! and a text component with its text:
//!
//! ```text
//! BL, BL bytes + UTF-16 contents, BD angle, BL, BL attachment, BL, B,
//! BD max width, BD
//! ```
//!
//! A setting whose mask bit is clear is taken from the label style
//! defaults; its stored value is then left over. The base references
//! are: own (soft), text style, owner style, arrowhead block, layer, -,
//! border linetype, leader linetype, dragged state linetype; a collector
//! lists its text components after them three times (hard pointers,
//! owners, soft pointers).

use super::bits::Bits;
use crate::types::Color;

#[derive(Clone, Copy, PartialEq)]
enum K {
    B,
    Bl,
    Bd,
    Cmc,
    Bits9,
    Text,
}

/// A field value.
#[derive(Clone, Debug, PartialEq)]
enum V {
    B(bool),
    Bl(i32),
    Bd(f64),
    /// Color word of a CMC.
    Cmc(u32),
    Bits(u16),
    Text(String),
}

#[derive(Clone)]
struct Field {
    name: &'static str,
    value: V,
    /// The bits the field was read from; written back while the value is
    /// unchanged.
    raw: String,
}

const BASE: [(&str, K); 42] = [
    ("height_set", K::Bl),
    ("b1", K::Bl),
    ("height", K::Bd),
    ("b3", K::Bl),
    ("kind", K::Bl),
    ("mask_a", K::Bl),
    ("mask_b", K::Bl),
    ("b7", K::Bl),
    ("b8", K::Bl),
    ("label_visible", K::B),
    ("visible", K::B),
    ("background_mask", K::B),
    ("b12", K::B),
    ("anchor_component", K::Bl),
    ("anchor", K::Bl),
    ("orientation", K::Bl),
    ("insert", K::Bl),
    ("b17", K::Bl),
    ("b18", K::Bl),
    ("plan_readable", K::Bl),
    ("readable_bias", K::Bd),
    ("flip_anchors", K::B),
    ("x_offset", K::Bd),
    ("b23", K::Bd),
    ("y_offset", K::Bd),
    ("b25", K::Bd),
    ("border_visible", K::B),
    ("gap", K::Bd),
    ("leader_visible", K::B),
    ("b29", K::B),
    ("leader_spline", K::B),
    ("b31", K::B),
    ("arrow_size", K::Bd),
    ("text_color", K::Cmc),
    ("text_lineweight", K::Bl),
    ("border_color", K::Cmc),
    ("border_lineweight", K::Bl),
    ("border_type", K::Bl),
    ("leader_color", K::Cmc),
    ("leader_lineweight", K::Bl),
    ("leader_attachment", K::Bl),
    ("b41", K::Bl),
];

const DRAGGED: [(&str, K); 13] = [
    ("d0", K::B),
    ("d1", K::B),
    ("d2", K::B),
    ("dragged_stacked", K::B),
    ("dragged_text_height", K::Bd),
    ("dragged_border_visible", K::B),
    ("dragged_gap", K::Bd),
    // A BL: older releases leave 32-bit values here when unset.
    ("dragged_border_type", K::Bl),
    ("dragged_background_mask", K::B),
    ("dragged_leader_justification", K::B),
    ("dragged_color", K::Cmc),
    ("dragged_lineweight", K::Bl),
    ("entries", K::Bl),
];

const ENTRY: [K; 10] = [K::Bl, K::Bd, K::Bl, K::Bits9, K::Bl, K::Bl, K::Bl, K::Bl, K::B, K::Bl];

/// What a line component (`AECC_LABEL_RADIAL_LINE_STYLE`) holds after the
/// base fields (LL01 to LL16: one setting of a new line changed per save).
const LINE: [(&str, K); 18] = [
    ("end_anchor_component", K::Bl),
    ("use_end_anchor", K::B),
    ("end_anchor", K::Bl),
    ("start_x_offset", K::Bd),
    ("l4", K::Bd),
    ("start_y_offset", K::Bd),
    ("l6", K::Bd),
    ("end_x_offset", K::Bd),
    ("l8", K::Bd),
    ("end_y_offset", K::Bd),
    ("l10", K::Bd),
    ("fixed_length", K::Bd),
    // A BL: 0 in a line anchored to the label, a value in one whose start
    // is anchored to another component.
    ("l12", K::Bl),
    ("line_angle", K::Bd),
    ("l14", K::Bd),
    ("length_type", K::Bl),
    ("percent_length", K::Bd),
    ("l17", K::Bl),
];

/// What a block component (`AECC_LABEL_BLOCK_STYLE`) holds after the base
/// fields (BB01 to BB10).
const BLOCK: [(&str, K); 6] = [
    ("k0", K::Bl),
    ("block_rotation", K::Bd),
    ("k2", K::Bd),
    ("block_height", K::Bd),
    ("k4", K::Bl),
    ("block_attachment", K::Bl),
];

/// What a tick component (`AECC_LABEL_TICK_STYLE`) holds after the base
/// fields (TA001 to TA003): a block component's fields, then `BL` 1,
/// `BL` 0 and whether it aligns with the labelled object.
const TICK: [(&str, K); 9] = [
    ("k0", K::Bl),
    ("block_rotation", K::Bd),
    ("k2", K::Bd),
    ("block_height", K::Bd),
    ("k4", K::Bl),
    ("block_attachment", K::Bl),
    ("k6", K::Bl),
    ("k7", K::Bl),
    ("align", K::B),
];

/// What a direction arrow component (`AECC_LABEL_VECTOR_ARROW_STYLE`)
/// holds after the base fields (TA008 to TA011).
const ARROW: [(&str, K); 8] = [
    ("r0", K::Bl),
    ("fixed", K::Bl),
    ("arrow_length", K::Bd),
    ("r3", K::Bd),
    ("arrowhead_size", K::Bd),
    ("r5", K::Bd),
    ("arrow_rotation", K::Bd),
    ("r7", K::Bl),
];

const DRAGGED_END: [(&str, K); 2] = [("d_end", K::Bl), ("dragged_max_width", K::Bd)];

const TEXT: [(&str, K); 9] = [
    ("t0", K::Bl),
    ("contents", K::Text),
    ("angle", K::Bd),
    ("t3", K::Bl),
    ("attachment", K::Bl),
    ("t5", K::Bl),
    ("t6", K::B),
    ("max_width", K::Bd),
    ("t8", K::Bd),
];

/// What a text iterator (`AECC_LABEL_TEXT_ITERATOR_STYLE`) adds to a text
/// component's fields: the kind of item it labels each of (0x1907A000 the
/// intersecting roads of an intersection); a curve or spiral iterator
/// (`AECC_LABEL_TEXT_ITERATOR_CURVE_OR_SPIRAL_STYLE`: 0x1900D752) then
/// `BL` 1, `BL` 0 and its spirals' text (its curves' is the contents).
const ITERATOR: [(&str, K); 1] = [("iterates", K::Bl)];
const CURVE_ITERATOR: [(&str, K); 3] = [("i1", K::Bl), ("i2", K::Bl), ("spiral_contents", K::Text)];

// Mask A bits.
const VISIBLE: u32 = 0x1;
const ORIENTATION: u32 = 0x2;
const PLAN_READABLE: u32 = 0x4;
const INSERT: u32 = 0x8;
const LEADER_VISIBLE: u32 = 0x100;
const LEADER_SPLINE: u32 = 0x800;
const ARROW_SIZE: u32 = 0x2000;
const ARROW_BLOCK: u32 = 0x4000;
const TEXT_COLOR: u32 = 0x10_0000;
const LAYER: u32 = 0x20_0000;
const TEXT_LINEWEIGHT: u32 = 0x80_0000;
/// Mask C bits of a line component's start point offsets (LL02, LL03).
const START_X_OFFSET: u32 = 0x1;
const START_Y_OFFSET: u32 = 0x2;
/// Mask C bits of a dragged-state copy's own line length and angle.
const LINE_FIXED_LENGTH: u32 = 0x80;
const LINE_ANGLE: u32 = 0x100;
/// Mask A bit of a style's dragged-state copy of a component.
const DRAGGED_COPY: u32 = 0x20_0000;
/// A line component's own linetype (LL16).
const COMPONENT_LINETYPE: u32 = 0x40_0000;
const LEADER_COLOR: u32 = 0x0800_0000;
const LEADER_LINETYPE: u32 = 0x1000_0000;
const LEADER_LINEWEIGHT: u32 = 0x2000_0000;
// Mask B bits.
const LEADER_ATTACHMENT: u32 = 0x2;
const READABLE_BIAS: u32 = 0x8;
const FLIP_ANCHORS: u32 = 0x10;
// Mask C bits (a collector's dragged state).
const STACKED: u32 = 0x1;
const D_TEXT_HEIGHT: u32 = 0x2;
const D_BORDER_VISIBLE: u32 = 0x4;
const D_GAP: u32 = 0x8;
const D_COLOR: u32 = 0x10;
const D_LINETYPE: u32 = 0x20;
const D_LINEWEIGHT: u32 = 0x40;
const D_JUSTIFICATION: u32 = 0x80;
const D_BORDER_TYPE: u32 = 0x100;
const D_BACKGROUND: u32 = 0x200;
const D_MAX_WIDTH: u32 = 0x400;

// Base reference slots.
const REF_TEXT_STYLE: usize = 1;
pub(super) const REF_OWNER: usize = 2;
const REF_ARROW: usize = 3;
const REF_LAYER: usize = 4;
/// A line component's linetype (ByLayer: LL16).
const REF_LINETYPE: usize = 5;
const REF_BORDER_LINETYPE: usize = 6;
/// A block component's block (its tenth reference).
const REF_BLOCK: usize = 9;
/// A tick component's block (its ninth reference: TB001).
const REF_TICK_BLOCK: usize = 8;
const REF_LEADER_LINETYPE: usize = 7;
const REF_DRAGGED_LINETYPE: usize = 8;
/// References before a collector's text components.
pub(super) const BASE_REFS: usize = 9;

/// The class data fields of a label collector or text component style.
#[derive(Clone)]
pub(super) struct LabelFields(Vec<Field>);

fn bit_string(data: &[u8], from: usize, to: usize) -> String {
    (from..to).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect()
}

fn read_value(b: &mut Bits, k: K) -> Option<V> {
    Some(match k {
        K::B => V::B(b.bit()?),
        K::Bl => V::Bl(b.bit_long()?),
        K::Bd => V::Bd(b.bit_double()?),
        K::Cmc => {
            b.bit_short()?;
            let word = b.bit_long()? as u32;
            b.raw_short_byte()?;
            V::Cmc(word)
        }
        K::Bits9 => V::Bits((0..9).try_fold(0u16, |v, _| Some(v << 1 | u16::from(b.bit()?)))?),
        K::Text => {
            let n = usize::try_from(b.bit_long()?).ok().filter(|n| n % 2 == 0 && n * 8 <= b.remaining())?;
            let units: Vec<u16> = (0..n / 2).map(|_| Some(u16::from_le_bytes([b.bits_value(8)?, b.bits_value(8)?]))).collect::<Option<_>>()?;
            V::Text(String::from_utf16_lossy(&units).trim_end_matches('\0').to_string())
        }
    })
}

impl LabelFields {
    fn read_list(&mut self, data: &[u8], b: &mut Bits, list: &[(&'static str, K)]) -> Option<()> {
        for (name, k) in list {
            let from = b.pos;
            let value = read_value(b, *k)?;
            self.0.push(Field { name, value, raw: bit_string(data, from, b.pos) });
        }
        Some(())
    }

    /// Reads the fields every label style record opens with: [`BASE`],
    /// `BL` n and n `BL` keys (n 0 but in the collectors of pipe and
    /// structure labels in plan, profile and section, which list two:
    /// 0x0405D122 and 0x0405D123 for a pipe, 0x0405D120 and 0x0405D121
    /// for a structure), then `BL` the dragged state's override mask.
    fn read_base(&mut self, data: &[u8], b: &mut Bits) -> Option<()> {
        self.read_list(data, b, &BASE)?;
        self.read_list(data, b, &[("keys", K::Bl)])?;
        let n = usize::try_from(self.long("keys")?).ok().filter(|n| *n * 2 <= b.remaining())?;
        for _ in 0..n {
            self.read_list(data, b, &[("key", K::Bl)])?;
        }
        self.read_list(data, b, &[("mask_c", K::Bl)])
    }

    /// Reads a line or block component's class data (`tail` its fields
    /// after the base ones); `None` unless they end exactly at `end`.
    pub(super) fn read_component(data: &[u8], start: usize, end: usize, line: bool) -> Option<Self> {
        let mut b = Bits::new(data, start, end);
        let mut f = LabelFields(Vec::new());
        f.read_base(data, &mut b)?;
        f.read_list(data, &mut b, if line { &LINE } else { &BLOCK })?;
        (b.pos == end).then_some(f)
    }

    /// Reads a collector's (`collector`) or text component's class data
    /// from `start` (the bit after the component lists); `None` unless the
    /// fields end exactly at `end`.
    pub(super) fn read(data: &[u8], start: usize, end: usize, collector: bool) -> Option<Self> {
        let mut b = Bits::new(data, start, end);
        let mut f = LabelFields(Vec::new());
        f.read_base(data, &mut b)?;
        if collector {
            f.read_list(data, &mut b, &DRAGGED)?;
            let n = usize::try_from(f.long("entries")?).ok().filter(|n| *n * 20 <= b.remaining())?;
            for _ in 0..n {
                let entry: Vec<(&'static str, K)> = ENTRY.iter().map(|k| ("entry", *k)).collect();
                f.read_list(data, &mut b, &entry)?;
            }
            f.read_list(data, &mut b, &DRAGGED_END)?;
        } else {
            f.read_list(data, &mut b, &TEXT)?;
        }
        (b.pos == end).then_some(f)
    }

    /// Reads a text iterator's class data (a curve or spiral iterator's
    /// with `curve`); `None` unless it ends exactly at `end`.
    pub(super) fn read_iterator(data: &[u8], start: usize, end: usize, curve: bool) -> Option<Self> {
        let mut b = Bits::new(data, start, end);
        let mut f = LabelFields(Vec::new());
        f.read_base(data, &mut b)?;
        f.read_list(data, &mut b, &TEXT)?;
        f.read_list(data, &mut b, &ITERATOR)?;
        if curve {
            f.read_list(data, &mut b, &CURVE_ITERATOR)?;
        }
        (b.pos == end).then_some(f)
    }

    /// What a text iterator labels each of, and a curve or spiral
    /// iterator's spirals' text.
    pub(super) fn iterator(&self) -> Option<(u32, Option<String>)> {
        Some((self.long("iterates")? as u32, self.text("spiral_contents").map(str::to_string)))
    }

    /// Writes a curve or spiral iterator's spirals' text.
    pub(super) fn set_spiral_contents(&mut self, text: &str) {
        self.set("spiral_contents", V::Text(text.to_string()));
    }

    /// Writes the fields: each unchanged one as it was read.
    pub(super) fn write(&self, w: &mut crate::io::dwg::dwg_stream_writers::DwgMergedWriter) {
        for f in &self.0 {
            if !f.raw.is_empty() {
                for c in f.raw.bytes() {
                    w.write_bit(c == b'1');
                }
                continue;
            }
            match &f.value {
                V::B(v) => w.write_bit(*v),
                V::Bl(v) => w.write_bit_long(*v),
                V::Bd(v) => w.write_bit_double(*v),
                V::Cmc(word) => {
                    w.write_bit_short(0);
                    w.write_bit_long(*word as i32);
                    w.write_byte(0);
                }
                V::Bits(v) => (0..9).rev().for_each(|i| w.write_bit(v >> i & 1 == 1)),
                V::Text(s) => {
                    let units: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
                    w.write_bit_long(units.len() as i32 * 2);
                    for u in units {
                        w.write_bytes(&u.to_le_bytes());
                    }
                }
            }
        }
    }

    fn get(&self, name: &str) -> Option<&V> {
        self.0.iter().find(|f| f.name == name).map(|f| &f.value)
    }

    fn set(&mut self, name: &str, value: V) {
        if let Some(f) = self.0.iter_mut().find(|f| f.name == name) {
            if f.value != value {
                f.value = value;
                f.raw.clear();
            }
        }
    }

    fn long(&self, name: &str) -> Option<i32> {
        match self.get(name)? {
            V::Bl(v) => Some(*v),
            _ => None,
        }
    }

    fn double(&self, name: &str) -> Option<f64> {
        match self.get(name)? {
            V::Bd(v) => Some(*v),
            _ => None,
        }
    }

    fn flag(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            V::B(v) => Some(*v),
            _ => None,
        }
    }

    fn color(&self, name: &str) -> Option<Color> {
        match self.get(name)? {
            V::Cmc(word) => color_of(*word),
            _ => None,
        }
    }

    fn text(&self, name: &str) -> Option<&str> {
        match self.get(name)? {
            V::Text(s) => Some(s),
            _ => None,
        }
    }

    fn mask(&self, name: &str) -> u32 {
        self.long(name).unwrap_or(0) as u32
    }

    fn set_mask(&mut self, name: &str, bit: u32, on: bool) {
        let m = self.mask(name);
        self.set(name, V::Bl((if on { m | bit } else { m & !bit }) as i32));
    }

    /// The value of an overridable setting: `Some` when its mask bit is set.
    fn when<T>(&self, mask: &str, bit: u32, value: impl FnOnce() -> Option<T>) -> Option<T> {
        (self.mask(mask) & bit != 0).then(value).flatten()
    }

    /// Sets the mask bit for `value` and, when given, the value.
    fn put<T>(&mut self, mask: &str, bit: u32, value: Option<T>, store: impl FnOnce(&mut Self, T)) {
        self.set_mask(mask, bit, value.is_some());
        if let Some(v) = value {
            store(self, v);
        }
    }
}

fn color_of(word: u32) -> Option<Color> {
    let [b, g, r, method] = word.to_le_bytes();
    Some(match method {
        0xC0 => Color::ByLayer,
        0xC1 => Color::ByBlock,
        0xC2 => Color::from_rgb(r, g, b),
        0xC3 => Color::from_index(i16::from(b)),
        0xC8 => Color::None,
        _ => return None,
    })
}

fn word_of(color: Color) -> u32 {
    match color {
        Color::ByLayer => 0xC000_0000,
        Color::ByBlock => 0xC100_0000,
        Color::None => 0xC800_0000,
        Color::Index(i) => 0xC300_0000 | u32::from(i),
        Color::Rgb { r, g, b } => 0xC200_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
    }
}

/// Leader settings of a label style; `None` takes the label style
/// defaults' value.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelLeader {
    pub visible: Option<bool>,
    /// A spline leader (straight when `false`).
    pub spline: Option<bool>,
    pub arrowhead_size: Option<f64>,
    /// Arrowhead block handle.
    pub arrowhead_block: Option<u64>,
    pub color: Option<Color>,
    /// Linetype handle.
    pub linetype: Option<u64>,
    /// Lineweight in 0.01 mm; -1 ByLayer, -2 ByBlock, -3 default.
    pub lineweight: Option<i32>,
}

/// Dragged state settings of a label style (how a label looks once
/// dragged from its anchor); `None` takes the label style defaults'
/// value.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelDraggedState {
    /// Stacked text (as composed when `false`).
    pub stacked: Option<bool>,
    pub text_height: Option<f64>,
    pub max_text_width: Option<f64>,
    pub gap: Option<f64>,
    pub border_visible: Option<bool>,
    /// 0 rectangular, 1 rounded rectangular, 2 circular.
    pub border_type: Option<i16>,
    pub background_mask: Option<bool>,
    pub leader_justification: Option<bool>,
    /// Where the leader meets the text: 0 top of the top line, 1 middle
    /// of the top line, 2 middle of the text, 3 middle of the bottom line,
    /// 4 bottom of the bottom line.
    pub leader_attachment: Option<i32>,
    pub color: Option<Color>,
    /// Linetype handle.
    pub linetype: Option<u64>,
    pub lineweight: Option<i32>,
}

/// General, behavior, plan readability, leader and dragged state settings
/// of a label style (`AECC_LABEL_COLLECTOR_STYLE`). Each `None` takes the
/// label style defaults' value.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelStyleSettings {
    pub visible: Option<bool>,
    /// Layer handle.
    pub layer: Option<u64>,
    /// Text style handle; there is no mask bit for it (a null reference
    /// takes the defaults' text style).
    pub text_style: Option<u64>,
    /// 0 the object, 1 the view, 2 the world coordinate system.
    pub orientation: Option<i32>,
    /// 0 none, 1 top, 2 bottom.
    pub insert: Option<i32>,
    pub plan_readable: Option<bool>,
    /// Readability bias angle in radians.
    pub readable_bias: Option<f64>,
    pub flip_anchors: Option<bool>,
    pub leader: LabelLeader,
    pub dragged: LabelDraggedState,
}

/// The border of a label text component.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelBorder {
    pub visible: bool,
    /// 0 rectangular, 1 rounded rectangular, 2 circular.
    pub border_type: i32,
    pub background_mask: bool,
    pub gap: f64,
    pub color: Color,
    pub lineweight: i32,
    /// Linetype handle.
    pub linetype: u64,
}

/// A label style's line component (`AECC_LABEL_RADIAL_LINE_STYLE`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelLineComponent {
    pub id: u32,
    pub name: Option<String>,
    pub visible: bool,
    /// `None` takes the defaults' color, lineweight, linetype.
    pub color: Option<Color>,
    pub lineweight: Option<i32>,
    pub linetype: Option<u64>,
    /// Ends at its end point anchor rather than at its length.
    pub use_end_anchor: bool,
    /// `None` takes the defaults' start point offsets.
    pub start_x_offset: Option<f64>,
    pub start_y_offset: Option<f64>,
    pub end_x_offset: f64,
    pub end_y_offset: f64,
    /// Angle in radians.
    pub angle: f64,
    /// Length as a percentage (of the anchors' distance) rather than fixed.
    pub percent: bool,
    pub fixed_length: f64,
    pub percent_length: f64,
}

/// A label style's block component (`AECC_LABEL_BLOCK_STYLE`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelBlockComponent {
    pub id: u32,
    pub name: Option<String>,
    pub visible: bool,
    pub color: Option<Color>,
    pub lineweight: Option<i32>,
    pub x_offset: f64,
    pub y_offset: f64,
    /// Rotation in radians.
    pub rotation: f64,
    pub height: f64,
    /// 9 insertion point; 0 top left to 8 bottom right.
    pub attachment: i32,
    /// The block definition (its block record), 0 for none.
    pub block: u64,
}

/// A label style's text component (`AECC_LABEL_TEXT_STYLE`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelTextComponent {
    /// Component id (a property id for the application's components, a
    /// text id for named ones).
    pub id: u32,
    /// Name of a component under a text id.
    pub name: Option<String>,
    /// Contents in the record's form: `<[property id(format codes)]>`
    /// fields within plain text (see [`super::parse_format`]).
    pub contents: String,
    /// Text height; `None` takes the label style defaults' height.
    pub height: Option<f64>,
    pub visible: bool,
    /// Id of the component this one is anchored to, 0 for the labelled
    /// object.
    pub anchor_component: u32,
    /// Name of the anchor component when its id is a text id.
    pub anchor_name: Option<String>,
    /// Anchor point: 0 top left, 1 top center, ... 8 bottom right (the
    /// stored code less 4); other codes are kept as stored less 4.
    pub anchor_point: i32,
    /// Text attachment: 0 top left, 1 top center, ... 8 bottom right.
    pub attachment: i32,
    pub x_offset: f64,
    pub y_offset: f64,
    /// Rotation in radians.
    pub angle: f64,
    pub max_width: f64,
    /// `None` takes the defaults' text color.
    pub text_color: Option<Color>,
    /// `None` takes the defaults' text lineweight.
    pub text_lineweight: Option<i32>,
    pub border: LabelBorder,
}

const ANCHOR_CODE: i32 = 4;

impl LabelFields {
    /// The collector settings, `refs` its references.
    pub(super) fn settings(&self, refs: &[u64]) -> LabelStyleSettings {
        let r = |i: usize| refs.get(i).copied().filter(|h| *h != 0);
        LabelStyleSettings {
            visible: self.when("mask_a", VISIBLE, || self.flag("label_visible")),
            layer: self.when("mask_a", LAYER, || r(REF_LAYER)),
            text_style: r(REF_TEXT_STYLE),
            orientation: self.when("mask_a", ORIENTATION, || self.long("orientation")),
            insert: self.when("mask_a", INSERT, || self.long("insert")),
            plan_readable: self.when("mask_a", PLAN_READABLE, || self.long("plan_readable").map(|v| v != 0)),
            readable_bias: self.when("mask_b", READABLE_BIAS, || self.double("readable_bias")),
            flip_anchors: self.when("mask_b", FLIP_ANCHORS, || self.flag("flip_anchors")),
            leader: LabelLeader {
                visible: self.when("mask_a", LEADER_VISIBLE, || self.flag("leader_visible")),
                spline: self.when("mask_a", LEADER_SPLINE, || self.flag("leader_spline")),
                arrowhead_size: self.when("mask_a", ARROW_SIZE, || self.double("arrow_size")),
                arrowhead_block: self.when("mask_a", ARROW_BLOCK, || r(REF_ARROW)),
                color: self.when("mask_a", LEADER_COLOR, || self.color("leader_color")),
                linetype: self.when("mask_a", LEADER_LINETYPE, || r(REF_LEADER_LINETYPE)),
                lineweight: self.when("mask_a", LEADER_LINEWEIGHT, || self.long("leader_lineweight")),
            },
            dragged: LabelDraggedState {
                stacked: self.when("mask_c", STACKED, || self.flag("dragged_stacked")),
                text_height: self.when("mask_c", D_TEXT_HEIGHT, || self.double("dragged_text_height")),
                max_text_width: self.when("mask_c", D_MAX_WIDTH, || self.double("dragged_max_width")),
                gap: self.when("mask_c", D_GAP, || self.double("dragged_gap")),
                border_visible: self.when("mask_c", D_BORDER_VISIBLE, || self.flag("dragged_border_visible")),
                border_type: self.when("mask_c", D_BORDER_TYPE, || self.long("dragged_border_type").map(|v| v as i16)),
                background_mask: self.when("mask_c", D_BACKGROUND, || self.flag("dragged_background_mask")),
                leader_justification: self.when("mask_c", D_JUSTIFICATION, || self.flag("dragged_leader_justification")),
                leader_attachment: self.when("mask_b", LEADER_ATTACHMENT, || self.long("leader_attachment")),
                color: self.when("mask_c", D_COLOR, || self.color("dragged_color")),
                linetype: self.when("mask_c", D_LINETYPE, || r(REF_DRAGGED_LINETYPE)),
                lineweight: self.when("mask_c", D_LINEWEIGHT, || self.long("dragged_lineweight")),
            },
        }
    }

    /// Writes `s` into a collector's fields and references.
    pub(super) fn set_settings(&mut self, s: &LabelStyleSettings, refs: &mut [u64]) {
        let mut r = |i: usize, v: Option<u64>| {
            if let Some(slot) = refs.get_mut(i) {
                *slot = v.unwrap_or(0);
            }
        };
        r(REF_TEXT_STYLE, s.text_style);
        r(REF_LAYER, s.layer);
        r(REF_ARROW, s.leader.arrowhead_block);
        r(REF_LEADER_LINETYPE, s.leader.linetype);
        r(REF_DRAGGED_LINETYPE, s.dragged.linetype);
        self.put("mask_a", VISIBLE, s.visible, |f, v| f.set("label_visible", V::B(v)));
        self.set_mask("mask_a", LAYER, s.layer.is_some());
        self.put("mask_a", ORIENTATION, s.orientation, |f, v| f.set("orientation", V::Bl(v)));
        self.put("mask_a", INSERT, s.insert, |f, v| f.set("insert", V::Bl(v)));
        self.put("mask_a", PLAN_READABLE, s.plan_readable, |f, v| f.set("plan_readable", V::Bl(i32::from(v))));
        self.put("mask_b", READABLE_BIAS, s.readable_bias, |f, v| f.set("readable_bias", V::Bd(v)));
        self.put("mask_b", FLIP_ANCHORS, s.flip_anchors, |f, v| f.set("flip_anchors", V::B(v)));
        let l = &s.leader;
        self.put("mask_a", LEADER_VISIBLE, l.visible, |f, v| f.set("leader_visible", V::B(v)));
        self.put("mask_a", LEADER_SPLINE, l.spline, |f, v| f.set("leader_spline", V::B(v)));
        self.put("mask_a", ARROW_SIZE, l.arrowhead_size, |f, v| f.set("arrow_size", V::Bd(v)));
        self.set_mask("mask_a", ARROW_BLOCK, l.arrowhead_block.is_some());
        self.put("mask_a", LEADER_COLOR, l.color, |f, v| f.set("leader_color", V::Cmc(word_of(v))));
        self.set_mask("mask_a", LEADER_LINETYPE, l.linetype.is_some());
        self.put("mask_a", LEADER_LINEWEIGHT, l.lineweight, |f, v| f.set("leader_lineweight", V::Bl(v)));
        let d = &s.dragged;
        self.put("mask_c", STACKED, d.stacked, |f, v| f.set("dragged_stacked", V::B(v)));
        self.put("mask_c", D_TEXT_HEIGHT, d.text_height, |f, v| f.set("dragged_text_height", V::Bd(v)));
        self.put("mask_c", D_MAX_WIDTH, d.max_text_width, |f, v| f.set("dragged_max_width", V::Bd(v)));
        self.put("mask_c", D_GAP, d.gap, |f, v| f.set("dragged_gap", V::Bd(v)));
        self.put("mask_c", D_BORDER_VISIBLE, d.border_visible, |f, v| f.set("dragged_border_visible", V::B(v)));
        self.put("mask_c", D_BORDER_TYPE, d.border_type, |f, v| f.set("dragged_border_type", V::Bl(i32::from(v))));
        self.put("mask_c", D_BACKGROUND, d.background_mask, |f, v| f.set("dragged_background_mask", V::B(v)));
        self.put("mask_c", D_JUSTIFICATION, d.leader_justification, |f, v| f.set("dragged_leader_justification", V::B(v)));
        self.put("mask_b", LEADER_ATTACHMENT, d.leader_attachment, |f, v| f.set("leader_attachment", V::Bl(v)));
        self.put("mask_c", D_COLOR, d.color, |f, v| f.set("dragged_color", V::Cmc(word_of(v))));
        self.set_mask("mask_c", D_LINETYPE, d.linetype.is_some());
        self.put("mask_c", D_LINEWEIGHT, d.lineweight, |f, v| f.set("dragged_lineweight", V::Bl(v)));
    }

    /// The text component, given its id, name, references and the anchor
    /// component's name.
    pub(super) fn text_component(&self, id: u32, name: Option<String>, refs: &[u64], anchor_name: Option<String>) -> Option<LabelTextComponent> {
        Some(LabelTextComponent {
            id,
            name,
            contents: self.text("contents")?.to_string(),
            height: (self.long("height_set")? != 0).then(|| self.double("height")).flatten(),
            visible: self.flag("visible")?,
            anchor_component: self.long("anchor_component")? as u32,
            anchor_name,
            anchor_point: (self.long("anchor")? & 0xFF) - ANCHOR_CODE,
            attachment: self.long("attachment")?,
            x_offset: self.double("x_offset")?,
            y_offset: self.double("y_offset")?,
            angle: self.double("angle")?,
            max_width: self.double("max_width")?,
            text_color: self.when("mask_a", TEXT_COLOR, || self.color("text_color")),
            text_lineweight: self.when("mask_a", TEXT_LINEWEIGHT, || self.long("text_lineweight")),
            border: LabelBorder {
                visible: self.flag("border_visible")?,
                border_type: self.long("border_type")?,
                background_mask: self.flag("background_mask")?,
                gap: self.double("gap")?,
                color: self.color("border_color")?,
                lineweight: self.long("border_lineweight")?,
                linetype: refs.get(REF_BORDER_LINETYPE).copied().unwrap_or(0),
            },
        })
    }

    /// The line component, given its id, name and references.
    pub(super) fn line_component(&self, id: u32, name: Option<String>, refs: &[u64]) -> Option<LabelLineComponent> {
        Some(LabelLineComponent {
            id,
            name,
            visible: self.flag("visible")?,
            color: self.when("mask_a", TEXT_COLOR, || self.color("text_color")),
            lineweight: self.when("mask_a", TEXT_LINEWEIGHT, || self.long("text_lineweight")),
            linetype: self.when("mask_a", COMPONENT_LINETYPE, || refs.get(REF_LINETYPE).copied()),
            use_end_anchor: self.flag("use_end_anchor")?,
            start_x_offset: self.when("mask_c", START_X_OFFSET, || self.double("start_x_offset")),
            start_y_offset: self.when("mask_c", START_Y_OFFSET, || self.double("start_y_offset")),
            end_x_offset: self.double("end_x_offset")?,
            end_y_offset: self.double("end_y_offset")?,
            angle: self.double("line_angle")?,
            percent: self.long("length_type")? == 1,
            fixed_length: self.double("fixed_length")?,
            percent_length: self.double("percent_length")?,
        })
    }

    /// A line component's fixed length and angle as shown: `None` for a
    /// style's dragged-state copy (mask A bit [`DRAGGED_COPY`]) that does
    /// not hold its own length (mask C 0x80) or angle (0x100), whose shown
    /// values are those of the component it copies.
    pub(super) fn line_shown(&self) -> Option<(f64, f64)> {
        let own = self.mask("mask_a") & DRAGGED_COPY == 0 || self.mask("mask_c") & (LINE_FIXED_LENGTH | LINE_ANGLE) == LINE_FIXED_LENGTH | LINE_ANGLE;
        own.then(|| Some((self.double("fixed_length")?, self.double("line_angle")?))).flatten()
    }

    /// Writes a line component's values into its fields and references.
    pub(super) fn set_line_component(&mut self, c: &LabelLineComponent, refs: &mut [u64]) {
        self.set("visible", V::B(c.visible));
        self.put("mask_a", TEXT_COLOR, c.color, |f, v| f.set("text_color", V::Cmc(word_of(v))));
        self.put("mask_a", TEXT_LINEWEIGHT, c.lineweight, |f, v| f.set("text_lineweight", V::Bl(v)));
        self.put("mask_a", COMPONENT_LINETYPE, c.linetype, |_, v| if let Some(slot) = refs.get_mut(REF_LINETYPE) { *slot = v });
        self.set("use_end_anchor", V::B(c.use_end_anchor));
        self.put("mask_c", START_X_OFFSET, c.start_x_offset, |f, v| f.set("start_x_offset", V::Bd(v)));
        self.put("mask_c", START_Y_OFFSET, c.start_y_offset, |f, v| f.set("start_y_offset", V::Bd(v)));
        self.set("end_x_offset", V::Bd(c.end_x_offset));
        self.set("end_y_offset", V::Bd(c.end_y_offset));
        self.set("line_angle", V::Bd(c.angle));
        self.set("length_type", V::Bl(i32::from(c.percent)));
        self.set("fixed_length", V::Bd(c.fixed_length));
        self.set("percent_length", V::Bd(c.percent_length));
    }

    /// The block component, given its id, name and references.
    pub(super) fn block_component(&self, id: u32, name: Option<String>, refs: &[u64]) -> Option<LabelBlockComponent> {
        Some(LabelBlockComponent {
            id,
            name,
            visible: self.flag("visible")?,
            color: self.when("mask_a", TEXT_COLOR, || self.color("text_color")),
            lineweight: self.when("mask_a", TEXT_LINEWEIGHT, || self.long("text_lineweight")),
            x_offset: self.double("x_offset")?,
            y_offset: self.double("y_offset")?,
            rotation: self.double("block_rotation")?,
            height: self.double("block_height")?,
            attachment: self.long("block_attachment")?,
            block: refs.get(REF_BLOCK).copied().unwrap_or(0),
        })
    }

    /// Writes a block component's values into its fields and references.
    pub(super) fn set_block_component(&mut self, c: &LabelBlockComponent, refs: &mut [u64]) {
        self.set("visible", V::B(c.visible));
        self.put("mask_a", TEXT_COLOR, c.color, |f, v| f.set("text_color", V::Cmc(word_of(v))));
        self.put("mask_a", TEXT_LINEWEIGHT, c.lineweight, |f, v| f.set("text_lineweight", V::Bl(v)));
        self.set("x_offset", V::Bd(c.x_offset));
        self.set("y_offset", V::Bd(c.y_offset));
        self.set("block_rotation", V::Bd(c.rotation));
        self.set("block_height", V::Bd(c.height));
        self.set("block_attachment", V::Bl(c.attachment));
        if let Some(slot) = refs.get_mut(REF_BLOCK) {
            *slot = c.block;
        }
    }

    /// Sets a collector's per component entry list to `n` entries: the
    /// entries are the same for every component apart from their number
    /// (1, 2, ...; LC00 to LC05: a component added or removed changes only
    /// their count), so the first one is repeated (a constant one when there
    /// is none).
    pub(super) fn set_entries(&mut self, n: usize) {
        let Some(at) = self.0.iter().position(|f| f.name == "entries") else { return };
        let old = self.0[at + 1..].iter().take_while(|f| f.name == "entry").count();
        let first: Vec<Field> = self.0[at + 1..at + 1 + old.min(ENTRY.len())].to_vec();
        let proto: Vec<Field> = if first.len() == ENTRY.len() {
            first
        } else {
            let values = [V::Bl(4), V::Bd(0.0), V::Bl(0), V::Bits(0), V::Bl(0xFF_FFFF), V::Bl(0), V::Bl(0), V::Bl(0), V::B(false), V::Bl(1)];
            values.into_iter().map(|value| Field { name: "entry", value, raw: String::new() }).collect()
        };
        let entries = (0..n).flat_map(|i| {
            let mut e = proto.clone();
            let last = e.len() - 1;
            e[last] = Field { name: "entry", value: V::Bl(i as i32 + 1), raw: String::new() };
            e
        });
        self.0.splice(at + 1..at + 1 + old, entries);
        self.set("entries", V::Bl(n as i32));
        self.set("d_end", V::Bl(n as i32));
    }

    /// The number of a collector's per component entries.
    #[cfg(test)]
    pub(super) fn entry_count(&self) -> i32 {
        self.long("entries").unwrap_or(-1)
    }

    /// Makes a text component's fields those of a component the label
    /// style editor adds (LC01): field `t5` 5 and the anchor word's kind
    /// byte 0xB2 for the labelled object's location (`anchor_point` -4),
    /// 0xB1 for an anchor point.
    pub(super) fn set_new_text_component(&mut self, c: &LabelTextComponent) {
        self.set("t5", V::Bl(5));
        let kind = if c.anchor_point < 0 { 0x0800_B200 } else { 0x0800_B100 };
        self.set("anchor", V::Bl(kind));
    }

    /// Writes a text component's values (its id and name are the record's
    /// head and texts) into its fields and references.
    pub(super) fn set_text_component(&mut self, c: &LabelTextComponent, refs: &mut [u64]) {
        self.set("contents", V::Text(c.contents.clone()));
        self.set("height_set", V::Bl(i32::from(c.height.is_some())));
        if let Some(h) = c.height {
            self.set("height", V::Bd(h));
        }
        self.set("visible", V::B(c.visible));
        self.set("anchor_component", V::Bl(c.anchor_component as i32));
        let anchor = self.long("anchor").unwrap_or(0);
        self.set("anchor", V::Bl(anchor & !0xFF | (c.anchor_point + ANCHOR_CODE) & 0xFF));
        self.set("attachment", V::Bl(c.attachment));
        self.set("x_offset", V::Bd(c.x_offset));
        self.set("y_offset", V::Bd(c.y_offset));
        self.set("angle", V::Bd(c.angle));
        self.set("max_width", V::Bd(c.max_width));
        self.put("mask_a", TEXT_COLOR, c.text_color, |f, v| f.set("text_color", V::Cmc(word_of(v))));
        self.put("mask_a", TEXT_LINEWEIGHT, c.text_lineweight, |f, v| f.set("text_lineweight", V::Bl(v)));
        let b = &c.border;
        self.set("border_visible", V::B(b.visible));
        self.set("border_type", V::Bl(b.border_type));
        self.set("background_mask", V::B(b.background_mask));
        self.set("gap", V::Bd(b.gap));
        self.set("border_color", V::Cmc(word_of(b.color)));
        self.set("border_lineweight", V::Bl(b.lineweight));
        if let Some(slot) = refs.get_mut(REF_BORDER_LINETYPE) {
            *slot = b.linetype;
        }
    }
}

/// The class data of a line (LC04), a block (LC05), a tick and a
/// direction arrow (TA000) component as the label style editor adds them.
const NEW_TICK: (usize, &str) = (557, "a63fffffc002080010749692aa40e954044444444444584fc28000000c0003fffffffe00000030000ffffffffa00000030000ffffffff40a91ba8111111111111813f9094068");
const NEW_ARROW: (usize, &str) = (636, "a63fffffc00e0800d0749692002c80022a406954044444444444584fc28000000c0003fffffffe00000030000ffffffffa00000030000ffffffff40a91fa26a666666666664fe044444444444604fea");
const NEW_LINE: (usize, &str) = (647, "a63fffffc00e0800d0749692002c80022a406954044444444444584fc28000000c0003fffffffe00000030000ffffffffa00000030000ffffffff40a8fc07000080258800455550aaaaaaaaaaab6a7f54c");
const NEW_BLOCK: (usize, &str) = (576, "a63fffffc00e0800d0749692002c80022a406954044444444444584fc28000000c0003fffffffe00000030000ffffffffa00000030000ffffffff40a91ba8111111111111813f909");

fn bytes(hex: &str) -> Vec<u8> {
    let hex = if hex.len() % 2 == 1 { format!("{hex}0") } else { hex.to_string() };
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0)).collect()
}

impl LabelFields {
    /// The class data of a new line (`line`) or block component.
    pub(super) fn new_component(line: bool) -> Option<Self> {
        let (len, hex) = if line { NEW_LINE } else { NEW_BLOCK };
        Self::read_component(&bytes(hex), 0, len, line)
    }

    /// The class data of a new tick (`tick`) or direction arrow component.
    pub(super) fn new_mark(tick: bool) -> Option<Self> {
        let (len, hex) = if tick { NEW_TICK } else { NEW_ARROW };
        Self::read_mark(&bytes(hex), 0, len, tick)
    }

    /// Reads a tick (`tick`) or direction arrow component's class data;
    /// `None` unless it ends exactly at `end`.
    pub(super) fn read_mark(data: &[u8], start: usize, end: usize, tick: bool) -> Option<Self> {
        let mut b = Bits::new(data, start, end);
        let mut f = LabelFields(Vec::new());
        f.read_base(data, &mut b)?;
        f.read_list(data, &mut b, if tick { &TICK } else { &ARROW })?;
        (b.pos == end).then_some(f)
    }

    /// The tick component, given its id, name and references.
    pub(super) fn tick_component(&self, id: u32, name: Option<String>, refs: &[u64]) -> Option<LabelTickComponent> {
        Some(LabelTickComponent {
            id,
            name,
            visible: self.flag("visible")?,
            color: self.when("mask_a", TEXT_COLOR, || self.color("text_color")),
            lineweight: self.when("mask_a", TEXT_LINEWEIGHT, || self.long("text_lineweight")),
            rotation: self.double("block_rotation")?,
            height: self.double("block_height")?,
            align_with_object: self.flag("align")?,
            block: refs.get(REF_TICK_BLOCK).copied().unwrap_or(0),
        })
    }

    /// Writes a tick component's values into its fields and references.
    pub(super) fn set_tick_component(&mut self, c: &LabelTickComponent, refs: &mut [u64]) {
        self.set("visible", V::B(c.visible));
        self.put("mask_a", TEXT_COLOR, c.color, |f, v| f.set("text_color", V::Cmc(word_of(v))));
        self.put("mask_a", TEXT_LINEWEIGHT, c.lineweight, |f, v| f.set("text_lineweight", V::Bl(v)));
        self.set("block_rotation", V::Bd(c.rotation));
        self.set("block_height", V::Bd(c.height));
        self.set("align", V::B(c.align_with_object));
        if let Some(slot) = refs.get_mut(REF_TICK_BLOCK) {
            *slot = c.block;
        }
    }

    /// The direction arrow component, given its id and name.
    pub(super) fn arrow_component(&self, id: u32, name: Option<String>) -> Option<LabelArrowComponent> {
        Some(LabelArrowComponent {
            id,
            name,
            visible: self.flag("visible")?,
            color: self.when("mask_a", TEXT_COLOR, || self.color("text_color")),
            lineweight: self.when("mask_a", TEXT_LINEWEIGHT, || self.long("text_lineweight")),
            x_offset: self.double("x_offset")?,
            y_offset: self.double("y_offset")?,
            fixed_length: self.long("fixed")? == 1,
            length: self.double("arrow_length")?,
            arrowhead_size: self.double("arrowhead_size")?,
            rotation: self.double("arrow_rotation")?,
        })
    }

    /// Writes a direction arrow component's values into its fields.
    pub(super) fn set_arrow_component(&mut self, c: &LabelArrowComponent) {
        self.set("visible", V::B(c.visible));
        self.put("mask_a", TEXT_COLOR, c.color, |f, v| f.set("text_color", V::Cmc(word_of(v))));
        self.put("mask_a", TEXT_LINEWEIGHT, c.lineweight, |f, v| f.set("text_lineweight", V::Bl(v)));
        self.set("x_offset", V::Bd(c.x_offset));
        self.set("y_offset", V::Bd(c.y_offset));
        self.set("fixed", V::Bl(i32::from(c.fixed_length)));
        self.set("arrow_length", V::Bd(c.length));
        self.set("arrowhead_size", V::Bd(c.arrowhead_size));
        self.set("arrow_rotation", V::Bd(c.rotation));
    }
}

/// A label style's text iterator component: a text component labelling
/// each of a kind of item.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelIteratorComponent {
    /// Its text settings (`contents` a curve or spiral iterator's text for
    /// curves).
    pub text: LabelTextComponent,
    /// What it labels each of: 0x1900D752 curves and spirals, 0x1907A000
    /// intersecting roads.
    pub iterates: u32,
    /// A curve or spiral iterator's text for spirals.
    pub spiral_contents: Option<String>,
}

/// A label style's tick component (`AECC_LABEL_TICK_STYLE`: alignment and
/// profile station labels).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelTickComponent {
    pub id: u32,
    pub name: Option<String>,
    pub visible: bool,
    pub color: Option<Color>,
    pub lineweight: Option<i32>,
    /// Rotation in radians.
    pub rotation: f64,
    pub height: f64,
    pub align_with_object: bool,
    /// The tick's block (its block record), 0 for the default tick.
    pub block: u64,
}

/// A label style's direction arrow component
/// (`AECC_LABEL_VECTOR_ARROW_STYLE`: line labels).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelArrowComponent {
    pub id: u32,
    pub name: Option<String>,
    pub visible: bool,
    pub color: Option<Color>,
    pub lineweight: Option<i32>,
    pub x_offset: f64,
    pub y_offset: f64,
    /// A fixed length rather than a minimum one.
    pub fixed_length: bool,
    pub length: f64,
    pub arrowhead_size: f64,
    /// Rotation in radians.
    pub rotation: f64,
}

#[cfg(test)]
mod tests {

    /// A new line and block component read with their defaults, and a
    /// line's start offset needs its mask C bit.
    #[test]
    fn line_and_block_components_read() {
        let line = LabelFields::read_component(&bytes(NEW_LINE.1), 0, NEW_LINE.0, true).unwrap();
        let l = line.line_component(1, None, &[0; 9]).unwrap();
        assert_eq!((l.fixed_length, l.percent_length, l.percent, l.start_x_offset), (1.0 / 12.0, 1.0, false, None));
        let mut edited = line.clone();
        let mut changed = l.clone();
        changed.start_x_offset = Some(0.01);
        changed.percent = true;
        edited.set_line_component(&changed, &mut [0; 9]);
        assert_eq!(edited.line_component(1, None, &[0; 9]).unwrap().start_x_offset, Some(0.01));
        assert_eq!(edited.long("mask_c"), Some(2045));
        let block = LabelFields::read_component(&bytes(NEW_BLOCK.1), 0, NEW_BLOCK.0, false).unwrap();
        let b = block.block_component(2, None, &[0; 10]).unwrap();
        assert_eq!((b.height, b.attachment, b.rotation), (1.0 / 120.0, 9, 0.0));
    }

    /// The class data of the default pipe label style (plan and profile).
    const PIPE: (usize, &str) = (747, "a60eed0003ce0800d0749692002c80022a406954044444444444584fc28000000c0003fffffffe00000030000ffffffffa00000030000ffffffff40a40822d1050408f441412854400000060001fffffffe80a094003fffffc02a20280c0");

    /// A pipe label style lists its two keys before the dragged state's
    /// mask and reads to its end.
    #[test]
    fn pipe_label_style_reads_its_keys() {
        let f = LabelFields::read(&bytes(PIPE.1), 0, PIPE.0, true).unwrap();
        let keys: Vec<i32> = f.0.iter().filter(|x| x.name == "key").filter_map(|x| if let V::Bl(v) = x.value { Some(v) } else { None }).collect();
        assert_eq!((f.long("keys"), keys, f.long("mask_c"), f.entry_count()), (Some(2), vec![0x0405_D122, 0x0405_D123], Some(0), 1));
    }

    use super::*;

    #[test]
    fn color_words_round_trip() {
        for c in [Color::ByLayer, Color::ByBlock, Color::from_index(1), Color::from_rgb(10, 20, 30)] {
            assert_eq!(color_of(word_of(c)), Some(c));
        }
        assert_eq!(word_of(Color::from_index(3)), 0xC300_0003);
        // Masks: a value is reported only with its bit set.
        let mut f = LabelFields(vec![Field { name: "mask_a", value: V::Bl(0), raw: String::new() }, Field { name: "orientation", value: V::Bl(0), raw: "10".into() }]);
        assert_eq!(f.when("mask_a", ORIENTATION, || f.long("orientation")), None);
        f.put("mask_a", ORIENTATION, Some(2), |f, v| f.set("orientation", V::Bl(v)));
        assert_eq!((f.mask("mask_a"), f.long("orientation"), f.0[1].raw.is_empty()), (ORIENTATION, Some(2), true));
    }
}
