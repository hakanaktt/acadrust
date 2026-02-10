//! DWG reader orchestrator — the main entry point for reading DWG files.
//!
//! Mirrors ACadSharp's `DwgReader` class.  Reads the file header,
//! section data, object map, objects, and builds the final [`CadDocument`].
//!
//! # Usage
//!
//! ```rust,ignore
//! use acadrust::io::dwg::reader::DwgReader;
//!
//! let doc = DwgReader::from_file("sample.dwg")?.read()?;
//! ```

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};

use crate::classes::DxfClassCollection;
use crate::document::CadDocument;
use crate::error::{DxfError, Result};
use crate::notification::{Notification, NotificationType};
use crate::preview::DwgPreview;
use crate::summary_info::CadSummaryInfo;
use crate::types::DxfVersion;

use super::super::builder::DwgDocumentBuilder;
use super::super::compression::lz77_ac18::Lz77Ac18Decompressor;
use super::super::compression::lz77_ac21::Lz77Ac21Decompressor;
use super::super::compression::Decompressor;
use super::super::constants::{ac18, ac21, section_names};
use super::super::encryption;
use super::super::file_header::{
    DwgFileHeader, DwgFileHeaderAC15, DwgFileHeaderAC18, DwgFileHeaderAC21,
    DwgLocalSectionMap, DwgSectionDescriptor, DwgSectionLocatorRecord,
};
use super::super::header_handles::DwgHeaderHandlesCollection;
use super::super::reed_solomon;
use super::{
    DwgAppInfoReader, DwgClassesReader, DwgHandleReader, DwgHeaderReader,
    DwgPreviewReader, DwgSummaryInfoReader,
};
use super::object_reader::DwgObjectReader;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration options for the DWG reader.
///
/// Mirrors ACadSharp's `DwgReaderConfiguration`.
#[derive(Debug, Clone)]
pub struct DwgReaderConfiguration {
    /// When `true`, parse errors within individual objects are caught and
    /// reported as notifications instead of aborting the entire read.
    ///
    /// Default: `false` (strict mode).
    pub failsafe: bool,

    /// When `true`, keep entities whose type is unknown rather than skipping
    /// them.
    pub keep_unknown_entities: bool,
}

impl Default for DwgReaderConfiguration {
    fn default() -> Self {
        Self {
            failsafe: false,
            keep_unknown_entities: false,
        }
    }
}

// ---------------------------------------------------------------------------
// DwgReader
// ---------------------------------------------------------------------------

/// DWG file reader — reads the binary DWG format and produces a [`CadDocument`].
///
/// # Architecture
///
/// The read pipeline is:
///
/// 1. Read the 6-byte version string from the file start.
/// 2. Read the file header (AC15 / AC18 / AC21 format).
/// 3. Read each section into a decompressed byte buffer:
///    - Summary info (AC18+)
///    - Header (system variables + handles)
///    - Classes (DXF class definitions)
///    - Handles (object map: handle → file offset)
///    - Preview (thumbnail)
///    - App info (AC18+)
/// 4. Read all objects using the object reader + handle map.
/// 5. Build the document using [`DwgDocumentBuilder`].
pub struct DwgReader<R: Read + Seek> {
    /// Underlying byte stream.
    reader: R,

    /// DWG version detected from the file.
    version: DxfVersion,

    /// User configuration.
    config: DwgReaderConfiguration,

    /// Parsed file header.
    file_header: DwgFileHeader,

    /// Notifications collected during reading.
    notifications: Vec<Notification>,
}

impl DwgReader<BufReader<File>> {
    /// Open a DWG file by path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref()).map_err(|e| {
            DxfError::Io(e)
        })?;
        let reader = BufReader::new(file);
        Self::from_reader(reader)
    }
}

impl<R: Read + Seek> DwgReader<R> {
    /// Create a DWG reader from any seekable byte stream.
    pub fn from_reader(mut reader: R) -> Result<Self> {
        // Read the 6-byte version string (e.g. "AC1015").
        let mut version_buf = [0u8; 6];
        reader.read_exact(&mut version_buf)?;
        let version_str = std::str::from_utf8(&version_buf)
            .map_err(|_| DxfError::InvalidFormat("Invalid version string encoding".into()))?;

        let version = DxfVersion::parse(version_str)
            .ok_or_else(|| DxfError::UnsupportedVersion(version_str.to_string()))?;

        // Create the appropriate file header structure.
        let file_header = DwgFileHeader::create(version)?;

        Ok(Self {
            reader,
            version,
            config: DwgReaderConfiguration::default(),
            file_header,
            notifications: Vec::new(),
        })
    }

    /// Set configuration options.
    pub fn with_config(mut self, config: DwgReaderConfiguration) -> Self {
        self.config = config;
        self
    }

