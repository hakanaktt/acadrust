//! Subassembly cross-section shape.
//!
//! A subassembly body keeps the shape its code last generated as a point
//! list followed by a link list, in the subassembly's own offset/elevation
//! frame:
//!
//! ```text
//! BL  point count
//! point count x { BL code count, B, B, BL, BD offset, BD elevation }
//! BL  link count
//! link count x { BL code count, B, B, BL n, n x RL point index }
//! BL  area count
//! area count x { BL code count, B, B, BL n, n x RL link index, BL }
//! ```
//!
//! The codes themselves are strings, read in that order: every point's,
//! then every link's, then every area's.
//!
//! The assembly places each shape: a subassembly's origin is a point of
//! the shape it hangs from, or the assembly's baseline point.

use super::bits::Bits;
use crate::types::{Vector2, Vector3};

/// Points, links and areas of a subassembly shape, with their codes.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubassemblyShape {
    /// (offset, elevation) of each point.
    pub points: Vec<Vector2>,
    /// Point indices each link runs through.
    pub links: Vec<Vec<u32>>,
    /// Link indices bounding each area (shape), in order.
    pub areas: Vec<Vec<u32>>,
    /// Codes of each point, link and area.
    pub point_codes: Vec<Vec<String>>,
    pub link_codes: Vec<Vec<String>>,
    pub area_codes: Vec<Vec<String>>,
}

const MAX_COUNT: i32 = 100_000;

fn count(bits: &mut Bits) -> Option<usize> {
    let n = bits.bit_long()?;
    (1..=MAX_COUNT).contains(&n).then_some(n as usize)
}

/// `BL code count, B, B`; returns the code count.
fn header(bits: &mut Bits) -> Option<usize> {
    let codes = bits.bit_long()?;
    bits.bit()?;
    bits.bit()?;
    (0..64).contains(&codes).then_some(codes as usize)
}

/// Shape plus the code count of every point, link and area.
fn read_at(data: &[u8], pos: usize, end: usize) -> Option<(SubassemblyShape, Vec<usize>, Vec<usize>, Vec<usize>)> {
    let mut bits = Bits::new(data, pos, end);
    let point_count = count(&mut bits)?;
    let mut shape = SubassemblyShape::default();
    let mut point_codes = Vec::with_capacity(point_count.min(1024));
    for _ in 0..point_count {
        point_codes.push(header(&mut bits)?);
        (0..64).contains(&bits.bit_long()?).then_some(())?;
        let x = bits.bit_double()?;
        let y = bits.bit_double()?;
        (x.abs() < 1e4 && y.abs() < 1e4).then_some(())?;
        shape.points.push(Vector2::new(x, y));
    }
    let link_count = count(&mut bits)?;
    let mut link_codes = Vec::with_capacity(link_count.min(1024));
    for _ in 0..link_count {
        link_codes.push(header(&mut bits)?);
        shape.links.push(indices(&mut bits, point_count)?);
    }
    // Areas follow; a body without any keeps an empty list.
    let mut area_codes = Vec::new();
    let mut areas_bits = Bits::new(data, bits.pos, end);
    if let Some(area_count) = areas_bits.bit_long().and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= MAX_COUNT as usize) {
        let mut areas = Vec::new();
        let mut codes = Vec::new();
        let parsed = (0..area_count).all(|_| {
            let Some(k) = header(&mut areas_bits) else { return false };
            let Some(links) = indices(&mut areas_bits, link_count) else { return false };
            if areas_bits.bit_long().is_none() {
                return false;
            }
            codes.push(k);
            areas.push(links);
            true
        });
        if parsed {
            shape.areas = areas;
            area_codes = codes;
        }
    }
    Some((shape, point_codes, link_codes, area_codes))
}

/// `BL n, n x RL index`, every index below `limit`.
fn indices(bits: &mut Bits, limit: usize) -> Option<Vec<u32>> {
    let n = bits.bit_long()?;
    (1..=64).contains(&n).then_some(())?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let index = u32::try_from(bits.raw_long()?).ok()?;
        ((index as usize) < limit).then_some(())?;
        out.push(index);
    }
    Some(out)
}

