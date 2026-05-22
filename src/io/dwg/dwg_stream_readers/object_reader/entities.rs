//! Entity readers for DWG object section.
//!
//! Each reader is the exact inverse of the corresponding writer in
//! `dwg_stream_writers/object_writer/entities.rs`. They read entity-specific
//! fields after common entity data has already been parsed.

use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use crate::io::dwg::dwg_version::DwgVersion;
use crate::types::{Color, Handle, Vector2, Vector3, DxfVersion};
use crate::entities::multileader::*;
use crate::entities::solid3d::{Wire, WireType, Silhouette};
use super::safe_count;

// ════════════════════════════════════════════════════════════════════════
//  Result structs
// ════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointData {
    pub location: Vector3,
    pub thickness: f64,
    pub normal: Vector3,
    pub x_axis_angle: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LineData {
    pub start: Vector3,
    pub end: Vector3,
    pub thickness: f64,
    pub normal: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CircleData {
    pub center: Vector3,
    pub radius: f64,
    pub thickness: f64,
    pub normal: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcData {
    pub center: Vector3,
    pub radius: f64,
    pub thickness: f64,
    pub normal: Vector3,
    pub start_angle: f64,
    pub end_angle: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EllipseData {
    pub center: Vector3,
    pub major_axis: Vector3,
    pub normal: Vector3,
    pub minor_axis_ratio: f64,
    pub start_parameter: f64,
    pub end_parameter: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RayData {
    pub base_point: Vector3,
    pub direction: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct XLineData {
    pub base_point: Vector3,
    pub direction: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SolidData {
    pub thickness: f64,
    pub elevation: f64,
    pub first_corner: Vector2,
    pub second_corner: Vector2,
    pub third_corner: Vector2,
    pub fourth_corner: Vector2,
    pub normal: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Face3DData {
    pub first_corner: Vector3,
    pub second_corner: Vector3,
    pub third_corner: Vector3,
    pub fourth_corner: Vector3,
    pub invisible_edges: i16,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InsertData {
    pub insert_point: Vector3,
    pub x_scale: f64,
    pub y_scale: f64,
    pub z_scale: f64,
    pub rotation: f64,
    pub normal: Vector3,
    pub has_attribs: bool,
    pub block_handle: u64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MInsertData {
    pub insert: InsertData,
    pub column_count: i16,
    pub row_count: i16,
    pub column_spacing: f64,
    pub row_spacing: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LwPolylineVertex {
    pub x: f64,
    pub y: f64,
    pub bulge: f64,
    pub start_width: f64,
    pub end_width: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LwPolylineData {
    pub flag: i16,
    pub constant_width: f64,
    pub elevation: f64,
    pub thickness: f64,
    pub normal: Vector3,
    pub vertices: Vec<LwPolylineVertex>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SplineData {
    pub scenario: i32,
    pub degree: i32,
    pub rational: bool,
    pub closed: bool,
    pub periodic: bool,
    pub knot_tolerance: f64,
    pub control_tolerance: f64,
    pub knots: Vec<f64>,
    pub control_points: Vec<Vector3>,
    pub weights: Vec<f64>,
    pub fit_tolerance: f64,
    pub begin_tangent: Vector3,
    pub end_tangent: Vector3,
    pub fit_points: Vec<Vector3>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextEntityData {
    pub insertion_point: Vector3,
    pub alignment_point: Vector3,
    pub normal: Vector3,
    pub thickness: f64,
    pub oblique_angle: f64,
    pub rotation: f64,
    pub height: f64,
    pub width_factor: f64,
    pub value: String,
    pub generation: i16,
    pub horizontal_alignment: i16,
    pub vertical_alignment: i16,
    pub style_handle: u64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MTextData {
    pub insertion_point: Vector3,
    pub normal: Vector3,
    pub x_direction: Vector3,
    pub rectangle_width: f64,
    pub rectangle_height: f64,
    pub height: f64,
    pub attachment_point: i16,
    pub drawing_direction: i16,
    pub extents_height: f64,
    pub extents_width: f64,
    pub value: String,
    pub style_handle: u64,
    pub linespacing_style: i16,
    pub linespacing_factor: f64,
    pub unknown_bit: bool,
    pub background_flags: i32,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShapeData {
    pub insertion_point: Vector3,
    pub size: f64,
    pub rotation: f64,
    pub relative_x_scale: f64,
    pub oblique_angle: f64,
    pub thickness: f64,
    pub shape_number: i16,
    pub normal: Vector3,
    pub style_handle: u64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LeaderData {
    pub unknown_bit: bool,
    pub annotation_type: i16,
    pub path_type: i16,
    pub vertices: Vec<Vector3>,
    pub origin: Vector3,
    pub normal: Vector3,
    pub horizontal_direction: Vector3,
    pub block_offset: Vector3,
    pub annotation_offset: Vector3,
    pub text_height: f64,
    pub text_width: f64,
    pub hookline_on_x_dir: bool,
    pub arrowhead_on: bool,
    pub annotation_handle: u64,
    pub dimstyle_handle: u64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToleranceData {
    pub insertion_point: Vector3,
    pub direction: Vector3,
    pub normal: Vector3,
    pub text: String,
    pub dimstyle_handle: u64,
}

// ════════════════════════════════════════════════════════════════════════
//  Reader functions — Simple entities
// ════════════════════════════════════════════════════════════════════════

pub fn read_point(reader: &mut DwgMergedReader) -> PointData {
    let location = reader.read_3bit_double();
    let thickness = reader.read_bit_thickness();
    let normal = reader.read_bit_extrusion();
    let x_axis_angle = reader.read_bit_double();
    PointData { location, thickness, normal, x_axis_angle }
}

pub fn read_line(reader: &mut DwgMergedReader, version: DwgVersion) -> LineData {
    let (start, end);
    if version.r13_14_only() {
        start = reader.read_3bit_double();
        end = reader.read_3bit_double();
    } else {
        let z_are_zero = reader.read_bit();
        let sx = reader.read_raw_double();
        let ex = reader.read_bit_double_with_default(sx);
        let sy = reader.read_raw_double();
        let ey = reader.read_bit_double_with_default(sy);
        let (sz, ez) = if !z_are_zero {
            let sz = reader.read_raw_double();
            let ez = reader.read_bit_double_with_default(sz);
            (sz, ez)
        } else {
            (0.0, 0.0)
        };
        start = Vector3::new(sx, sy, sz);
        end = Vector3::new(ex, ey, ez);
    }
    let thickness = reader.read_bit_thickness();
    let normal = reader.read_bit_extrusion();
    LineData { start, end, thickness, normal }
}

pub fn read_circle(reader: &mut DwgMergedReader) -> CircleData {
    let center = reader.read_3bit_double();
    let radius = reader.read_bit_double();
    let thickness = reader.read_bit_thickness();
    let normal = reader.read_bit_extrusion();
    CircleData { center, radius, thickness, normal }
}

pub fn read_arc(reader: &mut DwgMergedReader) -> ArcData {
    let center = reader.read_3bit_double();
    let radius = reader.read_bit_double();
    let thickness = reader.read_bit_thickness();
    let normal = reader.read_bit_extrusion();
    let start_angle = reader.read_bit_double();
    let end_angle = reader.read_bit_double();
    ArcData { center, radius, thickness, normal, start_angle, end_angle }
}

pub fn read_ellipse(reader: &mut DwgMergedReader) -> EllipseData {
    let center = reader.read_3bit_double();
    let major_axis = reader.read_3bit_double();
    let normal = reader.read_3bit_double();
    let minor_axis_ratio = reader.read_bit_double();
    let start_parameter = reader.read_bit_double();
    let end_parameter = reader.read_bit_double();
    EllipseData { center, major_axis, normal, minor_axis_ratio, start_parameter, end_parameter }
}

pub fn read_ray(reader: &mut DwgMergedReader) -> RayData {
    let base_point = reader.read_3bit_double();
    let direction = reader.read_3bit_double();
    RayData { base_point, direction }
}

pub fn read_xline(reader: &mut DwgMergedReader) -> XLineData {
    let base_point = reader.read_3bit_double();
    let direction = reader.read_3bit_double();
    XLineData { base_point, direction }
}

pub fn read_solid(reader: &mut DwgMergedReader) -> SolidData {
    let thickness = reader.read_bit_thickness();
    let elevation = reader.read_bit_double();
    let first_corner = reader.read_2raw_double();
    let second_corner = reader.read_2raw_double();
    let third_corner = reader.read_2raw_double();
    let fourth_corner = reader.read_2raw_double();
    let normal = reader.read_bit_extrusion();
    SolidData { thickness, elevation, first_corner, second_corner, third_corner, fourth_corner, normal }
}

pub fn read_face3d(reader: &mut DwgMergedReader, version: DwgVersion) -> Face3DData {
    if version.r13_14_only() {
        let first_corner = reader.read_3bit_double();
        let second_corner = reader.read_3bit_double();
        let third_corner = reader.read_3bit_double();
        let fourth_corner = reader.read_3bit_double();
        let invisible_edges = reader.read_bit_short();
        Face3DData { first_corner, second_corner, third_corner, fourth_corner, invisible_edges }
    } else {
        let has_no_flags = reader.read_bit();
        let z_are_same = reader.read_bit();

        let x1 = reader.read_raw_double();
        let y1 = reader.read_raw_double();
        let z1 = if !z_are_same { reader.read_raw_double() } else { 0.0 };

        let x2 = reader.read_bit_double_with_default(x1);
        let y2 = reader.read_bit_double_with_default(y1);
        let z2 = if !z_are_same { reader.read_bit_double_with_default(z1) } else { z1 };

        let x3 = reader.read_bit_double_with_default(x2);
        let y3 = reader.read_bit_double_with_default(y2);
        let z3 = if !z_are_same { reader.read_bit_double_with_default(z2) } else { z1 };

        let x4 = reader.read_bit_double_with_default(x3);
        let y4 = reader.read_bit_double_with_default(y3);
        let z4 = if !z_are_same { reader.read_bit_double_with_default(z3) } else { z1 };

        let invisible_edges = if !has_no_flags { reader.read_bit_short() } else { 0 };

        Face3DData {
            first_corner: Vector3::new(x1, y1, z1),
            second_corner: Vector3::new(x2, y2, z2),
            third_corner: Vector3::new(x3, y3, z3),
            fourth_corner: Vector3::new(x4, y4, z4),
            invisible_edges,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Reader functions — Moderate entities
// ════════════════════════════════════════════════════════════════════════

pub fn read_insert(reader: &mut DwgMergedReader, version: DwgVersion) -> InsertData {
    let insert_point = reader.read_3bit_double();
    let (x_scale, y_scale, z_scale);

    if version.r13_14_only() {
        x_scale = reader.read_bit_double();
        y_scale = reader.read_bit_double();
        z_scale = reader.read_bit_double();
    } else {
        // R2000+
        let data_flags = reader.main_mut().read_2bits();
        match data_flags {
            3 => { x_scale = 1.0; y_scale = 1.0; z_scale = 1.0; }
            2 => {
                x_scale = reader.read_raw_double();
                y_scale = x_scale; z_scale = x_scale;
            }
            1 => {
                x_scale = 1.0;
                y_scale = reader.read_bit_double_with_default(1.0);
                z_scale = reader.read_bit_double_with_default(1.0);
            }
            _ => {
                x_scale = reader.read_raw_double();
                y_scale = reader.read_bit_double_with_default(x_scale);
                z_scale = reader.read_bit_double_with_default(x_scale);
            }
        }
    }

    let rotation = reader.read_bit_double();
    let normal = reader.read_3bit_double();
    let has_attribs = reader.read_bit();
    let block_handle = reader.read_handle();

    InsertData { insert_point, x_scale, y_scale, z_scale, rotation, normal, has_attribs, block_handle }
}

pub fn read_minsert(reader: &mut DwgMergedReader, version: DwgVersion) -> MInsertData {
    let insert = read_insert(reader, version);
    let column_count = reader.read_bit_short();
    let row_count = reader.read_bit_short();
    let column_spacing = reader.read_bit_double();
    let row_spacing = reader.read_bit_double();
    MInsertData { insert, column_count, row_count, column_spacing, row_spacing }
}

pub fn read_lwpolyline(reader: &mut DwgMergedReader, version: DwgVersion) -> LwPolylineData {
    let flag = reader.read_bit_short();
    let has_constant_width = (flag & 0x4) != 0;
    let has_elevation = (flag & 0x8) != 0;
    let has_thickness = (flag & 0x2) != 0;
    let has_normal = (flag & 0x1) != 0;
    let has_bulges = (flag & 0x10) != 0;
    let has_widths = (flag & 0x20) != 0;

    let constant_width = if has_constant_width { reader.read_bit_double() } else { 0.0 };
    let elevation = if has_elevation { reader.read_bit_double() } else { 0.0 };
    // BT / BE types: self-compressing, but still flag-gated
    let thickness = if has_thickness { reader.read_bit_thickness() } else { 0.0 };
    let normal = if has_normal { reader.read_bit_extrusion() } else { Vector3::UNIT_Z };

    let num_pts = safe_count(reader.read_bit_long());
    let num_bulges = if has_bulges { safe_count(reader.read_bit_long()) } else { 0 };
    let has_vertex_ids = (flag & 0x400) != 0;
    let num_vertex_ids = if has_vertex_ids { safe_count(reader.read_bit_long()) } else { 0 };
    let num_widths = if has_widths { safe_count(reader.read_bit_long()) } else { 0 };

    // Read vertex positions
    let mut xs = Vec::with_capacity(num_pts as usize);
    let mut ys = Vec::with_capacity(num_pts as usize);

    if version.r13_14_only() {
        for _ in 0..num_pts {
            xs.push(reader.read_raw_double());
            ys.push(reader.read_raw_double());
        }
    } else if num_pts > 0 {
        // R2000+: first vertex is 2RD, rest are 2DD
        xs.push(reader.read_raw_double());
        ys.push(reader.read_raw_double());
        for i in 1..num_pts as usize {
            let px = xs[i - 1];
            let py = ys[i - 1];
            xs.push(reader.read_bit_double_with_default(px));
            ys.push(reader.read_bit_double_with_default(py));
        }
    }

    // Read bulges
    let mut bulges = vec![0.0f64; num_pts as usize];
    if has_bulges {
        for i in 0..num_bulges as usize {
            if i < bulges.len() {
                bulges[i] = reader.read_bit_double();
            }
        }
    }

    // Read vertex IDs (R2010+, flag 0x400)
    if has_vertex_ids {
        for _ in 0..num_vertex_ids {
            let _vertex_id = reader.read_bit_long();
        }
    }

    // Read widths
    let mut start_widths = vec![0.0f64; num_pts as usize];
    let mut end_widths = vec![0.0f64; num_pts as usize];
    if has_widths {
        for i in 0..num_widths as usize {
            if i < start_widths.len() {
                start_widths[i] = reader.read_bit_double();
                end_widths[i] = reader.read_bit_double();
            }
        }
    }

    let mut vertices = Vec::with_capacity(num_pts as usize);
    for i in 0..num_pts as usize {
        vertices.push(LwPolylineVertex {
            x: xs[i], y: ys[i],
            bulge: bulges[i],
            start_width: start_widths[i],
            end_width: end_widths[i],
        });
    }

    LwPolylineData { flag, constant_width, elevation, thickness, normal, vertices }
}

pub fn read_spline(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> SplineData {
    let mut _flags1 = 0i32;
    let mut _knot_param = 0i32;

    let scenario;
    if version.r2013_plus(dxf_version) {
        scenario = reader.read_bit_long();
        _flags1 = reader.read_bit_long();
        _knot_param = reader.read_bit_long();
    } else {
        scenario = reader.read_bit_long();
    }

    let degree = reader.read_bit_long();

    let mut rational = false;
    let mut closed = false;
    let mut periodic = false;
    let mut knot_tolerance = 0.0;
    let mut control_tolerance = 0.0;
    let mut knots = Vec::new();
    let mut control_points = Vec::new();
    let mut weights = Vec::new();
    let mut fit_tolerance = 0.0;
    let mut begin_tangent = Vector3::ZERO;
    let mut end_tangent = Vector3::ZERO;
    let mut fit_points = Vec::new();

    match scenario {
        1 => {
            rational = reader.read_bit();
            closed = reader.read_bit();
            periodic = reader.read_bit();
            knot_tolerance = reader.read_bit_double();
            control_tolerance = reader.read_bit_double();
            let num_knots = safe_count(reader.read_bit_long());
            let num_ctrl = safe_count(reader.read_bit_long());
            let has_weights = reader.read_bit();

            for _ in 0..num_knots {
                knots.push(reader.read_bit_double());
            }
            for _ in 0..num_ctrl {
                let pt = reader.read_3bit_double();
                control_points.push(pt);
                if has_weights {
                    weights.push(reader.read_bit_double());
                }
            }
        }
        _ => {
            fit_tolerance = reader.read_bit_double();
            begin_tangent = reader.read_3bit_double();
            end_tangent = reader.read_3bit_double();
            let num_fit = safe_count(reader.read_bit_long());
            for _ in 0..num_fit {
                fit_points.push(reader.read_3bit_double());
            }
        }
    }

    SplineData {
        scenario, degree, rational, closed, periodic,
        knot_tolerance, control_tolerance,
        knots, control_points, weights,
        fit_tolerance, begin_tangent, end_tangent, fit_points,
    }
}

/// Shared text entity data reader (used by Text, AttDef, AttEntity).
pub fn read_text_entity_data(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
) -> TextEntityData {
    if version.r13_14_only() {
        let elevation = reader.read_bit_double();
        let ix = reader.read_raw_double();
        let iy = reader.read_raw_double();
        let ax = reader.read_raw_double();
        let ay = reader.read_raw_double();
        let normal = reader.read_3bit_double();
        let thickness = reader.read_bit_double();
        let oblique_angle = reader.read_bit_double();
        let rotation = reader.read_bit_double();
        let height = reader.read_bit_double();
        let width_factor = reader.read_bit_double();
        let value = reader.read_variable_text();
        let generation = reader.read_bit_short();
        let horizontal_alignment = reader.read_bit_short();
        let vertical_alignment = reader.read_bit_short();

        TextEntityData {
            insertion_point: Vector3::new(ix, iy, elevation),
            alignment_point: Vector3::new(ax, ay, elevation),
            normal, thickness, oblique_angle, rotation,
            height, width_factor, value, generation,
            horizontal_alignment, vertical_alignment,
            style_handle: 0,
        }
    } else {
        let data_flags = reader.read_byte();
        let elevation = if (data_flags & 0x01) == 0 { reader.read_raw_double() } else { 0.0 };
        let ix = reader.read_raw_double();
        let iy = reader.read_raw_double();
        let (ax, ay) = if (data_flags & 0x02) == 0 {
            (reader.read_bit_double_with_default(ix),
             reader.read_bit_double_with_default(iy))
        } else { (0.0, 0.0) };
        let normal = reader.read_bit_extrusion();
        let thickness = reader.read_bit_thickness();
        let oblique_angle = if (data_flags & 0x04) == 0 { reader.read_raw_double() } else { 0.0 };
        let rotation = if (data_flags & 0x08) == 0 { reader.read_raw_double() } else { 0.0 };
        let height = reader.read_raw_double();
        let width_factor = if (data_flags & 0x10) == 0 { reader.read_raw_double() } else { 1.0 };
        let value = reader.read_variable_text();
        let generation = if (data_flags & 0x20) == 0 { reader.read_bit_short() } else { 0 };
        let horizontal_alignment = if (data_flags & 0x40) == 0 { reader.read_bit_short() } else { 0 };
        let vertical_alignment = if (data_flags & 0x80) == 0 { reader.read_bit_short() } else { 0 };

        TextEntityData {
            insertion_point: Vector3::new(ix, iy, elevation),
            alignment_point: Vector3::new(ax, ay, elevation),
            normal, thickness, oblique_angle, rotation,
            height, width_factor, value, generation,
            horizontal_alignment, vertical_alignment,
            style_handle: 0,
        }
    }
}

/// Read TEXT entity (wraps read_text_entity_data + style handle).
pub fn read_text(reader: &mut DwgMergedReader, version: DwgVersion) -> TextEntityData {
    let mut data = read_text_entity_data(reader, version);
    data.style_handle = reader.read_handle();
    data
}

pub fn read_mtext(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> MTextData {
    let insertion_point = reader.read_3bit_double();
    let normal = reader.read_3bit_double();
    let x_direction = reader.read_3bit_double();
    let rectangle_width = reader.read_bit_double();
    let rectangle_height = if version.r2007_plus() { reader.read_bit_double() } else { 0.0 };
    let height = reader.read_bit_double();
    let attachment_point = reader.read_bit_short();
    let drawing_direction = reader.read_bit_short();
    let extents_height = reader.read_bit_double();
    let extents_width = reader.read_bit_double();
    let value = reader.read_variable_text();

    let style_handle = reader.read_handle();

    let linespacing_style = reader.read_bit_short();
    let linespacing_factor = reader.read_bit_double();
    let unknown_bit = reader.read_bit();

    let mut background_flags = 0i32;
    if version.r2004_plus() {
        background_flags = reader.read_bit_long();
    }

    if version.r2018_plus(dxf_version) {
        let _is_not_annotative = reader.read_bit();
    }

    MTextData {
        insertion_point, normal, x_direction, rectangle_width, rectangle_height,
        height, attachment_point, drawing_direction, extents_height, extents_width,
        value, style_handle, linespacing_style, linespacing_factor, unknown_bit,
        background_flags,
    }
}

pub fn read_shape(reader: &mut DwgMergedReader) -> ShapeData {
    let insertion_point = reader.read_3bit_double();
    let size = reader.read_bit_double();
    let rotation = reader.read_bit_double();
    let relative_x_scale = reader.read_bit_double();
    let oblique_angle = reader.read_bit_double();
    let thickness = reader.read_bit_double();
    let shape_number = reader.read_bit_short();
    let normal = reader.read_3bit_double();
    let style_handle = reader.read_handle();
    ShapeData { insertion_point, size, rotation, relative_x_scale, oblique_angle, thickness, shape_number, normal, style_handle }
}

pub fn read_leader(reader: &mut DwgMergedReader, version: DwgVersion) -> LeaderData {
    let unknown_bit = reader.read_bit();
    let annotation_type = reader.read_bit_short();
    let path_type = reader.read_bit_short();

    let num_pts = safe_count(reader.read_bit_long());
    let mut vertices = Vec::with_capacity(num_pts as usize);
    for _ in 0..num_pts { vertices.push(reader.read_3bit_double()); }

    let origin = reader.read_3bit_double();
    let normal = reader.read_3bit_double();
    let horizontal_direction = reader.read_3bit_double();
    let block_offset = reader.read_3bit_double();
    let annotation_offset = reader.read_3bit_double();

    if version.r13_14_only() {
        let _dimgap = reader.read_bit_double();
    }

    let mut text_height = 0.0;
    let mut text_width = 0.0;
    if !version.r2010_plus() {
        text_height = reader.read_bit_double();
        text_width = reader.read_bit_double();
    }

    let hookline_on_x_dir = reader.read_bit();
    let arrowhead_on = reader.read_bit();

    if version.r13_14_only() {
        let _arrowhead_type = reader.read_bit_short();
        let _dimasz = reader.read_bit_double();
        let _unk1 = reader.read_bit();
        let _unk2 = reader.read_bit();
        let _unk3 = reader.read_bit_short();
        let _bbc = reader.read_bit_short();
        let _unk4 = reader.read_bit();
        let _unk5 = reader.read_bit();
    }

    if version.r2000_plus() {
        let _unk_bs = reader.read_bit_short();
        let _unk_b1 = reader.read_bit();
        let _unk_b2 = reader.read_bit();
    }

    let annotation_handle = reader.read_handle();
    let dimstyle_handle = reader.read_handle();

    LeaderData {
        unknown_bit, annotation_type, path_type, vertices,
        origin, normal, horizontal_direction, block_offset,
        annotation_offset, text_height, text_width,
        hookline_on_x_dir, arrowhead_on,
        annotation_handle, dimstyle_handle,
    }
}

pub fn read_tolerance(reader: &mut DwgMergedReader, version: DwgVersion) -> ToleranceData {
    if version.r13_14_only() {
        let _unk_short = reader.read_bit_short();
        let _text_height = reader.read_bit_double();
        let _dimgap = reader.read_bit_double();
    }

    let insertion_point = reader.read_3bit_double();
    let direction = reader.read_3bit_double();
    let normal = reader.read_3bit_double();
    let text = reader.read_variable_text();
    let dimstyle_handle = reader.read_handle();

    ToleranceData { insertion_point, direction, normal, text, dimstyle_handle }
}

// ════════════════════════════════════════════════════════════════════════
//  Result structs — Complex entities
// ════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionCommonData {
    pub version_byte: u8,
    pub normal: Vector3,
    pub text_middle_point: Vector3,
    pub flags_byte: u8,
    pub text: String,
    pub text_rotation: f64,
    pub horizontal_direction: f64,
    pub ins_scale: Vector3,
    pub ins_rotation: f64,
    pub attachment_point: i16,
    pub linespacing_style: i16,
    pub linespacing_factor: f64,
    pub actual_measurement: f64,
    pub unknown_bit: bool,
    pub flip_arrow1: bool,
    pub flip_arrow2: bool,
    pub insertion_point: Vector2,
    pub dimstyle_handle: u64,
    pub block_handle: u64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionLinearData {
    pub common: DimensionCommonData,
    pub first_point: Vector3,
    pub second_point: Vector3,
    pub definition_point: Vector3,
    pub ext_line_rotation: f64,
    pub rotation: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionAlignedData {
    pub common: DimensionCommonData,
    pub first_point: Vector3,
    pub second_point: Vector3,
    pub definition_point: Vector3,
    pub ext_line_rotation: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionRadiusData {
    pub common: DimensionCommonData,
    pub definition_point: Vector3,
    pub angle_vertex: Vector3,
    pub leader_length: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionDiameterData {
    pub common: DimensionCommonData,
    pub definition_point: Vector3,
    pub angle_vertex: Vector3,
    pub leader_length: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionAngular2LnData {
    pub common: DimensionCommonData,
    pub dimension_arc: Vector2,
    pub first_point: Vector3,
    pub second_point: Vector3,
    pub angle_vertex: Vector3,
    pub definition_point: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionAngular3PtData {
    pub common: DimensionCommonData,
    pub definition_point: Vector3,
    pub first_point: Vector3,
    pub second_point: Vector3,
    pub angle_vertex: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DimensionOrdinateData {
    pub common: DimensionCommonData,
    pub definition_point: Vector3,
    pub feature_location: Vector3,
    pub leader_endpoint: Vector3,
    pub is_ordinate_type_x: bool,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HatchBoundaryEdgeLine { pub start: Vector2, pub end: Vector2 }
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HatchBoundaryEdgeArc { pub center: Vector2, pub radius: f64, pub start_angle: f64, pub end_angle: f64, pub ccw: bool }
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HatchBoundaryEdgeEllipse { pub center: Vector2, pub major_endpoint: Vector2, pub minor_ratio: f64, pub start_angle: f64, pub end_angle: f64, pub ccw: bool }
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HatchBoundaryEdgeSpline { pub degree: i32, pub rational: bool, pub periodic: bool, pub knots: Vec<f64>, pub control_points: Vec<Vector3>, pub fit_points: Vec<Vector2>, pub start_tangent: Vector2, pub end_tangent: Vector2 }
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HatchEdge { Line(HatchBoundaryEdgeLine), Arc(HatchBoundaryEdgeArc), Ellipse(HatchBoundaryEdgeEllipse), Spline(HatchBoundaryEdgeSpline) }
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HatchBoundaryPath { pub flags: i32, pub edges: Vec<HatchEdge>, pub polyline_vertices: Vec<(Vector2, f64)>, pub polyline_closed: bool, pub boundary_handle_count: i32 }
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HatchPatternLine { pub angle: f64, pub base_point: Vector2, pub offset: Vector2, pub dashes: Vec<f64> }
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HatchData {
    pub gradient_enabled: bool,
    pub gradient_reserved: i32,
    pub gradient_angle: f64,
    pub gradient_shift: f64,
    pub gradient_single_color: bool,
    pub gradient_tint: f64,
    pub gradient_colors: Vec<(f64, crate::types::Color)>,
    pub gradient_name: String,
    pub elevation: f64,
    pub normal: Vector3,
    pub pattern_name: String,
    pub is_solid: bool,
    pub is_associative: bool,
    pub paths: Vec<HatchBoundaryPath>,
    pub style: i16,
    pub pattern_type: i16,
    pub pattern_angle: f64,
    pub pattern_scale: f64,
    pub is_double: bool,
    pub pattern_lines: Vec<HatchPatternLine>,
    pub pixel_size: f64,
    pub seed_points: Vec<Vector2>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ViewportData {
    pub center: Vector3,
    pub width: f64,
    pub height: f64,
    pub view_target: Vector3,
    pub view_direction: Vector3,
    pub twist_angle: f64,
    pub view_height: f64,
    pub lens_length: f64,
    pub front_clip_z: f64,
    pub back_clip_z: f64,
    pub snap_angle: f64,
    pub view_center: Vector2,
    pub snap_base: Vector2,
    pub snap_spacing: Vector2,
    pub grid_spacing: Vector2,
    pub circle_sides: i16,
    pub grid_major: i16,
    pub frozen_layer_count: i32,
    pub status_flags: i32,
    pub render_mode: u8,
    pub ucs_per_viewport: bool,
    pub ucs_origin: Vector3,
    pub ucs_x_axis: Vector3,
    pub ucs_y_axis: Vector3,
    pub ucs_elevation: f64,
    pub ucs_ortho_type: i16,
    pub shade_plot_mode: i16,
    pub default_lighting: bool,
    pub default_lighting_type: u8,
    pub brightness: f64,
    pub contrast: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Polyline2DData {
    pub flags: i16,
    pub smooth_surface: i16,
    pub start_width: f64,
    pub end_width: f64,
    pub thickness: f64,
    pub elevation: f64,
    pub normal: Vector3,
    pub owned_count: i32,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vertex2DData {
    pub handle: crate::types::Handle,
    pub flags: u8,
    pub x: f64, pub y: f64, pub z: f64,
    pub start_width: f64,
    pub end_width: f64,
    pub bulge: f64,
    pub vertex_id: i32,
    pub tangent_dir: f64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Polyline3DData {
    pub smooth_type: u8,
    pub closed_flag: u8,
    pub owned_count: i32,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vertex3DData {
    pub handle: crate::types::Handle,
    pub flags: u8,
    pub position: Vector3,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MLineVertexData {
    pub position: Vector3,
    pub direction: Vector3,
    pub miter: Vector3,
    pub segments: Vec<MLineSegmentData>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MLineSegmentData {
    pub parameters: Vec<f64>,
    pub area_fill_parameters: Vec<f64>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MLineData {
    pub scale_factor: f64,
    pub justification: u8,
    pub start_point: Vector3,
    pub normal: Vector3,
    pub openclosed: i16,
    pub lines_in_style: u8,
    pub vertex_count: i16,
    pub vertices: Vec<MLineVertexData>,
    pub style_handle: u64,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MeshData {
    pub version: i16,
    pub blend_crease: bool,
    pub subdivision_level: i32,
    pub vertices: Vec<Vector3>,
    pub faces: Vec<Vec<i32>>,
    pub edges: Vec<(i32, i32)>,
    pub crease_values: Vec<f64>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RasterImageData {
    pub class_version: i32,
    pub insertion_point: Vector3,
    pub u_vector: Vector3,
    pub v_vector: Vector3,
    pub size: Vector2,
    pub flags: i16,
    pub clipping_enabled: bool,
    pub brightness: u8,
    pub contrast: u8,
    pub fade: u8,
    pub clip_inverted: bool,
    pub clip_type: i16,
    pub definition_handle: u64,
    pub reactor_handle: u64,
    /// Clip boundary vertices in image pixel space (range 0..size for rect;
    /// arbitrary polygon for polygonal). For rectangular clips two corners
    /// are stored; for polygonal, three or more sequential vertices.
    pub clip_boundary_vertices: Vec<Vector2>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ole2FrameData {
    pub version: i16,
    pub mode: i16,
    pub data: Vec<u8>,
    pub trailing_byte: u8,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AttributeCommonData {
    pub text_data: TextEntityData,
    pub att_version: u8,
    pub att_type: u8,
    pub tag: String,
    pub field_length: i16,
    pub flags: u8,
    pub lock_position: bool,
}

// ════════════════════════════════════════════════════════════════════════
//  Reader functions — Complex entities
// ════════════════════════════════════════════════════════════════════════

/// Read common dimension data shared by all dimension types.
pub fn read_common_dimension_data(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    _dxf_version: DxfVersion,
) -> DimensionCommonData {
    let version_byte = if version.r2010_plus() { reader.read_byte() } else { 0 };
    let normal = reader.read_3bit_double();
    let text_mid = reader.read_2raw_double();
    let text_mid_z = reader.read_bit_double();
    let flags_byte = reader.read_byte();
    let text = reader.read_variable_text();
    let text_rotation = reader.read_bit_double();
    let horizontal_direction = reader.read_bit_double();
    let ins_scale = reader.read_3bit_double();
    let ins_rotation = reader.read_bit_double();

    let mut attachment_point = 0i16;
    let mut linespacing_style = 1i16;
    let mut linespacing_factor = 1.0;
    let mut actual_measurement = 0.0;
    if version.r2000_plus() {
        attachment_point = reader.read_bit_short();
        linespacing_style = reader.read_bit_short();
        linespacing_factor = reader.read_bit_double();
        actual_measurement = reader.read_bit_double();
    }

    let mut unknown_bit = false;
    let mut flip_arrow1 = false;
    let mut flip_arrow2 = false;
    if version.r2007_plus() {
        unknown_bit = reader.read_bit();
        flip_arrow1 = reader.read_bit();
        flip_arrow2 = reader.read_bit();
    }

    let insertion_point = reader.read_2raw_double();
    let dimstyle_handle = reader.read_handle();
    let block_handle = reader.read_handle();

    DimensionCommonData {
        version_byte, normal,
        text_middle_point: Vector3::new(text_mid.x, text_mid.y, text_mid_z),
        flags_byte, text, text_rotation, horizontal_direction,
        ins_scale, ins_rotation,
        attachment_point, linespacing_style, linespacing_factor,
        actual_measurement, unknown_bit, flip_arrow1, flip_arrow2,
        insertion_point, dimstyle_handle, block_handle,
    }
}

pub fn read_dimension_linear(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> DimensionLinearData {
    let common = read_common_dimension_data(reader, version, dxf_version);
    let first_point = reader.read_3bit_double();
    let second_point = reader.read_3bit_double();
    let definition_point = reader.read_3bit_double();
    let ext_line_rotation = reader.read_bit_double();
    let rotation = reader.read_bit_double();
    DimensionLinearData { common, first_point, second_point, definition_point, ext_line_rotation, rotation }
}

pub fn read_dimension_aligned(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> DimensionAlignedData {
    let common = read_common_dimension_data(reader, version, dxf_version);
    let first_point = reader.read_3bit_double();
    let second_point = reader.read_3bit_double();
    let definition_point = reader.read_3bit_double();
    let ext_line_rotation = reader.read_bit_double();
    DimensionAlignedData { common, first_point, second_point, definition_point, ext_line_rotation }
}

pub fn read_dimension_radius(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> DimensionRadiusData {
    let common = read_common_dimension_data(reader, version, dxf_version);
    let definition_point = reader.read_3bit_double();
    let angle_vertex = reader.read_3bit_double();
    let leader_length = reader.read_bit_double();
    DimensionRadiusData { common, definition_point, angle_vertex, leader_length }
}

pub fn read_dimension_diameter(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> DimensionDiameterData {
    let common = read_common_dimension_data(reader, version, dxf_version);
    let definition_point = reader.read_3bit_double();
    let angle_vertex = reader.read_3bit_double();
    let leader_length = reader.read_bit_double();
    DimensionDiameterData { common, definition_point, angle_vertex, leader_length }
}

pub fn read_dimension_angular_2ln(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> DimensionAngular2LnData {
    let common = read_common_dimension_data(reader, version, dxf_version);
    let dimension_arc = reader.read_2raw_double();
    let first_point = reader.read_3bit_double();
    let second_point = reader.read_3bit_double();
    let angle_vertex = reader.read_3bit_double();
    let definition_point = reader.read_3bit_double();
    DimensionAngular2LnData { common, dimension_arc, first_point, second_point, angle_vertex, definition_point }
}

pub fn read_dimension_angular_3pt(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> DimensionAngular3PtData {
    let common = read_common_dimension_data(reader, version, dxf_version);
    let definition_point = reader.read_3bit_double();
    let first_point = reader.read_3bit_double();
    let second_point = reader.read_3bit_double();
    let angle_vertex = reader.read_3bit_double();
    DimensionAngular3PtData { common, definition_point, first_point, second_point, angle_vertex }
}

pub fn read_dimension_ordinate(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> DimensionOrdinateData {
    let common = read_common_dimension_data(reader, version, dxf_version);
    let definition_point = reader.read_3bit_double();
    let feature_location = reader.read_3bit_double();
    let leader_endpoint = reader.read_3bit_double();
    let is_ordinate_type_x = reader.read_byte() == 1;
    DimensionOrdinateData { common, definition_point, feature_location, leader_endpoint, is_ordinate_type_x }
}

/// Read a hatch boundary path (both polyline and non-polyline variants).
pub fn read_hatch_boundary_path(reader: &mut DwgMergedReader, version: DwgVersion) -> HatchBoundaryPath {
    let flags = reader.read_bit_long();
    let is_polyline = (flags & 2) != 0;

    let mut edges = Vec::new();
    let mut polyline_vertices = Vec::new();
    let mut polyline_closed = false;

    if !is_polyline {
        let num_edges = safe_count(reader.read_bit_long());
        for _ in 0..num_edges {
            let edge_type = reader.read_byte();
            match edge_type {
                1 => {
                    let start = reader.read_2raw_double();
                    let end = reader.read_2raw_double();
                    edges.push(HatchEdge::Line(HatchBoundaryEdgeLine { start, end }));
                }
                2 => {
                    let center = reader.read_2raw_double();
                    let radius = reader.read_bit_double();
                    let start_angle = reader.read_bit_double();
                    let end_angle = reader.read_bit_double();
                    let ccw = reader.read_bit();
                    edges.push(HatchEdge::Arc(HatchBoundaryEdgeArc { center, radius, start_angle, end_angle, ccw }));
                }
                3 => {
                    let center = reader.read_2raw_double();
                    let major_endpoint = reader.read_2raw_double();
                    let minor_ratio = reader.read_bit_double();
                    let start_angle = reader.read_bit_double();
                    let end_angle = reader.read_bit_double();
                    let ccw = reader.read_bit();
                    edges.push(HatchEdge::Ellipse(HatchBoundaryEdgeEllipse { center, major_endpoint, minor_ratio, start_angle, end_angle, ccw }));
                }
                4 => {
                    let degree = reader.read_bit_long();
                    let rational = reader.read_bit();
                    let periodic = reader.read_bit();
                    let num_knots = safe_count(reader.read_bit_long());
                    let num_ctrl = safe_count(reader.read_bit_long());
                    let mut knots = Vec::new();
                    for _ in 0..num_knots { knots.push(reader.read_bit_double()); }
                    let mut control_points = Vec::new();
                    for _ in 0..num_ctrl {
                        let pt = reader.read_2raw_double();
                        let w = if rational { reader.read_bit_double() } else { 1.0 };
                        control_points.push(Vector3::new(pt.x, pt.y, w));
                    }
                    let mut fit_points = Vec::new();
                    let mut start_tangent = Vector2::ZERO;
                    let mut end_tangent = Vector2::ZERO;
                    if version.r2010_plus() {
                        let num_fit = safe_count(reader.read_bit_long());
                        if num_fit > 0 {
                            for _ in 0..num_fit { fit_points.push(reader.read_2raw_double()); }
                            start_tangent = reader.read_2raw_double();
                            end_tangent = reader.read_2raw_double();
                        }
                    }
                    edges.push(HatchEdge::Spline(HatchBoundaryEdgeSpline { degree, rational, periodic, knots, control_points, fit_points, start_tangent, end_tangent }));
                }
                _ => {}
            }
        }
    } else {
        let has_bulge = reader.read_bit();
        polyline_closed = reader.read_bit();
        let num_verts = safe_count(reader.read_bit_long());
        for _ in 0..num_verts {
            let pt = reader.read_2raw_double();
            let bulge = if has_bulge { reader.read_bit_double() } else { 0.0 };
            polyline_vertices.push((pt, bulge));
        }
    }

    let boundary_handle_count = reader.read_bit_long();

    HatchBoundaryPath { flags, edges, polyline_vertices, polyline_closed, boundary_handle_count }
}

pub fn read_hatch(reader: &mut DwgMergedReader, version: DwgVersion) -> HatchData {
    let mut gradient_enabled = false;
    let mut gradient_reserved = 0i32;
    let mut gradient_angle = 0.0;
    let mut gradient_shift = 0.0;
    let mut gradient_single_color = false;
    let mut gradient_tint = 0.0;
    let mut gradient_colors = Vec::new();
    let mut gradient_name = String::new();
    if version.r2004_plus() {
        let is_gradient = reader.read_bit_long();
        gradient_enabled = is_gradient != 0;
        gradient_reserved = reader.read_bit_long();
        gradient_angle = reader.read_bit_double();
        gradient_shift = reader.read_bit_double();
        gradient_single_color = reader.read_bit_long() != 0;
        gradient_tint = reader.read_bit_double();
        let num_colors = safe_count(reader.read_bit_long());
        for _ in 0..num_colors {
            let value = reader.read_bit_double();
            let color = reader.read_cm_color();
            gradient_colors.push((value, color));
        }
        gradient_name = reader.read_variable_text();
    }

    let elevation = reader.read_bit_double();
    let normal = reader.read_3bit_double();
    let pattern_name = reader.read_variable_text();
    let is_solid = reader.read_bit();
    let is_associative = reader.read_bit();

    let num_paths = safe_count(reader.read_bit_long());
    let mut paths = Vec::new();
    let mut has_derived = false;
    for _ in 0..num_paths {
        let p = read_hatch_boundary_path(reader, version);
        if (p.flags & 4) != 0 { has_derived = true; }
        paths.push(p);
    }

    let style = reader.read_bit_short();
    let pattern_type = reader.read_bit_short();

    let mut pattern_angle = 0.0;
    let mut pattern_scale = 1.0;
    let mut is_double = false;
    let mut pattern_lines = Vec::new();
    if !is_solid {
        pattern_angle = reader.read_bit_double();
        pattern_scale = reader.read_bit_double();
        is_double = reader.read_bit();
        let num_lines = reader.read_bit_short();
        for _ in 0..num_lines {
            let angle = reader.read_bit_double();
            let base_pt = reader.read_2bit_double();
            let offset = reader.read_2bit_double();
            let num_dashes = reader.read_bit_short();
            let mut dashes = Vec::new();
            for _ in 0..num_dashes { dashes.push(reader.read_bit_double()); }
            pattern_lines.push(HatchPatternLine { angle, base_point: base_pt, offset, dashes });
        }
    }

    let pixel_size = if has_derived { reader.read_bit_double() } else { 0.0 };

    let num_seeds = safe_count(reader.read_bit_long());
    let mut seed_points = Vec::new();
    for _ in 0..num_seeds { seed_points.push(reader.read_2raw_double()); }

    // boundary handles are read externally (for each path, path.boundary_handle_count handles)

    HatchData {
        gradient_enabled, gradient_reserved, gradient_angle, gradient_shift,
        gradient_single_color, gradient_tint, gradient_colors, gradient_name,
        elevation, normal, pattern_name, is_solid, is_associative,
        paths, style, pattern_type, pattern_angle, pattern_scale, is_double,
        pattern_lines, pixel_size, seed_points,
    }
}

pub fn read_viewport(reader: &mut DwgMergedReader, version: DwgVersion, _dxf_version: DxfVersion) -> ViewportData {
    let center = reader.read_3bit_double();
    let width = reader.read_bit_double();
    let height = reader.read_bit_double();

    // View data (read for all versions)
    let view_target = reader.read_3bit_double();
    let view_direction = reader.read_3bit_double();
    let twist_angle = reader.read_bit_double();
    let view_height = reader.read_bit_double();
    let lens_length = reader.read_bit_double();
    let front_clip_z = reader.read_bit_double();
    let back_clip_z = reader.read_bit_double();
    let snap_angle = reader.read_bit_double();
    let view_center = reader.read_2raw_double();
    let snap_base = reader.read_2raw_double();
    let snap_spacing = reader.read_2raw_double();
    let grid_spacing = reader.read_2raw_double();
    let circle_sides = reader.read_bit_short();

    let grid_major = if version.r2007_plus() {
        reader.read_bit_short()
    } else {
        0
    };

    // Status/UCS data (read for all versions)
    let frozen_layer_count = reader.read_bit_long();
    let status_flags = reader.read_bit_long();
    let _style_sheet = reader.read_variable_text();
    let render_mode = reader.read_byte();
    let _ucs_at_origin = reader.read_bit();
    let ucs_per_viewport = reader.read_bit();
    let ucs_origin = reader.read_3bit_double();
    let ucs_x_axis = reader.read_3bit_double();
    let ucs_y_axis = reader.read_3bit_double();
    let ucs_elevation = reader.read_bit_double();
    let ucs_ortho_type = reader.read_bit_short();

    let shade_plot_mode = if version.r2004_plus() {
        reader.read_bit_short()
    } else {
        0
    };
    let (default_lighting, default_lighting_type, brightness, contrast) = if version.r2007_plus() {
        let dl = reader.read_bit();
        let dlt = reader.read_byte();
        let br = reader.read_bit_double();
        let co = reader.read_bit_double();
        let _ambient_color = reader.read_cm_color();
        (dl, dlt, br, co)
    } else {
        (false, 0, 0.0, 0.0)
    };

    ViewportData {
        center, width, height, view_target, view_direction,
        twist_angle, view_height, lens_length, front_clip_z, back_clip_z,
        snap_angle, view_center, snap_base, snap_spacing, grid_spacing,
        circle_sides, grid_major, frozen_layer_count, status_flags, render_mode,
        ucs_per_viewport, ucs_origin, ucs_x_axis, ucs_y_axis, ucs_elevation,
        ucs_ortho_type, shade_plot_mode, default_lighting, default_lighting_type,
        brightness, contrast,
    }
}

pub fn read_polyline2d(reader: &mut DwgMergedReader, version: DwgVersion) -> Polyline2DData {
    let flags = reader.read_bit_short();
    let smooth_surface = reader.read_bit_short();
    let start_width = reader.read_bit_double();
    let end_width = reader.read_bit_double();
    let thickness = reader.read_bit_thickness();
    let elevation = reader.read_bit_double();
    let normal = reader.read_bit_extrusion();
    let owned_count = if version.r2004_plus() { reader.read_bit_long() } else { 0 };
    Polyline2DData { flags, smooth_surface, start_width, end_width, thickness, elevation, normal, owned_count }
}

pub fn read_vertex2d(reader: &mut DwgMergedReader, version: DwgVersion) -> Vertex2DData {
    let flags = reader.read_byte();
    let x = reader.read_bit_double();
    let y = reader.read_bit_double();
    let z = reader.read_bit_double();
    let sw = reader.read_bit_double();
    let (start_width, end_width) = if sw < 0.0 {
        (-sw, -sw) // negative = both widths equal
    } else {
        let ew = reader.read_bit_double();
        (sw, ew)
    };
    let bulge = reader.read_bit_double();
    let vertex_id = if version.r2010_plus() { reader.read_bit_long() } else { 0 };
    let tangent_dir = reader.read_bit_double();
    Vertex2DData { handle: crate::types::Handle::NULL, flags, x, y, z, start_width, end_width, bulge, vertex_id, tangent_dir }
}

pub fn read_polyline3d(reader: &mut DwgMergedReader, version: DwgVersion) -> Polyline3DData {
    let smooth_type = reader.read_byte();
    let closed_flag = reader.read_byte();
    let owned_count = if version.r2004_plus() { reader.read_bit_long() } else { 0 };
    Polyline3DData { smooth_type, closed_flag, owned_count }
}

pub fn read_vertex3d(reader: &mut DwgMergedReader) -> Vertex3DData {
    let flags = reader.read_byte();
    let position = reader.read_3bit_double();
    Vertex3DData { handle: crate::types::Handle::NULL, flags, position }
}

pub fn read_polyface_mesh(reader: &mut DwgMergedReader, version: DwgVersion) -> (i16, i16, i32) {
    let num_verts = reader.read_bit_short();
    let num_faces = reader.read_bit_short();
    let owned_count = if version.r2004_plus() { reader.read_bit_long() } else { 0 };
    (num_verts, num_faces, owned_count)
}

/// Face record data from OBJ_VERTEX_PFACE_FACE (type 14).
pub struct PfaceFaceData {
    pub handle: crate::types::Handle,
    pub index1: i16,
    pub index2: i16,
    pub index3: i16,
    pub index4: i16,
}

/// Read a VERTEX_PFACE_FACE record (type code 14).
/// Format: 4 × BS (vertex indices), no flags byte.
pub fn read_pface_face(reader: &mut DwgMergedReader) -> PfaceFaceData {
    let index1 = reader.read_bit_short();
    let index2 = reader.read_bit_short();
    let index3 = reader.read_bit_short();
    let index4 = reader.read_bit_short();
    PfaceFaceData { handle: crate::types::Handle::NULL, index1, index2, index3, index4 }
}

pub fn read_polygon_mesh(reader: &mut DwgMergedReader, version: DwgVersion) -> (i16, i16, i16, i16, i16, i16, i32) {
    let flags = reader.read_bit_short();
    let smooth_type = reader.read_bit_short();
    let m_count = reader.read_bit_short();
    let n_count = reader.read_bit_short();
    let m_smooth = reader.read_bit_short();
    let n_smooth = reader.read_bit_short();
    let owned_count = if version.r2004_plus() { reader.read_bit_long() } else { 0 };
    (flags, smooth_type, m_count, n_count, m_smooth, n_smooth, owned_count)
}

pub fn read_seqend(_reader: &mut DwgMergedReader) {
    // SEQEND has no entity-specific data
}

pub fn read_mline(reader: &mut DwgMergedReader) -> MLineData {
    let scale_factor = reader.read_bit_double();
    let justification = reader.read_byte();
    let start_point = reader.read_3bit_double();
    let normal = reader.read_3bit_double();
    let openclosed = reader.read_bit_short();
    let lines_in_style = reader.read_byte();
    let vertex_count = reader.read_bit_short();

    // Read vertices (position + direction + miter + segments)
    let mut vertices = Vec::with_capacity(vertex_count as usize);
    for _ in 0..vertex_count {
        let pos = reader.read_3bit_double();
        let dir = reader.read_3bit_double();
        let miter = reader.read_3bit_double();
        let mut segments = Vec::with_capacity(lines_in_style as usize);
        for _ in 0..lines_in_style {
            let num_params = reader.read_bit_short();
            let mut params = Vec::with_capacity(num_params as usize);
            for _ in 0..num_params { params.push(reader.read_bit_double()); }
            let num_area = reader.read_bit_short();
            let mut area_params = Vec::with_capacity(num_area as usize);
            for _ in 0..num_area { area_params.push(reader.read_bit_double()); }
            segments.push(MLineSegmentData { parameters: params, area_fill_parameters: area_params });
        }
        vertices.push(MLineVertexData { position: pos, direction: dir, miter, segments });
    }

    let style_handle = reader.read_handle();

    MLineData { scale_factor, justification, start_point, normal, openclosed, lines_in_style, vertex_count, vertices, style_handle }
}

pub fn read_mesh(reader: &mut DwgMergedReader) -> MeshData {
    let version = reader.read_bit_short();
    let blend_crease = reader.read_bit();
    let subdivision_level = reader.read_bit_long();

    let num_verts = safe_count(reader.read_bit_long());
    let mut vertices = Vec::with_capacity(num_verts as usize);
    for _ in 0..num_verts { vertices.push(reader.read_3bit_double()); }

    let total_face_data = safe_count(reader.read_bit_long());
    let mut faces = Vec::new();
    let mut i = 0;
    while i < total_face_data {
        let n = safe_count(reader.read_bit_long());
        i += 1;
        let mut face = Vec::new();
        for _ in 0..n {
            face.push(reader.read_bit_long());
            i += 1;
        }
        faces.push(face);
    }

    let num_edges = safe_count(reader.read_bit_long());
    let mut edges = Vec::with_capacity(num_edges as usize);
    for _ in 0..num_edges {
        let s = reader.read_bit_long();
        let e = reader.read_bit_long();
        edges.push((s, e));
    }

    let num_creases = safe_count(reader.read_bit_long());
    let mut crease_values = Vec::with_capacity(num_creases as usize);
    for _ in 0..num_creases { crease_values.push(reader.read_bit_double()); }

    let _trailing = reader.read_bit_long();

    MeshData { version, blend_crease, subdivision_level, vertices, faces, edges, crease_values }
}

pub fn read_raster_image(reader: &mut DwgMergedReader, version: DwgVersion) -> RasterImageData {
    let class_version = reader.read_bit_long();
    let insertion_point = reader.read_3bit_double();
    let u_vector = reader.read_3bit_double();
    let v_vector = reader.read_3bit_double();
    let size = reader.read_2raw_double();
    let flags = reader.read_bit_short();
    let clipping_enabled = reader.read_bit();
    let brightness = reader.read_byte();
    let contrast = reader.read_byte();
    let fade = reader.read_byte();
    let clip_inverted = if version.r2010_plus() { reader.read_bit() } else { false };

    // Clip boundary
    let clip_type = reader.read_bit_short();
    let mut clip_boundary_vertices: Vec<Vector2> = Vec::new();
    if clip_type == 1 {
        // Rectangular: 2 opposite-corner vertices
        clip_boundary_vertices.push(reader.read_2raw_double());
        clip_boundary_vertices.push(reader.read_2raw_double());
    } else {
        // Polygonal
        let n = safe_count(reader.read_bit_long()) as usize;
        clip_boundary_vertices.reserve(n);
        for _ in 0..n {
            clip_boundary_vertices.push(reader.read_2raw_double());
        }
    }

    let definition_handle = reader.read_handle();
    let reactor_handle = reader.read_handle();

    RasterImageData {
        class_version, insertion_point, u_vector, v_vector, size,
        flags, clipping_enabled, brightness, contrast, fade, clip_inverted,
        clip_type, definition_handle, reactor_handle,
        clip_boundary_vertices,
    }
}

pub fn read_wipeout(reader: &mut DwgMergedReader, version: DwgVersion) -> RasterImageData {
    // Wipeout uses the same data layout as RasterImage
    read_raster_image(reader, version)
}

pub fn read_ole2frame(reader: &mut DwgMergedReader, version: DwgVersion) -> Ole2FrameData {
    let ver = reader.read_bit_short();
    let mode = if version.r2000_plus() { reader.read_bit_short() } else { 0 };
    // OLE binary data can be very large (embedded images/documents),
    // so don't use safe_count (100 KB cap). Use a generous 10 MB cap instead.
    let data_len = (reader.read_bit_long().max(0) as usize).min(10_000_000);
    let data = reader.read_bytes(data_len);
    let trailing_byte = if version.r2000_plus() { reader.read_byte() } else { 3 };
    Ole2FrameData { version: ver, mode, data, trailing_byte }
}

pub fn read_attribute_definition(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> AttributeCommonData {
    let mut text_data = read_text_entity_data(reader, version);
    text_data.style_handle = reader.read_handle();

    let att_version = if version.r2010_plus() { reader.read_byte() } else { 0 };
    let att_type = if version.r2018_plus(dxf_version) { reader.read_byte() } else { 1 };

    let tag = reader.read_variable_text();
    let field_length = reader.read_bit_short();
    let flags = reader.read_byte();
    let lock_position = if version.r2007_plus() { reader.read_bit() } else { false };

    // AttDef-specific: second version byte + prompt
    if version.r2010_plus() {
        let _version2 = reader.read_byte();
    }
    let _prompt = reader.read_variable_text();

    AttributeCommonData { text_data, att_version, att_type, tag, field_length, flags, lock_position }
}

pub fn read_attribute_entity(reader: &mut DwgMergedReader, version: DwgVersion, dxf_version: DxfVersion) -> AttributeCommonData {
    let mut text_data = read_text_entity_data(reader, version);
    text_data.style_handle = reader.read_handle();

    let att_version = if version.r2010_plus() { reader.read_byte() } else { 0 };
    let att_type = if version.r2018_plus(dxf_version) { reader.read_byte() } else { 1 };

    let tag = reader.read_variable_text();
    let field_length = reader.read_bit_short();
    let flags = reader.read_byte();
    let lock_position = if version.r2007_plus() { reader.read_bit() } else { false };

    AttributeCommonData { text_data, att_version, att_type, tag, field_length, flags, lock_position }
}

// ════════════════════════════════════════════════════════════════════════
//  MultiLeader reader
// ════════════════════════════════════════════════════════════════════════

/// Data returned by the multileader reader.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MultiLeaderData {
    pub context: MultiLeaderAnnotContext,
    pub style_handle: u64,
    pub property_override_flags: u32,
    pub path_type: i16,
    pub line_color: Color,
    pub line_type_handle: u64,
    pub line_weight: i32,
    pub enable_landing: bool,
    pub enable_dogleg: bool,
    pub dogleg_length: f64,
    pub arrowhead_handle: u64,
    pub arrowhead_size: f64,
    pub content_type: i16,
    pub text_style_handle: u64,
    pub text_left_attachment: i16,
    pub text_right_attachment: i16,
    pub text_angle_type: i16,
    pub text_alignment: i16,
    pub text_color: Color,
    pub text_frame: bool,
    pub block_content_handle: u64,
    pub block_content_color: Color,
    pub block_scale: Vector3,
    pub block_rotation: f64,
    pub block_connection_type: i16,
    pub enable_annotation_scale: bool,
    pub block_attributes: Vec<BlockAttribute>,
    pub text_direction_negative: bool,
    pub text_align_in_ipe: i16,
    pub text_attachment_point: i16,
    pub scale_factor: f64,
    pub text_attachment_direction: i16,
    pub text_bottom_attachment: i16,
    pub text_top_attachment: i16,
    pub extend_leader_to_text: bool,
}

pub fn read_multileader(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> MultiLeaderData {
    // R2010+: version
    if version.r2010_plus() {
        let _ml_version = reader.read_bit_short();
    }

    // Annotation context (inline)
    let context = read_multileader_annotation_context(reader, version, dxf_version);

    // Common data
    let style_handle = reader.read_handle();
    let property_override_flags = reader.read_bit_long() as u32;
    let path_type = reader.read_bit_short();
    let line_color = reader.read_cm_color();
    let line_type_handle = reader.read_handle();
    let line_weight = reader.read_bit_long();
    let enable_landing = reader.read_bit();
    let enable_dogleg = reader.read_bit();
    let dogleg_length = reader.read_bit_double();
    let arrowhead_handle = reader.read_handle();
    let arrowhead_size = reader.read_bit_double();
    let content_type = reader.read_bit_short();
    let text_style_handle = reader.read_handle();
    let text_left_attachment = reader.read_bit_short();
    let text_right_attachment = reader.read_bit_short();
    let text_angle_type = reader.read_bit_short();
    let text_alignment = reader.read_bit_short();
    let text_color = reader.read_cm_color();
    let text_frame = reader.read_bit();
    let block_content_handle = reader.read_handle();
    let block_content_color = reader.read_cm_color();
    let block_scale = reader.read_3bit_double();
    let block_rotation = reader.read_bit_double();
    let block_connection_type = reader.read_bit_short();
    let enable_annotation_scale = reader.read_bit();

    // Block attributes
    let ba_count = safe_count(reader.read_bit_long());
    let mut block_attributes = Vec::with_capacity(ba_count as usize);
    for _ in 0..ba_count {
        let def_handle = reader.read_handle();
        let text = reader.read_variable_text();
        let index = reader.read_bit_short();
        let width = reader.read_bit_double();
        block_attributes.push(BlockAttribute {
            attribute_definition_handle: if def_handle != 0 { Some(Handle::from(def_handle)) } else { None },
            text,
            index,
            width,
        });
    }

    let text_direction_negative = reader.read_bit();
    let text_align_in_ipe = reader.read_bit_short();
    let text_attachment_point = reader.read_bit_short();
    let scale_factor = reader.read_bit_double();

    let mut text_attachment_direction: i16 = 0;
    let mut text_bottom_attachment: i16 = 9; // CenterOfText — matches MultiLeader::new() default
    let mut text_top_attachment: i16 = 9; // CenterOfText — matches MultiLeader::new() default
    if version.r2010_plus() {
        text_attachment_direction = reader.read_bit_short();
        text_bottom_attachment = reader.read_bit_short();
        text_top_attachment = reader.read_bit_short();
    }

    let mut extend_leader_to_text = false;
    if version.r2013_plus(dxf_version) {
        extend_leader_to_text = reader.read_bit();
    }

    MultiLeaderData {
        context,
        style_handle,
        property_override_flags,
        path_type,
        line_color,
        line_type_handle,
        line_weight,
        enable_landing,
        enable_dogleg,
        dogleg_length,
        arrowhead_handle,
        arrowhead_size,
        content_type,
        text_style_handle,
        text_left_attachment,
        text_right_attachment,
        text_angle_type,
        text_alignment,
        text_color,
        text_frame,
        block_content_handle,
        block_content_color,
        block_scale,
        block_rotation,
        block_connection_type,
        enable_annotation_scale,
        block_attributes,
        text_direction_negative,
        text_align_in_ipe,
        text_attachment_point,
        scale_factor,
        text_attachment_direction,
        text_bottom_attachment,
        text_top_attachment,
        extend_leader_to_text,
    }
}

fn read_multileader_annotation_context(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> MultiLeaderAnnotContext {
    // Leader root count
    let leader_root_count = safe_count(reader.read_bit_long());

    // Read each leader root
    let mut leader_roots = Vec::with_capacity(leader_root_count as usize);
    for _ in 0..leader_root_count {
        leader_roots.push(read_leader_root(reader, version, dxf_version));
    }

    // Common data
    let scale_factor = reader.read_bit_double();
    let content_base_point = reader.read_3bit_double();
    let text_height = reader.read_bit_double();
    let arrowhead_size = reader.read_bit_double();
    let landing_gap = reader.read_bit_double();
    let text_left_attachment = TextAttachmentType::from(reader.read_bit_short());
    let text_right_attachment = TextAttachmentType::from(reader.read_bit_short());
    let text_alignment = TextAlignmentType::from(reader.read_bit_short());
    let block_connection_type = BlockContentConnectionType::from(reader.read_bit_short());

    let has_text_contents = reader.read_bit();

    let mut text_string = String::new();
    let mut text_normal = Vector3::ZERO;
    let mut text_style_handle: Option<Handle> = None;
    let mut text_location = Vector3::ZERO;
    let mut text_direction = Vector3::UNIT_X;
    let mut text_rotation = 0.0;
    let mut text_width = 0.0;
    let mut text_boundary_height = 0.0;
    let mut line_spacing_factor = 1.0;
    let mut line_spacing_style = LineSpacingStyle::default();
    let mut text_color = Color::ByLayer;
    let mut text_attachment_point = TextAttachmentPointType::default();
    let mut text_flow_direction = FlowDirectionType::default();
    let mut background_fill_color = Color::ByLayer;
    let mut background_scale_factor = 1.5;
    let mut background_transparency = 0i32;
    let mut background_fill_enabled = false;
    let mut background_mask_fill_on = false;
    let mut column_type = 0i16;
    let mut text_height_automatic = false;
    let mut column_width = 0.0;
    let mut column_gutter = 0.0;
    let mut column_flow_reversed = false;
    let mut column_sizes: Vec<f64> = Vec::new();
    let mut word_break = false;

    if has_text_contents {
        text_string = reader.read_variable_text();
        text_normal = reader.read_3bit_double();
        let ts_handle = reader.read_handle();
        text_style_handle = if ts_handle != 0 { Some(Handle::from(ts_handle)) } else { None };
        text_location = reader.read_3bit_double();
        text_direction = reader.read_3bit_double();
        text_rotation = reader.read_bit_double();
        text_width = reader.read_bit_double();
        text_boundary_height = reader.read_bit_double();
        line_spacing_factor = reader.read_bit_double();
        line_spacing_style = LineSpacingStyle::from(reader.read_bit_short());
        text_color = reader.read_cm_color();
        text_attachment_point = TextAttachmentPointType::from(reader.read_bit_short());
        text_flow_direction = FlowDirectionType::from(reader.read_bit_short());
        background_fill_color = reader.read_cm_color();
        background_scale_factor = reader.read_bit_double();
        background_transparency = reader.read_bit_long();
        background_fill_enabled = reader.read_bit();
        background_mask_fill_on = reader.read_bit();
        column_type = reader.read_bit_short();
        text_height_automatic = reader.read_bit();
        column_width = reader.read_bit_double();
        column_gutter = reader.read_bit_double();
        column_flow_reversed = reader.read_bit();

        let col_count = safe_count(reader.read_bit_long());
        column_sizes = Vec::with_capacity(col_count as usize);
        for _ in 0..col_count {
            column_sizes.push(reader.read_bit_double());
        }

        word_break = reader.read_bit();
        let _unknown = reader.read_bit();
    }

    // has_block_contents bit is only present when has_text_contents is false
    // (else-if structure in the DWG format — text and block are mutually exclusive)
    let mut has_block_contents = false;

    let mut block_content_handle: Option<Handle> = None;
    let mut block_content_normal = Vector3::UNIT_Z;
    let mut block_content_location = Vector3::ZERO;
    let mut block_content_scale = Vector3::new(1.0, 1.0, 1.0);
    let mut block_rotation = 0.0;
    let mut block_content_color = Color::ByBlock;
    let mut transform_matrix = [0.0f64; 16];
    // Set identity
    transform_matrix[0] = 1.0;
    transform_matrix[5] = 1.0;
    transform_matrix[10] = 1.0;
    transform_matrix[15] = 1.0;

    if !has_text_contents {
        has_block_contents = reader.read_bit();

        if has_block_contents {
            let bh = reader.read_handle();
            block_content_handle = if bh != 0 { Some(Handle::from(bh)) } else { None };
            block_content_normal = reader.read_3bit_double();
            block_content_location = reader.read_3bit_double();
            block_content_scale = reader.read_3bit_double();
            block_rotation = reader.read_bit_double();
            block_content_color = reader.read_cm_color();

            for i in 0..16 {
                transform_matrix[i] = reader.read_bit_double();
            }
        }
    }

    let base_point = reader.read_3bit_double();
    let base_direction = reader.read_3bit_double();
    let base_vertical = reader.read_3bit_double();
    let normal_reversed = reader.read_bit();

    let mut text_top_attachment = TextAttachmentType::CenterOfText;
    let mut text_bottom_attachment = TextAttachmentType::CenterOfText;
    if version.r2010_plus() {
        text_top_attachment = TextAttachmentType::from(reader.read_bit_short());
        text_bottom_attachment = TextAttachmentType::from(reader.read_bit_short());
    }

    MultiLeaderAnnotContext {
        leader_roots,
        scale_factor,
        content_base_point,
        has_text_contents,
        text_string,
        text_normal,
        text_location,
        text_direction,
        text_rotation,
        text_height,
        text_width,
        text_boundary_height,
        line_spacing_factor,
        line_spacing_style,
        text_color,
        text_attachment_point,
        text_flow_direction,
        text_alignment,
        text_left_attachment,
        text_right_attachment,
        text_top_attachment,
        text_bottom_attachment,
        text_height_automatic,
        word_break,
        text_style_handle,
        has_block_contents,
        block_content_handle,
        block_content_normal,
        block_content_location,
        block_content_scale,
        block_rotation,
        block_content_color,
        block_connection_type,
        column_type,
        column_width,
        column_gutter,
        column_flow_reversed,
        column_sizes,
        background_fill_enabled,
        background_mask_fill_on,
        background_fill_color,
        background_scale_factor,
        background_transparency,
        base_point,
        base_direction,
        base_vertical,
        normal_reversed,
        arrowhead_size,
        landing_gap,
        transform_matrix,
        scale_handle: None,
    }
}

fn read_leader_root(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    _dxf_version: DxfVersion,
) -> LeaderRoot {
    let content_valid = reader.read_bit();
    let unknown = reader.read_bit();
    let connection_point = reader.read_3bit_double();
    let direction = reader.read_3bit_double();

    let bp_count = safe_count(reader.read_bit_long());
    let mut break_points = Vec::with_capacity(bp_count as usize);
    for _ in 0..bp_count {
        let start_point = reader.read_3bit_double();
        let end_point = reader.read_3bit_double();
        break_points.push(StartEndPointPair { start_point, end_point });
    }

    let leader_index = reader.read_bit_long();
    let landing_distance = reader.read_bit_double();

    let line_count = safe_count(reader.read_bit_long());
    let mut lines = Vec::with_capacity(line_count as usize);
    for _ in 0..line_count {
        lines.push(read_leader_line(reader, version));
    }

    let mut text_attachment_direction = TextAttachmentDirectionType::default();
    if version.r2010_plus() {
        text_attachment_direction = TextAttachmentDirectionType::from(reader.read_bit_short());
    }

    LeaderRoot {
        content_valid,
        unknown,
        connection_point,
        direction,
        break_points,
        leader_index,
        landing_distance,
        lines,
        text_attachment_direction,
    }
}

fn read_leader_line(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
) -> LeaderLine {
    let pt_count = safe_count(reader.read_bit_long());
    let mut points = Vec::with_capacity(pt_count as usize);
    for _ in 0..pt_count {
        points.push(reader.read_3bit_double());
    }

    let break_info_count = reader.read_bit_long();
    let mut segment_index = 0;
    let mut break_points = Vec::new();
    if break_info_count > 0 {
        segment_index = reader.read_bit_long();
        let sep_count = safe_count(reader.read_bit_long());
        break_points = Vec::with_capacity(sep_count as usize);
        for _ in 0..sep_count {
            let start_point = reader.read_3bit_double();
            let end_point = reader.read_3bit_double();
            break_points.push(StartEndPointPair { start_point, end_point });
        }
    }

    let index = reader.read_bit_long();

    let mut path_type = MultiLeaderPathType::default();
    let mut line_color = Color::ByBlock;
    let mut line_type_handle: Option<Handle> = None;
    let mut line_weight = crate::types::LineWeight::ByLayer;
    let mut arrowhead_size = 0.18;
    let mut arrowhead_handle: Option<Handle> = None;
    let mut override_flags = LeaderLinePropertyOverrideFlags::NONE;

    if version.r2010_plus() {
        path_type = MultiLeaderPathType::from(reader.read_bit_short());
        line_color = reader.read_cm_color();
        let lt_handle = reader.read_handle();
        line_type_handle = if lt_handle != 0 { Some(Handle::from(lt_handle)) } else { None };
        let lw = reader.read_bit_long();
        line_weight = crate::types::LineWeight::from_value(lw as i16);
        arrowhead_size = reader.read_bit_double();
        let ah_handle = reader.read_handle();
        arrowhead_handle = if ah_handle != 0 { Some(Handle::from(ah_handle)) } else { None };
        override_flags = LeaderLinePropertyOverrideFlags::from_bits_truncate(reader.read_bit_long() as u32);
    }

    LeaderLine {
        points,
        break_info_count,
        segment_index,
        break_points,
        index,
        path_type,
        line_color,
        line_type_handle,
        line_weight,
        arrowhead_size,
        arrowhead_handle,
        override_flags,
    }
}

// ════════════════════════════════════════════════════════════════════════
//  ACIS / Modeler-geometry readers (3DSOLID, REGION, BODY)
// ════════════════════════════════════════════════════════════════════════

/// Data returned by the ACIS entity reader (shared between 3DSOLID, REGION, BODY).
#[derive(Debug, Clone)]
pub struct AcisEntityData {
    /// True when the entity carries no ACIS data (empty body).
    pub acis_empty: bool,
    /// SAT text data (version 1, pre-R2007).
    pub sat_data: String,
    /// SAB binary data (version 2, R2007+).
    pub sab_data: Vec<u8>,
    /// Whether the data is binary SAB (true) or text SAT (false).
    pub is_binary: bool,
    /// ACIS version marker as read from the stream.
    pub version: i16,
    /// Point on entity (wireframe anchor), if wireframe data was present.
    pub point: Vector3,
    /// Whether the entity has a history handle (3DSOLID only, R2007+).
    pub has_history: bool,
    /// Wireframe edges for visualization.
    pub wires: Vec<Wire>,
    /// Silhouette data for viewports.
    pub silhouettes: Vec<Silhouette>,
}

/// Read modeler-geometry (ACIS) data shared by 3DSOLID, REGION, BODY.
///
/// This reads both `DECODE_3DSOLID` (acis data) and the wireframe +
/// `acis_empty_bit` + R2007+ trailing fields from `COMMON_3DSOLID`.
/// The caller must still read the 3DSOLID-specific history_id handle.
pub fn read_acis_entity(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
) -> AcisEntityData {
    let acis_empty = reader.read_bit();

    let mut sat_data = String::new();
    let mut sab_data = Vec::new();
    let mut is_binary = false;
    let mut acis_version: i16 = 0;

    if !acis_empty {
        // Unknown bit — per ODA spec / LibreDWG / ACadSharp this B
        // is always present between acis_empty and the version BS.
        let _unknown = reader.read_bit();

        acis_version = reader.read_bit_short();

        if acis_version == 1 {
            // SAT text — all DWG versions use the same encoding:
            // BL-sized blocks of encrypted bytes (cipher: 159 - byte)
            // terminated by BL(0).  Per LibreDWG dwg.spec.
            is_binary = false;

            let mut all_bytes = Vec::new();
            loop {
                let block_size = reader.read_bit_long().max(0) as usize;
                if block_size == 0 || block_size > 50_000_000 {
                    break;
                }
                let block = reader.read_bytes(block_size);
                all_bytes.extend_from_slice(&block);
            }

            // Decrypt with selective 159-substitution cipher
            // (per LibreDWG dwg.spec: bytes <= 32 pass through, bytes > 32: 159 - byte)
            let mut decoded = Vec::with_capacity(all_bytes.len());
            for &b in &all_bytes {
                if b <= 32 {
                    decoded.push(b);
                } else {
                    decoded.push(159u8.wrapping_sub(b));
                }
            }
            sat_data = String::from_utf8_lossy(&decoded).to_string();
            sat_data = crate::entities::solid3d::AcisData::strip_sat_terminator(&sat_data);
        } else {
            // SAB binary (version=2, R2007+):
            //
            // The SAB data starts IMMEDIATELY here — NO BL size prefix.
            // The data begins with "ACIS BinaryFile" header followed by
            // the full ODA ASM binary body. The SAB data runs from the
            // current main reader position for exactly
            //   floor((flag_position - current_pos) / 8) bytes
            // where flag_position = handle_start - 1.
            //
            // After the SAB body, the RemainingBits mod 8 = 3 trailing
            // entity-data bits are left unread:
            //   bit+0: wireframe_present = 0
            //   bit+1: MSB of BL:unknown_2007 = 1  ("10" indicator)
            //   bit+2: LSB of BL:unknown_2007 = 0
            // The flag bit (text-stream indicator = 0) sits at bit+3 =
            // flag_position and is written by the merged-stream writer.
            // We return early; the caller reads handles from the handle
            // stream (which is independent of main stream position).
            is_binary = true;
            let current_pos = reader.main_mut().position_in_bits();
            let handle_start = reader.handle_start();
            let remaining_bits = (handle_start - 1 - current_pos).max(0) as usize;
            let sab_bytes = remaining_bits / 8;
            if sab_bytes > 0 {
                sab_data = reader.read_bytes(sab_bytes);
            }
            return AcisEntityData {
                acis_empty,
                sat_data,
                sab_data,
                is_binary,
                version: acis_version,
                point: crate::types::Vector3::ZERO,
                has_history: false,
                wires: Vec::new(),
                silhouettes: Vec::new(),
            };
        }
    }

    // Wireframe data (version=1 SAT only; version=2 SAB returns early above)
    let wireframe_present = reader.read_bit();
    let mut point = Vector3::ZERO;
    let mut wires = Vec::new();

    if wireframe_present {
        point = reader.read_3bit_double();
        let raw_isolines = reader.read_bit_long();
        let num_isolines = safe_count(raw_isolines);
        for _ in 0..num_isolines {
            wires.push(read_wire(reader));
        }
    }

    // Silhouettes (inside wireframe section per LibreDWG)
    let mut silhouettes = Vec::new();
    if wireframe_present {
        let num_silhouettes = safe_count(reader.read_bit_long());
        for _ in 0..num_silhouettes {
            let viewport_id = reader.read_bit_long() as i64;
            let view_direction = reader.read_3bit_double();
            let up_vector = reader.read_3bit_double();
            let target = reader.read_3bit_double();
            let is_perspective = reader.read_bit();
            let num_wires = safe_count(reader.read_bit_long());
            let mut sil_wires = Vec::with_capacity(num_wires as usize);
            for _ in 0..num_wires {
                sil_wires.push(read_wire(reader));
            }
            silhouettes.push(Silhouette {
                viewport_id,
                view_direction,
                up_vector,
                target,
                is_perspective,
                wires: sil_wires,
            });
        }
    }

    // acis_empty_bit (COMMON_3DSOLID — always present)
    let _acis_empty_bit = reader.read_bit();

    // R2007+: unknown BL field (COMMON_3DSOLID)
    if version.r2007_plus() {
        let _unknown_2007 = reader.read_bit_long();
    }

    AcisEntityData {
        acis_empty,
        sat_data,
        sab_data,
        is_binary,
        version: acis_version,
        point,
        has_history: false,
        wires,
        silhouettes,
    }
}

/// Read a single wire struct from the DWG stream.
fn read_wire(reader: &mut DwgMergedReader) -> Wire {
    let acis_index = reader.read_bit_long();
    let wire_type_raw = reader.read_byte();
    let selection_marker = reader.read_bit_long();
    let color_val = reader.read_bit_long();
    let num_pts = safe_count(reader.read_bit_long());
    let mut pts = Vec::with_capacity(num_pts as usize);
    for _ in 0..num_pts {
        pts.push(reader.read_3bit_double());
    }
    let has_transform = reader.read_bit();
    let (mut x_axis, mut y_axis, mut z_axis) =
        (Vector3::UNIT_X, Vector3::UNIT_Y, Vector3::UNIT_Z);
    let mut translation = Vector3::ZERO;
    let mut scale = 1.0;
    let (mut has_rotation, mut has_reflection, mut has_shear) =
        (false, false, false);
    if has_transform {
        x_axis = reader.read_3bit_double();
        y_axis = reader.read_3bit_double();
        z_axis = reader.read_3bit_double();
        translation = reader.read_3bit_double();
        scale = reader.read_bit_double();
        has_rotation = reader.read_bit();
        has_reflection = reader.read_bit();
        has_shear = reader.read_bit();
    }
    let color = if color_val == 256 {
        Color::ByLayer
    } else if color_val == 0 {
        Color::ByBlock
    } else {
        Color::Index(color_val as u8)
    };
    Wire {
        acis_index,
        wire_type: WireType::from(wire_type_raw),
        selection_marker,
        color,
        points: pts,
        has_transform,
        has_rotation,
        has_reflection,
        has_shear,
        scale,
        translation,
        x_axis,
        y_axis,
        z_axis,
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Tests
// ════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter;
    use crate::io::dwg::dwg_version::DwgVersion;
    use crate::types::DxfVersion;

    fn make_reader(dwg: DwgVersion, dxf: DxfVersion, f: impl FnOnce(&mut DwgMergedWriter)) -> DwgMergedReader {
        let mut writer = DwgMergedWriter::new(dwg, dxf);
        f(&mut writer);
        let data = writer.merge();
        let hsb = writer.handle_start_bits();
        DwgMergedReader::new(data, dxf, hsb)
    }

    #[test]
    fn test_point_roundtrip() {
        let v = DwgVersion::AC15;
        let d = DxfVersion::AC1015;
        let mut r = make_reader(v, d, |w| {
            w.write_3bit_double(Vector3::new(1.0, 2.0, 3.0));
            w.write_bit_thickness(0.5);
            w.write_bit_extrusion(Vector3::UNIT_Z);
            w.write_bit_double(45.0);
        });
        let pt = read_point(&mut r);
        assert_eq!(pt.location, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(pt.thickness, 0.5);
        assert_eq!(pt.x_axis_angle, 45.0);
    }

    #[test]
    fn test_line_roundtrip_r2000() {
        let v = DwgVersion::AC15;
        let d = DxfVersion::AC1015;
        let mut r = make_reader(v, d, |w| {
            w.write_bit(false); // z_are_zero = false
            w.write_raw_double(1.0); // start.x
            w.write_bit_double_with_default(4.0, 1.0); // end.x
            w.write_raw_double(2.0); // start.y
            w.write_bit_double_with_default(5.0, 2.0); // end.y
            w.write_raw_double(3.0); // start.z
            w.write_bit_double_with_default(6.0, 3.0); // end.z
            w.write_bit_thickness(0.0);
            w.write_bit_extrusion(Vector3::UNIT_Z);
        });
        let ln = read_line(&mut r, v);
        assert_eq!(ln.start, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(ln.end, Vector3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn test_circle_roundtrip() {
        let v = DwgVersion::AC15;
        let d = DxfVersion::AC1015;
        let mut r = make_reader(v, d, |w| {
            w.write_3bit_double(Vector3::new(10.0, 20.0, 0.0));
            w.write_bit_double(5.0);
            w.write_bit_thickness(0.0);
            w.write_bit_extrusion(Vector3::UNIT_Z);
        });
        let c = read_circle(&mut r);
        assert_eq!(c.center, Vector3::new(10.0, 20.0, 0.0));
        assert_eq!(c.radius, 5.0);
    }

    #[test]
    fn test_ellipse_roundtrip() {
        let v = DwgVersion::AC15;
        let d = DxfVersion::AC1015;
        let mut r = make_reader(v, d, |w| {
            w.write_3bit_double(Vector3::new(5.0, 5.0, 0.0));
            w.write_3bit_double(Vector3::new(10.0, 0.0, 0.0));
            w.write_3bit_double(Vector3::UNIT_Z);
            w.write_bit_double(0.5);
            w.write_bit_double(0.0);
            w.write_bit_double(std::f64::consts::TAU);
        });
        let e = read_ellipse(&mut r);
        assert_eq!(e.center, Vector3::new(5.0, 5.0, 0.0));
        assert_eq!(e.major_axis, Vector3::new(10.0, 0.0, 0.0));
        assert_eq!(e.minor_axis_ratio, 0.5);
    }

    #[test]
    fn test_insert_roundtrip_r2000() {
        let v = DwgVersion::AC15;
        let d = DxfVersion::AC1015;
        let mut r = make_reader(v, d, |w| {
            w.write_3bit_double(Vector3::new(100.0, 200.0, 0.0));
            w.write_2bits(3); // all-ones scale
            w.write_bit_double(0.0); // rotation
            w.write_3bit_double(Vector3::UNIT_Z); // normal
            w.write_bit(false); // has_attribs
            w.write_handle(crate::io::dwg::dwg_reference_type::DwgReferenceType::HardPointer, 0x50);
        });
        let ins = read_insert(&mut r, v);
        assert_eq!(ins.insert_point, Vector3::new(100.0, 200.0, 0.0));
        assert_eq!(ins.x_scale, 1.0);
        assert_eq!(ins.y_scale, 1.0);
        assert_eq!(ins.z_scale, 1.0);
        assert_eq!(ins.block_handle, 0x50);
    }

    #[test]
    fn test_spline_roundtrip_scenario1() {
        let v = DwgVersion::AC15;
        let d = DxfVersion::AC1015;
        let mut r = make_reader(v, d, |w| {
            w.write_bit_long(1); // scenario
            w.write_bit_long(3); // degree
            w.write_bit(false); // rational
            w.write_bit(false); // closed
            w.write_bit(false); // periodic
            w.write_bit_double(1e-10); // knot_tol
            w.write_bit_double(1e-10); // ctrl_tol
            w.write_bit_long(6); // num_knots
            w.write_bit_long(3); // num_ctrl
            w.write_bit(false); // has_weights
            for k in &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0] {
                w.write_bit_double(*k);
            }
            w.write_3bit_double(Vector3::new(0.0, 0.0, 0.0));
            w.write_3bit_double(Vector3::new(5.0, 5.0, 0.0));
            w.write_3bit_double(Vector3::new(10.0, 0.0, 0.0));
        });
        let sp = read_spline(&mut r, v, d);
        assert_eq!(sp.scenario, 1);
        assert_eq!(sp.degree, 3);
        assert_eq!(sp.knots.len(), 6);
        assert_eq!(sp.control_points.len(), 3);
    }

    #[test]
    fn test_acis_sat_roundtrip_r2004() {
        use crate::entities::solid3d::AcisData;

        let sat = "700 0 1 0\n\
                   @7 unknown 12 ACIS 7.0 NT 24 Wed Jan 01 00:00:00 2025 1.0 9.9999999999999995e-007 1e-010\n\
                   body $-1 $1 $-1 $-1 #\n\
                   lump $-1 $-1 $2 $0 #\n\
                   shell $-1 $-1 $-1 $3 $-1 $1 #\n\
                   face $-1 $-1 $-1 $4 $2 $5 forward single #\n\
                   loop $-1 $-1 $6 $3 #\n\
                   plane-surface $-1 0 0 5 0 0 1 1 0 0 forward_v I I I I #\n\
                   coedge $-1 $6 $6 $-1 $7 forward $4 $-1 #\n\
                   edge $-1 $8 0 $8 1 $6 $9 forward #\n\
                   vertex $-1 $7 $10 #\n\
                   straight-curve $-1 -5 -5 5 1 0 0 I I #\n\
                   point $-1 -5 -5 5 #\n\
                   End-of-ACIS-data\n";

        // Write using the writer infrastructure
        let v = DwgVersion::AC18;
        let d = DxfVersion::AC1018;
        let acis = AcisData::from_sat(sat);
        let mut w = DwgMergedWriter::new(v, d);
        // Write: acis_empty + unknown + version + encrypted blocks + wireframe + acis_empty_bit
        w.write_bit(false); // acis_empty = false (has data)
        w.write_bit(false); // unknown bit (per ODA/LibreDWG spec)
        w.write_bit_short(1); // acis_version = 1 (SAT text)
        // Encrypt SAT with selective 159-substitution cipher
        let mut full_sat = acis.sat_data.clone();
        full_sat.push_str("End-of-ACIS-data\n");
        let plain = full_sat.as_bytes();
        let mut encrypted = Vec::with_capacity(plain.len());
        for &b in plain.iter() {
            if b <= 32 {
                encrypted.push(b);
            } else {
                encrypted.push(159u8.wrapping_sub(b));
            }
        }
        w.write_bit_long(encrypted.len() as i32);
        w.write_bytes(&encrypted);
        w.write_bit_long(0); // terminating empty block
        w.write_bit(false); // wireframe_present = false
        w.write_bit(false); // acis_empty_bit

        let data = w.merge();
        let hsb = w.handle_start_bits();
        let mut r = DwgMergedReader::new(data, d, hsb);

        let result = read_acis_entity(&mut r, v);
        assert!(!result.acis_empty);
        assert!(!result.is_binary);
        assert_eq!(result.version, 1);
        assert!(result.sat_data.contains("body"));
        assert!(result.sat_data.contains("plane-surface"));
        assert!(result.wires.is_empty());
    }

    #[test]
    fn test_acis_sab_roundtrip_r2007() {
        // Test SAB binary roundtrip (version 2)
        // The reader calculates SAB size from bit positions, not a BL prefix.
        let v = DwgVersion::AC21;
        let d = DxfVersion::AC1021;

        let sab_data: Vec<u8> = vec![
            0x41, 0x53, 0x4D, 0x20, // "ASM "
            0x42, 0x69, 0x6E, 0x00, // "Bin\0"
            0x01, 0x02, 0x03, 0x04, // some dummy data
        ];

        let mut w = DwgMergedWriter::new(v, d);
        w.write_bit(false); // acis_empty = false
        w.write_bit(false); // unknown bit (per ODA/LibreDWG spec)
        w.write_bit_short(2); // acis_version = 2 (SAB binary)
        // NO BL size prefix — reader infers size from remaining bits
        w.write_bytes(&sab_data);
        w.write_bit(false); // wireframe_present = false
        w.write_bit(false); // acis_empty_bit

        let data = w.merge();
        let hsb = w.handle_start_bits();
        let mut r = DwgMergedReader::new(data, d, hsb);
        r.set_handle_start(hsb); // required for SAB size calculation

        let result = read_acis_entity(&mut r, v);
        assert!(!result.acis_empty);
        assert!(result.is_binary);
        assert_eq!(result.version, 2);
        assert_eq!(result.sab_data, sab_data);
        assert!(result.sat_data.is_empty());
    }

    #[test]
    fn test_acis_empty_roundtrip() {
        let v = DwgVersion::AC15;
        let d = DxfVersion::AC1015;

        let mut w = DwgMergedWriter::new(v, d);
        w.write_bit(true); // acis_empty = true
        w.write_bit(false); // wireframe_present = false
        w.write_bit(false); // acis_empty_bit

        let data = w.merge();
        let hsb = w.handle_start_bits();
        let mut r = DwgMergedReader::new(data, d, hsb);

        let result = read_acis_entity(&mut r, v);
        assert!(result.acis_empty);
        assert!(result.sat_data.is_empty());
        assert!(result.sab_data.is_empty());
    }
}
