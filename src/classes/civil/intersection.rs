//! Intersections (`AECC_INTERSECTION`): where the roads meet and the angle
//! of each region between them.
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL, BL, 2 bits, GUID, 2 bits
//! BL, BL, BL n, BL, n x { BD station, 5 x BL }      roads
//! regions: { BL index (1, 2, ...), BL, BD angle, 2 bits }
//! ```
//!
//! A road's station is where the intersection lies on the road's alignment
//! (the record keeps no location: it follows from the first road). The
//! roads' alignments are the record's references, in road order.

use super::bits::Bits;

/// Stations of an intersection's roads and the angles of its regions, in
/// record order.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IntersectionLayout {
    pub road_stations: Vec<f64>,
    /// Radians.
    pub region_angles: Vec<f64>,
}

// ponytail: the BLs around the roads, the regions' second BL (it pairs a
// region with road arms) and their 2 bits are read past.
pub(super) fn layout(data: &[u8], start: usize, end: usize) -> Option<IntersectionLayout> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(2 + 128 + 2)?;
    bits.bit_long()?;
    bits.bit_long()?;
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n * 60 <= bits.remaining())?;
    bits.bit_long()?;
    let mut road_stations = Vec::with_capacity(n);
    for _ in 0..n {
        road_stations.push(bits.bit_double().filter(|v| v.is_finite())?);
        for _ in 0..5 {
            bits.bit_long()?;
        }
    }
    // The regions run until a BL that is not the next index.
    let mut region_angles = Vec::new();
    while bits.bit_long().and_then(|i| usize::try_from(i).ok()) == Some(region_angles.len() + 1) {
        bits.bit_long()?;
        region_angles.push(bits.bit_double().filter(|v| v.is_finite())?);
        bits.skip(2)?;
    }
    Some(IntersectionLayout { road_stations, region_angles })
}
