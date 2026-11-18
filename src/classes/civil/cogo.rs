//! COGO points.
//!
//! A COGO point entity (`AECC_COGO_POINT`) is not an AEC entity: after the
//! common entity data its body is only
//!
//! ```text
//! BS 0x04BE, BL point number, 6 bits (001010)
//! ```
//!
//! with no references of its own; what it draws is its proxy graphics. The
//! drawing's point database (`AECC_POINT_NODE`) holds every point:
//!
//! ```text
//! BS 0x04BE, BL 34 (its key in the root tree node), BL 0, BL 0, 3 bits (010)
//! BL count, count × { BL 1, 3BD position, BL number, BS 0x0420 }
//! BL n, n × BL number           points with their own point style
//! BL m, m × BL number           points with their own label style
//! BL 0, BL 0, BL 0
//! BL k, k × { BL number, BL description tag }
//! 13 × BL 0
//! handles: n × point style, m × label style, the AeccDbCogoPoint class node
//! strings: "", then the raw description of each of the k points in order
//! ```
//!
//! Records are in the database's own order, not by number. A point without
//! its own style draws with its point group's; the description tag
//! (`0x80000000 | n`) is the application's id for the description text and
//! equal texts share it. The values given as constants are the same in
//! every sample.
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
//! before the count. `id` is 33 for `_All Points` and the group name's tag
//! for the others; the point group node (`AECC_POINT_GROUP_NODE`) lists them
//! in draw order: `BS 0x04BE, BL 32 (its key in the root tree node), BL n,
//! n × BL id, BL 0, 3 bits (010)`,
//! handles `n × group, label style, point style`.

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
    /// The point's own point style (`AECC_POINT_STYLE`), 0 when it draws
    /// with its point group's.
    pub style: u64,
    /// The point's own label style (`AECC_LABEL_COLLECTOR_STYLE`), 0 when
    /// it uses its point group's.
    pub label_style: u64,
    /// Raw description, `None` when the point has none.
    pub description: Option<String>,
}

/// Point number a COGO point entity refers to.
pub(super) fn entity_number(data: &[u8], start: usize, end: usize) -> Option<u32> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    u32::try_from(bits.bit_long()?).ok().filter(|n| *n > 0)
}

fn record(bits: &mut Bits) -> Option<(u32, Vector3)> {
    (bits.bit_long()? == 1).then_some(())?;
    let position = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    let number = u32::try_from(bits.bit_long()?).ok().filter(|n| *n > 0)?;
    (bits.bit_short()? == 0x0420).then_some(())?;
    Some((number, position))
}

fn numbers(bits: &mut Bits) -> Option<Vec<u32>> {
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
    (0..n).map(|_| u32::try_from(bits.bit_long()?).ok()).collect()
}

/// The fields of a point database (see the module docs).
struct PointDatabase {
    records: Vec<(u32, Vector3)>,
    styled: Vec<u32>,
    labelled: Vec<u32>,
    described: Vec<u32>,
}

fn database(bits: &mut Bits) -> Option<PointDatabase> {
    (bits.bit_short()? == 0x04BE).then_some(())?;
    (bits.bit_long()? == 34 && bits.bit_long()? == 0 && bits.bit_long()? == 0).then_some(())?;
    bits.skip(3)?;
    let count = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 44 <= bits.remaining())?;
    let records = (0..count).map(|_| record(bits)).collect::<Option<Vec<_>>>()?;
    let styled = numbers(bits)?;
    let labelled = numbers(bits)?;
    for _ in 0..3 {
        (bits.bit_long()? == 0).then_some(())?;
    }
    let k = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 4)?;
    let described = (0..k)
        .map(|_| {
            let number = u32::try_from(bits.bit_long()?).ok()?;
            bits.bit_long()?;
            Some(number)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(PointDatabase { records, styled, labelled, described })
}

/// Every point of a point database, in record order. `handles` and
/// `strings` are the record's references and texts in stream order.
pub(super) fn points(data: &[u8], start: usize, end: usize, handles: &[u64], strings: &[String]) -> Vec<CogoPoint> {
    let Some(db) = database(&mut Bits::new(data, start, end)) else {
        return Vec::new();
    };
    let handle = |list: &[u32], offset: usize, number: u32| {
        list.iter().position(|n| *n == number).and_then(|i| handles.get(offset + i)).copied().unwrap_or(0)
    };
    db.records
        .iter()
        .map(|&(number, position)| CogoPoint {
            number,
            position,
            style: handle(&db.styled, 0, number),
            label_style: handle(&db.labelled, db.styled.len(), number),
            description: db
                .described
                .iter()
                .position(|n| *n == number)
                .and_then(|i| strings.get(1 + i))
                .cloned(),
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
    fn point_database_maps_styles_and_descriptions_by_number() {
        // Two points; only point 7 has its own styles and a description.
        let bs_04be = "00 10111110 00000100";
        let bs_0420 = "00 00100000 00000100";
        let record = |p: [f64; 3], n: u32| format!("{} {}{}{} {} {bs_0420}", bl(1), bd(p[0]), bd(p[1]), bd(p[2]), bl(n));
        let s = [
            format!("{bs_04be} {}{}{} 010", bl(34), bl(0), bl(0)),
            format!("{} {} {}", bl(2), record([1.0, 2.0, 3.0], 9), record([100.0, 200.0, 10.0], 7)),
            format!("{}{} {}{}", bl(1), bl(7), bl(1), bl(7)),
            format!("{}{}{}", bl(0), bl(0), bl(0)),
            format!("{}{}{}", bl(1), bl(7), bl(0x8000_0001)),
            bl(0).repeat(13),
        ]
        .concat();
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let points = points(&b, 0, len, &[0x68D, 0x690, 0x185], &["".into(), "TEST".into()]);
        assert_eq!(points.len(), 2);
        assert_eq!((points[0].number, points[0].style, points[0].label_style, points[0].description.as_deref()), (9, 0, 0, None));
        assert_eq!(points[1].position, Vector3::new(100.0, 200.0, 10.0));
        assert_eq!((points[1].style, points[1].label_style, points[1].description.as_deref()), (0x68D, 0x690, Some("TEST")));
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
