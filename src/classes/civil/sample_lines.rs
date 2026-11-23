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
//! The AutoCAD 2027 tutorial corpus's group records (Sections-Project-
//! Objects.dwg, Mass Haul-2.dwg: four groups of 0, 19, 32 and 132 lines)
//! carry the schema word `0x10025F04` — the mapped records' word — but
//! their class keys are not the shared reader's keys: bare 34-bit values
//! (a `00` prefix and a 32-bit word whose top byte takes 0x00, 0x04,
//! 0x07, 0x0B or 0x20 there), so the mapped branch stops at the keys and
//! the record returns None. Measured layout:
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL 0x10025F04, BL group id, 4 bits,
//! RC n (0..4), n x 34 bits, then a per-record tail
//! ```
//!
//! The tail has no field grammar common to the four records (the 0-line
//! group's tail is as long as the 19-line group's; the 132-line group's
//! is about 130 times the 32-line group's): it is the application's
//! save-state cache, byte-identical across saves of the same record, and
//! stays unmapped. The member lines are not in the body: the record's
//! handle stream lists them after its fourth reference, in station
//! order, and each line names itself and gives its station (see
//! [`sample_line_station`]); the group's name is its text stream's third
//! string.

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

/// A section view group plot of a sample line group (records of the
/// `0x10025F04` schema whose lines are listed in the record, as acadrust
/// and the application write them): its index (1 up), the point it was
/// placed at and a count (3 in every sample, from 2 to 10 views and one to
/// three section sources; not the number of views), with the values the
/// record holds besides (the same in every sample).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SectionViewGroupPlot {
    pub index: i32,
    pub origin: crate::types::Vector3,
    /// The count after the values: 3 in every sample, not the number of
    /// views.
    pub count: i32,
    /// The eight BDs after the point (0), the three values (-30, 30, 40),
    /// the 16 bytes after the count (0) and the 52 bits after them.
    pub extra: [f64; 8],
    pub values: [f64; 3],
    pub flags: [bool; 2],
    pub id: [u8; 16],
    pub tail: u64,
}

/// The group plots of a group record that lists its lines:
///
/// ```text
/// (after the keys) 155 bits, BL lines, BL n sources {BL 1, BL 0,
/// 128 bits, BL, 2 bits}, BL 0, BL p plots {BL index, 3BD point, 8 BD,
/// B, BD, BD, B, BD, BL count, 16 bytes, 52 bits}
/// ```
pub(super) fn group_plots(data: &[u8], start: usize, end: usize) -> Option<Vec<SectionViewGroupPlot>> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    (bits.bit_long()? as u32 == MAPPED).then_some(())?;
    bits.bit_long()?;
    bits.skip(4)?;
    for _ in 0..bits.bits_value(8)? {
        super::alignment::key(&mut bits)?;
    }
    bits.skip(155)?;
    bits.bit_long()?;
    for _ in 0..bits.bit_long()? {
        (bits.bit_long()? == 1 && bits.bit_long()? == 0).then_some(())?;
        bits.skip(128)?;
        bits.bit_long()?;
        bits.skip(2)?;
    }
    (bits.bit_long()? == 0).then_some(())?;
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 200)?;
    (0..n)
        .map(|_| {
            let index = bits.bit_long()?;
            let origin = crate::types::Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
            let mut extra = [0.0; 8];
            for v in &mut extra {
                *v = bits.bit_double()?;
            }
            let a = bits.bit()?;
            let (v0, v1) = (bits.bit_double()?, bits.bit_double()?);
            let b = bits.bit()?;
            let values = [v0, v1, bits.bit_double()?];
            let count = bits.bit_long()?;
            let mut id = [0u8; 16];
            for x in &mut id {
                *x = bits.bits_value(8)?;
            }
            let mut tail = 0u64;
            for _ in 0..52 {
                tail = tail << 1 | u64::from(bits.bits_value(1)?);
            }
            Some(SectionViewGroupPlot { index, origin, count, extra, values, flags: [a, b], id, tail })
        })
        .collect()
}

/// The word after a sample line's schema word in the layout the codec
/// writes (see `create::sample_lines::write_line`).
const LINE_WORD: u32 = 0x4000_8353;

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

/// One sample line's (`AECC_SAMPLE_LINE`) station:
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, 6 bits (they vary per record),
/// 275 bits (constants of the record kind, a per-record GUID among
/// them), BD station
/// ```
///
/// A record of the layout the codec writes (`BL 0x40008353` after the
/// schema word) holds 4 bits, RC 0, 120 bits and 12 BD (origin and
/// identity) before its station instead.
///
/// The 275 bits are the same length in every one of the 183 records of
/// `Sections-Project-Objects.dwg` (19 lines, stations 0 to
/// 1729.048573328025) and `Mass Haul-2.dwg` (164 lines, 50 to 1658.63)
/// whose stations the application lists the same way; within them only a
/// record GUID and the 6 tag bits vary. The line's name is its text
/// stream's third string, and its group lists its lines in station order
/// among its references.
pub(super) fn sample_line_station(data: &[u8], start: usize, end: usize) -> Option<f64> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    (bits.bit_long()? as u32 == MAPPED).then_some(())?;
    let at = bits.pos;
    if bits.bit_long()? as u32 == LINE_WORD {
        // The layout the codec writes (WL11 to WL13): 4 bits, RC 0, 120
        // bits, the origin and the identity (12 BD), then the station.
        bits.skip(4 + 8 + 120)?;
        for _ in 0..12 {
            bits.bit_double()?;
        }
    } else {
        bits.pos = at;
        bits.skip(6 + 275)?;
    }
    let station = bits.bit_double()?;
    (station.is_finite() && (0.0..1e7).contains(&station)).then_some(station)
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
    fn reads_a_sample_lines_station() {
        let s = [
            "0101010000 10 0101010000 0101010000 10".to_string(), // head
            bl(MAPPED),
            "000100".to_string(), // 6 bits, vary per record
            "1".repeat(120),      // the fixed run (a record GUID among the constants)
            "0".repeat(155),
            bd(1729.048573328025),
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
        assert_eq!(sample_line_station(&data, 0, len), Some(1729.048573328025));
        // Another schema word is not a sample line record.
        let other = s.replace(&bl(MAPPED), &bl(TUTORIAL));
        let data: Vec<u8> = other
            .bytes()
            .filter(|b| *b == b'0' || *b == b'1')
            .map(|b| b - b'0')
            .collect::<Vec<u8>>()
            .chunks(8)
            .map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | (b << (7 - i))))
            .collect();
        assert_eq!(sample_line_station(&data, 0, len), None);
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
