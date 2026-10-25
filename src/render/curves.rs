//! Curve and marker sampling for the scene compiler.
//!
//! Circles, arcs, ellipses, bulges, and splines are stored as analytic
//! [`Curve`](super::scene::Curve) values. The chord samples remain for
//! extents and for callers that still read a point list.

use std::f64::consts::TAU;

use crate::entities::{Arc, Circle, Ellipse, Hatch, Point, Spline};
use crate::types::{Matrix3, Transform, Vector2, Vector3};

use super::scene::Curve;

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

pub(crate) fn boundary_curves(hatch: &Hatch, path: &crate::entities::BoundaryPath) -> Vec<Curve> {
    let mut curves = Vec::new();
    for edge in &path.edges {
        match edge {
            crate::entities::BoundaryEdge::Line(line) => {
                curves.push(Curve::Line {
                    start: Vector3::new(line.start.x, line.start.y, hatch.elevation),
                    end: Vector3::new(line.end.x, line.end.y, hatch.elevation),
                });
            }
            crate::entities::BoundaryEdge::CircularArc(arc) => {
                let sweep = signed_sweep(arc.start_angle, arc.end_angle, arc.counter_clockwise);
                curves.push(Curve::Elliptic {
                    center: Vector3::new(arc.center.x, arc.center.y, hatch.elevation),
                    u: Vector3::new(arc.radius, 0.0, 0.0),
                    v: Vector3::new(0.0, arc.radius, 0.0),
                    start: arc.start_angle,
                    sweep,
                });
            }
            crate::entities::BoundaryEdge::EllipticArc(arc) => {
                let major = arc.major_axis_endpoint;
                let length = major.length();
                if length < 1e-12 {
                    continue;
                }
                let sweep = signed_sweep(arc.start_angle, arc.end_angle, arc.counter_clockwise);
                let major_dir = Vector3::new(major.x / length, major.y / length, 0.0);
                let minor_dir = Vector3::new(-major_dir.y, major_dir.x, 0.0);
                let minor = length * arc.minor_axis_ratio.abs();
                curves.push(Curve::Elliptic {
                    center: Vector3::new(arc.center.x, arc.center.y, hatch.elevation),
                    u: major_dir * length,
                    v: minor_dir * minor,
                    start: arc.start_angle,
                    sweep,
                });
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
                    curves.push(bulge_curve(
                        Vector3::new(from.x, from.y, hatch.elevation),
                        Vector3::new(to.x, to.y, hatch.elevation),
                        from.z,
                    ));
                }
            }
            crate::entities::BoundaryEdge::Spline(spline) => {
                let control: Vec<Vector3> = spline
                    .control_points
                    .iter()
                    .map(|point| Vector3::new(point.x, point.y, hatch.elevation))
                    .collect();
                let weights: Vec<f64> = if spline.rational {
                    spline.control_points.iter().map(|point| point.z).collect()
                } else {
                    Vec::new()
                };
                if control.len() >= 2 {
                    let degree = usize::try_from(spline.degree).unwrap_or(3).max(1);
                    curves.extend(nurbs_curves(degree, &control, &weights, &spline.knots));
                } else if spline.fit_points.len() >= 2 {
                    let points: Vec<Vector3> = spline
                        .fit_points
                        .iter()
                        .map(|point| Vector3::new(point.x, point.y, hatch.elevation))
                        .collect();
                    curves.extend(fit_cubics(&points, spline.periodic));
                }
            }
        }
    }
    curves
        .into_iter()
        .map(|curve| map_curve(curve, &|point| to_world(hatch, point)))
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

pub(crate) fn circle_curve(circle: &Circle) -> Curve {
    let (axis_x, axis_y) = circle.axes_wcs();
    Curve::Elliptic {
        center: circle.center_wcs(),
        u: axis_x * circle.radius,
        v: axis_y * circle.radius,
        start: 0.0,
        sweep: TAU,
    }
}

pub(crate) fn arc_curve(arc: &Arc) -> Curve {
    let basis = Matrix3::arbitrary_axis(arc.normal);
    let center = basis * arc.center;
    let mut sweep = arc.end_angle - arc.start_angle;
    if sweep <= 1e-12 {
        sweep += TAU;
    }
    Curve::Elliptic {
        center,
        u: basis * Vector3::UNIT_X * arc.radius,
        v: basis * Vector3::UNIT_Y * arc.radius,
        start: arc.start_angle,
        sweep,
    }
}

