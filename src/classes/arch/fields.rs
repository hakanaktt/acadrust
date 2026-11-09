//! Named numbers taken out of an Arch entity's class bytes and preview.

use super::geometry::polyline_from_type6;
use super::metrics::{door_size, member_size, opening_size, stair_size, window_size};
use super::ArchEntityData;
use crate::entities::{ProxyGraphicRecord, ProxyGraphics};
use crate::types::Vector3;

/// One number found in the class bytes, with the role it plays for this entity.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StoredNumber {
    pub bit: i64,
    pub value: f64,
    pub role: NumberRole,
}

/// What a stored number means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NumberRole {
    ProfileX,
    ProfileY,
    /// Reserved for compatibility; no semantic marker layout has been decoded.
    Marker,
    Width,
    Height,
    Length,
    /// Beam or column cross-section size.
    Section,
    Other,
}

/// One piece of the proxy-graphics preview.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PreviewRecord {
    Polyline(Vec<Vector3>),
    Text(String),
    FillOff,
    Other(u32),
}

pub(crate) fn decode_preview(graphic_data: Option<&[u8]>) -> Vec<PreviewRecord> {
    let Some(graphic_data) = graphic_data else {
        return Vec::new();
    };
    let Some(graphics) = ProxyGraphics::decode(graphic_data) else {
        return Vec::new();
    };
    graphics
        .records
        .into_iter()
        .map(|record| match record {
            ProxyGraphicRecord::FillOff => PreviewRecord::FillOff,
            ProxyGraphicRecord::UnicodeText(text) => PreviewRecord::Text(text.text),
            ProxyGraphicRecord::Unknown { record_type, data } => {
                if record_type == 6 {
                    polyline_from_type6(&data)
                        .map(PreviewRecord::Polyline)
                        .unwrap_or(PreviewRecord::Other(record_type))
                } else {
                    PreviewRecord::Other(record_type)
                }
            }
        })
        .collect()
}

pub(crate) fn decode_numbers(data: &ArchEntityData) -> Vec<StoredNumber> {
    let Some(raw) = data.raw_dwg_data.as_deref() else {
        return Vec::new();
    };
    let mut named = Vec::new();
    match data.kind {
        super::ArchKind::Door => {
            if let Some(size) = door_size(data) {
                named.push((size.width_bit, NumberRole::Width));
                named.push((size.height_bit, NumberRole::Height));
            }
        }
        super::ArchKind::Opening => {
            if let Some(size) = opening_size(data) {
                named.push((size.width_bit, NumberRole::Width));
                named.push((size.height_bit, NumberRole::Height));
            }
        }
        super::ArchKind::Window => {
            if let Some(size) = window_size(data) {
                named.push((size.width_bit, NumberRole::Width));
                named.push((size.height_bit, NumberRole::Height));
            }
        }
        super::ArchKind::Stair => {
            if let Some(size) = stair_size(data) {
                named.push((size.run_bit, NumberRole::Length));
                named.push((size.width_bit, NumberRole::Width));
                named.push((size.height_bit, NumberRole::Height));
            }
        }
        super::ArchKind::StructuralMember => {
            if let Some(size) = member_size(data) {
                named.push((size.length_bit, NumberRole::Length));
                named.push((size.breadth_bit, NumberRole::Section));
                named.push((size.depth_bit, NumberRole::Section));
            }
        }
        _ => {}
    }
    if data.kind == super::ArchKind::Wall && data.straight_wall_geometry().is_some() {
        if let Some(fields) = super::geometry::find_straight_wall_fields(data) {
            named.push((fields.start_x, NumberRole::ProfileX));
            named.push((fields.start_y, NumberRole::ProfileY));
            named.push((fields.width, NumberRole::Width));
            named.push((fields.length, NumberRole::Length));
            named.push((fields.height, NumberRole::Height));
        }
    }
    named
        .into_iter()
        .filter_map(|(bit, role)| {
            let value = super::geometry::read_encoded_double(raw, bit)?.0;
            Some(StoredNumber { bit, value, role })
        })
        .collect()
}
