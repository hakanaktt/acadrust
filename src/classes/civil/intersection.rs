//! Intersections (`AECC_INTERSECTION`): where the roads meet and the angle
//! of each region between them.
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL, BL, 2 bits, GUID, 2 bits
//! BL kind (1 crossing, 2 tee junction), BL, BL n, BL
//! n x { BD station, 4 x BL, BL arm }                 roads
//! 4 (crossing) or 2 (tee) x { BL index (1 up), BL quadrant
//!   (NE 2, SE 4, SW 3, NW 1), BD angle, 2 bits }      regions
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
    /// A tee junction (one road starting or ending on the other) rather
    /// than a crossing.
    pub tee: bool,
    pub road_stations: Vec<f64>,
    /// Radians.
    pub region_angles: Vec<f64>,
}

// ponytail: the BLs around the roads, the regions' quadrant words and
// their 2 bits are read past.
pub(super) fn layout(data: &[u8], start: usize, end: usize) -> Option<IntersectionLayout> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(2 + 128 + 2)?;
    let tee = match bits.bit_long()? {
        1 => false,
        2 => true,
        _ => return None,
    };
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
    let region_angles = (1..=if tee { 2 } else { 4 })
        .map(|i| {
            (bits.bit_long()? == i).then_some(())?;
            bits.bit_long()?;
            let angle = bits.bit_double().filter(|v| v.is_finite())?;
            bits.skip(2)?;
            Some(angle)
        })
        .collect::<Option<Vec<_>>>()?;
    Some(IntersectionLayout { tee, road_stations, region_angles })
}
