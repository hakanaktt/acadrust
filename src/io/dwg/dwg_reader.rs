//! DWG file reader
//!
//! Reads DWG binary files and extracts structural information including
//! file headers, section metadata, and integrity checksums (CRC values).
//!
//! ## AC1021 (R2007) CRC-64 Extraction
//!
//! The AC1021 format stores a 64-bit CRC in the compressed metadata header.
//! This reader extracts and reports all CRC values, including:
//! - **Header CRC-64**: The master integrity checksum at offset 0x108
//! - **Pages Map CRC**: Checksums for the page directory
//! - **Sections Map CRC**: Checksums for the section directory
//! - **Per-page CRC**: Individual page checksums (in section map entries)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use acadrust::io::dwg::dwg_reader::DwgReader;
//!
//! let reader = DwgReader::from_file("drawing.dwg")?;
//! let info = reader.read_file_header()?;
//!
//! // Access CRC-64 from AC1021 files
//! if let Some(metadata) = &info.ac21_metadata {
//!     println!("Header CRC-64: {:#018X}", metadata.header_crc64);
//! }
//! ```

use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};

use crate::error::DxfError;
use crate::io::dwg::checksum::{adler32_checksum, apply_magic_sequence, apply_mask};
use crate::io::dwg::decompressor_ac18::decompress_ac18;
use crate::io::dwg::decompressor_ac21::decompress_ac21;
use crate::io::dwg::dwg21_metadata::Dwg21CompressedMetadata;
use crate::io::dwg::dwg_version::DwgVersion;
use crate::io::dwg::parallel::{map_ordered, worker_count};
use crate::io::dwg::reed_solomon::reed_solomon_decode;
use crate::io::dwg::shard_plan::ShardState;
use crate::io::dwg::size_limits::{
    size_from_file, try_vec_with_capacity, try_zeroed_vec, MAX_DATA_PAGE_SIZE, MAX_SECTION_SIZE,
    MAX_SYSTEM_PAGE_SIZE,
};
use crate::io::read::{push_read_diagnostic, ReadDiagnostic, ReadStage, SourceFormat};
use crate::notification::{NotificationCollection, NotificationType};

fn report_read_error(
    notifications: &mut NotificationCollection,
    diagnostics: &mut Vec<ReadDiagnostic>,
    code: &str,
    stage: ReadStage,
    section: Option<&str>,
    message: String,
) {
    notifications.notify(NotificationType::Error, message.clone());
    let mut diagnostic = ReadDiagnostic::new(code, stage, message);
    diagnostic.section = section.map(str::to_owned);
    push_read_diagnostic(diagnostics, diagnostic);
}

fn parse_template(data: &[u8], version: crate::types::DxfVersion) -> Result<i16, DxfError> {
    let length_bytes = data
        .get(..2)
        .ok_or_else(|| DxfError::InvalidFormat("Truncated Template length".into()))?;
    let description_len = i16::from_le_bytes(length_bytes.try_into().unwrap());
    if description_len < 0 {
        return Err(DxfError::InvalidFormat(
            "Negative Template description length".into(),
        ));
    }

    let character_width = if version >= crate::types::DxfVersion::AC1021 {
        2
    } else {
        1
    };
    let description_bytes = (description_len as usize)
        .checked_mul(character_width)
        .ok_or_else(|| DxfError::InvalidFormat("Invalid Template length".into()))?;
    let measurement_offset = 2usize
        .checked_add(description_bytes)
        .ok_or_else(|| DxfError::InvalidFormat("Invalid Template length".into()))?;
    let measurement_bytes = data
        .get(measurement_offset..measurement_offset + 2)
        .ok_or_else(|| DxfError::InvalidFormat("Truncated Template payload".into()))?;
    let measurement = i16::from_le_bytes(measurement_bytes.try_into().unwrap());
    if !matches!(measurement, 0 | 1) {
        return Err(DxfError::InvalidFormat(format!(
            "Invalid MEASUREMENT value: {measurement}"
        )));
    }
    Ok(measurement)
}

#[cfg(test)]
mod template_tests {
    use super::parse_template;
    use crate::types::DxfVersion;

    #[test]
    fn parses_nonempty_description_and_rejects_malformed_payloads() {
        assert_eq!(
            parse_template(&[3, 0, b'a', b'b', b'c', 1, 0], DxfVersion::AC1018).unwrap(),
            1
        );
        assert_eq!(
            parse_template(&[1, 0, 0, 0, 1, 0], DxfVersion::AC1032).unwrap(),
            1
        );
        assert!(parse_template(&[], DxfVersion::AC1032).is_err());
        assert!(parse_template(&[0xFF, 0xFF, 0, 0], DxfVersion::AC1032).is_err());
        assert!(parse_template(&[3, 0, b'a', b'b', b'c'], DxfVersion::AC1018).is_err());
        assert!(parse_template(&[0, 0, 2, 0], DxfVersion::AC1032).is_err());
    }
}

/// AC1021 file header offset (data pages start after this)
const AC21_FILE_HEADER_SIZE: u64 = 0x480;

fn ac21_page_layout(
    compressed_size: u64,
    correction_factor: u64,
    block_size: usize,
) -> Result<(usize, usize, usize), DxfError> {
    let aligned = compressed_size.wrapping_add(7) & 0xFFFF_FFF8;
    let total_size = aligned.wrapping_mul(correction_factor) as usize;
    if total_size == 0 || total_size > 100_000_000 {
        return Err(DxfError::InvalidFormat(format!(
            "Invalid page buffer size: {} (compressed={}, factor={}, aligned={})",
            total_size, compressed_size, correction_factor, aligned
        )));
    }
    let factor = (total_size + block_size - 1) / block_size;
    Ok((total_size, factor, factor * 255))
}

fn decode_ac21_page(
    encoded: &[u8],
    total_size: usize,
    factor: usize,
    block_size: usize,
    compressed_size: u64,
    uncompressed_size: u64,
    decryption_key: Option<&[u8]>,
) -> Result<Vec<u8>, DxfError> {
    let mut compressed_data = try_zeroed_vec(total_size, "AC21 compressed page data")?;
    reed_solomon_decode(encoded, &mut compressed_data, factor, block_size);
    if let Some(key) = decryption_key {
        let mut rc4 = crate::io::dwg::security::Rc4::new(key);
        let decrypt_len = (compressed_size as usize).min(compressed_data.len());
        rc4.apply(&mut compressed_data[..decrypt_len]);
    }
    if compressed_size == uncompressed_size {
        compressed_data.truncate(uncompressed_size as usize);
        return Ok(compressed_data);
    }

    let mut padded_source = try_zeroed_vec(compressed_data.len() + 64, "AC21 padded page data")?;
    padded_source[..compressed_data.len()].copy_from_slice(&compressed_data);
    let mut decompressed_data = try_zeroed_vec(
        uncompressed_size as usize + 64,
        "AC21 decompressed page data",
    )?;
    decompress_ac21(
        &padded_source,
        0,
        compressed_size as u32,
        &mut decompressed_data,
    );
    decompressed_data.truncate(uncompressed_size as usize);
    Ok(decompressed_data)
}

/// Results from reading a DWG file header.
///
/// Contains version info, section layout, and all extracted CRC values.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DwgFileHeaderInfo {
    /// DWG version string (e.g., "AC1021")
    pub version_string: String,
    /// Parsed DWG version enum
    pub version: DwgVersion,
    /// ACAD maintenance version
    pub acad_maintenance_version: u8,
    /// Preview image address
    pub preview_address: i32,
    /// DWG internal version byte
    pub dwg_version: u8,
    /// Application release version
    pub app_release_version: u8,
    /// Drawing code page
    pub code_page: u16,
    /// Security type
    pub security_type: i32,
    /// Summary info address
    pub summary_info_addr: i32,
    /// VBA project address
    pub vba_project_addr: i32,

    // ── AC1021-specific data ──
    /// AC1021 compressed metadata (contains CRC-64 and section layout)
    pub ac21_metadata: Option<Dwg21CompressedMetadata>,
    /// Raw Reed-Solomon decoded values from file header
    pub ac21_header_crc: Option<i64>,
    /// Unknown key from AC1021 header
    pub ac21_unknown_key: Option<i64>,
    /// CRC of compressed data in AC1021 header
    pub ac21_compressed_data_crc: Option<i64>,
    /// Page records: page_id → (offset, size)
    pub page_records: HashMap<i32, (i64, i64)>,
    /// Section descriptors from the section map
    pub section_descriptors: Vec<DwgSectionInfo>,

    // ── AC15-specific data ──
    /// Section locator records for AC15 format: name → (file_offset, size)
    pub section_locators: HashMap<String, (i64, i64)>,
    /// Base file offset of AcDb:AcDbObjects section (AC15 only).
    /// Handle offsets in AC15 are absolute; subtract this to get buffer-relative.
    pub objects_base_offset: i64,
    /// Whether this file uses AC18 format (R2004/R2010/R2013/R2018).
    /// Determines which decompression path to use in `get_section_buffer`.
    pub is_ac18_format: bool,
}

/// Information about a DWG section (from the section map).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DwgSectionInfo {
    /// Section name (e.g., "AcDb:Header")
    pub name: String,
    /// Compressed size
    pub compressed_size: u64,
    /// Decompressed size
    pub decompressed_size: u64,
    /// Encryption flag
    pub encrypted: u64,
    /// Hash code
    pub hash_code: u64,
    /// Encoding type (4 = Reed-Solomon + LZ77)
    pub encoding: u64,
    /// Number of pages
    pub page_count: u64,
    /// Per-page CRC values and metadata
    pub pages: Vec<DwgPageCrcInfo>,
}

/// CRC information for a single page within a section.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DwgPageCrcInfo {
    /// Page number
    pub page_number: i64,
    /// Page offset within section
    pub offset: u64,
    /// Page size
    pub size: i64,
    /// Decompressed size
    pub decompressed_size: u64,
    /// Compressed size
    pub compressed_size: u64,
    /// Checksum value
    pub checksum: u64,
    /// **CRC value for this page**
    pub crc: u64,
}

/// Options controlling how DWG files are read.
///
/// When `failsafe` is enabled the reader will attempt to recover as
/// much data as possible from damaged or partially-corrupt files
/// instead of returning an error.  Specific behaviours:
///
/// * File-header parsing errors are caught and an empty/partial
///   result is returned rather than propagating the error.
/// * Missing pages in a section are skipped instead of aborting.
/// * Skipped records and sections are reported through the
///   [`CadDocument::notifications`](crate::document::CadDocument::notifications) collection.
///
/// MLeader annotation contexts, associative geometry dependencies, and edge
/// action parameters with unsupported payload layouts are preserved as raw DWG
/// objects with diagnostics even in strict mode, if their framing and common
/// object metadata are valid. Same-version DWG saves retain those records.
///
/// MESH entities are stored up to [`crate::MeshFaceBudget::from_system`] unless
/// [`Self::max_mesh_faces`] or [`Self::max_mesh_face_indices`] is set.
/// `Some(usize::MAX)` removes that cap. The object record still bounds the
/// face list, and a mesh that exceeds the budget is truncated with a
/// `mesh-face-budget` diagnostic.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DwgReadOptions {
    /// When `true`, recover as much data as possible from corrupt files.
    pub failsafe: bool,
    /// Faces retained from one MESH entity.
    ///
    /// `None` uses the system budget. `Some(usize::MAX)` stores every face
    /// that fits in the object record.
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_mesh_faces: Option<usize>,
    /// Face-corner indices retained from one MESH entity.
    ///
    /// `None` uses the system budget. `Some(usize::MAX)` stores every index
    /// that fits in the object record.
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_mesh_face_indices: Option<usize>,
    /// Upper bound on the decompressed bytes one read may allocate for
    /// sections and system pages (page map, section map).
    ///
    /// Each section's declared output size is charged against this budget
    /// before its buffer is allocated, so a file whose pages add up to
    /// gigabytes fails with a `DxfError` instead of exhausting memory. The
    /// budget is shared by every section in one read and is reset when a read
    /// starts. `None` keeps the previous per-section behaviour.
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_decompressed_bytes: Option<usize>,
    /// Upper bound on the decompressed size of one section, and on the sum
    /// of its pages.
    ///
    /// `None` uses the default of 1 GiB. A large drawing can exceed it: a
    /// 424 MB AC1032 file with 10,000 proxy entities carries an
    /// `AcDb:AcDbObjects` section of about 1.25 GB. Raise it to read such
    /// files; [`Self::max_decompressed_bytes`] still caps the whole read.
    #[cfg_attr(feature = "serde", serde(default))]
    pub max_section_bytes: Option<usize>,
    /// Optional password for decrypting password-protected DWG files.
    #[cfg_attr(feature = "serde", serde(default))]
    pub password: Option<String>,
    /// When `true`, attaching an AcDs modeler blob does not parse it to derive
    /// the entity's `point_of_reference`; that parse is most of the attach
    /// cost on a drawing with many solids.
    ///
    /// The default (`false`) derives the point at load time, as before. When
    /// set, a solid, region or body whose file stored no wireframe anchor keeps
    /// `point_of_reference` at the origin and is flagged
    /// (`AcisData::reference_point_deferred`); `reference_point()` on the
    /// entity derives the point on request, and moving or transforming the
    /// entity derives it first. A DWG written back from such a document omits
    /// the derived anchor that an eager read would have written (the
    /// wireframe point of a pre-R2013 save, and the inline cache a point
    /// forces). The DXF reader always derives eagerly.
    #[cfg_attr(feature = "serde", serde(default))]
    pub defer_solid_reference_point: bool,
    /// When `true`, entities are read without the verbatim bytes a DWG save
    /// copies for an unedited entity: `EntityCommon::raw_record`, and the raw
    /// EED of every XDATA block that decoded into a record. That saves memory
    /// for read-only use. The model is the same; a DWG written from the
    /// document re-encodes those entities and their XDATA records instead of
    /// copying the source bytes. Objects keep their raw data.
    ///
    /// The default (`false`) keeps the bytes, so an unedited entity is saved
    /// byte for byte.
    #[cfg_attr(feature = "serde", serde(default))]
    pub skip_roundtrip_data: bool,
}

impl Default for DwgReadOptions {
    fn default() -> Self {
        Self {
            failsafe: false,
            max_mesh_faces: None,
            max_mesh_face_indices: None,
            max_decompressed_bytes: None,
            max_section_bytes: None,
            password: None,
            defer_solid_reference_point: false,
            skip_roundtrip_data: false,
        }
    }
}

impl DwgReadOptions {
    /// Create options with failsafe mode enabled.
    pub fn failsafe() -> Self {
        Self {
            failsafe: true,
            ..Self::default()
        }
    }

    /// Replace both MESH caps. `usize::MAX` removes a cap.
    pub fn with_mesh_limits(mut self, max_faces: usize, max_face_indices: usize) -> Self {
        self.max_mesh_faces = Some(max_faces);
        self.max_mesh_face_indices = Some(max_face_indices);
        self
    }

    /// Store every MESH face that fits in its object record.
    pub fn with_unlimited_mesh(self) -> Self {
        self.with_mesh_limits(usize::MAX, usize::MAX)
    }

    /// Limit the total decompressed bytes one read may allocate.
    ///
    /// `usize::MAX` effectively removes the limit while still using the
    /// fallible allocation path.
    pub fn with_max_decompressed_bytes(mut self, max_bytes: usize) -> Self {
        self.max_decompressed_bytes = Some(max_bytes);
        self
    }

    /// Limit the decompressed size of one section (see
    /// [`Self::max_section_bytes`]).
    pub fn with_max_section_bytes(mut self, max_bytes: usize) -> Self {
        self.max_section_bytes = Some(max_bytes);
        self
    }

    /// The largest decompressed section size this read accepts.
    pub(crate) fn section_size_limit(&self) -> usize {
        self.max_section_bytes.unwrap_or(MAX_SECTION_SIZE)
    }

    /// Set a password for decrypting password-protected DWG files.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Skip deriving the reference point of AcDs-backed solids at load time
    /// (see [`Self::defer_solid_reference_point`]).
    pub fn with_deferred_solid_reference_point(mut self) -> Self {
        self.defer_solid_reference_point = true;
        self
    }

    /// Read without the bytes kept for a byte-exact re-save
    /// (see [`Self::skip_roundtrip_data`]).
    pub fn without_roundtrip_data(mut self) -> Self {
        self.skip_roundtrip_data = true;
        self
    }

    /// Budget applied while reading MESH entities.
    pub fn mesh_budget(&self) -> crate::io::dwg::mesh_budget::MeshFaceBudget {
        crate::io::dwg::mesh_budget::MeshFaceBudget::from_system()
            .with_overrides(self.max_mesh_faces, self.max_mesh_face_indices)
    }
}

/// Name of the datastore schema whose records hold the modeler bodies.
const ACDS_MODELER_SCHEMA: &[u8] = b"AcDb3DSolid_ASM_Data";
/// Every datastore segment opens with this header: `AC D5`, the 6-byte
/// segment name, u32 segment index, u32 `is_blob01`, u32 `segsize`,
/// u32 unknown, u32 datastore version, u32 unknown, u32 `data_algn_offset`
/// and u32 `objdata_algn_offset` (both in 16-byte units from the segment
/// start), then eight `0x55` bytes.
const ACDS_SEGMENT_HEADER_LEN: usize = 48;
/// A `_data_` record whose data is stored in `blob01` pages holds this value
/// where the data size would be, followed by the page reference.
const ACDS_BLOB_REFERENCE: u32 = 0xBB10_6BB1;
/// A `blob01` page: the segment header, then `total_data_size` (u64),
/// `page_start_offset` (u64), `page_index` (u32), `page_count` (u32) and
/// `page_data_size` (u64); the page's data follows.
const BLOB01_FRAME_LEN: usize = 80;

fn le_u32(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(buf.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

fn le_u64(buf: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(buf.get(at..at.checked_add(8)?)?.try_into().ok()?))
}

/// The segments of an AcDs datastore, located through its segment index.
///
/// The datastore file header (`jard`) holds, as u32 fields, the segment
/// index's offset at 24, its entry count at 32, and the segment index of the
/// schema index (36) and of the data index (40). The segment index (a
/// `segidx` segment) lists, after its header, one entry per segment index:
/// u64 offset from the datastore start and u32 size. A free index is all zero.
struct AcdsSegments<'a> {
    buf: &'a [u8],
    index: Vec<(u64, u32)>,
}

impl<'a> AcdsSegments<'a> {
    fn read(buf: &'a [u8]) -> Option<Self> {
        if buf.get(..4)? != b"jard" {
            return None;
        }
        let at = le_u32(buf, 24)? as usize;
        let count = le_u32(buf, 32)? as usize;
        let table = at.checked_add(ACDS_SEGMENT_HEADER_LEN)?;
        if count > buf.len().saturating_sub(table) / 12 || buf.get(at + 2..at + 8)? != b"segidx" {
            return None;
        }
        let index = (0..count)
            .map(|entry| {
                let p = table + 12 * entry;
                Some((le_u64(buf, p)?, le_u32(buf, p + 8)?))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { buf, index })
    }

    /// Segment `id` (its header included), when the index locates a segment
    /// of that name and index there.
    fn segment(&self, id: u32, name: &[u8; 6]) -> Option<&'a [u8]> {
        let &(offset, size) = self.index.get(id as usize)?;
        let start = usize::try_from(offset).ok()?;
        let segment = self.buf.get(start..start.checked_add(size as usize)?)?;
        (segment.get(..2)? == [0xAC, 0xD5]
            && segment.get(2..8)? == name
            && le_u32(segment, 8)? == id)
            .then_some(segment)
    }

    /// The index of schema `name` in the schema index (`schidx`) segment `id`.
    /// Its schema names sit at its data alignment offset: u32 count, then the
    /// names, each NUL-terminated; the i-th name is that of schema i.
    fn schema(&self, id: u32, name: &[u8]) -> Option<u32> {
        let segment = self.segment(id, b"schidx")?;
        let mut at = (le_u32(segment, 32)? as usize).checked_mul(16)?;
        let count = le_u32(segment, at)?;
        at += 4;
        for schema in 0..count {
            let text = segment.get(at..)?;
            let len = text.iter().position(|&byte| byte == 0)?;
            if &text[..len] == name {
                return Some(schema);
            }
            at += len + 1;
        }
        None
    }
}

