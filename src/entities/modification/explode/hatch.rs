//! EXPLODE implementation for patterned HATCH entities.
//!
//! ACAD explodes a patterned hatch to the visible portions of its pattern,
//! rather than to the boundary edges.  The implementation below generates the
//! infinite pattern families, intersects them with the analytic hatch edges,
//! applies the hatch fill rule, and emits the clipped dashes as LINE/POINT
//! entities.  Solid and gradient hatches are intentionally atomic (ACAD
//! leaves those entities in the drawing when EXPLODE is invoked).

use std::f64::consts::TAU;

use crate::entities::hatch::{
    BoundaryEdge, BoundaryPath, EllipticArcEdge, Hatch, HatchStyleType, LineEdge,
    PolylineEdge, SplineEdge,
};
use crate::entities::{EntityCommon, EntityType, Line, Point};
use crate::types::{Handle, Matrix3, Vector2, Vector3};

const EPS: f64 = 1.0e-9;
const INTERSECTION_EPS: f64 = 1.0e-7;

// Kept local so this module remains independently testable while the parent
// dispatch is being wired.  Its fields intentionally match explode.rs's
// shared `inherit_common` helper.
fn inherit_common(source: &EntityCommon) -> EntityCommon {
    EntityCommon {
        handle: Handle::NULL,
        layer: source.layer.clone(),
        layer_handle: source.layer_handle,
        color: source.color,
        line_weight: source.line_weight,
        linetype: source.linetype.clone(),
        linetype_scale: source.linetype_scale,
        transparency: source.transparency,
        color_name: source.color_name.clone(),
        color_book_handle: source.color_book_handle,
        linetype_handle: source.linetype_handle,
        full_visual_style_handle: source.full_visual_style_handle,
        face_visual_style_handle: source.face_visual_style_handle,
        edge_visual_style_handle: source.edge_visual_style_handle,
        material_flags: source.material_flags,
        material_handle: source.material_handle,
        shadow_flags: source.shadow_flags,
        plotstyle_flags: source.plotstyle_flags,
        plotstyle_handle: source.plotstyle_handle,
        entity_mode: source.entity_mode,
        invisible: source.invisible,
        owner_handle: source.owner_handle,
        ..EntityCommon::new()
    }
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Vector2,
    max: Vector2,
}

impl Bounds {
    fn empty() -> Self {
        Self {
            min: Vector2::new(f64::INFINITY, f64::INFINITY),
            max: Vector2::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        }
    }

    fn add(&mut self, p: Vector2) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
    }

    fn valid(&self) -> bool {
        self.min.x.is_finite()
            && self.min.y.is_finite()
            && self.max.x.is_finite()
            && self.max.y.is_finite()
            && self.max.x >= self.min.x
            && self.max.y >= self.min.y
    }

    fn diagonal(&self) -> f64 {
        (self.max - self.min).length()
    }
}

#[derive(Clone, Debug)]
struct LoopGeometry {
    /// A polygonal representation used only for the point-in-loop test.
    polygon: Vec<Vector2>,
}

