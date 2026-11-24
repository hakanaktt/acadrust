//! Survey figures and networks.
//!
//! A survey figure (`AECC_SVFIGURE`, RX class `AeccDbSvFigure`) keeps its
//! name and site in the text stream, and its geometry in two parts:
//!
//! * the **graphic-image section** draws the figure's XY bounding box as a
//!   closed rectangle at two elevations — two five-point rings (one
//!   constant z per ring, opposite windings) and four 2-point vertical
//!   corner connectors, tiling the 688-byte section of the samples. The
//!   ring elevations are the figure's own: on the tutorials' survey
//!   drawings they read back the live application's `MinElevation` /
//!   `MaxElevation` bit for bit (straight, curved, closed and open
//!   figures alike).
//! * the **class body** ends with a cache of the traverse vertices:
//!   an 8-bit count word, two 0 bits, then `count` entries of
//!   (RD x, RD y, RD z) at a 66-bit stride — a closed figure with curves
//!   repeats its first vertex once at the end (the curve closure). The
//!   stream is framed exactly: it ends 17 bits before the class body's
//!   end, so `count = (class_end - 6167) / 198` with no remainder, and the
//!   count word at the stream head confirms it (both checked; the samples'
//!   170 records — Survey-2B and Survey-3, 85 figures each — pass).
//!
//! The cache is a display cache: its x/y values sit within the shared
//! solved-point tolerance of the application's `BaseCurve` vertices
//! (~250 ulps at drawing magnitude; z is exact, save BLDG9's first vertex,
//! which the record stores at the minimum elevation). The arc parameters
//! the curved sides need are not stored anywhere in the record, so curved
//! lengths are the survey engine's, not the record's. The vertices are
//! therefore read verbatim and verified under the shared solved-point
//! tolerance (user ruling 2026-11-25, amending the D012-1 hold that
//! predated the completed grammar mapping); nothing is recomputed here.

use crate::classes::arch::polyline_from_type6;
use crate::entities::{ProxyGraphicRecord, ProxyGraphics};
use crate::types::Vector3;

use super::bits::Bits;

/// A survey figure (`AECC_SVFIGURE`) of a survey network: the figure a
/// survey point chain draws, named by its figure name.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurveyFigure {
    /// The figure's name (`LOT1`, `EP4`, `SHD3`, ...).
    pub name: String,
    /// The name of the survey site the figure belongs to.
    pub site: String,
}

/// A survey figure's name and site (its text stream reads `AeccImpSvFigure`,
/// ``, ``, name, site name).
pub(super) fn figure(strings: &[String]) -> Option<SurveyFigure> {
    Some(SurveyFigure { name: strings.get(3)?.clone(), site: strings.get(4)?.clone() })
        .filter(|f| !f.name.is_empty())
}

/// A survey network's (`AECC_SVNETWORK`) name: its text stream's third
/// string.
pub(super) fn network_name(strings: &[String]) -> Option<String> {
    strings.get(2).filter(|s| !s.is_empty()).cloned()
}

/// Geometry a survey figure record holds: the traverse vertices its class
/// body caches and the elevation range its bbox solid stores.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurveyFigureGeometry {
    /// The traverse vertices as stored, in traverse order. A closed figure
    /// with curves repeats its first vertex once at the end (the curve
    /// closure); an open figure, or a closed one without curves, does not.
    /// Within the shared solved-point tolerance of the live application's
    /// traverse (see the [module](self) documentation).
    pub vertices: Vec<Vector3>,
    /// The figure's lowest elevation — bit for bit the live application's
    /// `MinElevation` (the bbox solid's low ring).
    pub min_elevation: f64,
    /// The figure's highest elevation — the live application's
    /// `MaxElevation` (the bbox solid's high ring).
    pub max_elevation: f64,
}

/// The geometry of a survey figure record: `graphic` is its graphic-image
/// section, `data`/`start`/`end` its class stream.
pub(super) fn geometry(
    data: &[u8],
    start: usize,
    end: usize,
    graphic: &[u8],
) -> Option<SurveyFigureGeometry> {
    let vertices = traverse(data, start, end)?;
    let (min_elevation, max_elevation) = elevations(graphic)?;
    Some(SurveyFigureGeometry { vertices, min_elevation, max_elevation })
}