pub(crate) fn ellipse_curve(ellipse: &Ellipse) -> Option<Curve> {
    let major = ellipse.major_axis;
    let length = major.length();
    if length < 1e-12 {
        return None;
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
    Some(Curve::Elliptic {
        center: ellipse.center,
        u: major_dir * length,
        v: minor_dir * (length * ellipse.minor_axis_ratio.abs()),
        start: ellipse.start_parameter,
        sweep: if full { TAU } else { sweep },
    })
}

/// Circular arc for a polyline bulge, or a straight segment when the bulge is flat.
/// The arc lies in a plane of constant `z` through `start` and `end`.
pub(crate) fn bulge_curve(start: Vector3, end: Vector3, bulge: f64) -> Curve {
    if bulge.abs() < 1e-10 {
        return Curve::Line { start, end };
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-10 {
        return Curve::Line { start, end };
    }
    let bulge_sq = bulge * bulge;
    let radius = chord * (1.0 + bulge_sq) / (4.0 * bulge.abs());
    let offset = chord * (1.0 - bulge_sq) / (4.0 * bulge);
    let center = Vector3::new(
        (start.x + end.x) / 2.0 + (-dy / chord) * offset,
        (start.y + end.y) / 2.0 + (dx / chord) * offset,
        start.z,
    );
    let start_angle = (start.y - center.y).atan2(start.x - center.x);
    let end_angle = (end.y - center.y).atan2(end.x - center.x);
    let mut sweep = end_angle - start_angle;
    if bulge > 0.0 && sweep < 0.0 {
        sweep += TAU;
    } else if bulge < 0.0 && sweep > 0.0 {
        sweep -= TAU;
    }
    let (sin, cos) = start_angle.sin_cos();
    Curve::Elliptic {
        center,
        u: Vector3::new(radius * cos, radius * sin, 0.0),
        v: Vector3::new(-radius * sin, radius * cos, 0.0),
        start: 0.0,
        sweep,
    }
}

pub(crate) fn spline_curves(spline: &Spline) -> Vec<Curve> {
    let mut curves = if spline.control_points.len() >= 2 {
        let degree = usize::try_from(spline.degree).unwrap_or(3).max(1);
        nurbs_curves(
            degree,
            &spline.control_points,
            &spline.weights,
            &spline.knots,
        )
    } else {
        fit_cubics(&spline.fit_points, spline.flags.closed)
    };
    if spline.flags.closed {
        close_gap(&mut curves);
    }
    curves
}

pub(crate) fn transform_curve(transform: &Transform, curve: Curve) -> Curve {
    map_curve(curve, &|point| transform.apply(point))
}

pub(crate) fn map_curve(curve: Curve, map: &impl Fn(Vector3) -> Vector3) -> Curve {
    match curve {
        Curve::Line { start, end } => Curve::Line {
            start: map(start),
            end: map(end),
        },
        Curve::Elliptic {
            center,
            u,
            v,
            start,
            sweep,
        } => {
            let origin = map(center);
            Curve::Elliptic {
                center: origin,
                u: map(center + u) - origin,
                v: map(center + v) - origin,
                start,
                sweep,
            }
        }
        Curve::Cubic {
            p0,
            p1,
            p2,
            p3,
            weights,
        } => Curve::Cubic {
            p0: map(p0),
            p1: map(p1),
            p2: map(p2),
            p3: map(p3),
            weights,
        },
    }
}

pub(crate) fn curve_length(curve: &Curve) -> f64 {
    match curve {
        Curve::Line { start, end } => (*end - *start).length(),
        Curve::Elliptic { u, v, sweep, .. } => elliptic_length(*u, *v, sweep),
        Curve::Cubic {
            p0,
            p1,
            p2,
            p3,
            weights,
        } => cubic_length(&[*p0, *p1, *p2, *p3], weights, 0.0, 1.0),
    }
}

pub(crate) fn path_length(curves: &[Curve]) -> f64 {
    curves.iter().map(curve_length).sum()
}

/// Trim each curve to the arc-length window `[from, to]` along `curves`.
pub(crate) fn extract_range(curves: &[Curve], from: f64, to: f64) -> Vec<Curve> {
    let mut out = Vec::new();
    let mut origin = 0.0;
    for curve in curves {
        let length = curve_length(curve);
        let seg_end = origin + length;
        let start = from.max(origin);
        let end = to.min(seg_end);
        if end - start > 1e-9 && length > 1e-12 {
            out.push(trim_curve(
                curve,
                (start - origin) / length,
                (end - origin) / length,
            ));
        }
        origin = seg_end;
    }
    out
}

pub(crate) fn point_at_distance(curves: &[Curve], distance: f64) -> Option<Vector3> {
    let mut origin = 0.0;
    for curve in curves {
        let length = curve_length(curve);
        if distance <= origin + length + 1e-9 {
            let fraction = if length <= 1e-12 {
                0.0
            } else {
                ((distance - origin) / length).clamp(0.0, 1.0)
            };
            return Some(point_at_fraction(curve, fraction));
        }
        origin += length;
    }
    curves.last().map(|curve| point_at_fraction(curve, 1.0))
}

pub(crate) fn sample_curve(curve: &Curve, steps: usize) -> Vec<Vector3> {
    let steps = steps.max(1);
    match curve {
        Curve::Line { start, end } => vec![*start, *end],
        Curve::Elliptic {
            center,
            u,
            v,
            start,
            sweep,
        } => (0..=steps)
            .map(|step| {
                let t = start + sweep * step as f64 / steps as f64;
                elliptic_point(*center, *u, *v, t)
            })
            .collect(),
        Curve::Cubic { .. } => (0..=steps)
            .map(|step| point_at_fraction(curve, step as f64 / steps as f64))
            .collect(),
    }
}

pub(crate) fn distance_xy(curve: &Curve, point: Vector2) -> f64 {
    let target = Vector3::new(point.x, point.y, 0.0);
    match curve {
        Curve::Line { start, end } => segment_xy(*start, *end, point),
        Curve::Elliptic {
            center,
            u,
            v,
            start,
            sweep,
        } => {
            let mut best = f64::MAX;
            let steps = 24;
            for step in 0..=steps {
                let t = start + sweep * step as f64 / steps as f64;
                best = best.min(xy_delta(elliptic_point(*center, *u, *v, t), target));
            }
            let seed = *start + sweep * 0.5;
            let refined = refine_elliptic(*center, *u, *v, *start, *sweep, target, seed);
            best.min(xy_delta(refined, target))
        }
        Curve::Cubic {
            p0,
            p1,
            p2,
            p3,
            weights,
        } => {
            let mut best_t = 0.0;
            let mut best = f64::MAX;
            for step in 0..=16 {
                let t = step as f64 / 16.0;
                let distance = xy_delta(cubic_point(&[*p0, *p1, *p2, *p3], weights, t), target);
                if distance < best {
                    best = distance;
                    best_t = t;
                }
            }
            let t = refine_cubic(&[*p0, *p1, *p2, *p3], weights, best_t, target);
            best.min(xy_delta(
                cubic_point(&[*p0, *p1, *p2, *p3], weights, t),
                target,
            ))
        }
    }
}

/// Keep the portions of `curves` that lie inside a convex polygon.
pub(crate) fn clip_curves(curves: &[Curve], polygon: &[Vector2]) -> Vec<Curve> {
    if polygon.len() < 3 {
        return curves.to_vec();
    }
    let mut out = Vec::new();
    for curve in curves {
        match curve {
            Curve::Line { start, end } => {
                if let Some((start, end)) = super::clip::clip_segment(*start, *end, polygon) {
                    out.push(Curve::Line { start, end });
                }
            }
            Curve::Elliptic {
                center,
                u,
                v,
                start,
                sweep,
            } => out.extend(clip_elliptic(*center, *u, *v, *start, *sweep, polygon)),
            Curve::Cubic {
                p0,
                p1,
                p2,
                p3,
                weights,
            } => {
                let hom = cubic_hom([*p0, *p1, *p2, *p3], weights);
                out.extend(clip_cubic(&hom, polygon, 0));
            }
        }
    }
    out
}

fn elliptic_point(center: Vector3, u: Vector3, v: Vector3, t: f64) -> Vector3 {
    let (sin, cos) = t.sin_cos();
    center + u * cos + v * sin
}

fn elliptic_speed(u: Vector3, v: Vector3, t: f64) -> f64 {
    let (sin, cos) = t.sin_cos();
    (u * -sin + v * cos).length()
}

fn elliptic_length(u: Vector3, v: Vector3, sweep: &f64) -> f64 {
    let start_speed = elliptic_speed(u, v, 0.0);
    let mid_speed = elliptic_speed(u, v, sweep * 0.5);
    if (start_speed - mid_speed).abs() <= 1e-6 * start_speed.max(1.0) {
        return start_speed * sweep.abs();
    }
    let steps = 16;
    let mut acc = 0.0;
    let mut previous = start_speed;
    for step in 1..=steps {
        let speed = elliptic_speed(u, v, sweep * step as f64 / steps as f64);
        acc += (previous + speed) * 0.5;
        previous = speed;
    }
    acc * sweep.abs() / steps as f64
}

fn elliptic_parameter_at(u: Vector3, v: Vector3, start: f64, sweep: f64, fraction: f64) -> f64 {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.0 {
        return start;
    }
    if fraction >= 1.0 {
        return start + sweep;
    }
    let target = elliptic_length(u, v, &sweep) * fraction;
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..24 {
        let mid = (low + high) * 0.5;
        let length = elliptic_length(u, v, &(sweep * mid));
        if length < target {
            low = mid;
        } else {
            high = mid;
        }
    }
    start + sweep * (low + high) * 0.5
}

fn trim_curve(curve: &Curve, start_fraction: f64, end_fraction: f64) -> Curve {
    let start_fraction = start_fraction.clamp(0.0, 1.0);
    let end_fraction = end_fraction.clamp(start_fraction, 1.0);
    match *curve {
        Curve::Line { start, end } => {
            let delta = end - start;
            Curve::Line {
                start: start + delta * start_fraction,
                end: start + delta * end_fraction,
            }
        }
        Curve::Elliptic {
            center,
            u,
            v,
            start,
            sweep,
        } => {
            let from = elliptic_parameter_at(u, v, start, sweep, start_fraction);
            let to = elliptic_parameter_at(u, v, start, sweep, end_fraction);
            Curve::Elliptic {
                center,
                u,
                v,
                start: from,
                sweep: to - from,
            }
        }
        Curve::Cubic {
            p0,
            p1,
            p2,
            p3,
            weights,
        } => {
            let points = [p0, p1, p2, p3];
            let t0 = cubic_parameter_at(&points, &weights, start_fraction);
            let t1 = cubic_parameter_at(&points, &weights, end_fraction);
            let hom = cubic_hom(points, &weights);
            cubic_from_hom(&split_range(&hom, t0, t1))
        }
    }
}

fn point_at_fraction(curve: &Curve, fraction: f64) -> Vector3 {
    let fraction = fraction.clamp(0.0, 1.0);
    match *curve {
        Curve::Line { start, end } => start + (end - start) * fraction,
        Curve::Elliptic {
            center,
            u,
            v,
            start,
            sweep,
        } => elliptic_point(
            center,
            u,
            v,
            elliptic_parameter_at(u, v, start, sweep, fraction),
        ),
        Curve::Cubic {
            p0,
            p1,
            p2,
            p3,
            weights,
        } => {
            let points = [p0, p1, p2, p3];
            cubic_point(
                &points,
                &weights,
                cubic_parameter_at(&points, &weights, fraction),
            )
        }
    }
}

fn cubic_point(points: &[Vector3; 4], weights: &[f64; 4], t: f64) -> Vector3 {
    let basis = bernstein(t);
    let mut acc = [0.0; 4];
    for index in 0..4 {
        let weight = basis[index] * weights[index].max(1e-12);
        acc[0] += points[index].x * weight;
        acc[1] += points[index].y * weight;
        acc[2] += points[index].z * weight;
        acc[3] += weight;
    }
    cartesian(acc)
}

fn bernstein(t: f64) -> [f64; 4] {
    let m = 1.0 - t;
    [m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t]
}

fn cubic_length(points: &[Vector3; 4], weights: &[f64; 4], t0: f64, t1: f64) -> f64 {
    let steps = 16;
    let mut acc = 0.0;
    let mut previous = cubic_point(points, weights, t0);
    for step in 1..=steps {
        let t = t0 + (t1 - t0) * step as f64 / steps as f64;
        let point = cubic_point(points, weights, t);
        acc += (point - previous).length();
        previous = point;
    }
    acc
}

fn cubic_parameter_at(points: &[Vector3; 4], weights: &[f64; 4], fraction: f64) -> f64 {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.0 || fraction >= 1.0 {
        return fraction;
    }
    let total = cubic_length(points, weights, 0.0, 1.0);
    let target = total * fraction;
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..24 {
        let mid = (low + high) * 0.5;
        if cubic_length(points, weights, 0.0, mid) < target {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) * 0.5
}

fn cubic_hom(points: [Vector3; 4], weights: &[f64; 4]) -> [[f64; 4]; 4] {
    std::array::from_fn(|index| {
        let weight = weights[index].max(1e-12);
        [
            points[index].x * weight,
            points[index].y * weight,
            points[index].z * weight,
            weight,
        ]
    })
}

fn cartesian(point: [f64; 4]) -> Vector3 {
    let weight = if point[3].abs() < 1e-15 {
        1.0
    } else {
        point[3]
    };
    Vector3::new(point[0] / weight, point[1] / weight, point[2] / weight)
}

fn weight_of(point: [f64; 4]) -> f64 {
    if point[3].abs() < 1e-15 {
        1.0
    } else {
        point[3]
    }
}

fn cubic_from_hom(points: &[[f64; 4]]) -> Curve {
    let at = |index: usize| points[index.min(points.len() - 1)];
    Curve::Cubic {
        p0: cartesian(at(0)),
        p1: cartesian(at(1)),
        p2: cartesian(at(2)),
        p3: cartesian(at(3)),
        weights: [
            weight_of(at(0)),
            weight_of(at(1)),
            weight_of(at(2)),
            weight_of(at(3)),
        ],
    }
}

fn split_hom(points: &[[f64; 4]], t: f64) -> (Vec<[f64; 4]>, Vec<[f64; 4]>) {
    let mut rows = vec![points.to_vec()];
    for row in 1..points.len() {
        let previous = &rows[row - 1];
        let mut next = Vec::with_capacity(previous.len() - 1);
        for pair in previous.windows(2) {
            next.push(std::array::from_fn(|channel| {
                (1.0 - t) * pair[0][channel] + t * pair[1][channel]
            }));
        }
        rows.push(next);
    }
    let left: Vec<[f64; 4]> = rows.iter().map(|row| row[0]).collect();
    let right: Vec<[f64; 4]> = rows.iter().rev().map(|row| *row.last().unwrap()).collect();
    (left, right)
}

fn split_range(points: &[[f64; 4]; 4], t0: f64, t1: f64) -> [[f64; 4]; 4] {
    if t1 - t0 <= 1e-12 {
        let point = points[0];
        return [point, point, point, point];
    }
    let (_, right) = split_hom(points, t0);
    let local = if t0 >= 1.0 - 1e-12 {
        0.0
    } else {
        ((t1 - t0) / (1.0 - t0)).clamp(0.0, 1.0)
    };
    let (left, _) = split_hom(&right, local);
    std::array::from_fn(|index| left[index])
}

fn nurbs_curves(degree: usize, control: &[Vector3], weights: &[f64], knots: &[f64]) -> Vec<Curve> {
    let count = control.len();
    if count < 2 {
        return Vec::new();
    }
    let degree = degree.clamp(1, count - 1);
    let knots =
        if knots.len() == count + degree + 1 && knots.windows(2).all(|pair| pair[1] >= pair[0]) {
            knots.to_vec()
        } else {
            clamped_knots(degree, count)
        };
    let rational =
        weights.len() == count && weights.iter().any(|weight| (*weight - 1.0).abs() > 1e-12);
    let mut hom: Vec<[f64; 4]> = control
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let weight = if rational && weights[index] > 0.0 {
                weights[index]
            } else {
                1.0
            };
            [point.x * weight, point.y * weight, point.z * weight, weight]
        })
        .collect();
    let mut knots = knots;
    let mut interior = Vec::new();
    let domain_start = knots[degree];
    let domain_end = knots[knots.len() - degree - 1];
    for knot in knots.clone() {
        if knot > domain_start + 1e-10
            && knot < domain_end - 1e-10
            && interior
                .last()
                .is_none_or(|prev: &f64| (knot - prev).abs() > 1e-10)
        {
            interior.push(knot);
        }
    }
    for knot in interior {
        let have = knots
            .iter()
            .filter(|value| (**value - knot).abs() <= 1e-10)
            .count()
            .min(degree);
        for _ in have..degree {
            insert_knot(degree, &mut knots, &mut hom, knot);
        }
    }
    let mut curves = Vec::new();
    let mut index = 0;
    while index + degree < hom.len() {
        let span = &hom[index..=index + degree];
        curves.extend(bezier_to_cubics(span, 0));
        index += degree;
    }
    curves
}

fn clamped_knots(degree: usize, count: usize) -> Vec<f64> {
    let inner = count - degree;
    (0..count + degree + 1)
        .map(|index| {
            if index <= degree {
                0.0
            } else if index >= count {
                1.0
            } else {
                (index - degree) as f64 / inner as f64
            }
        })
        .collect()
}

fn find_span(degree: usize, knots: &[f64], u: f64) -> usize {
    let n = knots.len() - degree - 2;
    if u >= knots[n + 1] - 1e-14 {
        return n;
    }
    let mut low = degree;
    let mut high = n + 1;
    let mut mid = (low + high) / 2;
    while u < knots[mid] || u >= knots[mid + 1] {
        if u < knots[mid] {
            high = mid;
        } else {
            low = mid;
        }
        mid = (low + high) / 2;
        if high <= low + 1 {
            return low;
        }
    }
    mid
}

fn insert_knot(degree: usize, knots: &mut Vec<f64>, control: &mut Vec<[f64; 4]>, u: f64) {
    let n = control.len() - 1;
    let span = find_span(degree, knots, u);
    let mut next = vec![[0.0; 4]; control.len() + 1];
    let head = span.saturating_sub(degree);
    for index in 0..=head {
        next[index] = control[index];
    }
    for index in (span - degree + 1)..=span {
        let denom = knots[index + degree] - knots[index];
        let alpha = if denom.abs() < 1e-14 {
            0.0
        } else {
            (u - knots[index]) / denom
        };
        next[index] = std::array::from_fn(|channel| {
            (1.0 - alpha) * control[index - 1][channel] + alpha * control[index][channel]
        });
    }
    for index in (span + 1)..=(n + 1) {
        next[index] = control[index - 1];
    }
    knots.insert(span + 1, u);
    *control = next;
}

fn bezier_to_cubics(points: &[[f64; 4]], depth: usize) -> Vec<Curve> {
    match points.len() {
        0 | 1 => Vec::new(),
        2 => vec![Curve::Line {
            start: cartesian(points[0]),
            end: cartesian(points[1]),
        }],
        3 => bezier_to_cubics(&elevate_quadratic(points), depth),
        4 => vec![cubic_from_hom(points)],
        _ => {
            if depth >= 12 || cubic_fit_error(points) < 1e-5 {
                return vec![fitted_cubic(points)];
            }
            let (left, right) = split_hom(points, 0.5);
            let mut curves = bezier_to_cubics(&left, depth + 1);
            curves.extend(bezier_to_cubics(&right, depth + 1));
            curves
        }
    }
}

fn elevate_quadratic(points: &[[f64; 4]]) -> Vec<[f64; 4]> {
    let mut cubic = vec![[0.0; 4]; 4];
    cubic[0] = points[0];
    cubic[3] = points[2];
    for channel in 0..4 {
        cubic[1][channel] = (points[0][channel] + 2.0 * points[1][channel]) / 3.0;
        cubic[2][channel] = (2.0 * points[1][channel] + points[2][channel]) / 3.0;
    }
    cubic
}

fn fitted_cubic(points: &[[f64; 4]]) -> Curve {
    cubic_from_hom(&fitted_cubic_hom(points))
}

fn fitted_cubic_hom(points: &[[f64; 4]]) -> [[f64; 4]; 4] {
    let degree = (points.len() - 1) as f64;
    let scale = degree / 3.0;
    let mut cubic = [[0.0; 4]; 4];
    cubic[0] = points[0];
    cubic[3] = points[points.len() - 1];
    for channel in 0..4 {
        cubic[1][channel] = points[0][channel] + (points[1][channel] - points[0][channel]) * scale;
        cubic[2][channel] = points[points.len() - 1][channel]
            + (points[points.len() - 2][channel] - points[points.len() - 1][channel]) * scale;
    }
    cubic
}

fn cubic_fit_error(points: &[[f64; 4]]) -> f64 {
    let fitted = fitted_cubic_hom(points);
    let actual = de_casteljau(points, 0.5);
    let approx = de_casteljau(&fitted, 0.5);
    (cartesian(actual) - cartesian(approx)).length()
}

fn de_casteljau(points: &[[f64; 4]], t: f64) -> [f64; 4] {
    let mut row = points.to_vec();
    while row.len() > 1 {
        let mut next = Vec::with_capacity(row.len() - 1);
        for pair in row.windows(2) {
            next.push(std::array::from_fn(|channel| {
                (1.0 - t) * pair[0][channel] + t * pair[1][channel]
            }));
        }
        row = next;
    }
    row[0]
}

fn fit_cubics(points: &[Vector3], closed: bool) -> Vec<Curve> {
    if points.len() < 2 {
        return Vec::new();
    }
    if points.len() == 2 {
        return vec![Curve::Line {
            start: points[0],
            end: points[1],
        }];
    }
    let count = points.len();
    let segments = if closed { count } else { count - 1 };
    let mut curves = Vec::with_capacity(segments);
    for index in 0..segments {
        let p1 = points[index % count];
        let p2 = points[(index + 1) % count];
        let p0 = if index == 0 && !closed {
            p1 * 2.0 - points[1]
        } else {
            points[(index + count - 1) % count]
        };
        let p3 = if index + 1 == segments && !closed {
            p2 * 2.0 - points[count - 2]
        } else {
            points[(index + 2) % count]
        };
        curves.push(Curve::Cubic {
            p0: p1,
            p1: p1 + (p2 - p0) / 6.0,
            p2: p2 - (p3 - p1) / 6.0,
            p3: p2,
            weights: [1.0; 4],
        });
    }
    curves
}

fn close_gap(curves: &mut Vec<Curve>) {
    let Some(first) = curves.first() else {
        return;
    };
    let Some(last) = curves.last() else {
        return;
    };
    let start = point_at_fraction(first, 0.0);
    let end = point_at_fraction(last, 1.0);
    if (end - start).length_squared() > 1e-12 {
        curves.push(Curve::Line {
            start: end,
            end: start,
        });
    }
}

fn xy_delta(point: Vector3, target: Vector3) -> f64 {
    let dx = point.x - target.x;
    let dy = point.y - target.y;
    (dx * dx + dy * dy).sqrt()
}

fn segment_xy(start: Vector3, end: Vector3, point: Vector2) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 1e-18 {
        0.0
    } else {
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let x = start.x + dx * t - point.x;
    let y = start.y + dy * t - point.y;
    (x * x + y * y).sqrt()
}

fn refine_elliptic(
    center: Vector3,
    u: Vector3,
    v: Vector3,
    start: f64,
    sweep: f64,
    target: Vector3,
    seed: f64,
) -> Vector3 {
    let mut t = seed;
    for _ in 0..6 {
        let (sin, cos) = t.sin_cos();
        let pos = center + u * cos + v * sin;
        let d1 = u * -sin + v * cos;
        let d2 = u * -cos + v * -sin;
        let diff = Vector3::new(pos.x - target.x, pos.y - target.y, 0.0);
        let slope = Vector3::new(d1.x, d1.y, 0.0);
        let curve = Vector3::new(d2.x, d2.y, 0.0);
        let f = diff.x * slope.x + diff.y * slope.y;
        let fp = slope.x * slope.x + slope.y * slope.y + diff.x * curve.x + diff.y * curve.y;
        if fp.abs() < 1e-12 {
            break;
        }
        t -= f / fp;
    }
    if !parameter_in_sweep(t, start, sweep) {
        let end = elliptic_point(center, u, v, start + sweep);
        let start_point = elliptic_point(center, u, v, start);
        if xy_delta(end, target) < xy_delta(start_point, target) {
            return end;
        }
        return start_point;
    }
    elliptic_point(center, u, v, t)
}

fn refine_cubic(points: &[Vector3; 4], weights: &[f64; 4], seed: f64, target: Vector3) -> f64 {
    let mut t = seed.clamp(0.0, 1.0);
    for _ in 0..6 {
        let dt = 1e-3;
        let left = cubic_point(points, weights, (t - dt).clamp(0.0, 1.0));
        let right = cubic_point(points, weights, (t + dt).clamp(0.0, 1.0));
        let mid = cubic_point(points, weights, t);
        let span = ((t + dt).clamp(0.0, 1.0) - (t - dt).clamp(0.0, 1.0)).max(1e-6);
        let slope_x = (right.x - left.x) / span;
        let slope_y = (right.y - left.y) / span;
        let diff_x = mid.x - target.x;
        let diff_y = mid.y - target.y;
        let f = diff_x * slope_x + diff_y * slope_y;
        let fp = slope_x * slope_x + slope_y * slope_y;
        if fp.abs() < 1e-12 {
            break;
        }
        t = (t - f / fp).clamp(0.0, 1.0);
    }
    t
}

fn parameter_in_sweep(t: f64, start: f64, sweep: f64) -> bool {
    if sweep.abs() >= TAU - 1e-6 {
        return true;
    }
    let rel = (t - start).rem_euclid(TAU);
    if sweep >= 0.0 {
        rel <= sweep + 1e-6
    } else {
        rel >= TAU + sweep - 1e-6 || rel <= 1e-6
    }
}

fn clip_elliptic(
    center: Vector3,
    u: Vector3,
    v: Vector3,
    start: f64,
    sweep: f64,
    polygon: &[Vector2],
) -> Vec<Curve> {
    let mut cuts = vec![start, start + sweep];
    for index in 0..polygon.len() {
        let edge_start = polygon[index];
        let edge_end = polygon[(index + 1) % polygon.len()];
        for hit in elliptic_edge_hits(center, u, v, edge_start, edge_end) {
            if let Some(t) = wrap_into_sweep(hit, start, sweep) {
                cuts.push(t);
            }
        }
    }
    cuts.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-7);
    let mut out = Vec::new();
    for pair in cuts.windows(2) {
        let mid = (pair[0] + pair[1]) * 0.5;
        let point = elliptic_point(center, u, v, mid);
        if super::clip::point_inside(point, polygon) && pair[1] - pair[0] > 1e-7 {
            out.push(Curve::Elliptic {
                center,
                u,
                v,
                start: pair[0],
                sweep: pair[1] - pair[0],
            });
        }
    }
    out
}

