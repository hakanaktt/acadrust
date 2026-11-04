//! Centralized `apply_transform` implementations for all entity types.
//!
//! Each entity's transformation logic is defined here as a standalone
//! function, and [`EntityType`] exposes a convenience dispatch method.

use crate::entities::solid3d::AcisData;
use crate::entities::*;
use crate::types::{Matrix3, Transform, Vector2, Vector3};

/// Compose `transform` into an ACIS body's placement (the SAT `transform`
/// record) so the solid's actual geometry moves/rotates/scales, instead of
/// only nudging a reference point. ACIS keeps geometry in body-local space
/// and a `transform` record places it into the world as `world = scale·(p·M)
/// + T`; we fold the incoming world transform into that placement and bake
/// the result into a single 3×3 (scale = 1), which the tessellator applies
/// verbatim. Binary SAB is decoded, composed, and rewritten as text SAT
/// (the SAB writer is not lossless for this edit). A parse failure leaves
/// the data unchanged.
pub(crate) fn compose_acis_placement(acis: &mut AcisData, transform: &Transform) {
    let Some(mut doc) = acis.parse() else {
        return;
    };
    let (m, t, s) = doc.placement();
    // Existing placement as a column-vector linear map L (world = L·p + T):
    //   world = s·(p·M) + T  ⇒  L[i][j] = s·M[j][i].
    let mut l = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            l[i][j] = s * m[j][i];
        }
    }
    // Incoming transform: rotation/scale R and translation tr (column form).
    let mm = &transform.matrix.m;
    let r = [
        [mm[0][0], mm[0][1], mm[0][2]],
        [mm[1][0], mm[1][1], mm[1][2]],
        [mm[2][0], mm[2][1], mm[2][2]],
    ];
    let tr_in = [mm[0][3], mm[1][3], mm[2][3]];
    // L' = R·L
    let mut lp = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            lp[i][j] = r[i][0] * l[0][j] + r[i][1] * l[1][j] + r[i][2] * l[2][j];
        }
    }
    // T' = R·T + tr_in
    let tp = [
        r[0][0] * t[0] + r[0][1] * t[1] + r[0][2] * t[2] + tr_in[0],
        r[1][0] * t[0] + r[1][1] * t[1] + r[1][2] * t[2] + tr_in[1],
        r[2][0] * t[0] + r[2][1] * t[1] + r[2][2] * t[2] + tr_in[2],
    ];
    // Re-encode with scale = 1 and M_out[r][c] = L'[c][r], so that
    // `p·M_out` reproduces `L'·p`.
    let m_out = [
        [lp[0][0], lp[1][0], lp[2][0]],
        [lp[0][1], lp[1][1], lp[2][1]],
        [lp[0][2], lp[1][2], lp[2][2]],
    ];
    doc.set_placement(m_out, tp, 1.0);
    // Keep binary token types and modeler metadata when changing placement.
    // A SAB -> SAT -> SAB conversion can lose typed native fields.
    if acis.is_binary {
        acis.sab_data = crate::entities::acis::SabWriter::write(&doc);
        acis.sat_data.clear();
    } else {
        acis.sat_data = AcisData::strip_sat_terminator(&doc.to_sat_string());
    }
}

/// True when `transform` reverses orientation (negative upper-3×3
/// determinant — an odd number of mirrors). Plane-curve direction data
/// (polyline bulges, hatch boundary arc flags) must flip with it.
pub(crate) fn is_reflecting(transform: &Transform) -> bool {
    let m = transform.matrix.m;
    Matrix3::from_rows(
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    )
    .determinant()
        < 0.0
}

// ── Point ────────────────────────────────────────────────────────────────────

/// OCS→WCS and WCS→(new)OCS mappers for a planar entity whose stored
/// coordinates live in the OCS of `old_normal`. Mirrors `transform_insert`:
/// map to world, transform there, map back into the transformed normal's
/// frame. Both are the identity for the default +Z normal.
fn ocs_pair(old_normal: Vector3, new_normal: Vector3) -> (Matrix3, Matrix3) {
    (
        Matrix3::arbitrary_axis(old_normal),
        Matrix3::arbitrary_axis(new_normal).transpose(),
    )
}

pub(crate) fn transform_point(e: &mut Point, transform: &Transform) {
    e.common.raw_record = None;
    let old_normal = e.normal;
    e.location = transform.apply(e.location);
    e.normal = transform.apply_rotation(e.normal).normalize();
    rotate_ocs_angle(&mut e.x_axis_angle, old_normal, transform);
}

// ── Line ─────────────────────────────────────────────────────────────────────

pub(crate) fn transform_line(e: &mut Line, transform: &Transform) {
    e.common.raw_record = None;
    e.start = transform.apply(e.start);
    e.end = transform.apply(e.end);
    e.normal = transform.apply_rotation(e.normal).normalize();
}

// ── Circle ───────────────────────────────────────────────────────────────────

pub(crate) fn transform_circle(e: &mut Circle, transform: &Transform) {
    e.common.raw_record = None;
    // `center` is stored in OCS — transform in world, store back in the new
    // normal's OCS (see `ocs_pair`).
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    e.center = new_t * transform.apply(ocs * e.center);

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.radius *= scale_factor;

    e.normal = new_normal;
}

// ── Arc ──────────────────────────────────────────────────────────────────────

pub(crate) fn transform_arc(e: &mut Arc, transform: &Transform) {
    e.common.raw_record = None;
    // The start/end angles are measured in the arc's plane, so a rotation or
    // reflection must move them too — transform the actual endpoints and
    // recompute the angles in the new plane. (Translating alone leaves the
    // angles unchanged, as before.)
    let ocs = Matrix3::arbitrary_axis(e.normal);
    // `center` is stored in OCS — lift it (and the endpoints derived from it)
    // to world before transforming.
    let center_w = ocs * e.center;
    let endpoint = |ang: f64| {
        let local = Vector3::new(e.radius * ang.cos(), e.radius * ang.sin(), 0.0);
        center_w + ocs * local
    };
    let ws = transform.apply(endpoint(e.start_angle));
    let we = transform.apply(endpoint(e.end_angle));

    let new_center = transform.apply(center_w);
    let scale_factor = transform
        .apply_rotation(Vector3::new(1.0, 0.0, 0.0))
        .length();
    let new_normal = transform.apply_rotation(e.normal).normalize();

    let new_ocs_t = Matrix3::arbitrary_axis(new_normal).transpose();
    let angle_of = |w: Vector3| {
        let l = new_ocs_t * (w - new_center);
        l.y.atan2(l.x)
    };
    let (a_start, a_end) = (angle_of(ws), angle_of(we));

    // An arc is stored CCW from start→end. A reflection reverses orientation,
    // so the reflected CCW arc traces CW — swap the endpoints to keep it CCW.
    if is_reflecting(transform) {
        e.start_angle = a_end;
        e.end_angle = a_start;
    } else {
        e.start_angle = a_start;
        e.end_angle = a_end;
    }
    // Store the world-space centre back in the NEW normal's OCS.
    e.center = new_ocs_t * new_center;
    e.radius *= scale_factor;
    e.normal = new_normal;
}

// ── Ellipse ──────────────────────────────────────────────────────────────────

pub(crate) fn transform_ellipse(e: &mut Ellipse, transform: &Transform) {
    e.common.raw_record = None;
    use std::f64::consts::TAU;
    let fallback = |e: &mut Ellipse| {
        e.center = transform.apply(e.center);
        e.major_axis = transform.apply_rotation(e.major_axis);
        e.normal = transform.apply_rotation(e.normal).normalize();
    };
    let normal_len = e.normal.length();
    if normal_len <= 1e-12 || e.major_axis.length() <= 1e-12 {
        return fallback(e);
    }
    let minor = (e.normal * (1.0 / normal_len)).cross(&e.major_axis) * e.minor_axis_ratio;
    let major0 = transform.apply_rotation(e.major_axis);
    let minor0 = transform.apply_rotation(minor);
    let theta = 0.5 * (2.0 * major0.dot(&minor0)).atan2(major0.dot(&major0) - minor0.dot(&minor0));
    let (sin, cos) = theta.sin_cos();
    let major = major0 * cos + minor0 * sin;
    let minor = major0 * (-sin) + minor0 * cos;
    let cross = major.cross(&minor);
    let (major_len, cross_len) = (major.length(), cross.length());
    if major_len <= 1e-12 || cross_len <= 1e-12 * major_len * major_len {
        return fallback(e);
    }
    let full = e.is_full();
    e.center = transform.apply(e.center);
    e.major_axis = major;
    e.normal = cross * (1.0 / cross_len);
    e.minor_axis_ratio = (minor.length() / major_len).min(1.0);
    if !full {
        let sweep = (e.end_parameter - e.start_parameter).rem_euclid(TAU);
        e.start_parameter = (e.start_parameter - theta).rem_euclid(TAU);
        e.end_parameter = e.start_parameter + if sweep <= 1e-12 { TAU } else { sweep };
    }
}

// ── Polyline (3D heavy) ──────────────────────────────────────────────────────

pub(crate) fn transform_polyline(e: &mut Polyline, transform: &Transform) {
    e.common.raw_record = None;
    for vertex in &mut e.vertices {
        vertex.location = transform.apply(vertex.location);
    }
}

// ── Polyline2D ───────────────────────────────────────────────────────────────

pub(crate) fn transform_polyline2d(e: &mut Polyline2D, transform: &Transform) {
    e.common.raw_record = None;
    let flip = is_reflecting(transform);
    // Vertices are stored in OCS — transform in world, store back in the new
    // normal's OCS.
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    // Widths are lengths in the plane: SCALE 2 makes the band twice as wide.
    let scale_factor = transform
        .apply_rotation(Vector3::new(1.0, 0.0, 0.0))
        .length();
    e.start_width *= scale_factor;
    e.end_width *= scale_factor;
    for vertex in &mut e.vertices {
        vertex.location = new_t * transform.apply(ocs * vertex.location);
        vertex.start_width *= scale_factor;
        vertex.end_width *= scale_factor;
        if flip {
            // Bulge encodes the arc's side/direction in the plane; a
            // reflection reverses it.
            vertex.bulge = -vertex.bulge;
        }
    }
    e.normal = new_normal;
}

