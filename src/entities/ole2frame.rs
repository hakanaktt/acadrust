//! OLE2FRAME entity — embedded OLE object in a drawing

use super::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Transform, Vector3};

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OleFrameEnvelope {
    #[default]
    None,
    Geometry {
        marker: u16,
        extension_records: Vec<crate::compound_file::BinaryRecord>,
    },
    Legacy {
        records: Vec<crate::compound_file::BinaryRecord>,
    },
}

/// OLE object type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(i16)]
pub enum OleObjectType {
    /// Linked OLE object
    Link = 1,
    /// Embedded OLE object
    Embedded = 2,
    /// Static OLE object
    Static = 3,
}

impl OleObjectType {
    /// Create from DXF code value
    pub fn from_i16(v: i16) -> Self {
        match v {
            1 => OleObjectType::Link,
            3 => OleObjectType::Static,
            _ => OleObjectType::Embedded,
        }
    }
}

/// An embedded OLE2 object entity.
///
/// Stores the decoded OLE compound storage and bounding rectangle.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ole2Frame {
    /// Common entity data
    pub common: EntityCommon,
    /// OLE version (typically 2)
    pub version: i16,
    /// Name of the source application (e.g. "Excel.Sheet.12")
    pub source_application: String,
    /// Upper-left corner of the OLE frame
    pub upper_left_corner: Vector3,
    /// Lower-right corner of the OLE frame
    pub lower_right_corner: Vector3,
    /// Object type (link, embedded, static)
    pub ole_object_type: OleObjectType,
    /// Whether the object is in paper space
    pub is_paper_space: bool,
    /// OLE wrapper records and decoded compound-file storage.
    pub storage: crate::compound_file::StructuredStoragePayload,
    /// Typed DWG wrapper preceding the compound file.
    pub envelope: OleFrameEnvelope,
    /// DWG tile mode descriptor (0=model, 1=paper, 2=model in layout)
    pub dwg_mode: i16,
    /// Preserve the OLE object's aspect ratio while resizing.
    pub lock_aspect: u8,
    /// Compound-file bytes exactly as read, written back while `storage` is unchanged.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub source_storage: OleSourceStorage,
}

/// The compound file of an OLE2FRAME as it was read. Re-encoding a decoded compound file
/// changes its sector layout and size, while the DWG wrapper in front of it still records
/// the original size, so other DWG readers reject the rebuilt object and drop the frame.
/// Keeping the source bytes makes an unedited (or only moved/resized) frame round-trip
/// verbatim. Never part of entity equality.
#[derive(Clone, Default)]
pub struct OleSourceStorage(Option<(u64, std::sync::Arc<Vec<u8>>)>);

impl OleSourceStorage {
    fn digest(bytes: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        // Fixed-key SipHash: stable within a process, which is all the comparison needs.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }

    /// Source bytes, if `encoded` still matches what they decoded to.
    fn matching(&self, encoded: &[u8]) -> Option<&[u8]> {
        self.0
            .as_ref()
            .filter(|(digest, _)| *digest == Self::digest(encoded))
            .map(|(_, raw)| raw.as_slice())
    }
}

impl PartialEq for OleSourceStorage {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for OleSourceStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some((_, raw)) => write!(f, "OleSourceStorage({} bytes)", raw.len()),
            None => f.write_str("OleSourceStorage(None)"),
        }
    }
}

impl Ole2Frame {
    /// Create a new OLE2FRAME with defaults
    pub fn new() -> Self {
        Ole2Frame {
            common: EntityCommon::new(),
            version: 2,
            source_application: String::new(),
            upper_left_corner: Vector3::new(1.0, 1.0, 0.0),
            lower_right_corner: Vector3::ZERO,
            ole_object_type: OleObjectType::Embedded,
            is_paper_space: false,
            storage: crate::compound_file::StructuredStoragePayload::default(),
            envelope: OleFrameEnvelope::None,
            dwg_mode: 0,
            lock_aspect: 0,
            source_storage: OleSourceStorage::default(),
        }
    }

    /// Set the decoded payload of `data` (DWG/DXF OLE data) and remember its compound-file
    /// bytes so the writers can emit them unchanged. Returns the decoded corners.
    pub fn set_payload(&mut self, data: &[u8]) -> (Vector3, Vector3) {
        let (storage, envelope, upper_left, lower_right) = Self::decode_payload(data);
        self.storage = storage;
        self.envelope = envelope;
        self.source_storage = OleSourceStorage::default();
        // The envelope's length does not depend on the corner values.
        if let Some(raw) = data.get(self.encoded_envelope().len()..) {
            let digest = OleSourceStorage::digest(&self.storage.encode());
            self.source_storage =
                OleSourceStorage(Some((digest, std::sync::Arc::new(raw.to_vec()))));
        }
        (upper_left, lower_right)
    }