/// Shape from the first point list a valid link list follows, with codes
/// from `strings`, the record's text stream, where they start two strings
/// after the script assembly name.
// ponytail: probes every bit offset; parse the class header once its fields
// are known.
pub(super) fn find(data: &[u8], start: usize, end: usize, strings: &[String]) -> Option<SubassemblyShape> {
    let (mut shape, points, links, areas) = (start..end).find_map(|pos| read_at(data, pos, end))?;
    let first = strings.iter().position(|s| s == "AeccStockSubassemblyScriptsRC.dll").map(|i| i + 2);
    let mut codes = first.map(|i| strings[i.min(strings.len())..].iter());
    let mut take = |counts: &[usize]| -> Vec<Vec<String>> {
        counts
            .iter()
            .map(|&k| match codes.as_mut() {
                Some(iter) => iter.by_ref().take(k).cloned().collect(),
                None => Vec::new(),
            })
            .collect()
    };
    shape.point_codes = take(&points);
    shape.link_codes = take(&links);
    shape.area_codes = take(&areas);
    Some(shape)
}

/// Where a subassembly sits in its assembly.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Attachment {
    pub subassembly: u64,
    /// Subassembly it hangs from, 0 for the assembly baseline.
    pub parent: u64,
    /// Index into the parent shape's points; its origin is that point.
    pub point: u32,
}

/// Baseline point and subassembly tree of an assembly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssemblyLayout {
    /// Drawing position of the baseline point.
    pub origin: Vector3,
    pub attachments: Vec<Attachment>,
}

/// Attachment records each open with this tag (`v` is 70 in Civil 3D
/// 2013 drawings, 80 from 2018); a record that holds a point index follows
/// each one that does not. Returns `v`.
fn attachment_tag(bits: &mut Bits) -> Option<i16> {
    let v = bits.bit_short()?;
    (matches!(v, 70 | 80) && bits.bit_short()? == v && bits.bit_short()? == 0).then_some(())?;
    // `B 1, BL 240` from 2018; Civil 3D 2013 writes `B 0` and `00`
    // followed by the byte 0x40 in the same place.
    let set = bits.bit()?;
    let code = (u8::from(bits.bit()?) << 1) | u8::from(bits.bit()?);
    let byte = bits.raw_short_byte()?;
    (matches!((set, code, byte), (true, 1, 240) | (false, 0, 0x40))).then_some(())?;
    let flags = [bits.bit()?, bits.bit()?, bits.bit()?, bits.bit()?, bits.bit()?];
    (flags == [false, false, true, false, false]).then_some(v)
}

/// Parent point indices in subassembly order.
fn attachment_points(data: &[u8], start: usize, end: usize) -> Vec<u32> {
    let mut points = Vec::new();
    for pos in start..end {
        let mut bits = Bits::new(data, pos, end);
        let Some(tag) = attachment_tag(&mut bits) else {
            continue;
        };
        let body = bits.pos;
        if bits.bit_short() == Some(tag) {
            continue;
        }
        bits.pos = body;
        if let Some(index) = bits.bit_long().and_then(|v| u32::try_from(v).ok()) {
            points.push(index);
        }
    }
    points
}

/// `bounds` is the proxy extents; the baseline point is the first `3BD`
/// with z = 0 inside them.
pub(super) fn assembly_layout(
    data: &[u8],
    start: usize,
    end: usize,
    handles: &[u64],
    bounds: (Vector2, Vector2),
) -> Option<AssemblyLayout> {
    let (min, max) = bounds;
    let origin = (start..end).find_map(|pos| {
        let mut bits = Bits::new(data, pos, end);
        let x = bits.bit_double()?;
        let y = bits.bit_double()?;
        let inside = x > min.x && x < max.x && y > min.y && y < max.y;
        (inside && bits.bit_double()? == 0.0).then(|| Vector3::new(x, y, 0.0))
    })?;
    // Body handles: two nulls (one in Civil 3D 2013), the baseline style,
    // (subassembly, parent) pairs, then the drawing's Civil root.
    let style = handles.iter().take(3).position(|h| *h != 0)?;
    let pairs = handles.get(style + 1..handles.len().checked_sub(1)?)?;
    let points = attachment_points(data, start, end);
    if pairs.len() % 2 != 0 || pairs.len() / 2 != points.len() {
        return None;
    }
    let attachments = pairs
        .chunks(2)
        .zip(points)
        .map(|(pair, point)| Attachment {
            subassembly: pair[0],
            parent: pair[1],
            point,
        })
        .collect();
    Some(AssemblyLayout { origin, attachments })
}
