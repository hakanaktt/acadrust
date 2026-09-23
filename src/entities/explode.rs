//! Entity explosion — decompose complex entities into simpler primitives.
//!
//! The [`EntityType::explode`] method breaks a compound entity into its
//! constituent parts (e.g. a polyline into individual line/arc segments).
//!
//! Returned entities carry [`Handle::NULL`]; callers must assign valid
//! handles before inserting them into a document.  The convenience method
//! [`CadDocument::explode_entity`](crate::document::CadDocument::explode_entity)
//! does this automatically.

use std::f64::consts::PI;

use super::*;
use crate::types::{Handle, Vector3};

// ============================================================================
// Helpers
// ============================================================================

/// Create a new [`EntityCommon`] that inherits visual properties from *source*
/// but has [`Handle::NULL`] (the caller must allocate a real handle later).
fn inherit_common(source: &EntityCommon) -> EntityCommon {
    EntityCommon {
        handle: Handle::NULL,
        layer: source.layer.clone(),
        color: source.color,
        line_weight: source.line_weight,
        linetype: source.linetype.clone(),
        linetype_scale: source.linetype_scale,
        transparency: source.transparency,
        color_name: source.color_name.clone(),
        color_book_handle: source.color_book_handle,
        invisible: source.invisible,
        owner_handle: source.owner_handle,
        ..EntityCommon::new()
    }
}

/// Normalize an angle into the range [0, 2π).
fn normalize_angle(a: f64) -> f64 {
    let mut a = a % (2.0 * PI);
    if a < 0.0 {
        a += 2.0 * PI;
    }
    a
}

/// Given two 2-D points and a DXF *bulge* value, produce an [`Arc`] entity.
///
/// `elevation` is the Z coordinate shared by both points (OCS).
fn arc_from_bulge(
    p1x: f64,
    p1y: f64,
    p2x: f64,
    p2y: f64,
    bulge: f64,
    elevation: f64,
    thickness: f64,
    normal: Vector3,
    common: &EntityCommon,
) -> EntityType {
    let dx = p2x - p1x;
    let dy = p2y - p1y;
    let d = (dx * dx + dy * dy).sqrt();

    if d < 1e-10 {
        // Degenerate segment — return a point instead.
        return EntityType::Point(Point {
            common: inherit_common(common),
            location: Vector3::new(p1x, p1y, elevation),
            thickness,
            normal,
            x_axis_angle: 0.0,
        });
    }

    let b_sq = bulge * bulge;
    let radius = d * (1.0 + b_sq) / (4.0 * bulge.abs());

    // Perpendicular signed offset from midpoint to centre.
    let perp_offset = d * (1.0 - b_sq) / (4.0 * bulge);
    let mx = (p1x + p2x) / 2.0;
    let my = (p1y + p2y) / 2.0;
    // Left-perpendicular of chord direction.
    let px = -dy / d;
    let py = dx / d;
    let cx = mx + px * perp_offset;
    let cy = my + py * perp_offset;

    let angle_p1 = (p1y - cy).atan2(p1x - cx);
    let angle_p2 = (p2y - cy).atan2(p2x - cx);

    // DXF arcs always run CCW from start_angle to end_angle.
    let (sa, ea) = if bulge > 0.0 {
        (angle_p1, angle_p2)
    } else {
        (angle_p2, angle_p1)
    };

    EntityType::Arc(Arc {
        common: inherit_common(common),
        center: Vector3::new(cx, cy, elevation),
        radius: radius.abs(),
        start_angle: normalize_angle(sa),
        end_angle: normalize_angle(ea),
        thickness,
        normal,
    })
}

/// Build a [`Line`] entity between two 3-D points.
fn line_entity(
    start: Vector3,
    end: Vector3,
    thickness: f64,
    normal: Vector3,
    common: &EntityCommon,
) -> EntityType {
    EntityType::Line(Line {
        common: inherit_common(common),
        start,
        end,
        thickness,
        normal,
    })
}

// ============================================================================
// Per-entity explode functions
// ============================================================================

