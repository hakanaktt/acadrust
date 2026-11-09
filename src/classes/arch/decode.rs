//! Class-independent view of an Arch record.
//!
//! Every AEC entity and dictionary object starts with the same tag: marker
//! 80, two set bits, and the class schema number. Entities then carry a
//! placement identifier and a 3-axis frame. The text stream holds every
//! string the class wrote, in order, and the handle stream holds the class
//! body's references. Callers do not need a per-class layout to read those.

use super::{ArchEntityData, ArchKind, ArchObject};
use crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use crate::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use crate::types::{DxfVersion, Vector3};

/// Placement stored by an Arch entity after its schema tag.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AecPlacement {
    /// GUID or empty name written before the axes. Newly built entities use
    /// an empty identifier.
    pub identifier: String,
    pub origin: Vector3,
    pub x_axis: Vector3,
    pub y_axis: Vector3,
    pub z_axis: Vector3,
    /// The axes are orthonormal and `z_axis` is world up.
    pub rigid: bool,
}

/// What can be read from any Arch class without a private schema.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecodedAec {
    pub dxf_name: String,
    pub cpp_class_name: String,
    pub kind: ArchKind,
    /// `true` for a drawn entity, `false` for a dictionary object such as a style.
    pub is_entity: bool,
    /// Schema number from the shared AEC tag, when that tag is present.
    pub schema: Option<i16>,
    /// Strings from the class text stream, in the order the class wrote them.
    pub strings: Vec<String>,
    /// Entity placement. Dictionary objects do not have one.
    pub placement: Option<AecPlacement>,
    /// Closed or open polylines from the entity's proxy graphics.
    pub profiles: Vec<Vec<Vector3>>,
    /// Handles stored in the class body, after the common DWG header.
    pub handles: Vec<u64>,
}

impl ArchEntityData {
    /// Read the shared Arch envelope from this entity.
    ///
    /// The result always carries the class name, kind, proxy profiles, text
    /// stream, and class-body handles. `schema` and `placement` are set when
    /// the record uses the standard entity tag. Typed readers such as
    /// [`Self::straight_wall_geometry`](ArchEntityData::straight_wall_geometry)
    /// still interpret the class-specific fields that follow the envelope.
    pub fn decode(&self) -> DecodedAec {
        let mut decoded = identity(self.dxf_name.clone(), self.cpp_class_name.clone(), true);
        decoded.profiles.clone_from(&self.profiles);
        if let Ok(reader) = open_entity(self) {
            finish(&mut decoded, reader, true);
        }
        decoded
    }
}

impl ArchObject {
    /// Read the shared Arch envelope from this dictionary object.
    ///
    /// Styles, anchors, and display objects use the same opening tag as
    /// entities and then omit the placement frame.
    pub fn decode(&self) -> DecodedAec {
        let mut decoded = identity(self.dxf_name.clone(), self.cpp_class_name.clone(), false);
        if let Ok(reader) = open_object(self) {
            finish(&mut decoded, reader, false);
        }
        decoded
    }
}

fn identity(dxf_name: String, cpp_class_name: String, is_entity: bool) -> DecodedAec {
    let kind = crate::classes::arch::kind_of(&dxf_name).unwrap_or(ArchKind::Other);
    DecodedAec {
        dxf_name,
        cpp_class_name,
        kind,
        is_entity,
        schema: None,
        strings: Vec::new(),
        placement: None,
        profiles: Vec::new(),
        handles: Vec::new(),
    }
}

fn finish(decoded: &mut DecodedAec, mut reader: DwgMergedReader, entity: bool) {
    decoded.strings = read_strings(&reader);
    decoded.handles = read_handles(&reader);
    if entity {
        let (schema, placement) = read_entity_tag(&mut reader);
        decoded.schema = schema;
        decoded.placement = placement;
    } else {
        decoded.schema = read_object_tag(&mut reader);
    }
}