/// Each modeler body stored in an AcDs datastore, with the handle of the
/// entity that owns it, read through the datastore's own indexes.
///
/// The data index (`datidx`) holds u32 entry count, u32 unknown, then per
/// record u32 segment index, u32 entry offset and u32 schema index; a
/// segment index of 0 is a free record. Only records of the modeler schema
/// are read. A record's entry in its `_data_` segment, at the entry offset
/// after the segment header, is u32 entry size (20), u32 unknown, u64 owner
/// handle and u32 data offset. Its data, at the data offset from the
/// segment's object-data alignment offset, is a u32 size and that many bytes
/// of SAB, or a blob reference ([`acds_blob_pages`]). A record that does not
/// read this way is left out, so its entity keeps no body.
fn acds_modeler_bodies(buf: &[u8]) -> Vec<(u64, Vec<u8>)> {
    let Some(segments) = AcdsSegments::read(buf) else {
        return Vec::new();
    };
    let schema = le_u32(buf, 36).and_then(|id| segments.schema(id, ACDS_MODELER_SCHEMA));
    let data_index = le_u32(buf, 40).and_then(|id| segments.segment(id, b"datidx"));
    let (Some(schema), Some(data_index)) = (schema, data_index) else {
        return Vec::new();
    };
    let count = le_u32(data_index, ACDS_SEGMENT_HEADER_LEN).unwrap_or(0) as usize;
    let entries = ACDS_SEGMENT_HEADER_LEN + 8;
    if count > data_index.len().saturating_sub(entries) / 12 {
        return Vec::new();
    }
    (0..count)
        .filter_map(|record| {
            let p = entries + 12 * record;
            let (segment, entry) = (le_u32(data_index, p)?, le_u32(data_index, p + 4)?);
            if segment == 0 || le_u32(data_index, p + 8)? != schema {
                return None;
            }
            acds_record(&segments, segment, entry as usize)
        })
        .collect()
}

/// The owner handle and body of the `_data_` record entry `entry` of segment
/// `segment` (see [`acds_modeler_bodies`]).
fn acds_record(segments: &AcdsSegments, segment: u32, entry: usize) -> Option<(u64, Vec<u8>)> {
    let data = segments.segment(segment, b"_data_")?;
    let at = ACDS_SEGMENT_HEADER_LEN.checked_add(entry)?;
    if le_u32(data, at)? != 20 {
        return None;
    }
    let handle = le_u64(data, at + 8)?;
    let value = (le_u32(data, 36)? as usize)
        .checked_mul(16)?
        .checked_add(le_u32(data, at + 16)? as usize)?;
    let size = le_u32(data, value)?;
    let body = if size == ACDS_BLOB_REFERENCE {
        acds_blob_pages(segments, data.get(value..)?)?
    } else {
        let start = value + 4;
        data.get(start..start.checked_add(size as usize)?)?.to_vec()
    };
    Some((handle, body))
}

/// The data a `_data_` record keeps in `blob01` pages, from its blob
/// reference: u32 marker, u64 total data size, u32 page count, u32 size of
/// the reference after the marker (`32 + 8 * page_count`), u32 page size,
/// u32 last page size, two u32 unknown, then per page, in order, u32 segment
/// index and u32 size. Every page must be a `blob01` segment whose frame
/// agrees: the same total and page count, its own start offset, index and
/// size. The sizes add up to the total.
fn acds_blob_pages(segments: &AcdsSegments, reference: &[u8]) -> Option<Vec<u8>> {
    let total = le_u64(reference, 4)?;
    let count = le_u32(reference, 12)? as usize;
    if count == 0 || le_u32(reference, 16)? as usize != 32 + 8 * count {
        return None;
    }
    if total > segments.buf.len() as u64 {
        return None;
    }
    let mut body = Vec::with_capacity(total as usize);
    for index in 0..count {
        let id = le_u32(reference, 36 + 8 * index)?;
        let size = le_u32(reference, 40 + 8 * index)? as usize;
        let page = segments.segment(id, b"blob01")?;
        let agrees = le_u32(page, 12) == Some(1)
            && le_u64(page, 48) == Some(total)
            && le_u64(page, 56) == Some(body.len() as u64)
            && le_u32(page, 64) == Some(index as u32)
            && le_u32(page, 68) == Some(count as u32)
            && le_u64(page, 72) == Some(size as u64);
        if !agrees {
            return None;
        }
        body.extend_from_slice(page.get(BLOB01_FRAME_LEN..BLOB01_FRAME_LEN.checked_add(size)?)?);
    }
    (body.len() as u64 == total).then_some(body)
}

/// Fill an `AcisData` from a SAB blob and mark it binary v2.
fn acds_fill(acis: &mut crate::entities::solid3d::AcisData, blob: Vec<u8>) {
    acis.sab_data = blob;
    acis.sat_data = String::new();
    acis.is_binary = true;
    acis.version = crate::entities::solid3d::AcisVersion::Version2;
}

/// Attach the AcDs modeler bodies (from [`acds_modeler_bodies`]) to their
/// owning modeler entities, each body to the entity its record names.
/// Returns the number attached.
fn attach_acds_record_blobs(
    document: &mut crate::document::CadDocument,
    record_blobs: Vec<(u64, Vec<u8>)>,
    defer: bool,
) -> usize {
    // Finding a body's reference point parses the whole SAB,
    // and the blobs are independent: do that for all of them in parallel,
    // then hand the results to the document in record order.
    let prepared = super::parallel::map_ordered(record_blobs, |(h, blob)| {
        let mut acis = crate::entities::solid3d::AcisData::default();
        acds_fill(&mut acis, blob);
        // Deferred, the point is derived on request instead, so the SAB is
        // not parsed here.
        let centre = if defer {
            None
        } else {
            acis.geometry_centre().or_else(|| acis.placement_origin())
        };
        (h, acis, centre)
    });
    let mut attached = 0usize;
    for (h, acis, centre) in prepared {
        if let Some(entity) = document.get_entity_mut(crate::Handle::new(h)) {
            if acds_apply_prepared(entity, acis, centre, defer) {
                attached += 1;
            }
        }
    }
    attached
}

/// Apply a body already filled into `acis`, with its reference point already
/// found, so the expensive part can run off the document. A modeler entity
/// without a stored anchor takes the body's reference point, or with `defer`
/// is flagged to derive it on request. Returns false for other entities.
fn acds_apply_prepared(
    entity: &mut crate::entities::EntityType,
    acis: crate::entities::solid3d::AcisData,
    centre: Option<crate::types::Vector3>,
    defer: bool,
) -> bool {
    use crate::entities::EntityType;
    let (data, point) = match entity {
        EntityType::Solid3D(s) => (&mut s.acis_data, Some(&mut s.point_of_reference)),
        EntityType::Region(r) => (&mut r.acis_data, Some(&mut r.point_of_reference)),
        EntityType::Body(b) => (&mut b.acis_data, Some(&mut b.point_of_reference)),
        EntityType::Surface(s) => (&mut s.acis_data, None),
        _ => return false,
    };
    data.sab_data = acis.sab_data;
    data.sat_data = acis.sat_data;
    data.is_binary = acis.is_binary;
    data.version = acis.version;
    if let Some(point) = point {
        if *point == crate::types::Vector3::ZERO {
            if defer {
                data.reference_point_deferred = true;
            } else if let Some(centre) = centre {
                *point = centre;
            }
        }
    }
    true
}

/// Decode a section name from a fixed 64-byte, null-terminated field.
///
/// The name ends at the first null byte. Some writers leave non-zero garbage
/// in the bytes *after* the terminator instead of zero-padding the field, so
/// trimming trailing nulls is not enough — it would keep the embedded null and
/// the junk that follows (e.g. `"AcDb:Handles\0t…"`), and the name would then
/// fail to match when looking the section up.
fn section_name_from_field(name_buf: &[u8; 64]) -> String {
    let end = name_buf
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(name_buf.len());
    String::from_utf8_lossy(&name_buf[..end]).into_owned()
}

/// DWG file reader with CRC-64 extraction support.
///
/// Reads DWG binary files and provides access to all internal
/// integrity checksums including the AC1021 Header CRC-64.
/// Read one `T16` string (2-byte character-count prefix). R2007+ stores the
/// characters as UTF-16LE; earlier versions as one byte per character.
fn read_t16(
    cur: &mut &[u8],
    utf16: bool,
    encoding: crate::io::dxf::code_page::LegacyEncoding,
) -> String {
    if cur.len() < 2 {
        *cur = &[];
        return String::new();
    }
    let count = u16::from_le_bytes([cur[0], cur[1]]) as usize;
    *cur = &cur[2..];
    if utf16 {
        let bytes = count.saturating_mul(2).min(cur.len());
        let units: Vec<u16> = cur[..bytes]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        *cur = &cur[bytes..];
        String::from_utf16_lossy(&units)
            .trim_end_matches('\0')
            .to_string()
    } else {
        // Pre-R2007 strings are in the drawing code page
        let bytes = count.min(cur.len());
        let (s, _) = encoding.decode_without_bom_handling(&cur[..bytes]);
        *cur = &cur[bytes..];
        crate::io::dxf::code_page::decode_legacy_escapes_in(s.trim_end_matches('\0'), encoding)
    }
}

fn decode_field_xdata(document: &mut crate::document::CadDocument) {
    let wide = document.version >= crate::types::DxfVersion::AC1021;
    let fields: Vec<_> = document
        .eed_by_handle
        .keys()
        .copied()
        .filter(|h| {
            matches!(
                document.objects.get(h),
                Some(crate::objects::ObjectType::Field(_))
            )
        })
        .collect();
    for h in fields {
        let Some(blocks) = document.eed_by_handle.remove(&h) else {
            continue;
        };
        let mut kept = Vec::new();
        let mut xdata = crate::xdata::ExtendedData::new();
        for (app, bytes) in blocks {
            let name = document
                .app_ids
                .iter()
                .find(|a| a.handle.value() == app)
                .map(|a| a.name.clone());
            match (
                name,
                crate::io::dwg::eed_codec::decode_values(&bytes, wide, |_| None),
            ) {
                (Some(name), Some(values)) => {
                    let mut rec = crate::xdata::ExtendedDataRecord::new(name);
                    rec.values = values;
                    xdata.add_record(rec);
                }
                _ => kept.push((app, bytes)),
            }
        }
        if !kept.is_empty() {
            document.eed_by_handle.insert(h, kept);
        }
        if let Some(crate::objects::ObjectType::Field(f)) = document.objects.get_mut(&h) {
            f.xdata = xdata;
        }
    }
}

fn decode_block_units_xdata(document: &mut crate::document::CadDocument) {
    if document.version >= crate::types::DxfVersion::AC1021 {
        return;
    }
    let Some(acad) = document.app_ids.get("ACAD").map(|app| app.handle.value()) else {
        return;
    };
    for record in document.block_records.iter_mut() {
        let Some(blocks) = document.eed_by_handle.get_mut(&record.handle) else {
            continue;
        };
        blocks.retain(|(app, bytes)| {
            let units = (*app == acad)
                .then(|| crate::io::dwg::eed_codec::decode_values(bytes, false, |_| None))
                .flatten()
                .and_then(|values| crate::tables::block_record::design_center_units(&values));
            match units {
                Some(units) => {
                    if record.units == 0 {
                        record.units = units;
                    }
                    false
                }
                None => true,
            }
        });
        if blocks.is_empty() {
            document.eed_by_handle.remove(&record.handle);
        }
    }
}

/// Parse the decompressed `AcDb:SummaryInfo` section (R2004+): eight fixed
fn parse_summary_info(
    buf: &[u8],
    utf16: bool,
    encoding: crate::io::dxf::code_page::LegacyEncoding,
) -> crate::document::SummaryInfo {
    let mut cur: &[u8] = buf;
    let mut si = crate::document::SummaryInfo {
        title: read_t16(&mut cur, utf16, encoding),
        subject: read_t16(&mut cur, utf16, encoding),
        author: read_t16(&mut cur, utf16, encoding),
        keywords: read_t16(&mut cur, utf16, encoding),
        comments: read_t16(&mut cur, utf16, encoding),
        last_saved_by: read_t16(&mut cur, utf16, encoding),
        revision_number: read_t16(&mut cur, utf16, encoding),
        hyperlink_base: read_t16(&mut cur, utf16, encoding),
        custom_properties: Vec::new(),
    };
    // TDINDWG, TDCREATE, TDUPDATE — each a TIMERLL (2×u32 = 8 bytes).
    if cur.len() >= 24 {
        cur = &cur[24..];
    }
    if cur.len() >= 2 {
        let n = u16::from_le_bytes([cur[0], cur[1]]) as usize;
        cur = &cur[2..];
        for _ in 0..n.min(256) {
            let tag = read_t16(&mut cur, utf16, encoding);
            let val = read_t16(&mut cur, utf16, encoding);
            if tag.is_empty() && val.is_empty() {
                break;
            }
            si.custom_properties.push((tag, val));
        }
    }
    si
}

pub struct DwgReader<R: Read + Seek> {
    stream: R,
    /// Options controlling read behaviour.
    pub options: DwgReadOptions,
    /// Notifications collected during reading
    pub notifications: NotificationCollection,
    /// Source path, when opened from a file — copied onto the document so the
    /// `Filename` / `FilePath` fields can resolve.
    source_path: Option<String>,
    /// Optional monotonic read progress callback. The value is in 0..=1000 so
    /// callers can map DWG parsing into a larger file-open pipeline without
    /// coupling this crate to a UI or an async runtime.
    progress: Option<std::sync::Arc<dyn Fn(u16) + Send + Sync>>,
    legacy_font_resolver: Option<std::sync::Arc<crate::io::dwg::r12::ImportFontResolver>>,
    r12_code_page: Option<String>,
    /// Section-level diagnostics collected by `get_section_buffer`.
    ///
    /// Section extraction is also a public API, so it cannot take the
    /// read_with_stats diagnostic accumulator as an argument. Keep these
    /// diagnostics on the reader and drain them when assembling ReadStats.
    pending_diagnostics: Vec<ReadDiagnostic>,
    /// Decompressed bytes already charged against DwgReadOptions::max_decompressed_bytes in
    /// the current read.
    decompressed_bytes_used: usize,
    /// State of a [`Self::read_sharded`] call between shards.
    shard: Option<ShardState>,
    /// Derived decryption key for password-protected drawings.
    pub decryption_key: Option<Vec<u8>>,
}