fn wrap_into_sweep(t: f64, start: f64, sweep: f64) -> Option<f64> {
    if sweep.abs() >= TAU - 1e-6 {
        let mut rel = (t - start).rem_euclid(TAU);
        if rel > TAU - 1e-8 {
            rel = 0.0;
        }
        return Some(start + rel);
    }
    let rel = (t - start).rem_euclid(TAU);
    if sweep >= 0.0 {
        if rel <= sweep + 1e-6 {
            Some(start + rel.min(sweep))
        } else {
            None
        }
    } else if rel >= TAU + sweep - 1e-6 {
        Some(start - (TAU - rel).min(-sweep))
    } else if rel <= 1e-6 {
        Some(start)
    } else {
        None
    }
}

fn elliptic_edge_hits(
    center: Vector3,
    u: Vector3,
    v: Vector3,
    start: Vector2,
    end: Vector2,
) -> Vec<f64> {
    let edge = Vector2::new(end.x - start.x, end.y - start.y);
    let cross = |x: f64, y: f64| x * edge.y - y * edge.x;
    let a = cross(u.x, u.y);
    let b = cross(v.x, v.y);
    let c = -cross(center.x - start.x, center.y - start.y);
    let radius = (a * a + b * b).sqrt();
    if radius < 1e-12 || c.abs() > radius + 1e-8 {
        return Vec::new();
    }
    let alpha = b.atan2(a);
    let delta = (c / radius).clamp(-1.0, 1.0).acos();
    let mut hits = Vec::new();
    for sign in [-1.0, 1.0] {
        let t = alpha + sign * delta;
        let point = elliptic_point(center, u, v, t);
        let len2 = edge.x * edge.x + edge.y * edge.y;
        if len2 <= 1e-18 {
            continue;
        }
        let s = ((point.x - start.x) * edge.x + (point.y - start.y) * edge.y) / len2;
        if (0.0..=1.0).contains(&s) {
            hits.push(t);
        }
    }
    hits
}

