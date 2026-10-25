//! Trimmed B-rep faceter for ACIS solids, regions, bodies, and surfaces.
//!
//! Each model edge is sampled once, to a chord tolerance, and every face that
//! uses it reuses that polyline. Planar faces are triangulated in the plane.
//! Cones and cylinders are ruled between their boundary rings, so the only
//! error is the shared circumferential chord. Spheres, tori, and NURBS patches
//! are evaluated on a curvature-sized parameter grid. The grid is refined until
//! the bilinear sagitta is inside the tolerance, and neighboring cells share
//! their corner parameters so the patch does not crack.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use crate::entities::acis::{
    SatBSplineSurface, SatConeSurface, SatDocument, SatEllipseCurve, SatFace, SatIntCurve, SatLoop,
    SatPlaneSurface, SatSphereSurface, SatSplineSurface, SatStraightCurve, SatTorusSurface, Sense,
};
use crate::types::{Matrix3, Vector3};

/// World-space triangles, one normal per vertex, boundary edges, and the
/// parameter lines a wireframe style draws on curved faces.
#[derive(Debug, Clone)]
pub(crate) struct Tessellation {
    pub triangles: Vec<Vector3>,
    pub normals: Vec<Vector3>,
    pub edges: Vec<Vec<Vector3>>,
    pub isolines: Vec<Vec<Vector3>>,
    /// Analytic boundary edges. These replace the matching sampled polylines.
    pub edge_curves: Vec<super::scene::Curve>,
    /// Analytic parameter lines. These replace the matching sampled isolines.
    pub isoline_curves: Vec<super::scene::Curve>,
}

impl Tessellation {
    fn new() -> Self {
        Self {
            triangles: Vec::new(),
            normals: Vec::new(),
            edges: Vec::new(),
            isolines: Vec::new(),
            edge_curves: Vec::new(),
            isoline_curves: Vec::new(),
        }
    }
}

struct Placement {
    linear: Matrix3,
    normal_map: Matrix3,
    translation: Vector3,
}

impl Placement {
    fn from_document(document: &SatDocument) -> Self {
        let (matrix, translation, scale) = document.placement();
        let linear = Matrix3 {
            m: [
                [
                    scale * matrix[0][0],
                    scale * matrix[1][0],
                    scale * matrix[2][0],
                ],
                [
                    scale * matrix[0][1],
                    scale * matrix[1][1],
                    scale * matrix[2][1],
                ],
                [
                    scale * matrix[0][2],
                    scale * matrix[1][2],
                    scale * matrix[2][2],
                ],
            ],
        };
        let normal_map = linear
            .inverse()
            .map(|matrix| matrix.transpose())
            .unwrap_or(Matrix3::identity());
        Self {
            linear,
            normal_map,
            translation: Vector3::new(translation[0], translation[1], translation[2]),
        }
    }

    fn point(&self, point: Vector3) -> Vector3 {
        self.linear * point + self.translation
    }

    fn normal(&self, normal: Vector3) -> Vector3 {
        unit(self.normal_map * normal).unwrap_or(Vector3::UNIT_Z)
    }
}

/// Facet `document`. `tolerance` is a world-space chord error. `None` derives
/// one from the model diagonal, two thousandths of that diagonal.
pub(crate) fn tessellate(document: &SatDocument) -> Tessellation {
    let place = Placement::from_document(document);
    let tolerance = model_tolerance(document, &place);
    let mut edges = HashMap::<usize, Vec<Vector3>>::new();
    let mut tess = Tessellation::new();
    for record in &document.records {
        let Some(edge) = crate::entities::acis::SatEdge::from_record(record) else {
            continue;
        };
        let sampled: Vec<Vector3> = sample_edge(document, &edge, tolerance)
            .into_iter()
            .map(|point| place.point(point))
            .collect();
        if let Some(curve) = analytic_edge(document, &edge) {
            let curve = super::curves::map_curve(curve, &|point| place.point(point));
            tess.edge_curves.push(curve);
        } else if sampled.len() >= 2 {
            tess.edges.push(sampled.clone());
        }
        edges.insert(record.index as usize, sampled);
    }
    for record in &document.records {
        let Some(face) = SatFace::from_record(record) else {
            continue;
        };
        facet_face(document, &face, &edges, &place, tolerance, &mut tess);
    }
    orient_triangles(&mut tess);
    tess
}

/// Keep raster winding consistent with the oriented surface normals. ACIS
/// loops may be listed in either direction, and a mixed winding makes a
/// double-sided conceptual pass shade alternating triangles as if they were
/// back faces.
fn orient_triangles(tess: &mut Tessellation) {
    for (triangle, normals) in tess
        .triangles
        .chunks_exact_mut(3)
        .zip(tess.normals.chunks_exact_mut(3))
    {
        let geometric = (triangle[1] - triangle[0]).cross(&(triangle[2] - triangle[0]));
        let normal = normals[0] + normals[1] + normals[2];
        if geometric.dot(&normal) < 0.0 {
            triangle.swap(1, 2);
            normals.swap(1, 2);
        }
    }
}

fn facet_face(
    document: &SatDocument,
    face: &SatFace<'_>,
    edges: &HashMap<usize, Vec<Vector3>>,
    place: &Placement,
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let loops = face_loops(document, face)
        .into_iter()
        .map(|pointer| loop_polyline(document, pointer, edges))
        .filter(|loop_| !loop_.is_empty())
        .collect::<Vec<_>>();
    let surface = document.resolve(face.surface());
    let sense = face.sense();
    let Some(surface) = surface else {
        return;
    };
    if let Some(plane) = SatPlaneSurface::from_record(surface) {
        let normal = place.normal(oriented(vec3(plane.normal()), sense));
        let (triangles, normals) = triangulate_plane(&loops, normal);
        tess.triangles.extend(triangles);
        tess.normals.extend(normals);
        return;
    }
    if let Some(cone) = SatConeSurface::from_record(surface) {
        facet_cone(&cone, &loops, sense, place, tolerance, tess);
        return;
    }
    if let Some(sphere) = SatSphereSurface::from_record(surface) {
        if loops.is_empty() {
            facet_sphere(&sphere, sense, place, tolerance, tess);
        } else {
            facet_trimmed_sphere(&sphere, &loops, sense, place, tess);
        }
        return;
    }
    if let Some(torus) = SatTorusSurface::from_record(surface) {
        if loops.is_empty() {
            facet_torus(&torus, sense, place, tolerance, tess);
        }
        return;
    }
    if let Some(spline) = SatSplineSurface::from_record(surface) {
        if let Some(patch) = spline
            .bspline(document)
            .or_else(|| spline.offset_in(document).map(|(patch, _)| patch))
        {
            facet_spline(&patch, sense, place, tolerance, tess);
        }
    }
}

