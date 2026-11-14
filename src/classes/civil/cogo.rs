//! COGO points.
//!
//! A COGO point entity (`AECC_COGO_POINT`) is not an AEC entity: after the
//! common entity data its body is only `BS version, BL point number`. The
//! drawing's point database (`AECC_POINT_NODE`) holds every point:
//!
//! ```text
//! BS version, 17 bits, BL count,
//! count × { BL 1, 3BD position, BL number, BS }
//! BL count, count × BL number, ...
//! handles: count × point style, count × label style, class node
//! ```
//!
//! A point group (`AECC_POINT_PG`) lists the numbers of the points it holds:
//!
//! ```text
//! BS 0x04BE, BL id, BL n, n × BL, BL m, m × BL,
//! [BL 0 (as 2 bits), B 1, RC count, count × class key]   settings
//! BL a, 5 bits, [a = 0: 6 bits, BL serial], BL count, count × RL number
//! ```
//!
//! Groups made or edited by recent releases store `a = 0` and a serial
//! that counts the groups made; others store a nonzero `a` and nothing
//! before the count.

use super::alignment::key;
use super::bits::Bits;
use crate::types::Vector3;

/// One point of the drawing's point database.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CogoPoint {
    pub number: u32,
    /// Easting, northing and elevation.
    pub position: Vector3,
    /// Point style (`AECC_POINT_STYLE`).
    pub style: u64,
    /// Label style (`AECC_LABEL_COLLECTOR_STYLE`).
    pub label_style: u64,
}

/// Point number a COGO point entity refers to.
pub(super) fn entity_number(data: &[u8], start: usize, end: usize) -> Option<u32> {
    let mut bits = Bits::new(data, start, end);
    bits.bit_short()?;
    u32::try_from(bits.bit_long()?).ok().filter(|n| *n > 0)
}

fn record(bits: &mut Bits) -> Option<(u32, Vector3)> {
    if bits.bit_long()? != 1 {
        return None;
    }
    let position = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    let number = u32::try_from(bits.bit_long()?).ok().filter(|n| *n > 0)?;
    bits.bit_short()?;
    Some((number, position))
}

/// Every point of a point database, in record order.
pub(super) fn points(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Vec<CogoPoint> {
    let mut bits = Bits::new(data, start, end);
    let read = |bits: &mut Bits| -> Option<Vec<(u32, Vector3)>> {
        bits.bit_short()?;
        // ponytail: 17 bits skipped by length (one drawing with points);
        // name the fields once a drawing varies them.
        bits.pos += 17;
        let count = usize::try_from(bits.bit_long()?).ok()?;
        (0..count).map(|_| record(bits)).collect()
    };
    let records = read(&mut bits).unwrap_or_default();
    let count = records.len();
    let handle = |i: usize| handles.get(i).copied().unwrap_or(0);
    records
        .into_iter()
        .enumerate()
        .map(|(i, (number, position))| CogoPoint {
            number,
            position,
            style: handle(i),
            label_style: handle(count + i),
        })
        .collect()
}

/// Point numbers a point group holds, in record order.
// ponytail: the 5 and 6-bit runs and `a` are read past; name them once a
// sample varies them alongside the group's settings.
pub(super) fn group_numbers(data: &[u8], start: usize, end: usize) -> Option<Vec<u32>> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    for _ in 0..2 {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
        for _ in 0..n {
            bits.bit_long()?;
        }
    }
    // The settings' BL 0 is the 2-bit form; `a` is written with its byte.
    let mut probe = Bits::new(data, bits.pos, end);
    if probe.bits_value(3)? == 0b101 {
        for _ in 0..probe.bits_value(8)? {
            key(&mut probe)?;
        }
        bits = probe;
    }
    let a = bits.bit_long()?;
    bits.skip(5)?;
    if a == 0 {
        bits.skip(6)?;
        bits.bit_long()?;
    }
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 32 <= bits.remaining())?;
    (0..n).map(|_| u32::try_from(bits.raw_long()?).ok()).collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    #[test]
    fn group_numbers_follow_the_serial() {
        // Head without lists or settings, a = 0 (byte form), 5 + 6 bits,
        // serial 1, two numbers: 5 and 7.
        let s = "00 10111110 00000100 10 10 10 01 00000000 00000 100000 0100000001 0100000010
                 00000101 00000000 00000000 00000000 00000111 00000000 00000000 00000000";
        let b = bits_of(s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        assert_eq!(group_numbers(&b, 0, len), Some(vec![5, 7]));
    }
}