// ── Polyline3D ───────────────────────────────────────────────────────────────

pub(crate) fn transform_polyline3d(e: &mut Polyline3D, transform: &Transform) {
    e.common.raw_record = None;
    for v in &mut e.vertices {
        v.position = transform.apply(v.position);
    }
}

// ── LwPolyline ───────────────────────────────────────────────────────────────

pub(crate) fn transform_lwpolyline(e: &mut LwPolyline, transform: &Transform) {
    e.common.raw_record = None;
    let flip = is_reflecting(transform);
    // Vertices + elevation are stored in OCS — transform in world, store back
    // in the new normal's OCS.
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    // All vertices lie in one OCS plane.  Compute the transformed plane
    // elevation from its origin once; assigning `l.z` per vertex makes a
    // tilted non-uniformly scaled polyline drift as each vertex overwrites
    // the shared elevation field.
    let plane_origin = new_t * transform.apply(ocs * Vector3::new(0.0, 0.0, e.elevation));
    let new_elevation = plane_origin.z;
    // Widths are lengths in the plane: SCALE 2 makes the band twice as wide.
    let scale_factor = transform
        .apply_rotation(Vector3::new(1.0, 0.0, 0.0))
        .length();
    e.constant_width *= scale_factor;
    for vertex in &mut e.vertices {
        let pt3d = Vector3::new(vertex.location.x, vertex.location.y, e.elevation);
        let l = new_t * transform.apply(ocs * pt3d);
        vertex.location.x = l.x;
        vertex.location.y = l.y;
        vertex.start_width *= scale_factor;
        vertex.end_width *= scale_factor;
        if flip {
            // Bulge encodes the arc's side/direction in the plane; a
            // reflection reverses it. Without this, exploding a mirrored
            // INSERT bows every bulged segment to the wrong side.
            vertex.bulge = -vertex.bulge;
        }
    }
    e.elevation = new_elevation;
    e.normal = new_normal;
}

// ── Text ─────────────────────────────────────────────────────────────────────

/// Carry a stored orientation angle through a transform.
///
/// TEXT, MTEXT, SHAPE and the ATTRIB pair keep which way they face as a single
/// angle rather than as geometry, so transforming their points alone leaves the
/// glyphs pointing the old way — ROTATE put the text in the right place and
/// still drew it horizontal. Turn the angle into the direction it stands for,
/// send that through the transform, and read the angle back. `atan2` ignores
/// any scale the transform also carries, and a reflection lands on the mirrored
/// angle, which is what the mirror paths used to work out separately.
///
/// A transform that flattens the direction away leaves the angle alone rather
/// than snapping it to zero through `atan2(0, 0)`.
pub(crate) fn rotate_stored_angle(rotation: &mut f64, transform: &Transform) {
    let direction = Vector3::new(rotation.cos(), rotation.sin(), 0.0);
    let rotated = transform.apply_rotation(direction);
    if rotated.x != 0.0 || rotated.y != 0.0 {
        *rotation = rotated.y.atan2(rotated.x);
    }
}

/// Update an angle measured in a planar entity's object coordinate system.
pub(crate) fn rotate_ocs_angle(rotation: &mut f64, old_normal: Vector3, transform: &Transform) {
    let old_ocs = Matrix3::arbitrary_axis(old_normal);
    let new_normal = transform.apply_rotation(old_normal).normalize();
    let new_wcs_to_ocs = Matrix3::arbitrary_axis(new_normal).transpose();
    let direction = old_ocs * Vector3::new(rotation.cos(), rotation.sin(), 0.0);
    let transformed = new_wcs_to_ocs * transform.apply_rotation(direction);
    if transformed.x != 0.0 || transformed.y != 0.0 {
        *rotation = transformed.y.atan2(transformed.x);
    }
}

pub(crate) fn transform_text(e: &mut Text, transform: &Transform) {
    e.common.raw_record = None;
    // Insertion / alignment points are stored in OCS.
    let old_normal = e.normal;
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    e.insertion_point = new_t * transform.apply(ocs * e.insertion_point);
    if let Some(ref mut align) = e.alignment_point {
        *align = new_t * transform.apply(ocs * *align);
    }
    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.height *= scale_factor;
    e.normal = new_normal;
    rotate_ocs_angle(&mut e.rotation, old_normal, transform);
}

// ── MText ────────────────────────────────────────────────────────────────────

pub(crate) fn mtext_x_direction(e: &MText) -> Vector3 {
    e.dwg_x_direction
        .filter(|direction| {
            let angle = direction.y.atan2(direction.x);
            let delta = (angle - e.rotation + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            // A vertical direction has no defined angle in the XY plane.
            direction.x.hypot(direction.y) <= 1e-12 || delta.abs() < 1e-12
        })
        .unwrap_or_else(|| Vector3::new(e.rotation.cos(), e.rotation.sin(), 0.0))
}

pub(crate) fn transform_mtext(e: &mut MText, transform: &Transform) {
    e.common.raw_record = None;
    let old_normal = e.normal;
    let old_direction = mtext_x_direction(e);
    e.insertion_point = transform.apply(e.insertion_point);
    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.height *= scale_factor;
    e.rectangle_width *= scale_factor;
    if let Some(ref mut h) = e.rectangle_height {
        *h *= scale_factor;
    }
    let new_normal = transform.apply_rotation(old_normal).normalize();
    // The x-axis direction is a unit vector: AutoCAD's AUDIT rejects an MTEXT
    // whose direction carries the scale factor.
    let mut direction = transform.apply_rotation(old_direction);
    if direction.length() > 1e-12 {
        direction = direction.normalize();
    }
    e.normal = new_normal;
    e.dwg_x_direction = Some(direction);
    if direction.x.abs() > 1e-12 || direction.y.abs() > 1e-12 {
        e.rotation = direction.y.atan2(direction.x);
    } else {
        e.rotation = 0.0;
    }
}

// ── Spline ───────────────────────────────────────────────────────────────────

pub(crate) fn transform_spline(e: &mut Spline, transform: &Transform) {
    e.common.raw_record = None;
    for point in &mut e.control_points {
        *point = transform.apply(*point);
    }
    for point in &mut e.fit_points {
        *point = transform.apply(*point);
    }
    e.begin_tangent = transform.apply_rotation(e.begin_tangent);
    e.end_tangent = transform.apply_rotation(e.end_tangent);

    let axes = [Vector3::UNIT_X, Vector3::UNIT_Y, Vector3::UNIT_Z]
        .map(|axis| transform.apply_rotation(axis));
    let max_scale = axes.iter().map(|axis| axis.length()).fold(0.0, f64::max);
    let orthogonal = [(0, 1), (0, 2), (1, 2)]
        .into_iter()
        .all(|(left, right)| axes[left].dot(&axes[right]).abs() <= 1e-12 * max_scale.powi(2));
    // Exact for orthogonal axes; conservative for sheared transforms.
    let distance_scale = if orthogonal {
        max_scale
    } else {
        axes.iter()
            .map(|axis| axis.length_squared())
            .sum::<f64>()
            .sqrt()
    };
    e.control_tolerance *= distance_scale;
    e.fit_tolerance *= distance_scale;
    e.normal = transform_normal(transform, e.normal);
}

// ── Helix ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_helix(e: &mut Helix, transform: &Transform) {
    e.common.raw_record = None;
    let old_axis = e.axis_vector.normalize();
    let old_base = e.axis_base_point;
    let old_start = e.start_point;
    let transformed_axis = transform.apply_rotation(old_axis);
    let transformed_radial = transform.apply_rotation(old_start - old_base);
    let transformed_base = transform.apply(old_base);
    let transformed_start = transform.apply(old_start);

    transform_spline(&mut e.spline, transform);
    e.axis_base_point = transformed_base;
    e.start_point = transformed_start;
    e.axis_vector = transformed_axis.normalize();
    e.radius = transformed_radial.length();
    e.turn_height *= transformed_axis.length();
    if is_reflecting(transform) {
        e.handedness = !e.handedness;
    }
}

// ── Dimension ────────────────────────────────────────────────────────────────

fn transform_dimension_angle(
    old_normal: Vector3,
    new_normal: Vector3,
    angle: f64,
    transform: &Transform,
) -> f64 {
    let old_direction =
        Matrix3::arbitrary_axis(old_normal) * Vector3::new(angle.cos(), angle.sin(), 0.0);
    let new_direction =
        Matrix3::arbitrary_axis(new_normal).transpose() * transform.apply_rotation(old_direction);
    if new_direction.length_squared() > 1e-24 {
        new_direction.y.atan2(new_direction.x)
    } else {
        angle
    }
}

