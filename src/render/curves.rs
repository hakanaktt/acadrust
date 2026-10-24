//! Curve and marker sampling for the scene compiler.
//!
//! Chord spacing is about ten degrees, the same step used for bulged polylines.

use std::f64::consts::TAU;

use crate::entities::{Arc, Circle, Ellipse, Hatch, Point, Spline};
use crate::types::{Matrix3, Vector2, Vector3};

pub(crate) fn circle_points(circle: &Circle) -> Vec<Vector3> {
    arc_points_wcs(
        circle.center_wcs(),
        circle.axes_wcs(),
        circle.radius,
        circle.radius,
        0.0,
        TAU,
        true,
    )
}

pub(crate) fn arc_points(arc: &Arc) -> Vec<Vector3> {
    let basis = Matrix3::arbitrary_axis(arc.normal);
    let center = basis * arc.center;
    let axes = (basis * Vector3::UNIT_X, basis * Vector3::UNIT_Y);
    let mut sweep = arc.end_angle - arc.start_angle;
    if sweep <= 1e-12 {
        sweep += TAU;
    }
    arc_points_wcs(
        center,
        axes,
        arc.radius,
        arc.radius,
        arc.start_angle,
        arc.start_angle + sweep,
        false,
    )
}

pub(crate) fn ellipse_points(ellipse: &Ellipse) -> Vec<Vector3> {
    let major = ellipse.major_axis;
    let length = major.length();
    if length < 1e-12 {
        return Vec::new();
    }
    let major_dir = major.normalize();
    let minor_dir = ellipse.normal.cross(&major_dir).normalize();
    let mut sweep = ellipse.end_parameter - ellipse.start_parameter;
    let full = ellipse.is_full() || sweep.abs() >= TAU - 1e-8;
    if sweep <= 1e-12 {
        sweep += TAU;
    }
    if sweep < 0.0 && !full {
        sweep += TAU;
    }
    arc_points_wcs(
        ellipse.center,
        (major_dir, minor_dir),
        length,
        length * ellipse.minor_axis_ratio.abs(),
        ellipse.start_parameter,
        ellipse.start_parameter + if full { TAU } else { sweep },
        full,
    )
}

pub(crate) fn spline_points(spline: &Spline) -> Vec<Vector3> {
    let lines = crate::entities::EntityType::Spline(spline.clone()).flatten();
    let mut points = Vec::new();
    for entity in lines {
        if let crate::entities::EntityType::Line(line) = entity {
            if points
                .last()
                .is_none_or(|last: &Vector3| (*last - line.start).length_squared() > 1e-18)
            {
                points.push(line.start);
            }
            points.push(line.end);
        }
    }
    if points.len() < 2 && spline.fit_points.len() >= 2 {
        return spline.fit_points.clone();
    }
    points
}

/// PDMODE glyph in world space. `size` is the symbol size in drawing units.
pub(crate) fn point_chains(point: &Point, mode: i16, size: f64) -> Vec<Vec<Vector3>> {
    let shape = mode & 7;
    let circle = mode & 32 != 0;
    let square = mode & 64 != 0;
    if shape == 1 && !circle && !square {
        return Vec::new();
    }
    let basis = Matrix3::arbitrary_axis(point.normal);
    let rotation = Matrix3::rotation_z(point.x_axis_angle);
    let half = size.abs().max(1e-6) * 0.5;
    let map = |x: f64, y: f64| point.location + basis * (rotation * Vector3::new(x, y, 0.0));
    let mut chains = Vec::new();
    match shape {
        0 => chains.push(vec![point.location, point.location]),
        2 => {
            chains.push(vec![map(-half, 0.0), map(half, 0.0)]);
            chains.push(vec![map(0.0, -half), map(0.0, half)]);
        }
        3 => {
            chains.push(vec![map(-half, -half), map(half, half)]);
            chains.push(vec![map(-half, half), map(half, -half)]);
        }
        4 => chains.push(vec![map(0.0, -half), map(0.0, half)]),
        _ => {}
    }
    if circle {
        let samples = arc_points_wcs(
            point.location,
            (basis * Vector3::UNIT_X, basis * Vector3::UNIT_Y),
            half,
            half,
            0.0,
            TAU,
            true,
        );
        chains.push(samples);
    }
    if square {
        chains.push(vec![
            map(-half, -half),
            map(half, -half),
            map(half, half),
            map(-half, half),
            map(-half, -half),
        ]);
    }
    chains
}

pub(crate) fn path_rings(hatch: &Hatch) -> Vec<Vec<Vector3>> {
    hatch
        .paths
        .iter()
        .map(|path| path_ring(hatch, path))
        .filter(|ring| ring.len() >= 3)
        .collect()
}