    /// Read the entire DWG file and return a [`CadDocument`].
    ///
    /// This is the main entry point for reading DWG files.
    pub fn read(mut self) -> Result<CadDocument> {
        // Step 1: Read the file header.
        self.read_file_header()?;

        // Step 2: Read section data.
        let summary_info = self.read_summary_info();
        let (header_vars, header_handles) = self.read_header()?;
        let _acad_maintenance_version = self.file_header.maintenance_version() as i32;
        let classes = self.read_classes()?;
        let handle_map = self.read_handles()?;
        let _preview = self.read_preview();
        let _app_info = self.read_app_info();
        // Step 3: Build the handle queue for the object reader.
        let mut handle_queue: VecDeque<u64> = VecDeque::new();

        // Seed with header handles.
        let valid_header_handles = header_handles.get_valid_handles();
        for h in &valid_header_handles {
            handle_queue.push_back(*h);
        }

        // Step 4: Read all objects.
        // For AC15 (R13-R2000), objects are scattered in the raw file and the
        // handle map contains absolute file offsets. We load the entire file.
        // For AC18+, objects are in a decompressed AcDb:AcDbObjects section.
        let objects_data = match &self.file_header {
            DwgFileHeader::AC15(_) => {
                self.reader.seek(SeekFrom::Start(0))?;
                let mut buf = Vec::new();
                self.reader.read_to_end(&mut buf)?;
                buf
            }
            _ => self.get_section_stream(section_names::ACDB_OBJECTS)?,
        };
        let class_entries: Vec<_> = classes.iter().cloned().collect();
        let mut object_reader = DwgObjectReader::new(
            self.version,
            objects_data,
            handle_queue,
            handle_map,
            &class_entries,
        );
        object_reader.failsafe = self.config.failsafe;
        object_reader.read()?;
        // Step 5: Build the document.
        let mut builder = DwgDocumentBuilder::new(self.version);
        builder.header_handles = header_handles;
        builder.document.header = header_vars;
        builder.keep_unknown_entities = self.config.keep_unknown_entities;

        // Store summary info if available.
        // CadDocument doesn't embed summary info directly; it is available
        // externally via the reader. For now we skip storing it.
        let _ = summary_info;
        builder.document.classes = classes;

        builder.add_templates(object_reader.templates);
        builder.build_document();

        // Collect all notifications.
        let doc = builder.document;
        // Propagate notifications if the document supports them.
        // For now we just drop them.
        let _ = builder.notifications;
        let _ = object_reader.notifications;
        let _ = self.notifications;

        Ok(doc)
    }

    // ------------------------------------------------------------------
    // File header reading
    // ------------------------------------------------------------------

    /// Read the file header based on the detected version.
    fn read_file_header(&mut self) -> Result<()> {
        match self.version {
            DxfVersion::AC1012 | DxfVersion::AC1014 | DxfVersion::AC1015 => {
                self.read_file_header_ac15()?;
            }
            DxfVersion::AC1018 => {
                self.read_file_header_ac18()?;
            }
            DxfVersion::AC1021 => {
                self.read_file_header_ac21()?;
            }
            DxfVersion::AC1024 | DxfVersion::AC1027 | DxfVersion::AC1032 => {
                // R2010+ uses the same format as AC18 with minor differences.
                self.read_file_header_ac18()?;
            }
            _ => {
                return Err(DxfError::UnsupportedVersion(self.version.to_string()));
            }
        }
        Ok(())
    }