pub(crate) fn transform_dimension(e: &mut Dimension, transform: &Transform) {
    e.base_mut().common.raw_record = None;
    if let Dimension::Ordinate(d) = e {
        let old_normal = d.base.normal;
        let old_basis = Matrix3::arbitrary_axis(old_normal);
        let old_axis_angle = -d.base.horizontal_direction;
        let old_axis = old_basis * Vector3::new(old_axis_angle.cos(), old_axis_angle.sin(), 0.0);
        let old_text_angle = old_axis_angle + d.base.text_rotation;
        let old_text_axis =
            old_basis * Vector3::new(old_text_angle.cos(), old_text_angle.sin(), 0.0);

        d.definition_point = transform.apply(d.definition_point);
        d.feature_location = transform.apply(d.feature_location);
        d.leader_endpoint = transform.apply(d.leader_endpoint);
        d.base.definition_point = d.definition_point;
        d.base.text_middle_point = transform.apply(d.base.text_middle_point);

        let new_normal = transform_normal(transform, old_normal);
        let (old_ocs, new_wcs_to_ocs) = ocs_pair(old_normal, new_normal);
        d.base.insertion_point = new_wcs_to_ocs * transform.apply(old_ocs * d.base.insertion_point);
        d.base.normal = new_normal;

        let transformed_axis = new_wcs_to_ocs * transform.apply_rotation(old_axis);
        let mut new_axis_angle = old_axis_angle;
        if transformed_axis.length_squared() > 1e-24 {
            new_axis_angle = transformed_axis.y.atan2(transformed_axis.x);
            d.base.horizontal_direction = -new_axis_angle;
        }
        let transformed_text_axis = new_wcs_to_ocs * transform.apply_rotation(old_text_axis);
        if transformed_text_axis.length_squared() > 1e-24 {
            let relative = transformed_text_axis.y.atan2(transformed_text_axis.x) - new_axis_angle;
            d.base.text_rotation = relative.sin().atan2(relative.cos());
        }
        d.refresh_measurement();
        return;
    }

    let old_normal = e.base().normal;
    let new_normal = transform_normal(transform, old_normal);
    // A mirror moves a dimension but never mirrors its text: the text keeps
    // the direction it reads in (0 still meaning along the dimension line),
    // as mirrored text does with MIRRTEXT off.
    let keep_text = is_reflecting(transform);
    // A text rotation of 0 is "along the dimension line", not an angle: the
    // text turns with the line, so it stays 0.
    let text_rotation = if keep_text || e.base().text_rotation == 0.0 {
        e.base().text_rotation
    } else {
        transform_dimension_angle(old_normal, new_normal, e.base().text_rotation, transform)
    };
    let horizontal_direction = if keep_text {
        e.base().horizontal_direction
    } else {
        transform_dimension_angle(
            old_normal,
            new_normal,
            e.base().horizontal_direction,
            transform,
        )
    };
    let insertion_rotation = transform_dimension_angle(
        old_normal,
        new_normal,
        e.base().insertion_rotation,
        transform,
    );
    let linear_angles = match e {
        // An oblique angle of 0 means extension lines square to the
        // dimension line, which they stay whatever turns the dimension.
        Dimension::Linear(d) => Some((
            transform_dimension_angle(old_normal, new_normal, d.rotation, transform),
            if d.ext_line_rotation == 0.0 {
                0.0
            } else {
                transform_dimension_angle(old_normal, new_normal, d.ext_line_rotation, transform)
            },
        )),
        _ => None,
    };

    let definition_point = transform.apply(e.definition_point());
    e.set_definition_point(definition_point);
    {
        let base = e.base_mut();
        base.text_middle_point = transform.apply(base.text_middle_point);
        let (old_ocs, new_wcs_to_ocs) = ocs_pair(old_normal, new_normal);
        base.insertion_point = new_wcs_to_ocs * transform.apply(old_ocs * base.insertion_point);
        base.normal = new_normal;
        base.text_rotation = text_rotation;
        base.horizontal_direction = horizontal_direction;
        base.insertion_rotation = insertion_rotation;
    }

    match e {
        Dimension::Aligned(d) => {
            d.first_point = transform.apply(d.first_point);
            d.second_point = transform.apply(d.second_point);
        }
        Dimension::Linear(d) => {
            d.first_point = transform.apply(d.first_point);
            d.second_point = transform.apply(d.second_point);
            if let Some((rotation, ext_line_rotation)) = linear_angles {
                d.rotation = rotation;
                d.ext_line_rotation = ext_line_rotation;
            }
        }
        Dimension::Radius(d) => d.angle_vertex = transform.apply(d.angle_vertex),
        Dimension::Diameter(d) => d.angle_vertex = transform.apply(d.angle_vertex),
        Dimension::Angular2Ln(d) => {
            d.dimension_arc = transform.apply(d.dimension_arc);
            d.first_point = transform.apply(d.first_point);
            d.second_point = transform.apply(d.second_point);
            d.angle_vertex = transform.apply(d.angle_vertex);
        }
        Dimension::Angular3Pt(d) => {
            d.first_point = transform.apply(d.first_point);
            d.second_point = transform.apply(d.second_point);
            d.angle_vertex = transform.apply(d.angle_vertex);
        }
        Dimension::Arc(d) => {
            d.first_extension_point = transform.apply(d.first_extension_point);
            d.second_extension_point = transform.apply(d.second_extension_point);
            d.center_point = transform.apply(d.center_point);
            d.first_leader_point = transform.apply(d.first_leader_point);
            d.second_leader_point = transform.apply(d.second_leader_point);
        }
        Dimension::LargeRadial(d) => {
            d.chord_point = transform.apply(d.chord_point);
            d.override_center = transform.apply(d.override_center);
            d.jog_point = transform.apply(d.jog_point);
        }
        Dimension::Ordinate(_) => unreachable!(),
    }
    e.base_mut().actual_measurement = e.stored_measurement();
}