fn explode_lwpolyline(pl: &LwPolyline) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let seg_count = if pl.is_closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let v1 = &pl.vertices[i];
        let v2 = &pl.vertices[(i + 1) % n];

        if v1.bulge.abs() < 1e-10 {
            result.push(line_entity(
                Vector3::new(v1.location.x, v1.location.y, pl.elevation),
                Vector3::new(v2.location.x, v2.location.y, pl.elevation),
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

fn explode_polyline2d(pl: &Polyline2D) -> Vec<EntityType> {
    let n = pl.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let closed = pl.flags.is_closed();
    let seg_count = if closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let v1 = &pl.vertices[i];
        let v2 = &pl.vertices[(i + 1) % n];

        if v1.bulge.abs() < 1e-10 {
            result.push(line_entity(
                v1.location,
                v2.location,
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
                v1.location.z,
                pl.thickness,
                pl.normal,
                &pl.common,
            ));
        }
    }
    result
}

fn explode_polyline3d(pl: &Polyline) -> Vec<EntityType> {
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

fn explode_polyline3d_new(pl: &Polyline3D) -> Vec<EntityType> {
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

fn explode_circle(circle: &Circle) -> Vec<EntityType> {
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

fn explode_ellipse(ellipse: &Ellipse) -> Vec<EntityType> {
    // Approximate the ellipse as a spline (same approach AutoCAD uses).
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

fn explode_solid(solid: &Solid) -> Vec<EntityType> {
    // Explode a filled solid into its edge lines.
    let common = &solid.common;
    let corners = solid.boundary_corners();
    let n = corners.len();
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        result.push(line_entity(
            corners[i],
            corners[(i + 1) % n],
            solid.thickness,
            solid.normal,
            common,
        ));
    }
    result
}

fn explode_face3d(face: &Face3D) -> Vec<EntityType> {
    // Explode a 3D face into its edge lines (only visible edges).
    let common = &face.common;
    let corners = face.corners();
    let n = corners.len();
    let mut result = Vec::with_capacity(n);

    let invisible = [
        face.invisible_edges.is_first_invisible(),
        face.invisible_edges.is_second_invisible(),
        face.invisible_edges.is_third_invisible(),
        face.invisible_edges.is_fourth_invisible(),
    ];

    for i in 0..n {
        if !invisible[i] {
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

fn explode_spline(spline: &Spline) -> Vec<EntityType> {
    // Approximate the spline as a series of line segments.
    let points = if !spline.fit_points.is_empty() {
        &spline.fit_points
    } else if !spline.control_points.is_empty() {
        &spline.control_points
    } else {
        return Vec::new();
    };

    if points.len() < 2 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(points.len() - 1);
    for pair in points.windows(2) {
        result.push(line_entity(
            pair[0],
            pair[1],
            0.0,
            spline.normal,
            &spline.common,
        ));
    }

    if spline.flags.closed && points.len() > 2 {
        result.push(line_entity(
            *points.last().unwrap(),
            points[0],
            0.0,
            spline.normal,
            &spline.common,
        ));
    }

    result
}

fn explode_mtext(mtext: &MText) -> Vec<EntityType> {
    // Decompose MText into a single-line Text entity.
    let text = Text {
        common: inherit_common(&mtext.common),
        value: mtext.value.clone(),
        insertion_point: mtext.insertion_point,
        alignment_point: None,
        height: mtext.height,
        rotation: mtext.rotation,
        width_factor: 1.0,
        oblique_angle: 0.0,
        style: mtext.style.clone(),
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Baseline,
        normal: mtext.normal,
        thickness: 0.0,
        generation_flags: 0,
    };
    vec![EntityType::Text(text)]
}

fn explode_leader(leader: &Leader) -> Vec<EntityType> {
    // Decompose leader into line segments along the path.
    let n = leader.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(n - 1);
    for pair in leader.vertices.windows(2) {
        result.push(line_entity(
            pair[0],
            pair[1],
            0.0,
            leader.normal,
            &leader.common,
        ));
    }
    result
}

/// Length-to-half-width ratio of AutoCAD's default `_ClosedFilled` arrowhead:
/// a unit-long triangle whose base spans `[-1/6, 1/6]`.
const CLOSED_FILLED_HALF_WIDTH: f64 = 1.0 / 6.0;

/// Whether an arrowhead handle names a real block. A null handle is how a
/// file says "no block": the default `_ClosedFilled` arrowhead.
fn names_arrow_block(handle: Option<Handle>) -> bool {
    handle.is_some_and(|handle| handle != Handle::NULL)
}

/// The arrowhead block a leader line uses: its own when it overrides the
/// entity's, the entity's otherwise.
fn line_arrowhead_handle(ml: &MultiLeader, line: &LeaderLine) -> Option<Handle> {
    if line
        .override_flags
        .contains(LeaderLinePropertyOverrideFlags::ARROWHEAD)
    {
        line.arrowhead_handle
    } else {
        ml.arrowhead_handle
    }
}

/// The arrowhead size a leader line uses, already scaled by the annotation.
fn line_arrowhead_size(ml: &MultiLeader, line: &LeaderLine) -> f64 {
    if line
        .override_flags
        .contains(LeaderLinePropertyOverrideFlags::ARROWHEAD_SIZE)
    {
        line.arrowhead_size
    } else {
        ml.context.arrowhead_size
    }
}

/// A leader line's full path: its own vertices, arrow tip first, then the
/// root's connection point, where the landing starts. AutoCAD stores that last
/// point on the root only, so a straight leader is a single-vertex line.
fn leader_line_path(root: &LeaderRoot, line: &LeaderLine) -> Vec<Vector3> {
    let mut path = line.points.clone();

    if path
        .last()
        .is_some_and(|last| last.distance(&root.connection_point) > f64::EPSILON)
    {
        path.push(root.connection_point);
    }

    path
}

/// The default `_ClosedFilled` arrowhead at the start of `path`, pointing
/// away from the second vertex, and the point where the shaft now starts.
///
/// No arrowhead when the size is not positive or when the first segment is
/// shorter than the arrow itself, which is also what keeps the shaft from
/// being trimmed past its own end.
fn closed_filled_arrowhead(
    path: &[Vector3],
    size: f64,
    common: &EntityCommon,
) -> Option<(EntityType, Vector3)> {
    let (tip, next) = (*path.first()?, *path.get(1)?);
    let first_segment = tip - next;
    let length = first_segment.length();

    if size <= 0.0 || length < size {
        return None;
    }

    let direction = first_segment * (1.0 / length);
    let normal = Vector3::new(-direction.y, direction.x, 0.0) * (size * CLOSED_FILLED_HALF_WIDTH);
    let base = tip - direction * size;
    let mut solid = Solid::triangle(tip, base + normal, base - normal);
    solid.common = inherit_common(common);

    Some((EntityType::Solid(solid), base))
}

/// The landing: from the connection point, along the root's direction, for
/// the landing distance. Nothing when the dogleg is disabled, has no length,
/// or has no direction.
fn landing_line(ml: &MultiLeader, root: &LeaderRoot) -> Option<EntityType> {
    let direction_length = root.direction.length();

    if !ml.enable_dogleg || root.landing_distance <= 0.0 || direction_length <= f64::EPSILON {
        return None;
    }

    let end = root.connection_point + root.direction * (root.landing_distance / direction_length);

    Some(line_entity(
        root.connection_point,
        end,
        0.0,
        Vector3::UNIT_Z,
        &ml.common,
    ))
}

/// Explode one leader line: its arrowhead (default arrowhead only: a custom
/// block is left to the caller, which knows the document) and its segments.
fn explode_leader_line(
    ml: &MultiLeader,
    root: &LeaderRoot,
    line: &LeaderLine,
    result: &mut Vec<EntityType>,
) {
    let mut path = leader_line_path(root, line);
    let arrowhead = if names_arrow_block(line_arrowhead_handle(ml, line)) {
        None
    } else {
        closed_filled_arrowhead(&path, line_arrowhead_size(ml, line), &ml.common)
    };

    if let Some((solid, base)) = arrowhead {
        result.push(solid);
        path[0] = base;
    }

    for pair in path.windows(2) {
        result.push(line_entity(
            pair[0],
            pair[1],
            0.0,
            Vector3::UNIT_Z,
            &ml.common,
        ));
    }
}

fn explode_multileader(ml: &MultiLeader) -> Vec<EntityType> {
    // Decompose MultiLeader the way AutoCAD's EXPLODE does: per leader line,
    // the arrowhead and the segments down to the root's connection point;
    // per root, the landing; then the text.
    let mut result = Vec::new();

    if ml.path_type != MultiLeaderPathType::Invisible {
        for root in &ml.context.leader_roots {
            for line in &root.lines {
                explode_leader_line(ml, root, line, &mut result);
            }

            result.extend(landing_line(ml, root));
        }
    }

    // Add text content if available.
    if ml.context.has_text_contents && !ml.context.text_string.is_empty() {
        let text = MText {
            common: inherit_common(&ml.common),
            value: ml.context.text_string.clone(),
            insertion_point: ml.context.text_location,
            height: ml.context.text_height,
            rectangle_width: ml.context.text_width,
            rectangle_height: None,
            rotation: ml.context.text_rotation,
            style: "Standard".to_string(),
            attachment_point: AttachmentPoint::TopLeft,
            drawing_direction: DrawingDirection::LeftToRight,
            line_spacing_factor: ml.context.line_spacing_factor,
            normal: ml.context.text_normal,
            ..MText::new()
        };
        result.push(EntityType::MText(text));
    }

    result
}

fn explode_mline(mline: &MLine) -> Vec<EntityType> {
    // Decompose MLine into line segments (centerline path).
    let n = mline.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        result.push(line_entity(
            mline.vertices[i].position,
            mline.vertices[i + 1].position,
            0.0,
            mline.normal,
            &mline.common,
        ));
    }

    // Close if the MLine is closed.
    if mline.flags.contains(MLineFlags::CLOSED) && n > 2 {
        result.push(line_entity(
            mline.vertices[n - 1].position,
            mline.vertices[0].position,
            0.0,
            mline.normal,
            &mline.common,
        ));
    }

    result
}

fn explode_dimension(dim: &Dimension) -> Vec<EntityType> {
    // Simplified decomposition: produce extension lines, dimension line,
    // and a text entity with the measurement value.
    let base = dim.base();
    let common = &base.common;

    let text_value = base
        .text_override()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{:.4}", dim.measurement()));

    let mut result = Vec::new();

    match dim {
        Dimension::Linear(d) => {
            // Extension lines from definition points to dimension line.
            result.push(line_entity(
                d.first_point,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.second_point,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
            // Dimension line between first and second projected points.
            result.push(line_entity(
                d.first_point,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Aligned(d) => {
            result.push(line_entity(
                d.first_point,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.second_point,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.first_point,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Radius(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Diameter(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Angular2Ln(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.first_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.angle_vertex,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Angular3Pt(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.first_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.angle_vertex,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Ordinate(d) => {
            let points = d.leader_polyline(0.0, 0.0, None);
            for pair in points.windows(2) {
                if (pair[1] - pair[0]).length() > 1e-12 {
                    result.push(line_entity(pair[0], pair[1], 0.0, base.normal, common));
                }
            }
        }
        Dimension::Arc(d) => {
            result.push(line_entity(
                d.center_point,
                d.first_extension_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.center_point,
                d.second_extension_point,
                0.0,
                base.normal,
                common,
            ));
            if d.has_leader {
                result.push(line_entity(
                    d.first_leader_point,
                    d.second_leader_point,
                    0.0,
                    base.normal,
                    common,
                ));
            }
        }
        Dimension::LargeRadial(d) => {
            result.push(line_entity(
                d.definition_point,
                d.jog_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.jog_point,
                d.chord_point,
                0.0,
                base.normal,
                common,
            ));
        }
    }

    // Add the dimension text.
    let text = Text {
        common: inherit_common(common),
        value: text_value,
        insertion_point: base.text_middle_point,
        alignment_point: None,
        height: 2.5, // default text height
        rotation: base.text_rotation,
        width_factor: 1.0,
        oblique_angle: 0.0,
        style: base.style_name.clone(),
        horizontal_alignment: TextHorizontalAlignment::Center,
        vertical_alignment: TextVerticalAlignment::Middle,
        normal: base.normal,
        thickness: 0.0,
        generation_flags: 0,
    };
    result.push(EntityType::Text(text));

    result
}

fn explode_hatch(hatch: &Hatch) -> Vec<EntityType> {
    // Decompose hatch boundary paths into line/arc entities.
    let common = &hatch.common;
    let elevation = hatch.elevation;
    let normal = hatch.normal;
    let mut result = Vec::new();

    for path in &hatch.paths {
        for edge in &path.edges {
            match edge {
                BoundaryEdge::Line(le) => {
                    result.push(line_entity(
                        Vector3::new(le.start.x, le.start.y, elevation),
                        Vector3::new(le.end.x, le.end.y, elevation),
                        0.0,
                        normal,
                        common,
                    ));
                }
                BoundaryEdge::CircularArc(arc_edge) => {
                    let (sa, ea) = if arc_edge.counter_clockwise {
                        (arc_edge.start_angle, arc_edge.end_angle)
                    } else {
                        (arc_edge.end_angle, arc_edge.start_angle)
                    };
                    result.push(EntityType::Arc(Arc {
                        common: inherit_common(common),
                        center: Vector3::new(arc_edge.center.x, arc_edge.center.y, elevation),
                        radius: arc_edge.radius,
                        start_angle: normalize_angle(sa),
                        end_angle: normalize_angle(ea),
                        thickness: 0.0,
                        normal,
                    }));
                }
                BoundaryEdge::EllipticArc(ea_edge) => {
                    result.push(EntityType::Ellipse(Ellipse {
                        common: inherit_common(common),
                        center: Vector3::new(ea_edge.center.x, ea_edge.center.y, elevation),
                        major_axis: Vector3::new(
                            ea_edge.major_axis_endpoint.x,
                            ea_edge.major_axis_endpoint.y,
                            0.0,
                        ),
                        minor_axis_ratio: ea_edge.minor_axis_ratio,
                        start_parameter: ea_edge.start_angle,
                        end_parameter: ea_edge.end_angle,
                        normal,
                    }));
                }
                BoundaryEdge::Spline(sp_edge) => {
                    let control_points: Vec<Vector3> = sp_edge
                        .control_points
                        .iter()
                        .map(|cp| Vector3::new(cp.x, cp.y, elevation))
                        .collect();
                    if control_points.len() >= 2 {
                        let mut spline =
                            Spline::from_control_points(sp_edge.degree, control_points);
                        spline.common = inherit_common(common);
                        spline.flags.rational = sp_edge.rational;
                        spline.flags.periodic = sp_edge.periodic;
                        spline.knots = sp_edge.knots.clone();
                        result.push(EntityType::Spline(spline));
                    }
                }
                BoundaryEdge::Polyline(pl_edge) => {
                    // Each vertex has (x, y, bulge) stored in Vector3.
                    let verts = &pl_edge.vertices;
                    let n = verts.len();
                    let seg_count = if pl_edge.is_closed {
                        n
                    } else {
                        n.saturating_sub(1)
                    };
                    for i in 0..seg_count {
                        let v1 = &verts[i];
                        let v2 = &verts[(i + 1) % n];
                        let bulge = v1.z; // bulge stored in z component
                        if bulge.abs() < 1e-10 {
                            result.push(line_entity(
                                Vector3::new(v1.x, v1.y, elevation),
                                Vector3::new(v2.x, v2.y, elevation),
                                0.0,
                                normal,
                                common,
                            ));
                        } else {
                            result.push(arc_from_bulge(
                                v1.x, v1.y, v2.x, v2.y, bulge, elevation, 0.0, normal, common,
                            ));
                        }
                    }
                }
            }
        }
    }

    result
}

fn explode_mesh(mesh: &Mesh) -> Vec<EntityType> {
    // Decompose Mesh into Face3D entities (one per face).
    let common = &mesh.common;
    let mut result = Vec::with_capacity(mesh.faces.len());

    for face in &mesh.faces {
        let n = face.vertices.len();
        if n < 3 {
            continue;
        }
        let v = &face.vertices;
        let get =
            |idx: usize| -> Vector3 { mesh.vertices.get(idx).copied().unwrap_or(Vector3::ZERO) };

        if n == 3 {
            result.push(EntityType::Face3D(Face3D::triangle(
                get(v[0]),
                get(v[1]),
                get(v[2]),
            )));
        } else {
            // For quads and n-gons, fan-triangulate from vertex 0.
            for i in 1..n - 1 {
                let mut face3d = Face3D::triangle(get(v[0]), get(v[i]), get(v[i + 1]));
                face3d.common = inherit_common(common);
                result.push(EntityType::Face3D(face3d));
            }
            continue;
        }

        // Set common for properly constructed faces.
        if let Some(EntityType::Face3D(ref mut f)) = result.last_mut() {
            f.common = inherit_common(common);
        }
    }

    result
}

fn explode_polyface_mesh(mesh: &PolyfaceMesh) -> Vec<EntityType> {
    // Each face references vertices by 1-based index.
    let common = &mesh.common;
    let mut result = Vec::with_capacity(mesh.faces.len());

    let get_vertex = |idx: i16| -> Vector3 {
        let abs_idx = idx.unsigned_abs() as usize;
        if abs_idx == 0 || abs_idx > mesh.vertices.len() {
            Vector3::ZERO
        } else {
            mesh.vertices[abs_idx - 1].location
        }
    };

    for face in &mesh.faces {
        let p1 = get_vertex(face.index1);
        let p2 = get_vertex(face.index2);
        let p3 = get_vertex(face.index3);

        let mut face3d = if face.is_triangle() {
            Face3D::triangle(p1, p2, p3)
        } else {
            let p4 = get_vertex(face.index4);
            Face3D::new(p1, p2, p3, p4)
        };

        // Mark invisible edges from face index signs.
        if face.index1 < 0 {
            face3d.invisible_edges.set_first_invisible(true);
        }
        if face.index2 < 0 {
            face3d.invisible_edges.set_second_invisible(true);
        }
        if face.index3 < 0 {
            face3d.invisible_edges.set_third_invisible(true);
        }
        if face.index4 < 0 {
            face3d.invisible_edges.set_fourth_invisible(true);
        }

        face3d.common = inherit_common(common);
        result.push(EntityType::Face3D(face3d));
    }

    result
}

fn explode_polygon_mesh(mesh: &PolygonMeshEntity) -> Vec<EntityType> {
    // Decompose a polygon mesh grid (M × N) into Face3D entities.
    let common = &mesh.common;
    let m = mesh.m_vertex_count as usize;
    let n = mesh.n_vertex_count as usize;
    if m < 2 || n < 2 || mesh.vertices.len() < m * n {
        return Vec::new();
    }

    let get = |mi: usize, ni: usize| -> Vector3 { mesh.vertices[mi * n + ni].location };

    let mut result = Vec::with_capacity((m - 1) * (n - 1));
    for i in 0..m - 1 {
        for j in 0..n - 1 {
            let mut face = Face3D::new(get(i, j), get(i, j + 1), get(i + 1, j + 1), get(i + 1, j));
            face.common = inherit_common(common);
            result.push(EntityType::Face3D(face));
        }
    }

    result
}

// ============================================================================
// EntityType::explode
// ============================================================================

impl EntityType {
    /// Explode this entity into simpler constituent entities.
    ///
    /// Complex entities (polylines, hatches, meshes, etc.) are decomposed
    /// into primitives such as [`Line`], [`Arc`], [`Face3D`], and [`Text`].
    ///
    /// Atomic entities that cannot be further decomposed return an **empty**
    /// `Vec`.
    ///
    /// All returned entities have [`Handle::NULL`].  Use
    /// [`CadDocument::explode_entity`](crate::document::CadDocument::explode_entity)
    /// to obtain entities with properly allocated handles.
    pub fn explode(&self) -> Vec<EntityType> {
        match self {
            // Complex entities that decompose.
            EntityType::Circle(e) => explode_circle(e),
            EntityType::Ellipse(e) => explode_ellipse(e),
            EntityType::LwPolyline(e) => explode_lwpolyline(e),
            EntityType::Polyline2D(e) => explode_polyline2d(e),
            EntityType::Polyline(e) => explode_polyline3d(e),
            EntityType::Polyline3D(e) => explode_polyline3d_new(e),
            EntityType::Solid(e) => explode_solid(e),
            EntityType::Face3D(e) => explode_face3d(e),
            EntityType::Spline(e) => explode_spline(e),
            EntityType::Helix(e) => explode_spline(&e.spline),
            EntityType::MText(e) => explode_mtext(e),
            EntityType::Dimension(e) => explode_dimension(e),
            EntityType::Leader(e) => explode_leader(e),
            EntityType::MultiLeader(e) => explode_multileader(e),
            EntityType::MLine(e) => explode_mline(e),
            EntityType::Hatch(e) => explode_hatch(e),
            EntityType::Mesh(e) => explode_mesh(e),
            EntityType::PolyfaceMesh(e) => explode_polyface_mesh(e),
            EntityType::PolygonMesh(e) => explode_polygon_mesh(e),

            // Atomic / non-decomposable entities.
            EntityType::Point(_)
            | EntityType::Line(_)
            | EntityType::Arc(_)
            | EntityType::Text(_)
            | EntityType::Ray(_)
            | EntityType::XLine(_)
            | EntityType::Viewport(_)
            | EntityType::Block(_)
            | EntityType::BlockEnd(_)
            | EntityType::Seqend(_)
            | EntityType::Insert(_)
            | EntityType::AttributeDefinition(_)
            | EntityType::AttributeEntity(_)
            | EntityType::RasterImage(_)
            | EntityType::Solid3D(_)
            | EntityType::Region(_)
            | EntityType::Body(_)
            | EntityType::Surface(_)
            | EntityType::Table(_)
            | EntityType::Tolerance(_)
            | EntityType::Wipeout(_)
            | EntityType::Shape(_)
            | EntityType::Underlay(_)
            | EntityType::Ole2Frame(_)
            | EntityType::Light(_)
            | EntityType::SectionSymbol(_)
            | EntityType::ViewBorder(_)
            | EntityType::Extended(_)
            | EntityType::Unknown(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Vector2;

    #[test]
    fn test_atomic_entities_return_empty() {
        let line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 1.0, 0.0));
        assert!(line.explode().is_empty());

        let arc = EntityType::Arc(Arc::new());
        assert!(arc.explode().is_empty());

        let point = EntityType::Point(Point::new());
        assert!(point.explode().is_empty());

        let text = EntityType::Text(Text::new());
        assert!(text.explode().is_empty());
    }

    #[test]
    fn test_explode_circle() {
        let circle = Circle::from_center_radius(Vector3::new(5.0, 5.0, 0.0), 10.0);
        let entity = EntityType::Circle(circle);
        let parts = entity.explode();
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            EntityType::Arc(arc) => {
                assert_eq!(arc.center, Vector3::new(5.0, 5.0, 0.0));
                assert!((arc.radius - 10.0).abs() < 1e-10);
                assert!((arc.start_angle - 0.0).abs() < 1e-10);
                assert!((arc.end_angle - 2.0 * PI).abs() < 1e-10);
            }
            _ => panic!("Expected Arc from circle explosion"),
        }
    }

    #[test]
    fn test_explode_lwpolyline_lines_only() {
        let mut pl = LwPolyline::new();
        pl.add_point(Vector2::new(0.0, 0.0));
        pl.add_point(Vector2::new(10.0, 0.0));
        pl.add_point(Vector2::new(10.0, 10.0));

        let entity = EntityType::LwPolyline(pl);
        let parts = entity.explode();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], EntityType::Line(_)));
        assert!(matches!(&parts[1], EntityType::Line(_)));
    }

    #[test]
    fn test_explode_lwpolyline_closed() {
        let mut pl = LwPolyline::new();
        pl.add_point(Vector2::new(0.0, 0.0));
        pl.add_point(Vector2::new(10.0, 0.0));
        pl.add_point(Vector2::new(10.0, 10.0));
        pl.close();

        let entity = EntityType::LwPolyline(pl);
        let parts = entity.explode();
        assert_eq!(parts.len(), 3); // 3 segments for closed triangle
    }

    #[test]
    fn test_explode_lwpolyline_with_bulge() {
        let mut pl = LwPolyline::new();
        pl.add_point_with_bulge(Vector2::new(0.0, 0.0), 1.0); // semicircle
        pl.add_point(Vector2::new(10.0, 0.0));

        let entity = EntityType::LwPolyline(pl);
        let parts = entity.explode();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Arc(_)));
    }

    #[test]
    fn test_explode_solid_quad() {
        let first = Vector3::new(0.0, 0.0, 0.0);
        let second = Vector3::new(10.0, 0.0, 0.0);
        let third = Vector3::new(0.0, 10.0, 0.0);
        let fourth = Vector3::new(10.0, 10.0, 0.0);
        let solid = Solid::new(first, second, third, fourth);
        let entity = EntityType::Solid(solid);
        let parts = entity.explode();
        let expected = [
            (first, second),
            (second, fourth),
            (fourth, third),
            (third, first),
        ];
        assert_eq!(parts.len(), expected.len());
        for (part, (start, end)) in parts.iter().zip(expected) {
            let EntityType::Line(line) = part else {
                panic!("expected line")
            };
            assert_eq!((line.start, line.end), (start, end));
        }
    }

    #[test]
    fn test_explode_solid_triangle() {
        let solid = Solid::triangle(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(5.0, 10.0, 0.0),
        );
        let entity = EntityType::Solid(solid);
        let parts = entity.explode();
        assert_eq!(parts.len(), 3); // 3 edge lines
    }

    #[test]
    fn test_explode_face3d() {
        let face = Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(10.0, 10.0, 0.0),
            Vector3::new(0.0, 10.0, 0.0),
        );
        let entity = EntityType::Face3D(face);
        let parts = entity.explode();
        assert_eq!(parts.len(), 4); // All edges visible
    }

    #[test]
    fn test_explode_face3d_invisible_edges() {
        let mut face = Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(10.0, 10.0, 0.0),
            Vector3::new(0.0, 10.0, 0.0),
        );
        face.invisible_edges.set_first_invisible(true);
        face.invisible_edges.set_third_invisible(true);

        let entity = EntityType::Face3D(face);
        let parts = entity.explode();
        assert_eq!(parts.len(), 2); // Only 2 visible edges
    }

    #[test]
    fn test_explode_polyline3d() {
        let pl = Polyline::from_points(vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(10.0, 10.0, 0.0),
        ]);
        let entity = EntityType::Polyline(pl);
        let parts = entity.explode();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], EntityType::Line(_)));
    }

    #[test]
    fn test_explode_leader() {
        let mut leader = Leader::new();
        leader.vertices.push(Vector3::new(0.0, 0.0, 0.0));
        leader.vertices.push(Vector3::new(10.0, 10.0, 0.0));
        leader.vertices.push(Vector3::new(20.0, 10.0, 0.0));

        let entity = EntityType::Leader(leader);
        let parts = entity.explode();
        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], EntityType::Line(_)));
    }

    /// A callout the way AutoCAD stores it: the leader line holds only the
    /// arrow tip, the root holds the bend (connection point) and the landing.
    fn callout(tips: &[Vector3]) -> MultiLeader {
        let mut ml = MultiLeader::new();
        ml.context.arrowhead_size = 1.0;
        let mut root = LeaderRoot::new(0);
        root.connection_point = Vector3::new(10.0, 10.0, 0.0);
        root.direction = Vector3::new(2.0, 0.0, 0.0);
        root.landing_distance = 5.0;

        for (index, tip) in tips.iter().enumerate() {
            let mut line = LeaderLine::new(index as i32);
            line.points.push(*tip);
            root.lines.push(line);
        }

        ml.context.leader_roots.push(root);
        ml
    }

    fn line_ends(parts: &[EntityType]) -> Vec<(Vector3, Vector3)> {
        parts
            .iter()
            .filter_map(|part| match part {
                EntityType::Line(line) => Some((line.start, line.end)),
                _ => None,
            })
            .collect()
    }

    fn solids(parts: &[EntityType]) -> Vec<&Solid> {
        parts
            .iter()
            .filter_map(|part| match part {
                EntityType::Solid(solid) => Some(solid),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_explode_multileader_single_vertex_line() {
        let parts = EntityType::MultiLeader(callout(&[Vector3::ZERO])).explode();
        let arrows = solids(&parts);
        let lines = line_ends(&parts);

        // The arrowhead: tip at the first point, one unit long, 1/3 wide.
        assert_eq!(arrows.len(), 1);
        let base = Vector3::new(1.0, 1.0, 0.0) * (1.0 / 2f64.sqrt());
        assert_eq!(arrows[0].first_corner, Vector3::ZERO);
        let half_width = arrows[0].second_corner.distance(&arrows[0].third_corner) / 2.0;
        assert!((half_width - 1.0 / 6.0).abs() < 1e-9);
        assert!(((arrows[0].second_corner + arrows[0].third_corner) * 0.5).distance(&base) < 1e-9);

        // The shaft starts at the arrow base and reaches the connection
        // point; the landing follows the normalized direction.
        assert_eq!(lines.len(), 2);
        assert!(lines[0].0.distance(&base) < 1e-9);
        assert_eq!(lines[0].1, Vector3::new(10.0, 10.0, 0.0));
        assert_eq!(
            lines[1],
            (Vector3::new(10.0, 10.0, 0.0), Vector3::new(15.0, 10.0, 0.0))
        );
    }

    #[test]
    fn test_explode_multileader_does_not_repeat_the_connection_point() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.context.arrowhead_size = 0.0;
        ml.context.leader_roots[0].lines[0]
            .points
            .extend([Vector3::new(5.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)]);

        let lines = line_ends(&EntityType::MultiLeader(ml).explode());

        assert_eq!(
            lines,
            vec![
                (Vector3::ZERO, Vector3::new(5.0, 0.0, 0.0)),
                (Vector3::new(5.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)),
                (Vector3::new(10.0, 10.0, 0.0), Vector3::new(15.0, 10.0, 0.0)),
            ]
        );
    }

    #[test]
    fn test_explode_multileader_lines_share_one_landing() {
        let ml = callout(&[Vector3::ZERO, Vector3::new(20.0, 0.0, 0.0)]);
        let parts = EntityType::MultiLeader(ml).explode();

        // One arrowhead per leader line, never one on the landing.
        assert_eq!(solids(&parts).len(), 2);
        let landings = line_ends(&parts)
            .into_iter()
            .filter(|(start, _)| *start == Vector3::new(10.0, 10.0, 0.0))
            .count();
        assert_eq!(landings, 1);
    }

    #[test]
    fn test_explode_multileader_each_root_draws_its_own_path() {
        let mut ml = callout(&[Vector3::ZERO]);
        let mut second = ml.context.leader_roots[0].clone();
        second.connection_point = Vector3::new(30.0, 10.0, 0.0);
        ml.context.leader_roots.push(second);

        let parts = EntityType::MultiLeader(ml).explode();

        assert_eq!(solids(&parts).len(), 2);
        assert_eq!(line_ends(&parts).len(), 4);
    }

    #[test]
    fn test_explode_multileader_invisible_path_keeps_only_the_text() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.path_type = MultiLeaderPathType::Invisible;
        ml.context.has_text_contents = true;
        ml.context.text_string = "NOTE".to_string();

        let parts = EntityType::MultiLeader(ml).explode();

        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::MText(_)));
    }

    #[test]
    fn test_explode_multileader_without_dogleg_has_no_landing() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.enable_dogleg = false;

        let lines = line_ends(&EntityType::MultiLeader(ml).explode());

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].1, Vector3::new(10.0, 10.0, 0.0));
    }

    #[test]
    fn test_explode_multileader_custom_arrow_block_is_not_drawn_as_the_default() {
        let mut entity_level = callout(&[Vector3::ZERO]);
        entity_level.arrowhead_handle = Some(Handle::new(0x2A));

        let mut line_level = callout(&[Vector3::ZERO]);
        let line = &mut line_level.context.leader_roots[0].lines[0];
        line.arrowhead_handle = Some(Handle::new(0x2A));
        line.override_flags = LeaderLinePropertyOverrideFlags::ARROWHEAD;

        for ml in [entity_level, line_level] {
            let parts = EntityType::MultiLeader(ml).explode();
            assert!(solids(&parts).is_empty());
            // The shaft is not trimmed when no arrowhead covers its start.
            assert_eq!(line_ends(&parts)[0].0, Vector3::ZERO);
        }
    }

    #[test]
    fn test_explode_multileader_null_arrow_handle_is_the_default_arrow() {
        let mut ml = callout(&[Vector3::ZERO]);
        ml.arrowhead_handle = Some(Handle::NULL);

        assert_eq!(solids(&EntityType::MultiLeader(ml).explode()).len(), 1);
    }

    #[test]
    fn test_explode_multileader_line_overrides_the_arrow_size() {
        let mut ml = callout(&[Vector3::ZERO]);
        let line = &mut ml.context.leader_roots[0].lines[0];
        line.arrowhead_size = 2.0;
        line.override_flags = LeaderLinePropertyOverrideFlags::ARROWHEAD_SIZE;

        let parts = EntityType::MultiLeader(ml).explode();
        let base = (solids(&parts)[0].second_corner + solids(&parts)[0].third_corner) * 0.5;

        assert!((base.length() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_explode_multileader_arrow_longer_than_its_segment_is_dropped() {
        let mut ml = callout(&[Vector3::new(9.5, 10.0, 0.0)]);
        ml.context.arrowhead_size = 1.0;

        let parts = EntityType::MultiLeader(ml).explode();

        assert!(solids(&parts).is_empty());
        assert_eq!(line_ends(&parts)[0].0, Vector3::new(9.5, 10.0, 0.0));
    }

    #[test]
    fn test_explode_mtext() {
        let mtext = MText::with_value("Hello World", Vector3::new(5.0, 5.0, 0.0));
        let entity = EntityType::MText(mtext);
        let parts = entity.explode();
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            EntityType::Text(t) => {
                assert_eq!(t.value, "Hello World");
                assert_eq!(t.insertion_point, Vector3::new(5.0, 5.0, 0.0));
            }
            _ => panic!("Expected Text from MText explosion"),
        }
    }

    #[test]
    fn test_explode_mesh_triangles() {
        let mesh = Mesh::from_triangles(
            vec![
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 0.0),
                Vector3::new(5.0, 10.0, 0.0),
            ],
            &[(0, 1, 2)],
        );
        let entity = EntityType::Mesh(mesh);
        let parts = entity.explode();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Face3D(_)));
    }

    #[test]
    fn test_explode_ellipse() {
        let ellipse = Ellipse::from_center_axes(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0), 0.5);
        let entity = EntityType::Ellipse(ellipse);
        let parts = entity.explode();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Spline(_)));
    }

    #[test]
    fn test_explode_inherits_common_properties() {
        let mut circle = Circle::from_center_radius(Vector3::ZERO, 5.0);
        circle.common.layer = "WALLS".to_string();
        circle.common.color = Color::Index(1);
        circle.common.linetype = "DASHED".to_string();

        let entity = EntityType::Circle(circle);
        let parts = entity.explode();
        assert_eq!(parts.len(), 1);
        let common = parts[0].common();
        assert_eq!(common.layer, "WALLS");
        assert_eq!(common.color, Color::Index(1));
        assert_eq!(common.linetype, "DASHED");
        assert!(common.handle.is_null()); // handle not assigned yet
    }

    #[test]
    fn test_explode_spline() {
        let pts = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(3.0, 10.0, 0.0),
            Vector3::new(7.0, 10.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
        ];
        let spline = Spline::from_control_points(3, pts);
        let entity = EntityType::Spline(spline);
        let parts = entity.explode();
        assert_eq!(parts.len(), 3); // 3 line segments from 4 control points
        assert!(matches!(&parts[0], EntityType::Line(_)));
    }

    #[test]
    fn test_explode_dimension() {
        let dim = DimensionLinear::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));
        let entity = EntityType::Dimension(Dimension::Linear(dim));
        let parts = entity.explode();
        // Should contain dimension lines + text
        assert!(parts.len() >= 2);
        assert!(parts.iter().any(|p| matches!(p, EntityType::Text(_))));
        assert!(parts.iter().any(|p| matches!(p, EntityType::Line(_))));
    }

    #[test]
    fn test_explode_polyface_mesh() {
        let mut mesh = PolyfaceMesh::new();
        mesh.add_vertex(PolyfaceVertex::new(Vector3::new(0.0, 0.0, 0.0)));
        mesh.add_vertex(PolyfaceVertex::new(Vector3::new(10.0, 0.0, 0.0)));
        mesh.add_vertex(PolyfaceVertex::new(Vector3::new(5.0, 10.0, 0.0)));
        mesh.add_face(PolyfaceFace::triangle(1, 2, 3));

        let entity = EntityType::PolyfaceMesh(mesh);
        let parts = entity.explode();
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], EntityType::Face3D(_)));
    }

    #[test]
    fn test_document_explode_entity_assigns_handles() {
        use crate::document::CadDocument;

        let mut doc = CadDocument::new();
        let mut circle = Circle::from_center_radius(Vector3::ZERO, 5.0);
        circle.common.owner_handle = Handle::new(0x100);

        let entity = EntityType::Circle(circle);
        let parts = doc.explode_entity(&entity);
        assert_eq!(parts.len(), 1);
        assert!(parts[0].common().handle.is_valid());
        assert_eq!(parts[0].common().owner_handle, Handle::new(0x100));
    }
}
