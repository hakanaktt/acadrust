use crate::entities::solid3d::AcisVersion;
use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use crate::io::dwg::dwg_version::DwgVersion;
use crate::objects::*;
use crate::types::{DxfVersion, Handle};

use super::safe_count;

fn read_edge_curve(
    reader: &mut DwgMergedReader,
    curve_type: i32,
    out: &mut Vec<AssocCurveValue>,
    depth: usize,
) {
    let point = |reader: &mut DwgMergedReader| AssocCurveValue::Point(reader.read_3bit_double());
    let real = |reader: &mut DwgMergedReader| AssocCurveValue::Real(reader.read_bit_double());
    match curve_type {
        // Arc and ellipse: three vectors, then four or five reals.
        11 | 17 => {
            for _ in 0..3 {
                out.push(point(reader));
            }
            let scalar_count = if curve_type == 11 { 4 } else { 5 };
            for index in 0..scalar_count {
                // AutoCAD writes a terminal AC1024 edge arc without the
                // fourth scalar; AC1027 and later add it. Never infer a
                // missing value in nested curves, other versions, or a
                // partially read scalar.
                if curve_type == 11
                    && index == 3
                    && depth == 0
                    && reader.dxf_version() == DxfVersion::AC1024
                    && !reader.has_error()
                    && reader.main_remaining_bits() == 0
                {
                    break;
                }
                out.push(real(reader));
            }
        }
        // An unbounded line (a point and its direction) and a line segment
        // (its two ends).
        19 | 23 => {
            out.push(point(reader));
            out.push(point(reader));
        }
        42 => {
            out.push(AssocCurveValue::Bool(reader.read_bit()));
            out.push(AssocCurveValue::Bool(reader.read_bit()));
            out.push(AssocCurveValue::Int(reader.read_bit_long()));
            out.push(real(reader));
            // Knots, weights and control points: length, physical length and
            // grow length, then the items.
            for points in [false, false, true] {
                let length = reader.read_bit_long();
                out.push(AssocCurveValue::Int(length));
                out.push(AssocCurveValue::Int(reader.read_bit_long()));
                out.push(AssocCurveValue::Int(reader.read_bit_long()));
                for _ in 0..safe_count(length, reader) {
                    out.push(if points { point(reader) } else { real(reader) });
                }
            }
        }
        47 if depth < 8 => {
            let count = reader.read_bit_long();
            out.push(AssocCurveValue::Int(count));
            for _ in 0..safe_count(count, reader) {
                let part = reader.read_bit_long();
                out.push(AssocCurveValue::Int(part));
                read_edge_curve(reader, part, out, depth + 1);
            }
        }
        _ => {}
    }
}

fn handle(reader: &mut DwgMergedReader) -> Handle {
    Handle::from(reader.read_handle())
}

fn read_handles(reader: &mut DwgMergedReader, count: i32) -> Vec<Handle> {
    let count = safe_count(count, reader);
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(handle(reader));
    }
    values
}

fn eval_kind(code: i16) -> u8 {
    match code {
        // 330-369: soft/hard pointer and owner handles.
        i16::MIN..=-1 | 5 | 105 | 320..=369 | 390..=399 => 6,
        0..=9 | 100..=102 | 300..=309 | 410..=419 | 430..=439 | 470..=479 | 999 | 1000..=1009 => 5,
        10..=37 | 110..=139 | 210..=269 | 1010..=1039 | 1043..=1069 => 0,
        38..=59 | 140..=149 | 460..=469 | 1040..=1042 => 1,
        60..=79 | 170..=179 | 270..=279 | 370..=389 | 400..=409 | 1070 => 3,
        80..=99 | 420..=429 | 440..=459 | 1071 => 2,
        280..=289 => 4,
        _ => 0,
    }
}

fn read_eval_variant(reader: &mut DwgMergedReader) -> AssocEvalVariant {
    let code = reader.read_bit_short();
    let value = if code == 0 {
        AssocEvalValue::None
    } else {
        match eval_kind(code) {
            1 => AssocEvalValue::Real(reader.read_bit_double()),
            2 => AssocEvalValue::Long(reader.read_bit_long()),
            3 => AssocEvalValue::Short(reader.read_bit_short()),
            4 => AssocEvalValue::Byte(reader.read_byte()),
            5 => AssocEvalValue::Text(reader.read_variable_text()),
            6 => AssocEvalValue::Handle(handle(reader)),
            _ => AssocEvalValue::None,
        }
    };
    AssocEvalVariant { code, value }
}

fn read_value_param(reader: &mut DwgMergedReader) -> AssocValueParam {
    let class_version = reader.read_bit_long();
    let name = reader.read_variable_text();
    let unit_type = reader.read_bit_long();
    let count = safe_count(reader.read_bit_long(), reader);
    let mut variables = Vec::with_capacity(count as usize);
    for _ in 0..count {
        variables.push(AssocValueParamVariable {
            value: read_eval_variant(reader),
            handle: handle(reader),
        });
    }
    AssocValueParam {
        class_version,
        name,
        unit_type,
        variables,
        controlled_object_dependency: handle(reader),
    }
}

fn read_value_params(reader: &mut DwgMergedReader, count: i32) -> Vec<AssocValueParam> {
    let count = safe_count(count, reader);
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(read_value_param(reader));
    }
    values
}

fn read_dependency(reader: &mut DwgMergedReader) -> AssocDependency {
    let class_version = reader.read_bit_short();
    let status = reader.read_bit_long();
    let is_read_dependency = reader.read_bit();
    let is_write_dependency = reader.read_bit();
    let is_attached_to_object = reader.read_bit();
    let is_delegating_to_owning_action = reader.read_bit();
    let order = reader.read_bit_long();
    let dependent_on = handle(reader);
    let has_name = reader.read_bit();
    let name = has_name.then(|| reader.read_variable_text());
    AssocDependency {
        class_version,
        status,
        is_read_dependency,
        is_write_dependency,
        is_attached_to_object,
        is_delegating_to_owning_action,
        order,
        dependent_on,
        name,
        read_dependency: handle(reader),
        node: handle(reader),
        dependency_body: handle(reader),
        dependency_body_id: reader.read_bit_long(),
    }
}