// ── Hatch ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_hatch(e: &mut Hatch, transform: &Transform) {
    e.common.raw_record = None;
    let old_normal = e.normal;
    let old_elevation = e.elevation;

    let new_normal = transform.apply_rotation(old_normal).normalize();

    let old_ocs_to_wcs = Matrix3::arbitrary_axis(old_normal);
    let new_wcs_to_ocs = Matrix3::arbitrary_axis(new_normal).transpose();

    let origin_wcs = old_ocs_to_wcs * Vector3::new(0.0, 0.0, old_elevation);
    let new_origin_wcs = transform.apply(origin_wcs);
    let new_origin_ocs = new_wcs_to_ocs * new_origin_wcs;
    let new_elevation = new_origin_ocs.z;

    let ocs_x_wcs = old_ocs_to_wcs * Vector3::UNIT_X;
    let ocs_y_wcs = old_ocs_to_wcs * Vector3::UNIT_Y;

    let trans_ocs_x_wcs = transform.apply_rotation(ocs_x_wcs);
    let trans_ocs_y_wcs = transform.apply_rotation(ocs_y_wcs);

    let scale_x = trans_ocs_x_wcs.length();
    let scale_y = trans_ocs_y_wcs.length();

    let is_uniform =
        (scale_x - scale_y).abs() < 1e-10 && trans_ocs_x_wcs.dot(&trans_ocs_y_wcs).abs() < 1e-10;

    let x_in_new_ocs = new_wcs_to_ocs * trans_ocs_x_wcs;
    let y_in_new_ocs = new_wcs_to_ocs * trans_ocs_y_wcs;
    let is_flipped = (x_in_new_ocs.x * y_in_new_ocs.y - x_in_new_ocs.y * y_in_new_ocs.x) < 0.0;

    let transform_ocs_point = |p: Vector2| -> Vector2 {
        let p_wcs = old_ocs_to_wcs * Vector3::new(p.x, p.y, old_elevation);
        let p_new_wcs = transform.apply(p_wcs);
        let p_new_ocs = new_wcs_to_ocs * p_new_wcs;
        Vector2::new(p_new_ocs.x, p_new_ocs.y)
    };

    let transform_ocs_angle = |angle_rad: f64| -> f64 {
        let p_ocs = Vector2::new(angle_rad.cos(), angle_rad.sin());
        let p_wcs_dir = old_ocs_to_wcs * Vector3::new(p_ocs.x, p_ocs.y, 0.0);
        let transformed_wcs_dir = transform.apply_rotation(p_wcs_dir);
        let transformed_ocs_dir = new_wcs_to_ocs * transformed_wcs_dir;
        let mut new_angle_rad = transformed_ocs_dir.y.atan2(transformed_ocs_dir.x);
        if new_angle_rad < 0.0 {
            new_angle_rad += 2.0 * std::f64::consts::PI;
        }
        new_angle_rad
    };

    for path in &mut e.paths {
        for edge in &mut path.edges {
            match edge {
                BoundaryEdge::Line(line) => {
                    line.start = transform_ocs_point(line.start);
                    line.end = transform_ocs_point(line.end);
                }
                BoundaryEdge::CircularArc(arc) => {
                    // Clockwise angles are mirrored and their forward sweep is
                    // stored without normalization. Transform the geometric start
                    // angle and preserve that sweep.
                    let to_true = |a: f64, ccw: bool| if ccw { a } else { -a };
                    let norm = |a: f64| a.rem_euclid(2.0 * std::f64::consts::PI);
                    let stored_sweep = arc.end_angle - arc.start_angle;
                    let true_start = to_true(arc.start_angle, arc.counter_clockwise);
                    let new_ccw = arc.counter_clockwise ^ is_flipped;
                    let center = transform_ocs_point(arc.center);

                    if is_uniform {
                        let new_true_start = transform_ocs_angle(true_start);
                        let new_start = norm(to_true(new_true_start, new_ccw));
                        arc.center = center;
                        arc.radius *= scale_x;
                        arc.counter_clockwise = new_ccw;
                        arc.start_angle = new_start;
                        arc.end_angle = new_start + stored_sweep;
                    } else {
                        let major_axis_wcs = trans_ocs_x_wcs * arc.radius;
                        let major_axis_ocs_3d = new_wcs_to_ocs * major_axis_wcs;
                        let major_axis_endpoint =
                            Vector2::new(major_axis_ocs_3d.x, major_axis_ocs_3d.y);

                        // The ellipse parameter equals the circle's TRUE angle:
                        // the transformed axes carry the deformation, so the
                        // parameter is unchanged — only re-mirrored for the new
                        // direction flag, with the stored sweep carried over.
                        let mut start = norm(to_true(true_start, new_ccw));
                        let mut ratio = scale_y / scale_x;
                        let mut major = major_axis_endpoint;
                        if ratio > 1.0 {
                            let len = major.length() * ratio;
                            major = Vector2::new(-major.y, major.x).normalize() * len;
                            ratio = 1.0 / ratio;
                            // Major axis rotated +90°: true parameter shifts by
                            // -π/2; stored space mirrors that for CW edges.
                            let shift = if new_ccw {
                                -std::f64::consts::FRAC_PI_2
                            } else {
                                std::f64::consts::FRAC_PI_2
                            };
                            start = norm(start + shift);
                        }
                        *edge = BoundaryEdge::EllipticArc(EllipticArcEdge {
                            center,
                            major_axis_endpoint: major,
                            minor_axis_ratio: ratio,
                            start_angle: start,
                            end_angle: start + stored_sweep,
                            counter_clockwise: new_ccw,
                        });
                    }
                }
                BoundaryEdge::EllipticArc(ellipse) => {
                    let center = transform_ocs_point(ellipse.center);

                    let old_major_wcs = old_ocs_to_wcs
                        * Vector3::new(
                            ellipse.major_axis_endpoint.x,
                            ellipse.major_axis_endpoint.y,
                            0.0,
                        );
                    let old_major_len = ellipse.major_axis_endpoint.length();
                    let old_minor_len = old_major_len * ellipse.minor_axis_ratio;
                    let old_minor_ocs_dir = Vector2::new(
                        -ellipse.major_axis_endpoint.y,
                        ellipse.major_axis_endpoint.x,
                    )
                    .normalize();
                    let old_minor_wcs = old_ocs_to_wcs
                        * Vector3::new(
                            old_minor_ocs_dir.x * old_minor_len,
                            old_minor_ocs_dir.y * old_minor_len,
                            0.0,
                        );

                    let new_major_wcs = transform.apply_rotation(old_major_wcs);
                    let new_minor_wcs = transform.apply_rotation(old_minor_wcs);

                    let new_major_ocs_3d = new_wcs_to_ocs * new_major_wcs;
                    let new_minor_ocs_3d = new_wcs_to_ocs * new_minor_wcs;

                    let new_major_ocs = Vector2::new(new_major_ocs_3d.x, new_major_ocs_3d.y);
                    let new_minor_ocs = Vector2::new(new_minor_ocs_3d.x, new_minor_ocs_3d.y);

                    let new_major_len = new_major_ocs.length();
                    let new_minor_len = new_minor_ocs.length();

                    ellipse.center = center;
                    // Same stored-angle convention as CircularArc: CW edges
                    // keep mirrored parameters, and the stored sweep
                    // (end - start ≥ 0, end may exceed 2π to encode a wrap)
                    // must survive untouched. Transform only the start
                    // parameter and carry the sweep over.
                    let norm = |a: f64| a.rem_euclid(2.0 * std::f64::consts::PI);
                    let stored_sweep = ellipse.end_angle - ellipse.start_angle;
                    if is_flipped {
                        ellipse.counter_clockwise = !ellipse.counter_clockwise;
                        ellipse.start_angle = norm(-ellipse.start_angle);
                    }

                    if new_minor_len > new_major_len + 1e-12 {
                        ellipse.major_axis_endpoint = new_minor_ocs;
                        ellipse.minor_axis_ratio = new_major_len / new_minor_len;
                        // True parameter shifts by -π/2 (major rotated +90°);
                        // stored space mirrors that for CW edges.
                        let shift = if ellipse.counter_clockwise {
                            -std::f64::consts::FRAC_PI_2
                        } else {
                            std::f64::consts::FRAC_PI_2
                        };
                        ellipse.start_angle = norm(ellipse.start_angle + shift);
                        ellipse.end_angle = ellipse.start_angle + stored_sweep;
                    } else {
                        ellipse.major_axis_endpoint = new_major_ocs;
                        ellipse.minor_axis_ratio = if new_major_len > 1e-12 {
                            new_minor_len / new_major_len
                        } else {
                            1.0
                        };
                        ellipse.end_angle = ellipse.start_angle + stored_sweep;
                    }
                }
                BoundaryEdge::Spline(spline) => {
                    for cp in &mut spline.control_points {
                        let p_wcs = old_ocs_to_wcs * Vector3::new(cp.x, cp.y, old_elevation);
                        let p_new_wcs = transform.apply(p_wcs);
                        let p_new_ocs = new_wcs_to_ocs * p_new_wcs;
                        cp.x = p_new_ocs.x;
                        cp.y = p_new_ocs.y;
                    }
                    for fp in &mut spline.fit_points {
                        *fp = transform_ocs_point(*fp);
                    }
                    let trans_dir = |d: Vector2| -> Vector2 {
                        let d_wcs = old_ocs_to_wcs * Vector3::new(d.x, d.y, 0.0);
                        let d_new_wcs = transform.apply_rotation(d_wcs);
                        let d_new_ocs = new_wcs_to_ocs * d_new_wcs;
                        Vector2::new(d_new_ocs.x, d_new_ocs.y)
                    };
                    spline.start_tangent = trans_dir(spline.start_tangent);
                    spline.end_tangent = trans_dir(spline.end_tangent);
                }
                BoundaryEdge::Polyline(poly) => {
                    for v in &mut poly.vertices {
                        let t = transform_ocs_point(Vector2::new(v.x, v.y));
                        v.x = t.x;
                        v.y = t.y;
                        if is_flipped {
                            v.z = -v.z;
                        }
                    }
                }
            }
        }
    }

    for seed in &mut e.seed_points {
        *seed = transform_ocs_point(*seed);
    }

    let p_dir = Vector2::new(e.pattern_angle.cos(), e.pattern_angle.sin());
    let p_wcs_dir = old_ocs_to_wcs * Vector3::new(p_dir.x, p_dir.y, 0.0);
    let transformed_p_wcs_dir = transform.apply_rotation(p_wcs_dir);
    let transformed_p_ocs_dir = new_wcs_to_ocs * transformed_p_wcs_dir;
    e.pattern_angle = transformed_p_ocs_dir.y.atan2(transformed_p_ocs_dir.x);
    e.pattern_scale *= scale_x;

    let pattern_origin = transform_ocs_point(e.pattern_origin());
    e.record_pattern_origin(pattern_origin);

    for line in &mut e.pattern.lines {
        let l_dir = Vector2::new(line.angle.cos(), line.angle.sin());
        let l_wcs_dir = old_ocs_to_wcs * Vector3::new(l_dir.x, l_dir.y, 0.0);
        let transformed_l_wcs_dir = transform.apply_rotation(l_wcs_dir);
        let transformed_l_ocs_dir = new_wcs_to_ocs * transformed_l_wcs_dir;
        line.angle = transformed_l_ocs_dir.y.atan2(transformed_l_ocs_dir.x);

        line.base_point = transform_ocs_point(line.base_point);

        let off_wcs = old_ocs_to_wcs * Vector3::new(line.offset.x, line.offset.y, 0.0);
        let transformed_off_wcs = transform.apply_rotation(off_wcs);
        let transformed_off_ocs = new_wcs_to_ocs * transformed_off_wcs;
        line.offset = Vector2::new(transformed_off_ocs.x, transformed_off_ocs.y);

        for dash in &mut line.dash_lengths {
            *dash *= scale_x;
        }
    }

    if e.gradient_color.enabled {
        let g_dir = Vector2::new(e.gradient_color.angle.cos(), e.gradient_color.angle.sin());
        let g_wcs_dir = old_ocs_to_wcs * Vector3::new(g_dir.x, g_dir.y, 0.0);
        let transformed_g_wcs_dir = transform.apply_rotation(g_wcs_dir);
        let transformed_g_ocs_dir = new_wcs_to_ocs * transformed_g_wcs_dir;
        e.gradient_color.angle = transformed_g_ocs_dir.y.atan2(transformed_g_ocs_dir.x);
    }

    e.normal = new_normal;
    e.elevation = new_elevation;
}

// ── Solid ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_solid(e: &mut Solid, transform: &Transform) {
    e.common.raw_record = None;
    // Corners are stored in OCS.
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    e.first_corner = new_t * transform.apply(ocs * e.first_corner);
    e.second_corner = new_t * transform.apply(ocs * e.second_corner);
    e.third_corner = new_t * transform.apply(ocs * e.third_corner);
    e.fourth_corner = new_t * transform.apply(ocs * e.fourth_corner);
    e.normal = new_normal;
}

// ── Face3D ───────────────────────────────────────────────────────────────────

pub(crate) fn transform_face3d(e: &mut Face3D, transform: &Transform) {
    e.common.raw_record = None;
    e.first_corner = transform.apply(e.first_corner);
    e.second_corner = transform.apply(e.second_corner);
    e.third_corner = transform.apply(e.third_corner);
    e.fourth_corner = transform.apply(e.fourth_corner);
}

// ── Insert ───────────────────────────────────────────────────────────────────

/// Minimum absolute value accepted for insert scale factors.
const SCALE_EPSILON: f64 = 1e-12;

/// Transform a normal vector using inverse-transpose of the upper 3×3
/// matrix, which is the geometrically correct approach for normals under
/// non-uniform scale. Falls back to the original normal if the matrix is
/// singular.
pub(crate) fn transform_normal(transform: &Transform, normal: Vector3) -> Vector3 {
    let m4 = transform.matrix;
    let upper3x3 = Matrix3::from_rows(
        [m4.m[0][0], m4.m[0][1], m4.m[0][2]],
        [m4.m[1][0], m4.m[1][1], m4.m[1][2]],
        [m4.m[2][0], m4.m[2][1], m4.m[2][2]],
    );
    if let Some(inv) = upper3x3.inverse() {
        let inv_t = inv.transpose();
        let transformed = inv_t.transform_point(normal);
        let len = transformed.length();
        if len < 1e-10 {
            normal
        } else {
            transformed * (1.0 / len)
        }
    } else {
        normal
    }
}

