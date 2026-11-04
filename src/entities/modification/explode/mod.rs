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

use crate::entities::*;
use crate::tables::DimStyle;
use crate::types::{Handle, Matrix3, Vector2, Vector3};

// ============================================================================
// Helpers
// ============================================================================

/// Create a new [`EntityCommon`] that inherits visual properties from *source*
/// but has [`Handle::NULL`] (the caller must allocate a real handle later).
pub(crate) fn inherit_common(source: &EntityCommon) -> EntityCommon {
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

fn arrow_solid(
    tip: Vector3,
    toward: Vector3,
    size: f64,
    half_width: f64,
    common: &EntityCommon,
    normal: Vector3,
) -> EntityType {
    let direction = (toward - tip).normalize();
    if direction.length_squared() <= 1e-20 || size <= 0.0 {
        return EntityType::Solid(Solid::triangle(tip, tip, tip));
    }
    let side = normal.normalize().cross(&direction).normalize() * half_width;
    let base = tip + direction * size;
    let mut solid = Solid::triangle(tip, base + side, base - side);
    solid.common = inherit_common(common);
    EntityType::Solid(solid)
}

fn dimension_arrow(
    tip: Vector3,
    toward: Vector3,
    size: f64,
    common: &EntityCommon,
    normal: Vector3,
) -> EntityType {
    arrow_solid(tip, toward, size, size * 0.28, common, normal)
}

fn dimension_style_arrow(
    tip: Vector3,
    toward: Vector3,
    size: f64,
    common: &EntityCommon,
    normal: Vector3,
) -> EntityType {
    // ACAD's default closed arrow is one third of its length wide.
    arrow_solid(tip, toward, size, size / 6.0, common, normal)
}

fn dimension_attachment(attachment: AttachmentPointType) -> AttachmentPoint {
    match attachment {
        AttachmentPointType::TopLeft => AttachmentPoint::TopLeft,
        AttachmentPointType::TopCenter => AttachmentPoint::TopCenter,
        AttachmentPointType::TopRight => AttachmentPoint::TopRight,
        AttachmentPointType::MiddleLeft => AttachmentPoint::MiddleLeft,
        AttachmentPointType::MiddleCenter => AttachmentPoint::MiddleCenter,
        AttachmentPointType::MiddleRight => AttachmentPoint::MiddleRight,
        AttachmentPointType::BottomLeft => AttachmentPoint::BottomLeft,
        AttachmentPointType::BottomCenter => AttachmentPoint::BottomCenter,
        AttachmentPointType::BottomRight => AttachmentPoint::BottomRight,
    }
}

fn dimension_mtext(base: &DimensionBase, value: String, style: Option<&DimStyle>) -> EntityType {
    let height = style
        .map(|style| (style.dimtxt * style.dimscale).abs())
        .filter(|height| height.is_finite() && *height > 0.0)
        .unwrap_or(2.5);
    let value = if value.starts_with("\\A") {
        value
    } else {
        // DIMENSION EXPLODE writes dimension text as MTEXT with the standard
        // vertical alignment control code.  Keeping it in the value also
        // preserves the behavior when this detached entity is written to DXF.
        format!("\\A1;{value}")
    };
    let mut text = MText::with_value(value, base.text_middle_point)
        .with_height(height)
        .with_width(0.0);
    text.common = inherit_common(&base.common);
    text.style = style
        .map(|style| style.dimtxsty.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| base.style_name.clone());
    text.rotation = base.text_rotation;
    text.normal = base.normal;
    text.attachment_point = dimension_attachment(base.attachment_point);
    EntityType::MText(Box::new(text))
}

// ============================================================================
// Per-entity explode functions
// ============================================================================

mod dimensions;
mod hatch;
mod leaders;
mod meshes;
mod modeler;
mod primitives;
mod text;

pub(crate) use dimensions::explode_dimension_with_style;
use dimensions::*;
pub(crate) use hatch::hatch_area;
use leaders::*;
use meshes::*;
use modeler::*;
use primitives::*;
pub(crate) use primitives::{nurbs_points, spline_points};
use text::*;

fn explode_mline(mline: &MLine) -> Vec<EntityType> {
    // Context-free entities do not carry their MLINESTYLE definition.  Keep
    // Standard's two canonical offsets here; the document-aware API resolves
    // custom style objects and their element colors.
    let n = mline.vertices.len();
    if n < 2 {
        return Vec::new();
    }

    let offsets =
        if mline.style_name.eq_ignore_ascii_case("Standard") && mline.style_element_count >= 2 {
            vec![0.5 * mline.scale_factor, -0.5 * mline.scale_factor]
        } else {
            vec![0.0]
        };
    let mut result = Vec::with_capacity((n - 1) * offsets.len());
    for offset in offsets {
        for i in 0..n - 1 {
            let a = &mline.vertices[i];
            let b = &mline.vertices[i + 1];
            let start = a.position + a.miter.normalize() * offset;
            let end = b.position + b.miter.normalize() * offset;
            result.push(line_entity(start, end, 0.0, mline.normal, &mline.common));
        }
    }

    // Close if the MLine is closed.
    if mline.flags.contains(MLineFlags::CLOSED) && n > 2 {
        let offsets = if mline.style_name.eq_ignore_ascii_case("Standard")
            && mline.style_element_count >= 2
        {
            vec![0.5 * mline.scale_factor, -0.5 * mline.scale_factor]
        } else {
            vec![0.0]
        };
        for offset in offsets {
            result.push(line_entity(
                mline.vertices[n - 1].position + mline.vertices[n - 1].miter.normalize() * offset,
                mline.vertices[0].position + mline.vertices[0].miter.normalize() * offset,
                0.0,
                mline.normal,
                &mline.common,
            ));
        }
    }

    result
}

fn explode_wipeout(wipeout: &Wipeout) -> Vec<EntityType> {
    let mut points = if wipeout.is_rectangular() {
        wipeout.corners().to_vec()
    } else {
        wipeout.world_boundary_vertices()
    };
    if points.len() < 2 {
        return Vec::new();
    }
    let mut result = Vec::with_capacity(points.len());
    points.push(points[0]);
    for pair in points.windows(2) {
        result.push(line_entity(
            pair[0],
            pair[1],
            0.0,
            Vector3::UNIT_Z,
            &wipeout.common,
        ));
    }
    result
}

fn explode_table(table: &Table) -> Vec<EntityType> {
    if table.rows.is_empty() || table.columns.is_empty() {
        return Vec::new();
    }
    let normal = if table.normal.length_squared() > 1e-20 {
        table.normal.normalize()
    } else {
        Vector3::UNIT_Z
    };
    let horizontal = if table.horizontal_direction.length_squared() > 1e-20 {
        table.horizontal_direction.normalize()
    } else {
        Vector3::UNIT_X
    };
    let vertical = normal.cross(&horizontal).normalize();
    let origin = table.insertion_point;
    let x: Vec<f64> = std::iter::once(0.0)
        .chain(table.columns.iter().scan(0.0, |sum, column| {
            *sum += column.width;
            Some(*sum)
        }))
        .collect();
    let y: Vec<f64> = std::iter::once(0.0)
        .chain(table.rows.iter().scan(0.0, |sum, row| {
            *sum += row.height;
            Some(*sum)
        }))
        .collect();
    let point = |u: f64, v: f64| origin + horizontal * u + vertical * v;
    let mut result = Vec::new();
    for u in &x {
        result.push(line_entity(
            point(*u, 0.0),
            point(*u, *y.last().unwrap()),
            0.0,
            normal,
            &table.common,
        ));
    }
    let title_row = table
        .rows
        .first()
        .is_some_and(|row| row.cells.iter().any(|cell| !cell.text_value().is_empty()));
    for (row_boundary, v) in y.iter().enumerate() {
        if title_row && row_boundary == 1 {
            for column in 0..x.len() - 1 {
                result.push(line_entity(
                    point(x[column], *v),
                    point(x[column + 1], *v),
                    0.0,
                    normal,
                    &table.common,
                ));
            }
        } else {
            result.push(line_entity(
                point(0.0, *v),
                point(*x.last().unwrap(), *v),
                0.0,
                normal,
                &table.common,
            ));
        }
    }
    for (row_index, row) in table.rows.iter().enumerate() {
        for (column_index, cell) in row.cells.iter().enumerate().take(table.columns.len()) {
            let value = cell.text_value();
            if value.is_empty() {
                continue;
            }
            let mut text = MText::with_value(
                value,
                point(
                    (x[column_index] + x[column_index + 1]) * 0.5,
                    (y[row_index] + y[row_index + 1]) * 0.5,
                ),
            )
            .with_height(2.5)
            .with_width((x[column_index + 1] - x[column_index]).max(0.0));
            text.common = inherit_common(&table.common);
            text.normal = normal;
            text.attachment_point = AttachmentPoint::MiddleCenter;
            result.push(EntityType::MText(Box::new(text)));
        }
    }
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
                        (-arc_edge.end_angle, -arc_edge.start_angle)
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
                    let (start_parameter, end_parameter) = if ea_edge.counter_clockwise {
                        (ea_edge.start_angle, ea_edge.end_angle)
                    } else {
                        // Clockwise HATCH edges store mirrored parameters.
                        // Reverse the geometric endpoints for CCW ELLIPSE.
                        (-ea_edge.end_angle, -ea_edge.start_angle)
                    };
                    result.push(EntityType::Ellipse(Ellipse {
                        common: inherit_common(common),
                        center: Vector3::new(ea_edge.center.x, ea_edge.center.y, elevation),
                        major_axis: Vector3::new(
                            ea_edge.major_axis_endpoint.x,
                            ea_edge.major_axis_endpoint.y,
                            0.0,
                        ),
                        minor_axis_ratio: ea_edge.minor_axis_ratio,
                        start_parameter,
                        end_parameter,
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

impl EntityType {
    /// Explode this entity using ACAD's standalone-object rules.
    ///
    /// Primitive curves and faces that ACAD refuses to explode return an
    /// empty vector. Legacy exporters can use flatten instead.
    pub fn explode(&self) -> Vec<EntityType> {
        match self {
            EntityType::Helix(helix) => {
                let mut spline = helix.spline.clone();
                spline.common = inherit_common(&helix.common);
                vec![EntityType::Spline(spline)]
            }
            EntityType::MText(e) => explode_mtext(e),
            EntityType::LwPolyline(e) => explode_lwpolyline(e),
            EntityType::Polyline2D(e) => explode_polyline2d(e),
            EntityType::Polyline(e) => explode_polyline3d(e),
            EntityType::Polyline3D(e) => explode_polyline3d_new(e),
            EntityType::Dimension(e) => explode_dimension(e),
            EntityType::Leader(e) => explode_leader(e),
            EntityType::MultiLeader(e) => explode_multileader_native(e),
            EntityType::MLine(e) => explode_mline(e),
            EntityType::Table(e) => explode_table(e),
            EntityType::Wipeout(e) => explode_wipeout(e),
            EntityType::Hatch(e) => hatch::explode_hatch(e),
            EntityType::Mesh(e) => explode_mesh_native(e),
            EntityType::PolyfaceMesh(e) => explode_polyface_mesh_native(e),
            EntityType::PolygonMesh(e) => explode_polygon_mesh_native(e),
            EntityType::Region(e) => explode_region(e),
            EntityType::Solid3D(e) => explode_solid3d(e),
            EntityType::Body(e) => explode_body(e),
            EntityType::Surface(e) => explode_surface(e),
            EntityType::Circle(_)
            | EntityType::Arc(_)
            | EntityType::Ellipse(_)
            | EntityType::Spline(_)
            | EntityType::Solid(_)
            | EntityType::Face3D(_)
            | EntityType::Point(_)
            | EntityType::Line(_)
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
            | EntityType::Tolerance(_)
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

    /// Flatten this entity into the library's historical primitive geometry.
    /// This is retained for format conversion such as R12.
    pub fn flatten(&self) -> Vec<EntityType> {
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
            EntityType::Leader(e) => flatten_leader(e),
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
mod tests;
