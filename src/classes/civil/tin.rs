//! TIN surface triangulation.
//!
//! The surface body stores its triangulation as one block:
//!
//! ```text
//! BL  vertex count
//! BL  x4  indices of the four bounding corner vertices
//! vertex count x { RL flags, RL one incident triangle, RD x, RD y, RD z }
//! BL  triangle count
//! triangle count x { RL v0, v1, v2, RL n0, n1, n2, RL flags, RL reserved }
//! ```
//!
//! Vertices run counter-clockwise; `n[i]` is the triangle across the edge
//! that starts at `v[i]`, or -1 on the hull. The four corner vertices (flag
//! 0x40) span the surface extents, and the triangles that use them have
//! flag bit 0 set and are not drawn. A second copy of the block follows the
//! first; the first is the built surface.

use super::bits::Bits;
use crate::types::Vector3;

const CORNER_FLAG: i32 = 0x40;
const HIDDEN_FLAG: i32 = 0x1;
const VERTEX_BITS: usize = 2 * 32 + 3 * 64;
const TRIANGLE_BITS: usize = 8 * 32;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinVertex {
    pub position: Vector3,
    /// 0x80 for a surface point, 0x40 for a bounding corner.
    pub flags: i32,
    /// One triangle that uses this vertex.
    pub triangle: i32,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinTriangle {
    /// Vertex indices, counter-clockwise.
    pub vertices: [u32; 3],
    /// Neighbor across the edge starting at the same-index vertex, -1 on the hull.
    pub neighbors: [i32; 3],
    /// Bit 0: hidden. Higher bits are kept as stored.
    pub flags: i32,
}

impl TinTriangle {
    pub fn is_visible(&self) -> bool {
        self.flags & HIDDEN_FLAG == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TinSurface {
    pub vertices: Vec<TinVertex>,
    pub triangles: Vec<TinTriangle>,
    /// Indices of the four bounding corner vertices.
    pub corners: [u32; 4],
}

impl TinSurface {
    /// Corner positions of every drawn triangle.
    pub fn visible_triangles(&self) -> impl Iterator<Item = [Vector3; 3]> + '_ {
        self.triangles
            .iter()
            .filter(|triangle| triangle.is_visible())
            .map(|triangle| triangle.vertices.map(|v| self.vertices[v as usize].position))
    }
}

/// First triangulation block in `data[start..end]` (bit positions).
// ponytail: probes every bit offset; a body without a triangulation costs a
// full scan. Parse the class header instead once its fields are known.
pub(super) fn find(data: &[u8], start: usize, end: usize) -> Option<TinSurface> {
    (start..end).find_map(|pos| read_at(data, pos, end))
}

fn read_at(data: &[u8], pos: usize, end: usize) -> Option<TinSurface> {
    let mut bits = Bits::new(data, pos, end);
    let count = usize::try_from(bits.bit_long()?).ok()?;
    if count < 7 || count.checked_mul(VERTEX_BITS)? > bits.remaining() {
        return None;
    }
    let mut corners = [0u32; 4];
    for corner in &mut corners {
        *corner = u32::try_from(bits.bit_long()?).ok()?;
        if *corner as usize >= count {
            return None;
        }
    }
    let first = bits.raw_long()?;
    if first & (CORNER_FLAG | 0x80) == 0 {
        return None;
    }
    bits.pos -= 32;
    let mut vertices = Vec::with_capacity(count);
    for _ in 0..count {
        let flags = bits.raw_long()?;
        let triangle = bits.raw_long()?;
        let position = Vector3::new(bits.raw_double()?, bits.raw_double()?, bits.raw_double()?);
        if !(position.x.is_finite() && position.y.is_finite() && position.z.is_finite()) {
            return None;
        }
        vertices.push(TinVertex {
            position,
            flags,
            triangle,
        });
    }
    if corners
        .iter()
        .any(|&c| vertices[c as usize].flags & CORNER_FLAG == 0)
    {
        return None;
    }
    let triangle_count = usize::try_from(bits.bit_long()?).ok()?;
    if triangle_count == 0 || triangle_count.checked_mul(TRIANGLE_BITS)? > bits.remaining() {
        return None;
    }
    let mut triangles = Vec::with_capacity(triangle_count);
    for _ in 0..triangle_count {
        let mut vertex = [0u32; 3];
        for v in &mut vertex {
            *v = u32::try_from(bits.raw_long()?).ok()?;
            if *v as usize >= count {
                return None;
            }
        }
        let mut neighbors = [0i32; 3];
        for n in &mut neighbors {
            *n = bits.raw_long()?;
            if *n < -1 || *n as i64 >= triangle_count as i64 {
                return None;
            }
        }
        let flags = bits.raw_long()?;
        bits.raw_long()?;
        triangles.push(TinTriangle {
            vertices: vertex,
            neighbors,
            flags,
        });
    }
    Some(TinSurface {
        vertices,
        triangles,
        corners,
    })
}

/// Join `(a, b)` segments sharing end keys into polylines of keys.
fn chain<K: Copy + Eq + std::hash::Hash>(segments: &[(K, K)]) -> Vec<Vec<K>> {
    use std::collections::HashMap;
    let mut at: HashMap<K, Vec<usize>> = HashMap::new();
    for (i, (a, b)) in segments.iter().enumerate() {
        at.entry(*a).or_default().push(i);
        at.entry(*b).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    // Open chains start at a key one segment touches; the rest are loops.
    let mut starts: Vec<usize> = (0..segments.len())
        .filter(|&i| at[&segments[i].0].len() == 1 || at[&segments[i].1].len() == 1)
        .collect();
    starts.extend(0..segments.len());
    for first in starts {
        if used[first] {
            continue;
        }
        used[first] = true;
        let (a, b) = segments[first];
        let mut keys = if at[&a].len() == 1 { vec![a, b] } else { vec![b, a] };
        loop {
            let tail = *keys.last().unwrap();
            let Some(&next) = at[&tail].iter().find(|&&i| !used[i]) else {
                break;
            };
            used[next] = true;
            let (p, q) = segments[next];
            keys.push(if p == tail { q } else { p });
        }
        out.push(keys);
    }
    out
}

impl TinSurface {
    fn drawn(&self, triangle: i32) -> bool {
        triangle >= 0
            && self
                .triangles
                .get(triangle as usize)
                .is_some_and(TinTriangle::is_visible)
    }

    /// Outline of the drawn triangles: every edge without a drawn neighbor,
    /// joined into loops.
    pub fn boundary(&self) -> Vec<Vec<Vector3>> {
        let mut edges = Vec::new();
        for triangle in self.triangles.iter().filter(|t| t.is_visible()) {
            for edge in 0..3 {
                if !self.drawn(triangle.neighbors[edge]) {
                    edges.push((triangle.vertices[edge], triangle.vertices[(edge + 1) % 3]));
                }
            }
        }
        chain(&edges)
            .into_iter()
            .map(|keys| keys.iter().map(|&v| self.vertices[v as usize].position).collect())
            .collect()
    }

    /// Contour polylines at every multiple of `interval`, with their
    /// elevation. A vertex exactly at a contour elevation counts as above it.
    pub fn contours(&self, interval: f64) -> Vec<(f64, Vec<Vector3>)> {
        use std::collections::HashMap;
        if !(interval > 0.0) {
            return Vec::new();
        }
        type Edge = (u32, u32);
        let key = |a: u32, b: u32| (a.min(b), a.max(b));
        let mut levels: HashMap<i64, Vec<(Edge, Edge)>> = HashMap::new();
        for triangle in self.triangles.iter().filter(|t| t.is_visible()) {
            let v = triangle.vertices;
            let z = v.map(|i| self.vertices[i as usize].position.z);
            let low = (z.iter().copied().fold(f64::INFINITY, f64::min) / interval).ceil() as i64;
            let high = (z.iter().copied().fold(f64::NEG_INFINITY, f64::max) / interval).floor() as i64;
            for level in low..=high {
                let h = level as f64 * interval;
                let crossed: Vec<Edge> = (0..3)
                    .filter(|&e| (z[e] >= h) != (z[(e + 1) % 3] >= h))
                    .map(|e| key(v[e], v[(e + 1) % 3]))
                    .collect();
                if let [a, b] = crossed[..] {
                    levels.entry(level).or_default().push((a, b));
                }
            }
        }
        let mut out = Vec::new();
        let mut ordered: Vec<_> = levels.into_iter().collect();
        ordered.sort_by_key(|(level, _)| *level);
        for (level, segments) in ordered {
            let h = level as f64 * interval;
            let at = |(a, b): Edge| {
                let (p, q) = (self.vertices[a as usize].position, self.vertices[b as usize].position);
                let t = (h - p.z) / (q.z - p.z);
                Vector3::new(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t, h)
            };
            for keys in chain(&segments) {
                out.push((h, keys.into_iter().map(at).collect()));
            }
        }
        out
    }
}
