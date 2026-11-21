//! Grading criteria (`AECC_GRADING_CRITERIA`): the target and the
//! projection a grading uses, kept as keys after a style-like head:
//!
//! ```text
//! BS 0x04BE, BL id, BL n, n x BL, BL m, m x BL, BL 0, B 1,
//! RC count, count x key, BL 32, ...
//! ```
//!
//! Grading entities (`AECC_GRADING`) carry no style, criteria, or site
//! assignment of their own: their class stream opens with a constant
//! schema head and their body holds references, not assignments — see
//! [`GradingEntity`].
//!
//! A key is the class key of [`super::alignment::read_key`] with two flag
//! bits: 0x00400000 locks the value, and with 0x40000000 the key carries no
//! value (a locked value the criteria does not override). A value the
//! criteria does not override has no key at all; the keys come in no fixed
//! order. The first id list holds the ids of the criteria derived from this
//! one. The references run: the two lists' entries, then one per key that
//! takes a reference (types 0x0F, 0x10, 0x12), then one more. Derived
//! criteria store the target and the projection keys with the lock bit set.
//!
//! Key ids (low 16 bits):
//!
//! ```text
//! 0x3001 target         0x3020 elevation projection  0x3060 interior corner overlap
//! 0x3002 elevation      0x3021 relative elevation p. 0x3090 search order
//! 0x3003 relative elev. 0x3022 distance projection   0x3100 / 0x3102 cut slope format / value
//! 0x3004 distance       0x3023 surface projection    0x3110 / 0x3112 fill slope format / value
//! 0x3140 projection relative elevation               0x3120 / 0x3122 slope format / value
//! 0x3150 projection distance                         0x101E / 0x101F created / modified (FILETIME)
//! 0x3008 target surface (a reference)
//! ```

use super::alignment::{read_key, KeyValue};
use super::bits::Bits;

const LOCKED: u32 = 0x0040_0000;
const NO_VALUE: u32 = 0x4000_0000;

/// One criteria value: the value the criteria stores (`None` when it takes
/// the default) and whether it is locked.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CriteriaValue<T> {
    pub value: Option<T>,
    pub locked: bool,
}

/// What a grading projects to and how.
///
/// Enumerations as stored: target 0 surface, 1 elevation, 2 relative
/// elevation, 3 distance; slope format 0 slope, 1 grade; elevation and
/// surface projection 0 cut/fill slope, 1 cut slope, 2 fill slope; distance
/// projection 3 slope, 5 relative elevation; relative elevation projection
/// 3 slope, 6 distance; interior corner overlap 0 average slope, 1 hold
/// slope as minimum, 2 as maximum; search order 0 cut first, 1 fill first.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradingCriteria {
    pub target: CriteriaValue<i32>,
    pub elevation: CriteriaValue<f64>,
    pub relative_elevation: CriteriaValue<f64>,
    pub distance: CriteriaValue<f64>,
    pub projection_distance: CriteriaValue<f64>,
    pub projection_relative_elevation: CriteriaValue<f64>,
    pub elevation_projection: CriteriaValue<i32>,
    pub relative_elevation_projection: CriteriaValue<i32>,
    pub distance_projection: CriteriaValue<i32>,
    pub surface_projection: CriteriaValue<i32>,
    pub slope: CriteriaValue<f64>,
    pub slope_format: CriteriaValue<i32>,
    pub cut_slope: CriteriaValue<f64>,
    pub cut_slope_format: CriteriaValue<i32>,
    pub fill_slope: CriteriaValue<f64>,
    pub fill_slope_format: CriteriaValue<i32>,
    pub interior_corner_overlap: CriteriaValue<i32>,
    pub search_order: CriteriaValue<i32>,
    /// FILETIME stamps.
    pub created: Option<u64>,
    pub modified: Option<u64>,
    /// Ids of the criteria derived from this one.
    pub derived: Vec<i32>,
    /// Handles of the criteria derived from this one.
    pub derived_criteria: Vec<u64>,
    /// Handle of the surface a criteria targeting a surface grades to.
    pub target_surface: Option<u64>,
}