impl DwgReader<File> {
    /// Open a DWG file from a filesystem path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, DxfError> {
        Self::from_file_with_options(path, DwgReadOptions::default())
    }

    /// Open a DWG file from a filesystem path with custom options.
    pub fn from_file_with_options<P: AsRef<Path>>(
        path: P,
        options: DwgReadOptions,
    ) -> Result<Self, DxfError> {
        let path = path.as_ref();
        let file = File::open(path)?;
        Ok(Self {
            stream: file,
            options,
            notifications: NotificationCollection::new(),
            source_path: Some(path.to_string_lossy().into_owned()),
            progress: None,
            legacy_font_resolver: None,
            r12_code_page: None,
            pending_diagnostics: Vec::new(),
            decompressed_bytes_used: 0,
            shard: None,
            decryption_key: None,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl DwgReader<Cursor<memmap2::Mmap>> {
    /// Open through a read-only memory map. Section seeks then become pointer
    /// moves and the OS can page/readahead the file without repeated syscalls.
    pub fn from_mmap<P: AsRef<Path>>(path: P) -> Result<Self, DxfError> {
        let path = path.as_ref();
        let file = File::open(path)?;
        // SAFETY: read-only map owns its file-backed virtual-memory mapping;
        // this reader never mutates the file while the mapping is alive.
        let map = unsafe { memmap2::MmapOptions::new().map(&file)? };
        Ok(Self {
            stream: Cursor::new(map),
            options: DwgReadOptions::default(),
            notifications: NotificationCollection::new(),
            source_path: Some(path.to_string_lossy().into_owned()),
            progress: None,
            legacy_font_resolver: None,
            r12_code_page: None,
            pending_diagnostics: Vec::new(),
            decompressed_bytes_used: 0,
            shard: None,
            decryption_key: None,
        })
    }
}

impl<R: Read + Seek> DwgReader<R> {
    /// Override the code page used to decode AC1009 strings.
    ///
    /// By default, the file's declared code page is honored. Some legacy
    /// importers use an application encoding instead. This explicit override
    /// applies to header strings, tables, and entities, while preserving the
    /// declared `document.header.code_page`. Later DWG versions are unaffected.
    /// No system locale or application configuration is queried by the reader.
    pub fn with_r12_code_page(mut self, code_page: impl Into<String>) -> Result<Self, DxfError> {
        let code_page = code_page.into();
        if crate::io::dxf::code_page::try_encoding_from_code_page(&code_page).is_none() {
            return Err(DxfError::InvalidFormat(format!(
                "Unsupported AC1009 code page override: {code_page}"
            )));
        }
        self.r12_code_page = Some(code_page);
        Ok(self)
    }

    /// Resolve legacy font files to canonical identities during R12 import.
    ///
    /// Missing R12 object handles are synthesized in import order. Loading a
    /// text font consumes a transient identity once per resolved font; aliases
    /// and unavailable fonts substituted with the same fallback share that
    /// identity. Supply the writer's font resolution when matching identities
    /// across a native reader and another application's writer.
    ///
    /// The callback receives the stored font filename and returns a stable key
    /// for the resolved font, including any fallback. No font files are read by
    /// acadrust. Without this callback, stored font names identify distinct fonts.
    /// Existing entity handles are preserved in either mode.
    pub fn with_legacy_font_resolver<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        self.legacy_font_resolver = Some(std::sync::Arc::new(resolver));
        self
    }

    /// Create a reader from any seekable stream.
    pub fn from_stream(stream: R) -> Self {
        Self {
            stream,
            options: DwgReadOptions::default(),
            notifications: NotificationCollection::new(),
            source_path: None,
            progress: None,
            legacy_font_resolver: None,
            r12_code_page: None,
            pending_diagnostics: Vec::new(),
            decompressed_bytes_used: 0,
            shard: None,
            decryption_key: None,
        }
    }

    /// Create a reader from any seekable stream with custom options.
    pub fn from_stream_with_options(stream: R, options: DwgReadOptions) -> Self {
        Self {
            stream,
            options,
            notifications: NotificationCollection::new(),
            source_path: None,
            progress: None,
            legacy_font_resolver: None,
            r12_code_page: None,
            pending_diagnostics: Vec::new(),
            decompressed_bytes_used: 0,
            shard: None,
            decryption_key: None,
        }
    }

    /// Receive monotonic parsing progress in the inclusive range 0..=1000.
    ///
    /// The callback may run from Rayon worker threads while object records are
    /// decoded, so it must be thread-safe and should only perform cheap atomic
    /// updates.
    /// Set a password for decrypting password-protected DWG files.
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.options.password = Some(password.into());
        self
    }

    pub fn set_progress_callback(&mut self, progress: std::sync::Arc<dyn Fn(u16) + Send + Sync>) {
        self.progress = Some(progress);
    }

    #[inline]
    fn report_progress(&self, value: u16) {
        if let Some(progress) = &self.progress {
            progress(value.min(1000));
        }
    }

    /// Ensure the decryption key is derived and verified if the file is password-protected.
    pub fn ensure_decryption_key(&mut self, info: &DwgFileHeaderInfo) -> Result<(), DxfError> {
        if self.decryption_key.is_some() {
            return Ok(());
        }
        if !info
            .section_descriptors
            .iter()
            .any(|s| s.name == "AcDb:Security")
        {
            return Ok(());
        }
        let sec_data = self.get_section_buffer("AcDb:Security", info)?;
        let sec_info = crate::io::dwg::security::DwgSecurityInfo::parse(&sec_data)?;
        let Some(pwd) = &self.options.password else {
            return Err(DxfError::Decryption(
                "Drawing is password protected; please provide a password".to_string(),
            ));
        };
        let key = sec_info.derive_key(pwd)?;
        self.decryption_key = Some(key);
        Ok(())
    }

    /// Read the DWG file and reconstruct a `CadDocument`.
    ///
    /// This is the main entry point for reading a DWG file into a usable
    /// document. It orchestrates:
    /// 1. File header parsing
    /// 2. Section buffer extraction (Classes, Header, Handles, Objects)
    /// 3. Classes, header variables, and handle map parsing
    /// 4. Object dispatch and entity/object mapping via `DwgDocumentBuilder`
    ///
    /// In failsafe mode, file-header errors are caught and a partial
    /// document is returned instead of propagating the error.
    pub fn read(&mut self) -> std::result::Result<crate::document::CadDocument, DxfError> {
        self.read_with_stats().map(|outcome| outcome.document)
    }

    /// Tell the caller that a page's stored checksum does not match the
    /// bytes read: the data was damaged after it was written.
    fn check_page_checksum(
        &mut self,
        section: &str,
        source_offset: Option<u64>,
        stored: u32,
        computed: u32,
    ) {
        if stored == computed {
            return;
        }
        let message = format!(
            "{section}: page checksum mismatch at offset {:#X} (stored {stored:#010X}, computed {computed:#010X})",
            source_offset.unwrap_or(0)
        );
        self.notifications
            .notify(NotificationType::Warning, message.clone());
        self.report_section_diagnostic("checksum-mismatch", section, source_offset, message);
    }

    fn report_section_diagnostic(
        &mut self,
        code: &str,
        section: &str,
        source_offset: Option<u64>,
        message: impl Into<String>,
    ) {
        let mut diagnostic = ReadDiagnostic::new(code, ReadStage::Section, message);
        diagnostic.section = Some(section.to_owned());
        diagnostic.source_offset = source_offset;
        diagnostic.source_offset_basis = Some("file-byte".to_owned());
        push_read_diagnostic(&mut self.pending_diagnostics, diagnostic);
    }

    /// Charge decompressed bytes against the per-read budget before allocating them.
    fn charge_decompressed_bytes(
        &mut self,
        bytes: usize,
        what: impl std::fmt::Display,
    ) -> Result<(), DxfError> {
        let Some(limit) = self.options.max_decompressed_bytes else {
            return Ok(());
        };
        let used = self.decompressed_bytes_used;
        let remaining = limit.saturating_sub(used);
        if bytes > remaining {
            return Err(DxfError::InvalidFormat(format!(
                "{what} needs {bytes} decompressed bytes but only {remaining} of the {limit}-byte budget remains; raise or remove DwgReadOptions.max_decompressed_bytes"
            )));
        }
        self.decompressed_bytes_used = used + bytes;
        Ok(())
    }

    /// Recover as much usable drawing data as possible and apply conservative
    /// document repairs. Recovery always enables failsafe parsing.
    pub fn recover(&mut self) -> std::result::Result<crate::io::recover::RecoverOutcome, DxfError> {
        self.options.failsafe = true;
        crate::io::recover::RecoverOutcome::from_read_outcome(self.read_with_stats()?)
    }

    /// Like [`read`], but each pass-2 entity is offered to `visit` before it
    /// is stored on the document. Return `None` to keep it out of
    /// `CadDocument::entities` (the visitor may consume the value).
    pub fn read_visiting<F>(
        &mut self,
        mut visit: F,
    ) -> std::result::Result<crate::document::CadDocument, DxfError>
    where
        F: FnMut(
            &crate::document::CadDocument,
            crate::entities::EntityType,
        ) -> Option<crate::entities::EntityType>,
    {
        self.read_with_optional_visitor(Some(&mut visit))
            .map(|outcome| outcome.document)
    }

    /// Read the file and return the document with source/decode statistics.
    pub fn read_with_stats(
        &mut self,
    ) -> std::result::Result<crate::io::read::ReadOutcome, DxfError> {
        self.read_with_optional_visitor(
            None::<
                &mut dyn FnMut(
                    &crate::document::CadDocument,
                    crate::entities::EntityType,
                ) -> Option<crate::entities::EntityType>,
            >,
        )
    }

    /// Read the file in shards of at most `max_entities` model-space entity
    /// records each, passing each shard's document to `each` before the next
    /// shard is decoded, so only one shard's entities are held at a time.
    ///
    /// Every shard is a complete document with the header, classes, tables,
    /// block records and non-entity objects of the whole file. Model-space
    /// entities are divided between the shards in the order of the model
    /// space entity list, and each stays in the same shard as its child
    /// records (attributes, vertices, faces and sequence end); a shard holds
    /// more than `max_entities` records only when one entity's children
    /// exceed the limit. Block contents and paper-space entities are read
    /// with the first shard.
    ///
    /// The handle map and the decompressed object section are read once and
    /// shared by every shard. A drawing whose model space fits in one shard,
    /// and a drawing older than R2004, is read as one shard, exactly as
    /// [`Self::read_with_stats`] reads it.
    ///
    /// Returns the number of shards read. An error from `each` stops the read
    /// and is returned.
    ///
    /// ```rust,no_run
    /// use acadrust::DwgReader;
    ///
    /// # fn main() -> acadrust::Result<()> {
    /// let mut reader = DwgReader::from_file("large.dwg")?;
    /// let mut entities = 0;
    /// let shards = reader.read_sharded(10_000, |shard| {
    ///     entities += shard.document.model_space_entities().count();
    ///     Ok(())
    /// })?;
    /// println!("{entities} model-space entities in {shards} shards");
    /// # Ok(())
    /// # }
    /// ```
    pub fn read_sharded<F>(
        &mut self,
        max_entities: usize,
        mut each: F,
    ) -> std::result::Result<usize, DxfError>
    where
        F: FnMut(crate::io::read::ReadOutcome) -> std::result::Result<(), DxfError>,
    {
        self.shard = Some(ShardState::new(max_entities));
        let mut shards = 0;
        let result = loop {
            if let Err(error) = self.read_with_stats().and_then(&mut each) {
                break Err(error);
            }
            shards += 1;
            match self.shard.as_mut() {
                Some(shard) if shard.index + 1 < shard.count() => shard.index += 1,
                _ => break Ok(shards),
            }
        };
        self.shard = None;
        result
    }

    fn read_with_optional_visitor(
        &mut self,
        visit: Option<
            &mut dyn FnMut(
                &crate::document::CadDocument,
                crate::entities::EntityType,
            ) -> Option<crate::entities::EntityType>,
        >,
    ) -> std::result::Result<crate::io::read::ReadOutcome, DxfError> {
        // A decoder bug must reach the caller as `DxfError`, including when
        // failsafe is on. Record-level failures are handled inside the builder.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.read_drawing(visit))) {
            Ok(result) => result.map(|mut outcome| {
                current_style_names(&mut outcome.document);
                outcome
            }),
            Err(payload) => Err(DxfError::Parse(format!(
                "DWG reader failed: {}",
                crate::io::dwg::unwind_message(payload.as_ref())
            ))),
        }
    }

    fn read_drawing(
        &mut self,
        visit: Option<
            &mut dyn FnMut(
                &crate::document::CadDocument,
                crate::entities::EntityType,
            ) -> Option<crate::entities::EntityType>,
        >,
    ) -> std::result::Result<crate::io::read::ReadOutcome, DxfError> {
        let failsafe = self.options.failsafe;
        let perf = std::env::var_os("PERF").is_some();
        let total_started = web_time::Instant::now();
        let mut diagnostics = Vec::new();
        self.decompressed_bytes_used = 0;
        self.report_progress(0);

        // AC1009/R12 predates the section-map/object-stream model used by the
        // normal DWG reader. Dispatch it to the byte-aligned pre-R13 codec
        // before attempting the R13+ file-header parser. AC1006/R10 shares
        // that layout and is read into an R11/R12 document.
        self.stream.seek(SeekFrom::Start(0))?;
        let mut magic = [0u8; 6];
        self.stream.read_exact(&mut magic)?;
        self.stream.seek(SeekFrom::Start(0))?;
        if magic == *b"AC1009" || magic == *b"AD1009" || magic == *b"AC1006" {
            let mut bytes = Vec::new();
            self.stream.read_to_end(&mut bytes)?;
            let mut document = match crate::io::dwg::r12::read_document(
                &bytes,
                self.legacy_font_resolver.as_deref(),
                self.r12_code_page.as_deref(),
            ) {
                Ok(document) => document,
                Err(error) if failsafe => {
                    report_read_error(
                        &mut self.notifications,
                        &mut diagnostics,
                        "legacy-read-failed",
                        ReadStage::RecordStream,
                        None,
                        format!("Failsafe: legacy AC1009 read failed: {}", error),
                    );
                    let mut document = crate::document::CadDocument::default();
                    document
                        .notifications
                        .extend(std::mem::take(&mut self.notifications));
                    let stats = crate::io::read::ReadStats::from_document(
                        &document,
                        SourceFormat::Dwg,
                        1,
                        0,
                        0,
                        1,
                        // The AC1009 read failed as a whole: nothing of
                        // its record stream reached the document.
                        false,
                        true,
                        false,
                        diagnostics,
                    );
                    return Ok(crate::io::read::ReadOutcome::new(document, stats));
                }
                Err(error) => return Err(error),
            };
            if let Some(visit) = visit {
                let handles: Vec<_> = document
                    .entities()
                    .map(|entity| entity.common().handle)
                    .collect();
                for handle in handles {
                    let Some(entity) = document.get_entity(handle).cloned() else {
                        continue;
                    };
                    let replacement = visit(&document, entity);
                    document.remove_entity(handle);
                    if let Some(entity) = replacement {
                        document.add_entity(entity)?;
                    }
                }
            }
            self.report_progress(1000);
            crate::document::sync_dimension_arrow_header(&mut document);
            let stats = crate::io::read::ReadStats::from_document(
                &document,
                SourceFormat::Dwg,
                1,
                document.entities().count(),
                document.entities().count(),
                0,
                true,
                false,
                true,
                diagnostics,
            );
            return Ok(crate::io::read::ReadOutcome::new(document, stats));
        }

        // 1. Read the DWG file header and section map
        let stage_started = web_time::Instant::now();
        let mut info = match self.read_file_header() {
            Ok(info) => info,
            Err(e) if failsafe => {
                report_read_error(
                    &mut self.notifications,
                    &mut diagnostics,
                    "file-header-read-failed",
                    ReadStage::FileHeader,
                    None,
                    format!(
                        "Failsafe: file header read failed, returning partial document: {}",
                        e
                    ),
                );
                let mut doc = crate::document::CadDocument::default();
                doc.notifications
                    .extend(std::mem::take(&mut self.notifications));
                let stats = crate::io::read::ReadStats::from_document(
                    &doc,
                    SourceFormat::Dwg,
                    0,
                    0,
                    0,
                    0,
                    false,
                    true,
                    false,
                    diagnostics,
                );
                return Ok(crate::io::read::ReadOutcome::new(doc, stats));
            }
            Err(e) => return Err(e),
        };
        if perf {
            eprintln!(
                "[perf] dwg-read header={:.1}ms sections={} pages={}",
                stage_started.elapsed().as_secs_f64() * 1000.0,
                info.section_descriptors.len(),
                info.page_records.len(),
            );
        }
        if info
            .section_descriptors
            .iter()
            .any(|s| s.name == "AcDb:Security")
        {
            if let Err(e) = self.ensure_decryption_key(&info) {
                if !self.options.failsafe {
                    return Err(e);
                }
                report_read_error(
                    &mut self.notifications,
                    &mut diagnostics,
                    "decryption-failed",
                    ReadStage::FileHeader,
                    Some("AcDb:Security"),
                    format!("Failed to decrypt drawing: {}", e),
                );
            }
        }

        let dxf_version = crate::types::DxfVersion::parse(&info.version_string)
            .unwrap_or(crate::types::DxfVersion::Unknown);
        self.report_progress(20);
        let mut document = crate::document::CadDocument::with_version(dxf_version);
        document.maintenance_version = info.acad_maintenance_version;
        document.dwg_source_version = Some(dxf_version);

        // 2. Read Classes (AcDb:Classes)
        match self.get_section_buffer("AcDb:Classes", &info) {
            Ok(classes_buf) => {
                match crate::io::dwg::dwg_stream_readers::classes_reader::read_classes_with_encoding(
                    &classes_buf,
                    dxf_version,
                    info.acad_maintenance_version,
                    crate::io::dxf::code_page::encoding_from_dwg_code_page(info.code_page),
                ) {
                    Ok(classes) => document.classes = classes,
                    Err(e) => report_read_error(
                        &mut self.notifications,
                        &mut diagnostics,
                        "classes-decode-failed",
                        ReadStage::Classes,
                        Some("AcDb:Classes"),
                        format!("Failed to read classes: {}", e),
                    ),
                }
            }
            Err(e) => report_read_error(
                &mut self.notifications,
                &mut diagnostics,
                "classes-extract-failed",
                ReadStage::Classes,
                Some("AcDb:Classes"),
                format!("Failed to extract classes section: {}", e),
            ),
        }
        self.report_progress(30);

        // 3. Read Header Variables (AcDb:Header)
        match self.get_section_buffer("AcDb:Header", &info) {
            Ok(header_buf) => {
                match crate::io::dwg::dwg_stream_readers::header_reader::read_header_with_encoding(
                    &header_buf,
                    dxf_version,
                    info.acad_maintenance_version,
                    crate::io::dxf::code_page::encoding_from_dwg_code_page(info.code_page),
                ) {
                    Ok(header_vars) => document.header = header_vars,
                    Err(e) if !failsafe => return Err(e),
                    Err(e) => report_read_error(
                        &mut self.notifications,
                        &mut diagnostics,
                        "header-decode-failed",
                        ReadStage::Header,
                        Some("AcDb:Header"),
                        format!("Failed to read header: {}", e),
                    ),
                }
            }
            Err(e) => report_read_error(
                &mut self.notifications,
                &mut diagnostics,
                "header-extract-failed",
                ReadStage::Header,
                Some("AcDb:Header"),
                format!("Failed to extract header section: {}", e),
            ),
        }
        document.header.code_page =
            crate::io::dxf::code_page::dwg_code_page_name(info.code_page).to_string();
        if !crate::io::dxf::code_page::dwg_code_page_is_known(info.code_page) {
            self.notifications.notify(
                NotificationType::Warning,
                format!(
                    "Unknown DWG code page index {}; falling back to ANSI_1252 for text",
                    info.code_page
                ),
            );
        }

        // MEASUREMENT is stored in the optional Template section, independently
        // of insertion units. Older files may omit the section entirely.
        if let Ok(template_buf) = self.get_section_buffer("AcDb:Template", &info) {
            match parse_template(&template_buf, dxf_version) {
                Ok(measurement) => document.header.measurement = measurement,
                Err(error) if failsafe => report_read_error(
                    &mut self.notifications,
                    &mut diagnostics,
                    "template-decode-failed",
                    ReadStage::Header,
                    Some("AcDb:Template"),
                    error.to_string(),
                ),
                Err(error) => return Err(error),
            }
        }
        let empty_sparse_dependencies = info.is_ac18_format
            && info.section_descriptors.iter().any(|s| {
                s.name == "AcDb:FileDepList"
                    && s.compressed_size == 8
                    && s.page_count == 0
                    && s.pages.is_empty()
            });
        if !empty_sparse_dependencies
            && info
                .section_descriptors
                .iter()
                .any(|s| s.name == "AcDb:FileDepList")
        {
            // The section map marks this section 2 ("unknown") where other
            // sections hold 0; without a password its pages are plain.
            if self.decryption_key.is_none() {
                for s in &mut info.section_descriptors {
                    if s.name == "AcDb:FileDepList" && s.encrypted == 2 {
                        s.encrypted = 0;
                    }
                }
            }
            let dependencies =
                self.get_section_buffer("AcDb:FileDepList", &info)
                    .and_then(|bytes| {
                        super::file_dependencies::read(
                            &bytes,
                            dxf_version,
                            crate::io::dxf::code_page::encoding_from_dwg_code_page(info.code_page),
                        )
                    });
            match dependencies {
                Ok(files) => document.file_dependencies = files,
                Err(error) if failsafe => report_read_error(
                    &mut self.notifications,
                    &mut diagnostics,
                    "file-dependencies-decode-failed",
                    ReadStage::Header,
                    Some("AcDb:FileDepList"),
                    error.to_string(),
                ),
                Err(error) => return Err(error),
            }
        }
        self.report_progress(40);

        // 4. Read Handle Map (AcDb:Handles). The later shards of a sharded
        //    read reuse the handles and object section the first one read.
        let resumed = self.shard.as_mut().and_then(ShardState::resume);
        let mut handles_section_read = resumed.is_some();
        let (cached_objects, handle_count, handle_map) = match resumed {
            Some((objects, handle_count, handle_map)) => (Some(objects), handle_count, handle_map),
            None => {
                let handle_map = match self.get_section_buffer("AcDb:Handles", &info) {
                    Ok(handle_buf) => {
                        handles_section_read = true;
                        match crate::io::dwg::dwg_stream_readers::handle_reader::read_handle_map(
                            &handle_buf,
                        ) {
                            Ok(mut hm) => {
                                // AC15: Handle offsets are absolute file positions.
                                // Convert to buffer-relative by subtracting the objects
                                // section base offset.
                                if info.objects_base_offset != 0 {
                                    let base = info.objects_base_offset;
                                    for offset in hm.values_mut() {
                                        *offset -= base;
                                    }
                                }
                                hm
                            }
                            Err(e) => {
                                report_read_error(
                                    &mut self.notifications,
                                    &mut diagnostics,
                                    "handles-decode-failed",
                                    ReadStage::Handles,
                                    Some("AcDb:Handles"),
                                    format!("Failed to read handles: {}", e),
                                );
                                Default::default()
                            }
                        }
                    }
                    Err(e) => {
                        report_read_error(
                            &mut self.notifications,
                            &mut diagnostics,
                            "handles-extract-failed",
                            ReadStage::Handles,
                            Some("AcDb:Handles"),
                            format!("Failed to extract handles section: {}", e),
                        );
                        Default::default()
                    }
                };
                (None, handle_map.len(), handle_map)
            }
        };
        self.report_progress(50);

        // 5. Read Objects (AcDb:AcDbObjects) and build document
        let mut objects_section_read = false;
        let mut record_stream_read = false;
        let mut decoded_source_records = 0usize;
        let mut skipped_source_records = 0usize;
        let objects_started = web_time::Instant::now();
        if !handle_map.is_empty() {
            let objects_buf = match cached_objects {
                Some(objects) => Ok(objects),
                None => self
                    .get_section_buffer("AcDb:AcDbObjects", &info)
                    .map(std::sync::Arc::new),
            };
            match objects_buf {
                Ok(objects_buf) => {
                    objects_section_read = true;
                    match crate::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader::with_handle_map(
                    std::sync::Arc::clone(&objects_buf),
                    dxf_version,
                    handle_map,
                    crate::io::dxf::code_page::encoding_from_dwg_code_page(info.code_page),
                ) {
                    Ok(mut obj_reader) => {
                        if let Some(shard) =
                            self.shard.as_mut().filter(|shard| shard.objects.is_none())
                        {
                            shard.plan_first(&mut obj_reader, objects_buf, handle_count, &document);
                        } else {
                            // The object reader holds the only other copy and
                            // frees it once every record is decoded.
                            drop(objects_buf);
                        }
                        record_stream_read = true;
                        obj_reader.set_mesh_budget(self.options.mesh_budget());
                        let mut builder = crate::io::dwg::dwg_document_builder::DwgDocumentBuilder::new(obj_reader);
                        builder.set_failsafe(failsafe);
                        builder.set_skip_roundtrip_data(self.options.skip_roundtrip_data);
                        if let Some(progress) = &self.progress {
                            builder.set_progress_callback(progress.clone());
                        }
                        let build_outcome = match visit {
                            Some(visit) => builder.build_with_visitor_stats(&mut document, visit),
                            None => builder.build_with_stats(&mut document),
                        };
                        decoded_source_records = build_outcome.decoded_records;
                        skipped_source_records = build_outcome.skipped_records;
                        if !failsafe && skipped_source_records != 0 {
                            return Err(DxfError::Parse(build_outcome.diagnostics.iter()
                                .find(|diagnostic| diagnostic.code != "auxiliary-object-preserved")
                                .map(|diagnostic| diagnostic.message.clone())
                                .unwrap_or_else(|| "Invalid DWG object records".to_string())));
                        }
                        diagnostics.extend(build_outcome.diagnostics);
                        self.notifications.extend(build_outcome.notifications);
                    },
                    Err(e) => report_read_error(
                        &mut self.notifications,
                        &mut diagnostics,
                        "objects-init-failed",
                        ReadStage::Objects,
                        Some("AcDb:AcDbObjects"),
                        format!("Failed to init object reader: {}", e),
                    ),
                    }
                }
                Err(e) => report_read_error(
                    &mut self.notifications,
                    &mut diagnostics,
                    "objects-extract-failed",
                    ReadStage::Objects,
                    Some("AcDb:AcDbObjects"),
                    format!("Failed to extract objects section: {}", e),
                ),
            }
        }
        self.report_progress(930);
        if perf {
            eprintln!(
                "[perf] dwg-read objects={:.1}ms handles={} entities={} objects={}",
                objects_started.elapsed().as_secs_f64() * 1000.0,
                handle_count,
                document.entities().count(),
                document.objects.len(),
            );
        }

        // 6. R2013+ (AC1027+): 3DSOLID / REGION / BODY ACIS geometry is not
        //    stored inline — it lives as SAB blobs in the AcDs (Autodesk Data
        //    Store) section. Extract those blobs and attach them, in document
        //    order, to the modeler-geometry entities that arrived with only a
        //    stub. Files without an AcDs section keep their inline data.
        let acds_buf = match self.get_section_buffer("AcDb:AcDsPrototype_1b", &info) {
            Ok(acds_buf) => Some(acds_buf),
            Err(error) => {
                // Many drawings have no AcDs section; one that has it but
                // cannot be read loses every SAB blob.
                if Self::find_section(&info, "AcDb:AcDsPrototype_1b").is_some() {
                    self.notifications.notify(
                        NotificationType::Warning,
                        format!(
                            "AcDs: section 'AcDb:AcDsPrototype_1b' not read, modeler entities keep no SAB data: {}",
                            error
                        ),
                    );
                }
                None
            }
        };
        if let Some(acds_buf) = acds_buf {
            // Each record names the handle of the entity that owns its body;
            // a body whose record does not read is not attached.
            let attached = attach_acds_record_blobs(
                &mut document,
                acds_modeler_bodies(&acds_buf),
                self.options.defer_solid_reference_point,
            );
            let fingerprint = super::sab_fingerprint(document.entities().filter_map(|entity| {
                let acis = match entity {
                    crate::entities::EntityType::Solid3D(entity) => &entity.acis_data,
                    crate::entities::EntityType::Region(entity) => &entity.acis_data,
                    crate::entities::EntityType::Body(entity) => &entity.acis_data,
                    crate::entities::EntityType::Surface(entity) => &entity.acis_data,
                    _ => return None,
                };
                (!acis.sab_data.is_empty())
                    .then_some((entity.common().handle, acis.sab_data.as_slice()))
            }));
            document.raw_acds_data = Some(std::sync::Arc::new(acds_buf));
            document.raw_acds_fingerprint = fingerprint;
            if attached > 0 {
                self.notifications.notify(
                    NotificationType::Warning,
                    format!(
                        "AcDs: attached {} SAB blob(s) to modeler entities",
                        attached
                    ),
                );
            }
        }
        self.report_progress(970);

        // 7. Preview / thumbnail image. Stored uncompressed at the raw file
        //    offset in the header's preview seeker (all versions), wrapped in a
        //    sentinel-bracketed container. Best-effort: a malformed or absent
        //    preview simply leaves `document.preview` as `None`.
        if info.preview_address > 0 {
            let base = info.preview_address as u64;
            if self.stream.seek(SeekFrom::Start(base)).is_ok() {
                let mut head = [0u8; 20];
                if self.stream.read_exact(&mut head).is_ok() {
                    if let Some(overall) = crate::io::dwg::preview::overall_size(&head) {
                        // Guard against a garbage size before allocating.
                        if overall > 0 && overall < 64 * 1024 * 1024 {
                            let total = crate::io::dwg::preview::container_len(overall);
                            // Best-effort: an allocation failure leaves the preview empty.
                            if let Ok(mut buf) = try_zeroed_vec(total, "DWG preview") {
                                if self.stream.seek(SeekFrom::Start(base)).is_ok()
                                    && self.stream.read_exact(&mut buf).is_ok()
                                {
                                    document.preview =
                                        crate::io::dwg::preview::parse_preview(&buf, base);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Record the source path (Filename / FilePath fields) when opened from a
        // file rather than a bare stream.
        document.source_path = self.source_path.clone();
        decode_field_xdata(&mut document);
        decode_block_units_xdata(&mut document);
        crate::objects::restore_visual_style_roundtrip(&mut document);
        crate::objects::restore_table_style_roundtrip(&mut document);

        // Document summary information (Author/Title/Subject/… → the
        // Document-category dynamic-text fields).
        if let Ok(buf) = self.get_section_buffer("AcDb:SummaryInfo", &info) {
            let utf16 = crate::io::dwg::dwg_version::DwgVersion::from_dxf_version(dxf_version)
                .map(|v| v.r2007_plus())
                .unwrap_or(true);
            let encoding = crate::io::dxf::code_page::encoding_from_dwg_code_page(info.code_page);
            document.summary_info = parse_summary_info(&buf, utf16, encoding);
        }

        // R2000/R14 down-saved gradient hatches store their gradient in the
        // ACAD round-trip mechanism, not the object stream (the DWG gradient
        // block is R2004+): the two colours in EED (GradientColor1ACI /
        // GradientColor2ACI) and the gradient type in an ACAD_XREC_ROUNDTRIP
        // XRecord under the hatch's extension dictionary. Reconstruct the
        // gradient so it renders as ACAD/ODA show it instead of a flat
        // solid fill.
        //
        // Gate strictly to pre-R2004: a native R2004+ file carries the real
        // gradient in its hatch object (read_hatch reads `is_gradient` and the
        // colours directly), so that flag is authoritative. Such hatches often
        // ALSO keep stale GradientColor1/2ACI round-trip EED from an earlier
        // edit; recovering from it would resurrect a gradient the drawing has
        // since turned off (native `is_gradient` = 0), painting a solid-fill
        // hatch as a spurious gradient. Only R2000/R14, whose object stream has
        // no gradient block, need the round-trip fallback.
        let pre_r2004 = crate::io::dwg::dwg_version::DwgVersion::from_dxf_version(dxf_version)
            .map(|v| !v.r2004_plus())
            .unwrap_or(false);
        if pre_r2004 {
            recover_roundtrip_gradients(&mut document);
            // Pre-R2004 stores an MTEXT background fill as round-trip EED
            // instead of the native codes (dimension text fills, etc.).
            recover_mtext_bg_roundtrip(&mut document);
        }

        // 7. Preview / thumbnail image. Stored uncompressed at the raw file
        //    offset in the header's preview seeker (all versions), wrapped in a
        //    sentinel-bracketed container. Best-effort: a malformed or absent
        //    preview simply leaves `document.preview` as `None`.
        if info.preview_address > 0 {
            let base = info.preview_address as u64;
            if self.stream.seek(SeekFrom::Start(base)).is_ok() {
                let mut head = [0u8; 20];
                if self.stream.read_exact(&mut head).is_ok() {
                    if let Some(overall) = crate::io::dwg::preview::overall_size(&head) {
                        // Guard against a garbage size before allocating.
                        if overall > 0 && overall < 64 * 1024 * 1024 {
                            let total = crate::io::dwg::preview::container_len(overall);
                            // Best-effort: an allocation failure leaves the preview empty.
                            if let Ok(mut buf) = try_zeroed_vec(total, "DWG preview") {
                                if self.stream.seek(SeekFrom::Start(base)).is_ok()
                                    && self.stream.read_exact(&mut buf).is_ok()
                                {
                                    document.preview =
                                        crate::io::dwg::preview::parse_preview(&buf, base);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Record the source path (Filename / FilePath fields) when opened from a
        // file rather than a bare stream.
        document.source_path = self.source_path.clone();
        decode_field_xdata(&mut document);
        decode_block_units_xdata(&mut document);
        crate::objects::restore_visual_style_roundtrip(&mut document);
        crate::objects::restore_table_style_roundtrip(&mut document);

        // Document summary information (Author/Title/Subject/… → the
        // Document-category dynamic-text fields).
        if let Ok(buf) = self.get_section_buffer("AcDb:SummaryInfo", &info) {
            let utf16 = crate::io::dwg::dwg_version::DwgVersion::from_dxf_version(dxf_version)
                .map(|v| v.r2007_plus())
                .unwrap_or(true);
            let encoding = crate::io::dxf::code_page::encoding_from_dwg_code_page(info.code_page);
            document.summary_info = parse_summary_info(&buf, utf16, encoding);
        }

        // Transfer reader notifications to the document so callers can
        // inspect them via `document.notifications`.
        document
            .notifications
            .extend(std::mem::take(&mut self.notifications));

        // Pre-R2010 formats cannot store polyline vertex ids, but constraint
        // edge subentities already reference them; rebuild the ids so the
        // references survive a re-encode to R2010+ formats.
        document.restore_constraint_vertex_ids();

        if perf {
            eprintln!(
                "[perf] dwg-read total={:.1}ms entities={} objects={} classes={}",
                total_started.elapsed().as_secs_f64() * 1000.0,
                document.entities().count(),
                document.objects.len(),
                document.classes.iter().count(),
            );
        }
        self.report_progress(1000);
        crate::document::sync_dimension_arrow_header(&mut document);
        diagnostics.extend(std::mem::take(&mut self.pending_diagnostics));
        let source_sections =
            usize::from(handles_section_read).saturating_add(usize::from(objects_section_read));
        let stats = crate::io::read::ReadStats::from_document(
            &document,
            SourceFormat::Dwg,
            source_sections,
            handle_count,
            decoded_source_records,
            skipped_source_records,
            record_stream_read,
            failsafe,
            true,
            diagnostics,
        );
        Ok(crate::io::read::ReadOutcome::new(document, stats))
    }

    /// Read the file header and extract all CRC values.
    ///
    /// For AC1021 files, this extracts:
    /// - The Header CRC-64 from the compressed metadata
    /// - Page map CRC values
    /// - Section map CRC values
    /// - Per-page CRC values
    ///
    /// # Returns
    /// A `DwgFileHeaderInfo` containing all extracted data.
    pub fn read_file_header(&mut self) -> Result<DwgFileHeaderInfo, DxfError> {
        self.decompressed_bytes_used = 0;
        self.stream.seek(SeekFrom::Start(0))?;

        // Read version string (6 bytes)
        let mut version_buf = [0u8; 6];
        self.stream.read_exact(&mut version_buf)?;
        let version_string = String::from_utf8_lossy(&version_buf).to_string();

        let version = DwgVersion::from_version_string(&version_string)
            .ok_or_else(|| DxfError::UnsupportedVersion(version_string.clone()))?;

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "Reading DWG file version: {} ({:?})",
                version_string, version
            ),
        );

        let mut info = DwgFileHeaderInfo {
            version_string,
            version,
            acad_maintenance_version: 0,
            preview_address: 0,
            dwg_version: 0,
            app_release_version: 0,
            code_page: 0,
            security_type: 0,
            summary_info_addr: 0,
            vba_project_addr: 0,
            ac21_metadata: None,
            ac21_header_crc: None,
            ac21_unknown_key: None,
            ac21_compressed_data_crc: None,
            page_records: HashMap::new(),
            section_descriptors: Vec::new(),
            section_locators: HashMap::new(),
            objects_base_offset: 0,
            is_ac18_format: false,
        };

        match version {
            DwgVersion::AC9 => {
                self.stream.seek(SeekFrom::Start(0))?;
                let mut bytes = Vec::new();
                self.stream.read_to_end(&mut bytes)?;
                let r12 = crate::io::dwg::r12::header::read_header(&bytes)?;
                info.acad_maintenance_version = r12.maintenance_version;
                info.section_locators = r12
                    .tables
                    .iter()
                    .enumerate()
                    .map(|(index, table)| {
                        (
                            crate::io::dwg::r12::header::TABLE_NAMES[index].to_owned(),
                            (
                                table.address as i64,
                                (table.record_size as u32 * table.count as u32) as i64,
                            ),
                        )
                    })
                    .collect();
                info.objects_base_offset = r12.layout.entities_start as i64;
            }
            DwgVersion::AC21 => {
                self.read_file_metadata(&mut info)?;
                self.read_file_header_ac21(&mut info)?;
            }
            DwgVersion::AC18 | DwgVersion::AC24 => {
                self.read_file_metadata(&mut info)?;
                self.read_file_header_ac18(&mut info)?;
            }
            _ => {
                // AC15 format (R13/R14/R2000) — linear file with section locator records
                self.read_file_header_ac15(&mut info)?;
            }
        }

        Ok(info)
    }

    /// Read common file metadata shared between AC18 and AC21 formats.
    ///
    /// This reads bytes 6–0xFF of the file (after the version string).
    fn read_file_metadata(&mut self, info: &mut DwgFileHeaderInfo) -> Result<(), DxfError> {
        // Skip 5 bytes after version string
        let mut skip = [0u8; 5];
        self.stream.read_exact(&mut skip)?;

        // Maintenance version (1 byte)
        info.acad_maintenance_version = self.stream.read_u8()?;

        // Skip 1 byte
        self.stream.read_exact(&mut [0u8; 1])?;

        // Preview address (4 bytes)
        info.preview_address = self.stream.read_i32::<LittleEndian>()?;

        // DWG version (1 byte)
        info.dwg_version = self.stream.read_u8()?;

        // App release version (1 byte)
        info.app_release_version = self.stream.read_u8()?;

        // Drawing code page (2 bytes)
        info.code_page = self.stream.read_u16::<LittleEndian>()?;

        // Skip 3 bytes
        self.stream.read_exact(&mut [0u8; 3])?;

        // Security type (4 bytes)
        info.security_type = self.stream.read_i32::<LittleEndian>()?;

        // Skip unknown (4 bytes)
        self.stream.read_i32::<LittleEndian>()?;

        // Summary info address (4 bytes)
        info.summary_info_addr = self.stream.read_i32::<LittleEndian>()?;

        // VBA project address (4 bytes)
        info.vba_project_addr = self.stream.read_i32::<LittleEndian>()?;

        // Skip 2 unknown ints (8 bytes)
        self.stream.read_i32::<LittleEndian>()?;
        self.stream.read_i32::<LittleEndian>()?;

        // Skip 80 bytes of padding/unknown data
        let mut pad = [0u8; 80];
        self.stream.read_exact(&mut pad)?;

        Ok(())
    }

    /// Read AC15 (R13/R14/R2000) file header with section locator records.
    ///
    /// The AC15 file header is 0x61 (97) bytes:
    /// ```text
    /// [0x00] Version string (6 bytes)
    /// [0x06] Padding + maintenance version (7 bytes)
    /// [0x0D] Preview seeker (4 bytes)
    /// [0x11] Magic bytes (2 bytes: 0x1B, 0x19)
    /// [0x13] Code page (2 bytes LE)
    /// [0x15] Record count (4 bytes LE) — always 6
    /// [0x19] 6 × Section locator records (9 bytes each)
    /// [0x4F] CRC-16 (2 bytes)
    /// [0x51] End sentinel (16 bytes)
    /// [0x61] End of header → section data starts
    /// ```
    ///
    /// Section numbers: 0=Header, 1=Classes, 2=Handles,
    /// 3=ObjFreeSpace, 4=Template, 5=AuxHeader.
    /// AcDbObjects is not in the locator table — its position is
    /// inferred from the gap between the Classes section end and the
    /// Handles section start (see the calculation below for why the
    /// AuxHeader position is only used as a legacy fallback).
    fn read_file_header_ac15(&mut self, info: &mut DwgFileHeaderInfo) -> Result<(), DxfError> {
        use crate::io::dwg::file_headers::section_definition::names;

        // Seek past the version string (6 bytes already read)
        self.stream.seek(SeekFrom::Start(6))?;

        // 0x06: 5 zero bytes + maintenance version + 1 unknown byte (7 bytes total)
        let mut pad = [0u8; 5];
        self.stream.read_exact(&mut pad)?;
        info.acad_maintenance_version = self.stream.read_u8()?;
        let _unknown = self.stream.read_u8()?;

        // 0x0D: Preview seeker (4 bytes LE)
        info.preview_address = self.stream.read_i32::<LittleEndian>()?;

        // 0x11: Magic bytes (2 bytes)
        let _magic1 = self.stream.read_u8()?;
        let _magic2 = self.stream.read_u8()?;

        // 0x13: Code page (2 bytes LE)
        info.code_page = self.stream.read_u16::<LittleEndian>()?;

        // 0x15: Number of locator records (4 bytes LE) — should be 6
        let record_count = self.stream.read_i32::<LittleEndian>()?;

        // 0x19: Read locator records
        // Each record: number(1) + seeker(4) + size(4) = 9 bytes
        let section_name_for = |n: u8| -> &str {
            match n {
                0 => names::HEADER,
                1 => names::CLASSES,
                2 => names::HANDLES,
                3 => names::OBJ_FREE_SPACE,
                4 => names::TEMPLATE,
                5 => names::AUX_HEADER,
                _ => "Unknown",
            }
        };

        let mut handles_seeker: i64 = 0;
        let mut aux_header_end: i64 = 0;
        let mut classes_end: i64 = 0;

        for _ in 0..record_count.min(6) {
            let number = self.stream.read_u8()?;
            let seeker = self.stream.read_i32::<LittleEndian>()? as i64;
            let size = self.stream.read_i32::<LittleEndian>()? as i64;

            let name = section_name_for(number);
            info.section_locators
                .insert(name.to_string(), (seeker, size));

            // Track offsets for AcDbObjects position calculation
            if number == 1 {
                // Classes section
                classes_end = seeker + size;
            }
            if number == 2 {
                // Handles section
                handles_seeker = seeker;
            }
            if number == 5 {
                // AuxHeader — objects start right after this
                aux_header_end = seeker + size;
            }
        }

        // CRC-16 of every byte before it follows the locator records.
        if (0..=6).contains(&record_count) {
            let crc_at = self.stream.stream_position()?;
            let stored = self.stream.read_u16::<LittleEndian>()?;
            self.stream.seek(SeekFrom::Start(0))?;
            let mut bytes = vec![0u8; crc_at as usize];
            self.stream.read_exact(&mut bytes)?;
            self.stream.seek(SeekFrom::Start(crc_at + 2))?;
            let computed = crate::io::dwg::crc::crc16(crate::io::dwg::crc::CRC16_SEED, &bytes);
            if stored != computed {
                let message = format!(
                    "AC15 file header: CRC mismatch (stored {stored:#06X}, computed {computed:#06X})"
                );
                self.notifications
                    .notify(NotificationType::Warning, message.clone());
                self.report_section_diagnostic(
                    "checksum-mismatch",
                    "FileHeader",
                    Some(crc_at),
                    message,
                );
            }
        }

        // Calculate AcDbObjects position.
        //
        // AcDbObjects is not in the locator table. It occupies the gap between
        // the end of the Classes section and the start of the Handles section,
        // regardless of where Template/AuxHeader are physically placed: many
        // real-world R2000 files store Template/AuxHeader *after* Handles (and
        // acadrust's own R13/R14 writer places ObjFreeSpace/Template after
        // Handles too), so inferring the region from the AuxHeader end yields
        // a negative size and an empty document (issue #55).
        //
        // Handle-map offsets are absolute file positions, so starting the
        // region at classes_end is always safe: the buffer may then include a
        // few leading non-object bytes (e.g. ObjFreeSpace/Template), but those
        // are never referenced by any handle offset.
        let objects_start: i64;
        let objects_size: i64;
        if classes_end > 0 && handles_seeker > classes_end {
            objects_start = classes_end;
            objects_size = handles_seeker - classes_end;
        } else {
            // Malformed Classes locator — fall back to the legacy inference:
            // objects start right after the AuxHeader, or, when AuxHeader is
            // missing (R13/R14 layout), at 0x61 + the sum of all known
            // sections that precede it.
            if aux_header_end == 0 {
                let file_header_size: i64 = 0x61;
                let mut offset = file_header_size;
                for &sect in &[
                    names::HEADER,
                    names::CLASSES,
                    names::OBJ_FREE_SPACE,
                    names::TEMPLATE,
                    names::AUX_HEADER,
                ] {
                    if let Some(&(_, size)) = info.section_locators.get(sect) {
                        offset += size;
                    }
                }
                aux_header_end = offset;
            }
            objects_start = aux_header_end;
            objects_size = handles_seeker - aux_header_end;
        }

        if objects_size > 0 {
            info.section_locators.insert(
                names::ACDB_OBJECTS.to_string(),
                (objects_start, objects_size),
            );
            info.objects_base_offset = objects_start;
        }

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC15 file header: {} locator records, objects at offset {}, size {}",
                record_count, objects_start, objects_size
            ),
        );

        Ok(())
    }

    /// Read AC18 (R2004/R2010/R2013/R2018) inner file header, page map, and section map.
    ///
    /// The AC18 format stores a 0x6C-byte inner file header at file offset 0x80,
    /// XOR'd with a magic sequence. This header contains pointers to the page map
    /// and section map, which together describe the layout of all section pages.
    fn read_file_header_ac18(&mut self, info: &mut DwgFileHeaderInfo) -> Result<(), DxfError> {
        // Read the 0x6C-byte inner file header at offset 0x80
        self.stream.seek(SeekFrom::Start(0x80))?;
        let mut inner = [0u8; 0x6C];
        self.stream.read_exact(&mut inner)?;

        // XOR unmask with magic sequence
        apply_magic_sequence(&mut inner);

        // Verify identifier "AcFssFcAJMB\0"
        if &inner[..12] != b"AcFssFcAJMB\0" {
            return Err(DxfError::InvalidFormat(
                "Invalid AC18 inner file header identifier".into(),
            ));
        }

        // Parse inner file header fields
        let mut cursor = Cursor::new(&inner[..]);
        cursor.set_position(0x28);
        let _last_page_id = cursor.read_i32::<LittleEndian>()?;
        let _last_section_addr = cursor.read_u64::<LittleEndian>()?;
        let _second_header_addr = cursor.read_u64::<LittleEndian>()?;
        let _gap_amount = cursor.read_u32::<LittleEndian>()?;
        let _section_amount = cursor.read_u32::<LittleEndian>()?;

        cursor.set_position(0x50);
        let _section_page_map_id = cursor.read_u32::<LittleEndian>()?;
        let page_map_address_stored = cursor.read_u64::<LittleEndian>()?;
        let section_map_id = cursor.read_u32::<LittleEndian>()?;

        // The stored address is (actual - 0x100)
        let page_map_address = page_map_address_stored + 0x100;

        info.is_ac18_format = true;

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC18 inner header: page_map_address={:#X}, section_map_id={}",
                page_map_address, section_map_id
            ),
        );

        // Read page map (page_number → file_offset mapping)
        self.read_page_map_ac18(info, page_map_address)?;

        // Read section map (section descriptors with per-page info)
        self.read_section_map_ac18(info, section_map_id as i32)?;

        Ok(())
    }

    /// Read the AC18 page map from a known file offset.
    ///
    /// The page map is a system page (20-byte unmasked header + LZ77 data)
    /// containing (page_number, page_size) pairs. File offsets are computed
    /// by accumulating page sizes from offset 0x100.
    fn read_page_map_ac18(
        &mut self,
        info: &mut DwgFileHeaderInfo,
        page_map_address: u64,
    ) -> Result<(), DxfError> {
        self.stream.seek(SeekFrom::Start(page_map_address))?;

        // Read 20-byte system page header (NOT XOR-masked)
        let section_type = self.stream.read_i32::<LittleEndian>()?;
        let decomp_size = self.stream.read_i32::<LittleEndian>()?;
        let comp_size = self.stream.read_i32::<LittleEndian>()?;
        let compression = self.stream.read_i32::<LittleEndian>()?;
        let checksum = self.stream.read_u32::<LittleEndian>()?;

        // Read compressed data
        if comp_size <= 0 || comp_size > 10_000_000 {
            return Err(DxfError::InvalidFormat(format!(
                "Invalid AC18 page map compressed size: {}",
                comp_size
            )));
        }
        // The declared decompressed size sizes the output buffer: reject negative, zero and
        // oversized values before allocating anything.
        let decomp_size = size_from_file(
            "AC18 page map decompressed size",
            decomp_size,
            1..=MAX_SYSTEM_PAGE_SIZE,
        )?;
        let output_size = if compression == 2 {
            decomp_size
        } else {
            comp_size as usize
        };
        self.charge_decompressed_bytes(output_size, "AC18 page map")?;
        let mut compressed = try_zeroed_vec(comp_size as usize, "AC18 page map compressed data")?;
        self.stream.read_exact(&mut compressed)?;
        let computed = system_page_checksum(
            [section_type, decomp_size as i32, comp_size, compression],
            &compressed,
        );
        self.check_page_checksum("AC18 page map", Some(page_map_address), checksum, computed);

        // Decompress
        let decompressed = if compression == 2 {
            decompress_ac18(&compressed, decomp_size)
        } else {
            compressed
        };

        // Parse (page_number, page_size) pairs.
        // Pages are written sequentially starting at file offset 0x100.
        let mut cursor = Cursor::new(&decompressed);
        let mut file_offset: i64 = 0x100;

        while (cursor.position() as usize) + 8 <= decompressed.len() {
            let page_number = cursor.read_i32::<LittleEndian>()?;
            let page_size = cursor.read_i32::<LittleEndian>()?;

            // Entries with page_size <= 0 are alignment/padding markers
            // emitted by ACAD; skip them so they don't corrupt the
            // running file offset.
            if page_size <= 0 {
                continue;
            }

            if page_number > 0 {
                info.page_records
                    .insert(page_number, (file_offset, page_size as i64));
            }
            // Only advance for positive sizes; negative/zero sizes in gap entries are
            // invalid and must not corrupt subsequent page offsets.
            if page_size > 0 {
                file_offset += page_size as i64;
            }
        }

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC18: Read {} page records from page map",
                info.page_records.len()
            ),
        );

        Ok(())
    }

    /// Read the AC18 section map from a system page.
    ///
    /// The section map page describes all logical sections (Header, Classes,
    /// Handles, Objects, etc.) with their compression settings and per-page info.
    fn read_section_map_ac18(
        &mut self,
        info: &mut DwgFileHeaderInfo,
        section_map_id: i32,
    ) -> Result<(), DxfError> {
        let &(page_offset, _) = info.page_records.get(&section_map_id).ok_or_else(|| {
            DxfError::InvalidFormat(format!(
                "AC18 section map page {} not found in page records",
                section_map_id
            ))
        })?;

        self.stream.seek(SeekFrom::Start(page_offset as u64))?;

        // Read 20-byte system page header (NOT XOR-masked)
        let section_type = self.stream.read_i32::<LittleEndian>()?;
        let decomp_size = self.stream.read_i32::<LittleEndian>()?;
        let comp_size = self.stream.read_i32::<LittleEndian>()?;
        let compression = self.stream.read_i32::<LittleEndian>()?;
        let checksum = self.stream.read_u32::<LittleEndian>()?;

        // Read compressed data
        if comp_size <= 0 || comp_size > 10_000_000 {
            return Err(DxfError::InvalidFormat(format!(
                "Invalid AC18 section map compressed size: {}",
                comp_size
            )));
        }
        // The declared decompressed size sizes the output buffer: reject negative, zero and
        // oversized values before allocating anything.
        let decomp_size = size_from_file(
            "AC18 section map decompressed size",
            decomp_size,
            1..=MAX_SYSTEM_PAGE_SIZE,
        )?;
        let output_size = if compression == 2 {
            decomp_size
        } else {
            comp_size as usize
        };
        self.charge_decompressed_bytes(output_size, "AC18 section map")?;
        let mut compressed =
            try_zeroed_vec(comp_size as usize, "AC18 section map compressed data")?;
        self.stream.read_exact(&mut compressed)?;
        let computed = system_page_checksum(
            [section_type, decomp_size as i32, comp_size, compression],
            &compressed,
        );
        self.check_page_checksum(
            "AC18 section map",
            Some(page_offset as u64),
            checksum,
            computed,
        );

        // Decompress
        let decompressed = if compression == 2 {
            decompress_ac18(&compressed, decomp_size)
        } else {
            compressed
        };

        // Parse section descriptors
        let mut cursor = Cursor::new(&decompressed);

        // Header: numDescriptions(4), 0x02(4), 0x7400(4), 0x00(4), numDescriptions(4)
        let num_descriptions = cursor.read_i32::<LittleEndian>()?;
        let _marker = cursor.read_i32::<LittleEndian>()?; // 0x02
        let _max_decomp = cursor.read_i32::<LittleEndian>()?; // 0x7400
        let _unknown = cursor.read_i32::<LittleEndian>()?; // 0x00
        let _num_desc2 = cursor.read_i32::<LittleEndian>()?; // repeat

        for _ in 0..num_descriptions {
            // Per-descriptor: size(8), pageCount(4), maxDecompSize(4),
            //   unknown(4), compressedCode(4), sectionId(4), encrypted(4), name(64)
            let data_size = cursor.read_u64::<LittleEndian>()?;
            let page_count = cursor.read_i32::<LittleEndian>()?;
            let max_decomp_page_size = cursor.read_i32::<LittleEndian>()?;
            let _unknown = cursor.read_i32::<LittleEndian>()?;
            let compressed_code = cursor.read_i32::<LittleEndian>()?;
            let _section_id = cursor.read_i32::<LittleEndian>()?;
            let encrypted = cursor.read_i32::<LittleEndian>()?;

            // Section name (64-byte field, null-terminated). Some writers leave
            // non-zero garbage in the bytes *after* the terminator instead of
            // zero-padding, so cut at the first null rather than trimming
            // trailing nulls — otherwise the embedded null plus trailing junk
            // survives and the name fails to match (e.g. "AcDb:Handles\0t…").
            let mut name_buf = [0u8; 64];
            cursor.read_exact(&mut name_buf)?;
            let name = section_name_from_field(&name_buf);

            // Per-page entries: pageNumber(4), compressedSize(4), offset(8)
            let mut pages = Vec::new();
            for _ in 0..page_count {
                let page_number = cursor.read_i32::<LittleEndian>()?;
                let page_compressed_size = cursor.read_i32::<LittleEndian>()?;
                let page_offset_in_section = cursor.read_u64::<LittleEndian>()?;

                pages.push(DwgPageCrcInfo {
                    page_number: page_number as i64,
                    offset: page_offset_in_section,
                    size: 0,
                    decompressed_size: max_decomp_page_size as u64,
                    compressed_size: page_compressed_size as u64,
                    checksum: 0,
                    crc: 0,
                });
            }

            if !name.is_empty() {
                info.section_descriptors.push(DwgSectionInfo {
                    name: name.clone(),
                    compressed_size: data_size,
                    decompressed_size: max_decomp_page_size as u64,
                    encrypted: encrypted as u64,
                    hash_code: 0,
                    encoding: compressed_code as u64,
                    page_count: page_count as u64,
                    pages,
                });
            }
        }

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC18: Read {} section descriptors from section map",
                info.section_descriptors.len()
            ),
        );

        Ok(())
    }

    /// Read AC1021 (R2007) file header with CRC-64 extraction.
    ///
    /// This performs the full AC1021 header decoding pipeline:
    /// 1. Reed-Solomon decode the 0x400-byte encoded header
    /// 2. Extract CRC, key, and compression parameters
    /// 3. LZ77 AC21 decompress into 0x110-byte metadata buffer
    /// 4. Parse the `Dwg21CompressedMetadata` (including Header CRC-64)
    /// 5. Decode the page map and section map
    fn read_file_header_ac21(&mut self, info: &mut DwgFileHeaderInfo) -> Result<(), DxfError> {
        // After read_file_metadata, stream is at position 0x80 (128).
        // The Reed-Solomon encoded data follows immediately.
        // Do NOT seek — continue reading from current position.

        // Step 1: Read 0x400 bytes of Reed-Solomon encoded data
        let mut compressed_data = [0u8; 0x400];
        self.stream.read_exact(&mut compressed_data)?;

        // Step 2: Reed-Solomon decode (factor=3, block_size=239)
        let mut decoded_data = vec![0u8; 3 * 239]; // 717 bytes
        reed_solomon_decode(&compressed_data, &mut decoded_data, 3, 239);

        // Step 3: Extract header values from decoded data
        let mut cursor = Cursor::new(&decoded_data);

        let crc = cursor.read_i64::<LittleEndian>()?;
        let unknown_key = cursor.read_i64::<LittleEndian>()?;
        let compressed_data_crc = cursor.read_i64::<LittleEndian>()?;
        let compr_len = cursor.read_i32::<LittleEndian>()?;
        let _length2 = cursor.read_i32::<LittleEndian>()?;

        info.ac21_header_crc = Some(crc);
        info.ac21_unknown_key = Some(unknown_key);
        info.ac21_compressed_data_crc = Some(compressed_data_crc);

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC1021 header: CRC={:#018X}, UnknownKey={:#018X}, CompressedDataCRC={:#018X}, ComprLen={}",
                crc as u64, unknown_key as u64, compressed_data_crc as u64, compr_len
            ),
        );

        // Step 4: Extract 0x110-byte metadata
        let mut metadata_buffer = vec![0u8; 0x110];

        if compr_len < 0 {
            // Negative ComprLen means data is stored uncompressed (raw).
            // |ComprLen| = raw data length. Copy directly from offset 0x20.
            let raw_len = compr_len.unsigned_abs() as usize;
            let src_start = 32; // offset 0x20 in decoded data
            let copy_len = raw_len
                .min(0x110)
                .min(decoded_data.len().saturating_sub(src_start));
            metadata_buffer[..copy_len]
                .copy_from_slice(&decoded_data[src_start..src_start + copy_len]);
        } else {
            // Positive ComprLen means data is LZ77 compressed.
            // Decompress from byte offset 32 in decoded_data.
            decompress_ac21(&decoded_data, 32, compr_len as u32, &mut metadata_buffer);
        }

        // Step 5: Parse compressed metadata (extracts CRC-64)
        let metadata = Dwg21CompressedMetadata::from_bytes(&metadata_buffer)?;

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC1021 Header CRC-64 extracted: {:#018X}",
                metadata.header_crc64
            ),
        );

        // Note: The exact CRC-64 algorithm used by Autodesk for this field is
        // undocumented. No known open reference validates
        // this value. It is stored for informational/round-trip purposes.

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC1021 CRC Seeds: CrcSeed={:#018X}, CrcSeedEncoded={:#018X}, RandomSeed={:#018X}",
                metadata.crc_seed, metadata.crc_seed_encoded, metadata.random_seed
            ),
        );

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC1021 Pages Map CRC: compressed={:#018X}, uncompressed={:#018X}, seed={:#018X}",
                metadata.pages_map_crc_compressed,
                metadata.pages_map_crc_uncompressed,
                metadata.pages_map_crc_seed
            ),
        );

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC1021 Sections Map CRC: compressed={:#018X}, uncompressed={:#018X}, seed={:#018X}",
                metadata.sections_map_crc_compressed,
                metadata.sections_map_crc_uncompressed,
                metadata.sections_map_crc_seed
            ),
        );

        // Step 6: Read page map
        self.read_page_map_ac21(info, &metadata)?;

        // Step 7: Read section map
        self.read_section_map_ac21(info, &metadata)?;

        // Store metadata even if reads above fail (for diagnostics)
        info.ac21_metadata = Some(metadata.clone());

        Ok(())
    }

    /// Read the page map from an AC1021 file.
    ///
    /// The page map lists all data pages and their sizes, allowing
    /// the reader to build a page ID → file offset lookup table.
    fn read_page_map_ac21(
        &mut self,
        info: &mut DwgFileHeaderInfo,
        metadata: &Dwg21CompressedMetadata,
    ) -> Result<(), DxfError> {
        // The page map sizes come from the file header; check them before get_page_buffer
        // allocates buffers of these sizes.
        size_from_file(
            "AC21 page map compressed size",
            metadata.pages_map_size_compressed,
            1..=MAX_SYSTEM_PAGE_SIZE,
        )?;
        let decompressed_size = size_from_file(
            "AC21 page map decompressed size",
            metadata.pages_map_size_uncompressed,
            1..=MAX_SYSTEM_PAGE_SIZE,
        )?;
        self.charge_decompressed_bytes(decompressed_size, "AC21 page map")?;
        let page_buffer = self.get_page_buffer(
            metadata.pages_map_offset,
            metadata.pages_map_size_compressed,
            metadata.pages_map_size_uncompressed,
            metadata.pages_map_correction_factor,
            0xEF,
        )?;

        let mut cursor = Cursor::new(&page_buffer);
        let mut offset: i64 = 0;

        while (cursor.position() as usize) < page_buffer.len() {
            let size = cursor.read_i64::<LittleEndian>()?;
            let id = cursor.read_i64::<LittleEndian>()?;

            if size == 0 && id == 0 {
                // Terminator — all remaining bytes are padding
                break;
            }

            let ind = id.unsigned_abs();

            info.page_records.insert(ind as i32, (offset, size));
            offset += size;
        }

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC1021: Read {} page records from page map",
                info.page_records.len()
            ),
        );

        Ok(())
    }

    /// Read the section map from an AC1021 file.
    ///
    /// The section map describes all logical sections (Header, Classes,
    /// Handles, Objects, etc.) and their per-page CRC values.
    fn read_section_map_ac21(
        &mut self,
        info: &mut DwgFileHeaderInfo,
        metadata: &Dwg21CompressedMetadata,
    ) -> Result<(), DxfError> {
        // Look up the section map page
        let sections_map_id = metadata.sections_map_id as i32;
        let seeker = info
            .page_records
            .get(&sections_map_id)
            .map(|&(offset, _)| offset)
            .ok_or_else(|| {
                DxfError::InvalidFormat(format!(
                    "Section map page ID {} not found in page records",
                    sections_map_id
                ))
            })?;

        // The section map sizes come from the file header; check them before
        // get_page_buffer_at allocates buffers of these sizes.
        size_from_file(
            "AC21 section map compressed size",
            metadata.sections_map_size_compressed,
            1..=MAX_SYSTEM_PAGE_SIZE,
        )?;
        let decompressed_size = size_from_file(
            "AC21 section map decompressed size",
            metadata.sections_map_size_uncompressed,
            1..=MAX_SYSTEM_PAGE_SIZE,
        )?;
        self.charge_decompressed_bytes(decompressed_size, "AC21 section map")?;
        let section_buffer = self.get_page_buffer_at(
            seeker as u64,
            metadata.sections_map_size_compressed,
            metadata.sections_map_size_uncompressed,
            metadata.sections_map_correction_factor,
            239,
        )?;

        let mut cursor = Cursor::new(&section_buffer);

        while (cursor.position() as usize) < section_buffer.len() {
            // Check if there's enough data for at least the fixed header fields
            if section_buffer.len() - (cursor.position() as usize) < 64 {
                break;
            }

            let compressed_size = cursor.read_u64::<LittleEndian>()?;
            let decompressed_size = cursor.read_u64::<LittleEndian>()?;
            let encrypted = cursor.read_u64::<LittleEndian>()?;
            let hash_code = cursor.read_u64::<LittleEndian>()?;
            let section_name_length = cursor.read_i64::<LittleEndian>()?;
            let _unknown = cursor.read_u64::<LittleEndian>()?;
            let encoding = cursor.read_u64::<LittleEndian>()?;
            let page_count = cursor.read_u64::<LittleEndian>()?;

            // Read section name (UTF-16LE)
            let name = if section_name_length > 0 {
                let byte_len = section_name_length as usize;
                if cursor.position() as usize + byte_len > section_buffer.len() {
                    break;
                }
                let mut name_bytes = vec![0u8; byte_len];
                cursor.read_exact(&mut name_bytes)?;
                // Decode UTF-16LE
                let words: Vec<u16> = name_bytes
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16_lossy(&words)
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                String::new()
            };

            // Read per-page CRC information
            let mut pages = Vec::new();
            for _ in 0..page_count {
                if section_buffer.len() - (cursor.position() as usize) < 56 {
                    break;
                }
                let page_offset = cursor.read_u64::<LittleEndian>()?;
                let page_size = cursor.read_i64::<LittleEndian>()?;
                let page_number = cursor.read_i64::<LittleEndian>()?;
                let page_decompressed_size = cursor.read_u64::<LittleEndian>()?;
                let page_compressed_size = cursor.read_u64::<LittleEndian>()?;
                let page_checksum = cursor.read_u64::<LittleEndian>()?;
                let page_crc = cursor.read_u64::<LittleEndian>()?;

                self.notifications.notify(
                    NotificationType::Warning,
                    format!(
                        "  Section '{}' page {}: CRC={:#018X}, Checksum={:#018X}, CompSize={}, DecompSize={}",
                        name, page_number, page_crc, page_checksum,
                        page_compressed_size, page_decompressed_size
                    ),
                );

                pages.push(DwgPageCrcInfo {
                    page_number,
                    offset: page_offset,
                    size: page_size,
                    decompressed_size: page_decompressed_size,
                    compressed_size: page_compressed_size,
                    checksum: page_checksum,
                    crc: page_crc,
                });
            }

            if section_name_length > 0 {
                info.section_descriptors.push(DwgSectionInfo {
                    name: name.clone(),
                    compressed_size,
                    decompressed_size,
                    encrypted,
                    hash_code,
                    encoding,
                    page_count,
                    pages,
                });
            }
        }

        self.notifications.notify(
            NotificationType::Warning,
            format!(
                "AC1021: Read {} section descriptors from section map",
                info.section_descriptors.len()
            ),
        );

        Ok(())
    }

    /// Get a decompressed page buffer from the file.
    ///
    /// Handles the Reed-Solomon + LZ77 AC21 decompression pipeline.
    fn get_page_buffer(
        &mut self,
        page_offset: u64,
        compressed_size: u64,
        uncompressed_size: u64,
        correction_factor: u64,
        block_size: usize,
    ) -> Result<Vec<u8>, DxfError> {
        self.get_page_buffer_at(
            page_offset,
            compressed_size,
            uncompressed_size,
            correction_factor,
            block_size,
        )
    }

    /// Get a decompressed page buffer from a specific file offset.
    fn get_page_buffer_at(
        &mut self,
        page_offset: u64,
        compressed_size: u64,
        uncompressed_size: u64,
        correction_factor: u64,
        block_size: usize,
    ) -> Result<Vec<u8>, DxfError> {
        let (total_size, factor, read_length) =
            ac21_page_layout(compressed_size, correction_factor, block_size)?;

        // Read encoded data from file
        self.stream
            .seek(SeekFrom::Start(AC21_FILE_HEADER_SIZE + page_offset))?;
        let mut encoded_buffer = try_zeroed_vec(read_length, "AC21 encoded page data")?;
        let bytes_read = self.stream.read(&mut encoded_buffer)?;
        if bytes_read < read_length {
            // Pad remaining with zeros
            encoded_buffer[bytes_read..].fill(0);
        }

        decode_ac21_page(
            &encoded_buffer,
            total_size,
            factor,
            block_size,
            compressed_size,
            uncompressed_size,
            None,
        )
    }

    /// Get the merged decompressed buffer for a named section (AC21).
    ///
    /// Reads and concatenates all pages belonging to the given section,
    /// producing the complete section data ready for parsing.
    ///
    /// In failsafe mode, missing or unreadable pages are skipped and a
    /// warning notification is emitted rather than aborting the entire
    /// section read.
    ///
    /// # Arguments
    /// * `section_name` - Section name (e.g., "AcDb:Header", "AcDb:Classes")
    /// * `info` - Previously read file header info containing section descriptors
    ///
    /// # Returns
    /// The complete decompressed section buffer, or an error if the section
    /// is not found or a page cannot be read (unless in failsafe mode).
    pub fn get_section_buffer(
        &mut self,
        section_name: &str,
        info: &DwgFileHeaderInfo,
    ) -> Result<Vec<u8>, DxfError> {
        let failsafe = self.options.failsafe;
        let max_section = self.options.section_size_limit();
        let perf = std::env::var_os("PERF").is_some();
        let started = web_time::Instant::now();

        // ── AC15 path: direct read from section locators ──
        // If we have section_locators (AC15 format), read raw bytes
        // directly from the file at the recorded offset.
        if !info.section_locators.is_empty() {
            if let Some(&(offset, size)) = info.section_locators.get(section_name) {
                if offset < 0 || size <= 0 {
                    return Err(DxfError::Parse(format!(
                        "Section '{}' has invalid locator offset/size ({offset}, {size})",
                        section_name
                    )));
                }
                let requested_size = usize::try_from(size).map_err(|_| {
                    DxfError::Parse(format!(
                        "Section '{}' size {} does not fit in memory",
                        section_name, size
                    ))
                })?;
                // The section is stored uncompressed in the file, so only
                // what the file holds past the offset can be read.
                let file_len = self.stream.seek(SeekFrom::End(0))?;
                let available = file_len.saturating_sub(offset as u64);
                if !failsafe && requested_size as u64 > available {
                    return Err(DxfError::Parse(format!(
                        "Section '{}' size {} runs past the end of the file",
                        section_name, requested_size
                    )));
                }
                self.stream.seek(SeekFrom::Start(offset as u64))?;
                let read_size = requested_size.min(available as usize);
                self.charge_decompressed_bytes(
                    read_size,
                    format_args!("AC15 section '{}'", section_name),
                )?;
                let mut buf =
                    try_zeroed_vec(read_size, format_args!("AC15 section '{}'", section_name))?;
                if failsafe {
                    let mut filled = 0usize;
                    let mut read_error = None;
                    while filled < buf.len() {
                        match self.stream.read(&mut buf[filled..]) {
                            Ok(0) => break,
                            Ok(count) => filled += count,
                            Err(error) => {
                                read_error = Some(error);
                                break;
                            }
                        }
                    }
                    if filled != requested_size || read_error.is_some() {
                        let detail =
                            read_error
                                .map(|error| error.to_string())
                                .unwrap_or_else(|| {
                                    format!(
                                        "unexpected end of file after {} of {} bytes",
                                        filled, requested_size
                                    )
                                });
                        buf.truncate(filled);
                        self.notifications.notify(
                            NotificationType::Warning,
                            format!(
                                "Failsafe: truncated AC15 section '{}' at byte {}: {}",
                                section_name,
                                offset.saturating_add(filled as i64),
                                detail
                            ),
                        );
                        self.report_section_diagnostic(
                            "section-read-truncated",
                            section_name,
                            Some(offset.saturating_add(filled as i64) as u64),
                            format!(
                                "AC15 section read stopped after {} of {} bytes: {}",
                                filled, requested_size, detail
                            ),
                        );
                    }
                } else {
                    self.stream.read_exact(&mut buf)?;
                }
                if perf {
                    eprintln!(
                        "[perf] dwg-section name={} format=ac15 time={:.1}ms bytes={}",
                        section_name,
                        started.elapsed().as_secs_f64() * 1000.0,
                        buf.len(),
                    );
                }
                return Ok(buf);
            } else {
                return Err(DxfError::Parse(format!(
                    "Section '{}' not found in AC15 locator records",
                    section_name
                )));
            }
        }

        // ── AC18 path: page-based with LZ77 AC18 compression ──
        if info.is_ac18_format {
            let result = self.get_section_buffer_ac18(section_name, info);
            if perf {
                eprintln!(
                    "[perf] dwg-section name={} format=ac18 time={:.1}ms bytes={}",
                    section_name,
                    started.elapsed().as_secs_f64() * 1000.0,
                    result.as_ref().map_or(0, Vec::len),
                );
            }
            return result;
        }

        // ── AC21 path: page-based section descriptors ──
        // Find the section descriptor
        let section = info
            .section_descriptors
            .iter()
            .find(|s| s.name == section_name)
            .ok_or_else(|| {
                DxfError::Parse(format!("Section '{}' not found in file", section_name))
            })?;

        if section.encrypted != 0 && self.decryption_key.is_none() {
            self.ensure_decryption_key(info)?;
        }

        // Field 0x00 ("Data size") holds the total section data size.
        // Field 0x08 ("Max size") is the page partition size per spec §5.4.
        // We truncate to the total data size, not the page size.
        // Check the section size and every page size before any page is read, so that
        // neither strict nor failsafe mode allocates an unchecked size.
        let total_size = size_from_file(
            format_args!("AC21 section '{}' size", section_name),
            section.compressed_size,
            0..=max_section,
        )?;
        let mut page_total = 0u64;
        for page in &section.pages {
            size_from_file(
                format_args!(
                    "AC21 section '{}' page {} decompressed size",
                    section_name, page.page_number
                ),
                page.decompressed_size,
                0..=MAX_DATA_PAGE_SIZE,
            )?;
            size_from_file(
                format_args!(
                    "AC21 section '{}' page {} compressed size",
                    section_name, page.page_number
                ),
                page.compressed_size,
                0..=MAX_DATA_PAGE_SIZE,
            )?;
            page_total = page_total.saturating_add(page.decompressed_size);
        }
        let page_total = size_from_file(
            format_args!("AC21 section '{}' page total", section_name),
            page_total,
            0..=max_section,
        )?;
        // Charge the larger of the declared section size and the sum of its
        // pages, so a section that declares padding is still budgeted.
        self.charge_decompressed_bytes(
            total_size.max(page_total),
            format_args!("AC21 section '{}'", section_name),
        )?;
        // The pages append at most `page_total` bytes, so reserve that once,
        // fallibly: later `extend_from_slice`/`resize` calls cannot reallocate
        // (and so cannot hit the aborting allocator path) within this capacity.
        let mut result =
            try_vec_with_capacity(page_total, format_args!("AC21 section '{}'", section_name))?;

        // encoding=1 (stored): contiguous data followed by non-interleaved RS
        // parity. Reading only the payload deliberately skips that parity.
        // encoding=4 (compressed): data is LZ77-compressed then RS-encoded with RS(255,251).
        // System pages (page map, section map) use RS(255,239) per §5.3,
        // but those are decoded separately in read_page_map_ac21 / read_section_map_ac21.
        let encoding = section.encoding;
        let block_size: usize = 251;

        enum PagePayload {
            Ready(Vec<u8>),
            Encoded {
                bytes: Vec<u8>,
                total_size: usize,
                factor: usize,
                compressed_size: u64,
                uncompressed_size: u64,
            },
            Failed(String),
        }
        struct PreparedPage {
            number: i64,
            fill_size: usize,
            payload: PagePayload,
        }

        // File seeks stay ordered; CPU-heavy Reed-Solomon and LZ77 work runs
        // concurrently in bounded batches. This keeps peak memory near
        // `result + 2 * worker pages` instead of retaining encoded and decoded
        // copies of the whole section at once.
        let batch_pages = worker_count().saturating_mul(2);
        let mut skipped_pages = 0u32;
        for page_batch in section.pages.chunks(batch_pages) {
            let mut prepared = Vec::with_capacity(page_batch.len());
            for page in page_batch {
                let fill_size = page.decompressed_size as usize;
                let payload = match info.page_records.get(&(page.page_number as i32)) {
                    Some(&(page_offset, _)) if encoding == 1 => {
                        let read = (|| -> Result<Vec<u8>, DxfError> {
                            self.stream.seek(SeekFrom::Start(
                                AC21_FILE_HEADER_SIZE + page_offset as u64,
                            ))?;
                            let mut bytes = try_zeroed_vec(
                                fill_size,
                                format_args!("AC21 page {}", page.page_number),
                            )?;
                            self.stream.read_exact(&mut bytes)?;
                            if section.encrypted != 0 {
                                if let Some(key) = &self.decryption_key {
                                    let mut rc4 = crate::io::dwg::security::Rc4::new(key);
                                    rc4.apply(&mut bytes);
                                } else {
                                    return Err(DxfError::Decryption(format!(
                                        "Section '{}' is encrypted but no decryption key is available",
                                        section_name
                                    )));
                                }
                            }
                            Ok(bytes)
                        })();
                        match read {
                            Ok(bytes) => PagePayload::Ready(bytes),
                            Err(error) if failsafe => PagePayload::Failed(error.to_string()),
                            Err(error) => return Err(error),
                        }
                    }
                    Some(&(page_offset, _)) => {
                        match ac21_page_layout(page.compressed_size, 1, block_size) {
                            Ok((total_size, factor, read_length)) => {
                                let read = (|| -> Result<Vec<u8>, DxfError> {
                                    self.stream.seek(SeekFrom::Start(
                                        AC21_FILE_HEADER_SIZE + page_offset as u64,
                                    ))?;
                                    let mut bytes = try_zeroed_vec(
                                        read_length,
                                        format_args!("AC21 page {}", page.page_number),
                                    )?;
                                    let read = self.stream.read(&mut bytes)?;
                                    bytes[read..].fill(0);
                                    Ok(bytes)
                                })();
                                match read {
                                    Ok(bytes) => PagePayload::Encoded {
                                        bytes,
                                        total_size,
                                        factor,
                                        compressed_size: page.compressed_size,
                                        uncompressed_size: page.decompressed_size,
                                    },
                                    Err(error) if failsafe => {
                                        PagePayload::Failed(error.to_string())
                                    }
                                    Err(error) => return Err(error),
                                }
                            }
                            Err(error) if failsafe => PagePayload::Failed(error.to_string()),
                            Err(error) => return Err(error),
                        }
                    }
                    None if failsafe => PagePayload::Failed(format!(
                        "page {} not found in page map",
                        page.page_number
                    )),
                    None => {
                        return Err(DxfError::Parse(format!(
                            "Page {} not found in page map",
                            page.page_number
                        )))
                    }
                };
                prepared.push(PreparedPage {
                    number: page.page_number,
                    fill_size,
                    payload,
                });
            }

            let key_ref = if section.encrypted != 0 {
                self.decryption_key.as_deref()
            } else {
                None
            };
            let decoded = map_ordered(prepared, |page| {
                let data = match page.payload {
                    PagePayload::Ready(bytes) => Ok(bytes),
                    PagePayload::Encoded {
                        bytes,
                        total_size,
                        factor,
                        compressed_size,
                        uncompressed_size,
                    } => decode_ac21_page(
                        &bytes,
                        total_size,
                        factor,
                        block_size,
                        compressed_size,
                        uncompressed_size,
                        key_ref,
                    )
                    .map_err(|error| error.to_string()),
                    PagePayload::Failed(error) => Err(error),
                };
                (page.number, page.fill_size, data)
            });

            for (page_number, fill_size, page_result) in decoded {
                match page_result {
                    Ok(page_data) => result.extend_from_slice(&page_data),
                    Err(error) => {
                        skipped_pages += 1;
                        self.notifications.notify(
                            NotificationType::Error,
                            format!(
                                "Failsafe: skipped corrupt page {} in section '{}': {}",
                                page_number, section_name, error
                            ),
                        );
                        result.resize(result.len() + fill_size, 0);
                    }
                }
            }
        }

        if skipped_pages > 0 {
            self.notifications.notify(
                NotificationType::Warning,
                format!(
                    "Failsafe: {} of {} pages skipped in section '{}'",
                    skipped_pages,
                    section.pages.len(),
                    section_name
                ),
            );
        }

        // Truncate to the declared section size (last page may be padded)
        result.truncate(total_size);

        if perf {
            eprintln!(
                "[perf] dwg-section name={} format=ac21 time={:.1}ms bytes={} pages={}",
                section_name,
                started.elapsed().as_secs_f64() * 1000.0,
                result.len(),
                section.pages.len(),
            );
        }
        Ok(result)
    }

    /// Get the merged decompressed buffer for a named section (AC18).
    ///
    /// Reads data pages for the given section, XOR-unmasks their 32-byte
    /// headers, and decompresses the LZ77 AC18 data.
    fn get_section_buffer_ac18(
        &mut self,
        section_name: &str,
        info: &DwgFileHeaderInfo,
    ) -> Result<Vec<u8>, DxfError> {
        let failsafe = self.options.failsafe;
        let max_section = self.options.section_size_limit();
        let section = info
            .section_descriptors
            .iter()
            .find(|s| s.name == section_name)
            .ok_or_else(|| {
                DxfError::Parse(format!("Section '{}' not found in AC18 file", section_name))
            })?;

        if section.encrypted != 0 && self.decryption_key.is_none() {
            self.ensure_decryption_key(info)?;
        }

        // compressed_size field actually holds the total uncompressed section data size
        let total_size = size_from_file(
            format_args!("AC18 section '{}' size", section_name),
            section.compressed_size,
            0..=max_section,
        )?;
        let is_compressed = section.encoding == 2;
        let max_page_size = size_from_file(
            format_args!("AC18 section '{}' page size", section_name),
            section.decompressed_size,
            0..=MAX_DATA_PAGE_SIZE,
        )?;
        // The section is its pages: a declared size they cannot hold is a
        // corrupt header, while a large drawing legitimately spans many pages.
        // Count the page descriptors actually parsed: the declared count is file
        // data, and -1 reads as u64::MAX, which would lift this bound entirely.
        // Pages of zeros are not stored, and each page is placed at its own
        // offset with the gaps zero-filled, so a sparse section reaches as far
        // as its furthest page. The section size limit and the decompressed budget
        // still bound the allocation.
        let page_count = section.pages.len();
        let furthest_page_end = section
            .pages
            .iter()
            .map(|page| {
                usize::try_from(page.offset)
                    .unwrap_or(usize::MAX)
                    .saturating_add(max_page_size)
            })
            .max()
            .unwrap_or(0);
        let page_capacity = page_count
            .saturating_mul(max_page_size)
            .max(furthest_page_end);
        if total_size > page_capacity {
            // Some AC1032 writers include a small amount of section padding in the
            // declared size, beyond the sum of the parsed page capacities. Keep the
            // bounded allocation and zero-filled tail, while rejecting a declaration
            // that is clearly unrelated to the pages it names.
            let hard_limit = page_capacity.saturating_mul(2);
            if page_capacity == 0 || total_size > hard_limit {
                return Err(DxfError::Parse(format!(
                    "AC18 section '{}' size {} exceeds its {} page(s) of {} bytes",
                    section_name, total_size, page_count, max_page_size
                )));
            }
            self.notifications.notify(
                NotificationType::Warning,
                format!(
                    "AC18 section '{}' declared size {} is slightly larger than its {} page(s) of {} bytes; retaining the bounded section",
                    section_name, total_size, page_count, max_page_size
                ),
            );
        }

        self.charge_decompressed_bytes(
            total_size,
            format_args!("AC18 section '{}'", section_name),
        )?;
        let mut result =
            try_zeroed_vec(total_size, format_args!("AC18 section '{}'", section_name))?;
        let batch_pages = worker_count().saturating_mul(2);
        let mut skipped_pages = 0usize;

        for page_batch in section.pages.chunks(batch_pages) {
            let mut prepared = Vec::with_capacity(page_batch.len());
            for page in page_batch {
                let page_number = page.page_number as i32;
                let page_result = (|| -> Result<(usize, Vec<u8>, [u32; 4]), DxfError> {
                    let &(page_file_offset, _page_total_size) =
                        info.page_records.get(&page_number).ok_or_else(|| {
                            DxfError::Parse(format!(
                                "AC18 page {} not found in page records for section '{}'",
                                page_number, section_name
                            ))
                        })?;
                    if page_file_offset < 0 {
                        return Err(DxfError::Parse(format!(
                            "AC18 page {} has a negative file offset {}",
                            page_number, page_file_offset
                        )));
                    }

                    self.stream.seek(SeekFrom::Start(page_file_offset as u64))?;

                    // Read 32-byte data section header (XOR-masked)
                    let mut header = [0u8; 32];
                    self.stream.read_exact(&mut header)?;

                    // XOR unmask using the page's file position
                    apply_mask(&mut header, page_file_offset as u64);

                    // Parse header fields
                    let mut hcursor = Cursor::new(&header[..]);
                    let _section_type = hcursor.read_i32::<LittleEndian>()?;
                    let _section_id = hcursor.read_i32::<LittleEndian>()?;
                    let data_compressed_size = hcursor.read_i32::<LittleEndian>()?;
                    let _page_size = hcursor.read_i32::<LittleEndian>()?;
                    let data_offset = hcursor.read_i64::<LittleEndian>()?;
                    if data_compressed_size <= 0 || data_compressed_size > 10_000_000 {
                        return Err(DxfError::Parse(format!(
                            "invalid compressed size {}",
                            data_compressed_size
                        )));
                    }
                    if data_offset < 0 {
                        return Err(DxfError::Parse(format!(
                            "negative destination offset {}",
                            data_offset
                        )));
                    }

                    // Read compressed data
                    let mut compressed = try_zeroed_vec(
                        data_compressed_size as usize,
                        format_args!("AC18 page {} compressed data", page_number),
                    )?;
                    self.stream.read_exact(&mut compressed)?;
                    // Header checksum at 0x18 (its header with that field as
                    // zero, seeded with the data checksum), data checksum at
                    // 0x1C (the stored data, seed 0).
                    let stored_header = u32::from_le_bytes(header[0x18..0x1C].try_into().unwrap());
                    let stored_data = u32::from_le_bytes(header[0x1C..0x20].try_into().unwrap());
                    let computed_data = adler32_checksum(0, &compressed);
                    let mut zeroed = header;
                    zeroed[0x18..0x1C].fill(0);
                    let computed_header = adler32_checksum(stored_data, &zeroed);
                    let checksums = [stored_header, computed_header, stored_data, computed_data];
                    if section.encrypted != 0 {
                        if let Some(key) = &self.decryption_key {
                            let mut rc4 = crate::io::dwg::security::Rc4::new(key);
                            rc4.apply(&mut compressed);
                        } else {
                            return Err(DxfError::Decryption(format!(
                                "Section '{}' is encrypted but no decryption key is available",
                                section_name
                            )));
                        }
                    }

                    Ok((data_offset as usize, compressed, checksums))
                })();

                match page_result {
                    Ok((offset, data, checksums)) => {
                        // An encrypted page's checksums are not checked: what
                        // they cover is not settled for that layout.
                        if section.encrypted == 0 {
                            let at = info
                                .page_records
                                .get(&page_number)
                                .map(|&(offset, _)| offset as u64);
                            let name = format!("AC18 section '{section_name}' page {page_number}");
                            self.check_page_checksum(&name, at, checksums[0], checksums[1]);
                            self.check_page_checksum(&name, at, checksums[2], checksums[3]);
                        }
                        prepared.push((offset, data));
                    }
                    Err(error) if failsafe => {
                        skipped_pages += 1;
                        let page_offset = info
                            .page_records
                            .get(&page_number)
                            .and_then(|(offset, _)| (*offset >= 0).then_some(*offset as u64));
                        self.notifications.notify(
                            NotificationType::Warning,
                            format!(
                                "Failsafe: skipped corrupt AC18 page {} in section '{}': {}",
                                page_number, section_name, error
                            ),
                        );
                        self.report_section_diagnostic(
                            "section-page-skipped",
                            section_name,
                            page_offset,
                            format!(
                                "AC18 page {} skipped while reading section '{}': {}",
                                page_number, section_name, error
                            ),
                        );
                    }
                    Err(error) => return Err(error),
                }
            }

            let decoded = map_ordered(prepared, |(dst_start, compressed)| {
                let decompressed = if is_compressed {
                    decompress_ac18(&compressed, max_page_size)
                } else {
                    compressed
                };
                (dst_start, decompressed)
            });

            for (dst_start, decompressed) in decoded {
                if dst_start < total_size {
                    let copy_len = decompressed.len().min(total_size - dst_start);
                    result[dst_start..dst_start + copy_len]
                        .copy_from_slice(&decompressed[..copy_len]);
                }
            }
        }

        if skipped_pages > 0 {
            self.notifications.notify(
                NotificationType::Warning,
                format!(
                    "Failsafe: {} of {} pages skipped in AC18 section '{}'",
                    skipped_pages,
                    section.pages.len(),
                    section_name
                ),
            );
        }

        Ok(result)
    }

    /// Find a section descriptor by name.
    pub fn find_section<'a>(info: &'a DwgFileHeaderInfo, name: &str) -> Option<&'a DwgSectionInfo> {
        info.section_descriptors.iter().find(|s| s.name == name)
    }

    /// Extract all CRC values from the file and return them as a summary.
    ///
    /// This is a convenience method that reads the file header and
    /// formats all CRC values for display.
    pub fn extract_all_crcs(&mut self) -> Result<CrcExtractionReport, DxfError> {
        let info = self.read_file_header()?;

        let mut report = CrcExtractionReport {
            version: info.version_string.clone(),
            header_crc64: None,
            header_crc: info.ac21_header_crc,
            compressed_data_crc: info.ac21_compressed_data_crc,
            pages_map_crc_compressed: None,
            pages_map_crc_uncompressed: None,
            pages_map_crc_seed: None,
            sections_map_crc_compressed: None,
            sections_map_crc_uncompressed: None,
            sections_map_crc_seed: None,
            crc_seed: None,
            crc_seed_encoded: None,
            random_seed: None,
            page_crcs: Vec::new(),
            notifications: std::mem::take(&mut self.notifications),
        };

        if let Some(ref metadata) = info.ac21_metadata {
            report.header_crc64 = Some(metadata.header_crc64);
            report.pages_map_crc_compressed = Some(metadata.pages_map_crc_compressed);
            report.pages_map_crc_uncompressed = Some(metadata.pages_map_crc_uncompressed);
            report.pages_map_crc_seed = Some(metadata.pages_map_crc_seed);
            report.sections_map_crc_compressed = Some(metadata.sections_map_crc_compressed);
            report.sections_map_crc_uncompressed = Some(metadata.sections_map_crc_uncompressed);
            report.sections_map_crc_seed = Some(metadata.sections_map_crc_seed);
            report.crc_seed = Some(metadata.crc_seed);
            report.crc_seed_encoded = Some(metadata.crc_seed_encoded);
            report.random_seed = Some(metadata.random_seed);
        }

        for section in &info.section_descriptors {
            for page in &section.pages {
                report.page_crcs.push(PageCrcEntry {
                    section_name: section.name.clone(),
                    page_number: page.page_number,
                    crc: page.crc,
                    checksum: page.checksum,
                    compressed_size: page.compressed_size,
                    decompressed_size: page.decompressed_size,
                });
            }
        }

        Ok(report)
    }
}

