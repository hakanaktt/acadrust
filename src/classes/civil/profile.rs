//! Profiles (vertical alignments) and profile views.
//!
//! A design profile links its tangents by id (`BL id, BL previous, BL next`
//! then `2RD start, 2RD end` in station/elevation) and keeps each vertical
//! curve as a record naming the tangent it leads into, with its length. A
//! surface profile stores its sampled segments as `2RD start, 2RD end`
//! pairs that chain end to start. A profile view stores its station and
//! elevation range as three 3BD points `(st0, z0, 0) (st0, z0, 0)
//! (st1, z1, 0)`; its proxy graphics hold the frame rectangle.

use super::bits::Bits;
use crate::types::Vector2;

/// A design profile: its tangents in order and the vertical curve length
/// at each joint (0 where the tangents meet without a curve).
struct Design {
    tangents: Vec<(Vector2, Vector2)>,
    curves: Vec<f64>,
}

/// Every profile entity record opens with a 2-bit kind, then `00000001`,
/// sixteen zero bits and `10`.
const RECORD_TANGENT: u32 = 0b01;
const RECORD_CURVE: u32 = 0b11;

/// Kind of the record opening at `pos`, with the position after its prefix.
fn record_kind(data: &[u8], pos: usize, end: usize) -> Option<(u32, usize)> {
    let mut bits = Bits::new(data, pos, end);
    let mut prefix = 0u32;
    for _ in 0..28 {
        prefix = (prefix << 1) | u32::from(bits.bit()?);
    }
    ((prefix & 0x03FF_FFFF) == 0b0000_0001_0000_0000_0000_0000_10).then_some((prefix >> 26, bits.pos))
}

/// Tangent record body: `BL id, BL previous, BL next, RD station,
/// RD elevation, RD station, RD elevation`.
fn tangent_at(data: &[u8], pos: usize, end: usize) -> Option<(i32, i32, i32, Vector2, Vector2)> {
    let mut bits = Bits::new(data, pos, end);
    let id = bits.bit_long()?;
    let previous = bits.bit_long()?;
    let next = bits.bit_long()?;
    let mut value = || bits.raw_double().filter(|v| v.is_finite() && (*v == 0.0 || v.abs() > 1e-6));
    let a = Vector2::new(value()?, value()?);
    let b = Vector2::new(value()?, value()?);
    let plausible = a.x.abs() < 1e7 && b.x.abs() < 1e7 && a.y.abs() < 1e5 && b.y.abs() < 1e5 && b.x >= a.x;
    (id > 0 && plausible).then_some((id, previous, next, a, b))
}

/// Curve record body: `BL id, BL previous, BL next, 4 bits, RD length`.
fn curve_at(data: &[u8], pos: usize, end: usize) -> Option<(i32, i32, i32, f64)> {
    let mut bits = Bits::new(data, pos, end);
    let id = bits.bit_long()?;
    let previous = bits.bit_long()?;
    let next = bits.bit_long()?;
    for _ in 0..4 {
        bits.bit()?;
    }
    let length = bits.raw_double().filter(|v| v.is_finite() && *v > 1e-6 && *v < 1e6)?;
    (id > 0).then_some((id, previous, next, length))
}

/// Entities linked by id from the first tangent: tangents in order, and the
/// curve each joint carries. Tangents and curves share one id space.
fn design(data: &[u8], start: usize, end: usize) -> Option<Design> {
    let mut tangents: Vec<(i32, i32, i32, Vector2, Vector2)> = Vec::new();
    let mut curves: Vec<(i32, i32, i32, f64)> = Vec::new();
    for pos in start..end {
        match record_kind(data, pos, end) {
            Some((RECORD_TANGENT, body)) => tangents.extend(tangent_at(data, body, end)),
            Some((RECORD_CURVE, body)) => curves.extend(curve_at(data, body, end)),
            _ => {}
        }
    }
    // A stray prefix can carry plausible ids; a tangent belongs to the chain
    // only where it starts at its predecessor's end.
    let follow = |first: (i32, i32, i32, Vector2, Vector2)| {
        let mut chain = vec![first];
        let mut joints = Vec::new();
        loop {
            let last = *chain.last().unwrap();
            let (next_id, length) = match curves.iter().find(|c| c.0 == last.2 && c.1 == last.0) {
                Some(curve) => (curve.2, curve.3),
                None => (last.2, 0.0),
            };
            if next_id == 0 {
                break;
            }
            match tangents.iter().find(|r| r.0 == next_id && r.3.distance(&last.4) <= 1e-6) {
                Some(next) if !chain.iter().any(|c| c.0 == next.0) => {
                    chain.push(*next);
                    joints.push(length);
                }
                _ => break,
            }
        }
        (chain, joints)
    };
    let (chain, joints) = tangents
        .iter()
        .filter(|r| r.1 == 0)
        .map(|r| follow(*r))
        .max_by_key(|(chain, _)| chain.len())?;
    Some(Design {
        tangents: chain.iter().map(|r| (r.3, r.4)).collect(),
        curves: joints,
    })
}

