//! Geometry and entity comparison utilities for tests.
//!
//! Provides tolerance-based f64/Vector3 assertions, per-entity-type geometry
//! comparison, and entity sort-key extraction.

#![allow(dead_code)]

use acadrust::entities::EntityType;
use acadrust::types::Vector3;
use acadrust::CadDocument;
use std::collections::BTreeMap;

/// Default tolerance for floating-point comparisons.
pub const TOL: f64 = 1e-6;

// ===========================================================================
// Scalar & point assertions
// ===========================================================================

/// Check approximate equality of two f64 values within `tol`.
pub fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

/// Assert two f64 values are approximately equal.
pub fn assert_f64_eq(a: f64, b: f64, tol: f64) {
    assert!(
        approx_eq(a, b, tol),
        "f64 mismatch: {a} vs {b} (delta={}, tol={tol})",
        (a - b).abs()
    );
}

/// Assert two Vector3 values are approximately equal component-wise.
pub fn assert_vec3_eq(a: &Vector3, b: &Vector3, tol: f64) {
    assert!(
        approx_eq(a.x, b.x, tol) && approx_eq(a.y, b.y, tol) && approx_eq(a.z, b.z, tol),
        "Vector3 mismatch: ({},{},{}) vs ({},{},{}) tol={tol}",
        a.x, a.y, a.z, b.x, b.y, b.z
    );
}

// ===========================================================================
// Diff-based comparison helpers
// ===========================================================================

/// Append a diff message if two f64 values differ beyond tolerance.
pub fn check_f64(diffs: &mut Vec<String>, name: &str, a: f64, b: f64) {
    if !approx_eq(a, b, TOL) {
        diffs.push(format!("{name}: {a} vs {b}"));
    }
}

/// Append a diff message if two Vector3 values differ beyond tolerance.
pub fn check_vec3(diffs: &mut Vec<String>, name: &str, a: &Vector3, b: &Vector3) {
    if !approx_eq(a.x, b.x, TOL) || !approx_eq(a.y, b.y, TOL) || !approx_eq(a.z, b.z, TOL) {
        diffs.push(format!(
            "{name}: ({},{},{}) vs ({},{},{})",
            a.x, a.y, a.z, b.x, b.y, b.z
        ));
    }
}

// ===========================================================================
// Entity sort key — for aligning entity lists from different sources
// ===========================================================================