fn face_loops(
    document: &SatDocument,
    face: &SatFace<'_>,
) -> Vec<crate::entities::acis::SatPointer> {
    let mut loops = Vec::new();
    let mut cursor = face.first_loop();
    let mut guard = 0;
    while !cursor.is_null() && guard < 64 {
        loops.push(cursor);
        let Some(record) = document.resolve(cursor) else {
            break;
        };
        let Some(loop_) = SatLoop::from_record(record) else {
            break;
        };
        cursor = loop_.next_loop();
        guard += 1;
    }
    loops
}

fn loop_polyline(
    document: &SatDocument,
    pointer: crate::entities::acis::SatPointer,
    edges: &HashMap<usize, Vec<Vector3>>,
) -> Vec<Vector3> {
    let Some(record) = document.resolve(pointer) else {
        return Vec::new();
    };
    let Some(loop_) = SatLoop::from_record(record) else {
        return Vec::new();
    };
    let start = loop_.first_coedge();
    let mut cursor = start;
    let mut points = Vec::new();
    let mut guard = 0;
    while !cursor.is_null() && guard < 256 {
        let Some(record) = document.resolve(cursor) else {
            break;
        };
        let Some(coedge) = crate::entities::acis::SatCoedge::from_record(record) else {
            break;
        };
        if let Some(index) = coedge.edge().index() {
            if let Some(edge) = edges.get(&index) {
                let mut edge = edge.clone();
                if coedge.sense() == Sense::Reversed {
                    edge.reverse();
                }
                if points
                    .last()
                    .is_some_and(|last: &Vector3| (*last - edge[0]).length_squared() <= 1e-16)
                {
                    edge.remove(0);
                }
                points.extend(edge);
            }
        }
        cursor = coedge.next();
        if cursor == start {
            break;
        }
        guard += 1;
    }
    if points.len() >= 2 && (points[0] - *points.last().unwrap()).length_squared() <= 1e-16 {
        points.pop();
    }
    points
}

