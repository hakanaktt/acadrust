//! Solid and gradient hatch fills, including holes.
//!
//! `Normal` keeps odd-parity islands. `Outer` fills only the outermost band.
//! `Ignore` fills the outer ring and drops every island.

use crate::entities::{Hatch, HatchStyleType};
use crate::types::{Color, Vector2, Vector3};

use super::curves::{path_rings, to_world};

#[derive(Clone, Copy)]
struct V {
    x: f64,
    y: f64,
    z: f64,
}

impl V {
    fn from_point(point: Vector3) -> Self {
        Self {
            x: point.x,
            y: point.y,
            z: point.z,
        }
    }

    fn to_point(self) -> Vector3 {
        Vector3::new(self.x, self.y, self.z)
    }
}

/// Triangles in world coordinates, plus one RGB triple per vertex when the
/// hatch is a gradient. An empty color list means the entity style color.
pub(crate) fn solid_hatch(
    hatch: &Hatch,
    color_of: &dyn Fn(Color) -> [u8; 3],
) -> (Vec<Vector3>, Vec<[u8; 3]>) {
    let rings: Vec<Vec<V>> = path_rings(hatch)
        .into_iter()
        .map(|ring| ring.into_iter().map(V::from_point).collect())
        .collect();
    if rings.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let parents = parents_of(&rings);
    let mut triangles = Vec::new();
    for (index, _) in rings.iter().enumerate() {
        if parents[index].is_some() {
            continue;
        }
        match hatch.style {
            HatchStyleType::Ignore => {
                triangles.extend(triangulate(&rings[index], &[]));
            }
            HatchStyleType::Outer => {
                let holes = child_rings(&rings, &parents, index);
                triangles.extend(triangulate(&rings[index], &holes));
            }
            HatchStyleType::Normal => {
                append_parity(&rings, &parents, index, &mut triangles);
            }
        }
    }
    let points: Vec<Vector3> = triangles.iter().copied().map(V::to_point).collect();
    let colors = if hatch.gradient_color.is_enabled() {
        gradient_colors(hatch, &points, color_of)
    } else {
        Vec::new()
    };
    let points = points
        .into_iter()
        .map(|point| to_world(hatch, point))
        .collect();
    (points, colors)
}

fn append_parity(rings: &[Vec<V>], parents: &[Option<usize>], index: usize, out: &mut Vec<V>) {
    let holes = child_rings(rings, parents, index);
    out.extend(triangulate(&rings[index], &holes));
    for (child, parent) in parents.iter().enumerate() {
        if *parent == Some(index) {
            for (island, island_parent) in parents.iter().enumerate() {
                if *island_parent == Some(child) {
                    append_parity(rings, parents, island, out);
                }
            }
        }
    }
}

fn child_rings<'a>(rings: &'a [Vec<V>], parents: &[Option<usize>], index: usize) -> Vec<&'a [V]> {
    parents
        .iter()
        .enumerate()
        .filter(|(_, parent)| **parent == Some(index))
        .map(|(child, _)| rings[child].as_slice())
        .collect()
}

fn triangulate(outer: &[V], holes: &[&[V]]) -> Vec<V> {
    if outer.len() < 3 {
        return Vec::new();
    }
    let mut data =
        Vec::with_capacity((outer.len() + holes.iter().map(|hole| hole.len()).sum::<usize>()) * 2);
    let mut hole_indices = Vec::new();
    for point in outer {
        data.push(point.x);
        data.push(point.y);
    }
    for hole in holes {
        if hole.len() < 3 {
            continue;
        }
        hole_indices.push(data.len() / 2);
        for point in *hole {
            data.push(point.x);
            data.push(point.y);
        }
    }
    let indices = earcutr::earcut(&data, &hole_indices, 2).unwrap_or_default();
    let z = outer[0].z;
    indices
        .into_iter()
        .map(|index| {
            let offset = index * 2;
            V {
                x: data[offset],
                y: data[offset + 1],
                z,
            }
        })
        .collect()
}

fn parents_of(rings: &[Vec<V>]) -> Vec<Option<usize>> {
    let mut parents = vec![None; rings.len()];
    for (index, ring) in rings.iter().enumerate() {
        let sample = Vector2::new(ring[0].x, ring[0].y);
        let mut parent = None;
        let mut parent_area = f64::MAX;
        for (other, candidate) in rings.iter().enumerate() {
            if other == index {
                continue;
            }
            let area = signed_area(candidate).abs();
            if area < parent_area && contains(candidate, sample) {
                parent = Some(other);
                parent_area = area;
            }
        }
        parents[index] = parent;
    }
    parents
}

