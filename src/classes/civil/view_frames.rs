//! View frames and match lines of a plan production run.
//!
//! Geometry: the frame and sheet records hold their rectangles in the
//! **graphic-image section** — the `B`-flagged byte block of the common
//! entity preamble the common entity reader isolates (the container the
//! parcel segments read, upstream PR #591; here proven across a rotated
//! view frame and four sheets of `CH Beispielprojekt 2017.dwg`). The
//! section is a proxy-graphics metafile (ProxyGraphics):
//!
//! ```text
//! u32 section byte length, u32 record count, then that many records:
//!     u32 record byte length, u32 type, type-specific data
//!     type 7 (polygon): u32 count, then count × (RD x, RD y, RD z)
//!     (other types seen — 0x0c, 0x0e, 0x10, 0x12, 0x22 — are colour,
//!     layer, linetype and similar trait records; they are skipped)
//! ```
//!
//! The plan-production class bodies (the view frame group `AECC_PLANPROD`,
//! `AECC_VIEWFRAME`, `AECC_SHEET`) open their class stream with the same
//! head as the parcel segments (upstream PR #591) — `BL 80, 0, 80, 80, 0`,
//! before touching the section. The
//! group record's own section carries no point chain: it holds no
//! geometry in the sample.
//!
//! A **view frame's** single kind-7 chain is its frame rectangle walked
//! around its four corners, rotated to the alignment: opposite edges are
//! equal bit for bit, and the corners' axis-aligned bounds are the live
//! API's `GeometricExtents` exactly — the API exposes only those bounds,
//! the record the rotated corners themselves.
//!
//! A **sheet's** two kind-7 chains are axis-aligned rectangles at a
//! common elevation: the second is the sheet rectangle — its bounds are
//! the sheet's `GeometricExtents` to the last digit (checked against the
//! live API for every sheet the drawing exposes) — and the first spans
//! the same elevation band with its station-low edge pulled in (210 of
//! the 1680 m width in the sample). What the first rectangle bounds the
//! API does not expose, so it is reported without a name.
//!
//! Point chains ride both point-chain record kinds the metafile defines:
//! the CH sample's frames and sheets carry kind-7 polygons, the tutorial
//! corpus' frames and match lines (`Plan Production-Plan Profile
//! Sheets-Create.dwg`) kind-6 polylines — same payload either way.
//!
//! **A match line's** single chain is its rectangle walked around its
//! four corners at a common elevation and closed back onto the first; its
//! bounds are the live API's `GeometricExtents` exactly. A view frame's
//! chain may be closed the same way.

use super::bits::Bits;
use crate::classes::arch::polyline_from_type6;
use crate::entities::{ProxyGraphicRecord, ProxyGraphics};
use crate::types::Vector3;

/// The plan-production class bodies' shared schema word — the parcel
/// segments' word (upstream PR #591); `AECC_PLANPROD`, `AECC_VIEWFRAME`
/// and `AECC_SHEET` open their class stream with the same head.
const SCHEMA: u32 = 0x1002_5F04;

/// One view frame of a view frame group, with the match lines on its
/// sides. The frames of a run hang in a chain: the match line after one
/// frame is the one before the next.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewFrame {
    /// The frame's name (`VF - 1`, ...).
    pub name: String,
    /// The view frame group (`AECC_VIEWFRAME_GROUP`) the frame belongs to.
    pub group: u64,
    /// The match line on the frame's station-low side, 0 at the first.
    pub match_line_before: u64,
    /// The match line on the frame's station-high side, 0 at the last.
    pub match_line_after: u64,
}

/// One match line between two view frames.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchLine {
    /// The match line's name (`ML - 1`, ...).
    pub name: String,
    /// The view frame group the match line belongs to.
    pub group: u64,
    /// The frame on the line's station-low side.
    pub view_frame_before: u64,
    /// The frame on the line's station-high side.
    pub view_frame_after: u64,
}

