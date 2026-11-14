//! Grading criteria (`AECC_GRADING_CRITERIA`): the target and the
//! projection a grading uses, kept as keys after a style-like head:
//!
//! ```text
//! BS 0x04BE, BL id, BL n, n x BL, BL m, m x BL, BL 0, B 1,
//! RC count, count x key, BL 32, ...
//! ```
//!
//! A key is the class key of [`super::alignment::read_key`] with two flag
//! bits: 0x00400000 locks the value, and with 0x40000000 the key carries no
//! value (a locked value the criteria does not override). A value the
//! criteria does not override has no key at all; the keys come in no fixed
//! order. The first id list holds the ids of the criteria derived from this
//! one. Derived criteria store the target and the projection keys with the
//! lock bit set.
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
}

// ponytail: keys 0x31x1 (a copy of each slope), 0x31x3, 0x3024, 0x3130,
// 0x1008, 0x1010 and 0x3008 (a reference, with a surface target) are read
// past; no criteria property changed them in the samples.
pub(super) fn criteria(data: &[u8], start: usize, end: usize) -> Option<GradingCriteria> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    let list = |bits: &mut Bits| {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
        (0..n).map(|_| bits.bit_long()).collect::<Option<Vec<_>>>()
    };
    let derived = list(&mut bits)?;
    list(&mut bits)?;
    (bits.bit_long()? == 0 && bits.bit()?).then_some(())?;
    let mut out = GradingCriteria { derived, ..Default::default() };
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
            _ => {}
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
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
        let read = criteria(&b, 0, b.len() * 8).unwrap();
        assert_eq!(read.distance, CriteriaValue { value: Some(1.0), locked: false });
        assert_eq!(read.target, CriteriaValue { value: Some(3), locked: true });
        assert_eq!(read.slope_format, CriteriaValue { value: None, locked: true });
        assert_eq!(read.slope, CriteriaValue::default());
    }
}