    /// Read AC15 (R13–R2000) file header.
    ///
    /// Format:
    /// - 6 bytes: version string (already read)
    /// - 7 bytes: padding/flags
    /// - 4 bytes: ACAD maintenance version
    /// - 1 byte: unknown
    /// - 4 bytes: preview address
    /// - 1 byte: DWG version
    /// - 1 byte: application release version
    /// - 2 bytes: code page
    /// - 4 bytes: number of section locator records
    /// - N × 12 bytes: section locator records (number, seeker, size)
    /// - 2 bytes: CRC
    /// - 16 bytes: end sentinel
    fn read_file_header_ac15(&mut self) -> Result<()> {
        self.reader.seek(SeekFrom::Start(6))?;

        // 0x06: 7 bytes — six bytes of 0x00 (in R14: 5 zeros + ACADMAINTVER + 0x01).
        let mut padding = [0u8; 7];
        self.reader.read_exact(&mut padding)?;
        // Extract maintenance version from byte 5 of the padding (offset 0x0B).
        let maintenance_ver = padding[5];

        // 0x0D: Preview image address (4 bytes, i32 LE).
        let preview_addr = self.reader.read_i32::<LittleEndian>()? as i64;

        // 0x11: 2 undocumented bytes.
        let mut _undocumented = [0u8; 2];
        self.reader.read_exact(&mut _undocumented)?;

        // 0x13: Drawing code page (2 bytes, u16 LE).
        let code_page = self.reader.read_u16::<LittleEndian>()?;

        // 0x15: Number of section locator records (4 bytes, i32 LE).
        let num_records = self.reader.read_i32::<LittleEndian>()? as usize;

        // Read section locator records.
        // Each record: Number (1 byte) + Seeker (4 bytes i32) + Size (4 bytes i32) = 9 bytes.
        let mut records = HashMap::new();
        for _i in 0..num_records {
            let number = self.reader.read_u8()? as i32;
            let seeker = self.reader.read_i32::<LittleEndian>()? as i64;
            let size = self.reader.read_i32::<LittleEndian>()? as i64;
            records.insert(
                number as usize,
                DwgSectionLocatorRecord::new(number, seeker, size),
            );
        }

        // CRC — read but don't validate for now.
        let _crc = self.reader.read_u16::<LittleEndian>()?;

        // End sentinel — skip 16 bytes.
        let mut sentinel = [0u8; 16];
        self.reader.read_exact(&mut sentinel)?;

        // Populate file header.
        let header = DwgFileHeaderAC15 {
            version: self.version,
            preview_address: preview_addr,
            maintenance_version: maintenance_ver,
            drawing_code_page: code_page,
            records,
        };

        self.file_header = DwgFileHeader::AC15(header);
        Ok(())
    }

    /// Read AC18 (R2004+) file header.
    ///
    /// Format:
    /// - 6 bytes: version string (already read)
    /// - Various metadata fields
    /// - Encrypted header at offset 0x80 (108 bytes)
    /// - Page map and section map
    fn read_file_header_ac18(&mut self) -> Result<()> {
        self.reader.seek(SeekFrom::Start(6))?;

        // Skip padding.
        let mut skip = [0u8; 5];
        self.reader.read_exact(&mut skip)?;

        // Maintenance version.
        let maintenance_ver = self.reader.read_u8()?;

        // Skip 1 byte.
        self.reader.read_u8()?;

        // Preview address.
        let preview_addr = self.reader.read_i32::<LittleEndian>()? as i64;

        // Application version.
        let dwg_version = self.reader.read_u8()?;
        let app_release_version = self.reader.read_u8()?;

        // Code page.
        let code_page = self.reader.read_u16::<LittleEndian>()?;

        // 3 padding bytes.
        let mut filler = [0u8; 3];
        self.reader.read_exact(&mut filler)?;

        // Security type.
        let security_type = self.reader.read_i32::<LittleEndian>()? as i64;

        // Unknown value.
        let _unknown = self.reader.read_i32::<LittleEndian>()?;

        // Summary info address.
        let summary_info_addr = self.reader.read_i32::<LittleEndian>()? as i64;

        // VBA project address.
        let vba_project_addr = self.reader.read_i32::<LittleEndian>()? as i64;

        // Skip padding to 0x80.
        let current = self.reader.stream_position()? as i64;
        if current < 0x80 {
            let skip_bytes = 0x80 - current;
            let mut skip_buf = vec![0u8; skip_bytes as usize];
            self.reader.read_exact(&mut skip_buf)?;
        }

        // Read encrypted header (108 bytes at offset 0x80).
        let mut encrypted_header = [0u8; ac18::ENCRYPTED_HEADER_SIZE];
        self.reader.read_exact(&mut encrypted_header)?;

        // Decrypt the header metadata.
        let decrypted_header = self.decrypt_file_header_ac18(&encrypted_header);

        // Build the file header struct.
        let mut header = DwgFileHeaderAC18::new(self.version);
        header.preview_address = preview_addr;
        header.maintenance_version = maintenance_ver;
        header.drawing_code_page = code_page;
        header.dwg_version = dwg_version;
        header.app_release_version = app_release_version;
        header.summary_info_addr = summary_info_addr;
        header.security_type = security_type;
        header.vba_project_addr = vba_project_addr;

        // Apply decrypted values.
        header.root_tree_node_gap = decrypted_header.root_tree_node_gap;
        header.gap_array_size = decrypted_header.gap_array_size;
        header.crc_seed = decrypted_header.crc_seed;
        header.last_page_id = decrypted_header.last_page_id;
        header.last_section_addr = decrypted_header.last_section_addr;
        header.second_header_addr = decrypted_header.second_header_addr;
        header.gap_amount = decrypted_header.gap_amount;
        header.section_amount = decrypted_header.section_amount;
        header.section_page_map_id = decrypted_header.section_page_map_id;
        header.page_map_address = decrypted_header.page_map_address;
        header.section_map_id = decrypted_header.section_map_id;
        header.section_array_page_size = decrypted_header.section_array_page_size;

        // Read the page map and section map.
        self.read_page_map_ac18(&mut header)?;
        self.read_section_map_ac18(&mut header)?;

        self.file_header = DwgFileHeader::AC18(header);
        Ok(())
    }