// ponytail: keys 0x31x1 (a copy of each slope), 0x31x3, 0x3024, 0x3130,
// 0x1008 and 0x1010 are read past; no criteria property changed them.
pub(super) fn criteria(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Option<GradingCriteria> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    let list = |bits: &mut Bits| {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
        (0..n).map(|_| bits.bit_long()).collect::<Option<Vec<_>>>()
    };
    let derived = list(&mut bits)?;
    // References: the two lists' entries, then one per key taking one.
    let mut refs = derived.len() + list(&mut bits)?.len();
    (bits.bit_long()? == 0 && bits.bit()?).then_some(())?;
    let derived_criteria = handles.iter().take(derived.len()).copied().collect();
    let mut out = GradingCriteria { derived, derived_criteria, ..Default::default() };
    for _ in 0..bits.bits_value(8)? {
        let at = bits.pos;
        let key = bits.bit_long()? as u32;
        let value = if key & NO_VALUE != 0 {
            KeyValue::None
        } else {
            bits.pos = at;
            read_key(&mut bits)?.1
        };
        let locked = key & LOCKED != 0;
        let takes_reference = matches!((key >> 24) & 0x3F, 0x0F | 0x10 | 0x12);
        let f = CriteriaValue { value: if let KeyValue::Double(d) = value { Some(d) } else { None }, locked };
        let i = CriteriaValue { value: if let KeyValue::Long(l) = value { Some(l) } else { None }, locked };
        let stamp = if let KeyValue::Raw(r) = value { Some(r) } else { None };
        match key & 0xFFFF {
            0x3001 => out.target = i,
            0x3002 => out.elevation = f,
            0x3003 => out.relative_elevation = f,
            0x3004 => out.distance = f,
            0x3150 => out.projection_distance = f,
            0x3140 => out.projection_relative_elevation = f,
            0x3020 => out.elevation_projection = i,
            0x3021 => out.relative_elevation_projection = i,
            0x3022 => out.distance_projection = i,
            0x3023 => out.surface_projection = i,
            0x3122 => out.slope = f,
            0x3120 => out.slope_format = i,
            0x3102 => out.cut_slope = f,
            0x3100 => out.cut_slope_format = i,
            0x3112 => out.fill_slope = f,
            0x3110 => out.fill_slope_format = i,
            0x3060 => out.interior_corner_overlap = i,
            0x3090 => out.search_order = i,
            0x101E => out.created = stamp,
            0x101F => out.modified = stamp,
            0x3008 if takes_reference => out.target_surface = handles.get(refs).copied().filter(|h| *h != 0),
            _ => {}
        }
        refs += usize::from(takes_reference);
    }
    Some(out)
}

/// The references a grading entity (`AECC_GRADING`) record carries.
///
/// A grading's style and criteria assignment live with its group, not
/// here: Grading-3A.dwg holds the Ditch and the Cut Slope Grading styles
/// under byte-identical heads and references. `AECC_GRADING` also stores
/// no geometry — the record's graphic-image section draws the grading
/// solution.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradingEntity {
    /// Handle of the `AECC_PARCEL` the grading grades from. The two
    /// gradings of DE_RE2012_Vorentwurf_5000_2027.dwg point at its two
    /// parcels; drawings with a single parcel point at it from every
    /// grading.
    pub parcel: u64,
    /// Handle of the `AECC_LABEL_COLLECTOR_STYLE` the record hard-points
    /// at — in the samples the label style of the parcels of the site
    /// the parcel belongs to (its name strings style `AeccDbFace`),
    /// constant per drawing.
    pub label_style: u64,
}