pub(crate) fn transform_insert(e: &mut Insert, transform: &Transform) {
    e.common.raw_record = None;
    let new_normal = transform_normal(transform, e.normal);

    // `insert_point` is stored in the OCS defined by `normal`, not in world
    // space. Convert it to world, apply the transform there, then map it back
    // into the (possibly new) OCS — otherwise a world-space move/rotate of a
    // block whose extrusion direction isn't +Z lands on the wrong axes.
    // For a +Z normal the OCS is the identity, so this is a no-op.
    let ocs_old = Matrix3::arbitrary_axis(e.normal);
    let world_pos = ocs_old * e.insert_point;
    let new_world_pos = transform.apply(world_pos);
    let ocs_new = Matrix3::arbitrary_axis(new_normal);
    let new_position = ocs_new.transpose() * new_world_pos;

    let trans_ow = Matrix3::arbitrary_axis(e.normal) * Matrix3::rotation_z(e.rotation);

    let trans_wo_base = Matrix3::arbitrary_axis(new_normal);
    let trans_wo = trans_wo_base.transpose();

    let m4 = transform.matrix;
    let transformation = Matrix3::from_rows(
        [m4.m[0][0], m4.m[0][1], m4.m[0][2]],
        [m4.m[1][0], m4.m[1][1], m4.m[1][2]],
        [m4.m[2][0], m4.m[2][1], m4.m[2][2]],
    );

    let v = trans_wo * (transformation * (trans_ow * Vector3::UNIT_X));
    let new_rotation = v.y.atan2(v.x);

    let trans_wo_rot = Matrix3::rotation_z(new_rotation).transpose() * trans_wo;
    let s = trans_wo_rot
        * (transformation * (trans_ow * Vector3::new(e.x_scale(), e.y_scale(), e.z_scale())));

    // MINSERT spacing is measured in the INSERT's local X/Y axes.  Carry the
    // vectors through the same world transform so a nested array under a
    // uniform parent scale grows with its geometry (ACAD doubles a 10-unit
    // spacing under a 2x parent).  A scalar spacing cannot encode shear, so
    // retain the transformed vector length, which is the closest representable
    // INSERT spacing and is exact for similarity transforms.  The sign comes
    // from the new local axes, as for the scale factors above: a negative
    // spacing (an array running along -X or -Y) keeps running that way.
    if e.column_spacing != 0.0 {
        let v = transformation * (trans_ow * Vector3::new(e.column_spacing, 0.0, 0.0));
        e.column_spacing = v.length().copysign((trans_wo_rot * v).x);
    }
    if e.row_spacing != 0.0 {
        let v = transformation * (trans_ow * Vector3::new(0.0, e.row_spacing, 0.0));
        e.row_spacing = v.length().copysign((trans_wo_rot * v).y);
    }

    let clamp = |val: f64| -> f64 {
        if val.abs() < SCALE_EPSILON {
            SCALE_EPSILON
        } else {
            val
        }
    };

    e.normal = new_normal;
    e.insert_point = new_position;
    e.set_x_scale(clamp(s.x));
    e.set_y_scale(clamp(s.y));
    e.set_z_scale(clamp(s.z));
    e.rotation = new_rotation;

    for att in &mut e.attributes {
        att.apply_transform(transform);
    }
}

// ── Block ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_block(e: &mut Block, transform: &Transform) {
    e.common.raw_record = None;
    e.base_point = transform.apply(e.base_point);
}

// ── BlockEnd ─────────────────────────────────────────────────────────────────

pub(crate) fn transform_block_end(_e: &mut BlockEnd, _transform: &Transform) {
    // No geometry
}

// ── Ray ──────────────────────────────────────────────────────────────────────

pub(crate) fn transform_ray(e: &mut Ray, transform: &Transform) {
    e.common.raw_record = None;
    e.base_point = transform.apply(e.base_point);
    e.direction = transform.apply_rotation(e.direction).normalize();
}

// ── XLine ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_xline(e: &mut XLine, transform: &Transform) {
    e.common.raw_record = None;
    e.base_point = transform.apply(e.base_point);
    e.direction = transform.apply_rotation(e.direction).normalize();
}

// ── Viewport ─────────────────────────────────────────────────────────────────

pub(crate) fn transform_viewport(e: &mut Viewport, transform: &Transform) {
    e.common.raw_record = None;
    e.center = transform.apply(e.center);

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.width *= scale_factor;
    e.height *= scale_factor;

    e.view_direction = transform.apply_rotation(e.view_direction).normalize();
    e.ucs_x_axis = transform.apply_rotation(e.ucs_x_axis).normalize();
    e.ucs_y_axis = transform.apply_rotation(e.ucs_y_axis).normalize();

    e.ucs_origin = transform.apply(e.ucs_origin);
    e.view_target = transform.apply(e.view_target);
}

// ── AttributeDefinition ──────────────────────────────────────────────────────

pub(crate) fn transform_attribute_definition(e: &mut AttributeDefinition, transform: &Transform) {
    e.common.raw_record = None;
    // Insertion / alignment points are stored in OCS.
    let old_normal = e.normal;
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    e.insertion_point = new_t * transform.apply(ocs * e.insertion_point);
    e.alignment_point = new_t * transform.apply(ocs * e.alignment_point);
    e.normal = new_normal;

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.height *= scale_factor;
    rotate_ocs_angle(&mut e.rotation, old_normal, transform);
    // A multiline attribute draws its embedded MTEXT: transform it too.
    if let Some(m) = e.embedded_mtext.as_deref_mut() {
        transform_mtext(m, transform);
    }
}

// ── AttributeEntity ──────────────────────────────────────────────────────────

pub(crate) fn transform_attribute_entity(e: &mut AttributeEntity, transform: &Transform) {
    e.common.raw_record = None;
    // Insertion / alignment points are stored in OCS.
    let old_normal = e.normal;
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    e.insertion_point = new_t * transform.apply(ocs * e.insertion_point);
    e.alignment_point = new_t * transform.apply(ocs * e.alignment_point);
    e.normal = new_normal;

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.height *= scale_factor;
    rotate_ocs_angle(&mut e.rotation, old_normal, transform);
    if let Some(m) = e.embedded_mtext.as_deref_mut() {
        transform_mtext(m, transform);
    }
}

// ── Leader ───────────────────────────────────────────────────────────────────

pub(crate) fn transform_leader(e: &mut Leader, transform: &Transform) {
    e.common.raw_record = None;
    for vertex in &mut e.vertices {
        *vertex = transform.apply(*vertex);
    }
    e.block_offset = transform.apply(e.block_offset);
    e.annotation_offset = transform.apply(e.annotation_offset);
    e.horizontal_direction = transform.apply_rotation(e.horizontal_direction).normalize();
    e.normal = transform.apply_rotation(e.normal).normalize();
}

// ── MultiLeader ──────────────────────────────────────────────────────────────

pub(crate) fn transform_multileader(e: &mut MultiLeader, transform: &Transform) {
    e.common.raw_record = None;
    e.context.content_base_point = transform.apply(e.context.content_base_point);
    e.context.text_location = transform.apply(e.context.text_location);
    e.context.block_content_location = transform.apply(e.context.block_content_location);
    e.context.base_point = transform.apply(e.context.base_point);

    e.context.text_normal = transform.apply_rotation(e.context.text_normal).normalize();
    e.context.text_direction = transform
        .apply_rotation(e.context.text_direction)
        .normalize();
    e.context.base_direction = transform
        .apply_rotation(e.context.base_direction)
        .normalize();
    e.context.base_vertical = transform
        .apply_rotation(e.context.base_vertical)
        .normalize();

    for root in &mut e.context.leader_roots {
        root.connection_point = transform.apply(root.connection_point);
        root.direction = transform.apply_rotation(root.direction).normalize();
        for bp in &mut root.break_points {
            bp.start_point = transform.apply(bp.start_point);
            bp.end_point = transform.apply(bp.end_point);
        }
        for line in &mut root.lines {
            for point in &mut line.points {
                *point = transform.apply(*point);
            }
            for bp in &mut line.break_points {
                bp.start_point = transform.apply(bp.start_point);
                bp.end_point = transform.apply(bp.end_point);
            }
        }
    }

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    for root in &mut e.context.leader_roots {
        root.landing_distance *= scale_factor;
    }
    e.arrowhead_size *= scale_factor;
    e.text_height *= scale_factor;
    e.dogleg_length *= scale_factor;
    e.context.arrowhead_size *= scale_factor;
    e.context.text_height *= scale_factor;
    e.context.landing_gap *= scale_factor;
    e.context.text_width *= scale_factor;
    e.context.text_boundary_height *= scale_factor;
    e.context.block_content_scale = e.context.block_content_scale * scale_factor;
    rotate_stored_angle(&mut e.context.text_rotation, transform);
    rotate_stored_angle(&mut e.context.block_rotation, transform);
    rotate_stored_angle(&mut e.block_rotation, transform);
}

// ── MLine ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_mline(e: &mut MLine, transform: &Transform) {
    e.common.raw_record = None;
    e.start_point = transform.apply(e.start_point);
    for vertex in &mut e.vertices {
        vertex.position = transform.apply(vertex.position);
        vertex.direction = transform.apply_rotation(vertex.direction).normalize();
        vertex.miter = transform.apply_rotation(vertex.miter).normalize();
    }
    e.normal = transform.apply_rotation(e.normal).normalize();
}

// ── Mesh ─────────────────────────────────────────────────────────────────────

pub(crate) fn transform_mesh(e: &mut Mesh, transform: &Transform) {
    e.common.raw_record = None;
    for vertex in &mut e.vertices {
        *vertex = transform.apply(*vertex);
    }
}

// ── RasterImage ──────────────────────────────────────────────────────────────

pub(crate) fn transform_raster_image(e: &mut RasterImage, transform: &Transform) {
    e.common.raw_record = None;
    // Same as a wipeout: `u_vector` / `v_vector` carry the pixel size, and the
    // linear part already scales them — a second multiply squared the factor.
    e.insertion_point = transform.apply(e.insertion_point);
    e.u_vector = transform.apply_rotation(e.u_vector);
    e.v_vector = transform.apply_rotation(e.v_vector);
}

// ── Solid3D ──────────────────────────────────────────────────────────────────

