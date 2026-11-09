//! Profiles (vertical alignments) and profile views.
//!
//! A profile body stores its tangents as `2RD start, 2RD end` pairs in
//! (station, elevation); consecutive tangents of a surface profile share
//! their end points, and a design profile leaves a gap where a vertical
//! curve joins two tangents. A profile view stores its station and
//! elevation range as three 3BD points `(st0, z0, 0) (st0, z0, 0)
//! (st1, z1, 0)`; its proxy graphics hold the frame rectangle.

use super::bits::Bits;
use crate::types::Vector2;

/// Tangent runs of a profile, before vertical curves join them.
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
    let linked = |s: &(Vector2, Vector2)| {
        segments
            .iter()
            .any(|o| o.0 == s.1 || o.1 == s.0)
    };
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

/// Station/elevation polylines of a profile.
pub(super) fn polylines(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vector2>> {
    join_with_vertical_curves(tangent_runs(data, start, end))
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

/// PVIs of a design profile: its ends, the vertices inside a tangent run,
/// and the tangent intersections where a vertical curve joins two runs.
pub(super) fn pvis(data: &[u8], start: usize, end: usize) -> Vec<Pvi> {
    let runs = tangent_runs(data, start, end);
    let mut out: Vec<Pvi> = Vec::new();
    for (index, run) in runs.iter().enumerate() {
        if index == 0 {
            out.push(Pvi { point: run[0], curve_length: 0.0 });
        } else if let Some(pvi) = curve_pvi(&runs[index - 1], run) {
            out.push(pvi);
        }
        for p in &run[1..run.len() - 1] {
            out.push(Pvi { point: *p, curve_length: 0.0 });
        }
    }
    if let Some(last) = runs.last() {
        out.push(Pvi { point: *last.last().unwrap(), curve_length: 0.0 });
    }
    out
}

/// Where the tangents ending `previous` and starting `next` meet.
fn curve_pvi(previous: &[Vector2], next: &[Vector2]) -> Option<Pvi> {
    let p0 = *previous.last()?;
    let p2 = *next.first()?;
    let before = previous[previous.len().checked_sub(2)?];
    let after = *next.get(1)?;
    let g0 = (p0.y - before.y) / (p0.x - before.x);
    let g1 = (after.y - p2.y) / (after.x - p2.x);
    if (g0 - g1).abs() < 1e-12 {
        return None;
    }
    let x = (p2.y - p0.y + g0 * p0.x - g1 * p2.x) / (g0 - g1);
    (x > p0.x && x < p2.x).then(|| Pvi {
        point: Vector2::new(x, p0.y + g0 * (x - p0.x)),
        curve_length: p2.x - p0.x,
    })
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

/// Bridge the gap between two tangent runs with the parabola tangent to both.
fn join_with_vertical_curves(runs: Vec<Vec<Vector2>>) -> Vec<Vec<Vector2>> {
    let mut out: Vec<Vec<Vector2>> = Vec::new();
    for run in runs {
        let pvi = out.last().and_then(|previous| curve_pvi(previous, &run));
        match (out.last_mut(), pvi) {
            (Some(previous), Some(pvi)) => {
                let p0 = *previous.last().unwrap();
                let p2 = run[0];
                for i in 1..32 {
                    let t = i as f64 / 32.0;
                    let u = 1.0 - t;
                    previous.push(p0 * (u * u) + pvi.point * (2.0 * u * t) + p2 * (t * t));
                }
                previous.extend(run);
            }
            _ => out.push(run),
        }
    }
    out
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
