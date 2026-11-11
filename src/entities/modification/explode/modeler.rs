use super::*;
use crate::types::{Matrix4, Transform};

pub(super) fn sat_face_points(
    doc: &crate::entities::acis::SatDocument,
    face: &crate::entities::acis::SatFace<'_>,
) -> Vec<Vector3> {
    use crate::entities::acis::{SatCoedge, SatEdge, SatLoop, SatPoint, SatVertex};
    let Some(loop_record) = doc
        .resolve(face.first_loop())
        .and_then(SatLoop::from_record)
    else {
        return Vec::new();
    };
    let mut points = Vec::new();
    let mut pointer = loop_record.first_coedge();
    let mut seen = std::collections::HashSet::new();
    while !pointer.is_null() && seen.insert(pointer) {
        let Some(coedge) = doc.resolve(pointer).and_then(SatCoedge::from_record) else {
            break;
        };
        let Some(edge) = doc.resolve(coedge.edge()).and_then(SatEdge::from_record) else {
            break;
        };
        let Some(vertex) = doc
            .resolve(edge.start_vertex())
            .and_then(SatVertex::from_record)
        else {
            break;
        };
        let Some(point) = doc.resolve(vertex.point()).and_then(SatPoint::from_record) else {
            break;
        };
        let (x, y, z) = point.position();
        points.push(Vector3::new(x, y, z));
        pointer = coedge.next();
    }
    points
}

pub(super) fn modeler_regions(
    common: &EntityCommon,
    sat: Option<crate::entities::acis::SatDocument>,
) -> Vec<EntityType> {
    let Some(doc) = sat else { return Vec::new() };
    doc.faces()
        .into_iter()
        .filter_map(|face| {
            let points = sat_face_points(&doc, &face);
            let surface_type = doc
                .resolve(face.surface())
                .map(|record| record.entity_type.as_str())
                .unwrap_or("plane-surface");
            if surface_type.contains("plane") {
                (!points.is_empty()).then(|| {
                    let wire = Wire::from_points(points);
                    let mut region = Region::new();
                    region.common = inherit_common(common);
                    region.wires.push(wire);
                    EntityType::Region(region)
                })
            } else {
                // Analytic curved faces can have a one-vertex seam loop (for
                // example a cylindrical face). Preserve the face as a SURFACE
                // even when its detached polygon wire is degenerate.
                (!points.is_empty()).then(|| {
                    let wire = Wire::from_points(points);
                    let mut surface = Surface::new(SurfaceKind::Generic);
                    surface.common = inherit_common(common);
                    surface.wires.push(wire);
                    EntityType::Surface(Box::new(surface))
                })
            }
        })
        .collect()
}

pub(super) fn sat_point_entity(
    doc: &crate::entities::acis::SatDocument,
    pointer: crate::entities::acis::SatPointer,
) -> Option<Vector3> {
    use crate::entities::acis::{SatPoint, SatVertex};
    let vertex = doc.resolve(pointer).and_then(SatVertex::from_record)?;
    let point = doc
        .resolve(vertex.point())
        .and_then(SatPoint::from_record)?;
    let (x, y, z) = point.position();
    let point = Vector3::new(x, y, z);
    if point.x.is_finite() && point.y.is_finite() && point.z.is_finite() {
        Some(point)
    } else {
        None
    }
}

