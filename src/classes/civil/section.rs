//! Sections a sample line cuts through a surface or a corridor surface
//! (`AECC_SECTION`, `AECC_SECTION_CORRIDOR`).
//!
//! ```text
//! BL 80, 0, 80, 80, 0, BL, BL, 2 bits, GUID, 2 bits
//! 4 x 3BD frame: origin, then the offset, elevation and normal axes
//! BL n, n x { RC flags, BD offset, BD elevation, BL }
//! BL m, m x { RC flags, BL from point, BL to point, BL }
//! ```
//!
//! A surface section links each point to the next; a corridor section's
//! links and the code data after them are not mapped.

use super::bits::Bits;
use crate::types::Vector2;

/// Points of a section in its (offset, elevation) frame and the point pairs
/// it links.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SectionGeometry {
    pub points: Vec<Vector2>,
    pub links: Vec<(u32, u32)>,
}

// ponytail: the 2-bit runs, the flags and the BL closing each point and
// link are skipped (the flags are 0 in every sample, the BLs 0 or 1); the
// frame is read past, it is the identity in every sample.
pub(super) fn geometry(data: &[u8], start: usize, end: usize) -> Option<SectionGeometry> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(2 + 128 + 2)?;
    for _ in 0..12 {
        bits.bit_double()?;
    }
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 14 <= bits.remaining())?;
    let points = (0..n)
        .map(|_| {
            bits.skip(8)?;
            let point = Vector2::new(bits.bit_double()?, bits.bit_double()?);
            bits.bit_long()?;
            Some(point)
        })
        .collect::<Option<Vec<_>>>()?;
    let m = usize::try_from(bits.bit_long()?).ok().filter(|m| m * 14 <= bits.remaining())?;
    let links = (0..m)
        .map(|_| {
            bits.skip(8)?;
            let from = u32::try_from(bits.bit_long()?).ok()?;
            let to = u32::try_from(bits.bit_long()?).ok()?;
            bits.bit_long()?;
            Some((from, to))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(SectionGeometry { points, links })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    #[test]
    fn reads_two_points_and_their_link() {
        // Head, identity frame, points (0, 1) and (1, 0), one link 0 -> 1.
        let zeros = "0".repeat(128);
        let s = format!(
            "0101010000 10 0101010000 0101010000 10 10 10 00 {zeros} 00
             10 10 10 01 10 10 10 01 10 10 10 01
             0100000010 00000000 10 01 10 00000000 01 10 10
             0100000001 00000000 10 0100000001 10"
        );
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let read = geometry(&b, 0, len).unwrap();
        assert_eq!(read.points, vec![Vector2::new(0.0, 1.0), Vector2::new(1.0, 0.0)]);
        assert_eq!(read.links, vec![(0, 1)]);
    }
}
