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
