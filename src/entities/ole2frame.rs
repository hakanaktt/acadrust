//! OLE2FRAME entity — embedded OLE object in a drawing

use super::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Transform, Vector3};

/// The DWG/DXF frame header in front of an OLE2FRAME's compound file.
///
/// As saved, the data is a 128-byte header and then the compound file. The
/// header: u16 kind (`0x5580` or `0x5581`), the frame's four corners (upper
/// left, upper right, lower right, lower left; three RD each), 26 bytes not
/// decoded, then a u32 size, and the compound file, which fills the rest of
/// the data. The size is the compound file's length in most drawings and one
/// 512-byte sector more in some; it is kept as read while the compound file is
/// written back unchanged. The upper-left and lower-right corners are the frame's own fields;
/// the other two are kept here, since a frame need not be axis aligned.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OleFrameEnvelope {
    #[default]
    None,
    Geometry {
        /// The header kind, `0x5580` or `0x5581`.
        marker: u16,
        upper_right: Vector3,
        lower_left: Vector3,
        /// The 26 bytes between the corners and the compound-file size. The
        /// u32 at their offset 16 (header offset 114) is the item type: 1 for
        /// an embedded and 0 for a static object (equal to what the
        /// application reports for all 24 frames checked).
        extension_records: Vec<crate::compound_file::BinaryRecord>,
    },
    /// Data of any other layout, kept as read.
    Legacy {
        records: Vec<crate::compound_file::BinaryRecord>,
    },
}

/// Length of the OLE2FRAME frame header (see [`OleFrameEnvelope`]).
const FRAME_HEADER_LEN: usize = 128;

/// Offset of the item type field (u32) within the undecoded header bytes,
/// which start at header offset 98: header offset 114.
const ITEM_TYPE_OFFSET: usize = 16;