/// Explode a patterned HATCH into visible pattern primitives.
///
/// This is `pub(super)` so the dispatch in `explode.rs` can remain the public
/// API.  A solid or gradient HATCH returns an empty vector, matching native
/// ACAD EXPLODE (the source object survives).
pub(super) fn explode_hatch(hatch: &Hatch) -> Vec<EntityType> {
    if hatch.is_solid || hatch.gradient_color.enabled || hatch.pattern.lines.is_empty() {
        return Vec::new();
    }

    let mut bounds = Bounds::empty();
    let loops: Vec<LoopGeometry> = hatch
        .paths
        .iter()
        .map(|path| {
            let polygon = path_polygon(path, &mut bounds);
            LoopGeometry { polygon }
        })
        .filter(|l| l.polygon.len() >= 3)
        .collect();
    if loops.is_empty() || !bounds.valid() {
        return Vec::new();
    }

    let scale = if hatch.pattern_scale.is_finite() && hatch.pattern_scale.abs() > EPS {
        hatch.pattern_scale.abs()
    } else {
        1.0
    };
    let mut result = Vec::new();
    for line in &hatch.pattern.lines {
        let dash_lengths: Vec<f64> = line
            .dash_lengths
            .iter()
            .filter(|d| d.is_finite())
            .map(|d| *d * scale)
            .collect();
        // DXF's double-pattern bit requests a second family rotated by 90°.
        // The second family uses the same dash phase and spacing, rotated
        // around the recorded hatch pattern origin.
        let family_count = if hatch.is_double { 2 } else { 1 };
        for family in 0..family_count {
            let mut angle = line.angle
                + if family == 1 {
                    std::f64::consts::FRAC_PI_2
                } else {
                    0.0
                };
            // DXF stores the definition-line angle and the HATCH rotation
            // independently.  `pattern_angle` is therefore applied exactly once
            // here, after parsing.  Pattern builders that have already folded the
            // rotation into line.angle leave pattern_angle at zero.
            angle += hatch.pattern_angle;
            if !angle.is_finite() {
                continue;
            }
            let dir = Vector2::new(angle.cos(), angle.sin()).normalize();
            if dir.length_squared() < EPS {
                continue;
            }
            let mut offset = line.offset * scale;
            if family == 1 {
                offset = Vector2::new(-offset.y, offset.x);
            }
            if offset.length_squared() < EPS {
                // A zero offset is legal for a single definition line.  Avoid an
                // unbounded loop while preserving that one line.
                offset = Vector2::ZERO;
            }
            let origin = if family == 1 {
                let hatch_origin = hatch.pattern_origin();
                let q = line.base_point - hatch_origin;
                hatch_origin + Vector2::new(-q.y, q.x)
            } else {
                line.base_point
            };
            let line_count = line_indices(origin, offset, dir, &bounds);
            for index in line_count {
                let p = origin + offset * index as f64;
                let intersections = line_intersections(hatch, p, dir);
                if intersections.len() < 2 {
                    continue;
                }
                for pair in intersections.windows(2) {
                    let (ta, tb) = (pair[0], pair[1]);
                    if tb - ta <= INTERSECTION_EPS {
                        continue;
                    }
                    let mid = p + dir * ((ta + tb) * 0.5);
                    if !inside_fill(mid, &loops, hatch.style) {
                        continue;
                    }
                    if dash_lengths.is_empty() {
                        emit_line(
                            &mut result,
                            p + dir * ta,
                            p + dir * tb,
                            hatch,
                            &normal_basis(hatch.normal),
                        );
                    } else {
                        emit_dashes(&mut result, p, dir, ta, tb, &dash_lengths, hatch);
                    }
                }
            }
        }
    }
    result
}

fn normal_basis(normal: Vector3) -> Matrix3 {
    Matrix3::arbitrary_axis(if normal.length() > EPS {
        normal.normalize()
    } else {
        Vector3::UNIT_Z
    })
}

