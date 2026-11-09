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
//! ```
//!
//! The assembly places each shape: a subassembly's origin is a point of
//! the shape it hangs from, or the assembly's baseline point.

use super::bits::Bits;
use crate::types::{Vector2, Vector3};

/// Points and links of a subassembly shape.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SubassemblyShape {
    /// (offset, elevation) of each point.
    pub points: Vec<Vector2>,
    /// Point indices each link runs through.
    pub links: Vec<Vec<u32>>,
}

const MAX_COUNT: i32 = 100_000;

/// First point list that a valid link list follows in `data[start..end]`.
// ponytail: probes every bit offset; parse the class header once its fields
// are known.
pub(super) fn find(data: &[u8], start: usize, end: usize) -> Option<SubassemblyShape> {
    (start..end).find_map(|pos| read_at(data, pos, end))
}

fn count(bits: &mut Bits) -> Option<usize> {
    let n = bits.bit_long()?;
    (1..=MAX_COUNT).contains(&n).then_some(n as usize)
}

fn header(bits: &mut Bits) -> Option<()> {
    let codes = bits.bit_long()?;
    bits.bit()?;
    bits.bit()?;
    (0..64).contains(&codes).then_some(())
}

fn read_at(data: &[u8], pos: usize, end: usize) -> Option<SubassemblyShape> {
    let mut bits = Bits::new(data, pos, end);
    let point_count = count(&mut bits)?;
    let mut points = Vec::with_capacity(point_count.min(1024));
    for _ in 0..point_count {
        header(&mut bits)?;
        (0..64).contains(&bits.bit_long()?).then_some(())?;
        let x = bits.bit_double()?;
        let y = bits.bit_double()?;
        (x.abs() < 1e4 && y.abs() < 1e4).then_some(())?;
        points.push(Vector2::new(x, y));
    }
    let link_count = count(&mut bits)?;
    let mut links = Vec::with_capacity(link_count.min(1024));
    for _ in 0..link_count {
        header(&mut bits)?;
        let n = bits.bit_long()?;
        (1..=64).contains(&n).then_some(())?;
        let mut link = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let index = u32::try_from(bits.raw_long()?).ok()?;
            ((index as usize) < point_count).then_some(())?;
            link.push(index);
        }
        links.push(link);
    }
    Some(SubassemblyShape { points, links })
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

/// Attachment records each open with this tag; a record that holds a
/// point index follows each one that does not.
fn attachment_tag(bits: &mut Bits) -> Option<()> {
    (bits.bit_short()? == 80 && bits.bit_short()? == 80 && bits.bit_short()? == 0).then_some(())?;
    bits.bit()?.then_some(())?;
    (bits.bit_long()? == 240).then_some(())?;
    let flags = [bits.bit()?, bits.bit()?, bits.bit()?, bits.bit()?, bits.bit()?];
    (flags == [false, false, true, false, false]).then_some(())
}

/// Parent point indices in subassembly order.
fn attachment_points(data: &[u8], start: usize, end: usize) -> Vec<u32> {
    let mut points = Vec::new();
    for pos in start..end {
        let mut bits = Bits::new(data, pos, end);
        if attachment_tag(&mut bits).is_none() {
            continue;
        }
        let body = bits.pos;
        if bits.bit_short() == Some(80) {
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
    // Body handles: two nulls, the baseline style, (subassembly, parent)
    // pairs, then the drawing's Civil root.
    let pairs = handles.get(3..handles.len().checked_sub(1)?)?;
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