/// The header kind, the four corners, the undecoded header bytes and the
/// compound file of OLE2FRAME data, when it has the saved layout
/// ([`OleFrameEnvelope`]): a known kind, then the compound file after the
/// 128-byte header.
pub(crate) fn frame_parts(data: &[u8]) -> Option<(u16, [Vector3; 4], &[u8], &[u8])> {
    let header = data.get(..FRAME_HEADER_LEN)?;
    let kind = u16::from_le_bytes([header[0], header[1]]);
    if !matches!(kind, 0x5580 | 0x5581) {
        return None;
    }
    let value = |at: usize| f64::from_le_bytes(header[at..at + 8].try_into().unwrap());
    let corner = |index: usize| {
        let at = 2 + 24 * index;
        Vector3::new(value(at), value(at + 8), value(at + 16))
    };
    let corners = [corner(0), corner(1), corner(2), corner(3)];
    Some((kind, corners, &header[98..124], &data[FRAME_HEADER_LEN..]))
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
    /// Object type (link, embedded, static). In DWG data with the saved frame
    /// header it is the header's item type field (see [`OleFrameEnvelope`]).
    pub ole_object_type: OleObjectType,
    /// Whether the object is in paper space
    pub is_paper_space: bool,
    /// OLE wrapper records and decoded compound-file storage.
    pub storage: crate::compound_file::StructuredStoragePayload,
    /// Typed DWG wrapper preceding the compound file.
    pub envelope: OleFrameEnvelope,
    /// DWG tile mode descriptor (0=model, 1=paper, 2=model in layout)
    pub dwg_mode: i16,
    /// The frame's plot quality (DXF 73): the byte after the data in a DWG
    /// record (0 monochrome, 1 low graphics, 2 high graphics, as the
    /// application reports it). The name is kept for compatibility.
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
pub struct OleSourceStorage(Option<(u64, std::sync::Arc<Vec<u8>>, Option<u32>)>);

impl OleSourceStorage {
    fn digest(bytes: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        // Fixed-key SipHash: stable within a process, which is all the comparison needs.
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }

    /// Source bytes and the frame header's size as read, if `encoded` still
    /// matches what they decoded to.
    fn matching(&self, encoded: &[u8]) -> Option<(&[u8], Option<u32>)> {
        self.0
            .as_ref()
            .filter(|(digest, _, _)| *digest == Self::digest(encoded))
            .map(|(_, raw, size)| (raw.as_slice(), *size))
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
            Some((_, raw, _)) => write!(f, "OleSourceStorage({} bytes)", raw.len()),
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
        if let Some(kind) = self.header_item_type() {
            self.ole_object_type = kind;
        }
        self.source_storage = OleSourceStorage::default();
        // The envelope's length does not depend on its field values.
        if let Some(raw) = data.get(self.encoded_envelope(0).len()..) {
            let digest = OleSourceStorage::digest(&self.storage.encode());
            let size = match self.envelope {
                OleFrameEnvelope::Geometry { .. } => Some(u32::from_le_bytes(
                    data[FRAME_HEADER_LEN - 4..FRAME_HEADER_LEN].try_into().unwrap(),
                )),
                _ => None,
            };
            self.source_storage =
                OleSourceStorage(Some((digest, std::sync::Arc::new(raw.to_vec()), size)));
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
        let Some((marker, corners, extension, compound)) = frame_parts(data) else {
            // Another layout stays as read; its corners are not known.
            return (
                crate::compound_file::StructuredStoragePayload::default(),
                OleFrameEnvelope::Legacy {
                    records: crate::compound_file::BinaryRecord::split(data, 4096),
                },
                Vector3::new(1.0, 1.0, 0.0),
                Vector3::ZERO,
            );
        };
        (
            crate::compound_file::StructuredStoragePayload::decode(compound),
            OleFrameEnvelope::Geometry {
                marker,
                upper_right: corners[1],
                lower_left: corners[3],
                extension_records: crate::compound_file::BinaryRecord::split(extension, 4096),
            },
            corners[0],
            corners[2],
        )
    }

    /// The item type field of the frame header (offset 114, a u32: 1 for an
    /// embedded and 0 for a static object). Other values are not known and
    /// give `None`.
    fn header_item_type(&self) -> Option<OleObjectType> {
        let OleFrameEnvelope::Geometry {
            extension_records, ..
        } = &self.envelope
        else {
            return None;
        };
        let extension = crate::compound_file::BinaryRecord::join(extension_records);
        let field = extension.get(ITEM_TYPE_OFFSET..ITEM_TYPE_OFFSET + 4)?;
        match u32::from_le_bytes(field.try_into().ok()?) {
            1 => Some(OleObjectType::Embedded),
            0 => Some(OleObjectType::Static),
            _ => None,
        }
    }

    pub fn encoded_payload(&self) -> Vec<u8> {
        let encoded = self.storage.encode();
        let (storage, size) = self
            .source_storage
            .matching(&encoded)
            .unwrap_or((&encoded, None));
        let mut output = self.encoded_envelope(size.unwrap_or(storage.len() as u32));
        output.extend_from_slice(storage);
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

    /// The DWG wrapper in front of the compound file, with `size` in its size field.
    fn encoded_envelope(&self, size: u32) -> Vec<u8> {
        match &self.envelope {
            OleFrameEnvelope::None => Vec::new(),
            OleFrameEnvelope::Legacy { records } => {
                crate::compound_file::BinaryRecord::join(records)
            }
            OleFrameEnvelope::Geometry {
                marker,
                upper_right,
                lower_left,
                extension_records,
            } => {
                let mut bytes = marker.to_le_bytes().to_vec();
                for point in [
                    self.upper_left_corner,
                    *upper_right,
                    self.lower_right_corner,
                    *lower_left,
                ] {
                    bytes.extend_from_slice(&point.x.to_le_bytes());
                    bytes.extend_from_slice(&point.y.to_le_bytes());
                    bytes.extend_from_slice(&point.z.to_le_bytes());
                }
                let mut extension = crate::compound_file::BinaryRecord::join(extension_records);
                // The item type is written from `ole_object_type` when the header
                // field holds a known value; a link keeps the field as read.
                let item_type = match self.ole_object_type {
                    OleObjectType::Embedded => Some(1u32),
                    OleObjectType::Static => Some(0u32),
                    OleObjectType::Link => None,
                };
                if let (Some(value), Some(field)) = (
                    item_type.filter(|_| self.header_item_type().is_some()),
                    extension.get_mut(ITEM_TYPE_OFFSET..ITEM_TYPE_OFFSET + 4),
                ) {
                    field.copy_from_slice(&value.to_le_bytes());
                }
                bytes.extend_from_slice(&extension);
                bytes.extend_from_slice(&size.to_le_bytes());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compound_file::{CompoundFile, StructuredStoragePayload};

    /// OLE2FRAME data as saved: the 128-byte frame header (kind, the four
    /// corners, 26 bytes, the compound-file size), then the compound file.
    fn saved(kind: u16, corners: [Vector3; 4], compound: &[u8]) -> Vec<u8> {
        let mut data = kind.to_le_bytes().to_vec();
        for point in corners {
            for value in [point.x, point.y, point.z] {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        data.extend((0..26).map(|i| i as u8));
        data.extend_from_slice(&(compound.len() as u32).to_le_bytes());
        data.extend_from_slice(compound);
        data
    }

    fn compound() -> Vec<u8> {
        StructuredStoragePayload {
            compound_file: Some(CompoundFile::default()),
            ..StructuredStoragePayload::default()
        }
        .encode()
    }

    /// The header is read field by field: a frame turned off the axes keeps
    /// all four corners, and the compound file starts right after it.
    #[test]
    fn frame_header_gives_the_corners_and_the_compound_file() {
        let corners = [
            Vector3::new(36555.5, -3814.25, 0.0),
            Vector3::new(36725.75, -3660.5, 0.0),
            Vector3::new(36843.25, -3790.75, 0.0),
            Vector3::new(36673.0, -3944.5, 0.0),
        ];
        let compound = compound();
        let data = saved(0x5580, corners, &compound);
        let mut frame = Ole2Frame::new();
        let (upper_left, lower_right) = frame.set_payload(&data);
        (frame.upper_left_corner, frame.lower_right_corner) = (upper_left, lower_right);
        assert_eq!((upper_left, lower_right), (corners[0], corners[2]));
        assert!(frame.storage.compound_file.is_some());
        assert!(frame.storage.leading_records.is_empty());
        match &frame.envelope {
            OleFrameEnvelope::Geometry {
                marker,
                upper_right,
                lower_left,
                ..
            } => assert_eq!((*marker, *upper_right, *lower_left), (0x5580, corners[1], corners[3])),
            other => panic!("expected the frame header, got {other:?}"),
        }
        assert_eq!(frame.encoded_payload(), data);
    }

    /// Some drawings store a size one 512-byte sector larger than the compound
    /// file after the header. The compound file is still read, the size is
    /// written back as read while it is unchanged, and an edited compound file
    /// gets its own length.
    #[test]
    fn a_size_past_the_compound_file_is_kept_as_read() {
        let compound = compound();
        let mut data = saved(0x5580, [Vector3::ZERO; 4], &compound);
        data[124..128].copy_from_slice(&(compound.len() as u32 + 512).to_le_bytes());
        let mut frame = Ole2Frame::new();
        (frame.upper_left_corner, frame.lower_right_corner) = frame.set_payload(&data);
        assert!(matches!(frame.envelope, OleFrameEnvelope::Geometry { .. }));
        assert!(frame.storage.compound_file.is_some());
        assert_eq!(frame.encoded_payload(), data);

        frame.storage = StructuredStoragePayload::default();
        let edited = frame.encoded_payload();
        let size = u32::from_le_bytes(edited[124..128].try_into().unwrap()) as usize;
        assert_eq!(size, edited.len() - FRAME_HEADER_LEN);
    }

    /// Data without the saved header (another kind) is kept as read and no
    /// compound file is searched for in it.
    #[test]
    fn other_layouts_stay_as_read() {
        let corners = [Vector3::ZERO; 4];
        let compound = compound();
        for data in [saved(0x0080, corners, &compound)] {
            let mut frame = Ole2Frame::new();
            let (upper_left, lower_right) = frame.set_payload(&data);
            assert_eq!(upper_left, Vector3::new(1.0, 1.0, 0.0));
            assert_eq!(lower_right, Vector3::ZERO);
            assert!(matches!(frame.envelope, OleFrameEnvelope::Legacy { .. }));
            assert!(frame.storage.compound_file.is_none());
            assert_eq!(frame.encoded_payload(), data);
            assert_eq!(crate::entities::extract_presentation(&data), None);
        }
    }
    /// The u32 at header offset 114 is the item type: 1 for an embedded and
    /// 0 for a static object. It is read into `ole_object_type`, written back
    /// from it, and an unknown value leaves both as they are.
    #[test]
    fn header_item_type_is_read_and_written() {
        let compound = compound();
        for (value, kind) in [(1u32, OleObjectType::Embedded), (0, OleObjectType::Static)] {
            let mut data = saved(0x5581, [Vector3::ZERO; 4], &compound);
            data[114..118].copy_from_slice(&value.to_le_bytes());
            let mut frame = Ole2Frame::new();
            (frame.upper_left_corner, frame.lower_right_corner) = frame.set_payload(&data);
            assert_eq!(frame.ole_object_type, kind);
            assert_eq!(frame.encoded_payload(), data);
            frame.ole_object_type = match kind {
                OleObjectType::Embedded => OleObjectType::Static,
                _ => OleObjectType::Embedded,
            };
            let mut changed = data.clone();
            changed[114..118].copy_from_slice(&(1 - value).to_le_bytes());
            assert_eq!(frame.encoded_payload(), changed);
        }
        let mut data = saved(0x5580, [Vector3::ZERO; 4], &compound);
        data[114..118].copy_from_slice(&7u32.to_le_bytes());
        let mut frame = Ole2Frame::new();
        frame.ole_object_type = OleObjectType::Static;
        (frame.upper_left_corner, frame.lower_right_corner) = frame.set_payload(&data);
        assert_eq!(frame.ole_object_type, OleObjectType::Static);
        assert_eq!(frame.encoded_payload(), data);
    }
}