/// The traverse vertex cache at the class stream's tail: exactly
/// `(end - 6167) / 198` entries at a 66-bit stride from `end - 17`, the
/// count word at the stream head confirming the division (see the
/// [module](self) documentation).
fn traverse(data: &[u8], start: usize, end: usize) -> Option<Vec<Vector3>> {
    let tail = end.checked_sub(6167)?;
    let (count, 0) = (tail / 198, tail % 198) else { return None };
    (count > 0).then_some(())?;
    let stream_start = end - 17 - 198 * count;
    let head = stream_start.checked_sub(10)?.checked_sub(start)?;
    let mut bits = Bits::new(data, start + head, end);
    // the count word: the stream's own entry count, confirming the anchor
    (bits.raw_short_byte()? as usize == count).then_some(())?;
    bits.skip(2)?;
    (0..count)
        .map(|_| {
            let x = bits.raw_double()?;
            bits.skip(2)?;
            let y = bits.raw_double()?;
            bits.skip(2)?;
            let z = bits.raw_double()?;
            bits.skip(2)?;
            Some(Vector3::new(x, y, z))
        })
        .collect()
}

/// The two ring elevations of the figure's min/max-elevation bbox solid:
/// the section's type-6 records must be two closed axis-aligned rectangles
/// of five points (each at one constant z) and four 2-point vertical
/// connectors joining the two z values. Anything else is not a figure
/// solid this module reads (`None`).
fn elevations(g: &[u8]) -> Option<(f64, f64)> {
    let graphics = ProxyGraphics::decode(g)?;
    let mut rings: Vec<Vec<Vector3>> = Vec::new();
    let mut connectors = 0;
    for record in &graphics.records {
        if let ProxyGraphicRecord::Unknown { record_type: 6, data } = record {
            let points = polyline_from_type6(data)?;
            match points.len() {
                5 => {
                    let [p0, p1, p2, p3, p4] = points.as_slice() else { continue };
                    // a closed axis-aligned rectangle of nonzero size: the
                    // first corner repeated, sides alternating between the
                    // two x and the two y edges (either winding)
                    (p4 == p0
                        && p0.z == p1.z
                        && p0.z == p2.z
                        && p0.z == p3.z
                        && p0 != p1
                        && p1 != p2
                        && ((p0.y == p1.y && p1.x == p2.x && p2.y == p3.y && p3.x == p0.x)
                            || (p0.x == p1.x && p1.y == p2.y && p2.x == p3.x && p3.y == p0.y)))
                        .then_some(())?;
                    rings.push(points.clone());
                }
                2 => {
                    // a vertical connector between the two elevations
                    (points[0].x == points[1].x
                        && points[0].y == points[1].y
                        && points[0].z != points[1].z)
                        .then_some(())?;
                    connectors += 1;
                }
                _ => return None,
            }
        }
    }
    (rings.len() == 2 && connectors == 4).then_some(())?;
    let (a, b) = (rings[0][0].z, rings[1][0].z);
    Some(if a <= b { (a, b) } else { (b, a) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A graphic-image section over the extents rectangle `(10, 20)` to
    /// `(30, 60)`, drawn at the two elevations 100 (low ring) and 200
    /// (high ring) with the four corner connectors.
    fn section_bytes() -> Vec<u8> {
        let mut g = Vec::new();
        let put_u32 = |g: &mut Vec<u8>, v: u32| g.extend(v.to_le_bytes());
        let put_f64 = |g: &mut Vec<u8>, v: f64| g.extend(v.to_le_bytes());
        put_u32(&mut g, 0); // section length, patched below
        put_u32(&mut g, 7); // records: the text, two rings, four connectors
        put_u32(&mut g, 0); // text record length, patched below
        put_u32(&mut g, 36);
        put_f64(&mut g, 10.1);
        put_f64(&mut g, 20.1);
        while g.len() < 8 + 160 - 56 {
            put_f64(&mut g, 0.0);
        }
        for unit in "AeccDbSvFigure (AeccSurvey130)\0".encode_utf16() {
            g.extend(unit.to_le_bytes());
        }
        while g.len() < 8 + 160 {
            g.push(0);
        }
        // the bbox solid: low ring, high ring, four corner connectors
        let mut chain = Vec::new();
        let mut rect_at = |chain: &mut Vec<u8>, z: f64| {
            let start = chain.len() as u32;
            chain.extend(12u32.to_le_bytes());
            chain.extend(6u32.to_le_bytes());
            chain.extend(5u32.to_le_bytes());
            for (x, y) in [(10.0, 20.0), (30.0, 20.0), (30.0, 60.0), (10.0, 60.0), (10.0, 20.0)] {
                put_f64(chain, x);
                put_f64(chain, y);
                put_f64(chain, z);
            }
            let len = chain.len() as u32 - start;
            chain[start as usize..start as usize + 4].copy_from_slice(&len.to_le_bytes());
        };
        rect_at(&mut chain, 100.0);
        rect_at(&mut chain, 200.0);
        for (x, y) in [(10.0, 20.0), (30.0, 20.0), (30.0, 60.0), (10.0, 60.0)] {
            let start = chain.len() as u32;
            chain.extend(12u32.to_le_bytes());
            chain.extend(6u32.to_le_bytes());
            chain.extend(2u32.to_le_bytes());
            put_f64(&mut chain, x);
            put_f64(&mut chain, y);
            put_f64(&mut chain, 200.0);
            put_f64(&mut chain, x);
            put_f64(&mut chain, y);
            put_f64(&mut chain, 100.0);
            let len = chain.len() as u32 - start;
            chain[start as usize..start as usize + 4].copy_from_slice(&len.to_le_bytes());
        }
        let start = g.len() as u32;
        g.extend(&chain);
        let len = g.len() as u32;
        g[0..4].copy_from_slice(&len.to_le_bytes());
        let text_len = (start - 8) as u32;
        g[8..12].copy_from_slice(&text_len.to_le_bytes());
        g
    }

    /// A class stream tail carrying a two-vertex traverse (an open
    /// triangle's first two sides, say): head, count word 2, and the
    /// entries, ending 17 bits before the stream's end. The stream starts
    /// at bit 6150 of the returned bytes' frame, i.e. the caller's `end`
    /// is 6167 + 198 * count.
    fn traverse_bytes(points: &[(f64, f64, f64)]) -> (Vec<u8>, usize, usize) {
        let count = points.len() as u32;
        let stream_end = 6150 + 198 * points.len();
        let end = stream_end + 17;
        let mut data = vec![0u8; (end + 7) / 8];
        // the count word, 10 bits before the stream
        let mut put = |pos: usize, v: u64, width: usize| {
            for i in 0..width {
                let bit = if width == 8 {
                    // a raw byte-sized value, MSB first
                    (v >> (7 - i)) & 1
                } else {
                    // little-endian bytes, each MSB first
                    let b = (v >> (8 * (i / 8))) & 0xFF;
                    (b >> (7 - i % 8)) & 1
                };
                let p = pos + i;
                let mask = 1 << (7 - p % 8);
                if bit == 1 {
                    data[p / 8] |= mask;
                } else {
                    data[p / 8] &= !mask;
                }
            }
        };
        put(6140, count as u64, 8);
        let mut p = 6150;
        for (x, y, z) in points {
            for v in [x.to_bits(), y.to_bits(), z.to_bits()] {
                put(p, v, 64);
                p += 64;
                put(p, 0, 2);
                p += 2;
            }
        }
        (data, 0, end)
    }

    #[test]
    fn reads_a_figures_name_and_site() {
        let strings = ["AeccImpSvFigure".to_string(), "".into(), "".into(), "LOT1".into(), "Survey Site".into()];
        let f = figure(&strings).expect("figure decodes");
        assert_eq!(f.name, "LOT1");
        assert_eq!(f.site, "Survey Site");
    }

    #[test]
    fn a_nameless_text_stream_is_no_figure() {
        let bare = ["AeccImpSvFigure".to_string(), "".into(), "".into()];
        assert_eq!(figure(&bare), None);
    }

    #[test]
    fn reads_a_records_geometry() {
        let g = section_bytes();
        let (data, start, end) = traverse_bytes(&[
            (5377.00016316073, 5083.341377950699, 250.84800252883036),
            (5534.662578132203, 5191.0031041018665, 258.82401471588656),
        ]);
        let at = geometry(&data, start, end, &g).expect("geometry decodes");
        assert_eq!(at.vertices.len(), 2);
        assert_eq!(at.vertices[0], Vector3::new(5377.00016316073, 5083.341377950699, 250.84800252883036));
        assert_eq!(at.vertices[1], Vector3::new(5534.662578132203, 5191.0031041018665, 258.82401471588656));
        assert_eq!(at.min_elevation, 100.0);
        assert_eq!(at.max_elevation, 200.0);
    }

    #[test]
    fn a_count_word_that_contradicts_the_anchor_is_no_traverse() {
        let (mut data, start, end) = traverse_bytes(&[(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)]);
        // a wrong count word at the stream head (2 -> 3)
        data[6147 / 8] |= 1 << (7 - 6147 % 8);
        assert_eq!(geometry(&data, start, end, &section_bytes()), None);
    }

    #[test]
    fn a_class_body_without_the_anchor_shape_is_no_geometry() {
        let (data, start, end) = traverse_bytes(&[(1.0, 2.0, 3.0)]);
        // one vertex more than the division allows: the remainder is nonzero
        assert_eq!(geometry(&data, start, end - 1, &section_bytes()), None);
    }

    #[test]
    fn a_section_without_the_solid_shape_is_no_geometry() {
        let (data, start, end) = traverse_bytes(&[(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)]);
        let mut g = section_bytes();
        // drop the last connector: four 2-point chains are the shape
        g.truncate(g.len() - 60);
        assert_eq!(geometry(&data, start, end, &g), None);
    }
}