fn clip_cubic(points: &[[f64; 4]], polygon: &[Vector2], depth: usize) -> Vec<Curve> {
    if points.len() != 4 {
        return bezier_to_cubics(points, 0)
            .into_iter()
            .flat_map(|curve| clip_curves(&[curve], polygon))
            .collect();
    }
    let cartesian_points: Vec<Vector3> = points.iter().copied().map(cartesian).collect();
    if cartesian_points
        .iter()
        .all(|point| super::clip::point_inside(*point, polygon))
    {
        return vec![cubic_from_hom(points)];
    }
    if outside_one_edge(&cartesian_points, polygon) {
        return Vec::new();
    }
    if depth >= 10 {
        let mid = cartesian(de_casteljau(points, 0.5));
        return if super::clip::point_inside(mid, polygon) {
            vec![cubic_from_hom(points)]
        } else {
            Vec::new()
        };
    }
    let (left, right) = split_hom(points, 0.5);
    let mut out = clip_cubic(&left, polygon, depth + 1);
    out.extend(clip_cubic(&right, polygon, depth + 1));
    out
}

fn outside_one_edge(points: &[Vector3], polygon: &[Vector2]) -> bool {
    polygon.iter().enumerate().any(|(index, start)| {
        let end = polygon[(index + 1) % polygon.len()];
        points.iter().all(|point| {
            let cross =
                (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x);
            cross < -1e-7
        })
    })
}