    /// Read AC21 (R2007) file header.
    ///
    /// AC21 uses Reed-Solomon encoding for the file header data.
    fn read_file_header_ac21(&mut self) -> Result<()> {
        // Read the basic AC18 header first.
        self.read_file_header_ac18()?;

        // The AC21 file has additional Reed-Solomon encoded data at offset 0x80.
        // Re-read it from the file and decode.
        self.reader.seek(SeekFrom::Start(0x80))?;

        let mut rs_data = vec![0u8; ac21::RS_ENCODED_BLOCK_SIZE];
        self.reader.read_exact(&mut rs_data)?;

        // Decode the RS block.
        let decoded = reed_solomon::decode(
            &rs_data,
            ac21::DECOMPRESSED_HEADER_SIZE,
            3,
            ac21::RS_BLOCK_SIZE,
        );

        // Decompress the decoded block.
        let decompressor = Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&decoded, ac21::DECOMPRESSED_HEADER_SIZE)?;

        // Parse the decompressed metadata.
        self.parse_ac21_compressed_metadata(&decompressed)?;

        // Wrap the AC18 header into an AC21 header.
        if let DwgFileHeader::AC18(ac18_header) = std::mem::replace(
            &mut self.file_header,
            DwgFileHeader::create(self.version)?,
        ) {
            self.file_header = DwgFileHeader::AC21(DwgFileHeaderAC21 {
                base: ac18_header,
                compressed_metadata: None,
            });
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // AC18 encrypted file header data
    // ------------------------------------------------------------------

    /// Decrypt the AC18 file header metadata block (108 bytes).
    fn decrypt_file_header_ac18(&self, data: &[u8; ac18::ENCRYPTED_HEADER_SIZE]) -> DecryptedAC18HeaderData {

        // XOR decrypt with a rotating mask.
        let mut decrypted = [0u8; ac18::ENCRYPTED_HEADER_SIZE];
        let rand_seed = 1u32;
        let mut rand_state = rand_seed;

        for i in 0..ac18::ENCRYPTED_HEADER_SIZE {
            // Linear congruential generator matching ACadSharp's XOR decryption.
            rand_state = rand_state.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
            decrypted[i] = data[i] ^ ((rand_state >> 16) as u8);
        }

        // Parse decrypted data.
        let mut c = Cursor::new(&decrypted[..]);
        let _file_id_buf = {
            let mut b = [0u8; 12];
            c.read_exact(&mut b).unwrap_or_default();
            b
        };
        let _x00 = c.read_i32::<LittleEndian>().unwrap_or(0);
        let _x04 = c.read_i32::<LittleEndian>().unwrap_or(0);
        let _x08 = c.read_i32::<LittleEndian>().unwrap_or(0);
        let root_tree_node_gap = c.read_i32::<LittleEndian>().unwrap_or(0);
        let _lowermost_left_tree_node_gap = c.read_i32::<LittleEndian>().unwrap_or(0);
        let _lowermost_right_tree_node_gap = c.read_i32::<LittleEndian>().unwrap_or(0);
        let _unknown1 = c.read_i32::<LittleEndian>().unwrap_or(0);
        let last_page_id = c.read_i32::<LittleEndian>().unwrap_or(0);
        let last_section_addr = c.read_u64::<LittleEndian>().unwrap_or(0);
        let second_header_addr = c.read_u64::<LittleEndian>().unwrap_or(0);
        let gap_amount = c.read_u32::<LittleEndian>().unwrap_or(0);
        let section_amount = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x44: 0x20 (padding)
        let _x20 = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x48: 0x80 (padding)
        let _x80 = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x4C: 0x40 (padding)
        let _x40 = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x50: Section Page Map Id
        let section_page_map_id = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x54: Section Page Map address (add 0x100 to get the actual file position)
        let page_map_address = c.read_u64::<LittleEndian>().unwrap_or(0) + 0x100;
        // 0x5C: Section Map Id
        let section_map_id = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x60: Section page array size
        let section_array_page_size = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x64: Gap array size
        let gap_array_size = c.read_u32::<LittleEndian>().unwrap_or(0);
        // 0x68: CRC32
        let crc_seed = c.read_u32::<LittleEndian>().unwrap_or(0);

        DecryptedAC18HeaderData {
            root_tree_node_gap,
            gap_array_size,
            crc_seed,
            last_page_id,
            last_section_addr,
            second_header_addr,
            gap_amount,
            section_amount,
            section_page_map_id,
            page_map_address,
            section_map_id,
            section_array_page_size,
        }
    }

    // ------------------------------------------------------------------
    // AC18 page map and section map reading
    // ------------------------------------------------------------------

    /// Read the page map for AC18.
    ///
    /// The page map is itself a compressed data section at the page map address.
    /// It contains (section_number, size) pairs that map section IDs to file positions.
    ///
    /// Mirrors ACadSharp `DwgReader.readFileHeaderAC18` — "Read page map of the file" region.
    fn read_page_map_ac18(&mut self, header: &mut DwgFileHeaderAC18) -> Result<()> {
        let page_map_addr = header.page_map_address;
        if page_map_addr == 0 {
            return Ok(());
        }

        // Seek to the page map address (already includes +0x100 offset).
        self.reader.seek(SeekFrom::Start(page_map_addr))?;

        // Read the 20-byte page header (unencrypted for page map).
        let _section_type = self.reader.read_i32::<LittleEndian>()?;    // 0x41630E3B
        let decompressed_size = self.reader.read_i32::<LittleEndian>()? as usize;
        let compressed_size = self.reader.read_i32::<LittleEndian>()? as usize;
        let _compression_type = self.reader.read_i32::<LittleEndian>()?; // 0x02
        let _checksum = self.reader.read_i32::<LittleEndian>()?;

        // Read and decompress the page map data.
        let decompressed = if compressed_size > 0 && decompressed_size > 0 {
            let mut compressed_data = vec![0u8; compressed_size];
            self.reader.read_exact(&mut compressed_data)?;
            let decompressor = Lz77Ac18Decompressor;
            decompressor.decompress(&compressed_data, decompressed_size)?
        } else {
            return Ok(());
        };

        // Parse page records from the decompressed data.
        // Each record: section_number (i32) + size (i32) = 8 bytes.
        // Seeker is a running total starting at 0x100.
        let mut cursor = Cursor::new(&decompressed);
        let mut total = 0x100i64;

        while (cursor.position() as usize) < decompressed.len() {
            let section_number = match cursor.read_i32::<LittleEndian>() {
                Ok(v) => v,
                Err(_) => break,
            };
            let size = match cursor.read_i32::<LittleEndian>() {
                Ok(v) => v as i64,
                Err(_) => break,
            };

            if section_number >= 0 {
                header.records.insert(
                    section_number as usize,
                    DwgSectionLocatorRecord::new(section_number, total, size),
                );
            } else {
                // Negative section number = gap. Read 4 extra i32 values.
                let _ = cursor.read_i32::<LittleEndian>(); // Parent
                let _ = cursor.read_i32::<LittleEndian>(); // Left
                let _ = cursor.read_i32::<LittleEndian>(); // Right
                let _ = cursor.read_i32::<LittleEndian>(); // 0x00
            }

            total += size;
        }

        Ok(())
    }

    /// Read the section map for AC18.
    ///
    /// The section map is stored in the page identified by `section_map_id`.
    /// It contains section descriptors (name, compressed size, page count, etc.)
    /// and local section maps (page numbers referencing the page map records).
    ///
    /// Mirrors ACadSharp `DwgReader.readFileHeaderAC18` — "Read the data section map" region.
    fn read_section_map_ac18(&mut self, header: &mut DwgFileHeaderAC18) -> Result<()> {
        let section_map_id = header.section_map_id;
        if section_map_id == 0 {
            return Ok(());
        }

        // Find the section map page in the already-read page map records.
        let section_map_record = header.records.get(&(section_map_id as usize)).cloned()
            .ok_or_else(|| {
                DxfError::InvalidFormat(format!(
                    "Section map ID {} not found in page map records",
                    section_map_id
                ))
            })?;

        // Seek to the section map page and read its page header.
        self.reader.seek(SeekFrom::Start(section_map_record.seeker as u64))?;

        let _section_type = self.reader.read_i32::<LittleEndian>()?;    // 0x4163003B
        let decompressed_size = self.reader.read_i32::<LittleEndian>()? as usize;
        let compressed_size = self.reader.read_i32::<LittleEndian>()? as usize;
        let _compression_type = self.reader.read_i32::<LittleEndian>()?; // 0x02
        let _checksum = self.reader.read_i32::<LittleEndian>()?;

        // Read and decompress the section map data.
        let decompressed = if compressed_size > 0 && decompressed_size > 0 {
            let mut compressed_data = vec![0u8; compressed_size];
            self.reader.read_exact(&mut compressed_data)?;
            let decompressor = Lz77Ac18Decompressor;
            decompressor.decompress(&compressed_data, decompressed_size)?
        } else {
            return Ok(());
        };

        self.parse_section_descriptors_ac18(header, &decompressed)?;

        Ok(())
    }

    /// Parse section descriptors from decompressed section map data (AC18).
    ///
    /// Mirrors ACadSharp's section descriptor parsing in `readFileHeaderAC18`.
    fn parse_section_descriptors_ac18(
        &self,
        header: &mut DwgFileHeaderAC18,
        data: &[u8],
    ) -> Result<()> {
        if data.len() < 20 {
            return Ok(());
        }

        let mut cursor = Cursor::new(data);

        // 0x00: Number of section descriptions
        let num_descriptions = cursor.read_i32::<LittleEndian>().unwrap_or(0);
        // 0x04: 0x02
        let _x04 = cursor.read_i32::<LittleEndian>().unwrap_or(0);
        // 0x08: 0x00007400
        let _x08 = cursor.read_i32::<LittleEndian>().unwrap_or(0);
        // 0x0C: 0x00
        let _x0c = cursor.read_i32::<LittleEndian>().unwrap_or(0);
        // 0x10: NumDescriptions (repeated)
        let _x10 = cursor.read_i32::<LittleEndian>().unwrap_or(0);

        for _entry_idx in 0..num_descriptions {
            // 0x00: Size of section (8 bytes, total compressed data size)
            let compressed_size = cursor.read_u64::<LittleEndian>().unwrap_or(0);
            // 0x08: Page count
            let page_count = cursor.read_i32::<LittleEndian>().unwrap_or(0);
            // 0x0C: Max decompressed size of a section page (normally 0x7400)
            let max_decompressed_size = cursor.read_i32::<LittleEndian>().unwrap_or(0) as u64;
            // 0x10: Unknown
            let _unknown = cursor.read_i32::<LittleEndian>().unwrap_or(0);
            // 0x14: Compressed (1 = no, 2 = yes)
            let compressed_code = cursor.read_i32::<LittleEndian>().unwrap_or(0);
            // 0x18: Section ID
            let section_id = cursor.read_i32::<LittleEndian>().unwrap_or(0);
            // 0x1C: Encrypted (0 = no, 1 = yes, 2 = unknown)
            let encrypted = cursor.read_i32::<LittleEndian>().unwrap_or(0);
            // 0x20: Section name (64 bytes, null-terminated)
            let mut name_buf = [0u8; 64];
            let _ = cursor.read_exact(&mut name_buf);
            let name = std::str::from_utf8(&name_buf)
                .unwrap_or("")
                .split('\0')
                .next()
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                // Skip local sections for unnamed entries
                for _j in 0..page_count {
                    let _ = cursor.read_i32::<LittleEndian>(); // page_number
                    let _ = cursor.read_i32::<LittleEndian>(); // compressed_size
                    let _ = cursor.read_u64::<LittleEndian>(); // offset
                }
                continue;
            }

            let mut desc = DwgSectionDescriptor::new(&name);
            desc.compressed_size = compressed_size;
            desc.page_count = page_count;
            desc.decompressed_size = max_decompressed_size;
            desc.compressed_code = compressed_code;
            desc.section_id = section_id;
            desc.encrypted = encrypted;

            // Read local page maps for this section.
            for _page_idx in 0..page_count {
                // 0x00: Page number (index into page map records)
                let page_number = cursor.read_i32::<LittleEndian>().unwrap_or(0);
                // 0x04: Data size for this page (compressed size)
                let page_compressed_size = cursor.read_i32::<LittleEndian>().unwrap_or(0) as u64;
                // 0x08: Start offset for this page
                let page_offset = cursor.read_u64::<LittleEndian>().unwrap_or(0);

                let mut local = DwgLocalSectionMap::new();
                local.page_number = page_number;
                local.compressed_size = page_compressed_size;
                local.offset = page_offset;
                local.decompressed_size = max_decompressed_size;

                // Look up the actual file position from the page map records.
                if let Some(record) = header.records.get(&(page_number as usize)) {
                    local.seeker = record.seeker as u64;
                }

                desc.local_sections.push(local);
            }

            // Adjust the last page's decompressed size if the total doesn't fill evenly.
            let size_left = compressed_size % max_decompressed_size;
            if size_left > 0 && !desc.local_sections.is_empty() {
                let last_idx = desc.local_sections.len() - 1;
                desc.local_sections[last_idx].decompressed_size = size_left;
            }

            header.add_descriptor(desc);
        }

        Ok(())
    }

