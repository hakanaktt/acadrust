//! Corridor feature lines.
//!
//! A corridor body stores its baseline and every feature line as a run of
//! station records `B, BD station, BD x, BD y, BD z` in world coordinates,
//! one record per corridor station.

use super::bits::Bits;
use crate::types::Vector3;

struct Station {
    station: f64,
    point: Vector3,
}

fn station_at(data: &[u8], pos: usize, end: usize) -> Option<(Station, usize)> {
    let mut bits = Bits::new(data, pos, end);
    bits.bit()?;
    let station = bits.bit_double()?;
    let point = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    let plausible = station.is_finite()
        && station.abs() < 1e7
        && point.x.is_finite()
        && point.y.is_finite()
        && point.z.abs() < 1e5
        && (point.x.abs() > 1.0 || point.y.abs() > 1.0);
    plausible.then_some((Station { station, point }, bits.pos))
}

/// Runs of at least three consecutive station records.
// ponytail: probes every bit offset outside a run; parse the corridor's
// region/frequency headers instead once their layout is known.
pub(super) fn feature_lines(data: &[u8], start: usize, end: usize) -> Vec<Vec<(f64, Vector3)>> {
    let mut out = Vec::new();
    let mut pos = start;
    while pos < end {
        let Some((first, mut next)) = station_at(data, pos, end) else {
            pos += 1;
            continue;
        };
        let mut run = vec![first];
        while let Some((record, after)) = station_at(data, next, end) {
            let last = run.last().unwrap();
            let step = record.station - last.station;
            let gap = record.point.distance(&last.point);
            if !(0.0..=1000.0).contains(&step) || gap > step * 3.0 + 50.0 {
                break;
            }
            run.push(record);
            next = after;
        }
        if run.len() >= 3 {
            out.push(run.into_iter().map(|s| (s.station, s.point)).collect());
            pos = next;
        } else {
            pos += 1;
        }
    }
    out
}