pub(super) fn sat_edge_entity(
    doc: &crate::entities::acis::SatDocument,
    coedge: &crate::entities::acis::SatCoedge<'_>,
    edge: &crate::entities::acis::SatEdge<'_>,
    common: &EntityCommon,
) -> Option<EntityType> {
    use crate::entities::acis::{SatEllipseCurve, SatIntCurve, SatStraightCurve, Sense};
    let start = sat_point_entity(doc, edge.start_vertex())?;
    let end = sat_point_entity(doc, edge.end_vertex())?;
    let reverse = coedge.sense() == Sense::Reversed;
    let (start, end) = if reverse { (end, start) } else { (start, end) };
    let curve = doc.resolve(edge.curve())?;

    if SatStraightCurve::from_record(curve).is_some() {
        let mut line = Line::from_points(start, end);
        line.common = inherit_common(common);
        return Some(EntityType::Line(line));
    }

    if let Some(curve) = SatEllipseCurve::from_record(curve) {
        let (cx, cy, cz) = curve.center();
        let (nx, ny, nz) = curve.normal();
        let (ax, ay, az) = curve.major_axis();
        let normal = Vector3::new(nx, ny, nz).normalize();
        let center = Vector3::new(cx, cy, cz);
        let major_axis = Vector3::new(ax, ay, az);
        let start_parameter = edge.start_param();
        let end_parameter = edge.end_param();
        let full_circle = (curve.ratio() - 1.0).abs() <= 1e-9
            && (end_parameter - start_parameter).abs() >= std::f64::consts::TAU - 1e-8;
        if full_circle {
            let mut circle = Circle::from_center_radius(center, major_axis.length());
            circle.common = inherit_common(common);
            circle.normal = normal;
            return Some(EntityType::Circle(circle));
        }
        let mut ellipse = Ellipse {
            common: inherit_common(common),
            center,
            major_axis,
            minor_axis_ratio: curve.ratio(),
            start_parameter,
            end_parameter,
            normal,
        };
        if reverse {
            std::mem::swap(&mut ellipse.start_parameter, &mut ellipse.end_parameter);
        }
        return Some(EntityType::Ellipse(ellipse));
    }

    if let Some(curve) = SatIntCurve::from_record(curve) {
        let (degree, knots, controls) = curve.bspline_in(doc)?;
        if controls.len() < 2 || knots.len() < 2 {
            return None;
        }
        let full_start = knots.get(degree).copied().unwrap_or(0.0);
        let full_end = knots
            .get(knots.len().saturating_sub(degree + 1))
            .copied()
            .unwrap_or(full_start);
        // A reversed intcurve runs against its B-spline: curve parameter t
        // is spline parameter -t, and the spline runs from the edge's end to
        // its start.
        let curve_reversed = curve.sense() == Sense::Reversed;
        let (edge_start, edge_end) = if curve_reversed {
            (-edge.end_param(), -edge.start_param())
        } else {
            (edge.start_param(), edge.end_param())
        };
        let reverse = reverse != curve_reversed;
        if (edge_start - full_start).abs() > 1e-9 || (edge_end - full_end).abs() > 1e-9 {
            let mut fit_points: Vec<Vector3> = curve
                .sample_range(edge_start, edge_end, 32)
                .into_iter()
                .map(|(x, y, z)| Vector3::new(x, y, z))
                .collect();
            if reverse {
                fit_points.reverse();
            }
            if fit_points.len() >= 2 {
                let mut spline = Spline::from_fit_points(fit_points);
                spline.common = inherit_common(common);
                return Some(EntityType::Spline(spline));
            }
            return None;
        }
        // The decoded control points are homogeneous (x·w, y·w, z·w, w).
        let mut spline = Spline::from_control_points(
            degree as i32,
            controls
                .iter()
                .map(|p| {
                    let w = if p[3].abs() > 1e-12 { p[3] } else { 1.0 };
                    Vector3::new(p[0] / w, p[1] / w, p[2] / w)
                })
                .collect(),
        );
        spline.common = inherit_common(common);
        spline.degree = degree as i32;
        spline.knots = knots;
        spline.weights = controls.iter().map(|p| p[3]).collect();
        spline.flags.rational = spline
            .weights
            .iter()
            .any(|weight| (*weight - 1.0).abs() > 1e-12);
        if reverse {
            spline.control_points.reverse();
            spline.weights.reverse();
            spline.knots.reverse();
            if let (Some(first), Some(last)) =
                (spline.knots.first().copied(), spline.knots.last().copied())
            {
                for knot in &mut spline.knots {
                    *knot = first + last - *knot;
                }
            }
        }
        return Some(EntityType::Spline(spline));
    }

    None
}

