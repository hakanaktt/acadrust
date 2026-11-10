//! Corridor feature lines.
//!
//! A corridor body stores its feature lines as groups, one per point code
//! in the order of the corridor's code list:
//!
//! ```text
//! BL  segment count
//! segment count x { BL n, n x { BD station, BD x, BD y, BD z, B } }
//! ```
//!
//! Points are in world coordinates, one record per corridor station. A
//! corridor with several baselines keeps one such run of groups per
//! baseline, with other records between them.

use super::bits::Bits;
use crate::types::Vector3;

/// One `4BD` station record, `None` past the end or when implausible.
fn record(bits: &mut Bits) -> Option<(f64, Vector3)> {
    let station = bits.bit_double()?;
    let point = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    let plausible = station.is_finite()
        && station.abs() < 1e7
        && point.x.is_finite()
        && point.y.is_finite()
        && point.x.abs() < 1e9
        && point.y.abs() < 1e9
        && point.z.abs() < 1e5
        && (point.x.abs() > 1.0 || point.y.abs() > 1.0);
    plausible.then_some((station, point))
}

/// `BL n`, then `n` records each followed by a flag bit. Stations rise and
/// neighbors stay close, which rejects random bits that happen to parse.
fn segment(bits: &mut Bits) -> Option<Vec<(f64, Vector3)>> {
    let n = usize::try_from(bits.bit_long()?).ok()?;
    if n == 0 || n > 100_000 {
        return None;
    }
    let mut out: Vec<(f64, Vector3)> = Vec::with_capacity(n.min(4096));
    for _ in 0..n {
        let record = record(bits)?;
        bits.bit()?;
        if let Some(last) = out.last() {
            let step = record.0 - last.0;
            if !(0.0..=1000.0).contains(&step) || record.1.distance(&last.1) > step * 3.0 + 50.0 {
                return None;
            }
        }
        out.push(record);
    }
    Some(out)
}

/// `BL` segment count, then the segments.
fn group(bits: &mut Bits, first: bool) -> Option<Vec<Vec<(f64, Vector3)>>> {
    let n = usize::try_from(bits.bit_long()?).ok()?;
    if n == 0 || n > 1000 {
        return None;
    }
    let segments: Vec<_> = (0..n).map(|_| segment(bits)).collect::<Option<_>>()?;
    // A lone record is too weak to anchor the first group.
    (!first || segments.iter().map(Vec::len).sum::<usize>() >= 2).then_some(segments)
}

/// Feature lines of a corridor per baseline: each baseline's groups, one
/// per point code in the order of the corridor's code list, each group
/// holding that code's segments.
// ponytail: finds each baseline's first group by probing bit offsets; parse
// the baseline/region records before it once their layout is known.
pub(super) fn baselines(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vec<Vec<(f64, Vector3)>>>> {
    let mut out = Vec::new();
    let mut from = start;
    while let Some(mut pos) = (from..end).find(|&pos| group(&mut Bits::new(data, pos, end), true).is_some()) {
        let mut groups = Vec::new();
        loop {
            let mut bits = Bits::new(data, pos, end);
            match group(&mut bits, groups.is_empty()) {
                Some(found) => {
                    groups.push(found);
                    pos = bits.pos;
                }
                None => break,
            }
        }
        out.push(groups);
        from = pos;
    }
    out
}

/// Feature line groups of every baseline, one after another.
pub(super) fn feature_line_groups(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vec<(f64, Vector3)>>> {
    baselines(data, start, end).into_iter().flatten().collect()
}

/// Every feature line segment of a corridor.
pub(super) fn feature_lines(data: &[u8], start: usize, end: usize) -> Vec<Vec<(f64, Vector3)>> {
    feature_line_groups(data, start, end).into_iter().flatten().collect()
}