/// Complete CRC extraction report from a DWG file.
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CrcExtractionReport {
    /// DWG version string
    pub version: String,
    /// Header CRC-64 (AC1021 only)
    pub header_crc64: Option<u64>,
    /// Header CRC from Reed-Solomon decoded data
    pub header_crc: Option<i64>,
    /// Compressed data CRC
    pub compressed_data_crc: Option<i64>,
    /// Pages map CRC (compressed)
    pub pages_map_crc_compressed: Option<u64>,
    /// Pages map CRC (uncompressed)
    pub pages_map_crc_uncompressed: Option<u64>,
    /// Pages map CRC seed
    pub pages_map_crc_seed: Option<u64>,
    /// Sections map CRC (compressed)
    pub sections_map_crc_compressed: Option<u64>,
    /// Sections map CRC (uncompressed)
    pub sections_map_crc_uncompressed: Option<u64>,
    /// Sections map CRC seed
    pub sections_map_crc_seed: Option<u64>,
    /// Global CRC seed
    pub crc_seed: Option<u64>,
    /// Encoded CRC seed
    pub crc_seed_encoded: Option<u64>,
    /// Random seed
    pub random_seed: Option<u64>,
    /// Per-page CRC values
    pub page_crcs: Vec<PageCrcEntry>,
    /// Notifications collected during reading
    pub notifications: NotificationCollection,
}

