//! Parcel segments — the application's lot lines (`AECC_PARCEL_SEGMENT`,
//! RX class `AeccDbLotLine`).
//!
//! This completes the decode leg of the record layout documented in
//! upstream PR #589: the geometry blob whose fixed record offset nothing
//! stated is the entity's **graphic-image section** — the `B`-flagged,
//! bit-long-long-sized byte block of the common entity preamble the
//! common entity reader isolates (no extended data at all in the
//! samples). Everything below hangs off that container and off the
//! class stream's schema word, in record order:
//!
//! ```text
//! class stream: BL 80, 0, 80, 80, 0, BL 0x10025F04, 6 bits 010010, ...
//! graphic-image section: a proxy-graphics metafile (ProxyGraphics),
//!     a type-36 text record (the class name `AeccDbLotLine
//!     (AeccLand130)` at the label point, height 0.2) and a type-6
//!     polyline (u32 count, count x (RD x, RD y, RD z))
//! ```
//!
//! The one kind-6 chain of the samples (Assembly-1b.dwg: all 157
//! records) holds the geometry's **bounding-extents rectangle** walked
//! around its five corners — `(min x, min y)`, `(max x, min y)`,
//! `(max x, max y)`, `(min x, max y)`, the first corner again — at
//! `z` 0. Checked against the live API's every lot line of that
//! drawing (155 two-point lines plus two closed boundary loops of 15
//! and 12 vertices):
//!
//! * a **straight two-point line's endpoints are two opposite corners**
//!   of the rectangle, exactly (bit for bit, both diagonals and both
//!   directions occur; its length is the corner distance to the last
//!   digit), and
//! * an **arc** (bulged) line or a **many-vertex loop** keeps its
//!   vertices inside the rectangle only — the rectangle is the
//!   entity's extents, and the curve itself is not in the record's
//!   mapped parts.
//!
//! The record does not say which shape it holds, so no endpoints are
//! read per record: the corners are what a record gives.
//!
//! Records the codec writes itself (`create::add_lot_line`) carry the
//! same head and schema word but no graphic-image section; they decode
//! to `None` here. A shorter form (same head and schema word, no
//! graphic-image section) is a segment stored without a base curve and
//! also decodes to `None`.

use super::bits::Bits;
use crate::classes::arch::polyline_from_type6;
use crate::entities::{ProxyGraphicRecord, ProxyGraphics};
use crate::types::Vector2;

/// Schema word of the parcel segment records, shared with the grading
/// entities' head ([`super::grading`]).
pub(super) const SCHEMA: u32 = 0x1002_5F04;

/// Geometry a parcel segment record holds.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParcelSegmentGeometry {
    /// Where the parcel's area label sits (in most Assembly-1b.dwg
    /// samples 0.1, 0.1 into the extents from their low corner).
    pub label_point: Vector2,
    /// The bounding-extents rectangle's corners in record order: low,
    /// low-right, high, high-left (a straight two-point line's endpoints
    /// are two opposite corners; see the [module](self) documentation).
    pub corners: [Vector2; 4],
}

/// The geometry of a parcel segment record: `graphic` is its
/// graphic-image section, `data`/`start`/`end` its class stream.
pub(super) fn geometry(data: &[u8], start: usize, end: usize, graphic: &[u8]) -> Option<ParcelSegmentGeometry> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    (bits.bit_long()? as u32 == SCHEMA).then_some(())?;
    section(graphic)
}