/// A view frame record: its name in the text stream, then its group and
/// side match lines among its references:
///
/// ```text
/// references: owner, owner, style, style, group, match line before
///             (0 at the first frame), match line after (0 at the last)
/// ```
pub(super) fn view_frame(strings: &[String], refs: &[(u64, u8)]) -> Option<ViewFrame> {
    let (group, before, after) = match refs {
        [(.., 4), (.., 3), (.., 5), (.., 5), (group, 5), (before, 4), (after, 4)] => (group, before, after),
        _ => return None,
    };
    Some(ViewFrame {
        name: strings.get(2)?.clone(),
        group: *group,
        match_line_before: *before,
        match_line_after: *after,
    })
}

/// One sheet (`AECC_SHEET`) a plan production run creates, with the view
/// frame group the sheet's frame belongs to.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sheet {
    /// The sheet's label as stored (`Sheet 1 of 14`, ...).
    pub label: String,
    /// The view frame group the sheet's frame belongs to.
    pub group: u64,
}

/// A sheet record: its label in the text stream (fourth string), then its
/// group among its references:
///
/// ```text
/// references: owner, owner, group
/// ```
pub(super) fn sheet(strings: &[String], refs: &[(u64, u8)]) -> Option<Sheet> {
    let group = match refs {
        [(.., 4), (.., 3), (group, 5), ..] => *group,
        _ => return None,
    };
    Some(Sheet { label: strings.get(3)?.clone(), group })
}

/// A match line record:
///
/// ```text
/// references: view frame before, group, view frame after, owner, owner,
///             style
/// ```
pub(super) fn match_line(strings: &[String], refs: &[(u64, u8)]) -> Option<MatchLine> {
    let (before, group, after) = match refs {
        [(before, 4), (group, 5), (after, 4), (.., 4), (.., 3), (.., 5)] => (before, group, after),
        _ => return None,
    };
    Some(MatchLine {
        name: strings.get(2)?.clone(),
        group: *group,
        view_frame_before: *before,
        view_frame_after: *after,
    })
}

/// The rectangle a view frame record holds: its frame's four corners in
/// record order, rotated to the alignment (see the [module](self)
/// documentation).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewFrameGeometry {
    /// The frame's corners walked around the rectangle; opposite edges are
    /// equal bit for bit, and the corners' axis-aligned bounds are the
    /// live API's `GeometricExtents`.
    pub corners: [Vector3; 4],
}

/// The rectangles a sheet record holds (see the [module](self)
/// documentation).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SheetGeometry {
    /// The sheet rectangle: its corners' axis-aligned bounds are the
    /// sheet's live `GeometricExtents` to the last digit.
    pub extents: [Vector3; 4],
    /// The record's first rectangle — the same elevation band, its
    /// station-low edge pulled in; what it bounds the API does not expose.
    pub inner: [Vector3; 4],
}

/// The rectangle a match line record holds (see the [module](self)
/// documentation).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MatchLineGeometry {
    /// The match line's rectangle walked around its four corners, rotated
    /// with its frame; the corners' axis-aligned bounds are the live API's
    /// `GeometricExtents` exactly.
    pub corners: [Vector3; 4],
}

/// The plan-production class bodies' shared opening — the parcel
/// segments' head (upstream PR #591) with the plan-production schema
/// word (see the [module](self) documentation).
fn plan_production_head(bits: &mut Bits) -> Option<()> {
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    (bits.bit_long()? as u32 == SCHEMA).then_some(())?;
    Some(())
}

/// The graphic-image section (a proxy-graphics metafile) point-chain
/// records' polygons, in record order: the kind-6 polylines and kind-7
/// polygons the metafile defines (same `u32` count + XYZ payload — see
/// the [module](self) documentation); `None` unless the metafile decodes.
fn point_chains(g: &[u8]) -> Option<Vec<Vec<Vector3>>> {
    ProxyGraphics::decode(g)?
        .records
        .iter()
        .filter_map(|record| match record {
            ProxyGraphicRecord::Unknown { record_type: 6, data }
            | ProxyGraphicRecord::Unknown { record_type: 7, data } => Some(polyline_from_type6(data)),
            _ => None,
        })
        .collect()
}

/// One axis-aligned rectangle walked around its four corners, at one
/// elevation: low, low-right, high, high-left.
fn axis_aligned_rectangle(chain: &[Vector3]) -> Option<[Vector3; 4]> {
    let [a, b, c, d] = chain else { return None };
    (a.z == b.z
        && b.z == c.z
        && c.z == d.z
        && a.x == d.x
        && b.x == c.x
        && a.y == b.y
        && c.y == d.y
        && a.x <= b.x
        && a.y <= d.y)
        .then_some([*a, *b, *c, *d])
}