pub(crate) fn path_ring(hatch: &Hatch, path: &crate::entities::BoundaryPath) -> Vec<Vector3> {
    let mut raw = Vec::new();
    for edge in &path.edges {
        match edge {
            crate::entities::BoundaryEdge::Line(line) => {
                push_unique(
                    &mut raw,
                    Vector3::new(line.start.x, line.start.y, hatch.elevation),
                );
                push_unique(
                    &mut raw,
                    Vector3::new(line.end.x, line.end.y, hatch.elevation),
                );
            }
            crate::entities::BoundaryEdge::CircularArc(arc) => {
                let sweep = signed_sweep(arc.start_angle, arc.end_angle, arc.counter_clockwise);
                let samples = arc_points_xy(
                    arc.center,
                    arc.radius,
                    arc.radius,
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                    arc.start_angle,
                    arc.start_angle + sweep,
                    false,
                );
                for sample in samples {
                    push_unique(&mut raw, Vector3::new(sample.x, sample.y, hatch.elevation));
                }
            }
            crate::entities::BoundaryEdge::Polyline(poly) => {
                let count = if poly.is_closed {
                    poly.vertices.len()
                } else {
                    poly.vertices.len().saturating_sub(1)
                };
                for index in 0..count {
                    let from = poly.vertices[index];
                    let to = poly.vertices[(index + 1) % poly.vertices.len()];
                    let start = Vector3::new(from.x, from.y, hatch.elevation);
                    let end = Vector3::new(to.x, to.y, hatch.elevation);
                    for sample in super::compile::tessellate_bulge(start, end, from.z) {
                        push_unique(&mut raw, sample);
                    }
                }
            }
            crate::entities::BoundaryEdge::EllipticArc(arc) => {
                let major = arc.major_axis_endpoint;
                let length = major.length();
                if length < 1e-12 {
                    continue;
                }
                let major_dir = Vector2::new(major.x / length, major.y / length);
                let minor_dir = Vector2::new(-major_dir.y, major_dir.x);
                let sweep = signed_sweep(arc.start_angle, arc.end_angle, arc.counter_clockwise);
                for sample in arc_points_xy(
                    arc.center,
                    length,
                    length * arc.minor_axis_ratio.abs(),
                    major_dir,
                    minor_dir,
                    arc.start_angle,
                    arc.start_angle + sweep,
                    false,
                ) {
                    push_unique(&mut raw, Vector3::new(sample.x, sample.y, hatch.elevation));
                }
            }
            crate::entities::BoundaryEdge::Spline(spline) => {
                let source: Vec<Vector3> = if spline.fit_points.len() >= 2 {
                    spline
                        .fit_points
                        .iter()
                        .map(|point| Vector3::new(point.x, point.y, hatch.elevation))
                        .collect()
                } else {
                    spline
                        .control_points
                        .iter()
                        .map(|point| Vector3::new(point.x, point.y, hatch.elevation))
                        .collect()
                };
                for point in source {
                    push_unique(&mut raw, point);
                }
            }
        }
    }
    if raw.len() >= 2 && (raw[0] - *raw.last().unwrap()).length_squared() < 1e-16 {
        raw.pop();
    }
    raw
}

pub(crate) fn to_world(hatch: &Hatch, point: Vector3) -> Vector3 {
    Matrix3::arbitrary_axis(hatch.normal) * point
}

pub(crate) fn fan_triangles(ring: &[Vector3]) -> Vec<Vector3> {
    if ring.len() < 3 {
        return Vec::new();
    }
    let mut triangles = Vec::with_capacity((ring.len() - 2) * 3);
    for index in 1..ring.len() - 1 {
        triangles.push(ring[0]);
        triangles.push(ring[index]);
        triangles.push(ring[index + 1]);
    }
    triangles
}

fn arc_points_wcs(
    center: Vector3,
    axes: (Vector3, Vector3),
    radius_x: f64,
    radius_y: f64,
    start: f64,
    end: f64,
    closed: bool,
) -> Vec<Vector3> {
    let sweep = end - start;
    let steps = ((sweep.abs() / 10.0_f64.to_radians()).ceil() as usize).clamp(8, 128);
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let angle = start + sweep * step as f64 / steps as f64;
        points.push(center + axes.0 * (radius_x * angle.cos()) + axes.1 * (radius_y * angle.sin()));
    }
    if closed {
        if let Some(first) = points.first().copied() {
            if let Some(last) = points.last_mut() {
                *last = first;
            }
        }
    }
    points
}

fn arc_points_xy(
    center: Vector2,
    radius_x: f64,
    radius_y: f64,
    axis_x: Vector2,
    axis_y: Vector2,
    start: f64,
    end: f64,
    closed: bool,
) -> Vec<Vector2> {
    let samples = arc_points_wcs(
        Vector3::new(center.x, center.y, 0.0),
        (
            Vector3::new(axis_x.x, axis_x.y, 0.0),
            Vector3::new(axis_y.x, axis_y.y, 0.0),
        ),
        radius_x,
        radius_y,
        start,
        end,
        closed,
    );
    samples
        .into_iter()
        .map(|point| Vector2::new(point.x, point.y))
        .collect()
}

fn signed_sweep(start: f64, end: f64, counter_clockwise: bool) -> f64 {
    let mut sweep = end - start;
    if counter_clockwise {
        if sweep <= 1e-12 {
            sweep += TAU;
        }
    } else if sweep >= -1e-12 {
        sweep -= TAU;
    }
    sweep
}

fn push_unique(points: &mut Vec<Vector3>, point: Vector3) {
    if points
        .last()
        .is_none_or(|last| (*last - point).length_squared() > 1e-16)
    {
        points.push(point);
    }
}
