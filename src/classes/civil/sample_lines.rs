//! Sample line groups.
//!
//! A group stores one placement record per section view group (position,
//! swath `BD -left, BD right`, ..., `BD 3337.0`), then the station range and
//! increment (`BD start, BD end, BD increment`) 25 bits after the last one.
//! The sample lines follow from those along the parent alignment.

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

/// `count` BDs at `pos`, each with whether it was written in full (not as
/// the compact 0.0 / 1.0 forms).
fn doubles_at(data: &[u8], pos: usize, end: usize, count: usize) -> Option<Vec<(f64, bool)>> {
    let mut bits = Bits::new(data, pos, end);
    (0..count)
        .map(|_| {
            let before = bits.pos;
            let value = bits.bit_double().filter(|v| v.is_finite() && v.abs() < 1e7)?;
            Some((value, bits.pos - before == 66))
        })
        .collect()
}

fn whole(x: f64) -> bool {
    (x * 1000.0 - (x * 1000.0).round()).abs() < 1e-6 && x != 0.0
}

/// Each section view placement record ends with this BD.
const PLACEMENT_END: f64 = 3337.0;

pub(super) fn sample_lines(data: &[u8], start: usize, end: usize) -> Option<SampleLines> {
    // Last placement record; the station range follows it.
    let anchor = (start..end)
        .filter(|&pos| {
            let mut bits = Bits::new(data, pos, end);
            bits.bit_double() == Some(PLACEMENT_END) && bits.pos == pos + 66
        })
        .last()?;
    let (left, right) = (anchor.saturating_sub(320)..anchor).find_map(|pos| {
        let v = doubles_at(data, pos, end, 2)?;
        let ((l, lr), (r, rr)) = (v[0], v[1]);
        (lr && rr && l < 0.0 && r > 0.0 && whole(l) && whole(r) && -l <= 1000.0 && r <= 1000.0)
            .then_some((-l, r))
    })?;
    let range = anchor + 66 + 25;
    let mut bits = Bits::new(data, range, end);
    let first = bits.bit_double()?;
    let last = bits.bit_double()?;
    bits.bit()?;
    let increment = bits.bit_double()?;
    if !(last > first && increment > 0.0 && (last - first) / increment <= 5000.0) {
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