    /// Parse AC21 compressed metadata from a decompressed RS block.
    fn parse_ac21_compressed_metadata(&mut self, _data: &[u8]) -> Result<()> {
        // The AC21 compressed metadata contains the section page map
        // with page sizes and checksums for the RS-encoded pages.
        // This is a complex structure — for now we mark it as parsed
        // and rely on the AC18-compatible page map reading.
        Ok(())
    }

    // ------------------------------------------------------------------
    // Section data extraction
    // ------------------------------------------------------------------

    /// Get the decompressed byte buffer for a named section.
    ///
    /// This is the central section-extraction method that dispatches to
    /// AC15, AC18, or AC21 extraction based on the file header type.
    fn get_section_stream(&mut self, section_name: &str) -> Result<Vec<u8>> {
        match &self.file_header {
            DwgFileHeader::AC15(h) => self.get_section_buffer_ac15(h.clone(), section_name),
            DwgFileHeader::AC18(h) => self.get_section_buffer_ac18(h.clone(), section_name),
            DwgFileHeader::AC21(h) => self.get_section_buffer_ac21(h.clone(), section_name),
        }
    }

    /// Extract section data for AC15 (R13–R2000).
    ///
    /// AC15 sections are stored as contiguous blocks in the file at
    /// offsets given by the section locator records.
    fn get_section_buffer_ac15(
        &mut self,
        header: DwgFileHeaderAC15,
        section_name: &str,
    ) -> Result<Vec<u8>> {
        let index = section_names::get_section_locator_by_name(section_name)
            .ok_or_else(|| {
                DxfError::InvalidFormat(format!(
                    "Section '{}' not found in AC15 locator table",
                    section_name
                ))
            })?;

        let record = header.records.get(&index).ok_or_else(|| {
            DxfError::InvalidFormat(format!(
                "Section locator record {} not found for '{}'",
                index, section_name
            ))
        })?;

        if record.seeker < 0 || record.size <= 0 {
            return Ok(Vec::new());
        }
        self.reader.seek(SeekFrom::Start(record.seeker as u64))?;
        let mut data = vec![0u8; record.size as usize];
        self.reader.read_exact(&mut data)?;
        Ok(data)
    }

