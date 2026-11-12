//! COGO points.
//!
//! A COGO point entity (`AECC_COGO_POINT`) is not an AEC entity: after the
//! common entity data its body is only `BS version, BL point number`. The
//! drawing's point database (`AECC_POINT_NODE`) holds every point:
//!
//! ```text
//! BS version, ..., BL count, count × { BL 1, 3BD position, BL number, 18 bits }
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

/// Bits between a record's number and the next record.
// ponytail: constant in every point of the one drawing with COGO points
// (no descriptions); read the fields once a drawing varies them.
const RECORD_TAIL: usize = 18;

fn record(bits: &mut Bits) -> Option<(u32, Vector3)> {
    if bits.bit_long()? != 1 {
        return None;
    }
    let position = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    if ![position.x, position.y, position.z].iter().all(|v| v.is_finite() && v.abs() < 1e10) {
        return None;
    }
    let number = u32::try_from(bits.bit_long()?).ok().filter(|n| *n > 0)?;
    bits.pos += RECORD_TAIL;
    Some((number, position))
}

/// Every point of a point database, in record order.
pub(super) fn points(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Vec<CogoPoint> {
    // The count sits a few fields past the version: take the first position
    // where a count is followed by exactly that many records.
    for pos in start..(start + 256).min(end) {
        let mut bits = Bits::new(data, pos, end);
        let Some(count) = bits.bit_long().and_then(|c| usize::try_from(c).ok()).filter(|c| (1..10_000_000).contains(c)) else {
            continue;
        };
        if bits.remaining() < count * 200 {
            continue;
        }
        let records: Vec<_> = (0..count).map_while(|_| record(&mut bits)).collect();
        if records.len() != count {
            continue;
        }
        let handle = |i: usize| handles.get(i).copied().unwrap_or(0);
        return records
            .into_iter()
            .enumerate()
            .map(|(i, (number, position))| CogoPoint {
                number,
                position,
                style: handle(i),
                label_style: handle(count + i),
            })
            .collect();
    }
    Vec::new()
}
