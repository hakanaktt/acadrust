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
//!
//! A section view (`AECC_GRAPH_SECTION_VIEW`) has the profile view's
//! graph layout (see [`super::profile`]) with shorter band records:
//!
//! ```text
//! graph head, 3BD origin, 10 BD, RC n, n x 5 bits, BL m, m x record, 1 bit,
//! 3BD origin, BL, BL, 3 x 3BD range (min, centre, max), 3 x 3BD,
//! BL, BL byte count + UTF-16 title, BL 0, 7 x BD, 6 x BL,
//! 3BD origin, 4 x BD, BD vertical scale
//! record: BL 0, BL, BD, BL, BD, BL index, BD, BD
//! ```
//!
//! A mass haul view (`AECC_MASS_HAUL_VIEW`) has the same layout, with
//! stations for offsets and volumes for elevations; its origin is the
//! lowest station at volume 0 and keeps the view's elevation.

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

/// A mass haul view's frame: where it draws stations and volumes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MassHaulViewFrame {
    /// Drawing position of (`station_min`, volume 0).
    pub origin: Vector2,
    pub station_min: f64,
    pub station_max: f64,
    pub volume_min: f64,
    pub volume_max: f64,
    /// Drawing units per volume unit.
    pub vertical_scale: f64,
}

impl MassHaulViewFrame {
    /// Drawing position of a station and volume.
    pub fn to_drawing(&self, station: f64, volume: f64) -> Vector2 {
        Vector2::new(
            self.origin.x + station - self.station_min,
            self.origin.y + volume * self.vertical_scale,
        )
    }
}

pub(super) fn mass_haul_view_frame(data: &[u8], start: usize, end: usize) -> Option<MassHaulViewFrame> {
    let f = view_frame(data, start, end)?;
    Some(MassHaulViewFrame {
        origin: f.origin,
        station_min: f.offset_min,
        station_max: f.offset_max,
        volume_min: f.elevation_min,
        volume_max: f.elevation_max,
        vertical_scale: f.vertical_scale,
    })
}

/// Stations and volumes of a mass haul line (`AECC_MASSHAULLINE`): its
/// balance curve, read by the record's fields:
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL, BL, 2 bits, BL 0, GUID, 2 bits, GUID,
/// 127 bits, BD, BL 1, BL 0, BL n, n x 3BD (station, volume, 0)
/// ```
///
/// The first GUID is all zero bits in a line the wizard made and all one
/// bits in the tutorial's line; the BD is 300 and 500 in the two. The
/// tutorial's first BL is 268590852; the second read 1073775415 in the
/// shipped file and 1082132272 after the application resaved the record,
/// and 1082168083 in the wizard's fresh line (2026-11-27) — the word is
/// per-record and unnamed; the create path writes the fresh line's value
/// and the application rewrites it on its first save
/// ([`super::create::mass_haul`]).
///
/// After the curve the record carries its derived display data, mapped from
/// the tutorial's record (2026-11-26, re-walked bit-exact): a vertex table
/// (BL 131, BD 0.0, 131 x (BL, BL) identity rows, last (131, 132)), a
/// bit-verbatim copy of the whole curve record (the 132-vertex drawn line),
/// a second copy of the table with last row (131, 166), then exactly 166
/// (station, volume, 0) triplets — the free-haul construction drawn from the
/// curve and the head BD: the curve's points; its linear zero crossings;
/// the curve shifted +BD (a point's station + BD with that point's volume);
/// the stations where the curve meets its shift (curve(x) = curve(x + BD),
/// here 214.936…, 605.953…, and at the local-minimum ordinate); baseline
/// points at the crossings. A byte index map then re-samples those 166
/// vertices for display, and smaller paths (connectors, closing runs) reuse
/// the same vertices. The wizard's fresh line stores the same head, tables
/// and an empty polyline #2 (last row (131, 0)) with no construction at
/// all — that create-path form is what [`super::create::mass_haul`] writes
/// (2026-11-27, adopt-verified). The update path's construction is not
/// writable yet: the application's derived stations differ from any
/// interpolation of the stored curve by 1-2 ULP (38.5931834845066
/// recorded, 38.59318348450661 from the natural formulas), and the vertex
/// order, the resampling step and the smaller paths' composition are
/// unnamed.
// ponytail: the 2-bit runs and the 127 bits (zero in every record) are
// read past by their sizes; name them once a drawing varies them.
// ponytail: name the application's floating-point pipeline for the derived
// stations and the display-path composition rules; the update path's tail
// then gets a writer like the create path's (§3q of docs/civil-write.md).
pub(super) fn mass_haul_points(data: &[u8], start: usize, end: usize) -> Option<Vec<Vector2>> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(2)?;
    (bits.bit_long()? == 0).then_some(())?;
    bits.skip(128 + 2 + 128 + 127)?;
    bits.bit_double()?;
    (bits.bit_long()? == 1 && bits.bit_long()? == 0).then_some(())?;
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 6 <= bits.remaining())?;
    (0..n)
        .map(|_| {
            let p = Vector2::new(bits.bit_double()?, bits.bit_double()?);
            (bits.bit_double()? == 0.0).then_some(p)
        })
        .collect()
}

