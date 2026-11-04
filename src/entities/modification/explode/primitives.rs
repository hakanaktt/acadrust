use super::*;

pub(super) fn explode_lwpolyline(pl: &LwPolyline) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let seg_count = if pl.is_closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);
    let to_wcs = Matrix3::arbitrary_axis(pl.normal);

    for i in 0..seg_count {
        let v1 = &pl.vertices[i];
        let v2 = &pl.vertices[(i + 1) % n];

        if v1.bulge.abs() < 1e-10 {
            result.push(line_entity(
                to_wcs * Vector3::new(v1.location.x, v1.location.y, pl.elevation),
                to_wcs * Vector3::new(v2.location.x, v2.location.y, pl.elevation),
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        } else {
            result.push(arc_from_bulge(
                v1.location.x,
                v1.location.y,
                v2.location.x,
                v2.location.y,
                v1.bulge,
                pl.elevation,
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        }
    }
    result
}

pub(super) fn explode_polyline2d(pl: &Polyline2D) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let closed = pl.flags.is_closed();
    let seg_count = if closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);
    let to_wcs = Matrix3::arbitrary_axis(pl.normal);

    for i in 0..seg_count {
        let v1 = &pl.vertices[i];
        let v2 = &pl.vertices[(i + 1) % n];

        // A 2D polyline sits at its own elevation; a vertex's Z stands in
        // only where the polyline records none.
        let z = |vertex: &Vertex2D| {
            if pl.elevation != 0.0 {
                pl.elevation
            } else {
                vertex.location.z
            }
        };
        if v1.bulge.abs() < 1e-10 {
            result.push(line_entity(
                to_wcs * Vector3::new(v1.location.x, v1.location.y, z(v1)),
                to_wcs * Vector3::new(v2.location.x, v2.location.y, z(v2)),
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        } else {
            result.push(arc_from_bulge(
                v1.location.x,
                v1.location.y,
                v2.location.x,
                v2.location.y,
                v1.bulge,
                z(v1),
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        }
    }
    result
}

pub(super) fn explode_polyline3d(pl: &Polyline) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let closed = pl.flags.is_closed();
    let seg_count = if closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let start = pl.vertices[i].location;
        let end = pl.vertices[(i + 1) % n].location;
        result.push(line_entity(start, end, 0.0, Vector3::UNIT_Z, &pl.common));
    }
    result
}

pub(super) fn explode_polyline3d_new(pl: &Polyline3D) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let closed = pl.flags.closed;
    let seg_count = if closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let start = pl.vertices[i].position;
        let end = pl.vertices[(i + 1) % n].position;
        result.push(line_entity(start, end, 0.0, Vector3::UNIT_Z, &pl.common));
    }
    result
}

pub(super) fn explode_circle(circle: &Circle) -> Vec<EntityType> {
    // A circle explodes into a single full-circle arc.
    vec![EntityType::Arc(Arc {
        common: inherit_common(&circle.common),
        center: circle.center,
        radius: circle.radius,
        start_angle: 0.0,
        end_angle: 2.0 * PI,
        thickness: circle.thickness,
        normal: circle.normal,
    })]
}

pub(super) fn explode_ellipse(ellipse: &Ellipse) -> Vec<EntityType> {
    // Approximate the ellipse as a spline (same approach ACAD uses).
    let a = ellipse.major_axis_length();
    let b = ellipse.minor_axis_length();
    if a < 1e-10 {
        return Vec::new();
    }

    // Direction of major / minor axes in WCS.
    let major_dir = ellipse.major_axis.normalize();
    let minor_dir = ellipse.normal.cross(&major_dir).normalize();

    let is_full = ellipse.is_full();
    // Approximate with N points.
    let num_points: usize = if is_full { 36 } else { 24 };
    let (t_start, t_end) = if is_full {
        (0.0, 2.0 * PI)
    } else {
        (ellipse.start_parameter, ellipse.end_parameter)
    };

    let mut points = Vec::with_capacity(num_points + 1);
    for i in 0..=num_points {
        let t = t_start + (t_end - t_start) * (i as f64 / num_points as f64);
        let pt = ellipse.center + major_dir * (a * t.cos()) + minor_dir * (b * t.sin());
        points.push(pt);
    }

    let spline = Spline::from_control_points(3, points);
    let mut s = spline;
    s.common = inherit_common(&ellipse.common);
    if is_full {
        s.flags.closed = true;
    }
    vec![EntityType::Spline(s)]
}

pub(super) fn explode_solid(solid: &Solid) -> Vec<EntityType> {
    // Explode a filled solid into its edge lines.
    let common = &solid.common;
    let corners = solid.boundary_corners();
    let n = corners.len();
    let mut result = Vec::with_capacity(n);
    let to_wcs = Matrix3::arbitrary_axis(solid.normal);
    for i in 0..n {
        result.push(line_entity(
            to_wcs * corners[i],
            to_wcs * corners[(i + 1) % n],
            solid.thickness,
            solid.normal,
            common,
        ));
    }
    result
}

pub(super) fn explode_face3d(face: &Face3D) -> Vec<EntityType> {
    // Explode a 3D face into its edge lines (only visible edges).
    let common = &face.common;
    // Edge flags belong to the four stored slots, even when a mirrored
    // triangle repeats its first two corners rather than its last two.
    let corners = [
        face.first_corner,
        face.second_corner,
        face.third_corner,
        face.fourth_corner,
    ];
    let n = corners.len();
    let mut result = Vec::with_capacity(n);

    let invisible = [
        face.invisible_edges.is_first_invisible(),
        face.invisible_edges.is_second_invisible(),
        face.invisible_edges.is_third_invisible(),
        face.invisible_edges.is_fourth_invisible(),
    ];

    for i in 0..n {
        if !invisible[i] && (corners[i] - corners[(i + 1) % n]).length() >= 1e-10 {
            result.push(line_entity(
                corners[i],
                corners[(i + 1) % n],
                0.0,
                Vector3::UNIT_Z,
                common,
            ));
        }
    }
    result
}

/// Points on a NURBS curve (de Boor, rational when `weights` is given).
///
/// `segments` is the number of samples per non-empty knot span. Knot vectors
/// that do not match the control points are replaced by a clamped uniform one.
pub(crate) fn nurbs_points(
    degree: usize,
    control_points: &[Vector3],
    weights: &[f64],
    knots: &[f64],
    segments: usize,
) -> Vec<Vector3> {
    let n = control_points.len();
    if n < 2 {
        return control_points.to_vec();
    }
    let p = degree.clamp(1, n - 1);
    let knots: Vec<f64> = if knots.len() == n + p + 1 && knots.windows(2).all(|w| w[1] >= w[0]) {
        knots.to_vec()
    } else {
        // clamped uniform knot vector
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
    let rational = weights.len() == n && weights.iter().any(|w| (*w - 1.0).abs() > 1e-12);
    // homogeneous control points
    let hom: Vec<[f64; 4]> = control_points
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let w = if rational && weights[i] > 0.0 {
                weights[i]
            } else {
                1.0
            };
            [c.x * w, c.y * w, c.z * w, w]
        })
        .collect();
    let eval = |u: f64, k: usize| -> Vector3 {
        let mut d: Vec<[f64; 4]> = (0..=p).map(|j| hom[j + k - p]).collect();
        for r in 1..=p {
            for j in (r..=p).rev() {
                let i = j + k - p;
                let denom = knots[i + p + 1 - r] - knots[i];
                let alpha = if denom.abs() < 1e-15 {
                    0.0
                } else {
                    (u - knots[i]) / denom
                };
                let (prev, cur) = (d[j - 1], d[j]);
                d[j] = std::array::from_fn(|c| (1.0 - alpha) * prev[c] + alpha * cur[c]);
            }
        }
        let w = if d[p][3].abs() < 1e-15 { 1.0 } else { d[p][3] };
        Vector3::new(d[p][0] / w, d[p][1] / w, d[p][2] / w)
    };
    let segments = segments.max(1);
    let mut points = Vec::new();
    for k in p..n {
        let (u0, u1) = (knots[k], knots[k + 1]);
        if u1 - u0 <= 1e-15 {
            continue;
        }
        let first = if points.is_empty() { 0 } else { 1 };
        for s in first..=segments {
            let u = u0 + (u1 - u0) * s as f64 / segments as f64;
            points.push(eval(u, k));
        }
    }
    if points.is_empty() {
        return control_points.to_vec();
    }
    points
}

/// Points along a spline entity: the NURBS curve when control points are
/// present, otherwise the fit points.
pub(crate) fn spline_points(spline: &Spline) -> Vec<Vector3> {
    if spline.control_points.len() >= 2 {
        let degree = usize::try_from(spline.degree).unwrap_or(3).max(1);
        let mut points = nurbs_points(
            degree,
            &spline.control_points,
            &spline.weights,
            &spline.knots,
            16,
        );
        if spline.flags.closed && points.len() > 2 && points[0] != points[points.len() - 1] {
            points.push(points[0]);
        }
        points
    } else {
        let mut points = spline.fit_points.clone();
        if spline.flags.closed && points.len() > 2 {
            points.push(points[0]);
        }
        points
    }
}

pub(super) fn explode_spline(spline: &Spline) -> Vec<EntityType> {
    // Approximate the curve by line segments between points on the NURBS curve.
    let points = spline_points(spline);
    if points.len() < 2 {
        return Vec::new();
    }
    points
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .map(|pair| line_entity(pair[0], pair[1], 0.0, spline.normal, &spline.common))
        .collect()
}