    /// Extract section data for AC18 (R2004+).
    ///
    /// AC18 sections are stored in pages with encrypted headers and
    /// LZ77 compressed data.
    fn get_section_buffer_ac18(
        &mut self,
        header: DwgFileHeaderAC18,
        section_name: &str,
    ) -> Result<Vec<u8>> {
        let descriptor = header.descriptors.get(section_name).ok_or_else(|| {
            DxfError::InvalidFormat(format!(
                "Section descriptor '{}' not found for AC18",
                section_name
            ))
        })?;

        if descriptor.local_sections.is_empty() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        let decompressor = Lz77Ac18Decompressor;

        for page in &descriptor.local_sections {
            // Seek to the page's file offset.
            self.reader.seek(SeekFrom::Start(page.seeker))?;

            // Read the 32-byte encrypted page header.
            let mut page_header_data = [0u8; 32];
            self.reader.read_exact(&mut page_header_data)?;

            let decrypted = encryption::decrypt_data_section_header(
                &page_header_data,
                page.seeker,
            );

            let compressed_size = decrypted.compressed_size.max(0) as usize;
            let page_size = decrypted.page_size.max(0) as usize;

            if compressed_size == 0 || page_size == 0 {
                continue;
            }

            // Read compressed data.
            let mut compressed_data = vec![0u8; compressed_size];
            self.reader.read_exact(&mut compressed_data)?;

            // Decompress.
            if descriptor.compressed_code == 2 {
                let decompressed = decompressor.decompress(&compressed_data, page_size)?;
                result.extend_from_slice(&decompressed);
            } else {
                // No compression — raw data.
                result.extend_from_slice(&compressed_data);
            }
        }

        Ok(result)
    }