pub(crate) fn open_entity(data: &ArchEntityData) -> crate::error::Result<DwgMergedReader> {
    let raw = data.raw_dwg_data.as_deref().ok_or_else(|| {
        crate::error::DxfError::Parse("architecture entity has no DWG body".into())
    })?;
    let version = data.dwg_source_version.unwrap_or(DxfVersion::AC1032);
    let objects = DwgObjectReader::new(Vec::new(), version, Default::default())?;
    let (type_code, mut reader) = objects.read_raw_record(raw, data.dwg_handle_bits)?;
    let common = objects.read_common_entity_data(&mut reader, type_code);
    reader.set_ref_handle(common.common.handle);
    reader.check_error()?;
    Ok(reader)
}

fn open_object(object: &ArchObject) -> crate::error::Result<DwgMergedReader> {
    let raw = object.raw_dwg_data.as_deref().ok_or_else(|| {
        crate::error::DxfError::Parse("architecture object has no DWG body".into())
    })?;
    let version = object.raw_dwg_version.unwrap_or(DxfVersion::AC1032);
    let objects = DwgObjectReader::new(Vec::new(), version, Default::default())?;
    let (type_code, mut reader) = objects.read_raw_record(raw, object.raw_dwg_handle_bits)?;
    reader.set_ref_handle(object.handle.value());
    objects.read_common_non_entity_data(&mut reader, type_code);
    reader.check_error()?;
    Ok(reader)
}

fn read_strings(reader: &DwgMergedReader) -> Vec<String> {
    let mut reader = reader.clone();
    let mut strings = Vec::new();
    while reader.text_remaining_bits() > 0 && strings.len() < 10_000 {
        let remaining = reader.text_remaining_bits();
        let Some(text) = reader.try_read_variable_text() else {
            break;
        };
        if reader.check_error().is_err() || reader.text_remaining_bits() >= remaining {
            break;
        }
        strings.push(text);
    }
    strings
}

fn read_handles(reader: &DwgMergedReader) -> Vec<u64> {
    let mut reader = reader.clone();
    let mut handles = Vec::new();
    while reader.handle_remaining_bits() >= 8 && handles.len() < 10_000 {
        let remaining = reader.handle_remaining_bits();
        let value = reader.read_handle();
        if reader.check_error().is_err() || reader.handle_remaining_bits() >= remaining {
            break;
        }
        handles.push(value);
    }
    handles
}

fn read_entity_tag(reader: &mut DwgMergedReader) -> (Option<i16>, Option<AecPlacement>) {
    if reader.read_bit_short() != 80 || !reader.read_bit() || !reader.read_bit() {
        return (None, None);
    }
    let schema = reader.read_bit_short();
    if reader.check_error().is_err() {
        return (None, None);
    }
    if reader.read_bit_short() != 80 {
        return (Some(schema), None);
    }
    let _description = reader.read_variable_text();
    if reader.read_bit_short() != 80
        || reader.read_bit_short() != 0
        || reader.read_byte() != 1
        || reader.read_bit()
        || reader.read_bit_short() != 80
    {
        return (Some(schema), None);
    }
    let identifier = reader.read_variable_text();
    let mut axes = [Vector3::ZERO; 4];
    for axis in &mut axes {
        let x = reader.read_bit_double();
        let y = reader.read_bit_double();
        let z = reader.read_bit_double();
        if !x.is_finite() || !y.is_finite() || !z.is_finite() || reader.check_error().is_err() {
            return (Some(schema), None);
        }
        *axis = Vector3::new(x, y, z);
    }
    let _tail = reader.read_variable_text();
    let x_axis = axes[1];
    let y_axis = axes[2];
    let z_axis = axes[3];
    let rigid = (x_axis.length() - 1.0).abs() <= 1e-6
        && (y_axis.length() - 1.0).abs() <= 1e-6
        && (z_axis.length() - 1.0).abs() <= 1e-6
        && x_axis.dot(&y_axis).abs() <= 1e-6
        && y_axis.dot(&z_axis).abs() <= 1e-6
        && z_axis.dot(&x_axis).abs() <= 1e-6;
    (
        Some(schema),
        Some(AecPlacement {
            identifier,
            origin: axes[0],
            x_axis,
            y_axis,
            z_axis,
            rigid,
        }),
    )
}

fn read_object_tag(reader: &mut DwgMergedReader) -> Option<i16> {
    if reader.read_bit_short() != 80 || !reader.read_bit() || !reader.read_bit() {
        return None;
    }
    let schema = reader.read_bit_short();
    reader.check_error().ok()?;
    Some(schema)
}