/// Decode the persistent-subentity tail of `ASSOCGEOMDEPENDENCY` without
/// allowing an omitted optional field to invalidate the complete object.
///
/// AC1024 records use a class name in the text stream and only the compound
/// flag in the main stream.  AC1027+ records normally use the compact class
/// code layout.  Both forms occur in files produced by different writers,
/// and damaged auxiliary records can end after any prefix, so each optional
/// read is bounded before it is attempted.
fn read_persistent_subent(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> AssocPersistentSubentId {
    let compact_layout = version.r2013_plus(dxf_version)
        && reader.text_remaining_bits() == 0
        // The known compact class codes (1, 3 and 5) use a one-byte BL
        // payload: leading flag + 2-bit prefix + 8-bit code + compound flag.
        // With fewer bits left, prefer the text form so a lone compound flag
        // and byte padding are not consumed as a fabricated compact value.
        && reader.main_remaining_bits() >= 12;

    if compact_layout {
        let leading_flag = reader.read_bit();
        let class_code = reader.try_read_bit_long().unwrap_or_default();
        let class_name = AssocPersistentSubentId::class_name_for_code(class_code)
            .unwrap_or_default()
            .to_string();
        let mut values = Vec::new();
        if let Some(count) = AssocPersistentSubentId::field_count_for_class_name(&class_name) {
            values.reserve(count);
            for _ in 0..count {
                let Some(value) = reader.try_read_bit_long() else {
                    break;
                };
                values.push(value);
            }
        }
        let dependent_on_compound_object = reader.main_remaining_bits() >= 1 && reader.read_bit();
        AssocPersistentSubentId {
            class_name,
            dependent_on_compound_object,
            class_code,
            values,
            leading_flag,
        }
    } else {
        let class_name = reader.try_read_variable_text().unwrap_or_default();
        let class_code =
            AssocPersistentSubentId::code_for_class_name(&class_name).unwrap_or_default();
        let dependent_on_compound_object = reader.main_remaining_bits() >= 1 && reader.read_bit();
        AssocPersistentSubentId {
            class_name,
            dependent_on_compound_object,
            class_code,
            values: Vec::new(),
            leading_flag: false,
        }
    }
}

fn read_action(reader: &mut DwgMergedReader) -> AssocAction {
    let class_version = reader.read_bit_short();
    let geometry_status = reader.read_bit_long();
    let owning_network = handle(reader);
    let action_body = handle(reader);
    let action_index = reader.read_bit_long();
    let max_dependency_index = reader.read_bit_long();
    let count = safe_count(reader.read_bit_long(), reader);
    let mut dependencies = Vec::with_capacity(count as usize);
    for _ in 0..count {
        dependencies.push(AssocActionDependency {
            is_owned: reader.read_bit(),
            dependency: handle(reader),
        });
    }
    let mut owned_parameters = Vec::new();
    let mut values = Vec::new();
    if class_version > 1 {
        let _zero = reader.read_bit_short();
        let count = reader.read_bit_long();
        owned_parameters = read_handles(reader, count);
        let _zero = reader.read_bit_short();
        let count = reader.read_bit_long();
        values = read_value_params(reader, count);
    }
    AssocAction {
        class_version,
        geometry_status,
        owning_network,
        action_body,
        action_index,
        max_dependency_index,
        dependencies,
        owned_parameters,
        values,
    }
}

fn read_action_param(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> AssocActionParam {
    let is_r2013 = reader.read_bit_short();
    let version_value = if version.r2013_plus(dxf_version) {
        reader.read_bit_long()
    } else {
        0
    };
    AssocActionParam {
        is_r2013,
        version: version_value,
        name: reader.read_variable_text(),
    }
}

fn read_action_body(reader: &mut DwgMergedReader) -> AssocActionBody {
    AssocActionBody {
        version: reader.read_bit_long(),
    }
}

fn read_parameter_body(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> AssocParamBasedActionBody {
    if version.r2013_plus(dxf_version) {
        return AssocParamBasedActionBody::default();
    }
    let body_version = reader.read_bit_long();
    let minor = reader.read_bit_long();
    let count = reader.read_bit_long();
    let dependencies = read_handles(reader, count);
    let marker = reader.read_bit_long();
    let value_count = safe_count(reader.read_bit_long(), reader);
    let (empty_value_marker, dependency) = if value_count == 0 {
        (reader.read_bit_long(), handle(reader))
    } else {
        (0, Handle::NULL)
    };
    AssocParamBasedActionBody {
        version: body_version,
        minor,
        dependencies,
        marker,
        values: read_value_params(reader, value_count),
        empty_value_marker,
        dependency,
    }
}

fn read_surface_body(reader: &mut DwgMergedReader) -> AssocSurfaceBody {
    AssocSurfaceBody {
        version: reader.read_bit_long(),
        dependency: handle(reader),
        is_semi_associative: reader.read_bit(),
        marker: reader.read_bit_long(),
        is_semi_override: reader.read_bit(),
        grip_status: reader.read_bit_short(),
    }
}

fn surface_kind(name: &str) -> Option<AssocSurfaceActionKind> {
    Some(match name {
        "ASSOCPLANESURFACEACTIONBODY" => AssocSurfaceActionKind::Plane,
        "ASSOCEXTENDSURFACEACTIONBODY" => AssocSurfaceActionKind::Extend,
        "ASSOCEXTRUDEDSURFACEACTIONBODY" => AssocSurfaceActionKind::Extruded,
        "ASSOCLOFTEDSURFACEACTIONBODY" => AssocSurfaceActionKind::Lofted,
        "ASSOCNETWORKSURFACEACTIONBODY" => AssocSurfaceActionKind::Network,
        "ASSOCOFFSETSURFACEACTIONBODY" => AssocSurfaceActionKind::Offset,
        "ASSOCREVOLVEDSURFACEACTIONBODY" => AssocSurfaceActionKind::Revolved,
        "ASSOCTRIMSURFACEACTIONBODY" => AssocSurfaceActionKind::Trim,
        "ASSOCBLENDSURFACEACTIONBODY" => AssocSurfaceActionKind::Blend,
        "ASSOCPATCHSURFACEACTIONBODY" => AssocSurfaceActionKind::Patch,
        "ASSOCFILLETSURFACEACTIONBODY" => AssocSurfaceActionKind::Fillet,
        "ASSOCSWEPTSURFACEACTIONBODY" => AssocSurfaceActionKind::Swept,
        "ASSOCEDGECHAMFERACTIONBODY" => AssocSurfaceActionKind::EdgeChamfer,
        "ASSOCEDGEFILLETACTIONBODY" => AssocSurfaceActionKind::EdgeFillet,
        _ => return None,
    })
}

fn read_surface_action(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
    kind: AssocSurfaceActionKind,
) -> AssocSurfaceActionBody {
    let action_body = read_action_body(reader);
    let parameter_body = read_parameter_body(reader, version, dxf_version);
    // An R2004 plane-surface action stores the action and parameter bodies
    // only. The surface-body dependency handle and the path/class fields are
    // absent, and reading them walks off the end of the record.
    let has_surface_tail = reader.main_remaining_bits() >= 8
        && reader.handle_remaining_bits() >= 8
        && !reader.has_error();
    let surface_body = if has_surface_tail {
        read_surface_body(reader)
    } else {
        AssocSurfaceBody::default()
    };
    let path_status = if has_surface_tail {
        reader.read_bit_long()
    } else {
        0
    };
    let mut value = AssocSurfaceActionBody {
        kind,
        action_body,
        parameter_body,
        surface_body,
        path_status,
        ..AssocSurfaceActionBody::default()
    };
    match kind {
        AssocSurfaceActionKind::Network
        | AssocSurfaceActionKind::Patch
        | AssocSurfaceActionKind::EdgeChamfer
        | AssocSurfaceActionKind::EdgeFillet => {}
        _ if has_surface_tail => value.class_version = reader.read_bit_long(),
        _ => {}
    }
    match kind {
        AssocSurfaceActionKind::Extend => value.option = reader.read_byte(),
        AssocSurfaceActionKind::Offset => value.flags[0] = reader.read_bit(),
        AssocSurfaceActionKind::Trim => {
            value.flags[0] = reader.read_bit();
            value.flags[1] = reader.read_bit();
            value.distance = reader.read_bit_double();
        }
        AssocSurfaceActionKind::Blend => {
            value.flags[0] = reader.read_bit();
            value.flags[1] = reader.read_bit();
            value.flags[2] = reader.read_bit();
            value.status = reader.read_bit_short();
            value.flags[3] = reader.read_bit();
            value.flags[4] = reader.read_bit();
            value.secondary_status = reader.read_bit_short();
        }
        AssocSurfaceActionKind::Fillet => {
            value.status = reader.read_bit_short();
            value.first_point = reader.read_2raw_double();
            value.second_point = reader.read_2raw_double();
        }
        _ => {}
    }
    value
}

fn read_annotation_base(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> AssocAnnotationBase {
    if version.r2010_plus() {
        AssocAnnotationBase {
            version: reader.read_bit_short(),
            dependency: handle(reader),
            ..AssocAnnotationBase::default()
        }
    } else {
        AssocAnnotationBase {
            action_body: read_action_body(reader),
            parameter_body: read_parameter_body(reader, version, dxf_version),
            ..AssocAnnotationBase::default()
        }
    }
}

fn read_annotation_action(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
    kind: AssocAnnotationKind,
) -> AssocAnnotationActionBody {
    let mut value = AssocAnnotationActionBody {
        kind,
        ..AssocAnnotationActionBody::default()
    };
    if kind == AssocAnnotationKind::RestoreEntityState {
        value.action_body = read_action_body(reader);
        value.class_version = reader.read_bit_long();
        value.entity = handle(reader);
        return value;
    }
    value.annotation = read_annotation_base(reader, version, dxf_version);
    value.class_version = match kind {
        AssocAnnotationKind::ThreePointAngularDimension | AssocAnnotationKind::RotatedDimension => {
            reader.read_bit_short() as i32
        }
        _ => reader.read_bit_long(),
    };
    match kind {
        AssocAnnotationKind::MLeader => {
            let count = safe_count(reader.read_bit_long(), reader);
            value.actions.reserve(count as usize);
            for _ in 0..count {
                value.actions.push(AssocAnnotationDependency {
                    dependency_id: reader.read_bit_long(),
                    dependency: handle(reader),
                });
            }
        }
        AssocAnnotationKind::AlignedDimension
        | AssocAnnotationKind::OrdinateDimension
        | AssocAnnotationKind::RotatedDimension => {
            value.read_node = handle(reader);
            value.dimension_node = handle(reader);
        }
        AssocAnnotationKind::ThreePointAngularDimension => {
            value.read_node = handle(reader);
            value.dimension_node = handle(reader);
            value.dependency = handle(reader);
        }
        AssocAnnotationKind::RestoreEntityState => {}
    }
    value
}

fn read_single_dependency(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> AssocSingleDependencyActionParam {
    AssocSingleDependencyActionParam {
        action_param: read_action_param(reader, version, dxf_version),
        dependency_class_version: reader.read_bit_long(),
        dependency: handle(reader),
        class_version: reader.read_bit_long(),
    }
}

fn read_compound(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
    has_child_parameter: bool,
) -> AssocCompoundActionParam {
    let action_param = read_action_param(reader, version, dxf_version);
    let class_version = reader.read_bit_short();
    let status = reader.read_bit_short();
    let count = reader.read_bit_long();
    let parameters = read_handles(reader, count);
    let child_parameter = has_child_parameter.then(|| {
        let status = reader.read_bit_short();
        let id = reader.read_bit_long();
        let parameter = handle(reader);
        let (secondary_parameter, marker, tertiary_parameter) = if id != 0 {
            (handle(reader), reader.read_bit_long(), handle(reader))
        } else {
            (Handle::NULL, 0, Handle::NULL)
        };
        AssocChildParameter {
            status,
            id,
            parameter,
            secondary_parameter,
            marker,
            tertiary_parameter,
        }
    });
    AssocCompoundActionParam {
        action_param,
        class_version,
        status,
        parameters,
        child_parameter,
    }
}

fn read_array_action_body(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> AssocArrayActionBody {
    let mut transform = [0.0; 16];
    let action_body = read_action_body(reader);
    let parameter_body = read_parameter_body(reader, version, dxf_version);
    let body_version = reader.read_bit_long();
    let parameter_block = reader.read_variable_text();
    let item_list_version = reader.read_bit_long();
    let count = safe_count(reader.read_bit_long(), reader);
    let item_class = reader.read_variable_text();
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        // Some writers declare more items than the record holds; reading on
        // would turn the following streams into items.
        if reader.main_remaining_bits() <= 0 {
            break;
        }
        items.push(read_array_item(reader));
    }
    for item in &mut transform {
        *item = reader.read_bit_double();
    }
    AssocArrayActionBody {
        action_body,
        parameter_body,
        version: body_version,
        parameter_block,
        item_list_version,
        item_class,
        items,
        transform,
    }
}

fn read_array_item(reader: &mut DwgMergedReader) -> AssocArrayItem {
    let class_version = reader.read_bit_long();
    let location = [
        reader.read_bit_long(),
        reader.read_bit_long(),
        reader.read_bit_long(),
    ];
    let flags = reader.read_bit_long();
    let uses_default_transform = flags & 4 == 0;
    let mut x_direction = crate::types::Vector3::ZERO;
    let mut transform = [0.0; 16];
    if uses_default_transform {
        x_direction = reader.read_3bit_double();
    } else {
        for item in &mut transform {
            *item = reader.read_bit_double();
        }
    }
    let relative_transform = (flags & 2 != 0).then(|| {
        let mut matrix = [0.0; 16];
        for item in &mut matrix {
            *item = reader.read_bit_double();
        }
        matrix
    });
    AssocArrayItem {
        class_version,
        location,
        flags,
        uses_default_transform,
        x_direction,
        transform,
        relative_transform,
        first_handle: Some(handle(reader)),
        second_handle: (flags & 0x10 != 0).then(|| handle(reader)),
    }
}

fn read_array_parameters(reader: &mut DwgMergedReader) -> AssocArrayParameters {
    let version = reader.read_bit_long();
    let count = safe_count(reader.read_bit_long(), reader);
    let class_name = reader.read_variable_text();
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let class_version = reader.read_bit_long();
        let location = [
            reader.read_bit_long(),
            reader.read_bit_long(),
            reader.read_bit_long(),
        ];
        let flags = reader.read_bit_long();
        let uses_default_transform = false;
        let x_direction = crate::types::Vector3::ZERO;
        let mut transform = [0.0; 16];
        for item in &mut transform {
            *item = reader.read_bit_double();
        }
        let relative_transform = if flags & 2 != 0 {
            let mut matrix = [0.0; 16];
            for item in &mut matrix {
                *item = reader.read_bit_double();
            }
            Some(matrix)
        } else {
            None
        };
        let second_handle = (flags & 0x10 != 0).then(|| handle(reader));
        items.push(AssocArrayItem {
            class_version,
            location,
            flags,
            uses_default_transform,
            x_direction,
            transform,
            relative_transform,
            first_handle: None,
            second_handle,
        });
    }
    AssocArrayParameters {
        version,
        class_name,
        items,
        item_count: reader.read_bit_long(),
        row_count: reader.read_bit_long(),
        level_count: reader.read_bit_long(),
    }
}

fn read_dimension_association(reader: &mut DwgMergedReader) -> AssocDimensionAssociation {
    let associativity = reader.read_bit_long();
    let trans_space = reader.read_bit();
    let rotated_type = reader.read_byte();
    let dimension = handle(reader);
    let mut references: [Vec<AssocDimensionReference>; 4] = std::array::from_fn(|_| Vec::new());
    let mut total_references = 0usize;
    for slot in 0..4 {
        if associativity & (1 << slot) == 0 {
            continue;
        }
        loop {
            if total_references >= 6 {
                break;
            }
            let class_name = reader.read_variable_text();
            let osnap_type = reader.read_byte();
            let count = reader.read_bit_long();
            let xrefs = read_handles(reader, count);
            let (main_subent_type, main_gs_marker, xref_paths) =
                if AssocDimensionReference::stores_main_subentity(osnap_type, xrefs.len()) {
                    let main_subent_type = reader.read_bit_long();
                    let main_gs_marker = reader.read_bit_long();
                    let count = safe_count(reader.read_bit_long(), reader);
                    let mut xref_paths = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        xref_paths.push(reader.read_variable_text());
                    }
                    (main_subent_type, main_gs_marker, xref_paths)
                } else {
                    (0, 0, Vec::new())
                };
            let osnap_distance = reader.read_bit_double();
            let osnap_point = reader.read_3bit_double();
            let (
                intersection_objects,
                intersection_subent_type,
                intersection_gs_marker,
                intersection_xref_paths,
            ) = if osnap_type == 6 || osnap_type == 11 {
                let count = reader.read_bit_long();
                let intersection_objects = read_handles(reader, count);
                let intersection_subent_type = reader.read_bit_long();
                let intersection_gs_marker = reader.read_bit_long();
                let count = safe_count(reader.read_bit_long(), reader);
                let mut paths = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    paths.push(reader.read_variable_text());
                }
                (
                    intersection_objects,
                    intersection_subent_type,
                    intersection_gs_marker,
                    paths,
                )
            } else {
                (Vec::new(), 0, 0, Vec::new())
            };
            let has_last_point_reference = reader.read_bit();
            references[slot].push(AssocDimensionReference {
                class_name,
                osnap_type,
                xrefs,
                main_subent_type,
                main_gs_marker,
                xref_paths,
                osnap_distance,
                osnap_point,
                intersection_objects,
                intersection_subent_type,
                intersection_gs_marker,
                intersection_xref_paths,
                has_last_point_reference,
            });
            total_references += 1;
            if !has_last_point_reference {
                break;
            }
        }
    }
    AssocDimensionAssociation {
        associativity,
        trans_space,
        rotated_type,
        dimension,
        references,
    }
}

fn read_static_pers_subent_manager(reader: &mut DwgMergedReader) -> PersSubentManager {
    let class_version = reader.read_bit_long();
    let marker_zero = reader.read_bit_long();
    let marker_two = reader.read_bit_long();
    let associative_step_count = reader.read_bit_long();
    let associative_subent_count = reader.read_bit_long();
    // The body is a flat run of integers up to the end of the data stream.
    let mut values = Vec::new();
    while reader.main_remaining_bits() >= 2 {
        values.push(reader.read_bit_long());
    }
    PersSubentManager {
        class_version,
        marker_zero,
        marker_two,
        associative_step_count,
        associative_subent_count,
        values,
    }
}

fn read_constraint_node_common(
    reader: &mut DwgMergedReader,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> AssocConstraintNode {
    let node_id = reader.read_bit_long();
    let status_before = !version.r2013_plus(dxf_version);
    let mut status = if status_before { reader.read_byte() } else { 0 };
    let connection_count = safe_count(reader.read_bit_long(), reader);
    let mut connections = Vec::with_capacity(connection_count as usize);
    for _ in 0..connection_count {
        connections.push(reader.read_bit_long());
    }
    if !status_before {
        status = reader.read_byte();
    }
    AssocConstraintNode {
        node_id,
        status,
        connections,
        class_name: String::new(),
        registry_flag: false,
        data: AssocConstraintNodeData::None,
    }
}

fn is_plain_geometrical_constraint(class_name: &str) -> bool {
    matches!(
        class_name.to_ascii_uppercase().as_str(),
        "ACCENTERPOINTCONSTRAINT"
            | "ACCOLINEARCONSTRAINT"
            | "ACCONCENTRICCONSTRAINT"
            | "ACEQUALCURVATURECONSTRAINT"
            | "ACEQUALDISTANCECONSTRAINT"
            | "ACEQUALHELPPARAMETERCONSTRAINT"
            | "ACEQUALLENGTHCONSTRAINT"
            | "ACEQUALRADIUSCONSTRAINT"
            | "ACFIXEDCONSTRAINT"
            | "ACMIDPOINTCONSTRAINT"
            | "ACNORMALCONSTRAINT"
            | "ACPERPENDICULARCONSTRAINT"
            | "ACPOINTCOINCIDENCECONSTRAINT"
            | "ACPOINTCURVECONSTRAINT"
            | "ACSYMMETRICCONSTRAINT"
            | "ACTANGENTCONSTRAINT"
    )
}

/// Owner id, implied, active. A version-0 group stores no active flag.
fn read_geometrical_constraint(reader: &mut DwgMergedReader, legacy: bool) -> (i32, bool, bool) {
    (
        reader.read_bit_long(),
        reader.read_bit(),
        legacy || reader.read_bit(),
    )
}

fn read_explicit_constraint(
    reader: &mut DwgMergedReader,
    legacy: bool,
) -> (i32, bool, bool, Handle, Handle) {
    let (owner_id, is_implied, is_active) = read_geometrical_constraint(reader, legacy);
    (
        owner_id,
        is_implied,
        is_active,
        handle(reader),
        handle(reader),
    )
}

/// Node data of `class_name`. Version-0 constraints have no active flag.
/// Circles and arcs omit their reserved double in the R2010 inline layout,
/// including version-2 groups; the later registry layout stores it.
fn read_constraint_node_data(
    reader: &mut DwgMergedReader,
    class_name: &str,
    legacy: bool,
    circle_reserved: bool,
) -> AssocConstraintNodeData {
    match class_name.to_ascii_uppercase().as_str() {
        "ACG2SMOOTHCONSTRAINT" => {
            let (owner_id, is_implied, is_active) = read_geometrical_constraint(reader, legacy);
            let count = safe_count(reader.read_bit_long(), reader);
            let mut owned_constraint_ids = Vec::with_capacity(count as usize);
            for _ in 0..count {
                owned_constraint_ids.push(reader.read_bit_long());
            }
            AssocConstraintNodeData::Composite {
                owner_id,
                is_implied,
                is_active,
                owned_constraint_ids,
            }
        }
        "ACHELPPARAMETER" => AssocConstraintNodeData::HelpParameter {
            value: reader.read_bit_double(),
            reserved: reader.read_bit(),
        },
        "ACCONSTRAINEDCIRCLE" => AssocConstraintNodeData::Circle {
            geometry_dependency: handle(reader),
            geometry_node_id: reader.read_bit_long(),
            center: reader.read_3bit_double(),
            normal: reader.read_3bit_double(),
            direction: reader.read_3bit_double(),
            radius: reader.read_bit_double(),
            start_parameter: reader.read_bit_double(),
            end_parameter: reader.read_bit_double(),
            reserved: if circle_reserved {
                reader.read_bit_double()
            } else {
                0.0
            },
        },
        "ACCONSTRAINEDARC" => AssocConstraintNodeData::Arc {
            geometry_dependency: handle(reader),
            geometry_node_id: reader.read_bit_long(),
            center: reader.read_3bit_double(),
            normal: reader.read_3bit_double(),
            direction: reader.read_3bit_double(),
            radius: reader.read_bit_double(),
            start_parameter: reader.read_bit_double(),
            end_parameter: reader.read_bit_double(),
            reserved: if circle_reserved {
                reader.read_bit_double()
            } else {
                0.0
            },
            start_point: reader.read_3bit_double(),
            end_point: reader.read_3bit_double(),
        },
        "ACCONSTRAINEDIMPLICITPOINT" => {
            let geometry_dependency = handle(reader);
            AssocConstraintNodeData::ImplicitPoint {
                geometry_dependency,
                geometry_node_id: reader.read_bit_long(),
                point: (!geometry_dependency.is_null()).then(|| reader.read_3bit_double()),
                point_type: reader.read_byte(),
                point_index: reader.read_bit_long(),
                curve_id: reader.read_bit_long(),
            }
        }
        "ACCONSTRAINEDPOINT" => {
            let geometry_dependency = handle(reader);
            AssocConstraintNodeData::Point {
                geometry_dependency,
                geometry_node_id: reader.read_bit_long(),
                point: (!geometry_dependency.is_null()).then(|| reader.read_3bit_double()),
            }
        }
        "ACCONSTRAINEDRIGIDSET" => {
            let geometry_dependency = handle(reader);
            let geometry_node_id = reader.read_bit_long();
            let reserved = reader.read_bit();
            let mut transform = [0.0; 16];
            for value in &mut transform {
                *value = reader.read_bit_double();
            }
            let count = safe_count(reader.read_bit_long(), reader);
            let mut geometry_ids = Vec::with_capacity(count as usize);
            for _ in 0..count {
                geometry_ids.push(reader.read_bit_long());
            }
            AssocConstraintNodeData::RigidSet {
                geometry_dependency,
                geometry_node_id,
                reserved,
                transform,
                geometry_ids,
            }
        }
        "ACCONSTRAINEDLINE"
        | "ACCONSTRAINEDCONSTRUCTIONLINE"
        | "ACCONSTRAINED2POINTSCONSTRUCTIONLINE"
        | "ACCONSTRAINEDDATUMLINE" => AssocConstraintNodeData::Line {
            geometry_dependency: handle(reader),
            geometry_node_id: reader.read_bit_long(),
            point: reader.read_3bit_double(),
            direction: reader.read_3bit_double(),
        },
        "ACCONSTRAINEDBOUNDEDLINE" => AssocConstraintNodeData::BoundedLine {
            geometry_dependency: handle(reader),
            geometry_node_id: reader.read_bit_long(),
            point: reader.read_3bit_double(),
            direction: reader.read_3bit_double(),
            is_ray: reader.read_bit(),
            start_point: reader.read_3bit_double(),
            end_point: reader.read_3bit_double(),
        },
        "ACANGLECONSTRAINT" | "AC3POINTANGLECONSTRAINT" => {
            let (owner_id, is_implied, is_active, value_dependency, dimension_dependency) =
                read_explicit_constraint(reader, legacy);
            AssocConstraintNodeData::Angle {
                owner_id,
                is_implied,
                is_active,
                value_dependency,
                dimension_dependency,
                sector_type: reader.read_byte(),
            }
        }
        "ACPARALLELCONSTRAINT" | "ACHORIZONTALCONSTRAINT" | "ACVERTICALCONSTRAINT" => {
            let (owner_id, is_implied, is_active) = read_geometrical_constraint(reader, legacy);
            AssocConstraintNodeData::Parallel {
                owner_id,
                is_implied,
                is_active,
                datum_line_index: (!class_name.eq_ignore_ascii_case("AcParallelConstraint"))
                    .then(|| reader.read_bit_long()),
            }
        }
        "ACDISTANCECONSTRAINT" => {
            let (owner_id, is_implied, is_active, value_dependency, dimension_dependency) =
                read_explicit_constraint(reader, legacy);
            let direction_type = reader.read_byte();
            AssocConstraintNodeData::Distance {
                owner_id,
                is_implied,
                is_active,
                value_dependency,
                dimension_dependency,
                direction_type,
                distance: (direction_type != 0).then(|| reader.read_3bit_double()),
            }
        }
        "ACRADIUSDIAMETERCONSTRAINT" => {
            let (owner_id, is_implied, is_active, value_dependency, dimension_dependency) =
                read_explicit_constraint(reader, legacy);
            AssocConstraintNodeData::RadiusDiameter {
                owner_id,
                is_implied,
                is_active,
                value_dependency,
                dimension_dependency,
                mode: reader.read_byte(),
            }
        }
        "ACCONSTRAINEDELLIPSE" => AssocConstraintNodeData::Ellipse {
            geometry_dependency: handle(reader),
            geometry_node_id: reader.read_bit_long(),
            center: reader.read_3bit_double(),
            major_axis: reader.read_3bit_double(),
            axis_ratio: reader.read_bit_double(),
        },
        "ACCONSTRAINEDBOUNDEDELLIPSE" => AssocConstraintNodeData::BoundedEllipse {
            geometry_dependency: handle(reader),
            geometry_node_id: reader.read_bit_long(),
            center: reader.read_3bit_double(),
            major_axis: reader.read_3bit_double(),
            axis_ratio: reader.read_bit_double(),
            start_point: reader.read_3bit_double(),
            end_point: reader.read_3bit_double(),
        },
        "ACCONSTRAINEDSPLINE" => {
            let geometry_dependency = handle(reader);
            let geometry_node_id = reader.read_bit_long();
            let rational = reader.read_bit();
            let periodic = reader.read_bit();
            let degree = reader.read_bit_long();
            let knot_tolerance = reader.read_bit_double();
            let knot_count = safe_count(reader.read_bit_long(), reader);
            let knot_physical_length = reader.read_bit_long();
            let knot_grow_length = reader.read_bit_long();
            let mut knots = Vec::with_capacity(knot_count as usize);
            for _ in 0..knot_count {
                knots.push(reader.read_bit_double());
            }
            let weight_count = safe_count(reader.read_bit_long(), reader);
            let weight_physical_length = reader.read_bit_long();
            let weight_grow_length = reader.read_bit_long();
            let mut weights = Vec::with_capacity(weight_count as usize);
            for _ in 0..weight_count {
                weights.push(reader.read_bit_double());
            }
            let control_point_count = safe_count(reader.read_bit_long(), reader);
            let control_point_physical_length = reader.read_bit_long();
            let control_point_grow_length = reader.read_bit_long();
            let mut control_points = Vec::with_capacity(control_point_count as usize);
            for _ in 0..control_point_count {
                control_points.push(reader.read_3bit_double());
            }
            let implicit_point_count = safe_count(reader.read_bit_long(), reader);
            let mut implicit_point_ids = Vec::with_capacity(implicit_point_count as usize);
            for _ in 0..implicit_point_count {
                implicit_point_ids.push(reader.read_bit_long());
            }
            AssocConstraintNodeData::Spline {
                geometry_dependency,
                geometry_node_id,
                rational,
                periodic,
                degree,
                knot_tolerance,
                knot_physical_length,
                knot_grow_length,
                knots,
                weight_physical_length,
                weight_grow_length,
                weights,
                control_point_physical_length,
                control_point_grow_length,
                control_points,
                implicit_point_ids,
            }
        }
        _ if is_plain_geometrical_constraint(class_name) => {
            let (owner_id, is_implied, is_active) = read_geometrical_constraint(reader, legacy);
            AssocConstraintNodeData::Geometrical {
                owner_id,
                is_implied,
                is_active,
            }
        }
        _ => AssocConstraintNodeData::None,
    }
}

pub fn read_associative_data(
    reader: &mut DwgMergedReader,
    dxf_name: &str,
    version: DwgVersion,
    dxf_version: DxfVersion,
) -> Option<AssociativeData> {
    let name = associative_canonical_name(dxf_name);
    let value = match name.as_str() {
        "ASSOCDEPENDENCY" => AssociativeData::Dependency(read_dependency(reader)),
        "ASSOCVALUEDEPENDENCY" => AssociativeData::ValueDependency(AssocValueDependency {
            dependency: read_dependency(reader),
            class_version: reader.read_bit_long(),
            name: reader.read_variable_text(),
            value: read_eval_variant(reader),
        }),
        "ASSOCGEOMDEPENDENCY" => {
            let dependency = read_dependency(reader);
            AssociativeData::GeomDependency(AssocGeomDependency {
                dependency,
                class_version: reader.read_bit_short(),
                enabled: reader.read_bit(),
                persistent_subent: read_persistent_subent(reader, version, dxf_version),
            })
        }
        "ASSOCACTION" => AssociativeData::Action(read_action(reader)),
        "ASSOCNETWORK" => {
            let action = read_action(reader);
            let network_version = reader.read_bit_short();
            let network_action_index = reader.read_bit_long();
            let count = safe_count(reader.read_bit_long(), reader);
            let mut actions = Vec::with_capacity(count as usize);
            for _ in 0..count {
                actions.push(AssocActionDependency {
                    is_owned: reader.read_bit(),
                    dependency: handle(reader),
                });
            }
            let count = reader.read_bit_long();
            let owned_actions = read_handles(reader, count);
            AssociativeData::Network(AssocNetwork {
                action,
                network_version,
                network_action_index,
                actions,
                owned_actions,
            })
        }
        name if surface_kind(name).is_some() => AssociativeData::SurfaceActionBody(
            read_surface_action(reader, version, dxf_version, surface_kind(name).unwrap()),
        ),
        "ASSOCRESTOREENTITYSTATEACTIONBODY" => {
            AssociativeData::AnnotationActionBody(read_annotation_action(
                reader,
                version,
                dxf_version,
                AssocAnnotationKind::RestoreEntityState,
            ))
        }
        "ASSOCMLEADERACTIONBODY" => AssociativeData::AnnotationActionBody(read_annotation_action(
            reader,
            version,
            dxf_version,
            AssocAnnotationKind::MLeader,
        )),
        "ASSOCALIGNEDDIMACTIONBODY" => {
            AssociativeData::AnnotationActionBody(read_annotation_action(
                reader,
                version,
                dxf_version,
                AssocAnnotationKind::AlignedDimension,
            ))
        }
        "ASSOC3POINTANGULARDIMACTIONBODY" => {
            AssociativeData::AnnotationActionBody(read_annotation_action(
                reader,
                version,
                dxf_version,
                AssocAnnotationKind::ThreePointAngularDimension,
            ))
        }
        "ASSOCORDINATEDIMACTIONBODY" => {
            AssociativeData::AnnotationActionBody(read_annotation_action(
                reader,
                version,
                dxf_version,
                AssocAnnotationKind::OrdinateDimension,
            ))
        }
        "ASSOCROTATEDDIMACTIONBODY" => {
            AssociativeData::AnnotationActionBody(read_annotation_action(
                reader,
                version,
                dxf_version,
                AssocAnnotationKind::RotatedDimension,
            ))
        }
        "ASSOCPERSSUBENTMANAGER" => {
            let class_version = reader.read_bit_long();
            let markers = [
                reader.read_bit_long(),
                reader.read_bit_long(),
                reader.read_bit_long(),
            ];
            // The integers run up to the closing flag; their count is not
            // stored, so the record end bounds them.
            let mut values = Vec::new();
            while reader.main_remaining_bits() > 1 {
                values.push(reader.read_bit_long());
            }
            AssociativeData::PersSubentManager(AssocPersSubentManager {
                class_version,
                markers,
                values,
                final_flag: reader.read_bit(),
            })
        }
        "ASSOCEDGEACTIONPARAM" => {
            let single_dependency = read_single_dependency(reader, version, dxf_version);
            let parameter = handle(reader);
            let has_action = reader.read_bit();
            // A detached edge stores no cached curve after the flag.
            let action_type = if has_action { reader.read_bit_long() } else { 0 };
            let subcurve_kind = match action_type {
                11 => AssocSubcurveKind::Arc,
                17 => AssocSubcurveKind::Ellipse,
                19 => AssocSubcurveKind::Line,
                23 => AssocSubcurveKind::LineSegment3d,
                42 => AssocSubcurveKind::Nurb3d,
                27 => AssocSubcurveKind::Curve3d,
                _ => AssocSubcurveKind::None,
            };
            let mut curve = Vec::new();
            if has_action {
                read_edge_curve(reader, action_type, &mut curve, 0);
            }
            AssociativeData::EdgeActionParam(AssocEdgeActionParam {
                single_dependency,
                parameter,
                has_action,
                action_type,
                subcurve_kind,
                curve,
            })
        }
        "ASSOC2DCONSTRAINTGROUP" => {
            let action = read_action(reader);
            let group_version = reader.read_bit_long();
            let flag = reader.read_bit();
            let work_plane = [
                reader.read_3bit_double(),
                reader.read_3bit_double(),
                reader.read_3bit_double(),
            ];
            let dependency = handle(reader);
            let count = reader.read_bit_long();
            let actions = read_handles(reader, count);
            let next_node_id = reader.read_bit_long();
            // This is an ID seed, not an array length. Deleted nodes can
            // leave it much larger than the remaining record or node count.
            if next_node_id < 0 {
                reader.invalidate(format!(
                    "Negative DWG constraint-group next node id {next_node_id}"
                ));
            }
            let mut nodes = Vec::new();
            // Version 0 (the 2010 release) lists its nodes inline after a
            // count: class name, id, connections and data each, with no root,
            // class table, registry or status byte. They load into the
            // current layout and are written back as version 2.
            let legacy = group_version == 0;
            // R2010 version-2 groups also have an inline count/class/node
            // sequence, with status bytes and active flags. Older releases
            // of this codec emitted the later registry layout with root ID
            // zero even for R2010; retain that read compatibility.
            let inline_nodes = legacy
                || (!version.r2013_plus(dxf_version) && reader.main_remaining_bits() >= 2 && {
                    let position = reader.position_in_bits();
                    let remaining = reader.main_remaining_bits();
                    let count = reader.read_bit_long();
                    reader.set_position_in_bits(position);
                    // An empty inline list is only BL(0); a registry needs
                    // at least 11 bits for root, connections, flag and counts.
                    count > 0 || (count == 0 && remaining < 11)
                });
            if inline_nodes {
                let count = safe_count(reader.read_bit_long(), reader);
                nodes.reserve(count as usize + 1);
                if count > 0 {
                    nodes.push(AssocConstraintNode::default());
                }
                for _ in 0..count {
                    let class_name = reader.read_variable_text();
                    let mut node = if legacy {
                        let node_id = reader.read_bit_long();
                        let count = safe_count(reader.read_bit_long(), reader);
                        let connections = (0..count).map(|_| reader.read_bit_long()).collect();
                        AssocConstraintNode {
                            node_id,
                            connections,
                            ..Default::default()
                        }
                    } else {
                        read_constraint_node_common(reader, version, dxf_version)
                    };
                    let data = read_constraint_node_data(reader, &class_name, legacy, false);
                    let unknown = matches!(data, AssocConstraintNodeData::None);
                    if unknown {
                        reader.invalidate(format!(
                            "Unsupported DWG constraint node class {class_name}"
                        ));
                    }
                    node.class_name = class_name;
                    node.data = data;
                    nodes.push(node);
                    // An unknown class's data length is unknown; what
                    // follows it cannot be found.
                    if unknown {
                        break;
                    }
                }
            } else if next_node_id > 0 {
                let root_id = reader.read_bit_long();
                let connection_count = safe_count(reader.read_bit_long(), reader);
                let mut connections = Vec::with_capacity(connection_count as usize);
                for _ in 0..connection_count {
                    connections.push(reader.read_bit_long());
                }
                nodes.push(AssocConstraintNode {
                    node_id: root_id,
                    status: u8::from(reader.read_bit()),
                    connections,
                    class_name: String::new(),
                    registry_flag: false,
                    data: AssocConstraintNodeData::None,
                });
                let class_type_count = safe_count(reader.read_bit_long(), reader);
                let mut class_types = Vec::with_capacity(class_type_count as usize);
                for _ in 0..class_type_count {
                    class_types.push(reader.read_variable_text());
                }
                let registered_count = safe_count(reader.read_bit_long(), reader);
                let mut registry = Vec::with_capacity(registered_count as usize);
                for _ in 0..registered_count {
                    let registry_flag = reader.read_bit();
                    let class_index = reader.read_bit_long();
                    let node_id = reader.read_bit_long();
                    let class_name = class_index
                        .checked_sub(1)
                        .and_then(|index| class_types.get(index as usize))
                        .cloned()
                        .unwrap_or_default();
                    registry.push((class_name, node_id, registry_flag));
                }
                for (class_name, registered_node_id, registry_flag) in registry {
                    let mut node = read_constraint_node_common(reader, version, dxf_version);
                    if node.node_id == 0 {
                        node.node_id = registered_node_id;
                    }
                    node.data = read_constraint_node_data(reader, &class_name, false, true);
                    let unknown = matches!(node.data, AssocConstraintNodeData::None);
                    if unknown {
                        reader.invalidate(format!(
                            "Unsupported DWG constraint node class {class_name}"
                        ));
                    }
                    node.class_name = class_name;
                    node.registry_flag = registry_flag;
                    nodes.push(node);
                    if unknown {
                        break;
                    }
                }
            }
            AssociativeData::ConstraintGroup(Assoc2dConstraintGroup {
                action,
                version: if legacy { 2 } else { group_version },
                flag,
                work_plane,
                dependency,
                actions,
                next_node_id,
                nodes,
            })
        }
        "ASSOCVARIABLE" => {
            let action = read_action(reader);
            let class_version = reader.read_bit_long();
            let name = reader.read_variable_text();
            let expression = reader.read_variable_text();
            let evaluator = reader.read_variable_text();
            let description = reader.read_variable_text();
            let value = read_eval_variant(reader);
            let has_cached_value = reader.read_bit();
            let cached_value = if has_cached_value {
                reader.read_variable_text()
            } else {
                String::new()
            };
            let flag = reader.read_bit();
            let count = if reader.main_remaining_bits() >= 2 {
                safe_count(reader.read_bit_long(), reader)
            } else {
                0
            };
            let mut dependencies = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let dependency = handle(reader);
                dependencies.push(AssocVariableDependency {
                    dependency,
                    flags: reader.read_bit_long(),
                });
            }
            AssociativeData::Variable(AssocVariable {
                action,
                class_version,
                name,
                expression,
                evaluator,
                description,
                value,
                has_cached_value,
                cached_value,
                flag,
                dependencies,
            })
        }
        "ASSOCACTIONPARAM" => {
            AssociativeData::ActionParam(read_action_param(reader, version, dxf_version))
        }
        "ASSOCCOMPOUNDACTIONPARAM" => {
            AssociativeData::CompoundActionParam(read_compound(reader, version, dxf_version, false))
        }
        "ASSOCOSNAPPOINTREFACTIONPARAM" => {
            let compound = read_compound(reader, version, dxf_version, true);
            AssociativeData::OsnapPointRefActionParam(AssocOsnapPointRefActionParam {
                compound,
                status: reader.read_bit_short(),
                osnap_mode: reader.read_byte(),
                parameter: reader.read_bit_double(),
            })
        }
        "ASSOCPOINTREFACTIONPARAM" => {
            AssociativeData::PointRefActionParam(read_compound(reader, version, dxf_version, true))
        }
        "ASSOCOBJECTACTIONPARAM" => {
            AssociativeData::ObjectActionParam(read_single_dependency(reader, version, dxf_version))
        }
        "ASSOCPATHACTIONPARAM" => {
            let compound = read_compound(reader, version, dxf_version, false);
            AssociativeData::PathActionParam(AssocPathActionParam {
                compound,
                version: reader.read_bit_long(),
            })
        }
        "ASSOCDIMDEPENDENCYBODY" => AssociativeData::DimDependencyBody(AssocDimDependencyBody {
            dependency_body_version: reader.read_bit_short(),
            base_version: reader.read_bit_short(),
            name: reader.read_variable_text(),
            class_version: reader.read_bit_short(),
        }),
        "ASSOCFACEACTIONPARAM" => {
            let single_dependency = read_single_dependency(reader, version, dxf_version);
            AssociativeData::FaceActionParam(AssocFaceActionParam {
                single_dependency,
                index: reader.read_bit_long(),
            })
        }
        "ASSOCVERTEXACTIONPARAM" => {
            let single_dependency = read_single_dependency(reader, version, dxf_version);
            AssociativeData::VertexActionParam(AssocVertexActionParam {
                single_dependency,
                point: reader.read_3bit_double(),
            })
        }
        "ASSOCASMBODYACTIONPARAM" => {
            let single_dependency = read_single_dependency(reader, version, dxf_version);
            let data = super::entities::read_acis_entity(reader, version, dxf_version, false);
            let history = if data.version > 1 {
                handle(reader)
            } else {
                Handle::NULL
            };
            let mut acis_data = crate::entities::AcisData::new();
            acis_data.version = if data.is_binary {
                AcisVersion::Version2
            } else {
                AcisVersion::Version1
            };
            acis_data.sat_data = data.sat_data;
            acis_data.sab_data = data.sab_data;
            acis_data.is_binary = data.is_binary;
            acis_data.revision = data.revision;
            acis_data.materials = data.materials;
            acis_data.wireframe_data_present = data.wireframe_data_present;
            acis_data.wireframe_point_present = data.wireframe_point_present;
            acis_data.wireframe_isoline_present = data.wireframe_isoline_present;
            acis_data.acis_empty_bit = data.acis_empty_bit;
            acis_data.extra_acis_data = data.extra_acis_data.map(Box::new);
            acis_data.wireframe_isolines = data.isolines;
            AssociativeData::AsmBodyActionParam(AssocAsmBodyActionParam {
                single_dependency,
                acis_data,
                point_of_reference: data.point,
                wires: data.wires,
                silhouettes: data.silhouettes,
                history,
            })
        }
        "ASSOCARRAYMODIFYPARAMETERS"
        | "ASSOCARRAYPATHPARAMETERS"
        | "ASSOCARRAYPOLARPARAMETERS"
        | "ASSOCARRAYRECTANGULARPARAMETERS" => {
            AssociativeData::ArrayParameters(read_array_parameters(reader))
        }
        "ASSOCARRAYACTIONBODY" => {
            AssociativeData::ArrayActionBody(read_array_action_body(reader, version, dxf_version))
        }
        "ASSOCARRAYMODIFYACTIONBODY" => {
            let body = read_array_action_body(reader, version, dxf_version);
            let status = reader.read_bit_short();
            let count = safe_count(reader.read_bit_long(), reader);
            let mut item_locations = Vec::with_capacity(count as usize);
            for _ in 0..count {
                item_locations.push([
                    reader.read_bit_long(),
                    reader.read_bit_long(),
                    reader.read_bit_long(),
                ]);
            }
            AssociativeData::ArrayModifyActionBody(AssocArrayModifyActionBody {
                body,
                status,
                item_locations,
            })
        }
        "DIMASSOC" => AssociativeData::DimensionAssociation(read_dimension_association(reader)),
        "PERSUBENTMGR" => {
            AssociativeData::PersSubentManagerStatic(read_static_pers_subent_manager(reader))
        }
        "ACDBCENTERMARKACTIONBODY" | "ACDBCENTERLINEACTIONBODY" => {
            AssociativeData::SmartCenterActionBody(AssocSmartCenterActionBody {
                action_body: read_action_body(reader),
                parameter_body: read_parameter_body(reader, version, dxf_version),
                version: reader.read_bit_long(),
            })
        }
        "ASSOCVIEWREPACTIONBODY" => AssociativeData::ViewRepActionBody(AssocViewRepActionBody {
            action_body: read_action_body(reader),
            class_version: reader.read_bit_short(),
            view_rep: handle(reader),
            view_type: reader.read_bit_long(),
            rotation: reader.read_bit_double(),
        }),
        "ASSOCVIEWBORDERACTIONPARAM"
        | "ASSOCVIEWREPACTIONPARAM"
        | "ASSOCVIEWSYMBOLACTIONPARAM"
        | "ASSOCVIEWSTYLEACTIONPARAM" => {
            let kind = match name.as_str() {
                "ASSOCVIEWREPACTIONPARAM" => AssocViewObjectActionParamKind::ViewRep,
                "ASSOCVIEWSYMBOLACTIONPARAM" => AssocViewObjectActionParamKind::ViewSymbol,
                "ASSOCVIEWSTYLEACTIONPARAM" => AssocViewObjectActionParamKind::ViewStyle,
                _ => AssocViewObjectActionParamKind::ViewBorder,
            };
            AssociativeData::ViewObjectActionParam(AssocViewObjectActionParam {
                kind,
                single_dependency: read_single_dependency(reader, version, dxf_version),
                class_version: reader.read_bit_short(),
            })
        }
        "ASSOCVIEWREPHATCHMANAGER" => {
            let compound = read_compound(reader, version, dxf_version, false);
            let class_version = reader.read_bit_short();
            let count = safe_count(reader.read_bit_long(), reader);
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                items.push(AssocViewRepHatchManagerItem {
                    first_id: reader.read_bit_long_long(),
                    second_id: reader.read_bit_long_long(),
                    status: reader.read_bit_long(),
                    parameter: handle(reader),
                });
            }
            AssociativeData::ViewRepHatchManager(AssocViewRepHatchManager {
                compound,
                class_version,
                items,
            })
        }
        "ASSOCVIEWREPHATCHACTIONPARAM" => {
            AssociativeData::ViewRepHatchActionParam(AssocViewRepHatchActionParam {
                single_dependency: read_single_dependency(reader, version, dxf_version),
                class_version: reader.read_bit_short(),
                normal: reader.read_3bit_double(),
                hatch_index: reader.read_bit_long(),
                flags: reader.read_bit_long(),
            })
        }
        "ASSOCVIEWLABELACTIONPARAM" => {
            AssociativeData::ViewLabelActionParam(AssocViewLabelActionParam {
                single_dependency: read_single_dependency(reader, version, dxf_version),
                class_version: reader.read_bit_short(),
                label_version: reader.read_bit_short(),
                offset: reader.read_2raw_double(),
                flag: reader.read_byte(),
            })
        }
        _ => return None,
    };
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::read_surface_action;
    use crate::io::dwg::dwg_reference_type::DwgReferenceType::HardPointer;
    use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
    use crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter;
    use crate::io::dwg::dwg_version::DwgVersion;
    use crate::objects::AssocSurfaceActionKind;
    use crate::types::DxfVersion;

    #[test]
    fn r2004_plane_surface_action_ends_after_the_parameter_body() {
        let dwg = DwgVersion::AC18;
        let dxf = DxfVersion::AC1018;
        let mut writer = DwgMergedWriter::new(dwg, dxf);
        writer.write_bit_long(1); // action body version
        writer.write_bit_long(0); // parameter version
        writer.write_bit_long(0); // minor
        writer.write_bit_long(1); // one dependency
        writer.write_handle(HardPointer, 0x50E);
        writer.write_bit_long(0); // marker
        writer.write_bit_long(0); // no values
        writer.write_bit_long(0); // empty-value marker
        writer.write_handle(HardPointer, 0x50D);
        let mut reader = DwgMergedReader::new(writer.merge(), dxf, writer.handle_start_bits());
        let action = read_surface_action(&mut reader, dwg, dxf, AssocSurfaceActionKind::Plane);
        reader.check_error().expect("plane action");
        assert_eq!(action.action_body.version, 1);
        assert_eq!(action.parameter_body.dependencies.len(), 1);
        assert_eq!(action.parameter_body.dependencies[0].value(), 0x50E);
        assert_eq!(action.parameter_body.dependency.value(), 0x50D);
        assert_eq!(action.surface_body.dependency.value(), 0);
        assert_eq!(action.class_version, 0);
    }
}