/// Point on a tangent at `station`.
fn on(tangent: (Vector2, Vector2), station: f64) -> Vector2 {
    let (a, b) = tangent;
    if (b.x - a.x).abs() < 1e-12 {
        return a;
    }
    Vector2::new(station, a.y + (b.y - a.y) * (station - a.x) / (b.x - a.x))
}

impl Design {
    /// Tangent pieces trimmed by the curves, and the curves sampled as
    /// symmetric parabolas, in station order.
    fn pieces(&self) -> Vec<(ProfilePiece, Vec<Vector2>)> {
        let mut out = Vec::new();
        for (index, &(a, b)) in self.tangents.iter().enumerate() {
            let cut_in = index.checked_sub(1).map_or(0.0, |i| self.curves[i] / 2.0);
            let cut_out = self.curves.get(index).map_or(0.0, |l| l / 2.0);
            let from = on((a, b), a.x + cut_in);
            let to = on((a, b), b.x - cut_out);
            // A step (no station length) draws nothing.
            if to.x > from.x + 1e-9 {
                out.push((ProfilePiece::Tangent, vec![from, to]));
            }
            if cut_out > 0.0 {
                let next = self.tangents[index + 1];
                let pvi = b;
                let p0 = on((a, b), b.x - cut_out);
                let p2 = on(next, next.0.x + cut_out);
                let curve = (0..=32)
                    .map(|i| {
                        let t = i as f64 / 32.0;
                        let u = 1.0 - t;
                        p0 * (u * u) + pvi * (2.0 * u * t) + p2 * (t * t)
                    })
                    .collect();
                out.push((ProfilePiece::Parabola, curve));
            }
        }
        out
    }
}