/// Compute a rough geometric sort key for an entity so that entities from
/// two independently-parsed documents can be paired up for comparison.
pub fn entity_sort_key(e: &EntityType) -> (f64, f64, f64) {
    match e {
        EntityType::Line(l) => (l.start.x, l.start.y, l.start.z),
        EntityType::Circle(c) => (c.center.x, c.center.y, c.center.z),
        EntityType::Arc(a) => (a.center.x, a.center.y, a.center.z),
        EntityType::Ellipse(el) => (el.center.x, el.center.y, el.center.z),
        EntityType::Point(p) => (p.location.x, p.location.y, p.location.z),
        EntityType::Text(t) => (t.insertion_point.x, t.insertion_point.y, t.insertion_point.z),
        EntityType::MText(t) => (t.insertion_point.x, t.insertion_point.y, t.insertion_point.z),
        EntityType::LwPolyline(lw) => {
            if let Some(v) = lw.vertices.first() {
                (v.location.x, v.location.y, lw.elevation)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        EntityType::Spline(s) => {
            if let Some(cp) = s.control_points.first() {
                (cp.x, cp.y, cp.z)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        EntityType::Insert(ins) => (ins.insert_point.x, ins.insert_point.y, ins.insert_point.z),
        EntityType::Ray(r) => (r.base_point.x, r.base_point.y, r.base_point.z),
        EntityType::XLine(xl) => (xl.base_point.x, xl.base_point.y, xl.base_point.z),
        EntityType::Solid(s) => (s.first_corner.x, s.first_corner.y, s.first_corner.z),
        EntityType::Face3D(f) => (f.first_corner.x, f.first_corner.y, f.first_corner.z),
        EntityType::Hatch(h) => (h.elevation, h.pattern_angle, h.pattern_scale),
        EntityType::Leader(l) => {
            if let Some(v) = l.vertices.first() {
                (v.x, v.y, v.z)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        EntityType::Tolerance(t) => {
            (t.insertion_point.x, t.insertion_point.y, t.insertion_point.z)
        }
        EntityType::Shape(s) => (s.insertion_point.x, s.insertion_point.y, s.insertion_point.z),
        EntityType::Viewport(vp) => (vp.center.x, vp.center.y, vp.center.z),
        EntityType::Dimension(d) => {
            let b = d.base();
            (b.definition_point.x, b.definition_point.y, b.definition_point.z)
        }
        _ => {
            let c = e.common();
            (c.handle.value() as f64, 0.0, 0.0)
        }
    }
}

/// Group entities by type name, sorted by geometric key within each group.
pub fn sorted_entities_by_type(doc: &CadDocument) -> BTreeMap<&'static str, Vec<&EntityType>> {
    let mut groups: BTreeMap<&'static str, Vec<&EntityType>> = BTreeMap::new();
    for e in doc.entities() {
        groups
            .entry(super::entity_type_name(e))
            .or_default()
            .push(e);
    }
    for entities in groups.values_mut() {
        entities.sort_by(|a, b| {
            entity_sort_key(a)
                .partial_cmp(&entity_sort_key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    groups
}

// ===========================================================================
// Per-entity-type geometry comparison
// ===========================================================================

/// Compare two entities of the same type. Returns a list of mismatch descriptions.
/// Empty vec = entities match.
pub fn compare_entity_geometry(a: &EntityType, b: &EntityType) -> Vec<String> {
    let mut diffs = Vec::new();

    // Layer should match
    let layer_a = &a.common().layer;
    let layer_b = &b.common().layer;
    if layer_a != layer_b {
        diffs.push(format!("layer: {layer_a:?} vs {layer_b:?}"));
    }

    match (a, b) {
        (EntityType::Line(ea), EntityType::Line(eb)) => {
            check_vec3(&mut diffs, "start", &ea.start, &eb.start);
            check_vec3(&mut diffs, "end", &ea.end, &eb.end);
        }
        (EntityType::Circle(ea), EntityType::Circle(eb)) => {
            check_vec3(&mut diffs, "center", &ea.center, &eb.center);
            check_f64(&mut diffs, "radius", ea.radius, eb.radius);
        }
        (EntityType::Arc(ea), EntityType::Arc(eb)) => {
            check_vec3(&mut diffs, "center", &ea.center, &eb.center);
            check_f64(&mut diffs, "radius", ea.radius, eb.radius);
            check_f64(&mut diffs, "start_angle", ea.start_angle, eb.start_angle);
            check_f64(&mut diffs, "end_angle", ea.end_angle, eb.end_angle);
        }
        (EntityType::Ellipse(ea), EntityType::Ellipse(eb)) => {
            check_vec3(&mut diffs, "center", &ea.center, &eb.center);
            check_vec3(&mut diffs, "major_axis", &ea.major_axis, &eb.major_axis);
            check_f64(&mut diffs, "minor_axis_ratio", ea.minor_axis_ratio, eb.minor_axis_ratio);
            check_f64(&mut diffs, "start_parameter", ea.start_parameter, eb.start_parameter);
            check_f64(&mut diffs, "end_parameter", ea.end_parameter, eb.end_parameter);
        }
        (EntityType::Point(ea), EntityType::Point(eb)) => {
            check_vec3(&mut diffs, "location", &ea.location, &eb.location);
        }
        (EntityType::Text(ea), EntityType::Text(eb)) => {
            check_vec3(&mut diffs, "insertion_point", &ea.insertion_point, &eb.insertion_point);
            check_f64(&mut diffs, "height", ea.height, eb.height);
            if ea.value != eb.value {
                diffs.push(format!("value: {:?} vs {:?}", ea.value, eb.value));
            }
        }
        (EntityType::MText(ea), EntityType::MText(eb)) => {
            check_vec3(&mut diffs, "insertion_point", &ea.insertion_point, &eb.insertion_point);
            check_f64(&mut diffs, "height", ea.height, eb.height);
            if ea.value != eb.value {
                diffs.push(format!("value: {:?} vs {:?}", ea.value, eb.value));
            }
        }
        (EntityType::LwPolyline(ea), EntityType::LwPolyline(eb)) => {
            if ea.vertices.len() != eb.vertices.len() {
                diffs.push(format!(
                    "vertex_count: {} vs {}",
                    ea.vertices.len(),
                    eb.vertices.len()
                ));
            } else {
                for (i, (va, vb)) in ea.vertices.iter().zip(eb.vertices.iter()).enumerate() {
                    if !approx_eq(va.location.x, vb.location.x, TOL)
                        || !approx_eq(va.location.y, vb.location.y, TOL)
                    {
                        diffs.push(format!(
                            "vertex[{i}]: ({},{}) vs ({},{})",
                            va.location.x, va.location.y, vb.location.x, vb.location.y
                        ));
                    }
                    if !approx_eq(va.bulge, vb.bulge, TOL) {
                        diffs.push(format!("vertex[{i}].bulge: {} vs {}", va.bulge, vb.bulge));
                    }
                }
            }
            if ea.is_closed != eb.is_closed {
                diffs.push(format!("is_closed: {} vs {}", ea.is_closed, eb.is_closed));
            }
        }
        (EntityType::Spline(ea), EntityType::Spline(eb)) => {
            if ea.degree != eb.degree {
                diffs.push(format!("degree: {} vs {}", ea.degree, eb.degree));
            }
            if ea.control_points.len() != eb.control_points.len() {
                diffs.push(format!(
                    "control_points_count: {} vs {}",
                    ea.control_points.len(),
                    eb.control_points.len()
                ));
            } else {
                for (i, (pa, pb)) in ea.control_points.iter().zip(eb.control_points.iter()).enumerate() {
                    check_vec3(&mut diffs, &format!("control_point[{i}]"), pa, pb);
                }
            }
            if ea.knots.len() != eb.knots.len() {
                diffs.push(format!(
                    "knots_count: {} vs {}",
                    ea.knots.len(),
                    eb.knots.len()
                ));
            }
        }
        (EntityType::Insert(ea), EntityType::Insert(eb)) => {
            if ea.block_name != eb.block_name {
                diffs.push(format!("block_name: {:?} vs {:?}", ea.block_name, eb.block_name));
            }
            check_vec3(&mut diffs, "insert_point", &ea.insert_point, &eb.insert_point);
            check_f64(&mut diffs, "x_scale", ea.x_scale, eb.x_scale);
            check_f64(&mut diffs, "y_scale", ea.y_scale, eb.y_scale);
            check_f64(&mut diffs, "z_scale", ea.z_scale, eb.z_scale);
            check_f64(&mut diffs, "rotation", ea.rotation, eb.rotation);
        }
        (EntityType::Ray(ea), EntityType::Ray(eb)) => {
            check_vec3(&mut diffs, "base_point", &ea.base_point, &eb.base_point);
            check_vec3(&mut diffs, "direction", &ea.direction, &eb.direction);
        }
        (EntityType::XLine(ea), EntityType::XLine(eb)) => {
            check_vec3(&mut diffs, "base_point", &ea.base_point, &eb.base_point);
            check_vec3(&mut diffs, "direction", &ea.direction, &eb.direction);
        }
        (EntityType::Solid(ea), EntityType::Solid(eb)) => {
            check_vec3(&mut diffs, "first_corner", &ea.first_corner, &eb.first_corner);
            check_vec3(&mut diffs, "second_corner", &ea.second_corner, &eb.second_corner);
            check_vec3(&mut diffs, "third_corner", &ea.third_corner, &eb.third_corner);
            check_vec3(&mut diffs, "fourth_corner", &ea.fourth_corner, &eb.fourth_corner);
        }
        (EntityType::Face3D(ea), EntityType::Face3D(eb)) => {
            check_vec3(&mut diffs, "first_corner", &ea.first_corner, &eb.first_corner);
            check_vec3(&mut diffs, "second_corner", &ea.second_corner, &eb.second_corner);
            check_vec3(&mut diffs, "third_corner", &ea.third_corner, &eb.third_corner);
            check_vec3(&mut diffs, "fourth_corner", &ea.fourth_corner, &eb.fourth_corner);
        }
        (EntityType::Hatch(ea), EntityType::Hatch(eb)) => {
            if ea.pattern.name != eb.pattern.name {
                diffs.push(format!("pattern_name: {:?} vs {:?}", ea.pattern.name, eb.pattern.name));
            }
            if ea.is_solid != eb.is_solid {
                diffs.push(format!("is_solid: {} vs {}", ea.is_solid, eb.is_solid));
            }
            check_f64(&mut diffs, "pattern_scale", ea.pattern_scale, eb.pattern_scale);
        }
        (EntityType::Leader(ea), EntityType::Leader(eb)) => {
            if ea.vertices.len() != eb.vertices.len() {
                diffs.push(format!(
                    "vertex_count: {} vs {}",
                    ea.vertices.len(),
                    eb.vertices.len()
                ));
            } else {
                for (i, (va, vb)) in ea.vertices.iter().zip(eb.vertices.iter()).enumerate() {
                    check_vec3(&mut diffs, &format!("vertex[{i}]"), va, vb);
                }
            }
        }
        (EntityType::Tolerance(ea), EntityType::Tolerance(eb)) => {
            check_vec3(&mut diffs, "insertion_point", &ea.insertion_point, &eb.insertion_point);
            if ea.text != eb.text {
                diffs.push(format!("text: {:?} vs {:?}", ea.text, eb.text));
            }
        }
        (EntityType::Shape(ea), EntityType::Shape(eb)) => {
            check_vec3(&mut diffs, "insertion_point", &ea.insertion_point, &eb.insertion_point);
            check_f64(&mut diffs, "size", ea.size, eb.size);
        }
        (EntityType::Viewport(ea), EntityType::Viewport(eb)) => {
            check_vec3(&mut diffs, "center", &ea.center, &eb.center);
            check_f64(&mut diffs, "width", ea.width, eb.width);
            check_f64(&mut diffs, "height", ea.height, eb.height);
        }
        _ => {}
    }
    diffs
}