pub(crate) fn transform_solid3d(e: &mut Solid3D, transform: &Transform) {
    e.common.raw_record = None;
    e.point_of_reference = transform.apply(e.point_of_reference);
    compose_acis_placement(&mut e.acis_data, transform);
    for wire in &mut e.wires {
        for pt in &mut wire.points {
            *pt = transform.apply(*pt);
        }
    }
    for silhouette in &mut e.silhouettes {
        silhouette.target = transform.apply(silhouette.target);
        silhouette.view_direction = transform
            .apply_rotation(silhouette.view_direction)
            .normalize();
        silhouette.up_vector = transform.apply_rotation(silhouette.up_vector).normalize();
        for wire in &mut silhouette.wires {
            for pt in &mut wire.points {
                *pt = transform.apply(*pt);
            }
        }
    }
}

// ── Region ───────────────────────────────────────────────────────────────────

pub(crate) fn transform_region(e: &mut Region, transform: &Transform) {
    e.common.raw_record = None;
    e.point_of_reference = transform.apply(e.point_of_reference);
    compose_acis_placement(&mut e.acis_data, transform);
    for wire in &mut e.wires {
        for pt in &mut wire.points {
            *pt = transform.apply(*pt);
        }
    }
    for silhouette in &mut e.silhouettes {
        silhouette.target = transform.apply(silhouette.target);
        silhouette.view_direction = transform
            .apply_rotation(silhouette.view_direction)
            .normalize();
        silhouette.up_vector = transform.apply_rotation(silhouette.up_vector).normalize();
        for wire in &mut silhouette.wires {
            for pt in &mut wire.points {
                *pt = transform.apply(*pt);
            }
        }
    }
}

// ── Body ─────────────────────────────────────────────────────────────────────

pub(crate) fn transform_body(e: &mut Body, transform: &Transform) {
    e.common.raw_record = None;
    e.point_of_reference = transform.apply(e.point_of_reference);
    compose_acis_placement(&mut e.acis_data, transform);
    for wire in &mut e.wires {
        for pt in &mut wire.points {
            *pt = transform.apply(*pt);
        }
    }
    for silhouette in &mut e.silhouettes {
        silhouette.target = transform.apply(silhouette.target);
        silhouette.view_direction = transform
            .apply_rotation(silhouette.view_direction)
            .normalize();
        silhouette.up_vector = transform.apply_rotation(silhouette.up_vector).normalize();
        for wire in &mut silhouette.wires {
            for pt in &mut wire.points {
                *pt = transform.apply(*pt);
            }
        }
    }
}

/// Keep embedded loft curves in their local frame while moving their placement.
pub(crate) fn transform_loft_placement(e: &mut crate::entities::Surface, transform: &Transform) {
    e.common.raw_record = None;
    if let crate::entities::SurfaceData::Lofted { loft_transform, .. } = &mut e.surface_data {
        let previous = *loft_transform;
        *loft_transform = std::array::from_fn(|index| {
            let row = index % 4;
            let column = index / 4;
            (0..4)
                .map(|inner| transform.matrix.m[row][inner] * previous[column * 4 + inner])
                .sum()
        });
    }
}

pub(crate) fn transform_surface(e: &mut crate::entities::Surface, transform: &Transform) {
    e.common.raw_record = None;
    transform_loft_placement(e, transform);
    e.point_of_reference = transform.apply(e.point_of_reference);
    compose_acis_placement(&mut e.acis_data, transform);
    for wire in &mut e.wires {
        for pt in &mut wire.points {
            *pt = transform.apply(*pt);
        }
    }
    for silhouette in &mut e.silhouettes {
        silhouette.target = transform.apply(silhouette.target);
        silhouette.view_direction = transform
            .apply_rotation(silhouette.view_direction)
            .normalize();
        silhouette.up_vector = transform.apply_rotation(silhouette.up_vector).normalize();
        for wire in &mut silhouette.wires {
            for pt in &mut wire.points {
                *pt = transform.apply(*pt);
            }
        }
    }
}

// ── Table ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_table(e: &mut Table, transform: &Transform) {
    e.common.raw_record = None;
    e.insertion_point = transform.apply(e.insertion_point);
    e.horizontal_direction = transform.apply_rotation(e.horizontal_direction).normalize();
    e.normal = transform.apply_rotation(e.normal).normalize();

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    for col in &mut e.columns {
        col.width *= scale_factor;
    }
    for row in &mut e.rows {
        row.height *= scale_factor;
    }
}

// ── Tolerance ────────────────────────────────────────────────────────────────

pub(crate) fn transform_tolerance(e: &mut Tolerance, transform: &Transform) {
    e.common.raw_record = None;
    e.insertion_point = transform.apply(e.insertion_point);
    e.direction = transform.apply_rotation(e.direction).normalize();
    e.normal = transform.apply_rotation(e.normal).normalize();

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.text_height *= scale_factor;
    e.dimension_gap *= scale_factor;
}

// ── PolyfaceMesh ─────────────────────────────────────────────────────────────

pub(crate) fn transform_polyface_mesh(e: &mut PolyfaceMesh, transform: &Transform) {
    e.common.raw_record = None;
    for v in &mut e.vertices {
        v.location = transform.apply(v.location);
    }
    e.normal = transform.apply_rotation(e.normal).normalize();
}

// ── Wipeout ──────────────────────────────────────────────────────────────────

pub(crate) fn transform_wipeout(e: &mut Wipeout, transform: &Transform) {
    e.common.raw_record = None;
    // `u_vector` / `v_vector` carry both the image direction AND its size, so
    // the linear part of the transform (rotation + scale, via `apply_rotation`)
    // already scales them. Multiplying again by the scale factor double-scaled
    // the mask (a 2× insert grew the wipeout 4×).
    e.insertion_point = transform.apply(e.insertion_point);
    e.u_vector = transform.apply_rotation(e.u_vector);
    e.v_vector = transform.apply_rotation(e.v_vector);
}

// ── Shape ────────────────────────────────────────────────────────────────────

pub(crate) fn transform_shape(e: &mut Shape, transform: &Transform) {
    e.common.raw_record = None;
    // Insertion point is stored in OCS.
    let old_normal = e.normal;
    let new_normal = transform.apply_rotation(e.normal).normalize();
    let (ocs, new_t) = ocs_pair(e.normal, new_normal);
    e.insertion_point = new_t * transform.apply(ocs * e.insertion_point);
    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.size *= scale_factor;
    e.normal = new_normal;
    rotate_ocs_angle(&mut e.rotation, old_normal, transform);
}

// ── Underlay ─────────────────────────────────────────────────────────────────

pub(crate) fn transform_underlay(e: &mut Underlay, transform: &Transform) {
    e.common.raw_record = None;
    let old_normal = e.normal;
    e.insertion_point = transform.apply(e.insertion_point);
    e.normal = transform.apply_rotation(e.normal).normalize();

    let unit_x = Vector3::new(1.0, 0.0, 0.0);
    let transformed_unit = transform.apply_rotation(unit_x);
    let scale_factor = transformed_unit.length();
    e.x_scale *= scale_factor;
    e.y_scale *= scale_factor;
    e.z_scale *= scale_factor;
    rotate_ocs_angle(&mut e.rotation, old_normal, transform);
}

// ── Seqend ───────────────────────────────────────────────────────────────────

pub(crate) fn transform_seqend(_e: &mut Seqend, _transform: &Transform) {
    // No geometry
}

// ── Ole2Frame ────────────────────────────────────────────────────────────────

pub(crate) fn transform_ole2frame(e: &mut Ole2Frame, transform: &Transform) {
    e.common.raw_record = None;
    e.upper_left_corner = transform.apply(e.upper_left_corner);
    e.lower_right_corner = transform.apply(e.lower_right_corner);
}

// ── PolygonMesh ──────────────────────────────────────────────────────────────

pub(crate) fn transform_polygon_mesh(e: &mut PolygonMeshEntity, transform: &Transform) {
    e.common.raw_record = None;
    for v in &mut e.vertices {
        v.location = transform.apply(v.location);
    }
    e.normal = transform.apply_rotation(e.normal).normalize();
}

// ── UnknownEntity ────────────────────────────────────────────────────────────

pub(crate) fn transform_unknown(_e: &mut UnknownEntity, _transform: &Transform) {
    // No geometry
}

pub(crate) fn transform_invalidates_raw_record(entity: &EntityType) -> bool {
    match entity {
        EntityType::Unknown(_) | EntityType::BlockEnd(_) | EntityType::Seqend(_) => false,
        EntityType::Extended(value) => !matches!(
            &value.data,
            ExtendedEntityData::Camera { .. }
                | ExtendedEntityData::Proxy(_)
                | ExtendedEntityData::OleFrame(_)
                | ExtendedEntityData::LayoutPrintConfig(_)
                | ExtendedEntityData::Format(_)
                | ExtendedEntityData::Legacy(_)
                | ExtendedEntityData::DynamicBlock(_)
                | ExtendedEntityData::RegisteredClass(_)
                | ExtendedEntityData::Arch(_)
        ),
        _ => true,
    }
}

// ── EntityType dispatch ──────────────────────────────────────────────────────