/// CRC entry for a single page.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PageCrcEntry {
    /// Section name
    pub section_name: String,
    /// Page number
    pub page_number: i64,
    /// CRC value
    pub crc: u64,
    /// Checksum value
    pub checksum: u64,
    /// Compressed size
    pub compressed_size: u64,
    /// Decompressed size
    pub decompressed_size: u64,
}

impl std::fmt::Display for CrcExtractionReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== DWG CRC Extraction Report ===")?;
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f)?;

        if let Some(crc64) = self.header_crc64 {
            writeln!(f, "--- Header CRC-64 ---")?;
            writeln!(f, "  Header CRC-64:              {:#018X}", crc64)?;
        }

        if let Some(crc) = self.header_crc {
            writeln!(f, "  Header CRC (RS decoded):    {:#018X}", crc as u64)?;
        }
        if let Some(crc) = self.compressed_data_crc {
            writeln!(f, "  Compressed Data CRC:        {:#018X}", crc as u64)?;
        }

        if self.crc_seed.is_some() {
            writeln!(f)?;
            writeln!(f, "--- CRC Seeds ---")?;
            if let Some(v) = self.crc_seed {
                writeln!(f, "  CRC Seed:                   {:#018X}", v)?;
            }
            if let Some(v) = self.crc_seed_encoded {
                writeln!(f, "  CRC Seed Encoded:           {:#018X}", v)?;
            }
            if let Some(v) = self.random_seed {
                writeln!(f, "  Random Seed:                {:#018X}", v)?;
            }
        }

        if self.pages_map_crc_compressed.is_some() {
            writeln!(f)?;
            writeln!(f, "--- Pages Map CRC ---")?;
            if let Some(v) = self.pages_map_crc_compressed {
                writeln!(f, "  Compressed:                 {:#018X}", v)?;
            }
            if let Some(v) = self.pages_map_crc_uncompressed {
                writeln!(f, "  Uncompressed:               {:#018X}", v)?;
            }
            if let Some(v) = self.pages_map_crc_seed {
                writeln!(f, "  Seed:                       {:#018X}", v)?;
            }
        }

        if self.sections_map_crc_compressed.is_some() {
            writeln!(f)?;
            writeln!(f, "--- Sections Map CRC ---")?;
            if let Some(v) = self.sections_map_crc_compressed {
                writeln!(f, "  Compressed:                 {:#018X}", v)?;
            }
            if let Some(v) = self.sections_map_crc_uncompressed {
                writeln!(f, "  Uncompressed:               {:#018X}", v)?;
            }
            if let Some(v) = self.sections_map_crc_seed {
                writeln!(f, "  Seed:                       {:#018X}", v)?;
            }
        }

        if !self.page_crcs.is_empty() {
            writeln!(f)?;
            writeln!(f, "--- Per-Page CRC Values ---")?;
            for entry in &self.page_crcs {
                writeln!(
                    f,
                    "  {} [page {}]: CRC={:#018X}, Checksum={:#018X} (comp={}, decomp={})",
                    entry.section_name,
                    entry.page_number,
                    entry.crc,
                    entry.checksum,
                    entry.compressed_size,
                    entry.decompressed_size
                )?;
            }
        }

        Ok(())
    }
}