/// A rectangle walked around its four corners at one elevation, possibly
/// rotated: four points, or five whose last closes back onto the first.
/// Opposite edges are equal bit for bit and the corner is right to 1e-9
/// of the edge products (1.5e-12 in the sample).
fn rectangle(chain: Vec<Vector3>) -> Option<[Vector3; 4]> {
    let [a, b, c, d] = match <[Vector3; 5]>::try_from(chain) {
        Ok([a, b, c, d, closing]) => (closing == a).then_some([a, b, c, d])?,
        Err(chain) => <[Vector3; 4]>::try_from(chain).ok()?,
    };
    let dot = (b.x - a.x) * (c.x - b.x) + (b.y - a.y) * (c.y - b.y);
    let edges = ((b.x - a.x).hypot(b.y - a.y)) * ((c.x - b.x).hypot(c.y - b.y));
    (a.z == b.z
        && b.z == c.z
        && c.z == d.z
        && b.x - a.x == c.x - d.x
        && b.y - a.y == c.y - d.y
        && c.x - b.x == d.x - a.x
        && c.y - b.y == d.y - a.y
        && dot.abs() <= 1e-9 * edges)
        .then_some([a, b, c, d])
}

/// A view frame record's frame rectangle: `graphic` is the entity's
/// graphic-image section, `data`/`start`/`end` its class stream.
pub(super) fn view_frame_geometry(data: &[u8], start: usize, end: usize, graphic: &[u8]) -> Option<ViewFrameGeometry> {
    let mut bits = Bits::new(data, start, end);
    plan_production_head(&mut bits)?;
    let chains = point_chains(graphic)?;
    let [chain]: [_; 1] = chains.try_into().ok()?;
    Some(ViewFrameGeometry { corners: rectangle(chain)? })
}

/// A sheet record's rectangles: `graphic` is the entity's graphic-image
/// section, `data`/`start`/`end` its class stream.
pub(super) fn sheet_geometry(data: &[u8], start: usize, end: usize, graphic: &[u8]) -> Option<SheetGeometry> {
    let mut bits = Bits::new(data, start, end);
    plan_production_head(&mut bits)?;
    let chains = point_chains(graphic)?;
    let [inner, extents]: [Vec<Vector3>; 2] = chains.try_into().ok()?;
    let inner = axis_aligned_rectangle(&inner)?;
    let extents = axis_aligned_rectangle(&extents)?;
    // The sheet rectangle encloses the first one; in the sample three
    // edges are shared and the station-low edge is pulled in.
    (inner[0].z == extents[0].z
        && inner[0].x >= extents[0].x
        && inner[0].y >= extents[0].y
        && inner[2].x <= extents[2].x
        && inner[2].y <= extents[2].y)
        .then_some(SheetGeometry { extents, inner })
}

