//! Sample line groups.
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL, BL id, 4 bits, RC n, n keys, ...
//! ```
//!
//! Two record forms share that head. Records written by the generator of
//! the original samples continue
//!
//! ```text
//! 157 bits
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
//!
//! Records written by the generators of the AutoCAD 2027 tutorial corpus
//! (the only other form seen) end differently: after the head, the schema
//! word `0x10020004` and a group word, their keys are bare 34-bit values
//! — observed types 0x00, 0x03 and 0x98 at an exact 34-bit stride, where
//! the shared reader (`alignment::read_key`) expects a value after the
//! key word and rejects the type bytes outright. No sample determines
//! their layout past the keys, so the decoder reports the keys and stops
//! there (see [`SampleLines::unparsed_bits`]); the group's stations may
//! not be in this record at all but in its child `AECC_SAMPLE_LINE`
//! records, which have no decoder yet.

use super::bits::Bits;

/// Stations and swath of the sample lines a group generates.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SampleLines {
    /// Stations of the placements, for records whose placement layout
    /// this decoder maps; empty for records whose tail is not mapped.
    pub stations: Vec<f64>,
    /// Swath width left of the alignment, `0.0` when unmapped.
    pub left: f64,
    /// Swath width right of the alignment, `0.0` when unmapped.
    pub right: f64,
    /// The class keys after the head, in record order: read as key words
    /// with a value where the shared reader applies, else as bare values.
    pub keys: Vec<u32>,
    /// Bits after the class keys this decoder does not map (0 when the
    /// whole record is decoded).
    pub unparsed_bits: usize,
}

/// Each placement record of a mapped record ends with this BD.
const PLACEMENT_END: f64 = 3337.0;

// ponytail: the 157-, 128-, 21-, 83-, 23- and 45-bit runs are skipped by
// length (the same bits in all four mapped groups seen, zeros or constants; the
// 128 bits are a per-placement GUID); name them once a drawing varies them.
pub(super) fn sample_lines(data: &[u8], start: usize, end: usize) -> Option<SampleLines> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(4)?;
    let count = bits.bits_value(8)?;
    // Class keys. Keys with a value after the key word go through the
    // shared reader; the bare 34-bit keys of the tutorial generators are
    // read when it does not apply.
    let at = bits.pos;
    let keyed = (0..count)
        .map(|_| super::alignment::read_key(&mut bits).map(|(key, _)| key))
        .collect::<Option<Vec<u32>>>();
    if let Some(keys) = keyed {
        if let Some((stations, left, right)) = placements(&mut bits) {
            return Some(SampleLines { stations, left, right, keys, unparsed_bits: 0 });
        }
    }
    bits.pos = at;
    let keys = (0..count).map(|_| bits.bit_long().map(|key| key as u32)).collect::<Option<Vec<u32>>>()?;
    // The head and keys are all a tutorial record is decoded to so far;
    // its stations, if the record carries any, stay unparsed.
    Some(SampleLines { stations: Vec::new(), left: 0.0, right: 0.0, keys, unparsed_bits: bits.remaining() })
}

/// The sources and placements after the class keys of a record whose
/// layout is mapped, then its station range (`bits` left after the keys).
fn placements(bits: &mut Bits) -> Option<(Vec<f64>, f64, f64)> {
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
    Some((stations, left, right))
}