#[cfg(test)]
mod analytic_tests {
    use super::*;

    #[test]
    fn semicircle_bulge_matches_the_sampled_crown() {
        let start = Vector3::new(0.0, 0.0, 0.0);
        let end = Vector3::new(10.0, 0.0, 0.0);
        let curve = bulge_curve(start, end, 1.0);
        let Curve::Elliptic { sweep, .. } = curve else {
            panic!("bulge 1 is a semicircle");
        };
        assert!((sweep.abs() - std::f64::consts::PI).abs() < 1e-9);
        let crown = point_at_fraction(&curve, 0.5);
        let samples = super::super::compile::tessellate_bulge(start, end, 1.0);
        let sampled = samples[samples.len() / 2];
        assert!((crown - sampled).length() < 1e-4);
        assert!(crown.y.abs() > 4.0, "crown fell back to the chord");
        assert!((curve_length(&curve) - 5.0 * std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn clamped_cubic_is_one_bezier() {
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 2.0, 0.0),
            Vector3::new(2.0, -1.0, 0.0),
            Vector3::new(3.0, 0.0, 0.0),
        ];
        let spline = Spline::from_control_points(3, points.clone());
        let curves = spline_curves(&spline);
        assert_eq!(curves.len(), 1);
        let Curve::Cubic {
            p0,
            p1,
            p2,
            p3,
            weights,
        } = &curves[0]
        else {
            panic!("expected a cubic");
        };
        assert_eq!(weights, &[1.0, 1.0, 1.0, 1.0]);
        for (got, expect) in [p0, p1, p2, p3].into_iter().zip(&points) {
            assert!((*got - *expect).length() < 1e-9);
        }
    }

    #[test]
    fn rational_quarter_circle_stays_on_the_circle() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let control = vec![
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ];
        let knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let curves = nurbs_curves(2, &control, &[1.0, w, 1.0], &knots);
        assert_eq!(curves.len(), 1);
        let Curve::Cubic {
            p0,
            p1,
            p2,
            p3,
            weights,
        } = &curves[0]
        else {
            panic!("quarter circle elevates to one cubic");
        };
        let mid = cubic_point(&[*p0, *p1, *p2, *p3], weights, 0.5);
        let radius = (mid.x * mid.x + mid.y * mid.y).sqrt();
        assert!(
            (radius - 1.0).abs() < 1e-5 && (mid.x - mid.y).abs() < 1e-5,
            "midpoint {mid:?} left the quarter circle"
        );
    }
}
