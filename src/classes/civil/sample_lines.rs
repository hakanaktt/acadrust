//! Sample line groups.
//!
//! Two record forms share the head; the schema word after it tells them
//! apart:
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL schema, BL id, 4 bits, RC n, n keys, ...
//! ```
//!
//! Records written by the generator of the original samples carry the
//! schema word `0x10025F04` and continue
//!
//! ```text
//! 157 bits
//! BL n, n sources { BL 1, BL 0, 128 bits, BL, 2 bits }
//! BL 0, BL n, n placements {
//!     BL index (1, 2, ...), 2 bits, GUID, 21 bits,
//!     BD -left, BD right, B, BD 60, BL, 83 bits, BD end, 23 bits }
//! 2 bits, BD first, BD last, B, BD increment (tangent, curve, spiral),
//! BD left, BD right, 45 bits
//! ```
//!
//! The station range, increments and swath close the record; the sample
//! lines follow from those along the parent alignment.
//!
//! Records written by the generators of the AutoCAD 2027 tutorial corpus
//! carry the schema word `0x10020004` and keys that are bare 34-bit
//! values (observed types 0x00, 0x03 and 0x98 at an exact 34-bit stride,
//! where the shared reader expects a value after the key word). No sample
//! maps their layout past the keys, so nothing after them is read.

use super::bits::Bits;

/// Stations and swath of the sample lines a group generates.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SampleLines {
    /// Stations of the placements.
    pub stations: Vec<f64>,
    /// Swath width left of the alignment.
    pub left: f64,
    /// Swath width right of the alignment.
    pub right: f64,
}

/// Schema word of the records whose sources and placements this decoder
/// maps.
const MAPPED: u32 = 0x1002_5F04;

/// Schema word of the AutoCAD 2027 tutorial corpus's records.
const TUTORIAL: u32 = 0x1002_0004;

pub(super) fn sample_lines(data: &[u8], start: usize, end: usize) -> Option<SampleLines> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    let schema = bits.bit_long()? as u32;
    bits.bit_long()?;
    bits.skip(4)?;
    let count = bits.bits_value(8)?;
    match schema {
        MAPPED => {
            for _ in 0..count {
                super::alignment::key(&mut bits)?;
            }
            placements(&mut bits)
        }
        // The tutorial records' bare 34-bit keys are all of the form this
        // decoder maps; what follows them is not known yet.
        TUTORIAL => {
            for _ in 0..count {
                bits.bit_long()?;
            }
            None
        }
        _ => None,
    }
}

/// The sources and placements after the class keys of a mapped record,
/// then its station range (`bits` left after the keys).
fn placements(bits: &mut Bits) -> Option<SampleLines> {
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
        // The BD here closed every placement of the mapped samples; its
        // value belongs to the generator, not the layout.
        bits.bit_double()?;
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
    (last > first && increment > 0.0 && (last - first) / increment <= 5000.0).then_some(())?;
    let mut stations: Vec<f64> = (0..)
        .map(|i| first + i as f64 * increment)
        .take_while(|s| *s < last - 1e-6)
        .collect();
    stations.push(last);
    Some(SampleLines { stations, left, right })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bl(v: u32) -> String {
        match v {
            0 => "10".into(),
            1..=255 => format!("01{v:08b}"),
            v => format!("00{}", v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>()),
        }
    }

    fn bd(v: f64) -> String {
        format!("00{}", v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>())
    }

    #[test]
    fn reads_stations_of_a_mapped_record() {
        let s = [
            "0101010000 10 0101010000 0101010000 10".to_string(), // head
            bl(MAPPED),
            bl(0x4000_0501),
            "0010".to_string(),
            "00000000".to_string(), // RC 0 keys
            "0".repeat(157),
            bl(0), // no sources
            bl(0),
            bl(0), // no placements
            "00".to_string(),
            bd(100.0),
            bd(110.0),
            "0".to_string(),
            bd(2.5),
            bd(0.0),
            bd(0.0),
            bd(-4.5),
            bd(4.5),
            "0".repeat(45),
        ]
        .concat();
        let data: Vec<u8> = s
            .bytes()
            .filter(|b| *b == b'0' || *b == b'1')
            .map(|b| b - b'0')
            .collect::<Vec<u8>>()
            .chunks(8)
            .map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | (b << (7 - i))))
            .collect();
        let len = s.bytes().filter(|b| *b == b'0' || *b == b'1').count();
        let lines = sample_lines(&data, 0, len).expect("mapped record decodes");
        assert_eq!(lines.stations, vec![100.0, 102.5, 105.0, 107.5, 110.0]);
        assert_eq!((lines.left, lines.right), (-4.5, 4.5));
    }

    #[test]
    fn reads_a_tutorial_records_bare_keys_and_stops_at_the_unmapped_tail() {
        let s = [
            "0101010000 10 0101010000 0101010000 10".to_string(), // head
            bl(TUTORIAL),
            bl(0x4080_8129),
            "0101".to_string(),
            "00000011".to_string(), // RC 3 keys
            bl(0x0000_0C00),
            bl(0),
            bl(0),
            "0".repeat(64),
        ]
        .concat();
        let data: Vec<u8> = s
            .bytes()
            .filter(|b| *b == b'0' || *b == b'1')
            .map(|b| b - b'0')
            .collect::<Vec<u8>>()
            .chunks(8)
            .map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | (b << (7 - i))))
            .collect();
        let len = s.bytes().filter(|b| *b == b'0' || *b == b'1').count();
        assert_eq!(sample_lines(&data, 0, len), None);
    }
}
