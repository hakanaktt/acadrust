//! Sample line groups.
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL, BL id, 4 bits, RC n, n keys, 157 bits
//! BL n, n sources { BL 1, BL 0, 128 bits, BL, 2 bits }
//! BL 0, BL n, n placements {
//!     BL index (1, 2, ...), 2 bits, GUID, 21 bits,
//!     BD -left, BD right, B, BD 60, BL, 83 bits, BD 3337, 23 bits }
//! 2 bits, BD first, BD last, B, BD increment (tangent, curve, spiral),
//! BD left, BD right, 45 bits
//! ```
//!
//! The station range, increments and swath close the record; the sample
//! lines follow from those along the parent alignment.

use super::bits::Bits;

/// Stations and swath of the sample lines a group generates.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SampleLines {
    pub stations: Vec<f64>,
    /// Swath width left of the alignment.
    pub left: f64,
    /// Swath width right of the alignment.
    pub right: f64,
}

/// Each placement record ends with this BD.
const PLACEMENT_END: f64 = 3337.0;

// ponytail: the 157-, 128-, 21-, 83-, 23- and 45-bit runs are skipped by
// length (the same bits in all four groups seen, zeros or constants; the
// 128 bits are a per-placement GUID); name them once a drawing varies them.
pub(super) fn sample_lines(data: &[u8], start: usize, end: usize) -> Option<SampleLines> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(4)?;
    for _ in 0..bits.bits_value(8)? {
        super::alignment::key(&mut bits)?;
    }
    bits.skip(157)?;
    for _ in 0..bits.bit_long()? {
        (bits.bit_long()? == 1 && bits.bit_long()? == 0).then_some(())?;
        bits.skip(128)?;
        bits.bit_long()?;
        bits.skip(2)?;
    }
    (bits.bit_long()? == 0).then_some(())?;
    for index in 1..=bits.bit_long()? {
        (bits.bit_long()? == index).then_some(())?;
        bits.skip(2 + 128 + 21)?;
        bits.bit_double()?;
        bits.bit_double()?;
        bits.bit()?;
        bits.bit_double()?;
        bits.bit_long()?;
        bits.skip(83)?;
        (bits.bit_double()? == PLACEMENT_END).then_some(())?;
        bits.skip(23)?;
    }
    bits.skip(2)?;
    let first = bits.bit_double()?;
    let last = bits.bit_double()?;
    bits.bit()?;
    let increment = bits.bit_double()?;
    bits.bit_double()?;
    bits.bit_double()?;
    let left = bits.bit_double()?;
    let right = bits.bit_double()?;
    if bits.remaining() != 45 || !(last > first && increment > 0.0 && (last - first) / increment <= 5000.0) {
        return None;
    }
    let mut stations: Vec<f64> = (0..)
        .map(|i| first + i as f64 * increment)
        .take_while(|s| *s < last - 1e-6)
        .collect();
    stations.push(last);
    Some(SampleLines {
        stations,
        left,
        right,
    })
}