/// A section view's frame: the drawing position of offset 0 at the lowest
/// elevation, the offset and elevation ranges and the vertical scale.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SectionViewFrame {
    /// Drawing position of (offset 0, `elevation_min`).
    pub origin: Vector2,
    pub offset_min: f64,
    pub offset_max: f64,
    pub elevation_min: f64,
    pub elevation_max: f64,
    /// Drawing units per elevation unit.
    pub vertical_scale: f64,
}

impl SectionViewFrame {
    /// Drawing position of an offset and elevation.
    pub fn to_drawing(&self, offset: f64, elevation: f64) -> Vector2 {
        Vector2::new(
            self.origin.x + offset,
            self.origin.y + (elevation - self.elevation_min) * self.vertical_scale,
        )
    }
}

// ponytail: the 5-bit runs, the band records' values and the fields
// between the ranges and the vertical scale are read past by their sizes.
pub(super) fn view_frame(data: &[u8], start: usize, end: usize) -> Option<SectionViewFrame> {
    let mut bits = Bits::new(data, start, end);
    super::profile::graph_head(&mut bits)?;
    let point = |bits: &mut Bits| -> Option<Vector2> {
        let p = Vector2::new(bits.bit_double()?, bits.bit_double()?);
        (p.x.is_finite() && p.y.is_finite() && bits.bit_double()? == 0.0).then_some(p)
    };
    // The origin keeps the view's elevation (0 for a section view).
    let anchor = |bits: &mut Bits| -> Option<Vector2> {
        let p = Vector2::new(bits.bit_double()?, bits.bit_double()?);
        (p.x.is_finite() && p.y.is_finite() && bits.bit_double()?.is_finite()).then_some(p)
    };
    let origin = anchor(&mut bits)?;
    // The identity, then `BL n` sections (a view of a line without a
    // section has `BL 0`, two bits).
    for _ in 0..9 {
        bits.bit_double()?;
    }
    let items = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= 1000)?;
    bits.skip(5 * items)?;
    for _ in 0..bits.bit_long()? {
        (bits.bit_long()? == 0).then_some(())?;
        bits.bit_long()?;
        bits.bit_double()?;
        bits.bit_long()?;
        bits.bit_double()?;
        bits.bit_long()?;
        bits.bit_double()?;
        bits.bit_double()?;
    }
    bits.skip(1)?;
    (anchor(&mut bits)? == origin).then_some(())?;
    bits.bit_long()?;
    bits.bit_long()?;
    let (low, _, high) = (point(&mut bits)?, point(&mut bits)?, point(&mut bits)?);
    (high.x > low.x && high.y > low.y).then_some(())?;
    for _ in 0..3 {
        point(&mut bits)?;
    }
    bits.bit_long()?;
    let title = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 8 <= bits.remaining())?;
    bits.skip(8 * title)?;
    (bits.bit_long()? == 0).then_some(())?;
    for _ in 0..7 {
        bits.bit_double()?;
    }
    for _ in 0..6 {
        bits.bit_long()?;
    }
    (anchor(&mut bits)? == origin).then_some(())?;
    for _ in 0..4 {
        bits.bit_double()?;
    }
    let vertical_scale = bits.bit_double().filter(|v| v.is_finite() && *v > 0.0)?;
    Some(SectionViewFrame {
        origin,
        offset_min: low.x,
        offset_max: high.x,
        elevation_min: low.y,
        elevation_max: high.y,
        vertical_scale,
    })
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

    #[test]
    fn reads_a_mass_haul_lines_balance_curve() {
        // Head, two GUIDs, the 127 zero bits, BD 500, BL 1, BL 0, then
        // (0, 0) and (25, -641.5).
        let raw = |v: f64| v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>();
        let s = format!(
            "0101010000 10 0101010000 0101010000 10 10 10 00 10 {} 11 {} {} 00{} 0100000001 10
             0100000010 10 10 10 00{} 00{} 10",
            "0".repeat(128),
            "01".repeat(64),
            "0".repeat(127),
            raw(500.0),
            raw(25.0),
            raw(-641.5)
        );
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let read = mass_haul_points(&b, 0, len).unwrap();
        assert_eq!(read, vec![Vector2::new(0.0, 0.0), Vector2::new(25.0, -641.5)]);
        // A record with another value where BL 1 stands is not this layout.
        let other = s.replacen("0100000001 10
", "0100000010 10
", 1);
        let b = bits_of(&other);
        assert_eq!(mass_haul_points(&b, 0, len), None);
    }
}