/// Runs of a surface (sampled) profile: `2RD start, 2RD end` segments that
/// chain end to start.
fn tangent_runs(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vector2>> {
    let mut segments: Vec<(Vector2, Vector2)> = Vec::new();
    let mut pos = start;
    while pos + 256 <= end {
        match segment_at(data, pos, end) {
            Some(segment) => {
                segments.push(segment);
                pos += 256;
            }
            None => pos += 1,
        }
    }
    // Keep a segment only when it chains to a neighbor; a lone match is noise.
    let linked = |s: &(Vector2, Vector2)| segments.iter().any(|o| o.0 == s.1 || o.1 == s.0);
    let mut kept: Vec<(Vector2, Vector2)> = segments.iter().copied().filter(linked).collect();
    kept.sort_by(|a, b| a.0.x.total_cmp(&b.0.x));
    kept.dedup();
    let mut runs: Vec<Vec<Vector2>> = Vec::new();
    for (a, b) in kept {
        match runs.last_mut() {
            Some(run) if *run.last().unwrap() == a => run.push(b),
            Some(run) if run.last().unwrap().x > a.x => {} // overlapping copy
            _ => runs.push(vec![a, b]),
        }
    }
    runs
}

/// Tangents and vertical curves of a design profile, or the runs of a
/// surface profile, in station order.
pub(super) fn pieces(data: &[u8], start: usize, end: usize) -> Vec<(ProfilePiece, Vec<Vector2>)> {
    match design(data, start, end) {
        Some(design) => design.pieces(),
        None => tangent_runs(data, start, end)
            .into_iter()
            .map(|run| (ProfilePiece::Tangent, run))
            .collect(),
    }
}

/// Station/elevation polylines of a profile.
pub(super) fn polylines(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vector2>> {
    let mut out: Vec<Vec<Vector2>> = Vec::new();
    for (_, run) in pieces(data, start, end) {
        match out.last_mut() {
            Some(last) if last.last().is_some_and(|p| p.distance(&run[0]) < 1e-9) => last.extend(&run[1..]),
            _ => out.push(run),
        }
    }
    out
}

/// A vertical point of intersection.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pvi {
    /// Station and elevation.
    pub point: Vector2,
    /// Length of the vertical curve at this PVI, 0 without one.
    pub curve_length: f64,
}

/// PVIs of a design profile: its start, every tangent joint with the curve
/// it carries, and its end. Empty for a surface profile.
pub(super) fn pvis(data: &[u8], start: usize, end: usize) -> Vec<Pvi> {
    let Some(design) = design(data, start, end) else {
        return Vec::new();
    };
    let mut out = vec![Pvi { point: design.tangents[0].0, curve_length: 0.0 }];
    for (index, tangent) in design.tangents.iter().enumerate().skip(1) {
        out.push(Pvi { point: tangent.0, curve_length: design.curves[index - 1] });
    }
    out.push(Pvi { point: design.tangents.last().unwrap().1, curve_length: 0.0 });
    out
}

fn segment_at(data: &[u8], pos: usize, end: usize) -> Option<(Vector2, Vector2)> {
    let mut bits = Bits::new(data, pos, end);
    let mut value = || bits.raw_double().filter(|v| v.is_finite() && (*v == 0.0 || v.abs() > 1e-6));
    let a = Vector2::new(value()?, value()?);
    let b = Vector2::new(value()?, value()?);
    let plausible = a.x.abs() < 1e7
        && b.x.abs() < 1e7
        && a.y.abs() < 1e5
        && b.y.abs() < 1e5
        && b.x > a.x
        && b.x - a.x < 1e5
        && ((b.y - a.y) / (b.x - a.x)).abs() < 10.0;
    plausible.then_some((a, b))
}

/// Whether a profile piece is a tangent or a vertical curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProfilePiece {
    Tangent,
    /// Symmetric parabola between two tangents.
    Parabola,
}

/// Station and elevation range of a profile view and how they map to the
/// drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewFrame {
    /// Drawing position of (`station_min`, `elevation_min`).
    pub origin: Vector2,
    pub station_min: f64,
    pub station_max: f64,
    pub elevation_min: f64,
    pub elevation_max: f64,
    /// Drawing units per elevation unit.
    pub vertical_scale: f64,
}

impl ProfileViewFrame {
    /// Drawing position of a station and elevation.
    pub fn to_drawing(&self, station: f64, elevation: f64) -> Vector2 {
        Vector2::new(
            self.origin.x + station - self.station_min,
            self.origin.y + (elevation - self.elevation_min) * self.vertical_scale,
        )
    }
}

/// Range triple whose station span equals the frame width.
pub(super) fn view_frame(
    data: &[u8],
    start: usize,
    end: usize,
    min: Vector2,
    max: Vector2,
) -> Option<ProfileViewFrame> {
    let width = max.x - min.x;
    let height = max.y - min.y;
    (start..end).find_map(|pos| {
        let mut bits = Bits::new(data, pos, end);
        let mut point = || {
            let x = bits.bit_double()?;
            let y = bits.bit_double()?;
            (bits.bit_double()? == 0.0).then_some(Vector2::new(x, y))
        };
        let a = point()?;
        let b = point()?;
        let c = point()?;
        let fits = a == b
            && c.y > a.y
            && ((c.x - a.x) - width).abs() <= 1e-6 * width.max(1.0)
            && height > 0.0;
        fits.then(|| ProfileViewFrame {
            origin: min,
            station_min: a.x,
            station_max: c.x,
            elevation_min: a.y,
            elevation_max: c.y,
            vertical_scale: height / (c.y - a.y),
        })
    })
}