/// The checksum of an AC18 system page (page map, section map): Adler-32 of
/// its 20-byte header with the checksum field as zero, then of its data.
fn system_page_checksum(header: [i32; 4], data: &[u8]) -> u32 {
    let mut bytes = [0u8; 20];
    for (slot, value) in bytes.chunks_exact_mut(4).zip(header) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
    adler32_checksum(adler32_checksum(0, &bytes), data)
}

#[cfg(test)]
mod dwg_reader_tests {
    use super::{acds_modeler_bodies, section_name_from_field};

    fn field(prefix: &[u8]) -> [u8; 64] {
        let mut b = [0u8; 64];
        b[..prefix.len()].copy_from_slice(prefix);
        b
    }

    #[test]
    fn clean_zero_padded_name() {
        assert_eq!(
            section_name_from_field(&field(b"AcDb:Handles")),
            "AcDb:Handles"
        );
    }

    #[test]
    fn stops_at_first_null_ignoring_trailing_garbage() {
        // Terminator at index 12, then non-zero junk — the real-world case that
        // broke `trim_end_matches('\0')`.
        let mut b = field(b"AcDb:Handles");
        b[13] = b't';
        b[14] = 0x01;
        b[20] = b'X';
        assert_eq!(section_name_from_field(&b), "AcDb:Handles");
    }