fn gradient_colors(
    hatch: &Hatch,
    points: &[Vector3],
    color_of: &dyn Fn(Color) -> [u8; 3],
) -> Vec<[u8; 3]> {
    let gradient = &hatch.gradient_color;
    let mut min = Vector2::new(f64::MAX, f64::MAX);
    let mut max = Vector2::new(f64::MIN, f64::MIN);
    for point in points {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    let span = Vector2::new((max.x - min.x).max(1e-9), (max.y - min.y).max(1e-9));
    let center = Vector2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);
    let angle = gradient.angle;
    let direction = Vector2::new(angle.cos(), angle.sin());
    let name = gradient.name.to_ascii_uppercase();
    let radius = (span.x * span.x + span.y * span.y).sqrt() * 0.5;
    points
        .iter()
        .map(|point| {
            let place = Vector2::new(point.x, point.y);
            let mut t = match name.as_str() {
                "CYLINDER" | "INVCYLINDER" => {
                    let along =
                        (place.x - center.x) * direction.x + (place.y - center.y) * direction.y;
                    let across_x = place.x - center.x - direction.x * along;
                    let across_y = place.y - center.y - direction.y * along;
                    (across_x * across_x + across_y * across_y).sqrt() / radius
                }
                "SPHERICAL" | "HEMISPHERICAL" | "CURVED" | "INVSPHERICAL" | "INVHEMISPHERICAL"
                | "INVCURVED" => {
                    let dx = place.x - center.x;
                    let dy = place.y - center.y;
                    (dx * dx + dy * dy).sqrt() / radius
                }
                _ => {
                    let projected = (place.x - min.x) * direction.x.abs()
                        + (place.y - min.y) * direction.y.abs();
                    let extent = span.x * direction.x.abs() + span.y * direction.y.abs();
                    projected / extent.max(1e-9)
                }
            };
            if name.starts_with("INV") {
                t = 1.0 - t;
            }
            t = (t + gradient.shift).clamp(0.0, 1.0);
            sample_gradient(gradient, t, color_of)
        })
        .collect()
}

fn sample_gradient(
    gradient: &crate::entities::HatchGradientPattern,
    t: f64,
    color_of: &dyn Fn(Color) -> [u8; 3],
) -> [u8; 3] {
    if gradient.colors.is_empty() {
        return [255, 255, 255];
    }
    if gradient.colors.len() == 1 || gradient.is_single_color {
        let base = color_of(gradient.colors[0].color);
        let tint = gradient.color_tint.clamp(0.0, 1.0);
        return [
            lerp_u8(base[0], 255, tint * (1.0 - t)),
            lerp_u8(base[1], 255, tint * (1.0 - t)),
            lerp_u8(base[2], 255, tint * (1.0 - t)),
        ];
    }
    let mut stops: Vec<(f64, [u8; 3])> = gradient
        .colors
        .iter()
        .map(|stop| (stop.value, color_of(stop.color)))
        .collect();
    stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for pair in stops.windows(2) {
        if t <= pair[1].0 {
            let span = (pair[1].0 - pair[0].0).max(1e-9);
            let local = ((t - pair[0].0) / span) as f32;
            return [
                lerp_u8(pair[0].1[0], pair[1].1[0], local as f64),
                lerp_u8(pair[0].1[1], pair[1].1[1], local as f64),
                lerp_u8(pair[0].1[2], pair[1].1[2], local as f64),
            ];
        }
    }
    stops.last().map(|stop| stop.1).unwrap_or([255, 255, 255])
}

fn lerp_u8(from: u8, to: u8, t: f64) -> u8 {
    let t = t.clamp(0.0, 1.0);
    (from as f64 + (to as f64 - from as f64) * t).round() as u8
}

fn signed_area(ring: &[V]) -> f64 {
    let mut area = 0.0;
    for (index, point) in ring.iter().enumerate() {
        let next = ring[(index + 1) % ring.len()];
        area += point.x * next.y - next.x * point.y;
    }
    area * 0.5
}

fn contains(ring: &[V], point: Vector2) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = ring.len() - 1;
    for current in 0..ring.len() {
        let a = ring[current];
        let b = ring[previous];
        if (a.y > point.y) != (b.y > point.y) {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}