impl EntityType {
    /// Apply a general transform to this entity.
    ///
    /// Dispatches to the appropriate per-entity implementation.
    /// This is equivalent to calling `entity.as_entity_mut().apply_transform(t)`.
    pub fn apply_transform(&mut self, transform: &Transform) {
        match self {
            EntityType::Point(e) => transform_point(e, transform),
            EntityType::Line(e) => transform_line(e, transform),
            EntityType::Circle(e) => transform_circle(e, transform),
            EntityType::Arc(e) => transform_arc(e, transform),
            EntityType::Ellipse(e) => transform_ellipse(e, transform),
            EntityType::Polyline(e) => transform_polyline(e, transform),
            EntityType::Polyline2D(e) => transform_polyline2d(e, transform),
            EntityType::Polyline3D(e) => transform_polyline3d(e, transform),
            EntityType::LwPolyline(e) => transform_lwpolyline(e, transform),
            EntityType::Text(e) => transform_text(e, transform),
            EntityType::MText(e) => transform_mtext(e, transform),
            EntityType::Spline(e) => transform_spline(e, transform),
            EntityType::Helix(e) => transform_helix(e, transform),
            EntityType::Dimension(e) => e.apply_transform(transform),
            EntityType::Hatch(e) => transform_hatch(e, transform),
            EntityType::Solid(e) => transform_solid(e, transform),
            EntityType::Face3D(e) => transform_face3d(e, transform),
            EntityType::Insert(e) => transform_insert(e, transform),
            EntityType::Block(e) => transform_block(e, transform),
            EntityType::BlockEnd(e) => transform_block_end(e, transform),
            EntityType::Ray(e) => transform_ray(e, transform),
            EntityType::XLine(e) => transform_xline(e, transform),
            EntityType::Viewport(e) => transform_viewport(e, transform),
            EntityType::AttributeDefinition(e) => transform_attribute_definition(e, transform),
            EntityType::AttributeEntity(e) => transform_attribute_entity(e, transform),
            EntityType::Leader(e) => transform_leader(e, transform),
            EntityType::MultiLeader(e) => transform_multileader(e, transform),
            EntityType::MLine(e) => transform_mline(e, transform),
            EntityType::Mesh(e) => transform_mesh(e, transform),
            EntityType::RasterImage(e) => transform_raster_image(e, transform),
            EntityType::Solid3D(e) => transform_solid3d(e, transform),
            EntityType::Region(e) => transform_region(e, transform),
            EntityType::Body(e) => transform_body(e, transform),
            EntityType::Surface(e) => transform_surface(e, transform),
            EntityType::Table(e) => transform_table(e, transform),
            EntityType::Tolerance(e) => transform_tolerance(e, transform),
            EntityType::PolyfaceMesh(e) => transform_polyface_mesh(e, transform),
            EntityType::Wipeout(e) => transform_wipeout(e, transform),
            EntityType::Shape(e) => transform_shape(e, transform),
            EntityType::Underlay(e) => transform_underlay(e, transform),
            EntityType::Seqend(e) => transform_seqend(e, transform),
            EntityType::Ole2Frame(e) => transform_ole2frame(e, transform),
            EntityType::PolygonMesh(e) => transform_polygon_mesh(e, transform),
            EntityType::Light(e) => e.apply_transform(transform),
            EntityType::SectionSymbol(e) => e.apply_transform(transform),
            EntityType::ViewBorder(e) => e.apply_transform(transform),
            EntityType::Extended(e) => e.apply_transform(transform),
            EntityType::Unknown(e) => transform_unknown(e, transform),
        }
        // Unknown records have no geometry for their XDATA to follow.
        if !matches!(self, EntityType::Unknown(_)) {
            let _ = self.common_mut().extended_data.apply_transform(transform);
        }
        // Applying a semantic transform invalidates verbatim DWG provenance;
        // unknown entities have no editable geometry and keep their payload.
        if transform_invalidates_raw_record(self) {
            self.common_mut().raw_record = None;
        }
    }

    /// Apply rotation around an axis.
    pub fn apply_rotation(&mut self, axis: Vector3, angle: f64) {
        self.apply_transform(&Transform::from_rotation(axis, angle));
    }

    /// Apply uniform scaling.
    pub fn apply_scaling(&mut self, scale: f64) {
        self.apply_transform(&Transform::from_scale(scale));
    }

    /// Apply non-uniform scaling.
    pub fn apply_scaling_xyz(&mut self, scale: Vector3) {
        self.apply_transform(&Transform::from_scaling(scale));
    }