fn triangulate_plane(loops: &[Vec<Vector3>], normal: Vector3) -> (Vec<Vector3>, Vec<Vector3>) {
    let Some(outer_index) = loops
        .iter()
        .enumerate()
        .filter(|(_, loop_)| loop_.len() >= 3)
        .max_by(|(_, left), (_, right)| {
            polygon_area(left)
                .abs()
                .partial_cmp(&polygon_area(right).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(index, _)| index)
    else {
        return (Vec::new(), Vec::new());
    };
    let normal = if polygon_area(&loops[outer_index]).is_sign_negative() {
        normal * -1.0
    } else {
        normal
    };
    let axis_u = plane_axis(normal);
    let axis_v = unit(normal.cross(&axis_u)).unwrap_or(Vector3::UNIT_Y);
    let mut flat = Vec::new();
    let mut holes = Vec::new();
    let mut source = Vec::new();
    let project = |point: &Vector3| [point.dot(&axis_u), point.dot(&axis_v)];
    for (index, loop_) in loops.iter().enumerate() {
        if loop_.len() < 3 {
            continue;
        }
        if index != outer_index {
            holes.push(flat.len() / 2);
        }
        for point in loop_ {
            let uv = project(point);
            flat.push(uv[0]);
            flat.push(uv[1]);
            source.push(*point);
        }
    }
    let indices = earcutr::earcut(&flat, &holes, 2).unwrap_or_default();
    let triangles = indices
        .into_iter()
        .filter_map(|index| source.get(index).copied())
        .collect::<Vec<_>>();
    let normals = vec![normal; triangles.len()];
    (triangles, normals)
}

fn triangulate_cone_loop(
    points: &[Vector3],
    frame: &ConeFrame,
    flip: bool,
    tolerance: f64,
) -> Option<(Vec<Vector3>, Vec<Vector3>)> {
    if points.len() < 3 {
        return None;
    }

    let mut parameters = Vec::with_capacity(points.len());
    let mut previous = frame.angle(points[0]);
    parameters.push((previous, frame.height(points[0])));
    for point in points.iter().skip(1) {
        let mut angle = frame.angle(*point);
        while angle - previous > PI {
            angle -= TAU;
        }
        while angle - previous < -PI {
            angle += TAU;
        }
        parameters.push((angle, frame.height(*point)));
        previous = angle;
    }

    let min_height = parameters
        .iter()
        .map(|(_, height)| *height)
        .fold(f64::INFINITY, f64::min);
    let max_height = parameters
        .iter()
        .map(|(_, height)| *height)
        .fold(f64::NEG_INFINITY, f64::max);
    if max_height - min_height <= 1e-8 {
        return None;
    }

    let mut flat = Vec::with_capacity(parameters.len() * 2);
    for (angle, height) in &parameters {
        flat.extend([*angle, *height]);
    }
    let indices = earcutr::earcut(&flat, &[], 2).ok()?;
    if indices.is_empty() {
        return None;
    }

    let mut uv_triangles = indices
        .chunks_exact(3)
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        .collect::<Vec<_>>();
    let boundary_edges = (0..parameters.len())
        .map(|index| edge_key(index, (index + 1) % parameters.len()))
        .collect::<std::collections::HashSet<_>>();
    let tolerance = tolerance.abs().max(1e-6);
    let max_vertices = 16_384;

    // Split only interior edges, sharing every inserted midpoint between the
    // incident triangles so the refined mesh remains conforming.
    for _ in 0..12 {
        let mut split_edges = std::collections::HashSet::new();
        for &[a, b, c] in &uv_triangles {
            let corners = [a, b, c];
            let world = corners.map(|index| {
                let (angle, height) = parameters[index];
                frame.point_at(angle, height)
            });
            let mut longest = None;
            for side in 0..3 {
                let first = corners[side];
                let second = corners[(side + 1) % 3];
                let key = edge_key(first, second);
                let (u0, h0) = parameters[first];
                let (u1, h1) = parameters[second];
                let curved_midpoint = frame.point_at((u0 + u1) * 0.5, (h0 + h1) * 0.5);
                let chord_midpoint = (world[side] + world[(side + 1) % 3]) * 0.5;
                let error = (curved_midpoint - chord_midpoint).length();
                if !boundary_edges.contains(&key) && error > tolerance {
                    split_edges.insert(key);
                }
                let length_squared = (u1 - u0).powi(2) + (h1 - h0).powi(2);
                if !boundary_edges.contains(&key)
                    && longest.map_or(true, |(_, best_length)| length_squared > best_length)
                {
                    longest = Some((key, length_squared));
                }
            }

            let center_uv = corners
                .iter()
                .map(|index| parameters[*index])
                .fold((0.0, 0.0), |sum, point| (sum.0 + point.0 / 3.0, sum.1 + point.1 / 3.0));
            let curved_center = frame.point_at(center_uv.0, center_uv.1);
            let chord_center = (world[0] + world[1] + world[2]) / 3.0;
            let center_error = (curved_center - chord_center).length();
            if center_error > tolerance {
                if let Some((key, _)) = longest {
                    split_edges.insert(key);
                }
            }
        }

        if split_edges.is_empty() || parameters.len() + split_edges.len() > max_vertices {
            break;
        }

        let mut midpoint_indices = std::collections::HashMap::new();
        for &(first, second) in &split_edges {
            let (u0, h0) = parameters[first];
            let (u1, h1) = parameters[second];
            let index = parameters.len();
            parameters.push(((u0 + u1) * 0.5, (h0 + h1) * 0.5));
            midpoint_indices.insert((first, second), index);
        }

        let midpoint = |first: usize, second: usize| {
            midpoint_indices.get(&edge_key(first, second)).copied()
        };
        let mut refined = Vec::with_capacity(uv_triangles.len() * 2);
        for [a, b, c] in uv_triangles {
            let mab = midpoint(a, b);
            let mbc = midpoint(b, c);
            let mca = midpoint(c, a);
            match (mab, mbc, mca) {
                (None, None, None) => refined.push([a, b, c]),
                (Some(m), None, None) => refined.extend([[a, m, c], [m, b, c]]),
                (None, Some(m), None) => refined.extend([[b, m, a], [m, c, a]]),
                (None, None, Some(m)) => refined.extend([[c, m, b], [m, a, b]]),
                (Some(ab), Some(bc), None) => {
                    refined.extend([[b, bc, ab], [a, ab, c], [ab, bc, c]]);
                }
                (None, Some(bc), Some(ca)) => {
                    refined.extend([[c, ca, bc], [ca, a, b], [ca, b, bc]]);
                }
                (Some(ab), None, Some(ca)) => {
                    refined.extend([[a, ab, ca], [ab, b, c], [ab, c, ca]]);
                }
                (Some(ab), Some(bc), Some(ca)) => refined.extend([
                    [a, ab, ca],
                    [ab, b, bc],
                    [ca, bc, c],
                    [ab, bc, ca],
                ]),
            }
        }
        uv_triangles = refined;
    }

    let mut triangles = Vec::with_capacity(uv_triangles.len() * 3);
    let mut normals = Vec::with_capacity(uv_triangles.len() * 3);
    for triangle in uv_triangles {
        for index in triangle {
            let (angle, height) = *parameters.get(index)?;
            let point = frame.point_at(angle, height);
            triangles.push(point);
            normals.push(frame.normal_at(point, flip));
        }
    }
    Some((triangles, normals))
}

fn edge_key(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn polygon_area(points: &[Vector3]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let normal = newell(points);
    let axis_u = plane_axis(normal);
    let axis_v = normal.cross(&axis_u);
    let mut area = 0.0;
    for index in 0..points.len() {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        let su = start.dot(&axis_u);
        let sv = start.dot(&axis_v);
        let eu = end.dot(&axis_u);
        let ev = end.dot(&axis_v);
        area += su * ev - eu * sv;
    }
    area * 0.5
}

fn facet_cone(
    cone: &SatConeSurface<'_>,
    loops: &[Vec<Vector3>],
    sense: Sense,
    place: &Placement,
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let frame = ConeFrame::from_surface(cone, place);
    let mut rings = Vec::new();
    let mut apices = Vec::new();
    for loop_ in loops {
        if loop_.len() >= 3 && !collapsed(loop_) {
            rings.push(loop_.clone());
        } else if let Some(point) = loop_.first() {
            apices.push(*point);
        }
    }
    let flip = sense == Sense::Reversed;
    if rings.len() == 1 && apices.len() == 1 {
        let ring = close_ring(rings[0].clone());
        push_fan(&ring, apices[0], &frame, flip, tess);
    } else if rings.len() >= 2 {
        push_ruled(&rings[0], &rings[1], &frame, flip, tess);
    } else if rings.len() == 1 {
        let height_min = rings[0]
            .iter()
            .map(|point| frame.height(*point))
            .fold(f64::INFINITY, f64::min);
        let height_max = rings[0]
            .iter()
            .map(|point| frame.height(*point))
            .fold(f64::NEG_INFINITY, f64::max);
        if height_max - height_min <= 1e-8 {
            let normal = place.normal(oriented(frame.axis, sense));
            let (triangles, normals) = triangulate_plane(&rings, normal);
            tess.triangles.extend(triangles);
            tess.normals.extend(normals);
        } else {
            if let Some((triangles, normals)) =
                triangulate_cone_loop(&rings[0], &frame, flip, tolerance)
            {
                tess.triangles.extend(triangles);
                tess.normals.extend(normals);
            }
        }
    }
    push_cone_isolines(&frame, &rings, tolerance, tess);
}

fn push_fan(
    ring: &[Vector3],
    apex: Vector3,
    frame: &ConeFrame,
    flip: bool,
    tess: &mut Tessellation,
) {
    for pair in ring.windows(2) {
        if (pair[1] - pair[0]).length_squared() <= 1e-16 {
            continue;
        }
        tess.triangles.extend([apex, pair[0], pair[1]]);
        tess.normals.extend([
            frame.normal_at(apex, flip),
            frame.normal_at(pair[0], flip),
            frame.normal_at(pair[1], flip),
        ]);
    }
}

fn push_ruled(
    first: &[Vector3],
    second: &[Vector3],
    frame: &ConeFrame,
    flip: bool,
    tess: &mut Tessellation,
) {
    let left = angled_ring(first, frame);
    let right = angled_ring(second, frame);
    if left.len() < 2 || right.len() < 2 {
        return;
    }
    let (left, right) = if closed_angular_ring(&left) && closed_angular_ring(&right) {
        let start = left[0].0;
        (
            rotate_angular_ring(&left, start),
            rotate_angular_ring(&right, start),
        )
    } else {
        (left, right)
    };
    let mut i = 0;
    let mut j = 0;
    while i + 1 < left.len() || j + 1 < right.len() {
        let advance_left = if i + 1 >= left.len() {
            false
        } else if j + 1 >= right.len() {
            true
        } else {
            left[i + 1].0 <= right[j + 1].0
        };
        let triangle = if advance_left {
            let triangle = [left[i].1, left[i + 1].1, right[j].1];
            i += 1;
            triangle
        } else {
            let triangle = [left[i].1, right[j + 1].1, right[j].1];
            j += 1;
            triangle
        };
        let [a, b, c] = triangle;
        if triangle_area(&a, &b, &c) <= 1e-14 {
            continue;
        }
        tess.triangles.extend([a, b, c]);
        tess.normals.extend([
            frame.normal_at(a, flip),
            frame.normal_at(b, flip),
            frame.normal_at(c, flip),
        ]);
    }
}

fn push_cone_isolines(
    frame: &ConeFrame,
    rings: &[Vec<Vector3>],
    _tolerance: f64,
    tess: &mut Tessellation,
) {
    let Some(first) = rings.first() else {
        return;
    };
    let mut heights: Vec<f64> = rings
        .iter()
        .filter_map(|ring| ring.first().map(|point| frame.height(*point)))
        .collect();
    heights.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    heights.dedup_by(|left, right| (*left - *right).abs() <= 1e-8);
    if heights.len() >= 2 {
        let low = heights[0];
        let high = *heights.last().unwrap();
        for step in 1..4 {
            let height = low + (high - low) * (step as f64 / 4.0);
            tess.isoline_curves.push(frame.circle_curve(height));
        }
        for step in 0..4 {
            let angle = TAU * (step as f64 / 4.0);
            let start = frame.point_at(angle, low);
            let end = frame.point_at(angle, high);
            tess.isolines.push(vec![start, end]);
        }
        let _ = first;
    }
}

struct ConeFrame {
    center: Vector3,
    axis: Vector3,
    major: Vector3,
    minor: Vector3,
    sin_half: f64,
    cos_half: f64,
    radius: f64,
}

impl ConeFrame {
    fn from_surface(cone: &SatConeSurface<'_>, place: &Placement) -> Self {
        let axis = unit(place.normal(vec3(cone.axis()))).unwrap_or(Vector3::UNIT_Z);
        let major = place.point(vec3(cone.major_axis())) - place.point(Vector3::ZERO);
        let major = if major.length_squared() <= 1e-18 {
            plane_axis(axis) * cone.radius().abs().max(1e-6)
        } else {
            major
        };
        let ratio = if cone.ratio().abs() <= 1e-12 {
            1.0
        } else {
            cone.ratio()
        };
        let minor = axis.cross(&major) * ratio;
        Self {
            center: place.point(vec3(cone.center())),
            axis,
            major,
            minor,
            sin_half: cone.sin_half_angle(),
            cos_half: cone.cos_half_angle(),
            radius: major.length().max(1e-9),
        }
    }

    fn height(&self, point: Vector3) -> f64 {
        (point - self.center).dot(&self.axis)
    }

    fn angle(&self, point: Vector3) -> f64 {
        let radial = point - self.center - self.axis * self.height(point);
        let major_norm = self.major.length_squared();
        let minor_norm = self.minor.length_squared();
        let cross = self.major.dot(&self.minor);
        let determinant = major_norm * minor_norm - cross * cross;
        if determinant <= 1e-18 {
            return radial.dot(&self.minor).atan2(radial.dot(&self.major));
        }
        let major_component =
            (radial.dot(&self.major) * minor_norm - radial.dot(&self.minor) * cross)
                / determinant;
        let minor_component =
            (radial.dot(&self.minor) * major_norm - radial.dot(&self.major) * cross)
                / determinant;
        minor_component.atan2(major_component)
    }

    fn scale_at(&self, height: f64) -> f64 {
        let cosine = if self.cos_half.abs() <= 1e-12 {
            1.0
        } else {
            self.cos_half
        };
        1.0 + height * self.sin_half / (cosine * self.radius)
    }

    fn point_at(&self, angle: f64, height: f64) -> Vector3 {
        let scale = self.scale_at(height);
        self.center
            + self.axis * height
            + (self.major * angle.cos() + self.minor * angle.sin()) * scale
    }

    fn normal_at(&self, point: Vector3, flip: bool) -> Vector3 {
        let height = self.height(point);
        let angle = self.angle(point);
        let scale = self.scale_at(height);
        let radial = self.major * angle.cos() + self.minor * angle.sin();
        let tangent_angle = (self.major * -angle.sin() + self.minor * angle.cos()) * scale;
        let cosine = if self.cos_half.abs() <= 1e-12 {
            1.0
        } else {
            self.cos_half
        };
        let tangent_height = self.axis + radial * (self.sin_half / (cosine * self.radius));
        let mut normal = unit(tangent_angle.cross(&tangent_height))
            .unwrap_or(unit(radial).unwrap_or(Vector3::UNIT_X));
        if flip {
            normal = normal * -1.0;
        }
        normal
    }

    fn circle_curve(&self, height: f64) -> super::scene::Curve {
        let scale = self.scale_at(height);
        super::scene::Curve::Elliptic {
            center: self.center + self.axis * height,
            u: self.major * scale,
            v: self.minor * scale,
            start: 0.0,
            sweep: TAU,
        }
    }
}

fn angled_ring(points: &[Vector3], frame: &ConeFrame) -> Vec<(f64, Vector3)> {
    if points.is_empty() {
        return Vec::new();
    }
    let mut samples = Vec::with_capacity(points.len() + 1);
    let mut previous = frame.angle(points[0]);
    samples.push((previous, points[0]));
    for point in points.iter().skip(1) {
        let mut angle = frame.angle(*point);
        while angle < previous - PI {
            angle += TAU;
        }
        while angle > previous + PI {
            angle -= TAU;
        }
        previous = angle;
        samples.push((angle, *point));
    }
    if samples.len() >= 3 {
        let span = samples.last().unwrap().0 - samples[0].0;
        if span < 0.0 {
            samples.reverse();
        }
        let span = samples.last().unwrap().0 - samples[0].0;
        if span > TAU * 0.75 && (samples[0].1 - samples.last().unwrap().1).length_squared() > 1e-16
        {
            let first = samples[0];
            samples.push((first.0 + TAU, first.1));
        }
    }
    samples
}

fn closed_angular_ring(ring: &[(f64, Vector3)]) -> bool {
    ring.len() >= 4
        && ring.last().unwrap().0 - ring[0].0 > TAU * 0.75
        && (ring[0].1 - ring.last().unwrap().1).length_squared() <= 1e-16
}

fn rotate_angular_ring(ring: &[(f64, Vector3)], start: f64) -> Vec<(f64, Vector3)> {
    let end = start + TAU;
    let point_at = |angle: f64| {
        let first = ring[0].0;
        let angle = first + (angle - first).rem_euclid(TAU);
        for pair in ring.windows(2) {
            if angle <= pair[1].0 {
                let span = pair[1].0 - pair[0].0;
                let t = if span.abs() <= 1e-12 {
                    0.0
                } else {
                    ((angle - pair[0].0) / span).clamp(0.0, 1.0)
                };
                return pair[0].1 + (pair[1].1 - pair[0].1) * t;
            }
        }
        ring.last().unwrap().1
    };
    let start_point = point_at(start);
    let mut rotated = vec![(start, start_point)];
    for (angle, point) in ring {
        let turns = ((start - angle) / TAU).ceil();
        let angle = *angle + turns * TAU;
        if angle > start + 1e-8 && angle < end - 1e-8 {
            rotated.push((angle, *point));
        }
    }
    rotated.push((end, start_point));
    rotated
}

fn close_ring(mut points: Vec<Vector3>) -> Vec<Vector3> {
    if points.len() >= 2 && (points[0] - *points.last().unwrap()).length_squared() > 1e-16 {
        points.push(points[0]);
    }
    points
}

fn collapsed(points: &[Vector3]) -> bool {
    let Some(origin) = points.first() else {
        return true;
    };
    points
        .iter()
        .all(|point| (*point - *origin).length_squared() <= 1e-12)
}

fn facet_sphere(
    sphere: &SatSphereSurface<'_>,
    sense: Sense,
    place: &Placement,
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let center = place.point(vec3(sphere.center()));
    let radius = (place.point(vec3(sphere.center()) + vec3(sphere.u_direction())) - center)
        .length()
        .max(sphere.radius().abs() * place.linear.m[0][0].abs().max(1e-9));
    let radius = if sphere.radius().abs() > 1e-12 {
        (place.point(
            vec3(sphere.center())
                + unit(vec3(sphere.u_direction())).unwrap_or(Vector3::UNIT_X)
                    * sphere.radius().abs(),
        ) - center)
            .length()
    } else {
        radius
    };
    let pole = unit(place.normal(vec3(sphere.pole()))).unwrap_or(Vector3::UNIT_Z);
    let east = unit(place.normal(vec3(sphere.u_direction()))).unwrap_or(plane_axis(pole));
    let north = unit(pole.cross(&east)).unwrap_or(Vector3::UNIT_Y);
    let flip = sense == Sense::Reversed;
    let step = chord_step(radius, tolerance);
    let columns = (TAU / step).ceil().max(8.0) as usize;
    let rows = (PI / step).ceil().max(4.0) as usize;
    let point_at = |row: usize, column: usize| -> Vector3 {
        let v = PI * (row as f64 / rows as f64);
        let u = TAU * (column as f64 / columns as f64);
        center
            + east * (v.sin() * u.cos() * radius)
            + north * (v.sin() * u.sin() * radius)
            + pole * (v.cos() * radius)
    };
    let normal_at = |point: Vector3| {
        let mut normal = unit(point - center).unwrap_or(pole);
        if flip {
            normal = normal * -1.0;
        }
        normal
    };
    for column in 0..columns {
        let south = center + pole * radius;
        let a = point_at(1, column);
        let b = point_at(1, column + 1);
        push_triangle(
            tess,
            south,
            b,
            a,
            normal_at(south),
            normal_at(b),
            normal_at(a),
        );
        let north_pole = center - pole * radius;
        let c = point_at(rows - 1, column);
        let d = point_at(rows - 1, column + 1);
        push_triangle(
            tess,
            north_pole,
            c,
            d,
            normal_at(north_pole),
            normal_at(c),
            normal_at(d),
        );
    }
    for row in 1..rows.saturating_sub(1) {
        for column in 0..columns {
            let a = point_at(row, column);
            let b = point_at(row, column + 1);
            let c = point_at(row + 1, column + 1);
            let d = point_at(row + 1, column);
            push_triangle(tess, a, b, c, normal_at(a), normal_at(b), normal_at(c));
            push_triangle(tess, a, c, d, normal_at(a), normal_at(c), normal_at(d));
        }
    }
    for step in 0..4 {
        let u = TAU * (step as f64 / 4.0);
        tess.isoline_curves.push(super::scene::Curve::Elliptic {
            center,
            u: pole * radius,
            v: (east * u.cos() + north * u.sin()) * radius,
            start: 0.0,
            sweep: PI,
        });
    }
    for step in 1..4 {
        let v = PI * (step as f64 / 4.0);
        let radial = v.sin() * radius;
        tess.isoline_curves.push(super::scene::Curve::Elliptic {
            center: center + pole * (v.cos() * radius),
            u: east * radial,
            v: north * radial,
            start: 0.0,
            sweep: TAU,
        });
    }
}

fn facet_trimmed_sphere(
    sphere: &SatSphereSurface<'_>,
    loops: &[Vec<Vector3>],
    sense: Sense,
    place: &Placement,
    tess: &mut Tessellation,
) {
    let center = place.point(vec3(sphere.center()));
    let radius = sphere.radius().abs().max(1e-9);
    let flip = sense == Sense::Reversed;
    for loop_ in loops {
        if loop_.len() < 3 {
            continue;
        }
        let mean = loop_
            .iter()
            .copied()
            .fold(Vector3::ZERO, |sum, point| sum + point)
            / loop_.len() as f64;
        let mut direction = mean - center;
        if direction.length_squared() <= 1e-18 {
            direction = unit(place.normal(vec3(sphere.pole()))).unwrap_or(Vector3::UNIT_Z);
        }
        let apex = center + unit(direction).unwrap_or(Vector3::UNIT_Z) * radius;
        let ring = close_ring(
            loop_
                .iter()
                .map(|point| {
                    let direction = *point - center;
                    center + unit(direction).unwrap_or(Vector3::UNIT_X) * radius
                })
                .collect(),
        );
        for pair in ring.windows(2) {
            let normal = |point: Vector3| {
                let mut normal = unit(point - center).unwrap_or(Vector3::UNIT_Z);
                if flip {
                    normal = normal * -1.0;
                }
                normal
            };
            push_triangle(
                tess,
                apex,
                pair[0],
                pair[1],
                normal(apex),
                normal(pair[0]),
                normal(pair[1]),
            );
        }
    }
}

fn facet_torus(
    torus: &SatTorusSurface<'_>,
    sense: Sense,
    place: &Placement,
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let center = place.point(vec3(torus.center()));
    let normal = unit(place.normal(vec3(torus.normal()))).unwrap_or(Vector3::UNIT_Z);
    let east = unit(place.normal(vec3(torus.u_direction()))).unwrap_or(plane_axis(normal));
    let north = unit(normal.cross(&east)).unwrap_or(Vector3::UNIT_Y);
    let major = torus.major_radius().abs().max(1e-9);
    let minor = torus.minor_radius().abs().max(1e-9);
    let major = (place.point(vec3(torus.center()) + east * major) - center).length();
    let minor = (place.point(vec3(torus.center()) + normal * minor) - center).length();
    let flip = sense == Sense::Reversed;
    let columns = (TAU / chord_step(major + minor, tolerance)).ceil().max(8.0) as usize;
    let rows = (TAU / chord_step(minor, tolerance)).ceil().max(8.0) as usize;
    let point_at = |row: usize, column: usize| -> Vector3 {
        let u = TAU * (column as f64 / columns as f64);
        let v = TAU * (row as f64 / rows as f64);
        let tube = east * u.cos() + north * u.sin();
        center + tube * (major + minor * v.cos()) + normal * (minor * v.sin())
    };
    let normal_at = |row: usize, column: usize| {
        let u = TAU * (column as f64 / columns as f64);
        let v = TAU * (row as f64 / rows as f64);
        let tube = east * u.cos() + north * u.sin();
        let mut normal = unit(tube * v.cos() + normal * v.sin()).unwrap_or(tube);
        if flip {
            normal = normal * -1.0;
        }
        normal
    };
    for row in 0..rows {
        for column in 0..columns {
            let a = point_at(row, column);
            let b = point_at(row, column + 1);
            let c = point_at(row + 1, column + 1);
            let d = point_at(row + 1, column);
            push_triangle(
                tess,
                a,
                b,
                c,
                normal_at(row, column),
                normal_at(row, column + 1),
                normal_at(row + 1, column + 1),
            );
            push_triangle(
                tess,
                a,
                c,
                d,
                normal_at(row, column),
                normal_at(row + 1, column + 1),
                normal_at(row + 1, column),
            );
        }
    }
    for step in 0..4 {
        let column = (columns * step) / 4;
        tess.isolines
            .push((0..=rows).map(|row| point_at(row, column)).collect());
        let row = (rows * step) / 4;
        tess.isolines
            .push((0..=columns).map(|column| point_at(row, column)).collect());
    }
}

fn facet_spline(
    patch: &SatBSplineSurface,
    sense: Sense,
    place: &Placement,
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let u0 = knot_start(&patch.u_knots, patch.degree_u);
    let u1 = knot_end(&patch.u_knots, patch.degree_u);
    let v0 = knot_start(&patch.v_knots, patch.degree_v);
    let v1 = knot_end(&patch.v_knots, patch.degree_v);
    if u1 <= u0 || v1 <= v0 {
        return;
    }
    let mut divisions = 4usize;
    while divisions < 64 && !spline_grid_flat(patch, u0, u1, v0, v1, divisions, tolerance) {
        divisions *= 2;
    }
    let flip = sense == Sense::Reversed;
    let at = |u: f64, v: f64| place.point(eval_spline(patch, u, v));
    let normal_at = |u: f64, v: f64| {
        let delta_u = (u1 - u0) / divisions as f64;
        let delta_v = (v1 - v0) / divisions as f64;
        let point = eval_spline(patch, u, v);
        let du = eval_spline(patch, (u + delta_u).min(u1), v) - point;
        let dv = eval_spline(patch, u, (v + delta_v).min(v1)) - point;
        let mut normal = unit(place.normal(du.cross(&dv))).unwrap_or(Vector3::UNIT_Z);
        if flip {
            normal = normal * -1.0;
        }
        normal
    };
    for row in 0..divisions {
        for column in 0..divisions {
            let u_a = u0 + (u1 - u0) * (column as f64 / divisions as f64);
            let u_b = u0 + (u1 - u0) * ((column + 1) as f64 / divisions as f64);
            let v_a = v0 + (v1 - v0) * (row as f64 / divisions as f64);
            let v_b = v0 + (v1 - v0) * ((row + 1) as f64 / divisions as f64);
            let a = at(u_a, v_a);
            let b = at(u_b, v_a);
            let c = at(u_b, v_b);
            let d = at(u_a, v_b);
            push_triangle(
                tess,
                a,
                b,
                c,
                normal_at(u_a, v_a),
                normal_at(u_b, v_a),
                normal_at(u_b, v_b),
            );
            push_triangle(
                tess,
                a,
                c,
                d,
                normal_at(u_a, v_a),
                normal_at(u_b, v_b),
                normal_at(u_a, v_b),
            );
        }
    }
    for step in 0..=4 {
        let u = u0 + (u1 - u0) * (step as f64 / 4.0);
        tess.isolines.push(
            (0..=divisions)
                .map(|row| {
                    let v = v0 + (v1 - v0) * (row as f64 / divisions as f64);
                    at(u, v)
                })
                .collect(),
        );
        let v = v0 + (v1 - v0) * (step as f64 / 4.0);
        tess.isolines.push(
            (0..=divisions)
                .map(|column| {
                    let u = u0 + (u1 - u0) * (column as f64 / divisions as f64);
                    at(u, v)
                })
                .collect(),
        );
    }
}

fn spline_grid_flat(
    patch: &SatBSplineSurface,
    u0: f64,
    u1: f64,
    v0: f64,
    v1: f64,
    divisions: usize,
    tolerance: f64,
) -> bool {
    for row in 0..divisions {
        for column in 0..divisions {
            let u_a = u0 + (u1 - u0) * (column as f64 / divisions as f64);
            let u_b = u0 + (u1 - u0) * ((column + 1) as f64 / divisions as f64);
            let v_a = v0 + (v1 - v0) * (row as f64 / divisions as f64);
            let v_b = v0 + (v1 - v0) * ((row + 1) as f64 / divisions as f64);
            let corners = [
                eval_spline(patch, u_a, v_a),
                eval_spline(patch, u_b, v_a),
                eval_spline(patch, u_b, v_b),
                eval_spline(patch, u_a, v_b),
            ];
            let middle = eval_spline(patch, (u_a + u_b) * 0.5, (v_a + v_b) * 0.5);
            let bilinear = (corners[0] + corners[1] + corners[2] + corners[3]) * 0.25;
            if (middle - bilinear).length() > tolerance {
                return false;
            }
        }
    }
    true
}

fn analytic_edge(
    document: &SatDocument,
    edge: &crate::entities::acis::SatEdge<'_>,
) -> Option<super::scene::Curve> {
    let curve = document.resolve(edge.curve())?;
    let t0 = edge.start_param();
    let t1 = edge.end_param();
    if !t0.is_finite() || !t1.is_finite() {
        return None;
    }
    if let Some(ellipse) = SatEllipseCurve::from_record(curve) {
        let center = vec3(ellipse.center());
        let major = vec3(ellipse.major_axis());
        if major.length_squared() <= 1e-18 {
            return None;
        }
        let normal = unit(vec3(ellipse.normal())).unwrap_or(Vector3::UNIT_Z);
        let minor = normal.cross(&major) * ellipse.ratio();
        let mut sweep = t1 - t0;
        if sweep.abs() < 1e-8 {
            let start = vertex_point(document, edge.start_vertex());
            let end = vertex_point(document, edge.end_vertex());
            let closed = match (start, end) {
                (Some(start), Some(end)) => (start - end).length_squared() <= 1e-12,
                _ => true,
            };
            if !closed {
                return None;
            }
            sweep = TAU.copysign(if sweep < 0.0 { -1.0 } else { 1.0 });
        }
        return Some(super::scene::Curve::Elliptic {
            center,
            u: major,
            v: minor,
            start: t0,
            sweep,
        });
    }
    None
}

fn sample_edge(
    document: &SatDocument,
    edge: &crate::entities::acis::SatEdge<'_>,
    tolerance: f64,
) -> Vec<Vector3> {
    let start = vertex_point(document, edge.start_vertex());
    let end = vertex_point(document, edge.end_vertex());
    let Some(curve) = document.resolve(edge.curve()) else {
        return match (start, end) {
            (Some(point), _) => vec![point],
            (None, Some(point)) => vec![point],
            (None, None) => Vec::new(),
        };
    };
    let t0 = edge.start_param();
    let t1 = edge.end_param();
    if let Some(straight) = SatStraightCurve::from_record(curve) {
        let root = vec3(straight.root_point());
        let direction = vec3(straight.direction());
        return sample_parameter(t0, t1, tolerance, |parameter| root + direction * parameter);
    }
    if let Some(ellipse) = SatEllipseCurve::from_record(curve) {
        return sample_parameter(t0, t1, tolerance, |parameter| {
            ellipse_point(&ellipse, parameter)
        });
    }
    if let Some(spline) = SatIntCurve::from_record(curve) {
        if let Some((degree, knots, control)) = spline.bspline_in(document) {
            return sample_parameter(t0, t1, tolerance, |parameter| {
                de_boor(degree, &knots, &control, parameter)
            });
        }
    }
    match (start, end) {
        (Some(start), Some(end)) if (end - start).length_squared() > 1e-16 => vec![start, end],
        (Some(point), _) | (_, Some(point)) => vec![point],
        (None, None) => Vec::new(),
    }
}

fn vertex_point(
    document: &SatDocument,
    pointer: crate::entities::acis::SatPointer,
) -> Option<Vector3> {
    let vertex = crate::entities::acis::SatVertex::from_record(document.resolve(pointer)?)?;
    let point = crate::entities::acis::SatPoint::from_record(document.resolve(vertex.point())?)?;
    Some(vec3(point.position()))
}

fn ellipse_point(ellipse: &SatEllipseCurve<'_>, parameter: f64) -> Vector3 {
    let center = vec3(ellipse.center());
    let major = vec3(ellipse.major_axis());
    let normal = unit(vec3(ellipse.normal())).unwrap_or(Vector3::UNIT_Z);
    let minor = normal.cross(&major) * ellipse.ratio();
    center + major * parameter.cos() + minor * parameter.sin()
}

fn sample_parameter(
    start: f64,
    end: f64,
    tolerance: f64,
    eval: impl Fn(f64) -> Vector3,
) -> Vec<Vector3> {
    if !start.is_finite() || !end.is_finite() || (end - start).abs() <= 1e-15 {
        return vec![eval(start)];
    }
    let mut points = vec![eval(start)];
    subdivide(start, end, tolerance, &eval, &mut points, 0);
    let last = eval(end);
    if points
        .last()
        .is_none_or(|point| (*point - last).length_squared() > 1e-16)
    {
        points.push(last);
    }
    points
}

fn subdivide(
    start: f64,
    end: f64,
    tolerance: f64,
    eval: &impl Fn(f64) -> Vector3,
    points: &mut Vec<Vector3>,
    depth: u32,
) {
    if depth >= 16 {
        return;
    }
    let middle_parameter = (start + end) * 0.5;
    let middle = eval(middle_parameter);
    let left = *points.last().unwrap();
    let right = eval(end);
    let chord = right - left;
    let length = chord.length();
    let sagitta = if length <= 1e-12 {
        (middle - left).length()
    } else {
        let t = ((middle - left).dot(&chord) / (length * length)).clamp(0.0, 1.0);
        (middle - (left + chord * t)).length()
    };
    let turn = {
        let a = middle - left;
        let b = right - middle;
        let denom = a.length() * b.length();
        if denom <= 1e-12 {
            0.0
        } else {
            (a.dot(&b) / denom).clamp(-1.0, 1.0).acos()
        }
    };
    if sagitta > tolerance || turn > 0.25 {
        subdivide(start, middle_parameter, tolerance, eval, points, depth + 1);
        points.push(middle);
        subdivide(middle_parameter, end, tolerance, eval, points, depth + 1);
    }
}

fn eval_spline(patch: &SatBSplineSurface, u: f64, v: f64) -> Vector3 {
    let columns = patch.control_count_u.max(1);
    let rows = patch.control_count_v.max(1);
    let mut curve = Vec::with_capacity(rows);
    for row in 0..rows {
        let controls: Vec<[f64; 4]> = (0..columns)
            .map(|column| {
                patch
                    .control_points
                    .get(row * columns + column)
                    .copied()
                    .unwrap_or([0.0, 0.0, 0.0, 1.0])
            })
            .collect();
        curve.push(de_boor_homogeneous(
            patch.degree_u,
            &patch.u_knots,
            &controls,
            u,
        ));
    }
    cartesian(de_boor_homogeneous(
        patch.degree_v,
        &patch.v_knots,
        &curve,
        v,
    ))
}

fn de_boor(degree: usize, knots: &[f64], control: &[[f64; 4]], parameter: f64) -> Vector3 {
    cartesian(de_boor_homogeneous(degree, knots, control, parameter))
}

fn de_boor_homogeneous(
    degree: usize,
    knots: &[f64],
    control: &[[f64; 4]],
    parameter: f64,
) -> [f64; 4] {
    if control.is_empty() || knots.len() < 2 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let degree = degree.min(control.len().saturating_sub(1));
    let domain_start = knots.get(degree).copied().unwrap_or(0.0);
    let domain_end = knots
        .get(knots.len().saturating_sub(degree + 1))
        .copied()
        .unwrap_or(domain_start);
    let parameter = parameter.clamp(domain_start.min(domain_end), domain_start.max(domain_end));
    let mut span = degree;
    while span + 1 < knots.len() && knots[span + 1] <= parameter && span + 1 < control.len() {
        span += 1;
    }
    span = span.min(control.len() - 1).max(degree);
    let mut work: Vec<[f64; 4]> = (0..=degree)
        .map(|offset| control[(span - degree + offset).min(control.len() - 1)])
        .collect();
    for round in 1..=degree {
        for index in (round..=degree).rev() {
            let knot_index = span - degree + index;
            let upper = knots
                .get(knot_index + degree - round + 1)
                .copied()
                .unwrap_or(0.0);
            let lower = knots.get(knot_index).copied().unwrap_or(0.0);
            let span_width = upper - lower;
            let weight = if span_width.abs() > 1e-12 {
                (parameter - lower) / span_width
            } else {
                0.0
            };
            for channel in 0..4 {
                work[index][channel] =
                    (1.0 - weight) * work[index - 1][channel] + weight * work[index][channel];
            }
        }
    }
    work[degree]
}

fn cartesian(point: [f64; 4]) -> Vector3 {
    let weight = if point[3].abs() > 1e-12 {
        point[3]
    } else {
        1.0
    };
    Vector3::new(point[0] / weight, point[1] / weight, point[2] / weight)
}

fn knot_start(knots: &[f64], degree: usize) -> f64 {
    knots.get(degree).copied().unwrap_or(0.0)
}

fn knot_end(knots: &[f64], degree: usize) -> f64 {
    knots
        .get(knots.len().saturating_sub(degree + 1))
        .copied()
        .unwrap_or(0.0)
}

/// Triangulate indexed faces. Each face keeps a constant normal, so creases
/// stay sharp. `None` indices are skipped.
pub(crate) fn facet_faces(vertices: &[Vector3], faces: &[Vec<usize>]) -> Tessellation {
    let mut tess = Tessellation::new();
    for face in faces {
        let polygon = face
            .iter()
            .filter_map(|index| vertices.get(*index).copied())
            .collect::<Vec<_>>();
        if polygon.len() < 3 {
            continue;
        }
        let normal = unit(newell(&polygon)).unwrap_or(Vector3::UNIT_Z);
        let (triangles, normals) = triangulate_plane(&[polygon], normal);
        let edge = face
            .iter()
            .filter_map(|index| vertices.get(*index).copied())
            .collect::<Vec<_>>();
        if edge.len() >= 2 {
            let mut closed = edge;
            closed.push(closed[0]);
            tess.edges.push(closed);
        }
        tess.triangles.extend(triangles);
        tess.normals.extend(normals);
    }
    tess
}

/// `m` rows by `n` columns, row-major. Closed flags wrap that direction.
pub(crate) fn facet_grid(
    vertices: &[Vector3],
    rows: usize,
    columns: usize,
    close_rows: bool,
    close_columns: bool,
) -> Tessellation {
    if rows < 2 || columns < 2 || vertices.len() < rows * columns {
        return Tessellation::new();
    }
    let mut faces = Vec::new();
    let row_count = if close_rows { rows } else { rows - 1 };
    let column_count = if close_columns { columns } else { columns - 1 };
    for row in 0..row_count {
        for column in 0..column_count {
            let next_row = (row + 1) % rows;
            let next_column = (column + 1) % columns;
            faces.push(vec![
                row * columns + column,
                row * columns + next_column,
                next_row * columns + next_column,
                next_row * columns + column,
            ]);
        }
    }
    facet_faces(vertices, &faces)
}

fn push_triangle(
    tess: &mut Tessellation,
    a: Vector3,
    b: Vector3,
    c: Vector3,
    na: Vector3,
    nb: Vector3,
    nc: Vector3,
) {
    if triangle_area(&a, &b, &c) <= 1e-14 {
        return;
    }
    tess.triangles.extend([a, b, c]);
    tess.normals.extend([na, nb, nc]);
}

fn model_tolerance(document: &SatDocument, place: &Placement) -> f64 {
    let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut grow = |point: Vector3| {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        min.z = min.z.min(point.z);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
        max.z = max.z.max(point.z);
    };
    for record in &document.records {
        if let Some(point) = crate::entities::acis::SatPoint::from_record(record) {
            grow(place.point(vec3(point.position())));
        }
        if let Some(sphere) = SatSphereSurface::from_record(record) {
            let center = place.point(vec3(sphere.center()));
            let radius = sphere.radius().abs();
            grow(center + Vector3::new(radius, radius, radius));
            grow(center - Vector3::new(radius, radius, radius));
        }
        if let Some(torus) = SatTorusSurface::from_record(record) {
            let center = place.point(vec3(torus.center()));
            let reach = torus.major_radius().abs() + torus.minor_radius().abs();
            grow(center + Vector3::new(reach, reach, reach));
            grow(center - Vector3::new(reach, reach, reach));
        }
        if let Some(spline) = SatSplineSurface::from_record(record) {
            if let Some(patch) = spline.bspline(document) {
                for control in &patch.control_points {
                    grow(place.point(cartesian(*control)));
                }
            }
        }
    }
    let diagonal = (max - min).length();
    if !diagonal.is_finite() || diagonal < 1e-9 {
        1e-4
    } else {
        (diagonal * 0.002).max(1e-6)
    }
}

fn chord_step(radius: f64, tolerance: f64) -> f64 {
    let radius = radius.abs().max(tolerance.max(1e-9));
    let ratio = (1.0 - tolerance / radius).clamp(-1.0, 1.0);
    let angle = 2.0 * ratio.acos();
    if angle.is_finite() {
        angle.clamp(0.08, PI / 2.0)
    } else {
        0.3
    }
}

fn newell(points: &[Vector3]) -> Vector3 {
    let mut normal = Vector3::ZERO;
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        normal.x += (current.y - next.y) * (current.z + next.z);
        normal.y += (current.z - next.z) * (current.x + next.x);
        normal.z += (current.x - next.x) * (current.y + next.y);
    }
    normal
}

fn plane_axis(normal: Vector3) -> Vector3 {
    let hint = if normal.x.abs() < 0.9 {
        Vector3::UNIT_X
    } else {
        Vector3::UNIT_Y
    };
    unit(normal.cross(&hint)).unwrap_or(Vector3::UNIT_X)
}

fn triangle_area(a: &Vector3, b: &Vector3, c: &Vector3) -> f64 {
    (*b - *a).cross(&(*c - *a)).length() * 0.5
}

fn oriented(normal: Vector3, sense: Sense) -> Vector3 {
    if sense == Sense::Reversed {
        normal * -1.0
    } else {
        normal
    }
}

fn vec3(value: (f64, f64, f64)) -> Vector3 {
    Vector3::new(value.0, value.1, value.2)
}

fn unit(value: Vector3) -> Option<Vector3> {
    let length = value.length();
    (length > 1e-12).then_some(value * (1.0 / length))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trimmed_cylinder_loop_triangulates_on_its_surface() {
        let frame = ConeFrame {
            center: Vector3::ZERO,
            axis: Vector3::UNIT_Z,
            major: Vector3::UNIT_X * 2.0,
            minor: Vector3::UNIT_Y * 2.0,
            sin_half: 0.0,
            cos_half: 1.0,
            radius: 2.0,
        };
        let mut boundary = (0..=16)
            .map(|step| frame.point_at(PI * 0.5 * step as f64 / 16.0, 0.0))
            .collect::<Vec<_>>();
        boundary.extend((1..=16).map(|step| frame.point_at(PI * 0.5, 5.0 * step as f64 / 16.0)));
        boundary.extend(
            (1..=16).map(|step| frame.point_at(PI * 0.5 * (1.0 - step as f64 / 16.0), 5.0)),
        );

        let tolerance = 0.005;
        let (triangles, normals) =
            triangulate_cone_loop(&boundary, &frame, false, tolerance).unwrap();
        assert!(!triangles.is_empty());
        assert_eq!(triangles.len(), normals.len());
        for (triangle, normal) in triangles.chunks_exact(3).zip(normals.chunks_exact(3)) {
            let center = (triangle[0] + triangle[1] + triangle[2]) / 3.0;
            let radius = center.x.hypot(center.y);
            assert!(radius > 1.8, "triangle collapsed into the cylinder: {center:?}");
            let uv = triangle
                .iter()
                .map(|point| (frame.angle(*point), frame.height(*point)))
                .collect::<Vec<_>>();
            let surface_center = frame.point_at(
                uv.iter().map(|point| point.0).sum::<f64>() / 3.0,
                uv.iter().map(|point| point.1).sum::<f64>() / 3.0,
            );
            assert!(
                (surface_center - center).length() <= tolerance * 1.01,
                "triangle exceeds surface tolerance: {:?}",
                (surface_center - center).length()
            );
            for (point, normal) in triangle.iter().zip(normal) {
                let radial = unit(Vector3::new(point.x, point.y, 0.0)).unwrap();
                assert!(
                    radial.dot(normal) > 0.99,
                    "inward cylinder normal: {normal:?}"
                );
            }
        }
    }

    #[test]
    fn cone_angle_inverts_an_elliptical_frame() {
        let frame = ConeFrame {
            center: Vector3::ZERO,
            axis: Vector3::UNIT_Z,
            major: Vector3::UNIT_X * 4.0,
            minor: Vector3::UNIT_Y * 0.7,
            sin_half: 0.0,
            cos_half: 1.0,
            radius: 4.0,
        };
        let expected = 1.23;
        let point = frame.point_at(expected, 2.0);
        assert!((frame.angle(point) - expected).abs() < 1e-12);
    }
}