fn line_indices(origin: Vector2, offset: Vector2, dir: Vector2, bounds: &Bounds) -> Vec<i32> {
    if offset.length_squared() < EPS {
        return vec![0];
    }
    let diagonal = bounds.diagonal().max(1.0);
    let spacing = offset.length();
    // The projection of all boundary points onto the offset direction gives a
    // tighter range than an arbitrary world-size cap.
    let normal = offset.normalize();
    let corners = [
        bounds.min,
        Vector2::new(bounds.max.x, bounds.min.y),
        bounds.max,
        Vector2::new(bounds.min.x, bounds.max.y),
    ];
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for c in corners {
        let q = (c - origin).dot(&normal);
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let mut first = (lo / spacing).floor() as i32 - 2;
    let mut last = (hi / spacing).ceil() as i32 + 2;
    // Degenerate or malformed offsets should still produce a finite result.
    let cap = (diagonal / spacing).ceil() as i32 + 8;
    first = first.max(-cap);
    last = last.min(cap);
    // Keep the unused argument explicit: it documents that direction controls
    // the line while offset controls family spacing.
    let _ = dir;
    // ACAD walks the visible scan lines from the positive offset side
    // toward the negative side (the order is observable in the generated
    // entity handles), so preserve that deterministic ordering.
    (first..=last).rev().collect()
}

fn emit_line(result: &mut Vec<EntityType>, a: Vector2, b: Vector2, hatch: &Hatch, basis: &Matrix3) {
    if a.distance(&b) <= EPS {
        result.push(EntityType::Point(Point {
            common: inherit_common(&hatch.common),
            location: *basis * Vector3::new(a.x, a.y, hatch.elevation),
            thickness: 0.0,
            normal: hatch.normal,
            x_axis_angle: 0.0,
        }));
    } else {
        result.push(EntityType::Line(Line {
            common: inherit_common(&hatch.common),
            start: *basis * Vector3::new(a.x, a.y, hatch.elevation),
            end: *basis * Vector3::new(b.x, b.y, hatch.elevation),
            thickness: 0.0,
            normal: hatch.normal,
        }));
    }
}

fn emit_dashes(
    result: &mut Vec<EntityType>,
    origin: Vector2,
    dir: Vector2,
    ta: f64,
    tb: f64,
    dashes: &[f64],
    hatch: &Hatch,
) {
    let total: f64 = dashes.iter().map(|d| d.abs()).sum();
    if total <= EPS {
        emit_line(
            result,
            origin + dir * ta,
            origin + dir * tb,
            hatch,
            &normal_basis(hatch.normal),
        );
        return;
    }
    let mut t = ta;
    // DXF dash phase is measured from the definition line base point.  Find
    // the active element at ta, then advance element by element to tb.
    let mut phase = ta.rem_euclid(total);
    let mut element = 0usize;
    while element + 1 < dashes.len() && phase >= dashes[element].abs() - EPS {
        phase -= dashes[element].abs();
        element += 1;
    }
    let mut remaining = (dashes[element].abs() - phase).max(0.0);
    while t < tb - EPS {
        let next = (t + remaining).min(tb);
        let value = dashes[element];
        if value > EPS {
            let basis = normal_basis(hatch.normal);
            emit_line(result, origin + dir * t, origin + dir * next, hatch, &basis);
        }
        t = next;
        element = (element + 1) % dashes.len();
        remaining = dashes[element].abs();
        if remaining <= EPS {
            // A zero dash is a dot at the current location.  Emit exactly one
            // POINT and advance to the following pattern element.
            if dashes[element].abs() <= EPS && dashes[element] >= 0.0 {
                let basis = normal_basis(hatch.normal);
                emit_line(result, origin + dir * t, origin + dir * t, hatch, &basis);
            }
            element = (element + 1) % dashes.len();
            remaining = dashes[element].abs();
        }
    }
}

fn path_polygon(path: &BoundaryPath, bounds: &mut Bounds) -> Vec<Vector2> {
    let mut polygon = Vec::new();
    for edge in &path.edges {
        let samples = edge_samples(edge);
        for p in samples {
            bounds.add(p);
            if polygon
                .last()
                .map_or(true, |q: &Vector2| q.distance(&p) > 1e-8)
            {
                polygon.push(p);
            }
        }
    }
    if polygon.len() > 1 && polygon[0].distance(polygon.last().unwrap()) <= 1e-8 {
        polygon.pop();
    }
    polygon
}

fn edge_samples(edge: &BoundaryEdge) -> Vec<Vector2> {
    match edge {
        BoundaryEdge::Line(e) => vec![e.start, e.end],
        BoundaryEdge::CircularArc(e) => {
            let (start, end) = if e.counter_clockwise {
                (e.start_angle, e.end_angle)
            } else {
                (-e.start_angle, -e.end_angle)
            };
            let sweep = oriented_sweep(start, end, e.counter_clockwise);
            let n = ((sweep.abs() * 16.0).ceil() as usize).clamp(4, 96);
            (0..=n)
                .map(|i| {
                    let a = start
                        + if e.counter_clockwise { 1.0 } else { -1.0 } * sweep.abs() * i as f64
                            / n as f64;
                    e.center + Vector2::new(e.radius * a.cos(), e.radius * a.sin())
                })
                .collect()
        }
        BoundaryEdge::EllipticArc(e) => {
            let (start, end) = if e.counter_clockwise {
                (e.start_angle, e.end_angle)
            } else {
                (-e.start_angle, -e.end_angle)
            };
            let sweep = oriented_sweep(start, end, e.counter_clockwise);
            let n = ((sweep.abs() * 16.0).ceil() as usize).clamp(8, 128);
            let major = e.major_axis_endpoint;
            let a = major.length();
            let ux = if a > EPS { major / a } else { Vector2::UNIT_X };
            let uy = Vector2::new(-ux.y, ux.x);
            (0..=n)
                .map(|i| {
                    let t = start
                        + if e.counter_clockwise { 1.0 } else { -1.0 } * sweep.abs() * i as f64
                            / n as f64;
                    e.center + ux * (a * t.cos()) + uy * (a * e.minor_axis_ratio * t.sin())
                })
                .collect()
        }
        BoundaryEdge::Spline(e) => spline_samples(e),
        BoundaryEdge::Polyline(e) => polyline_samples(e),
    }
}

fn polyline_samples(e: &PolylineEdge) -> Vec<Vector2> {
    if e.vertices.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let count = if e.is_closed {
        e.vertices.len()
    } else {
        e.vertices.len().saturating_sub(1)
    };
    for i in 0..count {
        let a = e.vertices[i];
        let b = e.vertices[(i + 1) % e.vertices.len()];
        let bulge = a.z;
        if bulge.abs() <= EPS {
            out.push(Vector2::new(a.x, a.y));
            if i + 1 == count {
                out.push(Vector2::new(b.x, b.y));
            }
        } else {
            let (center, radius, start, end, ccw) = bulge_arc(a, b, bulge);
            let sweep = oriented_sweep(start, end, ccw);
            let n = ((sweep.abs() * 16.0).ceil() as usize).clamp(4, 96);
            for j in 0..=n {
                let t = start + if ccw { 1.0 } else { -1.0 } * sweep.abs() * j as f64 / n as f64;
                let p = center + Vector2::new(radius * t.cos(), radius * t.sin());
                if out.last().map_or(true, |q: &Vector2| q.distance(&p) > 1e-8) {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn spline_samples(e: &SplineEdge) -> Vec<Vector2> {
    if e.control_points.len() >= 2 {
        let cps: Vec<Vector3> = e
            .control_points
            .iter()
            .map(|p| Vector3::new(p.x, p.y, 0.0))
            .collect();
        let weights: Vec<f64> = if e.rational {
            e.control_points.iter().map(|p| p.z).collect()
        } else {
            vec![1.0; cps.len()]
        };
        let degree = usize::try_from(e.degree).unwrap_or(3).max(1);
        let points = eval_nurbs(degree, &cps, &weights, &e.knots, 32);
        points.into_iter().map(|p| Vector2::new(p.x, p.y)).collect()
    } else {
        e.fit_points.clone()
    }
}

fn eval_nurbs(
    degree: usize,
    cps: &[Vector3],
    weights: &[f64],
    knots_in: &[f64],
    per_span: usize,
) -> Vec<Vector3> {
    let n = cps.len();
    if n < 2 {
        return cps.to_vec();
    }
    let p = degree.min(n - 1).max(1);
    let knots: Vec<f64> =
        if knots_in.len() == n + p + 1 && knots_in.windows(2).all(|w| w[1] >= w[0]) {
            knots_in.to_vec()
        } else {
            let inner = n - p;
            (0..n + p + 1)
                .map(|i| {
                    if i <= p {
                        0.0
                    } else if i >= n {
                        1.0
                    } else {
                        (i - p) as f64 / inner as f64
                    }
                })
                .collect()
        };
    let hom: Vec<[f64; 4]> = cps
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let w = weights.get(i).copied().unwrap_or(1.0).max(EPS);
            [c.x * w, c.y * w, c.z * w, w]
        })
        .collect();
    let eval = |u: f64, k: usize| {
        let mut d: Vec<[f64; 4]> = (0..=p).map(|j| hom[j + k - p]).collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = j + k - p;
                let den = knots[i + p + 1 - r] - knots[i];
                let alpha = if den.abs() <= EPS {
                    0.0
                } else {
                    (u - knots[i]) / den
                };
                let (prev, cur) = (d[j - 1], d[j]);
                d[j] = std::array::from_fn(|c| (1.0 - alpha) * prev[c] + alpha * cur[c]);
            }
        }
        let w = if d[p][3].abs() <= EPS { 1.0 } else { d[p][3] };
        Vector3::new(d[p][0] / w, d[p][1] / w, d[p][2] / w)
    };
    let mut out = Vec::new();
    for k in p..n {
        if knots[k + 1] - knots[k] <= EPS {
            continue;
        }
        let first = if out.is_empty() { 0 } else { 1 };
        for i in first..=per_span.max(1) {
            let u = knots[k] + (knots[k + 1] - knots[k]) * i as f64 / per_span.max(1) as f64;
            out.push(eval(u, k));
        }
    }
    out
}

fn oriented_sweep(start: f64, end: f64, ccw: bool) -> f64 {
    let mut d = if ccw { end - start } else { start - end };
    while d < 0.0 {
        d += TAU;
    }
    while d >= TAU {
        d -= TAU;
    }
    if d.abs() <= EPS && (end - start).abs() > EPS {
        TAU
    } else {
        d
    }
}

fn angle_on_arc(theta: f64, start: f64, end: f64, ccw: bool) -> bool {
    let sweep = oriented_sweep(start, end, ccw);
    let from = if ccw { theta - start } else { start - theta };
    let mut d = from % TAU;
    if d < 0.0 {
        d += TAU;
    }
    d <= sweep + INTERSECTION_EPS
}

fn line_intersections(hatch: &Hatch, p: Vector2, d: Vector2) -> Vec<f64> {
    let mut out = Vec::new();
    for path in &hatch.paths {
        for edge in &path.edges {
            match edge {
                BoundaryEdge::Line(e) => intersect_line_segment(p, d, e, &mut out),
                BoundaryEdge::CircularArc(e) => {
                    let (start, end) = if e.counter_clockwise {
                        (e.start_angle, e.end_angle)
                    } else {
                        (-e.start_angle, -e.end_angle)
                    };
                    intersect_circle_arc(
                        p,
                        d,
                        e.center,
                        e.radius,
                        start,
                        end,
                        e.counter_clockwise,
                        &mut out,
                    );
                }
                BoundaryEdge::EllipticArc(e) => {
                    let (start, end) = if e.counter_clockwise {
                        (e.start_angle, e.end_angle)
                    } else {
                        (-e.start_angle, -e.end_angle)
                    };
                    intersect_ellipse_arc(p, d, e, start, end, &mut out);
                }
                BoundaryEdge::Polyline(e) => intersect_polyline(p, d, e, &mut out),
                BoundaryEdge::Spline(e) => intersect_spline(p, d, e, &mut out),
            }
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out.dedup_by(|a, b| (*a - *b).abs() <= INTERSECTION_EPS);
    out
}

fn intersect_line_segment(p: Vector2, d: Vector2, e: &LineEdge, out: &mut Vec<f64>) {
    let v = e.end - e.start;
    let den = d.cross(&v);
    if den.abs() <= EPS {
        return;
    }
    let q = e.start - p;
    let t = q.cross(&v) / den;
    let u = q.cross(&d) / den;
    if u >= -INTERSECTION_EPS && u <= 1.0 + INTERSECTION_EPS {
        out.push(t);
    }
}

fn intersect_circle_arc(
    p: Vector2,
    d: Vector2,
    center: Vector2,
    radius: f64,
    start: f64,
    end: f64,
    counter_clockwise: bool,
    out: &mut Vec<f64>,
) {
    let q = p - center;
    let b = 2.0 * q.dot(&d);
    let c = q.dot(&q) - radius * radius;
    let disc = b * b - 4.0 * c;
    if disc < -EPS {
        return;
    }
    let root = disc.max(0.0).sqrt();
    for t in [(-b - root) * 0.5, (-b + root) * 0.5] {
        let point = p + d * t;
        let theta = (point.y - center.y).atan2(point.x - center.x);
        if angle_on_arc(theta, start, end, counter_clockwise) {
            out.push(t);
        }
    }
}

fn intersect_ellipse_arc(
    p: Vector2,
    d: Vector2,
    e: &EllipticArcEdge,
    start: f64,
    end: f64,
    out: &mut Vec<f64>,
) {
    let a = e.major_axis_endpoint.length();
    let b = a * e.minor_axis_ratio.abs();
    if a <= EPS || b <= EPS {
        return;
    }
    let ux = e.major_axis_endpoint / a;
    let uy = Vector2::new(-ux.y, ux.x);
    let q = p - e.center;
    let x0 = q.dot(&ux);
    let y0 = q.dot(&uy);
    let dx = d.dot(&ux);
    let dy = d.dot(&uy);
    let aa = dx * dx / (a * a) + dy * dy / (b * b);
    let bb = 2.0 * (x0 * dx / (a * a) + y0 * dy / (b * b));
    let cc = x0 * x0 / (a * a) + y0 * y0 / (b * b) - 1.0;
    let disc = bb * bb - 4.0 * aa * cc;
    if disc < -EPS || aa.abs() <= EPS {
        return;
    }
    let root = disc.max(0.0).sqrt();
    for t in [(-bb - root) / (2.0 * aa), (-bb + root) / (2.0 * aa)] {
        let point = p + d * t;
        let q = point - e.center;
        let theta = (q.dot(&uy) / b).atan2(q.dot(&ux) / a);
        if angle_on_arc(theta, start, end, e.counter_clockwise) {
            out.push(t);
        }
    }
}

fn intersect_polyline(p: Vector2, d: Vector2, e: &PolylineEdge, out: &mut Vec<f64>) {
    let count = if e.is_closed {
        e.vertices.len()
    } else {
        e.vertices.len().saturating_sub(1)
    };
    for i in 0..count {
        let a = e.vertices[i];
        let b = e.vertices[(i + 1) % e.vertices.len()];
        if a.z.abs() <= EPS {
            intersect_line_segment(
                p,
                d,
                &LineEdge {
                    start: Vector2::new(a.x, a.y),
                    end: Vector2::new(b.x, b.y),
                },
                out,
            );
        } else {
            let (center, radius, start, end, ccw) = bulge_arc(a, b, a.z);
            intersect_circle_arc(p, d, center, radius, start, end, ccw, out);
        }
    }
}

fn intersect_spline(p: Vector2, d: Vector2, e: &SplineEdge, out: &mut Vec<f64>) {
    let points = spline_samples(e);
    for pair in points.windows(2) {
        intersect_line_segment(
            p,
            d,
            &LineEdge {
                start: pair[0],
                end: pair[1],
            },
            out,
        );
    }
}

fn bulge_arc(
    a: crate::types::Vector3,
    b: crate::types::Vector3,
    bulge: f64,
) -> (Vector2, f64, f64, f64, bool) {
    let p1 = Vector2::new(a.x, a.y);
    let p2 = Vector2::new(b.x, b.y);
    let chord = p2 - p1;
    let d = chord.length().max(EPS);
    let theta = 4.0 * bulge.atan();
    let radius = d * (1.0 + bulge * bulge) / (4.0 * bulge.abs().max(EPS));
    let mid = (p1 + p2) * 0.5;
    let perp = Vector2::new(-chord.y / d, chord.x / d);
    let center = mid + perp * (d * (1.0 - bulge * bulge) / (4.0 * bulge));
    let s = (p1 - center).y.atan2((p1 - center).x);
    let e = (p2 - center).y.atan2((p2 - center).x);
    (center, radius, s, e, theta > 0.0)
}

fn inside_fill(point: Vector2, loops: &[LoopGeometry], style: HatchStyleType) -> bool {
    let contains: Vec<bool> = loops
        .iter()
        .map(|l| point_in_polygon(point, &l.polygon))
        .collect();
    match style {
        HatchStyleType::Ignore => contains.iter().any(|v| *v),
        HatchStyleType::Outer => {
            // The outermost loop is the one with the largest area.  ACAD's
            // OUTER style ignores all nested islands.
            let outer = loops
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    polygon_area(&a.polygon)
                        .abs()
                        .partial_cmp(&polygon_area(&b.polygon).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i);
            outer.map_or(false, |i| contains[i])
        }
        HatchStyleType::Normal => contains.into_iter().filter(|v| *v).count() % 2 == 1,
    }
}

fn polygon_area(poly: &[Vector2]) -> f64 {
    poly.iter()
        .zip(poly.iter().cycle().skip(1))
        .take(poly.len())
        .map(|(a, b)| a.cross(b))
        .sum::<f64>()
        * 0.5
}

fn point_in_polygon(point: Vector2, poly: &[Vector2]) -> bool {
    let mut inside = false;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[(i + 1) % poly.len()];
        if point_segment_distance(point, a, b) <= INTERSECTION_EPS {
            return true;
        }
        if (a.y > point.y) != (b.y > point.y) {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

fn point_segment_distance(p: Vector2, a: Vector2, b: Vector2) -> f64 {
    let v = b - a;
    let t = ((p - a).dot(&v) / v.length_squared().max(EPS)).clamp(0.0, 1.0);
    p.distance(&(a + v * t))
}

// ── area ─────────────────────────────────────────────────────────────────────

/// The filled area of a hatch, as its `Area` property reports it: each loop's
/// exact area (arcs and elliptic arcs integrated, splines finely sampled),
/// with islands subtracted according to the hatch style.
pub(crate) fn hatch_area(hatch: &Hatch) -> Option<f64> {
    let loops: Vec<(f64, Vec<Vector2>)> = hatch
        .paths
        .iter()
        .filter_map(|path| {
            let polygon = path_polygon(path, &mut Bounds::empty());
            (polygon.len() >= 3).then(|| (loop_area(path).abs(), polygon))
        })
        .collect();
    if loops.is_empty() {
        return None;
    }
    let total = loops
        .iter()
        .enumerate()
        .map(|(i, (area, polygon))| {
            let depth = loops
                .iter()
                .enumerate()
                .filter(|(j, (outer, outer_polygon))| {
                    *j != i && *outer > *area && loop_inside(polygon, outer_polygon)
                })
                .count();
            let sign = match (hatch.style, depth) {
                (_, 0) => 1.0,
                (HatchStyleType::Ignore, _) => 0.0,
                (HatchStyleType::Outer, d) if d > 1 => 0.0,
                (_, d) if d % 2 == 1 => -1.0,
                _ => 1.0,
            };
            sign * area
        })
        .sum::<f64>();
    total.is_finite().then_some(total.max(0.0))
}

/// Signed area enclosed by a boundary loop (counter-clockwise positive), by
/// Green's theorem over its edges; a gap between consecutive edges closes
/// with a straight segment.
fn loop_area(path: &BoundaryPath) -> f64 {
    let mut twice = 0.0;
    let mut ends: Vec<(Vector2, Vector2)> = Vec::new();
    for edge in &path.edges {
        let Some((integral, start, end)) = edge_integral(edge) else {
            continue;
        };
        twice += integral;
        ends.push((start, end));
    }
    for i in 0..ends.len() {
        twice += ends[i].1.cross(&ends[(i + 1) % ends.len()].0);
    }
    twice / 2.0
}

/// `∮ x dy − y dx` along one edge, with the edge's start and end points.
fn edge_integral(edge: &BoundaryEdge) -> Option<(f64, Vector2, Vector2)> {
    // The parameter range an arc edge runs over, in its direction of travel.
    let range = |start: f64, end: f64, ccw: bool| {
        if ccw {
            (start, start + oriented_sweep(start, end, true))
        } else {
            (-start, -start - oriented_sweep(-start, -end, false))
        }
    };
    Some(match edge {
        BoundaryEdge::Line(e) => (e.start.cross(&e.end), e.start, e.end),
        BoundaryEdge::CircularArc(e) => {
            let (a, b) = range(e.start_angle, e.end_angle, e.counter_clockwise);
            let (c, r) = (e.center, e.radius);
            let at = |t: f64| c + Vector2::new(r * t.cos(), r * t.sin());
            let integral =
                r * r * (b - a) + r * c.x * (b.sin() - a.sin()) - r * c.y * (b.cos() - a.cos());
            (integral, at(a), at(b))
        }
        BoundaryEdge::EllipticArc(e) => {
            let (a, b) = range(e.start_angle, e.end_angle, e.counter_clockwise);
            let (c, major) = (e.center, e.major_axis_endpoint);
            let minor = Vector2::new(-major.y, major.x) * e.minor_axis_ratio;
            let at = |t: f64| c + major * t.cos() + minor * t.sin();
            let integral = major.cross(&minor) * (b - a)
                + c.cross(&major) * (b.cos() - a.cos())
                + c.cross(&minor) * (b.sin() - a.sin());
            (integral, at(a), at(b))
        }
        BoundaryEdge::Polyline(e) => {
            let v = &e.vertices;
            let first = v.first()?;
            let segments = if e.is_closed { v.len() } else { v.len() - 1 };
            let integral = (0..segments)
                .map(|i| {
                    let (p, q) = (v[i], v[(i + 1) % v.len()]);
                    let chord = Vector2::new(p.x, p.y).cross(&Vector2::new(q.x, q.y));
                    let theta = 4.0 * p.z.atan();
                    let half_sin = (theta / 2.0).sin();
                    if half_sin.abs() < EPS {
                        return chord;
                    }
                    let r = ((q.x - p.x).hypot(q.y - p.y)) / (2.0 * half_sin.abs());
                    chord + r * r * (theta - theta.sin())
                })
                .sum::<f64>();
            let last = if e.is_closed { first } else { v.last()? };
            (
                integral,
                Vector2::new(first.x, first.y),
                Vector2::new(last.x, last.y),
            )
        }
        BoundaryEdge::Spline(e) => {
            let points: Vec<Vector2> = if e.control_points.len() >= 2 {
                let cps: Vec<Vector3> = e
                    .control_points
                    .iter()
                    .map(|p| Vector3::new(p.x, p.y, 0.0))
                    .collect();
                let weights: Vec<f64> = if e.rational {
                    e.control_points.iter().map(|p| p.z).collect()
                } else {
                    vec![1.0; cps.len()]
                };
                let degree = usize::try_from(e.degree).unwrap_or(3).max(1);
                eval_nurbs(degree, &cps, &weights, &e.knots, 256)
                    .into_iter()
                    .map(|p| Vector2::new(p.x, p.y))
                    .collect()
            } else {
                e.fit_points.clone()
            };
            let integral = points.windows(2).map(|w| w[0].cross(&w[1])).sum::<f64>();
            (integral, *points.first()?, *points.last()?)
        }
    })
}

/// Whether loop `inner` lies inside loop `outer`, judged at a point well
/// inside `inner`: neighbouring loops that share an edge overlap only along
/// it, so a corner can fall a drafting tolerance inside the other loop.
fn loop_inside(inner: &[Vector2], outer: &[Vector2]) -> bool {
    point_in_polygon(interior_point(inner), outer)
}

/// The middle of the widest span of `poly` along the horizontal line through
/// the middle of its bounding box.
fn interior_point(poly: &[Vector2]) -> Vector2 {
    let (lo, hi) = poly
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.y), hi.max(p.y))
        });
    let y = (lo + hi) / 2.0;
    let mut xs: Vec<f64> = (0..poly.len())
        .filter_map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            ((a.y > y) != (b.y > y)).then(|| a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y))
        })
        .collect();
    xs.sort_by(f64::total_cmp);
    xs.chunks_exact(2)
        .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
        .map_or(poly[0], |span| Vector2::new((span[0] + span[1]) / 2.0, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::hatch::{BoundaryPathFlags, HatchPattern, HatchPatternLine};

    fn square_hatch() -> Hatch {
        let mut h = Hatch::with_pattern(HatchPattern::new("TEST"));
        let mut p = BoundaryPath::with_flags(BoundaryPathFlags::EXTERNAL);
        p.add_edge(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(0.0, 0.0),
            end: Vector2::new(10.0, 0.0),
        }));
        p.add_edge(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(10.0, 0.0),
            end: Vector2::new(10.0, 10.0),
        }));
        p.add_edge(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(10.0, 10.0),
            end: Vector2::new(0.0, 10.0),
        }));
        p.add_edge(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(0.0, 10.0),
            end: Vector2::new(0.0, 0.0),
        }));
        h.add_path(p);
        h
    }

    #[test]
    fn solid_and_gradient_hatches_are_atomic() {
        let mut h = square_hatch();
        h.is_solid = true;
        assert!(explode_hatch(&h).is_empty());
        h.is_solid = false;
        h.gradient_color.enabled = true;
        assert!(explode_hatch(&h).is_empty());
    }

    #[test]
    fn diagonal_pattern_is_clipped_to_square() {
        let mut h = square_hatch();
        h.pattern.lines.push(HatchPatternLine {
            angle: std::f64::consts::FRAC_PI_4,
            base_point: Vector2::ZERO,
            offset: Vector2::new(-2.24506, 2.24506),
            dash_lengths: Vec::new(),
        });
        let parts = explode_hatch(&h);
        assert_eq!(parts.len(), 5);
        for part in parts {
            match part {
                EntityType::Line(line) => {
                    assert!(line.start.x >= -1e-7 && line.start.x <= 10.0 + 1e-7);
                    assert!(line.end.x >= -1e-7 && line.end.x <= 10.0 + 1e-7);
                    assert!(line.start.y >= -1e-7 && line.start.y <= 10.0 + 1e-7);
                    assert!(line.end.y >= -1e-7 && line.end.y <= 10.0 + 1e-7);
                }
                _ => panic!("patterned square should emit LINEs"),
            }
        }
    }

    #[test]
    fn normal_style_excludes_island_and_zero_dash_emits_point() {
        let mut h = square_hatch();
        let mut island = BoundaryPath::new();
        for (a, b) in [
            ((3.0, 3.0), (7.0, 3.0)),
            ((7.0, 3.0), (7.0, 7.0)),
            ((7.0, 7.0), (3.0, 7.0)),
            ((3.0, 7.0), (3.0, 3.0)),
        ] {
            island.add_edge(BoundaryEdge::Line(LineEdge {
                start: Vector2::new(a.0, a.1),
                end: Vector2::new(b.0, b.1),
            }));
        }
        h.add_path(island);
        h.pattern.lines.push(HatchPatternLine {
            angle: 0.0,
            base_point: Vector2::ZERO,
            offset: Vector2::new(0.0, 2.0),
            dash_lengths: vec![0.0, -1.0],
        });
        let parts = explode_hatch(&h);
        assert!(parts.iter().any(|e| matches!(e, EntityType::Point(_))));
        // A line at y=5 is split around the island; no emitted LINE may have
        // both endpoints strictly inside it.
        for part in parts {
            if let EntityType::Line(line) = part {
                let mid = Vector2::new(
                    (line.start.x + line.end.x) * 0.5,
                    (line.start.y + line.end.y) * 0.5,
                );
                assert!(!(mid.x > 3.0 && mid.x < 7.0 && mid.y > 3.0 && mid.y < 7.0));
            }
        }
    }

    #[test]
    fn ocs_output_uses_hatch_normal_and_elevation() {
        let mut h = square_hatch();
        h.elevation = 4.0;
        h.normal = Vector3::new(0.0, 1.0, 0.0);
        h.pattern.lines.push(HatchPatternLine {
            angle: 0.0,
            base_point: Vector2::new(0.0, 5.0),
            offset: Vector2::new(0.0, 10.0),
            dash_lengths: Vec::new(),
        });
        let parts = explode_hatch(&h);
        assert!(!parts.is_empty());
        if let EntityType::Line(line) = &parts[0] {
            assert!((line.start.y - 4.0).abs() < 1e-7);
            assert!((line.normal.y - 1.0).abs() < 1e-7);
        } else {
            panic!("expected a clipped line");
        }
    }

    #[test]
    fn double_pattern_adds_the_perpendicular_family() {
        let mut h = square_hatch();
        h.is_double = true;
        h.pattern.lines.push(HatchPatternLine {
            angle: 0.0,
            base_point: Vector2::ZERO,
            offset: Vector2::new(0.0, 2.0),
            dash_lengths: Vec::new(),
        });
        let parts = explode_hatch(&h);
        assert!(parts.len() >= 8);
        let horizontal = parts
            .iter()
            .filter(|e| match e {
                EntityType::Line(line) => (line.start.y - line.end.y).abs() < 1e-7,
                _ => false,
            })
            .count();
        let vertical = parts
            .iter()
            .filter(|e| match e {
                EntityType::Line(line) => (line.start.x - line.end.x).abs() < 1e-7,
                _ => false,
            })
            .count();
        assert!(horizontal >= 4 && vertical >= 4);
    }
}