    /// Extract section data for AC21 (R2007).
    ///
    /// AC21 uses Reed-Solomon encoded pages + LZ77 AC21 compression.
    fn get_section_buffer_ac21(
        &mut self,
        header: DwgFileHeaderAC21,
        section_name: &str,
    ) -> Result<Vec<u8>> {
        let descriptor = header.base.descriptors.get(section_name).ok_or_else(|| {
            DxfError::InvalidFormat(format!(
                "Section descriptor '{}' not found for AC21",
                section_name
            ))
        })?;

        if descriptor.local_sections.is_empty() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        let decompressor = Lz77Ac21Decompressor;

        for page in &descriptor.local_sections {
            // For AC21, pages use RS encoding.
            // Seek to the page's file offset.
            let page_offset = ac21::DATA_PAGE_BASE_OFFSET + page.seeker;
            self.reader.seek(SeekFrom::Start(page_offset))?;

            let compressed_size = page.compressed_size;
            let decompressed_size = page.decompressed_size as usize;

            if compressed_size == 0 || decompressed_size == 0 {
                continue;
            }

            // Compute RS parameters.
            let (factor, read_size) = reed_solomon::compute_page_buffer_params(
                compressed_size,
                1, // Correction factor — may come from compressed metadata.
                251,
            );

            // Read the RS encoded data.
            let mut rs_data = vec![0u8; read_size];
            let bytes_read = self.reader.read(&mut rs_data)?;
            rs_data.truncate(bytes_read);

            // RS decode.
            let decoded = reed_solomon::decode(&rs_data, compressed_size as usize, factor, 251);

            // LZ77 AC21 decompress.
            if descriptor.compressed_code == 2 {
                let decompressed = decompressor.decompress(&decoded, decompressed_size)?;
                result.extend_from_slice(&decompressed);
            } else {
                result.extend_from_slice(&decoded);
            }
        }

        Ok(result)
    }