/// A match line record's rectangle: `graphic` is the entity's
/// graphic-image section, `data`/`start`/`end` its class stream.
pub(super) fn match_line_geometry(data: &[u8], start: usize, end: usize, graphic: &[u8]) -> Option<MatchLineGeometry> {
    let mut bits = Bits::new(data, start, end);
    plan_production_head(&mut bits)?;
    let chains = point_chains(graphic)?;
    let [chain]: [_; 1] = chains.try_into().ok()?;
    // A match line's walk is always closed: five points.
    (chain.len() == 5).then_some(())?;
    Some(MatchLineGeometry { corners: rectangle(chain)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plan-production class body opening (shared head) as bytes.
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
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        bits.as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect()
    }

    /// A graphic-image section over the given point chains, plus an
    /// unparsed four-byte filler chain (kind 0x10) in between; `kind` is
    /// the point-chain record kind (6 polyline, 7 polygon).
    fn section_of_kind(kind: u32, chains: &[&[(f64, f64, f64)]], filler: bool) -> Vec<u8> {
        let mut g = Vec::new();
        let put_u32 = |g: &mut Vec<u8>, v: u32| g.extend(v.to_le_bytes());
        let put_f64 = |g: &mut Vec<u8>, v: f64| g.extend(v.to_le_bytes());
        put_u32(&mut g, 0); // section length, patched below
        put_u32(&mut g, chains.len() as u32 + u32::from(filler));
        if filler {
            put_u32(&mut g, 12);
            put_u32(&mut g, 0x10);
            put_u32(&mut g, 0x4c);
        }
        for chain in chains {
            put_u32(&mut g, (12 + 24 * chain.len()) as u32);
            put_u32(&mut g, kind);
            put_u32(&mut g, chain.len() as u32);
            for &(x, y, z) in *chain {
                put_f64(&mut g, x);
                put_f64(&mut g, y);
                put_f64(&mut g, z);
            }
        }
        let len = g.len() as u32;
        g[0..4].copy_from_slice(&len.to_le_bytes());
        g
    }

    /// A graphic-image section of kind-7 polygons (the CH sample's
    /// point-chain record kind).
    fn section(chains: &[&[(f64, f64, f64)]], filler: bool) -> Vec<u8> {
        section_of_kind(7, chains, filler)
    }

    /// A rotated rectangle: 830 m at 18.43 degrees, 287 m wide, around the
    /// sample's low corner.
    fn rotated_corners() -> Vec<(f64, f64, f64)> {
        [
            (5518791.420847342, 4351596.896317183, 0.0),
            (5519578.861366005, 4351859.265197898, 0.0),
            (5519669.584099456, 4351586.9815486735, 0.0),
            (5518882.143580793, 4351324.612667958, 0.0),
        ]
        .into()
    }

    #[test]
    fn reads_a_frames_rotated_rectangle() {
        let class = class_stream();
        let g = section(&[&rotated_corners()], false);
        let at = view_frame_geometry(&class, 0, class.len() * 8, &g).expect("geometry decodes");
        assert_eq!(at.corners[0], Vector3::new(5518791.420847342, 4351596.896317183, 0.0));
        assert_eq!(at.corners[2], Vector3::new(5519669.584099456, 4351586.9815486735, 0.0));
    }

    #[test]
    fn a_frame_needs_its_right_angle() {
        let class = class_stream();
        let mut corners = rotated_corners();
        corners[2].1 += 1.0; // no longer a rectangle
        let g = section(&[&corners], false);
        assert_eq!(view_frame_geometry(&class, 0, class.len() * 8, &g), None);
    }

    #[test]
    fn reads_a_sheets_two_rectangles() {
        let class = class_stream();
        let inner = [
            (5522166.0, 4350250.0, 273.4),
            (5523636.0, 4350250.0, 273.4),
            (5523636.0, 4350844.0, 273.4),
            (5522166.0, 4350844.0, 273.4),
        ];
        let extents = [
            (5521956.012249335, 4350250.0, 273.4),
            (5523636.012249335, 4350250.0, 273.4),
            (5523636.012249335, 4350844.0, 273.4),
            (5521956.012249335, 4350844.0, 273.4),
        ];
        // An unparsed filler chain sits between the record's chains.
        let g = section(&[&inner, &extents], true);
        let at = sheet_geometry(&class, 0, class.len() * 8, &g).expect("geometry decodes");
        assert_eq!(at.extents[0], Vector3::new(5521956.012249335, 4350250.0, 273.4));
        assert_eq!(at.inner[0], Vector3::new(5522166.0, 4350250.0, 273.4));
    }

    #[test]
    fn a_sheet_with_one_chain_is_not_a_sheet_geometry() {
        let class = class_stream();
        let g = section(&[&[(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 1.0, 0.0)]], false);
        assert_eq!(sheet_geometry(&class, 0, class.len() * 8, &g), None);
    }

    /// The tutorial drawing's first match line (`ML - 1` of `Plan
    /// Production-Plan Profile Sheets-Create.dwg`): a closed axis-aligned
    /// rectangle carried in a kind-6 polyline, whose bounds are the live
    /// API's `GeometricExtents` bit for bit.
    fn match_line_chain() -> Vec<(f64, f64, f64)> {
        [
            (979164.0938625354, 178278.73408678515, 0.0),
            (979528.6093829795, 178278.73408678515, 0.0),
            (979528.6093829795, 178653.52285458875, 0.0),
            (979164.0938625354, 178653.52285458875, 0.0),
            (979164.0938625354, 178278.73408678515, 0.0),
        ]
        .into()
    }

    #[test]
    fn reads_a_match_lines_closed_rectangle() {
        let class = class_stream();
        let g = section_of_kind(6, &[&match_line_chain()], false);
        let at = match_line_geometry(&class, 0, class.len() * 8, &g).expect("geometry decodes");
        assert_eq!(at.corners[0], Vector3::new(979164.0938625354, 178278.73408678515, 0.0));
        assert_eq!(at.corners[2], Vector3::new(979528.6093829795, 178653.52285458875, 0.0));
    }

    #[test]
    fn a_match_line_chain_must_close() {
        let class = class_stream();
        let mut open = match_line_chain();
        open.truncate(4); // the walk does not return to the low corner
        let g = section_of_kind(6, &[&open], false);
        assert_eq!(match_line_geometry(&class, 0, class.len() * 8, &g), None);
    }

    #[test]
    fn reads_a_rotated_match_line_and_a_closed_frame() {
        let class = class_stream();
        let mut closed = rotated_corners();
        closed.push(closed[0]);
        let g = section_of_kind(6, &[&closed], false);
        let at = match_line_geometry(&class, 0, class.len() * 8, &g).expect("rotated match line");
        assert_eq!(at.corners[1], Vector3::new(5519578.861366005, 4351859.265197898, 0.0));
        let at = view_frame_geometry(&class, 0, class.len() * 8, &g).expect("closed frame");
        assert_eq!(at.corners[3], Vector3::new(5518882.143580793, 4351324.612667958, 0.0));
        // A walk that does not close, or leaves its elevation, is refused.
        let mut open = closed.clone();
        open[4].0 += 1.0;
        let g = section_of_kind(6, &[&open], false);
        assert_eq!(match_line_geometry(&class, 0, class.len() * 8, &g), None);
        let mut lifted = closed;
        lifted[2].2 = 1.0;
        let g = section_of_kind(6, &[&lifted], false);
        assert_eq!(match_line_geometry(&class, 0, class.len() * 8, &g), None);
    }

    #[test]
    fn a_skewed_chain_is_not_a_match_line_rectangle() {
        let class = class_stream();
        let mut skewed = match_line_chain();
        skewed[1].1 += 1.0; // the low edge leaves the axis
        let g = section_of_kind(6, &[&skewed], false);
        assert_eq!(match_line_geometry(&class, 0, class.len() * 8, &g), None);
    }

    #[test]
    fn a_frame_also_reads_from_a_kind_6_chain() {
        // The tutorial corpus carries its view frames' rectangles in
        // kind-6 polylines, not the CH sample's kind-7 polygons.
        let class = class_stream();
        let g = section_of_kind(6, &[&rotated_corners()], false);
        let at = view_frame_geometry(&class, 0, class.len() * 8, &g).expect("geometry decodes");
        assert_eq!(at.corners[0], Vector3::new(5518791.420847342, 4351596.896317183, 0.0));
    }

    #[test]
    fn another_schema_word_is_not_a_plan_production_record() {
        let mut bits = String::new();
        let bl = |v: u32, bits: &mut String| {
            bits.push_str("00");
            for b in v.to_le_bytes() {
                bits.push_str(&format!("{b:08b}"));
            }
        };
        for v in [80, 0, 80, 80, 0, SCHEMA ^ 1] {
            bl(v, &mut bits);
        }
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        let g = section(&[&rotated_corners()], false);
        assert_eq!(view_frame_geometry(&data, 0, data.len() * 8, &g), None);
        assert_eq!(sheet_geometry(&data, 0, data.len() * 8, &g), None);
        let g6 = section_of_kind(6, &[&match_line_chain()], false);
        assert_eq!(match_line_geometry(&data, 0, data.len() * 8, &g6), None);
    }

    #[test]
    fn chains_must_tile_the_section() {
        let class = class_stream();
        let mut g = section(&[&rotated_corners()], false);
        g.truncate(g.len() - 8); // the section length no longer matches
        assert_eq!(view_frame_geometry(&class, 0, class.len() * 8, &g), None);
    }
}