    #[test]
    fn empty_when_first_byte_null() {
        assert_eq!(section_name_from_field(&[0u8; 64]), "");
    }

    use crate::entities::{Body, EntityType, Region, Solid3D};
    use crate::types::{Transform, Vector3};

    /// The SAB of a body whose two vertex points have the bounding-box centre
    /// (2, 4, 6).
    fn centred_sab() -> Vec<u8> {
        use crate::entities::acis::{SabWriter, SatDocument};
        let mut sat = SatDocument::new_body();
        sat.add_point(0.0, 0.0, 0.0);
        sat.add_point(4.0, 8.0, 12.0);
        SabWriter::write(&sat)
    }

    const CENTRE: Vector3 = Vector3 {
        x: 2.0,
        y: 4.0,
        z: 6.0,
    };

    /// A document with one solid, one region and one body (no anchor stored),
    /// each given `centred_sab()` through the record-table attach path.
    fn attached_modelers(defer: bool) -> (crate::document::CadDocument, [crate::Handle; 3]) {
        let sab = centred_sab();
        let mut document = crate::document::CadDocument::new();
        let solid = document
            .add_entity(EntityType::Solid3D(Solid3D::new()))
            .unwrap();
        let region = document
            .add_entity(EntityType::Region(Region::new()))
            .unwrap();
        let body = document.add_entity(EntityType::Body(Body::new())).unwrap();
        let handles = [solid, region, body];
        let blobs = handles.iter().map(|h| (h.value(), sab.clone())).collect();
        assert_eq!(
            super::attach_acds_record_blobs(&mut document, blobs, defer),
            3
        );
        (document, handles)
    }

    /// `(point_of_reference, reference_point_deferred, reference_point())` of
    /// the solid, region and body.
    fn reference_state(
        document: &crate::document::CadDocument,
        handles: &[crate::Handle; 3],
    ) -> [(Vector3, bool, Vector3); 3] {
        handles.map(|h| match document.get_entity(h) {
            Some(EntityType::Solid3D(e)) => (
                e.point_of_reference,
                e.acis_data.reference_point_deferred,
                e.reference_point(),
            ),
            Some(EntityType::Region(e)) => (
                e.point_of_reference,
                e.acis_data.reference_point_deferred,
                e.reference_point(),
            ),
            Some(EntityType::Body(e)) => (
                e.point_of_reference,
                e.acis_data.reference_point_deferred,
                e.reference_point(),
            ),
            other => panic!("expected a modeler entity, got {other:?}"),
        })
    }

    #[test]
    fn acds_attach_derives_the_reference_point_by_default() {
        let (document, handles) = attached_modelers(false);
        for (field, deferred, point) in reference_state(&document, &handles) {
            assert_eq!(field, CENTRE);
            assert!(!deferred);
            assert_eq!(point, CENTRE);
        }
    }

    #[test]
    fn acds_attach_defers_the_reference_point_when_asked() {
        let (document, handles) = attached_modelers(true);
        for (field, deferred, point) in reference_state(&document, &handles) {
            // The blob was stored but not parsed for a centre; asking derives it.
            assert_eq!(field, Vector3::ZERO);
            assert!(deferred);
            assert_eq!(point, CENTRE);
        }
        match document.get_entity(handles[0]) {
            Some(EntityType::Solid3D(e)) => assert_eq!(e.acis_data.sab_data, centred_sab()),
            other => panic!("expected a solid, got {other:?}"),
        }
    }

    #[test]
    fn a_stored_anchor_wins_over_the_deferred_point() {
        let mut document = crate::document::CadDocument::new();
        let mut solid = Solid3D::new();
        solid.point_of_reference = Vector3::new(1.0, 2.0, 3.0);
        let handle = document.add_entity(EntityType::Solid3D(solid)).unwrap();
        let blobs = vec![(handle.value(), centred_sab())];
        assert_eq!(
            super::attach_acds_record_blobs(&mut document, blobs, true),
            1
        );
        match document.get_entity(handle) {
            Some(EntityType::Solid3D(e)) => {
                assert_eq!(e.point_of_reference, Vector3::new(1.0, 2.0, 3.0));
                assert!(!e.acis_data.reference_point_deferred);
                assert_eq!(e.reference_point(), Vector3::new(1.0, 2.0, 3.0));
            }
            other => panic!("expected a solid, got {other:?}"),
        }
    }

    #[test]
    fn moving_a_deferred_modeler_gives_the_moved_centre() {
        use crate::entities::modification::{transform, translate};

        fn close(a: Vector3, b: Vector3) {
            assert!(
                (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9,
                "{a:?} != {b:?}"
            );
        }
        fn apply(document: &mut crate::document::CadDocument, handle: crate::Handle, op: u8) {
            let offset = Vector3::new(10.0, -3.0, 0.5);
            let turn = Transform::from_rotation(Vector3::new(0.0, 0.0, 1.0), 0.5)
                .then(&Transform::from_translation(offset));
            match document.get_entity_mut(handle) {
                Some(EntityType::Solid3D(e)) if op == 0 => translate::translate_solid3d(e, offset),
                Some(EntityType::Solid3D(e)) => transform::transform_solid3d(e, &turn),
                Some(EntityType::Region(e)) if op == 0 => translate::translate_region(e, offset),
                Some(EntityType::Region(e)) => transform::transform_region(e, &turn),
                Some(EntityType::Body(e)) if op == 0 => translate::translate_body(e, offset),
                Some(EntityType::Body(e)) => transform::transform_body(e, &turn),
                other => panic!("expected a modeler entity, got {other:?}"),
            }
        }

        // op 0 translates, op 1 rotates then translates. The eager read is the
        // reference: its field is the moved centre.
        for op in [0u8, 1] {
            let (mut eager, eager_handles) = attached_modelers(false);
            let (mut deferred, deferred_handles) = attached_modelers(true);
            for (e, d) in eager_handles.iter().zip(deferred_handles.iter()) {
                apply(&mut eager, *e, op);
                apply(&mut deferred, *d, op);
            }
            let want = reference_state(&eager, &eager_handles);
            let got = reference_state(&deferred, &deferred_handles);
            for ((want_field, _, _), (field, flag, point)) in want.into_iter().zip(got) {
                assert_ne!(want_field, CENTRE);
                assert!(!flag, "a moved entity is resolved");
                close(field, want_field);
                close(point, want_field);
            }
        }
    }

    /// The 48-byte segment header: `AC D5`, the name, u32 segment index,
    /// u32 `is_blob01`, u32 `segsize`, u32 unknown, u32 version, u32 unknown,
    /// u32 `data_algn_offset`, u32 `objdata_algn_offset`, eight `0x55`.
    fn segment(
        name: &[u8; 6],
        id: u32,
        is_blob: u32,
        align: (u32, u32),
        content: &[u8],
    ) -> Vec<u8> {
        let mut seg = vec![0xAC, 0xD5];
        seg.extend_from_slice(name);
        for value in [id, is_blob, 0, 0, 1, 0, align.0, align.1] {
            seg.extend_from_slice(&value.to_le_bytes());
        }
        seg.extend_from_slice(&[0x55; 8]);
        seg.extend_from_slice(content);
        seg.resize(seg.len().div_ceil(16) * 16, 0x70);
        let size = seg.len() as u32;
        seg[16..20].copy_from_slice(&size.to_le_bytes());
        seg
    }

    fn words(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|value| value.to_le_bytes()).collect()
    }

    /// A datastore as saved: the 128-byte `jard` header (segment-index offset
    /// at 24, entry count at 32, schema-index and data-index segments at 36
    /// and 40), the segments, then the segment index (segment 1): per index a
    /// u64 offset and u32 size, a free index all zero.
    fn datastore(segments: &[(u32, Vec<u8>)], schema_index: u32, data_index: u32) -> Vec<u8> {
        let count = segments.iter().map(|(id, _)| *id).max().unwrap() as usize + 1;
        let mut buf = vec![0u8; 128];
        let mut index = vec![(0u64, 0u32); count];
        for (id, bytes) in segments {
            index[*id as usize] = (buf.len() as u64, bytes.len() as u32);
            buf.extend_from_slice(bytes);
        }
        let at = buf.len() as u32;
        let mut table = Vec::new();
        for (offset, size) in &index {
            table.extend_from_slice(&offset.to_le_bytes());
            table.extend_from_slice(&size.to_le_bytes());
        }
        buf.extend_from_slice(&segment(b"segidx", 1, 0, (0, 0), &table));
        buf[..4].copy_from_slice(b"jard");
        let fields = [
            (24, at),
            (32, count as u32),
            (36, schema_index),
            (40, data_index),
        ];
        for (field, value) in fields {
            buf[field..field + 4].copy_from_slice(&value.to_le_bytes());
        }
        buf
    }

