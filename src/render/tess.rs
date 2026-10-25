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
    SatBSplineSurface, SatCoedge, SatConeSurface, SatDocument, SatEllipseCurve, SatFace,
    SatIntCurve, SatLoop, SatPCurve, SatPlaneSurface, SatSphereSurface, SatSplineSurface,
    SatStraightCurve, SatTorusSurface, Sense,
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
            facet_trimmed_sphere(&sphere, &loops, sense, place, tolerance, tess);
        }
        return;
    }
    if let Some(torus) = SatTorusSurface::from_record(surface) {
        if loops.is_empty() {
            facet_torus(&torus, sense, place, tolerance, tess);
        } else {
            facet_trimmed_torus(&torus, &loops, sense, place, tolerance, tess);
        }
        return;
    }
    if let Some(spline) = SatSplineSurface::from_record(surface) {
        if let Some(patch) = spline
            .bspline(document)
            .or_else(|| spline.offset_in(document).map(|(patch, _)| patch))
        {
            if let Some((parameters, boundaries)) = spline_face_parameters(document, face, edges) {
                facet_trimmed_spline(
                    &patch,
                    &parameters,
                    &boundaries,
                    sense,
                    place,
                    tolerance,
                    tess,
                );
            } else {
                facet_spline(&patch, sense, place, tolerance, tess);
            }
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

/// Decode the UV boundary of a spline face from its coedge p-curves.
///
/// The ordinary edge samples are retained as the source points passed to the
/// parameter triangulator. This keeps every trimmed spline face on the exact
/// same sampled boundary as adjacent faces, while the p-curves provide the
/// surface parameters needed to evaluate the interior.
fn spline_face_parameters(
    document: &SatDocument,
    face: &SatFace<'_>,
    edges: &HashMap<usize, Vec<Vector3>>,
) -> Option<(Vec<Vec<(f64, f64)>>, Vec<Vec<Vector3>>)> {
    let mut parameters = Vec::new();
    let mut boundaries = Vec::new();
    for loop_pointer in face_loops(document, face) {
        let loop_record = document.resolve(loop_pointer)?;
        let loop_ = SatLoop::from_record(loop_record)?;
        let start = loop_.first_coedge();
        let mut cursor = start;
        let mut parameter_ring: Vec<(f64, f64)> = Vec::new();
        let mut boundary_ring: Vec<Vector3> = Vec::new();
        let mut guard = 0;
        while !cursor.is_null() && guard < 256 {
            let coedge_record = document.resolve(cursor)?;
            let coedge = SatCoedge::from_record(coedge_record)?;
            let edge_index = coedge.edge().index()?;
            let edge_points = edges.get(&edge_index)?;
            if edge_points.len() < 2 {
                return None;
            }
            let pcurve_record = document.resolve(coedge.pcurve())?;
            let pcurve = SatPCurve::from_record(pcurve_record)?;
            let mut uv = pcurve.sample_in(document, edge_points.len().saturating_sub(1).max(1));
            if uv.len() < 2 || uv.len() != edge_points.len() {
                return None;
            }
            let mut boundary = edge_points.clone();
            // Edge samples are oriented from the edge's start vertex. A
            // reversed coedge walks both the 3-D edge and its p-curve back.
            if coedge.sense() == Sense::Reversed {
                uv.reverse();
                boundary.reverse();
            }
            if parameter_ring
                .last()
                .zip(uv.first())
                .is_some_and(|(last, first)| {
                    (last.0 - first.0).abs() <= 1e-8 && (last.1 - first.1).abs() <= 1e-8
                })
            {
                uv.remove(0);
                boundary.remove(0);
            }
            parameter_ring.extend(uv);
            boundary_ring.extend(boundary);
            cursor = coedge.next();
            if cursor == start {
                break;
            }
            guard += 1;
        }
        if parameter_ring.len() >= 2
            && parameter_ring
                .first()
                .zip(parameter_ring.last())
                .is_some_and(|(first, last)| {
                    (first.0 - last.0).abs() <= 1e-8 && (first.1 - last.1).abs() <= 1e-8
                })
        {
            parameter_ring.pop();
            boundary_ring.pop();
        }
        if parameter_ring.len() < 3 || parameter_ring.len() != boundary_ring.len() {
            return None;
        }
        if parameter_ring
            .iter()
            .any(|(u, v)| !u.is_finite() || !v.is_finite())
            || boundary_ring
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite() || !point.z.is_finite())
        {
            return None;
        }
        parameters.push(parameter_ring);
        boundaries.push(boundary_ring);
    }
    (!parameters.is_empty()).then_some((parameters, boundaries))
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
    // Earcut expects the first ring to be the exterior and all following
    // rings to be holes. ACIS does not guarantee that loop order, so place
    // the largest projected ring first before building the flattened input.
    let ordered = std::iter::once(outer_index)
        .chain((0..loops.len()).filter(move |index| *index != outer_index));
    for (ring_index, index) in ordered.enumerate() {
        let loop_ = &loops[index];
        if loop_.len() < 3 {
            continue;
        }
        if ring_index > 0 {
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

    let (parameters, indices, sources) = triangulate_parameters(
        &[parameters],
        |angle, height| frame.point_at(angle, height),
        tolerance,
        Some(&[points.to_vec()]),
    )?;
    let mut triangles = Vec::with_capacity(indices.len() * 3);
    let mut normals = Vec::with_capacity(indices.len() * 3);
    for triangle in indices {
        for index in triangle {
            let (angle, height) = parameters[index];
            let point = sources[index].unwrap_or_else(|| frame.point_at(angle, height));
            triangles.push(point);
            normals.push(frame.normal_at(point, flip));
        }
    }
    Some((triangles, normals))
}

/// Refine a trimmed parameter polygon without changing its sampled boundaries.
/// Each inserted interior midpoint is shared by all incident triangles.
fn triangulate_parameters(
    loops: &[Vec<(f64, f64)>],
    point_at: impl Fn(f64, f64) -> Vector3,
    tolerance: f64,
    source_loops: Option<&[Vec<Vector3>]>,
) -> Option<(Vec<(f64, f64)>, Vec<[usize; 3]>, Vec<Option<Vector3>>)> {
    let area = |ring: &Vec<(f64, f64)>| {
        (0..ring.len())
            .map(|index| {
                let (u0, v0) = ring[index];
                let (u1, v1) = ring[(index + 1) % ring.len()];
                u0 * v1 - u1 * v0
            })
            .sum::<f64>()
            .abs()
    };
    let outer = (0..loops.len())
        .filter(|&index| loops[index].len() >= 3)
        .max_by(|&left, &right| area(&loops[left]).total_cmp(&area(&loops[right])))?;
    let mut parameters = Vec::new();
    let mut sources = Vec::new();
    let mut flat = Vec::new();
    let mut holes = Vec::new();
    let mut boundary_edges = std::collections::HashSet::new();
    for index in std::iter::once(outer).chain((0..loops.len()).filter(|&index| index != outer)) {
        let ring = &loops[index];
        if ring.len() < 3 {
            continue;
        }
        let start = parameters.len();
        if start > 0 {
            holes.push(start);
        }
        for (offset, &(u, v)) in ring.iter().enumerate() {
            flat.extend([u, v]);
            parameters.push((u, v));
            sources.push(
                source_loops
                    .and_then(|loops| loops.get(index))
                    .and_then(|ring| ring.get(offset))
                    .copied(),
            );
        }
        for offset in 0..ring.len() {
            boundary_edges.insert(edge_key(start + offset, start + (offset + 1) % ring.len()));
        }
    }
    let indices = earcutr::earcut(&flat, &holes, 2).ok()?;
    if indices.is_empty() {
        return None;
    }

    let uv_triangles = indices
        .chunks_exact(3)
        .map(|triangle| [triangle[0], triangle[1], triangle[2]])
        // Nearly collinear trims can leave zero-area parameter triangles.
        // On a curved surface their three evaluated points need not be
        // collinear, so emitting them would add spurious folded faces.
        .filter(|triangle| {
            let [a, b, c] = triangle.map(|index| parameters[index]);
            let u = (b.0 - a.0).abs().max((c.0 - a.0).abs());
            let v = (b.1 - a.1).abs().max((c.1 - a.1).abs());
            let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
            cross.abs() > 1e-12 * u.max(v).powi(2)
        })
        .collect::<Vec<_>>();
    let tolerance = tolerance.abs().max(1e-6);
    let max_vertices = 16_384;

    let mut uv_triangles = uv_triangles;

    // Split only interior edges, sharing every inserted midpoint between the
    // incident triangles so the refined mesh remains conforming.
    for _ in 0..16 {
        let mut split_edges = std::collections::HashSet::new();
        for &[a, b, c] in &uv_triangles {
            let corners = [a, b, c];
            let world = corners.map(|index| {
                let (angle, height) = parameters[index];
                point_at(angle, height)
            });
            let mut longest = None;
            for side in 0..3 {
                let first = corners[side];
                let second = corners[(side + 1) % 3];
                let key = edge_key(first, second);
                let (u0, h0) = parameters[first];
                let (u1, h1) = parameters[second];
                let curved_midpoint = point_at((u0 + u1) * 0.5, (h0 + h1) * 0.5);
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
                .fold((0.0, 0.0), |sum, point| {
                    (sum.0 + point.0 / 3.0, sum.1 + point.1 / 3.0)
                });
            let curved_center = point_at(center_uv.0, center_uv.1);
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
            sources.push(None);
            midpoint_indices.insert((first, second), index);
        }

        let midpoint =
            |first: usize, second: usize| midpoint_indices.get(&edge_key(first, second)).copied();
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
                (Some(ab), Some(bc), Some(ca)) => {
                    refined.extend([[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]])
                }
            }
        }
        uv_triangles = refined;
    }

    Some((parameters, uv_triangles, sources))
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
    let tolerance = tolerance.min(frame.major.length().max(frame.minor.length()) * 0.001);
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
            let normal = unit(frame.major.cross(&frame.minor)).unwrap_or(Vector3::UNIT_Z);
            let normal = oriented(normal, sense);
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
        let local_axis = unit(vec3(cone.axis())).unwrap_or(Vector3::UNIT_Z);
        let major = vec3(cone.major_axis());
        let major = major - local_axis * major.dot(&local_axis);
        let major = if major.length_squared() <= 1e-18 {
            plane_axis(local_axis) * cone.radius().abs().max(1e-6)
        } else {
            major
        };
        let ratio = if cone.ratio().abs() <= 1e-12 {
            1.0
        } else {
            cone.ratio()
        };
        let radius = major.length().max(1e-9);
        let minor = local_axis.cross(&major) * ratio;
        Self {
            center: place.point(vec3(cone.center())),
            axis: place.linear * local_axis,
            major: place.linear * major,
            minor: place.linear * minor,
            sin_half: cone.sin_half_angle(),
            cos_half: cone.cos_half_angle(),
            radius,
        }
    }

    fn height(&self, point: Vector3) -> f64 {
        // The transformed axis need not be perpendicular to the ellipse
        // plane after shear. Its dual recovers the original local height.
        let plane = self.major.cross(&self.minor);
        let denominator = self.axis.dot(&plane);
        if denominator.abs() <= 1e-18 {
            return 0.0;
        }
        (point - self.center).dot(&plane) / denominator
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
            (radial.dot(&self.major) * minor_norm - radial.dot(&self.minor) * cross) / determinant;
        let minor_component =
            (radial.dot(&self.minor) * major_norm - radial.dot(&self.major) * cross) / determinant;
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
        if self.major.cross(&self.minor).dot(&self.axis) < 0.0 {
            normal = normal * -1.0;
        }
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
    rotated.sort_by(|a, b| a.0.total_cmp(&b.0));
    rotated.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-10);
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
    let local_center = vec3(sphere.center());
    let center = place.point(local_center);
    let radius = sphere.radius().abs();
    let pole = unit(vec3(sphere.pole())).unwrap_or(Vector3::UNIT_Z);
    let u_direction = vec3(sphere.u_direction());
    let east =
        unit(u_direction - pole * u_direction.dot(&pole)).unwrap_or_else(|| plane_axis(pole));
    let north = unit(pole.cross(&east)).unwrap_or(Vector3::UNIT_Y);
    let flip = sense == Sense::Reversed;
    let placement_stretch = place
        .linear
        .m
        .iter()
        .flatten()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt();
    let tolerance = tolerance.min(radius * placement_stretch * 0.001);
    let step = chord_step(radius * placement_stretch, tolerance);
    let columns = (TAU / step).ceil().max(8.0) as usize;
    let rows = (PI / step).ceil().max(4.0) as usize;
    let direction_at = |row: usize, column: usize| -> Vector3 {
        let v = PI * (row as f64 / rows as f64);
        let u = TAU * (column as f64 / columns as f64);
        east * (v.sin() * u.cos()) + north * (v.sin() * u.sin()) + pole * v.cos()
    };
    let point_at = |row, column| place.point(local_center + direction_at(row, column) * radius);
    let normal_at = |direction: Vector3| {
        let mut normal = place.normal(direction);
        if flip {
            normal = normal * -1.0;
        }
        normal
    };
    for column in 0..columns {
        let south = place.point(local_center + pole * radius);
        let a = point_at(1, column);
        let b = point_at(1, column + 1);
        push_triangle(
            tess,
            south,
            b,
            a,
            normal_at(pole),
            normal_at(direction_at(1, column + 1)),
            normal_at(direction_at(1, column)),
        );
        let north_pole = place.point(local_center - pole * radius);
        let c = point_at(rows - 1, column);
        let d = point_at(rows - 1, column + 1);
        push_triangle(
            tess,
            north_pole,
            c,
            d,
            normal_at(-pole),
            normal_at(direction_at(rows - 1, column)),
            normal_at(direction_at(rows - 1, column + 1)),
        );
    }
    for row in 1..rows.saturating_sub(1) {
        for column in 0..columns {
            let a = point_at(row, column);
            let b = point_at(row, column + 1);
            let c = point_at(row + 1, column + 1);
            let d = point_at(row + 1, column);
            let na = normal_at(direction_at(row, column));
            let nb = normal_at(direction_at(row, column + 1));
            let nc = normal_at(direction_at(row + 1, column + 1));
            let nd = normal_at(direction_at(row + 1, column));
            push_triangle(tess, a, b, c, na, nb, nc);
            push_triangle(tess, a, c, d, na, nc, nd);
        }
    }
    for step in 0..4 {
        let u = TAU * (step as f64 / 4.0);
        tess.isoline_curves.push(super::scene::Curve::Elliptic {
            center,
            u: place.linear * (pole * radius),
            v: place.linear * ((east * u.cos() + north * u.sin()) * radius),
            start: 0.0,
            sweep: PI,
        });
    }
    for step in 1..4 {
        let v = PI * (step as f64 / 4.0);
        let radial = v.sin() * radius;
        tess.isoline_curves.push(super::scene::Curve::Elliptic {
            center: place.point(local_center + pole * (v.cos() * radius)),
            u: place.linear * (east * radial),
            v: place.linear * (north * radial),
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
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let Some(inverse) = place.linear.inverse() else {
        return;
    };
    let center = vec3(sphere.center());
    let radius = sphere.radius().abs();
    if radius <= 1e-12 {
        return;
    }
    let stretch = place
        .linear
        .m
        .iter()
        .map(|row| row.iter().map(|value| value * value).sum::<f64>().sqrt())
        .fold(0.0_f64, f64::max);
    let tolerance = tolerance.min(radius * stretch * 0.001);

    // A stereographic chart centered on the trim avoids the sphere's
    // longitude seam and remains regular at either original pole.
    let mut radial_loops = Vec::new();
    let mut boundaries = Vec::new();
    let mut directions = Vec::new();
    for ring in loops.iter().filter(|ring| ring.len() >= 3) {
        let local = ring
            .iter()
            .map(|point| inverse * (*point - place.translation))
            .collect::<Vec<_>>();
        let radial = local
            .iter()
            .map(|point| unit(*point - center).unwrap_or(Vector3::UNIT_Z))
            .collect::<Vec<_>>();
        directions.extend(radial.iter().copied());
        radial_loops.push(radial);
        boundaries.push(ring.clone());
    }
    if radial_loops.is_empty() {
        return;
    }
    let mut chart_pole = directions
        .iter()
        .copied()
        .fold(Vector3::ZERO, |sum, direction| sum + direction);
    if chart_pole.length_squared() <= 1e-18 {
        chart_pole = vec3(sphere.pole());
    }
    let clearance = |axis: Vector3| {
        directions
            .iter()
            .map(|p| 1.0 + p.dot(&axis))
            .fold(f64::INFINITY, f64::min)
    };
    // The negative axis of an oriented circular trim lies on its removed
    // side, including circular holes. Put the chart singularity there so a
    // large cap cannot be mistaken for its bounded, smaller complement.
    let circular_axes = radial_loops.iter().filter_map(|ring| {
        let axis = unit(newell(ring))?;
        let height = ring[0].dot(&axis);
        ring.iter()
            .all(|p| (p.dot(&axis) - height).abs() < 1e-8)
            .then_some(axis)
    });
    if let Some(axis) = circular_axes.max_by(|&a, &b| clearance(a).total_cmp(&clearance(b))) {
        chart_pole = axis;
    }
    let mut chart_pole = unit(chart_pole).unwrap_or(Vector3::UNIT_Z);
    if clearance(chart_pole) < 1e-6 {
        // Move the chart singularity off a sampled trim before projection.
        chart_pole = [
            Vector3::UNIT_X,
            -Vector3::UNIT_X,
            Vector3::UNIT_Y,
            -Vector3::UNIT_Y,
            Vector3::UNIT_Z,
            -Vector3::UNIT_Z,
        ]
        .into_iter()
        .max_by(|&a, &b| clearance(a).total_cmp(&clearance(b)))
        .unwrap();
    }
    let chart_east = vec3(sphere.u_direction());
    let chart_east = chart_east - chart_pole * chart_east.dot(&chart_pole);
    let chart_east = unit(chart_east).unwrap_or_else(|| plane_axis(chart_pole));
    let chart_north = unit(chart_pole.cross(&chart_east)).unwrap_or(Vector3::UNIT_Y);
    let mut parameters = Vec::with_capacity(radial_loops.len());
    for radial in radial_loops {
        let mut ring = Vec::with_capacity(radial.len());
        for direction in radial {
            let denominator = 1.0 + direction.dot(&chart_pole);
            if denominator <= 1e-8 {
                return;
            }
            ring.push((
                direction.dot(&chart_east) / denominator,
                direction.dot(&chart_north) / denominator,
            ));
        }
        parameters.push(ring);
    }
    let direction_at = |u: f64, v: f64| {
        let denominator = 1.0 + u * u + v * v;
        (chart_east * (2.0 * u) + chart_north * (2.0 * v) + chart_pole * (1.0 - u * u - v * v))
            / denominator
    };
    let point_at = |u, v| place.point(center + direction_at(u, v) * radius);
    let normal_at = |u, v| place.normal(oriented(direction_at(u, v), sense));
    for group in parameter_regions(&parameters) {
        let rings = group
            .iter()
            .map(|&index| parameters[index].clone())
            .collect::<Vec<_>>();
        let source_rings = group
            .iter()
            .map(|&index| boundaries[index].clone())
            .collect::<Vec<_>>();
        let Some((parameters, indices, sources)) =
            triangulate_parameters(&rings, point_at, tolerance, Some(&source_rings))
        else {
            continue;
        };
        for triangle in indices {
            let points = triangle.map(|index| {
                let (u, v) = parameters[index];
                sources[index].unwrap_or_else(|| point_at(u, v))
            });
            let normals = triangle.map(|index| {
                let (u, v) = parameters[index];
                normal_at(u, v)
            });
            push_triangle(
                tess, points[0], points[1], points[2], normals[0], normals[1], normals[2],
            );
        }
    }
}

/// Group each exterior with its immediate holes, regardless of loop order.
/// Disjoint exteriors and islands nested inside holes form separate regions.
fn parameter_regions(loops: &[Vec<(f64, f64)>]) -> Vec<Vec<usize>> {
    let areas = loops
        .iter()
        .map(|ring| {
            ring.iter()
                .zip(ring.iter().cycle().skip(1))
                .take(ring.len())
                .map(|(&(u0, v0), &(u1, v1))| u0 * v1 - u1 * v0)
                .sum::<f64>()
                .abs()
        })
        .collect::<Vec<_>>();
    let contains = |ring: &[(f64, f64)], point: (f64, f64)| {
        let mut inside = false;
        for (&a, &b) in ring
            .iter()
            .zip(ring.iter().cycle().skip(1))
            .take(ring.len())
        {
            if (a.1 > point.1) != (b.1 > point.1)
                && point.0 < (b.0 - a.0) * (point.1 - a.1) / (b.1 - a.1) + a.0
            {
                inside = !inside;
            }
        }
        inside
    };
    let parents = loops
        .iter()
        .enumerate()
        .map(|(index, ring)| {
            (0..loops.len())
                .filter(|&other| areas[other] > areas[index] && contains(&loops[other], ring[0]))
                .min_by(|&a, &b| areas[a].total_cmp(&areas[b]))
        })
        .collect::<Vec<_>>();
    let depths = (0..loops.len())
        .map(|index| {
            let mut depth = 0;
            let mut parent = parents[index];
            while let Some(index) = parent {
                depth += 1;
                parent = parents[index];
            }
            depth
        })
        .collect::<Vec<_>>();
    (0..loops.len())
        .filter(|&index| depths[index] % 2 == 0)
        .map(|index| {
            std::iter::once(index)
                .chain((0..loops.len()).filter(|&child| parents[child] == Some(index)))
                .collect()
        })
        .collect()
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

/// A fillet can cross either periodic seam, or wrap all the way around the
/// torus between two closed boundary rings. Unwrap its trims before earcut,
/// then evaluate the refined triangles on the actual curved surface.
fn facet_trimmed_torus(
    torus: &SatTorusSurface<'_>,
    loops: &[Vec<Vector3>],
    sense: Sense,
    place: &Placement,
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let Some(inverse) = place.linear.inverse() else {
        return;
    };
    let center = vec3(torus.center());
    let axis = unit(vec3(torus.normal())).unwrap_or(Vector3::UNIT_Z);
    let east = vec3(torus.u_direction());
    let east = unit(east - axis * east.dot(&axis)).unwrap_or(plane_axis(axis));
    let north = axis.cross(&east);
    let major = torus.major_radius().abs();
    let minor = torus.minor_radius().abs();
    let point_at = |u: f64, v: f64| {
        let radial = east * u.cos() + north * u.sin();
        place.point(center + radial * (major + minor * v.cos()) + axis * (minor * v.sin()))
    };
    let normal_at = |u: f64, v: f64| {
        let radial = east * u.cos() + north * u.sin();
        place.normal(oriented(radial * v.cos() + axis * v.sin(), sense))
    };
    let mut emit = |vertices: [(f64, f64, Vector3); 3]| {
        let uv = vertices.map(|(u, v, _)| (u, v));
        let [a, b, c] = vertices.map(|(_, _, point)| point);
        let [na, nb, nc] = uv.map(|(u, v)| normal_at(u, v));
        push_triangle(tess, a, b, c, na, nb, nc);
    };
    let mut parameters = Vec::new();
    let mut boundaries = Vec::new();
    let mut windings = Vec::new();
    for ring in loops.iter().filter(|ring| ring.len() >= 3) {
        let mut uv = Vec::<(f64, f64)>::with_capacity(ring.len());
        for &point in ring {
            let local = inverse * (point - place.translation) - center;
            let x = local.dot(&east);
            let y = local.dot(&north);
            let u = y.atan2(x);
            let v = local.dot(&axis).atan2(x.hypot(y) - major);
            let point = if let Some(&(previous_u, previous_v)) = uv.last() {
                (unwrap_angle(u, previous_u), unwrap_angle(v, previous_v))
            } else {
                (u, v)
            };
            uv.push(point);
        }
        let first = uv[0];
        let last = *uv.last().unwrap();
        windings.push((
            unwrap_angle(first.0, last.0) - first.0,
            unwrap_angle(first.1, last.1) - first.1,
        ));
        parameters.push(uv);
        boundaries.push(ring.clone());
    }
    if parameters.is_empty() {
        return;
    }
    if windings.iter().any(|&(u, v)| u.abs() > PI || v.abs() > PI) {
        // A periodic band has two winding trims. Join them on a grid instead
        // of earcutting long, thin triangles across a full revolution.
        if parameters.len() != 2 {
            return;
        }
        let swap = windings[0].1.abs() > PI;
        let along = |w: (f64, f64)| if swap { w.1 } else { w.0 };
        let across = |w: (f64, f64)| if swap { w.0 } else { w.1 };
        if windings
            .iter()
            .any(|&w| along(w).abs() < PI || across(w).abs() > PI)
        {
            return;
        }
        if swap {
            for ring in &mut parameters {
                for point in ring {
                    *point = (point.1, point.0);
                }
            }
        }
        let start = parameters[0][0].0;
        let left = periodic_parameter_ring(&parameters[0], &boundaries[0], start);
        let mut right = periodic_parameter_ring(&parameters[1], &boundaries[1], start);
        let shift = ((left[0].1 - right[0].1) / TAU).round() * TAU;
        for (_, v, _) in &mut right {
            *v += shift;
        }
        let world_radius = (major + minor)
            * place
                .linear
                .m
                .iter()
                .flatten()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
        let step = chord_step(world_radius, tolerance * 0.5);
        let divisions = (TAU / step).ceil() as usize;
        let mut columns = left.iter().chain(&right).map(|p| p.0).collect::<Vec<_>>();
        columns.extend((0..=divisions).map(|index| start + TAU * index as f64 / divisions as f64));
        columns.sort_by(f64::total_cmp);
        columns.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        let value_at = |ring: &[(f64, f64, Vector3)], u: f64| {
            let index = ring.partition_point(|p| p.0 < u).clamp(1, ring.len() - 1);
            let (u0, v0, p0) = ring[index - 1];
            let (u1, v1, p1) = ring[index];
            let t = ((u - u0) / (u1 - u0)).clamp(0.0, 1.0);
            (v0 + (v1 - v0) * t, p0 + (p1 - p0) * t)
        };
        let width = columns
            .iter()
            .map(|&u| (value_at(&right, u).0 - value_at(&left, u).0).abs())
            .fold(0.0, f64::max);
        let rows = (width / step).ceil().max(1.0) as usize;
        for pair in columns.windows(2) {
            let at = |u: f64, row: usize| {
                let (v0, p0) = value_at(&left, u);
                let (v1, p1) = value_at(&right, u);
                let v = v0 + (v1 - v0) * row as f64 / rows as f64;
                let uv = if swap { (v, u) } else { (u, v) };
                // Extra grid columns must stay on the shared sampled edge
                // chords, rather than opening cracks against adjacent faces.
                let point = if row == 0 {
                    p0
                } else if row == rows {
                    p1
                } else {
                    point_at(uv.0, uv.1)
                };
                (uv.0, uv.1, point)
            };
            for row in 0..rows {
                let a = at(pair[0], row);
                let b = at(pair[1], row);
                let c = at(pair[1], row + 1);
                let d = at(pair[0], row + 1);
                emit([a, b, c]);
                emit([a, c, d]);
            }
        }
        return;
    } else {
        // Put holes in the same periodic copy as the exterior, even when
        // their first points lie on opposite sides of atan2's branch cut.
        let mean = |ring: &Vec<(f64, f64)>| {
            ring.iter()
                .fold((0.0, 0.0), |sum, p| (sum.0 + p.0, sum.1 + p.1))
        };
        let reference = parameters.iter().max_by_key(|ring| ring.len()).unwrap();
        let reference_mean = mean(reference);
        let reference_mean = (
            reference_mean.0 / reference.len() as f64,
            reference_mean.1 / reference.len() as f64,
        );
        for ring in &mut parameters {
            let center = mean(ring);
            let shift_u = ((reference_mean.0 - center.0 / ring.len() as f64) / TAU).round() * TAU;
            let shift_v = ((reference_mean.1 - center.1 / ring.len() as f64) / TAU).round() * TAU;
            for point in ring {
                point.0 += shift_u;
                point.1 += shift_v;
            }
        }
    }
    let Some((parameters, indices, sources)) =
        triangulate_parameters(&parameters, point_at, tolerance, Some(&boundaries))
    else {
        return;
    };
    for triangle in indices {
        emit(triangle.map(|index| {
            let uv = parameters[index];
            (
                uv.0,
                uv.1,
                sources[index].unwrap_or_else(|| point_at(uv.0, uv.1)),
            )
        }));
    }
}

fn unwrap_angle(angle: f64, previous: f64) -> f64 {
    previous + (angle - previous + PI).rem_euclid(TAU) - PI
}

/// Rotate a closed winding ring to a common seam, retaining its sample points.
fn periodic_parameter_ring(
    ring: &[(f64, f64)],
    points: &[Vector3],
    start: f64,
) -> Vec<(f64, f64, Vector3)> {
    let mut ring = ring
        .iter()
        .zip(points)
        .map(|(&(u, v), &point)| (start + (u - start).rem_euclid(TAU), v, point))
        .collect::<Vec<_>>();
    ring.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (first_u, first_v, first_point) = ring[0];
    let (last_u, last_v, last_point) = *ring.last().unwrap();
    let span = first_u + TAU - last_u;
    let t = if span > 1e-12 {
        (start + TAU - last_u) / span
    } else {
        0.0
    };
    let v = last_v + (first_v - last_v) * t;
    let point = last_point + (first_point - last_point) * t;
    let mut result = vec![(start, v, point)];
    result.extend(
        ring.into_iter()
            .filter(|&(u, _, _)| u > start + 1e-10 && u < start + TAU - 1e-10),
    );
    result.push((start + TAU, v, point));
    result
}

fn facet_trimmed_spline(
    patch: &SatBSplineSurface,
    parameters: &[Vec<(f64, f64)>],
    boundaries: &[Vec<Vector3>],
    sense: Sense,
    place: &Placement,
    tolerance: f64,
    tess: &mut Tessellation,
) {
    let u0 = knot_start(&patch.u_knots, patch.degree_u);
    let u1 = knot_end(&patch.u_knots, patch.degree_u);
    let v0 = knot_start(&patch.v_knots, patch.degree_v);
    let v1 = knot_end(&patch.v_knots, patch.degree_v);
    if u1 <= u0 || v1 <= v0 || parameters.len() != boundaries.len() {
        facet_spline(patch, sense, place, tolerance, tess);
        return;
    }
    // A malformed or periodic p-curve should never make the evaluator clamp
    // an entire trim to one side of the patch. Let the untrimmed fallback
    // handle such data at the caller when no usable p-curves are available.
    let in_domain = |(u, v): &(f64, f64)| {
        *u >= u0 - 1e-7 && *u <= u1 + 1e-7 && *v >= v0 - 1e-7 && *v <= v1 + 1e-7
    };
    if parameters
        .iter()
        .any(|ring| ring.len() < 3 || ring.iter().any(|uv| !in_domain(uv)))
    {
        facet_spline(patch, sense, place, tolerance, tess);
        return;
    }
    let at = |u: f64, v: f64| place.point(eval_spline(patch, u, v));
    let normal_at = |u: f64, v: f64| spline_normal_at(patch, u, v, u0, u1, v0, v1, sense, place);
    for group in parameter_regions(parameters) {
        let rings = group
            .iter()
            .map(|&index| parameters[index].clone())
            .collect::<Vec<_>>();
        let source_rings = group
            .iter()
            .map(|&index| boundaries[index].clone())
            .collect::<Vec<_>>();
        let Some((parameters, indices, sources)) =
            triangulate_parameters(&rings, at, tolerance, Some(&source_rings))
        else {
            continue;
        };
        for triangle in indices {
            let points = triangle.map(|index| {
                let (u, v) = parameters[index];
                sources[index].unwrap_or_else(|| at(u, v))
            });
            let normals = triangle.map(|index| {
                let (u, v) = parameters[index];
                normal_at(u, v)
            });
            push_triangle(
                tess, points[0], points[1], points[2], normals[0], normals[1], normals[2],
            );
        }
    }
}

fn spline_normal_at(
    patch: &SatBSplineSurface,
    u: f64,
    v: f64,
    u0: f64,
    u1: f64,
    v0: f64,
    v1: f64,
    sense: Sense,
    place: &Placement,
) -> Vector3 {
    // Use central differences in the interior and a one-sided difference at
    // the domain boundary. The old forward-only estimate collapsed to zero
    // on the final row/column, producing UNIT_Z fallback normals and visible
    // dark triangular fans on ruled spline faces.
    let du_step = ((u1 - u0) * 1e-4).max(1e-7);
    let dv_step = ((v1 - v0) * 1e-4).max(1e-7);
    let (u_left, u_right) = ((u - du_step).max(u0), (u + du_step).min(u1));
    let (v_left, v_right) = ((v - dv_step).max(v0), (v + dv_step).min(v1));
    let du = eval_spline(patch, u_right, v) - eval_spline(patch, u_left, v);
    let dv = eval_spline(patch, u, v_right) - eval_spline(patch, u, v_left);
    let mut normal = unit(place.normal(du.cross(&dv))).unwrap_or(Vector3::UNIT_Z);
    if sense == Sense::Reversed {
        normal = normal * -1.0;
    }
    normal
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
    let at = |u: f64, v: f64| place.point(eval_spline(patch, u, v));
    let normal_at = |u: f64, v: f64| spline_normal_at(patch, u, v, u0, u1, v0, v1, sense, place);
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
        let sweep = ellipse_edge_sweep(document, edge)?;
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

fn ellipse_edge_sweep(
    document: &SatDocument,
    edge: &crate::entities::acis::SatEdge<'_>,
) -> Option<f64> {
    let sweep = edge.end_param() - edge.start_param();
    if !sweep.is_finite() {
        return None;
    }
    if sweep.abs() >= 1e-8 {
        return Some(sweep);
    }
    let start = vertex_point(document, edge.start_vertex());
    let end = vertex_point(document, edge.end_vertex());
    if let (Some(start), Some(end)) = (start, end) {
        if (start - end).length_squared() > 1e-12 {
            return None;
        }
    }
    Some(TAU.copysign(if sweep < 0.0 { -1.0 } else { 1.0 }))
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
        let Some(sweep) = ellipse_edge_sweep(document, edge) else {
            return Vec::new();
        };
        if sweep.abs() >= TAU - 1e-8 {
            return sample_parameter_with_turn(t0, t0 + sweep, tolerance, 0.08, |parameter| {
                ellipse_point(&ellipse, parameter)
            });
        }
        return sample_parameter(t0, t0 + sweep, tolerance, |parameter| {
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
    sample_parameter_with_turn(start, end, tolerance, 0.25, eval)
}

fn sample_parameter_with_turn(
    start: f64,
    end: f64,
    tolerance: f64,
    max_turn: f64,
    eval: impl Fn(f64) -> Vector3,
) -> Vec<Vector3> {
    if !start.is_finite() || !end.is_finite() || (end - start).abs() <= 1e-15 {
        return vec![eval(start)];
    }
    let mut points = vec![eval(start)];
    subdivide(start, end, tolerance, max_turn, &eval, &mut points, 0);
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
    max_turn: f64,
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
    if sagitta > tolerance || turn > max_turn {
        subdivide(
            start,
            middle_parameter,
            tolerance,
            max_turn,
            eval,
            points,
            depth + 1,
        );
        points.push(middle);
        subdivide(
            middle_parameter,
            end,
            tolerance,
            max_turn,
            eval,
            points,
            depth + 1,
        );
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
            let extent = place
                .linear
                .m
                .map(|row| radius * row.iter().map(|value| value * value).sum::<f64>().sqrt());
            let extent = Vector3::new(extent[0], extent[1], extent[2]);
            grow(center + extent);
            grow(center - extent);
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
    fn torus_band_preserves_shared_boundary_chords() {
        let mut document = SatDocument::new_body();
        let index =
            document.add_torus_surface([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 4.0, 1.0, [1.0, 0.0, 0.0]);
        let torus = SatTorusSurface::from_record(document.record(index as usize).unwrap()).unwrap();
        // Different sampling densities and starting angles force added grid
        // columns and a seam point between existing samples on the second ring.
        let loops = [(0.2_f64, 17, 0.0), (1.0_f64, 23, 0.11)]
            .into_iter()
            .map(|(v, count, offset)| {
                (0..count)
                    .map(|index| {
                        let u = offset + TAU * index as f64 / count as f64;
                        Vector3::new(
                            (4.0 + v.cos()) * u.cos(),
                            (4.0 + v.cos()) * u.sin(),
                            v.sin(),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut tess = Tessellation::new();
        facet_trimmed_torus(
            &torus,
            &loops,
            Sense::Forward,
            &Placement::from_document(&document),
            0.005,
            &mut tess,
        );
        assert!(!tess.triangles.is_empty());
        for ring in &loops {
            let boundary = tess
                .triangles
                .iter()
                .filter(|p| (p.z - ring[0].z).abs() < 1e-10)
                .collect::<Vec<_>>();
            assert!(boundary.len() > ring.len());
            for point in boundary {
                assert!(
                    ring.iter().enumerate().any(|(index, &a)| {
                        let b = ring[(index + 1) % ring.len()];
                        let edge = b - a;
                        let t = ((*point - a).dot(&edge) / edge.length_squared()).clamp(0.0, 1.0);
                        (*point - a - edge * t).length() < 1e-10
                    }),
                    "fillet boundary left the shared edge chord: {point:?}"
                );
            }
            for point in ring {
                assert!(tess
                    .triangles
                    .iter()
                    .any(|p| (*p - *point).length() < 1e-10));
            }
        }
    }

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
            assert!(
                radius > 1.8,
                "triangle collapsed into the cylinder: {center:?}"
            );
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
    fn rotated_closed_cylinder_rings_are_monotonic_and_do_not_cross() {
        let frame = ConeFrame {
            center: Vector3::ZERO,
            axis: Vector3::UNIT_Z,
            major: Vector3::UNIT_X * 5.0,
            minor: Vector3::UNIT_Y * 5.0,
            sin_half: 0.0,
            cos_half: 1.0,
            radius: 5.0,
        };
        let ring = |offset: f64, count: usize, height: f64| {
            (0..count)
                .map(|index| frame.point_at(offset + TAU * index as f64 / count as f64, height))
                .collect::<Vec<_>>()
        };
        let mut tess = Tessellation::new();
        push_ruled(
            &ring(0.0, 24, 0.0),
            &ring(PI, 37, 10.0),
            &frame,
            false,
            &mut tess,
        );
        assert!(!tess.triangles.is_empty());
        for triangle in tess.triangles.chunks_exact(3) {
            let first = frame.angle(triangle[0]);
            let angles = triangle
                .iter()
                .map(|point| unwrap_angle(frame.angle(*point), first))
                .collect::<Vec<_>>();
            let covered = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - angles.iter().copied().fold(f64::INFINITY, f64::min);
            assert!(covered < PI, "crossed cylinder triangle: {triangle:?}");
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

    #[test]
    fn planar_face_accepts_holes_before_the_outer_loop() {
        // ACIS loop records do not guarantee that the exterior loop appears
        // first. Earcut requires that order in its flattened input.
        let hole = vec![
            Vector3::new(-1.0, -1.0, 0.0),
            Vector3::new(1.0, -1.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(-1.0, 1.0, 0.0),
        ];
        let outer = vec![
            Vector3::new(-5.0, -5.0, 0.0),
            Vector3::new(5.0, -5.0, 0.0),
            Vector3::new(5.0, 5.0, 0.0),
            Vector3::new(-5.0, 5.0, 0.0),
        ];
        let (triangles, normals) = triangulate_plane(&[hole, outer], Vector3::UNIT_Z);
        assert!(!triangles.is_empty());
        assert_eq!(triangles.len(), normals.len());
        assert!(triangles.chunks_exact(3).all(|triangle| {
            let center = (triangle[0] + triangle[1] + triangle[2]) / 3.0;
            center.x.abs() > 1.0 || center.y.abs() > 1.0
        }));
    }
}