/// The record's graphic-image section is a proxy-graphics metafile: a
/// type-36 text record (the class name, at the label point) and a type-6
/// polyline walking the extents rectangle (see the [module](self)
/// documentation).
fn section(g: &[u8]) -> Option<ParcelSegmentGeometry> {
    let graphics = ProxyGraphics::decode(g)?;
    let mut label_point = None;
    let mut corners = None;
    for record in &graphics.records {
        match record {
            ProxyGraphicRecord::UnicodeText(text) => {
                label_point = Some(Vector2::new(text.position.x, text.position.y));
            }
            ProxyGraphicRecord::Unknown { record_type: 6, data } => {
                // The extents rectangle: five corners around it, the first
                // one repeated, walked low, low-right, high, high-left.
                let points = polyline_from_type6(data)?;
                let [p0, p1, p2, p3, p4] = points.as_slice() else { continue };
                let c = [p0, p1, p2, p3].map(|p| Vector2::new(p.x, p.y));
                (p4 == p0
                    && c[0].x == c[3].x
                    && c[1].x == c[2].x
                    && c[0].y == c[1].y
                    && c[2].y == c[3].y
                    && c[0].x <= c[1].x
                    && c[0].y <= c[3].y)
                    .then_some(())?;
                corners = Some(c);
            }
            _ => {}
        }
    }
    Some(ParcelSegmentGeometry { label_point: label_point?, corners: corners? })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A class stream opening (head and schema word) as bytes, MSB first.
    fn class_stream() -> Vec<u8> {
        let mut bits = String::new();
        // BL: 2-bit form, then the value's raw bytes in memory order
        // (little-endian for the 32-bit form).
        let bl = |v: u32, bits: &mut String| match v {
            0 => bits.push_str("10"),
            v if v < 256 => bits.push_str(&format!("01{v:08b}")),
            v => {
                bits.push_str("00");
                for b in v.to_le_bytes() {
                    bits.push_str(&format!("{b:08b}"));
                }
            }
        };
        for v in [80, 0, 80, 80, 0, SCHEMA] {
            bl(v, &mut bits);
        }
        bits.push_str("010010"); // the unkeyed tag
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        bits.as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect()
    }

    /// A graphic-image section over the extents rectangle `(10, 20)` to
    /// `(30, 60)` with its label point at `(10.1, 20.1)`.
    fn section_bytes() -> Vec<u8> {
        let mut g = Vec::new();
        let put_u32 = |g: &mut Vec<u8>, v: u32| g.extend(v.to_le_bytes());
        let put_f64 = |g: &mut Vec<u8>, v: f64| g.extend(v.to_le_bytes());
        put_u32(&mut g, 0); // section length, patched below
        put_u32(&mut g, 2); // records: the text, the rectangle
        put_u32(&mut g, 0); // text record length, patched below
        put_u32(&mut g, 36);
        put_f64(&mut g, 10.1);
        put_f64(&mut g, 20.1);
        while g.len() < 8 + 160 - 56 {
            put_f64(&mut g, 0.0);
        }
        for unit in "AeccDbLotLine (AeccLand130)\0".encode_utf16() {
            g.extend(unit.to_le_bytes());
        }
        while g.len() < 8 + 160 {
            g.push(0);
        }
        // one kind-6 chain: the rectangle, the first corner repeated
        put_u32(&mut g, 12 + 24 * 5);
        put_u32(&mut g, 6);
        put_u32(&mut g, 5);
        for (x, y) in [(10.0, 20.0), (30.0, 20.0), (30.0, 60.0), (10.0, 60.0), (10.0, 20.0)] {
            put_f64(&mut g, x);
            put_f64(&mut g, y);
            put_f64(&mut g, 0.0);
        }
        let len = g.len() as u32;
        g[0..4].copy_from_slice(&len.to_le_bytes());
        let blob = (8 + 160 - 8) as u32;
        g[8..12].copy_from_slice(&blob.to_le_bytes());
        g
    }

    #[test]
    fn reads_a_records_geometry() {
        let class = class_stream();
        let g = section_bytes();
        let at = geometry(&class, 0, class.len() * 8, &g).expect("geometry decodes");
        assert_eq!(at.label_point, Vector2::new(10.1, 20.1));
        assert_eq!(
            at.corners,
            [
                Vector2::new(10.0, 20.0),
                Vector2::new(30.0, 20.0),
                Vector2::new(30.0, 60.0),
                Vector2::new(10.0, 60.0),
            ]
        );
    }

    #[test]
    fn another_schema_word_is_not_a_parcel_segment() {
        let mut bits = String::new();
        for v in [80u32, 0, 80, 80, 0, 0x1002_0004] {
            bits.push_str("00");
            for b in v.to_le_bytes() {
                bits.push_str(&format!("{b:08b}"));
            }
        }
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        assert_eq!(geometry(&data, 0, data.len() * 8, &section_bytes()), None);
    }

    #[test]
    fn a_section_without_its_extents_chain_decodes_to_none() {
        let class = class_stream();
        let mut g = section_bytes();
        // drop the chain header: the walk runs into the section end
        g.truncate(8 + 160 + 4);
        assert_eq!(geometry(&class, 0, class.len() * 8, &g), None);
    }

    #[test]
    fn a_chain_of_the_wrong_shape_decodes_to_none() {
        let class = class_stream();
        let mut g = section_bytes();
        // six corners is not the five-corner rectangle
        let count_at = 8 + 160 + 8;
        g[count_at..count_at + 4].copy_from_slice(&6u32.to_le_bytes());
        assert_eq!(geometry(&class, 0, class.len() * 8, &g), None);
    }
}