    /// Apply scaling with a specific origin point.
    pub fn apply_scaling_with_origin(&mut self, scale: Vector3, origin: Vector3) {
        self.apply_transform(&Transform::from_scaling_with_origin(scale, origin));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Vector3;

    #[test]
    fn test_transform_line() {
        let mut line = Line::from_points(Vector3::new(1.0, 0.0, 0.0), Vector3::new(2.0, 0.0, 0.0));
        let t = Transform::from_scale(2.0);
        transform_line(&mut line, &t);
        assert!((line.start.x - 2.0).abs() < 1e-10);
        assert!((line.end.x - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_multiline_attribute_transforms_embedded_mtext() {
        let mut att = AttributeEntity::new("T".into(), "V".into());
        let mut m = MText::new();
        m.insertion_point = Vector3::new(10.0, 0.0, 0.0);
        m.height = 2.5;
        att.embedded_mtext = Some(Box::new(m));

        // SCALE 2 about the origin: the drawn MTEXT must scale with the attribute.
        transform_attribute_entity(&mut att, &Transform::from_scale(2.0));
        let m = att.embedded_mtext.as_ref().unwrap();
        assert!((m.insertion_point.x - 20.0).abs() < 1e-10);
        assert!((m.height - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_circle_scales_radius() {
        let mut circle = Circle::new();
        circle.center = Vector3::ZERO;
        circle.radius = 5.0;
        let t = Transform::from_scale(3.0);
        transform_circle(&mut circle, &t);
        assert!((circle.radius - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_entity_type_dispatch() {
        let mut entity = EntityType::Circle({
            let mut c = Circle::new();
            c.radius = 5.0;
            c
        });
        entity.apply_transform(&Transform::from_scale(2.0));
        if let EntityType::Circle(c) = &entity {
            assert!((c.radius - 10.0).abs() < 1e-10);
        } else {
            panic!("Expected Circle");
        }
    }

    fn assert_vector_near(actual: Vector3, expected: Vector3) {
        assert!(
            actual.distance(&expected) < 1e-9,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn test_transform_linear_dimension_rotates_out_of_world_plane() {
        let mut linear = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
        linear.definition_point = Vector3::new(0.0, 5.0, 0.0);
        linear.base.definition_point = linear.definition_point;
        let mut dimension = Dimension::Linear(linear);

        transform_dimension(
            &mut dimension,
            &Transform::from_rotation(Vector3::UNIT_Y, std::f64::consts::FRAC_PI_2),
        );

        let Dimension::Linear(linear) = dimension else {
            unreachable!()
        };
        assert_vector_near(linear.second_point, Vector3::new(0.0, 0.0, -10.0));
        assert_vector_near(linear.base.normal, Vector3::UNIT_X);
        assert!((linear.measurement() - 10.0).abs() < 1e-9);
        assert!((linear.base.actual_measurement - 10.0).abs() < 1e-9);
    }

    // 0 is "along the dimension line" / "square extension lines", not an
    // angle: a turned dimension keeps it (OpenCADStudio#1761).
    #[test]
    fn test_rotating_a_dimension_keeps_its_default_angles() {
        let mut linear = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
        linear.definition_point = Vector3::new(0.0, 5.0, 0.0);
        let mut dimension = Dimension::Linear(linear);
        transform_dimension(
            &mut dimension,
            &Transform::from_rotation(Vector3::UNIT_Z, std::f64::consts::FRAC_PI_2),
        );
        let Dimension::Linear(linear) = dimension else {
            unreachable!()
        };
        assert!((linear.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert_eq!(linear.ext_line_rotation, 0.0);
        assert_eq!(linear.base.text_rotation, 0.0);
    }

    #[test]
    fn test_transform_linear_dimension_mirrors_about_offset_plane() {
        let mut linear = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
        linear.definition_point = Vector3::new(0.0, 5.0, 0.0);
        linear.base.definition_point = linear.definition_point;
        let mut dimension = Dimension::Linear(linear);
        let plane_point = Vector3::new(0.0, 2.0, 0.0);
        let reflection = Transform::from_translation(-plane_point)
            .then(&Transform::from_scaling(Vector3::new(1.0, -1.0, 1.0)))
            .then(&Transform::from_translation(plane_point));

        transform_dimension(&mut dimension, &reflection);

        let Dimension::Linear(linear) = dimension else {
            unreachable!()
        };
        assert_vector_near(linear.definition_point, Vector3::new(0.0, -1.0, 0.0));
        assert!((linear.measurement() - 10.0).abs() < 1e-9);
        assert!((linear.base.actual_measurement - 10.0).abs() < 1e-9);
    }

    // Mirrored boundary arcs must remain continuous with adjacent lines.
    #[test]
    fn test_mirror_hatch_arc_edge_stays_continuous() {
        use crate::entities::hatch::{BoundaryEdge, BoundaryPath, CircularArcEdge, LineEdge};
        use crate::types::Vector2;

        // Path: line from (2,0)→... then a CCW half-circle r=1 centered at
        // (1,0) from angle 0 to π (i.e. (2,0) → (0,0)), then line back.
        let mut path = BoundaryPath::new();
        path.edges.push(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(0.0, -1.0),
            end: Vector2::new(2.0, 0.0),
        }));
        path.edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
            center: Vector2::new(1.0, 0.0),
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::PI,
            counter_clockwise: true,
        }));
        path.edges.push(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(0.0, 0.0),
            end: Vector2::new(0.0, -1.0),
        }));
        let mut hatch = Hatch::new();
        hatch.paths.push(path);

        // Mirror across the Y axis (x → -x): det < 0, handedness flips.
        let t = Transform::from_scaling(Vector3::new(-1.0, 1.0, 1.0));
        transform_hatch(&mut hatch, &t);

        let edges = &hatch.paths[0].edges;
        let (l1, arc, l2) = match (&edges[0], &edges[1], &edges[2]) {
            (BoundaryEdge::Line(a), BoundaryEdge::CircularArc(b), BoundaryEdge::Line(c)) => {
                (a, b, c)
            }
            _ => panic!("edge kinds changed"),
        };
        // Stored-angle convention: true point of a CW edge is at -θ.
        let pt = |theta: f64, ccw: bool| {
            let a = if ccw { theta } else { -theta };
            (
                arc.center.x + arc.radius * a.cos(),
                arc.center.y + arc.radius * a.sin(),
            )
        };
        let (sx, sy) = pt(arc.start_angle, arc.counter_clockwise);
        let (ex, ey) = pt(arc.end_angle, arc.counter_clockwise);
        assert!(
            !arc.counter_clockwise,
            "mirror must flip the direction flag"
        );
        assert!(
            (sx - l1.end.x).abs() < 1e-9 && (sy - l1.end.y).abs() < 1e-9,
            "arc start {:?} must meet previous line end {:?}",
            (sx, sy),
            (l1.end.x, l1.end.y)
        );
        assert!(
            (ex - l2.start.x).abs() < 1e-9 && (ey - l2.start.y).abs() < 1e-9,
            "arc end {:?} must meet next line start {:?}",
            (ex, ey),
            (l2.start.x, l2.start.y)
        );
        // Midpoint sanity: the half-circle bulges DOWN after a y-axis mirror?
        // Original bulges up (+y); x-mirror keeps +y bulge at mirrored x.
        let mid_a = {
            let s = if arc.counter_clockwise {
                arc.start_angle
            } else {
                -arc.start_angle
            };
            let e = if arc.counter_clockwise {
                arc.end_angle
            } else {
                -arc.end_angle
            };
            let mut sweep = e - s;
            if arc.counter_clockwise && sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
            if !arc.counter_clockwise && sweep >= 0.0 {
                sweep -= std::f64::consts::TAU;
            }
            s + sweep / 2.0
        };
        let (mx, my) = (
            arc.center.x + arc.radius * mid_a.cos(),
            arc.center.y + arc.radius * mid_a.sin(),
        );
        assert!(
            (mx - (-1.0)).abs() < 1e-9 && (my - 1.0).abs() < 1e-9,
            "arc midpoint {:?} must be the mirror of (1,1) → (-1,1)",
            (mx, my)
        );
    }

    // Wrapped boundary sweeps stay unnormalized through transforms.
    #[test]
    fn test_hatch_arc_wrap_sweep_survives_transform() {
        use crate::entities::hatch::{BoundaryEdge, BoundaryPath, CircularArcEdge};
        use crate::types::Vector2;

        let mk = || {
            let mut path = BoundaryPath::new();
            path.edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
                center: Vector2::new(10.0, 5.0),
                radius: 61.3,
                start_angle: 5.80985,
                end_angle: 6.63571, // > 2π: wrap-encoded short arc
                counter_clockwise: false,
            }));
            let mut h = Hatch::new();
            h.paths.push(path);
            h
        };

        // Pure translation: angles must be untouched.
        let mut h = mk();
        transform_hatch(
            &mut h,
            &Transform::from_translation(Vector3::new(3.0, -2.0, 0.0)),
        );
        let BoundaryEdge::CircularArc(a) = &h.paths[0].edges[0] else {
            panic!()
        };
        assert!(
            (a.start_angle - 5.80985).abs() < 1e-9,
            "start changed: {}",
            a.start_angle
        );
        assert!(
            (a.end_angle - 6.63571).abs() < 1e-9,
            "end changed: {}",
            a.end_angle
        );
        assert!(!a.counter_clockwise);

        // Mirror: sweep magnitude must survive (0.82586), direction flag flips.
        let mut h = mk();
        transform_hatch(
            &mut h,
            &Transform::from_scaling(Vector3::new(-1.0, 1.0, 1.0)),
        );
        let BoundaryEdge::CircularArc(a) = &h.paths[0].edges[0] else {
            panic!()
        };
        assert!(a.counter_clockwise, "mirror must flip the flag");
        let sweep = a.end_angle - a.start_angle;
        assert!(
            (sweep - 0.82586).abs() < 1e-9,
            "sweep must stay 0.82586, got {sweep}"
        );
    }

    // Exploding a mirrored INSERT routes plain entities through
    // apply_transform → transform_lwpolyline. The bulge encodes which side
    // the arc bows to; a reflection must negate it or every bulged segment
    // bows the wrong way after EXPLODE.
    #[test]
    fn test_reflecting_transform_negates_lwpolyline_bulge() {
        let mut lw = LwPolyline::new();
        let mut v0 = LwVertex::new(crate::types::Vector2::new(0.0, 0.0));
        v0.bulge = 0.5;
        let mut v1 = LwVertex::new(crate::types::Vector2::new(10.0, 0.0));
        v1.bulge = -0.3;
        lw.add_vertex(v0);
        lw.add_vertex(v1);

        // Mirror across Y (x → -x): reflection, bulges must negate.
        let mut a = lw.clone();
        transform_lwpolyline(
            &mut a,
            &Transform::from_scaling(Vector3::new(-1.0, 1.0, 1.0)),
        );
        assert!((a.vertices[0].bulge - (-0.5)).abs() < 1e-12);
        assert!((a.vertices[1].bulge - 0.3).abs() < 1e-12);

        // Pure rotation (det > 0): bulges untouched.
        let mut b = lw.clone();
        transform_lwpolyline(
            &mut b,
            &Transform::from_rotation(Vector3::new(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_2),
        );
        assert!((b.vertices[0].bulge - 0.5).abs() < 1e-12);
        assert!((b.vertices[1].bulge - (-0.3)).abs() < 1e-12);
    }

    // SCALE in AutoCAD scales polyline widths with the geometry: constant
    // width (43), vertex start/end widths (40/41) and the 2D polyline's
    // default widths. Rotation and reflection keep them.
    #[test]
    fn test_transform_scales_polyline_widths() {
        let mut lw = LwPolyline::new();
        lw.constant_width = 1.0;
        let mut v0 = LwVertex::new(crate::types::Vector2::new(0.0, 0.0));
        v0.start_width = 0.5;
        v0.end_width = 2.0;
        lw.add_vertex(v0);
        lw.add_vertex(LwVertex::new(crate::types::Vector2::new(10.0, 0.0)));

        let mut pl = Polyline2D::new();
        pl.start_width = 1.0;
        pl.end_width = 1.5;
        let mut w0 = Vertex2D::new(Vector3::new(0.0, 0.0, 0.0));
        w0.start_width = 0.5;
        w0.end_width = 3.0;
        pl.add_vertex(w0);
        pl.add_vertex(Vertex2D::new(Vector3::new(10.0, 0.0, 0.0)));

        let widths = |lw: &LwPolyline, pl: &Polyline2D| {
            [
                lw.constant_width,
                lw.vertices[0].start_width,
                lw.vertices[0].end_width,
                pl.start_width,
                pl.end_width,
                pl.vertices[0].start_width,
                pl.vertices[0].end_width,
            ]
        };
        let original = widths(&lw, &pl);
        let cases = [
            (Transform::from_scale(2.0), 2.0),
            (
                Transform::from_rotation(Vector3::new(0.0, 0.0, 1.0), 1.0),
                1.0,
            ),
            (Transform::from_scaling(Vector3::new(-1.0, 1.0, 1.0)), 1.0),
        ];
        for (t, factor) in cases {
            let (mut a, mut b) = (lw.clone(), pl.clone());
            transform_lwpolyline(&mut a, &t);
            transform_polyline2d(&mut b, &t);
            for (got, was) in widths(&a, &b).iter().zip(original) {
                assert!(
                    (got - was * factor).abs() < 1e-12,
                    "{got} != {was} * {factor}"
                );
            }
        }
    }

    #[test]
    fn test_transform_multileader_scales_text_width_and_block() {
        let mut ml = MultiLeader::default();
        ml.context.text_width = 20.0;
        ml.context.text_boundary_height = 5.0;
        let t = Transform::from_scale(2.0);
        transform_multileader(&mut ml, &t);
        assert!((ml.context.text_width - 40.0).abs() < 1e-10);
        assert!((ml.context.text_boundary_height - 10.0).abs() < 1e-10);
        assert_eq!(ml.context.block_content_scale, Vector3::new(2.0, 2.0, 2.0));
        // AutoCAD keeps the entity-level block scale override.
        assert_eq!(ml.block_scale, Vector3::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn test_transform_mtext_keeps_a_unit_direction() {
        let mut m = MText::new();
        m.insertion_point = Vector3::new(0.0, 20.0, 0.0);
        m.height = 2.5;
        m.dwg_x_direction = Some(Vector3::UNIT_X);

        // SCALE 2 about the origin: height doubles, the direction stays (1, 0, 0).
        transform_mtext(&mut m, &Transform::from_scale(2.0));
        assert!((m.height - 5.0).abs() < 1e-10);
        assert_vector_near(m.dwg_x_direction.unwrap(), Vector3::UNIT_X);

        transform_mtext(
            &mut m,
            &Transform::from_rotation(Vector3::UNIT_Z, std::f64::consts::FRAC_PI_2)
                .then(&Transform::from_scale(3.0)),
        );
        assert_vector_near(m.dwg_x_direction.unwrap(), Vector3::UNIT_Y);
    }

    #[test]
    fn test_transform_minsert_keeps_negative_spacing_direction() {
        // Two columns 2 apart and two rows 3 apart, both running along -X / -Y.
        let source = Insert::new("B", Vector3::new(1.0, 1.0, 0.0)).with_array(2, 2, -2.0, -3.0);
        let transforms = [
            Transform::from_rotation(Vector3::UNIT_Z, std::f64::consts::FRAC_PI_2),
            Transform::from_scale(2.0),
            Transform::from_scaling(Vector3::new(-1.0, 1.0, 1.0)),
        ];
        for t in &transforms {
            let mut minsert = source.clone();
            transform_insert(&mut minsert, t);
            // Every array instance lands where the transform sends it.
            let expected: Vec<Vector3> =
                source.array_points().iter().map(|p| t.apply(*p)).collect();
            for (actual, expected) in minsert.array_points().iter().zip(&expected) {
                assert_vector_near(*actual, *expected);
            }
        }
    }
}