    // ------------------------------------------------------------------
    // Section readers
    // ------------------------------------------------------------------

    /// Read the header section (system variables + handles).
    fn read_header(&mut self) -> Result<(crate::document::HeaderVariables, DwgHeaderHandlesCollection)> {
        let data = self.get_section_stream(section_names::HEADER)?;
        let acad_maint_ver = self.file_header.maintenance_version() as i32;
        let reader = DwgHeaderReader::new(self.version, data);
        reader.read(acad_maint_ver)
    }

    /// Read the classes section.
    fn read_classes(&mut self) -> Result<DxfClassCollection> {
        let data = self.get_section_stream(section_names::CLASSES)?;
        let fh_version = self.file_header.version();
        let fh_maint = self.file_header.maintenance_version();
        let reader = DwgClassesReader::new(self.version, data, fh_version, fh_maint);
        reader.read()
    }

    /// Read the handles (object map) section.
    fn read_handles(&mut self) -> Result<HashMap<u64, i64>> {
        let data = self.get_section_stream(section_names::HANDLES)?;
        let reader = DwgHandleReader::new(self.version, data);
        reader.read()
    }

    /// Read the summary info section (AC18+).
    ///
    /// Returns `None` if the section doesn't exist or can't be read.
    fn read_summary_info(&mut self) -> Option<CadSummaryInfo> {
        let data = self.get_section_stream(section_names::SUMMARY_INFO).ok()?;
        if data.is_empty() {
            return None;
        }
        let reader = DwgSummaryInfoReader::new(self.version, data);
        reader.read().ok()
    }

    /// Read the preview (thumbnail) section.
    fn read_preview(&mut self) -> Option<DwgPreview> {
        let preview_addr = self.file_header.preview_address();
        if preview_addr <= 0 {
            return None;
        }

        // For AC15, the preview is at the preview_address directly.
        // For AC18+, it may be in the Preview section.
        let data = match &self.file_header {
            DwgFileHeader::AC15(_) => {
                // Read from the raw file at the preview address.
                self.reader.seek(SeekFrom::Start(preview_addr as u64)).ok()?;
                // The preview has start/end sentinels; read a reasonable amount.
                let mut buf = vec![0u8; 32768]; // 32 KB max
                let n = self.reader.read(&mut buf).ok()?;
                buf.truncate(n);
                buf
            }
            _ => {
                self.get_section_stream(section_names::PREVIEW).ok()?
            }
        };

        if data.is_empty() {
            return None;
        }

        let reader = DwgPreviewReader::new(self.version, data);
        reader.read().ok()
    }

    /// Read the app info section (AC18+).
    fn read_app_info(&mut self) -> Option<super::AppInfo> {
        let data = self.get_section_stream(section_names::APP_INFO).ok()?;
        if data.is_empty() {
            return None;
        }
        let reader = DwgAppInfoReader::new(self.version, data);
        reader.read().ok()
    }

    // ------------------------------------------------------------------
    // Notification helper
    // ------------------------------------------------------------------

    #[allow(dead_code)]
    fn notify(&mut self, message: &str, ntype: NotificationType) {
        self.notifications.push(Notification::new(ntype, message));
    }
}

// ---------------------------------------------------------------------------
// Decrypted AC18 header data (internal)
// ---------------------------------------------------------------------------

/// Decrypted fields from the AC18 encrypted header block at offset 0x80.
struct DecryptedAC18HeaderData {
    root_tree_node_gap: i32,
    gap_array_size: u32,
    crc_seed: u32,
    last_page_id: i32,
    last_section_addr: u64,
    second_header_addr: u64,
    gap_amount: u32,
    section_amount: u32,
    section_page_map_id: u32,
    page_map_address: u64,
    section_map_id: u32,
    section_array_page_size: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configuration_default() {
        let config = DwgReaderConfiguration::default();
        assert!(!config.failsafe);
        assert!(!config.keep_unknown_entities);
    }
}