pub(super) fn sat_region_curves(
    common: &EntityCommon,
    doc: &crate::entities::acis::SatDocument,
    face: &crate::entities::acis::SatFace<'_>,
) -> Vec<EntityType> {
    use crate::entities::acis::{SatCoedge, SatEdge, SatLoop};
    let mut result = Vec::new();
    let mut loop_pointer = face.first_loop();
    let mut seen_loops = std::collections::HashSet::new();
    while !loop_pointer.is_null() && seen_loops.insert(loop_pointer) {
        let Some(loop_record) = doc.resolve(loop_pointer).and_then(SatLoop::from_record) else {
            break;
        };
        let mut coedge_pointer = loop_record.first_coedge();
        let mut seen_coedges = std::collections::HashSet::new();
        while !coedge_pointer.is_null() && seen_coedges.insert(coedge_pointer) {
            let Some(coedge) = doc.resolve(coedge_pointer).and_then(SatCoedge::from_record) else {
                break;
            };
            if let Some(edge) = doc.resolve(coedge.edge()).and_then(SatEdge::from_record) {
                if let Some(entity) = sat_edge_entity(doc, &coedge, &edge, common) {
                    result.push(entity);
                }
            }
            coedge_pointer = coedge.next();
        }
        loop_pointer = loop_record.next_loop();
    }
    result
}

/// The body's `transform` record as a world transform (`world = scale·(p·M)
/// + T`, row vectors), `None` when the body has none.
fn body_placement(doc: &crate::entities::acis::SatDocument) -> Option<Transform> {
    if !doc.records.iter().any(|r| r.entity_type == "transform") {
        return None;
    }
    let (m, t, s) = doc.placement();
    let mut matrix = Matrix4::identity();
    for i in 0..3 {
        for j in 0..3 {
            matrix.m[i][j] = s * m[j][i];
        }
        matrix.m[i][3] = t[i];
    }
    Some(Transform::from_matrix(matrix))
}

pub(super) fn explode_region(region: &Region) -> Vec<EntityType> {
    if region.wires.is_empty() {
        // SAT text or binary SAB: a DWG 2013+ stores every body as SAB.
        if let Some(doc) = region.acis_data.parse() {
            let mut result = Vec::new();
            for face in doc.faces() {
                result.extend(sat_region_curves(&region.common, &doc, &face));
            }
            if !result.is_empty() {
                if let Some(placement) = body_placement(&doc) {
                    for entity in &mut result {
                        entity.apply_transform(&placement);
                    }
                }
                return result;
            }
        }
    }
    let mut result = Vec::new();
    for wire in &region.wires {
        for pair in wire.points.windows(2) {
            result.push(line_entity(
                pair[0],
                pair[1],
                0.0,
                Vector3::UNIT_Z,
                &region.common,
            ));
        }
        if wire.points.len() > 2 {
            result.push(line_entity(
                *wire.points.last().unwrap(),
                wire.points[0],
                0.0,
                Vector3::UNIT_Z,
                &region.common,
            ));
        }
    }
    if result.is_empty() {
        return modeler_regions(&region.common, region.parse_sat())
            .into_iter()
            .flat_map(|entity| match entity {
                EntityType::Region(region) => explode_region(&region),
                _ => Vec::new(),
            })
            .collect();
    }
    result
}

pub(super) fn explode_solid3d(solid: &Solid3D) -> Vec<EntityType> {
    modeler_regions(&solid.common, solid.parse_sat())
}

pub(super) fn explode_body(body: &Body) -> Vec<EntityType> {
    modeler_regions(&body.common, body.parse_sat())
}

pub(super) fn explode_surface(surface: &Surface) -> Vec<EntityType> {
    if matches!(surface.kind, SurfaceKind::Plane) {
        modeler_regions(&surface.common, surface.parse_sat())
    } else {
        Vec::new()
    }
}