/// Reads a grading entity record: `data`/`start`/`end` is its class
/// stream, `handles` its class-body references in stream order.
///
/// The class stream opens with a constant schema head, identical in all
/// 26 observed records (15 Sample drawings and Grading-3A.dwg): the six
/// words below, the lot lines' schema word (see
/// [`super::parcel_segments::SCHEMA`]), and three more words. The two
/// references follow the common ones; everything after them (an ordinal
/// of the grading within its group and byte runs that match no oracle
/// quantity) is unmapped and read past.
pub(super) fn entity(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Option<GradingEntity> {
    let mut bits = Bits::new(data, start, end);
    for word in [80u16, 256, 0xC76A, 80, 80, 0] {
        (bits.bit_short()? as u16 == word).then_some(())?;
    }
    (bits.bit_long()? as u32 == super::parcel_segments::SCHEMA).then_some(())?;
    for word in [80u16, 0x1540, 0xC1C0] {
        (bits.bit_short()? as u16 == word).then_some(())?;
    }
    let parcel = handles.get(1).copied().filter(|handle| *handle != 0)?;
    let label_style = handles.get(4).copied().filter(|handle| *handle != 0)?;
    Some(GradingEntity { parcel, label_style })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    /// The grading entity records' schema head as bits, MSB first: the
    /// six leading words, the lot lines' schema word, three more words.
    fn entity_head() -> String {
        let mut bits = String::new();
        let bs = |v: u16, bits: &mut String| match v {
            0 => bits.push_str("10"),
            256 => bits.push_str("11"),
            v if v < 256 => bits.push_str(&format!("01{v:08b}")),
            v => {
                bits.push_str("00");
                for b in v.to_le_bytes() {
                    bits.push_str(&format!("{b:08b}"));
                }
            }
        };
        for v in [80u16, 256, 0xC76A, 80, 80, 0] {
            bs(v, &mut bits);
        }
        bits.push_str("00");
        for b in super::super::parcel_segments::SCHEMA.to_le_bytes() {
            bits.push_str(&format!("{b:08b}"));
        }
        for v in [80u16, 0x1540, 0xC1C0] {
            bs(v, &mut bits);
        }
        bits
    }

    #[test]
    fn reads_values_locks_and_valueless_keys() {
        // BS 0x04BE, BL 0, no lists, BL 0, B 1, RC 3:
        // 0x02033004 BD 1.0 (distance), 0x0D433001 BL 3 (target, locked),
        // 0x4D433120 (slope format, locked, no value).
        let b = bits_of(
            "00 10111110 00000100 10 10 10 10 1 00000011
             00 00000100 00110000 00000011 00000010 01
             00 00000001 00110000 01000011 00001101 0100000011
             00 00100000 00110001 01000011 01001101 0000000",
        );
        let read = criteria(&b, 0, b.len() * 8, &[]).unwrap();
        assert_eq!(read.distance, CriteriaValue { value: Some(1.0), locked: false });
        assert_eq!(read.target, CriteriaValue { value: Some(3), locked: true });
        assert_eq!(read.slope_format, CriteriaValue { value: None, locked: true });
        assert_eq!(read.slope, CriteriaValue::default());
    }

    #[test]
    fn reads_a_grading_entitys_references() {
        // The head, then an unmapped tail; references as the handle
        // stream holds them: null, parcel, null, null, style, null.
        let mut bits = entity_head();
        bits.push_str("00000000"); // the unmapped body the samples carry
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        let read = entity(&data, 0, data.len() * 8, &[0, 0xc68, 0, 0, 0x63d, 0]).unwrap();
        assert_eq!(read.parcel, 0xc68);
        assert_eq!(read.label_style, 0x63d);
    }

    #[test]
    fn another_schema_word_is_not_a_grading() {
        let mut bits = entity_head();
        // flip the schema word's first bit: 0x10025F04 becomes 0x10025F84
        assert_eq!(&bits[54..55], "0");
        bits.replace_range(54..55, "1");
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        assert_eq!(entity(&data, 0, data.len() * 8, &[0, 0xc68, 0, 0, 0x63d]), None);
    }

    #[test]
    fn a_grading_without_its_references_decodes_to_none() {
        let bits = entity_head();
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        assert_eq!(entity(&data, 0, data.len() * 8, &[]), None);
        assert_eq!(entity(&data, 0, data.len() * 8, &[0, 0, 0, 0, 0]), None);
    }
}
