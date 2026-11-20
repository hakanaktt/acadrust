//! Less-common but fully structured DWG entity classes.
//!
//! Keeping these class-based entities behind one wrapper avoids treating
//! supported records as opaque proxy payloads while keeping the main entity
//! enum compact.

use crate::entities::{Entity, EntityCommon, MText};
use crate::types::{BoundingBox3D, Color, Handle, Matrix3, Matrix4, Transform, Vector2, Vector3};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExtendedEntity {
    pub common: EntityCommon,
    pub data: ExtendedEntityData,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExtendedEntityData {
    Camera {
        view_handle: Handle,
    },
    SectionObject(SectionObjectData),
    ArcAlignedText(ArcAlignedTextData),
    RemoteText(RemoteTextData),
    GeoPositionMarker(GeoPositionMarkerData),
    CoordinationModel(CoordinationModelData),
    PointCloud(PointCloudData),
    PointCloudEx(PointCloudExData),
    Proxy(ProxyEntityData),
    OleFrame(OleFrameData),
    LayoutPrintConfig(LayoutPrintConfigData),
    Format(FormatData),
    Legacy(LegacyEntityData),
    DynamicBlock(crate::objects::DynamicBlockData),
    RegisteredClass(RegisteredClassEntityData),
    /// ACAD Arch entity (wall, door, stair, space, and the other
    /// AEC entities). The class record stays intact; profiles can be moved
    /// and reshaped.
    Arch(crate::classes::arch::ArchEntityData),
}

impl ExtendedEntityData {
    pub(crate) fn preserve_storage_data_from(&mut self, source: &Self) {
        match (self, source) {
            (Self::LayoutPrintConfig(value), Self::LayoutPrintConfig(source)) => {
                value.raw_dwg_data = source.raw_dwg_data.clone();
                value.raw_dwg_version = source.raw_dwg_version;
            }
            (Self::Format(value), Self::Format(source)) => {
                value.raw_dwg_data = source.raw_dwg_data.clone();
                value.raw_dwg_version = source.raw_dwg_version;
                value.raw_dxf_codes = source.raw_dxf_codes.clone();
            }
            (Self::Arch(value), Self::Arch(source)) => {
                value.raw_dwg_data = source.raw_dwg_data.clone();
                value.raw_dxf_codes = source.raw_dxf_codes.clone();
                value.dwg_source_version = source.dwg_source_version;
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SectionObjectData {
    pub state: i32,
    pub flags: i32,
    pub name: String,
    pub vertical_direction: Vector3,
    pub top_height: f64,
    pub bottom_height: f64,
    pub indicator_alpha: i16,
    pub indicator_color: Color,
    pub vertices: Vec<Vector3>,
    pub back_line_vertices: Vec<Vector3>,
    pub settings_handle: Handle,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArcAlignedTextData {
    pub text: String,
    pub font_name: String,
    pub big_font_name: String,
    pub style_name: String,
    pub center: Vector3,
    pub radius: f64,
    pub x_scale: f64,
    pub text_size: f64,
    pub character_spacing: f64,
    pub offset_from_arc: f64,
    pub right_offset: f64,
    pub left_offset: f64,
    pub start_angle: f64,
    pub end_angle: f64,
    pub reverse: bool,
    pub text_direction: i16,
    pub alignment: i16,
    pub text_position: i16,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub character_set: i16,
    pub pitch_and_family: i16,
    pub is_shx: bool,
    pub text_color: i32,
    pub normal: Vector3,
    pub wizard_flag: bool,
    pub arc_handle: Handle,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RemoteTextData {
    pub position: Vector3,
    pub normal: Vector3,
    pub rotation: f64,
    pub height: f64,
    pub style_handle: Handle,
    pub style_name: String,
    pub flags: i16,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeoPositionMarkerData {
    pub class_version: i32,
    pub position: Vector3,
    pub radius: f64,
    pub notes: String,
    pub landing_gap: f64,
    pub mtext_visible: bool,
    pub text_alignment: u8,
    pub enable_frame_text: bool,
    pub embedded_mtext: Option<MText>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CoordinationModelData {
    pub flags: i16,
    pub definition_handle: Handle,
    pub transform: [f64; 16],
    pub unit_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointCloudClip {
    pub inverted: bool,
    pub clip_type: i16,
    pub vertices: Vec<Vector2>,
    pub z_min: f64,
    pub z_max: f64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointCloudData {
    pub class_version: i16,
    pub origin: Vector3,
    pub saved_filename: String,
    pub source_files: Vec<String>,
    pub extents_min: Vector3,
    pub extents_max: Vector3,
    pub point_count: i64,
    pub ucs_name: String,
    pub ucs_origin: Vector3,
    pub ucs_x_direction: Vector3,
    pub ucs_y_direction: Vector3,
    pub ucs_z_direction: Vector3,
    pub definition_handle: Handle,
    pub reactor_handle: Handle,
    pub show_intensity: bool,
    pub intensity_scheme: i16,
    pub minimum_intensity: f64,
    pub maximum_intensity: f64,
    pub low_intensity_threshold: f64,
    pub high_intensity_threshold: f64,
    pub show_clipping: bool,
    pub clippings: Vec<PointCloudClip>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointCloudExCrop {
    pub crop_type: i16,
    pub inside: bool,
    pub inverted: bool,
    pub plane: Vector3,
    pub x_direction: Vector3,
    pub y_direction: Vector3,
    pub points: Vec<Vector3>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointCloudExData {
    pub class_version: i16,
    pub extents_min: Vector3,
    pub extents_max: Vector3,
    pub ucs_origin: Vector3,
    pub ucs_x_direction: Vector3,
    pub ucs_y_direction: Vector3,
    pub ucs_z_direction: Vector3,
    pub locked: bool,
    pub definition_handle: Handle,
    pub reactor_handle: Handle,
    pub name: String,
    pub show_intensity: bool,
    pub show_cropping: bool,
    pub hidden_scans: Vec<String>,
    pub hidden_regions: Vec<i32>,
    pub stylization_type: i16,
    pub intensity_color_scheme: String,
    pub current_color_scheme: String,
    pub classification_color_scheme: String,
    pub elevation_min: f64,
    pub elevation_max: f64,
    pub intensity_min: i32,
    pub intensity_max: i32,
    pub intensity_out_of_range_behavior: i16,
    pub elevation_out_of_range_behavior: i16,
    pub elevation_apply_to_fixed_range: bool,
    pub intensity_as_gradient: bool,
    pub elevation_as_gradient: bool,
    pub croppings: Vec<PointCloudExCrop>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProxyEntityData {
    pub proxy_id: i32,
    pub class_id: i32,
    /// R2004+ binary text-stream name of the proxied DXF class.
    pub dxf_subclass: String,
    pub version: i32,
    pub dwg_version: i32,
    pub maintenance_version: i32,
    pub from_dxf: bool,
    pub graphics: crate::objects::ProxyPayload,
    pub payload: crate::objects::ProxyPayload,
    /// Opaque custom strings following `dxf_subclass` in the R2007+ text
    /// stream. Kept separate because proxy payloads may address both streams.
    #[cfg_attr(feature = "serde", serde(default))]
    pub text_payload: crate::objects::ProxyPayload,
    pub object_ids: Vec<crate::objects::ProxyObjectReference>,
    /// The entity's own DXF groups, as a DXF file stores them after `70 1`
    /// (original data format DXF): `100 McDbEntity2`, `10 ...`, ... up to
    /// the next record. Written back as they are. Empty for binary data.
    #[cfg_attr(feature = "serde", serde(default))]
    pub dxf_data: Vec<(i32, String)>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OleFrameData {
    pub flag: i16,
    pub mode: i16,
    pub storage: crate::compound_file::StructuredStoragePayload,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutPrintConfigData {
    pub class_version: i16,
    pub flag: i16,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_dwg_data: Option<Vec<u8>>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub raw_dwg_handle_bits: i64,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_dwg_version: Option<crate::types::DxfVersion>,
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormatData {
    /// Opaque SPDSGraphiCS payload. The vendor schema is not public.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_dwg_data: Option<Vec<u8>>,
    pub raw_dwg_handle_bits: i64,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_dwg_version: Option<crate::types::DxfVersion>,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_dxf_codes: Option<Vec<(i32, String)>>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LegacyEntityData {
    Repeat,
    EndRepeat {
        columns: i16,
        rows: i16,
        column_spacing: f64,
        row_spacing: f64,
    },
    Load {
        filename: String,
    },
    Jump {
        address: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegisteredClassEntityData {
    pub dxf_name: String,
    pub cpp_class_name: String,
    pub properties: Vec<crate::objects::SemanticProperty>,
    pub payload: crate::objects::ProxyPayload,
    pub object_ids: Vec<crate::objects::ProxyObjectReference>,
}

impl ExtendedEntity {
    pub fn class_name(&self) -> &str {
        match &self.data {
            ExtendedEntityData::Camera { .. } => "CAMERA",
            ExtendedEntityData::SectionObject(_) => "SECTIONOBJECT",
            ExtendedEntityData::ArcAlignedText(_) => "ARCALIGNEDTEXT",
            ExtendedEntityData::RemoteText(_) => "RTEXT",
            ExtendedEntityData::GeoPositionMarker(_) => "POSITIONMARKER",
            ExtendedEntityData::CoordinationModel(_) => "COORDINATION_MODEL",
            ExtendedEntityData::PointCloud(_) => "ACDBPOINTCLOUD",
            ExtendedEntityData::PointCloudEx(_) => "ACDBPOINTCLOUDEX",
            ExtendedEntityData::Proxy(_) => "ACAD_PROXY_ENTITY",
            ExtendedEntityData::OleFrame(_) => "OLEFRAME",
            ExtendedEntityData::LayoutPrintConfig(_) => "LAYOUTPRINTCONFIG",
            ExtendedEntityData::Format(_) => "Format",
            ExtendedEntityData::Legacy(LegacyEntityData::Repeat) => "REPEAT",
            ExtendedEntityData::Legacy(LegacyEntityData::EndRepeat { .. }) => "ENDREP",
            ExtendedEntityData::Legacy(LegacyEntityData::Load { .. }) => "LOAD",
            ExtendedEntityData::Legacy(LegacyEntityData::Jump { .. }) => "JUMP",
            ExtendedEntityData::DynamicBlock(data) => {
                data.entity_dxf_name().unwrap_or("DYNAMICBLOCKENTITY")
            }
            ExtendedEntityData::RegisteredClass(data) => &data.dxf_name,
            ExtendedEntityData::Arch(data) => data.dxf_name.as_str(),
        }
    }
}

impl Entity for ExtendedEntity {
    fn common(&self) -> &super::EntityCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::EntityCommon {
        &mut self.common
    }

    fn bounding_box(&self) -> BoundingBox3D {
        let points: Vec<Vector3> = match &self.data {
            ExtendedEntityData::SectionObject(data) => data
                .vertices
                .iter()
                .chain(data.back_line_vertices.iter())
                .copied()
                .collect(),
            ExtendedEntityData::ArcAlignedText(data) => vec![data.center],
            ExtendedEntityData::RemoteText(data) => vec![data.position],
            ExtendedEntityData::GeoPositionMarker(data) => vec![data.position],
            ExtendedEntityData::PointCloud(data) => {
                vec![data.extents_min, data.extents_max]
            }
            ExtendedEntityData::PointCloudEx(data) => point_cloud_ex_corners(data).to_vec(),
            ExtendedEntityData::Arch(data) => data.profiles.iter().flatten().copied().collect(),
            _ => Vec::new(),
        };
        BoundingBox3D::from_points(&points).unwrap_or_default()
    }

    fn translate(&mut self, offset: Vector3) {
        match &mut self.data {
            ExtendedEntityData::SectionObject(data) => {
                for point in &mut data.vertices {
                    *point = *point + offset;
                }
                for point in &mut data.back_line_vertices {
                    *point = *point + offset;
                }
            }
            ExtendedEntityData::ArcAlignedText(data) => data.center = data.center + offset,
            ExtendedEntityData::RemoteText(data) => data.position = data.position + offset,
            ExtendedEntityData::GeoPositionMarker(data) => data.position = data.position + offset,
            ExtendedEntityData::PointCloud(data) => {
                data.origin = data.origin + offset;
                data.extents_min = data.extents_min + offset;
                data.extents_max = data.extents_max + offset;
                data.ucs_origin = data.ucs_origin + offset;
            }
            ExtendedEntityData::PointCloudEx(data) => {
                // The extents stay in the scan's own coordinates; the
                // placement moves.
                data.ucs_origin = data.ucs_origin + offset;
            }
            ExtendedEntityData::Arch(data) => {
                let _ = data.translate(&mut self.common.graphic_data, offset);
            }
            _ => {}
        }
    }

    fn entity_type(&self) -> &'static str {
        "EXTENDED_ENTITY"
    }

    fn apply_transform(&mut self, transform: &Transform) {
        match &mut self.data {
            ExtendedEntityData::SectionObject(data) => {
                for point in data.vertices.iter_mut().chain(&mut data.back_line_vertices) {
                    *point = transform.apply(*point);
                }
                data.vertical_direction = transform
                    .apply_rotation(data.vertical_direction)
                    .normalize();
            }
            ExtendedEntityData::ArcAlignedText(data) => {
                let old_ocs = Matrix3::arbitrary_axis(data.normal);
                let new_normal = transform.apply_rotation(data.normal).normalize();
                let new_ocs_t = Matrix3::arbitrary_axis(new_normal).transpose();
                let transformed_angle = |angle: f64| {
                    let direction = old_ocs * Vector3::new(angle.cos(), angle.sin(), 0.0);
                    let local = new_ocs_t * transform.apply_rotation(direction);
                    local.y.atan2(local.x)
                };
                data.start_angle = transformed_angle(data.start_angle);
                data.end_angle = transformed_angle(data.end_angle);
                if super::transform::is_reflecting(transform) {
                    std::mem::swap(&mut data.start_angle, &mut data.end_angle);
                }
                data.center = transform.apply(data.center);
                data.normal = new_normal;
                let scale = transform.apply_rotation(old_ocs * Vector3::UNIT_X).length();
                data.radius *= scale;
                data.text_size *= scale;
                data.character_spacing *= scale;
                data.offset_from_arc *= scale;
                data.right_offset *= scale;
                data.left_offset *= scale;
            }
            ExtendedEntityData::RemoteText(data) => {
                let old_ocs = Matrix3::arbitrary_axis(data.normal);
                let direction =
                    old_ocs * Vector3::new(data.rotation.cos(), data.rotation.sin(), 0.0);
                let new_normal = transform.apply_rotation(data.normal).normalize();
                let rotated = Matrix3::arbitrary_axis(new_normal).transpose()
                    * transform.apply_rotation(direction);
                if rotated.x != 0.0 || rotated.y != 0.0 {
                    data.rotation = rotated.y.atan2(rotated.x);
                }
                data.position = transform.apply(data.position);
                data.normal = new_normal;
                data.height *= transform.apply_rotation(direction).length();
            }
            ExtendedEntityData::GeoPositionMarker(data) => {
                data.position = transform.apply(data.position);
                let scale = transform.apply_rotation(Vector3::UNIT_X).length();
                data.radius *= scale;
                data.landing_gap *= scale;
                if let Some(text) = &mut data.embedded_mtext {
                    text.apply_transform(transform);
                }
            }
            ExtendedEntityData::CoordinationModel(data) => {
                let mut placement = Matrix4::zero();
                for (row, values) in placement.m.iter_mut().enumerate() {
                    values.copy_from_slice(&data.transform[row * 4..row * 4 + 4]);
                }
                let placement = transform.matrix * placement;
                for (row, values) in placement.m.iter().enumerate() {
                    data.transform[row * 4..row * 4 + 4].copy_from_slice(values);
                }
            }
            ExtendedEntityData::PointCloud(data) => {
                data.origin = transform.apply(data.origin);
                let (min, max) = transformed_extents(data.extents_min, data.extents_max, transform);
                data.extents_min = min;
                data.extents_max = max;
                data.ucs_origin = transform.apply(data.ucs_origin);
                data.ucs_x_direction = transform.apply_rotation(data.ucs_x_direction);
                data.ucs_y_direction = transform.apply_rotation(data.ucs_y_direction);
                data.ucs_z_direction = transform.apply_rotation(data.ucs_z_direction);
            }
            ExtendedEntityData::PointCloudEx(data) => {
                // Extents and crop geometry live in the scan's local frame.
                data.ucs_origin = transform.apply(data.ucs_origin);
                data.ucs_x_direction = transform.apply_rotation(data.ucs_x_direction);
                data.ucs_y_direction = transform.apply_rotation(data.ucs_y_direction);
                data.ucs_z_direction = transform.apply_rotation(data.ucs_z_direction);
            }
            ExtendedEntityData::Arch(data) => {
                let _ = data.transform_points(&mut self.common.graphic_data, |point| {
                    transform.apply(point)
                });
                return;
            }
            // Cameras refer to VIEW table records. Remaining classes contain
            // opaque payloads or drawing metadata, with no editable geometry.
            ExtendedEntityData::Camera { .. }
            | ExtendedEntityData::Proxy(_)
            | ExtendedEntityData::OleFrame(_)
            | ExtendedEntityData::LayoutPrintConfig(_)
            | ExtendedEntityData::Format(_)
            | ExtendedEntityData::Legacy(_)
            | ExtendedEntityData::DynamicBlock(_)
            | ExtendedEntityData::RegisteredClass(_) => return,
        }
        self.common.raw_record = None;
    }
}

fn transformed_extents(min: Vector3, max: Vector3, transform: &Transform) -> (Vector3, Vector3) {
    let corners: [Vector3; 8] = std::array::from_fn(|index| {
        transform.apply(Vector3::new(
            if index & 1 == 0 { min.x } else { max.x },
            if index & 2 == 0 { min.y } else { max.y },
            if index & 4 == 0 { min.z } else { max.z },
        ))
    });
    let bounds = BoundingBox3D::from_points(&corners).unwrap();
    (bounds.min, bounds.max)
}

pub fn point_cloud_ex_to_world(data: &PointCloudExData, p: Vector3) -> Vector3 {
    data.ucs_origin
        + data.ucs_x_direction * p.x
        + data.ucs_y_direction * p.y
        + data.ucs_z_direction * p.z
}

pub fn point_cloud_ex_corners(data: &PointCloudExData) -> [Vector3; 8] {
    let (a, b) = (data.extents_min, data.extents_max);
    let mut out = [Vector3::ZERO; 8];
    for (i, corner) in out.iter_mut().enumerate() {
        let p = Vector3::new(
            if i & 1 == 0 { a.x } else { b.x },
            if i & 2 == 0 { a.y } else { b.y },
            if i & 4 == 0 { a.z } else { b.z },
        );
        *corner = point_cloud_ex_to_world(data, p);
    }
    out
}