    /// A schema index whose names start at its data alignment offset.
    fn schema_index(id: u32, names: &[&[u8]]) -> Vec<u8> {
        let mut content = words(&[names.len() as u32, 0]);
        content.resize(48, 0x73);
        content.extend_from_slice(&(names.len() as u32).to_le_bytes());
        for name in names {
            content.extend_from_slice(name);
            content.push(0);
        }
        segment(b"schidx", id, 0, (6, 0), &content)
    }

    /// A data index: per record (segment, entry offset, schema).
    fn data_index(id: u32, records: &[(u32, u32, u32)]) -> Vec<u8> {
        let mut content = words(&[records.len() as u32, 0]);
        for (segment, entry, schema) in records {
            content.extend_from_slice(&words(&[*segment, *entry, *schema]));
        }
        segment(b"datidx", id, 0, (0, 0), &content)
    }

    /// A `_data_` segment: the 20-byte entries (u32 20, u32 1, u64 handle,
    /// u32 data offset), then the data from the object-data alignment offset.
    fn data_segment(id: u32, records: &[(u64, Vec<u8>)]) -> Vec<u8> {
        let table = (records.len() * 20).div_ceil(16) * 16;
        let mut content = Vec::new();
        let mut data = Vec::new();
        for (handle, value) in records {
            content.extend_from_slice(&words(&[20, 1]));
            content.extend_from_slice(&handle.to_le_bytes());
            content.extend_from_slice(&(data.len() as u32).to_le_bytes());
            data.extend_from_slice(value);
        }
        content.resize(table, 0x62);
        content.extend_from_slice(&data);
        segment(b"_data_", id, 0, (0, ((48 + table) / 16) as u32), &content)
    }

    /// Inline data: u32 size, then the bytes.
    fn inline(body: &[u8]) -> Vec<u8> {
        let mut value = (body.len() as u32).to_le_bytes().to_vec();
        value.extend_from_slice(body);
        value
    }

    /// A blob reference: marker, u64 total, u32 page count, u32 reference
    /// size, u32 page size, u32 last page size, two u32, then the pages.
    fn blob_reference(total: usize, pages: &[(u32, usize)]) -> Vec<u8> {
        let mut value = words(&[0xBB10_6BB1]);
        value.extend_from_slice(&(total as u64).to_le_bytes());
        value.extend_from_slice(&words(&[
            pages.len() as u32,
            32 + 8 * pages.len() as u32,
            pages[0].1 as u32,
            pages[pages.len() - 1].1 as u32,
            0,
            0,
        ]));
        for (id, size) in pages {
            value.extend_from_slice(&words(&[*id, *size as u32]));
        }
        value
    }

    /// A `blob01` page: u64 total, u64 start, u32 index, u32 count, u64 size,
    /// then the page's data.
    fn blob_page(
        id: u32,
        total: usize,
        start: usize,
        index: u32,
        count: u32,
        data: &[u8],
    ) -> Vec<u8> {
        let mut content = (total as u64).to_le_bytes().to_vec();
        content.extend_from_slice(&(start as u64).to_le_bytes());
        content.extend_from_slice(&words(&[index, count]));
        content.extend_from_slice(&(data.len() as u64).to_le_bytes());
        content.extend_from_slice(data);
        let mut page = segment(b"blob01", id, 1, (0, 0), &content);
        page[16..20].copy_from_slice(&48u32.to_le_bytes());
        page
    }

    fn body(label: &[u8]) -> Vec<u8> {
        let mut value = b"ASM BinaryFile".to_vec();
        value.extend_from_slice(label);
        value
    }

    /// Records are read through the data index, each body paired with the
    /// handle its own entry names, whatever order the records, segments and
    /// pages are in. Free records and records of another schema are skipped,
    /// and a body no record names attaches to nothing.
    #[test]
    fn acds_bodies_follow_the_datastore_indexes() {
        const ASM: u32 = 2;
        let inline_a = body(b" inline a");
        let inline_b = body(b" inline b, second data segment");
        let paged = body(b" split across two blob01 pages");
        let split = 11;
        let first = data_segment(
            7,
            &[
                (0x22, inline(b"\x89PNG thumbnail")),
                (
                    0xA2,
                    blob_reference(paged.len(), &[(9, split), (8, paged.len() - split)]),
                ),
                (0x91, inline(&inline_a)),
            ],
        );
        let second = data_segment(12, &[(0x95, inline(&inline_b))]);
        let buf = datastore(
            &[
                (
                    3,
                    schema_index(
                        3,
                        &[
                            b"AcDb_Thumbnail_Schema",
                            b"AcDbDs::LegacySchema",
                            b"AcDb3DSolid_ASM_Data",
                        ],
                    ),
                ),
                (
                    4,
                    data_index(
                        4,
                        &[(7, 0, 0), (12, 0, ASM), (0, 0, 0), (7, 20, ASM), (7, 40, ASM)],
                    ),
                ),
                (12, second),
                (6, segment(b"_data_", 6, 0, (0, 0), &inline(&body(b" stale")))),
                (8, blob_page(8, paged.len(), split, 1, 2, &paged[split..])),
                (7, first),
                (9, blob_page(9, paged.len(), 0, 0, 2, &paged[..split])),
            ],
            3,
            4,
        );
        assert_eq!(
            acds_modeler_bodies(&buf),
            vec![(0x95, inline_b), (0xA2, paged), (0x91, inline_a)]
        );
    }

    /// A record that does not read as the layout says is left out: a page
    /// whose frame disagrees with the reference, an entry of another size,
    /// a data index naming a segment that is not `_data_`. The others read.
    #[test]
    fn acds_records_that_do_not_read_keep_no_body() {
        let good = body(b" good");
        let paged = body(b" one page");
        let mut odd_entry = data_segment(5, &[(0x31, inline(&good))]);
        odd_entry[48..52].copy_from_slice(&24u32.to_le_bytes());
        let buf = datastore(
            &[
                (2, schema_index(2, &[b"AcDb3DSolid_ASM_Data"])),
                (
                    3,
                    data_index(3, &[(4, 0, 0), (4, 20, 0), (4, 40, 0), (5, 0, 0), (2, 0, 0)]),
                ),
                (
                    4,
                    data_segment(
                        4,
                        &[
                            (0x11, inline(&good)),
                            (0x12, blob_reference(paged.len(), &[(6, paged.len())])),
                            (0x13, inline(&good)[..8].to_vec()),
                        ],
                    ),
                ),
                (5, odd_entry),
                (6, blob_page(6, paged.len() + 1, 0, 0, 1, &paged)),
            ],
            2,
            3,
        );
        assert_eq!(acds_modeler_bodies(&buf), vec![(0x11, good)]);
        // No datastore header: nothing is read.
        assert!(acds_modeler_bodies(&buf[4..]).is_empty());
    }
}

/// Reconstruct a gradient hatch that was down-saved (R2000/R2004) as a solid
/// fill plus round-trip metadata: the two colours live in EED
/// (`GradientColor1ACI` / `GradientColor2ACI`) and the gradient type name in an
/// `ACAD_XREC_ROUNDTRIP` XRecord under the hatch's extension dictionary. When a
/// solid hatch carries this metadata but no live gradient block, rebuild
/// `gradient_color` so it renders as the intended gradient.
pub(crate) fn recover_roundtrip_gradients(document: &mut crate::document::CadDocument) {
    use crate::entities::EntityType;
    use crate::objects::ObjectType;
    use crate::types::{Color, Handle};

    const GRADIENT_NAMES: &[&str] = &[
        "SPHERICAL",
        "HEMISPHERICAL",
        "CURVED",
        "CYLINDER",
        "INVSPHERICAL",
        "INVHEMISPHERICAL",
        "INVCURVED",
        "INVCYLINDER",
        "LINEAR",
    ];

    // Phase 1 — collect (immutable): which hatches need a gradient, and what.
    let mut recovered: Vec<(Handle, u8, u8, String)> = Vec::new();
    for e in document.entities() {
        let EntityType::Hatch(h) = e else { continue };
        if h.gradient_color.enabled || !h.is_solid {
            continue;
        }
        let eed = &h.common.extended_data;
        // EED app names vary in case across versions (R14 upper-cases them:
        // GRADIENTCOLOR1ACI vs R2000's GradientColor1ACI), so match loosely.
        let aci = |app: &str| -> Option<u8> {
            eed.records()
                .iter()
                .find(|r| r.application_name.eq_ignore_ascii_case(app))
                .and_then(|r| {
                    r.values.iter().find_map(|v| match v {
                        crate::xdata::XDataValue::Integer16(n) => Some(*n as u8),
                        _ => None,
                    })
                })
        };
        let (Some(c1), Some(c2)) = (aci("GradientColor1ACI"), aci("GradientColor2ACI")) else {
            continue;
        };

        // A live down-saved gradient always carries an ACAD_XREC_ROUNDTRIP
        // XRecord (under the hatch's extension dictionary) alongside the
        // GradientColor*ACI EED. When only the EED survives — no extension
        // dictionary, or one without that XRecord — the colours are stale
        // metadata left behind by an edit that turned a gradient into a plain
        // solid fill (or recoloured it). Resurrecting a gradient then paints a
        // genuine single-colour solid hatch as a two-colour gradient, so treat
        // the missing round-trip XRecord as proof the hatch is really solid.
        //
        // Gradient type: the trailing name string in that same XRecord
        // (hatch xdict → "ACAD_XREC_ROUNDTRIP" → XRecord raw_data).
        let mut name = String::new();
        let mut has_roundtrip = false;
        if let Some(xd) = h.common.xdictionary_handle {
            if let Some(ObjectType::Dictionary(d)) = document.objects.get(&xd) {
                // R14 mis-sizes dictionary key strings, leaving trailing
                // garbage bytes ("ACAD_XREC_ROUNDTRIP\x03q0…"), so match by
                // prefix rather than equality.
                if let Some((_, xrec_h)) = d.entries.iter().find(|(k, _)| {
                    k.len() >= 19 && k[..19].eq_ignore_ascii_case("ACAD_XREC_ROUNDTRIP")
                }) {
                    has_roundtrip = true;
                    if let Some(ObjectType::XRecord(xr)) = document.objects.get(xrec_h) {
                        let ascii: String = xr
                            .raw_data
                            .iter()
                            .map(|&b| {
                                if (32..127).contains(&b) {
                                    b as char
                                } else {
                                    ' '
                                }
                            })
                            .collect();
                        let up = ascii.to_ascii_uppercase();
                        for cand in GRADIENT_NAMES {
                            if up.contains(cand) {
                                name = (*cand).to_string();
                                break;
                            }
                        }
                        // A DXF-sourced round-trip XRecord encodes the gradient
                        // type as a number in the 1004 binary (first LE int32 =
                        // 123 + gradient index in ACAD's dropdown order),
                        // not as an ASCII name; decode it when no name matched.
                        if name.is_empty() && xr.raw_data.len() >= 4 {
                            const BY_INDEX: &[&str] = &[
                                "LINEAR",
                                "CYLINDER",
                                "INVCYLINDER",
                                "SPHERICAL",
                                "HEMISPHERICAL",
                                "CURVED",
                                "INVSPHERICAL",
                                "INVHEMISPHERICAL",
                                "INVCURVED",
                            ];
                            let v = u32::from_le_bytes([
                                xr.raw_data[0],
                                xr.raw_data[1],
                                xr.raw_data[2],
                                xr.raw_data[3],
                            ]);
                            if (123..123 + BY_INDEX.len() as u32).contains(&v) {
                                name = BY_INDEX[(v - 123) as usize].to_string();
                            }
                        }
                    }
                }
            }
        }
        // No round-trip XRecord ⇒ the gradient EED is stale; keep it solid.
        if !has_roundtrip {
            continue;
        }
        if name.is_empty() {
            name = "LINEAR".to_string();
        }
        recovered.push((e.common().handle, c1, c2, name));
    }

    // Phase 2 — apply (mutable).
    for (handle, c1, c2, name) in recovered {
        if let Some(EntityType::Hatch(h)) = document.get_entity_mut(handle) {
            h.gradient_color.enabled = true;
            h.gradient_color.is_single_color = c1 == c2;
            h.gradient_color.colors = vec![
                crate::entities::hatch::GradientColorEntry {
                    value: 0.0,
                    color: Color::from_index(c1 as i16),
                    color_name: None,
                    book_name: None,
                },
                crate::entities::hatch::GradientColorEntry {
                    value: 1.0,
                    color: Color::from_index(c2 as i16),
                    color_name: None,
                    book_name: None,
                },
            ];
            h.gradient_color.name = name;
        }
    }
}

/// Recover an MTEXT background fill that a pre-R2004 (R2000/R14) save stored as
/// round-trip EED (`ACAD_MTEXT_BBRT` … `ACAD_MTEXT_BERT`) instead of the native
/// codes (90 flags / 63|421 colour / 45 scale / 441 transparency), which those
/// versions predate. ACAD/ODA render the fill from this metadata; without it
/// a down-saved dimension text (or any MTEXT) shows no background. Applied only
/// when the entity has no native fill flag. The colour is stored with the
/// ACAD "method byte" (0xC2 = RGB, 0xC3 = ACI, 0xC0/0xC1 = ByLayer/ByBlock).
pub(crate) fn recover_mtext_bg_roundtrip(document: &mut crate::document::CadDocument) {
    use crate::entities::EntityType;
    use crate::types::{Color, Handle};
    use crate::xdata::XDataValue;

    fn color_from_method(v: i32) -> Color {
        let u = v as u32;
        match (u >> 24) & 0xFF {
            0xC0 => Color::ByLayer,
            0xC1 => Color::ByBlock,
            0xC3 => Color::from_index((u & 0xFF) as i16),
            _ => Color::Rgb {
                r: ((u >> 16) & 0xFF) as u8,
                g: ((u >> 8) & 0xFF) as u8,
                b: (u & 0xFF) as u8,
            },
        }
    }

    let mut recovered: Vec<(Handle, i32, f64, Color, i32)> = Vec::new();
    for e in document.entities() {
        let EntityType::MText(m) = e else { continue };
        if m.background_fill_flags != 0 {
            continue; // native fill present — nothing to recover
        }
        let Some(rec) = m
            .common
            .extended_data
            .records()
            .iter()
            .find(|r| r.application_name.eq_ignore_ascii_case("ACAD"))
        else {
            continue;
        };
        let vals = &rec.values;
        let Some(begin) = vals
            .iter()
            .position(|v| matches!(v, XDataValue::String(s) if s == "ACAD_MTEXT_BBRT"))
        else {
            continue;
        };
        // Walk the BBRT…BERT block as (Integer16 code, value) pairs.
        let (mut flags, mut scale, mut color, mut transp) = (0i32, 1.0f64, Color::ByLayer, 0i32);
        let mut i = begin + 1;
        while i + 1 < vals.len() {
            if matches!(&vals[i], XDataValue::String(s) if s == "ACAD_MTEXT_BERT") {
                break;
            }
            if let XDataValue::Integer16(code) = vals[i] {
                match (code, &vals[i + 1]) {
                    (91, XDataValue::Integer32(v)) => flags = *v,
                    (46, XDataValue::Real(v)) => scale = *v,
                    (64, XDataValue::Integer32(v)) => color = color_from_method(*v),
                    (442, XDataValue::Integer32(v)) => transp = *v,
                    _ => {}
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        if flags != 0 {
            recovered.push((m.common.handle, flags, scale, color, transp));
        }
    }
    for (h, flags, scale, color, transp) in recovered {
        if let Some(EntityType::MText(m)) = document.get_entity_mut(h) {
            m.background_fill_flags = flags;
            m.background_scale = scale;
            m.background_color = color;
            m.background_transparency = transp;
        }
    }
}

#[cfg(test)]
mod section_recovery_tests {
    use super::*;

    fn ac15_info(offset: i64, size: i64) -> DwgFileHeaderInfo {
        DwgFileHeaderInfo {
            version_string: "AC1015".to_owned(),
            version: DwgVersion::AC15,
            acad_maintenance_version: 0,
            preview_address: 0,
            dwg_version: 0,
            app_release_version: 0,
            code_page: 0,
            security_type: 0,
            summary_info_addr: 0,
            vba_project_addr: 0,
            ac21_metadata: None,
            ac21_header_crc: None,
            ac21_unknown_key: None,
            ac21_compressed_data_crc: None,
            page_records: HashMap::new(),
            section_descriptors: Vec::new(),
            section_locators: HashMap::from([("AcDb:Header".to_owned(), (offset, size))]),
            objects_base_offset: 0,
            is_ac18_format: false,
        }
    }

    fn ac18_info(page_number: i64) -> DwgFileHeaderInfo {
        DwgFileHeaderInfo {
            version_string: "AC1018".to_owned(),
            version: DwgVersion::AC18,
            acad_maintenance_version: 0,
            preview_address: 0,
            dwg_version: 0,
            app_release_version: 0,
            code_page: 0,
            security_type: 0,
            summary_info_addr: 0,
            vba_project_addr: 0,
            ac21_metadata: None,
            ac21_header_crc: None,
            ac21_unknown_key: None,
            ac21_compressed_data_crc: None,
            page_records: HashMap::new(),
            section_descriptors: vec![DwgSectionInfo {
                name: "AcDb:Header".to_owned(),
                compressed_size: 8,
                decompressed_size: 8,
                encrypted: 0,
                hash_code: 0,
                encoding: 0,
                page_count: 1,
                pages: vec![DwgPageCrcInfo {
                    page_number,
                    offset: 0,
                    size: 0,
                    decompressed_size: 8,
                    compressed_size: 0,
                    checksum: 0,
                    crc: 0,
                }],
            }],
            section_locators: HashMap::new(),
            objects_base_offset: 0,
            is_ac18_format: true,
        }
    }

    #[test]
    fn ac15_failsafe_keeps_readable_prefix_after_truncated_section() {
        let stream = Cursor::new(vec![0, 1, 2, 3, 4]);
        let mut reader = DwgReader::from_stream_with_options(stream, DwgReadOptions::failsafe());
        let bytes = reader
            .get_section_buffer("AcDb:Header", &ac15_info(2, 10))
            .unwrap();
        assert_eq!(bytes, vec![2, 3, 4]);
        assert_eq!(reader.pending_diagnostics.len(), 1);
        assert_eq!(reader.pending_diagnostics[0].code, "section-read-truncated");
    }

    #[test]
    fn ac15_strict_mode_still_rejects_truncated_section() {
        let stream = Cursor::new(vec![0, 1, 2, 3, 4]);
        let mut reader = DwgReader::from_stream(stream);
        assert!(reader
            .get_section_buffer("AcDb:Header", &ac15_info(2, 10))
            .is_err());
    }

    #[test]
    fn ac18_failsafe_skips_missing_page_and_preserves_section_offsets() {
        let mut reader = DwgReader::from_stream_with_options(
            Cursor::new(Vec::new()),
            DwgReadOptions::failsafe(),
        );
        let bytes = reader
            .get_section_buffer("AcDb:Header", &ac18_info(7))
            .unwrap();
        assert_eq!(bytes, vec![0; 8]);
        assert_eq!(reader.pending_diagnostics.len(), 1);
        assert_eq!(reader.pending_diagnostics[0].code, "section-page-skipped");
    }

    #[test]
    fn ac18_strict_mode_still_rejects_missing_page() {
        let mut reader = DwgReader::from_stream(Cursor::new(Vec::new()));
        assert!(reader
            .get_section_buffer("AcDb:Header", &ac18_info(7))
            .is_err());
    }
}

/// The current text and dimension styles' names from their handles: a DWG stores only the handles, and the
/// names stayed "Standard", so a drawing read from DWG lost its current styles (and wrote them as Standard).
/// Ported from CADCraft's DWG bridge (storytold/cadcraft #1051; MIT OR Apache-2.0).
fn current_style_names(doc: &mut crate::document::CadDocument) {
    let h = doc.header.current_text_style_handle;
    if let Some(s) = doc
        .text_styles
        .iter()
        .find(|s| !h.is_null() && s.handle == h)
    {
        doc.header.current_text_style_name = s.name.clone();
    }
    let h = doc.header.current_dimstyle_handle;
    if let Some(s) = doc
        .dim_styles
        .iter()
        .find(|s| !h.is_null() && s.handle == h)
    {
        doc.header.current_dimstyle_name = s.name.clone();
    }
}