    pub(crate) fn decode_payload(
        data: &[u8],
    ) -> (
        crate::compound_file::StructuredStoragePayload,
        OleFrameEnvelope,
        Vector3,
        Vector3,
    ) {
        let mut storage = crate::compound_file::StructuredStoragePayload::decode(data);
        let leading = crate::compound_file::BinaryRecord::join(&storage.leading_records);
        let default = (Vector3::new(1.0, 1.0, 0.0), Vector3::new(0.0, 0.0, 0.0));
        if leading.len() < 98 {
            return (storage, OleFrameEnvelope::None, default.0, default.1);
        }
        let read_f64 = |offset: usize| {
            leading
                .get(offset..offset + 8)
                .and_then(|bytes| bytes.try_into().ok())
                .map(f64::from_le_bytes)
        };
        let values = [
            read_f64(2),
            read_f64(10),
            read_f64(18),
            read_f64(26),
            read_f64(34),
            read_f64(42),
            read_f64(50),
            read_f64(58),
            read_f64(66),
            read_f64(74),
            read_f64(82),
            read_f64(90),
        ];
        let Some(values) = values.into_iter().collect::<Option<Vec<f64>>>() else {
            let records = std::mem::take(&mut storage.leading_records);
            return (
                storage,
                OleFrameEnvelope::Legacy { records },
                default.0,
                default.1,
            );
        };
        let finite = values
            .iter()
            .all(|value| value.is_finite() && value.abs() < 1e15);
        let close = |left: f64, right: f64| {
            (left - right).abs() <= 1e-6 * (1.0 + left.abs().max(right.abs()))
        };
        let rectangle = close(values[1], values[4])
            && close(values[7], values[10])
            && close(values[0], values[9])
            && close(values[3], values[6]);
        if !finite || !rectangle {
            let records = std::mem::take(&mut storage.leading_records);
            return (
                storage,
                OleFrameEnvelope::Legacy { records },
                default.0,
                default.1,
            );
        }
        storage.leading_records.clear();
        (
            storage,
            OleFrameEnvelope::Geometry {
                marker: u16::from_le_bytes([leading[0], leading[1]]),
                extension_records: crate::compound_file::BinaryRecord::split(&leading[98..], 4096),
            },
            Vector3::new(values[0], values[1], values[2]),
            Vector3::new(values[6], values[7], values[8]),
        )
    }

    pub fn encoded_payload(&self) -> Vec<u8> {
        let mut output = self.encoded_envelope();
        let storage = self.storage.encode();
        output.extend_from_slice(self.source_storage.matching(&storage).unwrap_or(&storage));
        output
    }

    /// True when this frame came from a file but its compound file was changed, so the
    /// writers re-encode it instead of keeping the source bytes. Other DWG readers may
    /// reject such a rebuilt object, so callers can warn before saving.
    pub fn rewrites_source_storage(&self) -> bool {
        self.source_storage.0.is_some()
            && self
                .source_storage
                .matching(&self.storage.encode())
                .is_none()
    }

    /// The DWG wrapper in front of the compound file (marker, corners, extension records).
    fn encoded_envelope(&self) -> Vec<u8> {
        match &self.envelope {
            OleFrameEnvelope::None => Vec::new(),
            OleFrameEnvelope::Legacy { records } => {
                crate::compound_file::BinaryRecord::join(records)
            }
            OleFrameEnvelope::Geometry {
                marker,
                extension_records,
            } => {
                let upper_right = Vector3::new(
                    self.lower_right_corner.x,
                    self.upper_left_corner.y,
                    self.upper_left_corner.z,
                );
                let lower_left = Vector3::new(
                    self.upper_left_corner.x,
                    self.lower_right_corner.y,
                    self.lower_right_corner.z,
                );
                let mut bytes = marker.to_le_bytes().to_vec();
                for point in [
                    self.upper_left_corner,
                    upper_right,
                    self.lower_right_corner,
                    lower_left,
                ] {
                    bytes.extend_from_slice(&point.x.to_le_bytes());
                    bytes.extend_from_slice(&point.y.to_le_bytes());
                    bytes.extend_from_slice(&point.z.to_le_bytes());
                }
                bytes.extend_from_slice(&crate::compound_file::BinaryRecord::join(
                    extension_records,
                ));
                bytes
            }
        }
    }
}

impl Default for Ole2Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Ole2Frame {
    fn common(&self) -> &super::EntityCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::EntityCommon {
        &mut self.common
    }

    fn bounding_box(&self) -> BoundingBox3D {
        BoundingBox3D::from_points(&[self.upper_left_corner, self.lower_right_corner])
            .unwrap_or_else(|| BoundingBox3D::from_point(self.upper_left_corner))
    }
    fn translate(&mut self, offset: Vector3) {
        super::translate::translate_ole2frame(self, offset);
    }
    fn entity_type(&self) -> &'static str {
        "OLE2FRAME"
    }
    fn apply_transform(&mut self, transform: &Transform) {
        super::transform::transform_ole2frame(self, transform);
    }
}
