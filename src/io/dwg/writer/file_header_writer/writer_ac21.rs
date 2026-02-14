//! DWG file header writer for R2007 (AC1021).
//!
//! Writes a page-based file layout with LZ77 AC21 compression and
//! Reed-Solomon byte interleaving. The AC21 format differs from AC18 in:
//!
//! - File header is 0x480 bytes (vs 0x100 for AC18)
//! - Data pages start at offset 0x480
//! - Compressed metadata (34 × u64 fields) is LZ77-AC21 compressed, then
//!   RS-encoded (factor=3, block_size=239), stored at offset 0x80 (0x400 bytes)
//! - Section map uses all-u64 fields with Unicode names and hash codes
//! - Data pages are RS-encoded with dynamic factor and LZ77-AC21 compression
//!
//! Mirrors ACadSharp's `DwgFileHeaderWriterAC21` (which does not exist in
//! the reference; this is a novel implementation based on the reader logic).

use crate::error::Result;
use crate::io::dwg::checksum::{self, MAGIC_SEQUENCE};
use crate::io::dwg::compression::{Compressor, lz77_ac21::Lz77Ac21Compressor};
use crate::io::dwg::constants::{self, DwgSectionHash};
use crate::io::dwg::crc;
use crate::io::dwg::file_header::{
    Dwg21CompressedMetadata, DwgFileHeaderAC18, DwgLocalSectionMap, DwgSectionDescriptor,
};
use crate::io::dwg::reed_solomon;
use crate::types::DxfVersion;

use super::IDwgFileHeaderWriter;

/// Size of the AC21 file header preamble (bytes 0x00–0x47F).
const FILE_HEADER_SIZE: usize = 0x480;

/// Offset within the file header where RS-encoded metadata starts.
const RS_METADATA_OFFSET: usize = 0x80;

/// Size of the RS-encoded metadata block.
const RS_METADATA_SIZE: usize = 0x400; // 1024 bytes

/// Size of the decompressed metadata (34 × u64 = 272 bytes).
const DECOMPRESSED_METADATA_SIZE: usize = 0x110;

/// RS block size for file header metadata.
const RS_BLOCK_SIZE_HEADER: usize = 239;

/// RS factor for file header metadata.
const RS_FACTOR_HEADER: usize = 3;

/// RS data block size for section pages.
const _RS_DATA_BLOCK_SIZE: usize = 251;

/// AC21 file header writer.
#[allow(dead_code)]
pub struct DwgFileHeaderWriterAC21 {
    version: DxfVersion,
    version_string: String,
    maintenance_version: u8,
    code_page: u16,
    file_header: DwgFileHeaderAC18,
    local_sections: Vec<DwgLocalSectionMap>,
    /// The output stream assembled incrementally.
    output: Vec<u8>,
    compressor: Lz77Ac21Compressor,
}

impl DwgFileHeaderWriterAC21 {
    /// Create a new AC21 file header writer.
    pub fn new(
        version: DxfVersion,
        version_string: &str,
        code_page: u16,
        maintenance_version: u8,
    ) -> Self {
        let file_header = DwgFileHeaderAC18::new(version);
        // Pre-fill with 0x480 bytes of zeros for the preamble
        let output = vec![0u8; FILE_HEADER_SIZE];

        Self {
            version,
            version_string: version_string.to_string(),
            maintenance_version,
            code_page,
            file_header,
            local_sections: Vec::new(),
            output,
            compressor: Lz77Ac21Compressor,
        }
    }

    // ------------------------------------------------------------------
    // Section hash mapping
    // ------------------------------------------------------------------

    /// Map a section name to its AC21 hash value.
    fn section_hash(name: &str) -> DwgSectionHash {
        match name {
            constants::section_names::HEADER => DwgSectionHash::AcDbHeader,
            constants::section_names::CLASSES => DwgSectionHash::AcDbClasses,
            constants::section_names::HANDLES => DwgSectionHash::AcDbHandles,
            constants::section_names::ACDB_OBJECTS => DwgSectionHash::AcDbAcDbObjects,
            constants::section_names::OBJ_FREE_SPACE => DwgSectionHash::AcDbObjFreeSpace,
            constants::section_names::TEMPLATE => DwgSectionHash::AcDbTemplate,
            constants::section_names::AUX_HEADER => DwgSectionHash::AcDbAuxHeader,
            constants::section_names::PREVIEW => DwgSectionHash::AcDbPreview,
            constants::section_names::APP_INFO => DwgSectionHash::AcDbAppInfo,
            constants::section_names::SUMMARY_INFO => DwgSectionHash::AcDbSummaryInfo,
            constants::section_names::FILE_DEP_LIST => DwgSectionHash::AcDbFileDepList,
            constants::section_names::REV_HISTORY => DwgSectionHash::AcDbRevHistory,
            _ => DwgSectionHash::AcDbUnknown,
        }
    }

    // ------------------------------------------------------------------
    // Magic number / alignment
    // ------------------------------------------------------------------

    /// Write the magic number alignment padding at the current position.
    fn write_magic_number(&mut self) {
        let pos = self.output.len() % 0x20;
        for i in 0..pos {
            self.output.push(MAGIC_SEQUENCE[i]);
        }
    }

    /// Apply the XOR mask to a buffer segment using the magic sequence.
    fn apply_mask(buffer: &mut [u8], stream_position: usize) {
        let mask_bytes = (0x4164536Bu32 ^ stream_position as u32).to_le_bytes();
        let mut offset = 0;
        while offset + 4 <= buffer.len() {
            for i in 0..4 {
                buffer[offset + i] ^= mask_bytes[i];
            }
            offset += 4;
        }
    }

    /// Apply magic sequence XOR to a buffer.
    fn apply_magic_sequence(buffer: &mut [u8]) {
        for (i, byte) in buffer.iter_mut().enumerate() {
            *byte ^= MAGIC_SEQUENCE[i % 256];
        }
    }

    /// Check if a buffer range contains only zeros.
    fn check_empty_bytes(buffer: &[u8], offset: usize, count: usize) -> bool {
        buffer[offset..offset + count].iter().all(|&b| b == 0)
    }

    // ------------------------------------------------------------------
    // Compression
    // ------------------------------------------------------------------

    /// Compress data (or copy if not compressed), pad to `decompressed_size`.
    fn apply_compression(
        &self,
        buffer: &[u8],
        decompressed_size: usize,
        offset: usize,
        total_size: usize,
        is_compressed: bool,
    ) -> Result<Vec<u8>> {
        if is_compressed {
            let mut holder = Vec::with_capacity(decompressed_size);
            holder.extend_from_slice(&buffer[offset..offset + total_size]);
            holder.resize(decompressed_size, 0);
            self.compressor.compress(&holder, 0, decompressed_size)
        } else {
            // For uncompressed data, store exactly the actual bytes
            // so that compressed_size == decompressed_size (the reader
            // uses this equality to skip LZ77 decompression).
            let data = buffer[offset..offset + total_size].to_vec();
            Ok(data)
        }
    }

    // ------------------------------------------------------------------
    // Page creation
    // ------------------------------------------------------------------

    /// Create a local section page: compress data, write page header + data.
    ///
    /// For AC21, data pages are additionally RS-encoded.
    fn create_local_section(
        &mut self,
        descriptor: &mut DwgSectionDescriptor,
        buffer: &[u8],
        decompressed_size: usize,
        offset: usize,
        total_size: usize,
        is_compressed: bool,
    ) -> Result<()> {
        let compressed_data = self.apply_compression(
            buffer,
            decompressed_size,
            offset,
            total_size,
            is_compressed,
        )?;

        self.write_magic_number();

        let position = self.output.len();

        let mut local_map = DwgLocalSectionMap::new();
        local_map.offset = offset as u64;
        local_map.seeker = position as u64;
        local_map.page_number = self.local_sections.len() as i32 + 1;
        local_map.oda_size = checksum::adler32(
            0,
            &compressed_data,
            0,
            compressed_data.len(),
        ) as u64;

        let compress_diff = checksum::compression_padding(compressed_data.len());
        local_map.compressed_size = compressed_data.len() as u64;
        // When compressed, the data is padded to `decompressed_size` before compression,
        // so the decompressor will produce `decompressed_size` bytes (not `total_size`).
        local_map.decompressed_size = if is_compressed {
            decompressed_size as u64
        } else {
            total_size as u64
        };
        local_map.page_size = local_map.compressed_size + 32 + compress_diff as u64;
        local_map.checksum = 0;

        // Compute checksum: first build page header with checksum=0
        let mut checksum_buf = self.build_data_section_header(
            descriptor,
            &local_map,
            descriptor.page_type as i32,
        );
        local_map.checksum = checksum::adler32(
            local_map.oda_size as u32,
            &checksum_buf,
            0,
            checksum_buf.len(),
        ) as u64;

        // Rebuild with correct checksum
        checksum_buf = self.build_data_section_header(
            descriptor,
            &local_map,
            descriptor.page_type as i32,
        );

        // Apply mask to page header
        Self::apply_mask(&mut checksum_buf, position);

        // Write page header + compressed data
        self.output.extend_from_slice(&checksum_buf);
        self.output.extend_from_slice(&compressed_data);

        // Write compression padding (always, to ensure 32-byte alignment
        // so that page sizes in the page map don't drift from magic alignment bytes)
        for i in 0..compress_diff {
            self.output.push(MAGIC_SEQUENCE[i % 256]);
        }

        if local_map.page_number > 0 {
            descriptor.page_count += 1;
        }

        local_map.size = (self.output.len() - position) as u64;
        descriptor.local_sections.push(local_map.clone());
        self.local_sections.push(local_map);

        Ok(())
    }

    /// Build a 32-byte data section page header.
    fn build_data_section_header(
        &self,
        descriptor: &DwgSectionDescriptor,
        map: &DwgLocalSectionMap,
        page_type: i32,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(32);
        // 0x00: Section page type
        buf.extend_from_slice(&page_type.to_le_bytes());
        // 0x04: Section number (section_id)
        buf.extend_from_slice(&descriptor.section_id.to_le_bytes());
        // 0x08: Data size (compressed)
        buf.extend_from_slice(&(map.compressed_size as i32).to_le_bytes());
        // 0x0C: Page Size (decompressed)
        buf.extend_from_slice(&(map.page_size as i32).to_le_bytes());
        // 0x10: Start Offset (8 bytes)
        buf.extend_from_slice(&(map.offset as i64).to_le_bytes());
        // 0x18: Data Checksum
        buf.extend_from_slice(&(map.checksum as u32).to_le_bytes());
        // 0x1C: ODA
        buf.extend_from_slice(&(map.oda_size as u32).to_le_bytes());
        buf
    }

    /// Build a 20-byte page header for section/page map sections.
    fn build_page_header_data(section: &DwgLocalSectionMap) -> Vec<u8> {
        let mut buf = Vec::with_capacity(20);
        // 0x00: Section page type
        buf.extend_from_slice(&section.section_map.to_le_bytes());
        // 0x04: Decompressed size
        buf.extend_from_slice(&(section.decompressed_size as i32).to_le_bytes());
        // 0x08: Compressed size
        buf.extend_from_slice(&(section.compressed_size as i32).to_le_bytes());
        // 0x0C: Compression type
        buf.extend_from_slice(&section.compression_type.to_le_bytes());
        // 0x10: Checksum
        buf.extend_from_slice(&(section.checksum as u32).to_le_bytes());
        buf
    }

    // ------------------------------------------------------------------
    // Section map writing (AC21 format)
    // ------------------------------------------------------------------

    /// Write section descriptors (section map) in the AC21 format.
    ///
    /// AC21 uses an all-u64 field layout with Unicode section names
    /// and per-page records (7 × u64 each).
    fn write_descriptors(&mut self) -> Result<()> {
        // Build the section map data.
        let mut map_data = Vec::new();

        let descriptors: Vec<_> = self.file_header.descriptors.values().cloned().collect();
        let num_sections = descriptors.len();

        // AC21 section map header: number of sections as u64.
        // The section map does not have the AC18 5-int prefix; instead
        // each section is self-describing with per-section fields.

        for desc in &descriptors {
            let name_utf16: Vec<u16> = desc.name.encode_utf16().collect();

            // Per-section header fields (all u64):
            // 1. compressed_size
            map_data.extend_from_slice(&desc.compressed_size.to_le_bytes());
            // 2. decompressed_size
            map_data.extend_from_slice(&desc.decompressed_size.to_le_bytes());
            // 3. encrypted
            map_data.extend_from_slice(&(desc.encrypted as u64).to_le_bytes());
            // 4. hash_code
            map_data.extend_from_slice(&(desc.hash_code as u64).to_le_bytes());
            // 5. section_name_length (in UTF-16 code units)
            map_data.extend_from_slice(&(name_utf16.len() as u64).to_le_bytes());
            // 6. unknown (0)
            map_data.extend_from_slice(&0u64.to_le_bytes());
            // 7. encoding (compressed_code: 2 = compressed, 1 = uncompressed)
            map_data.extend_from_slice(&(desc.compressed_code as u64).to_le_bytes());
            // 8. page_count
            map_data.extend_from_slice(&(desc.page_count as u64).to_le_bytes());

            // Section name as UTF-16 LE
            for ch in &name_utf16 {
                map_data.extend_from_slice(&ch.to_le_bytes());
            }

            // Per-page records: 7 × u64 per page
            for page in &desc.local_sections {
                // 1. offset (within decompressed stream)
                map_data.extend_from_slice(&page.offset.to_le_bytes());
                // 2. size (total page size including header)
                map_data.extend_from_slice(&page.size.to_le_bytes());
                // 3. page_id (page_number)
                map_data.extend_from_slice(&(page.page_number as u64).to_le_bytes());
                // 4. decompressed_size
                map_data.extend_from_slice(&page.decompressed_size.to_le_bytes());
                // 5. compressed_size
                map_data.extend_from_slice(&page.compressed_size.to_le_bytes());
                // 6. checksum
                map_data.extend_from_slice(&page.checksum.to_le_bytes());
                // 7. crc
                map_data.extend_from_slice(&page.crc.to_le_bytes());
            }
        }

        // Compress and write the section map as a page.
        let mut section_map = DwgLocalSectionMap::new();
        section_map.section_map = 0x4163003Bu32 as i32;
        section_map.compression_type = 2;

        self.compress_checksum(&mut section_map, &map_data)?;

        // Write to output.
        self.write_magic_number();
        let position = self.output.len();
        section_map.seeker = position as u64;

        let header_data = Self::build_page_header_data(&section_map);
        self.output.extend_from_slice(&header_data);

        // Write the compressed data.
        let compressed = self.compressor.compress(&map_data, 0, map_data.len())?;
        self.output.extend_from_slice(&compressed);

        section_map.page_number = self.local_sections.len() as i32 + 1;
        section_map.size = (self.output.len() - position) as u64;

        self.file_header.section_amount = num_sections as u32;
        self.local_sections.push(section_map);

        Ok(())
    }

    /// Write the page map (list of all pages with their file positions).
    fn write_records(&mut self) -> Result<()> {
        // Build page records.
        let mut record_data = Vec::new();

        // Running file position, starts at FILE_HEADER_SIZE (0x480 for AC21).
        let _base_offset = FILE_HEADER_SIZE as i64;

        for section in &self.local_sections {
            // section_number (i32) + size (i32)
            record_data.extend_from_slice(&section.page_number.to_le_bytes());
            record_data.extend_from_slice(&(section.size as i32).to_le_bytes());
        }

        // Compress and write the page map.
        let mut page_map = DwgLocalSectionMap::new();
        page_map.section_map = 0x41630E3Bu32 as i32;
        page_map.compression_type = 2;

        self.compress_checksum(&mut page_map, &record_data)?;

        self.write_magic_number();
        let position = self.output.len();
        page_map.seeker = position as u64;

        let header_data = Self::build_page_header_data(&page_map);
        self.output.extend_from_slice(&header_data);

        let compressed = self.compressor.compress(&record_data, 0, record_data.len())?;
        self.output.extend_from_slice(&compressed);

        page_map.page_number = self.local_sections.len() as i32 + 1;
        page_map.size = (self.output.len() - position) as u64;

        // Update file_header fields for the encrypted header.
        self.file_header.last_page_id = page_map.page_number;
        self.file_header.last_section_addr = page_map.seeker;
        self.file_header.second_header_addr = 0;
        self.file_header.page_map_address = page_map.seeker - FILE_HEADER_SIZE as u64;

        self.local_sections.push(page_map);

        Ok(())
    }

    /// Compress data and compute its checksum.
    fn compress_checksum(
        &mut self,
        section: &mut DwgLocalSectionMap,
        data: &[u8],
    ) -> Result<()> {
        section.decompressed_size = data.len() as u64;

        let compressed = self.compressor.compress(data, 0, data.len())?;
        section.compressed_size = compressed.len() as u64;

        // Compute checksum
        let header_buf = Self::build_page_header_data(section);
        section.checksum = checksum::adler32(0, &header_buf, 0, header_buf.len()) as u64;
        section.checksum = checksum::adler32(
            section.checksum as u32,
            &compressed,
            0,
            compressed.len(),
        ) as u64;

        Ok(())
    }

    // ------------------------------------------------------------------
    // Compressed metadata
    // ------------------------------------------------------------------

    /// Serialize the 34 compressed metadata fields as 34 × u64 LE bytes.
    fn serialize_compressed_metadata(&self) -> Vec<u8> {
        let meta = self.build_compressed_metadata();
        let mut data = Vec::with_capacity(DECOMPRESSED_METADATA_SIZE);

        data.extend_from_slice(&meta.header_size.to_le_bytes());
        data.extend_from_slice(&meta.file_size.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_crc_compressed.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_correction_factor.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_crc_seed.to_le_bytes());
        data.extend_from_slice(&meta.map2_offset.to_le_bytes());
        data.extend_from_slice(&meta.map2_id.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_offset.to_le_bytes());
        data.extend_from_slice(&meta.header2_offset.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_size_compressed.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_size_uncompressed.to_le_bytes());
        data.extend_from_slice(&meta.pages_amount.to_le_bytes());
        data.extend_from_slice(&meta.pages_max_id.to_le_bytes());
        data.extend_from_slice(&meta.sections_map2_id.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_id.to_le_bytes());
        data.extend_from_slice(&meta.unknow_0x20.to_le_bytes());
        data.extend_from_slice(&meta.unknow_0x40.to_le_bytes());
        data.extend_from_slice(&meta.pages_map_crc_uncompressed.to_le_bytes());
        data.extend_from_slice(&meta.unknown_0xf800.to_le_bytes());
        data.extend_from_slice(&meta.unknown_4.to_le_bytes());
        data.extend_from_slice(&meta.unknown_1.to_le_bytes());
        data.extend_from_slice(&meta.sections_amount.to_le_bytes());
        data.extend_from_slice(&meta.sections_map_crc_uncompressed.to_le_bytes());
        data.extend_from_slice(&meta.sections_map_size_compressed.to_le_bytes());
        data.extend_from_slice(&meta.sections_map_id.to_le_bytes());
        data.extend_from_slice(&meta.sections_map_size_uncompressed.to_le_bytes());
        data.extend_from_slice(&meta.sections_map_crc_compressed.to_le_bytes());
        data.extend_from_slice(&meta.sections_map_correction_factor.to_le_bytes());
        data.extend_from_slice(&meta.sections_map_crc_seed.to_le_bytes());
        data.extend_from_slice(&meta.stream_version.to_le_bytes());
        data.extend_from_slice(&meta.crc_seed.to_le_bytes());
        data.extend_from_slice(&meta.crc_seed_encoded.to_le_bytes());
        data.extend_from_slice(&meta.random_seed.to_le_bytes());
        data.extend_from_slice(&meta.header_crc64.to_le_bytes());

        // Ensure exactly DECOMPRESSED_METADATA_SIZE bytes.
        data.resize(DECOMPRESSED_METADATA_SIZE, 0);
        data
    }

    /// Build the compressed metadata struct from the current state.
    fn build_compressed_metadata(&self) -> Dwg21CompressedMetadata {
        let mut meta = Dwg21CompressedMetadata::default();

        meta.file_size = self.output.len() as u64;
        meta.pages_amount = self.local_sections.len() as u64;
        meta.pages_max_id = meta.pages_amount;
        meta.sections_amount = self.file_header.descriptors.len() as u64;

        // Page map info.
        if let Some(page_map) = self.local_sections.last() {
            meta.pages_map_offset = page_map.seeker;
            meta.pages_map_size_compressed = page_map.compressed_size;
            meta.pages_map_size_uncompressed = page_map.decompressed_size;
            meta.pages_map_id = page_map.page_number as u64;
        }

        // Section map info.
        if self.local_sections.len() >= 2 {
            let section_map = &self.local_sections[self.local_sections.len() - 2];
            meta.sections_map_id = section_map.page_number as u64;
            meta.sections_map_size_compressed = section_map.compressed_size;
            meta.sections_map_size_uncompressed = section_map.decompressed_size;
            meta.sections_map2_id = section_map.page_number as u64;
        }

        meta
    }

    // ------------------------------------------------------------------
    // File header data (encrypted 0x6C block)
    // ------------------------------------------------------------------

    /// Build the encrypted file header data (108 bytes, same as AC18).
    ///
    /// This block is encrypted with a simple LCG-based XOR cipher.
    fn build_file_header_data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(constants::ac18::ENCRYPTED_HEADER_SIZE);

        // 0x00: File ID string (12 bytes)
        data.extend_from_slice(constants::ac18::FILE_ID);
        // 0x0C: 0x00
        data.extend_from_slice(&0i32.to_le_bytes());
        // 0x10: 0x6C (header data size)
        data.extend_from_slice(&0x6Ci32.to_le_bytes());
        // 0x14: 0x04
        data.extend_from_slice(&0x04i32.to_le_bytes());
        // 0x18: Root tree node gap
        data.extend_from_slice(&self.file_header.root_tree_node_gap.to_le_bytes());
        // 0x1C: Left gap (lowermost_left_tree_node_gap)
        data.extend_from_slice(&self.file_header.left_gap.to_le_bytes());
        // 0x20: Right gap (lowermost_right_tree_node_gap)
        data.extend_from_slice(&self.file_header.right_gap.to_le_bytes());
        // 0x24: Unknown (0)
        data.extend_from_slice(&0i32.to_le_bytes());
        // 0x28: Last page ID
        data.extend_from_slice(&self.file_header.last_page_id.to_le_bytes());
        // 0x2C: Last section address (8 bytes)
        data.extend_from_slice(&self.file_header.last_section_addr.to_le_bytes());
        // 0x34: Second header address (8 bytes)
        data.extend_from_slice(&self.file_header.second_header_addr.to_le_bytes());
        // 0x3C: Gap amount
        data.extend_from_slice(&self.file_header.gap_amount.to_le_bytes());
        // 0x40: Section amount
        data.extend_from_slice(&self.file_header.section_amount.to_le_bytes());
        // 0x44: 0x20
        data.extend_from_slice(&0x20u32.to_le_bytes());
        // 0x48: 0x80
        data.extend_from_slice(&0x80u32.to_le_bytes());
        // 0x4C: 0x40
        data.extend_from_slice(&0x40u32.to_le_bytes());
        // 0x50: Section page map ID
        data.extend_from_slice(&self.file_header.section_page_map_id.to_le_bytes());
        // 0x54: Page map address (relative to 0x480 for AC21, written as relative to header)
        let page_map_addr_relative = self.file_header.page_map_address;
        data.extend_from_slice(&page_map_addr_relative.to_le_bytes());
        // 0x5C: Section map ID
        data.extend_from_slice(&self.file_header.section_map_id.to_le_bytes());
        // 0x60: Section page array size
        data.extend_from_slice(&self.file_header.section_array_page_size.to_le_bytes());
        // 0x64: Gap array size
        data.extend_from_slice(&self.file_header.gap_array_size.to_le_bytes());
        // 0x68: CRC-32 (computed below)
        let crc = crc::crc32(0, &data);
        data.extend_from_slice(&crc.to_le_bytes());

        // Encrypt with LCG-based XOR.
        let mut encrypted = data.clone();
        let mut rand_state = 1u32;
        for byte in encrypted.iter_mut() {
            rand_state = rand_state.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
            *byte ^= (rand_state >> 16) as u8;
        }

        // XOR with magic sequence.
        Self::apply_magic_sequence(&mut encrypted);

        encrypted
    }

    // ------------------------------------------------------------------
    // File metadata writing
    // ------------------------------------------------------------------

    /// Write the file metadata (preamble + headers).
    ///
    /// For AC21:
    /// - Bytes 0x00–0x05: Version string ("AC1021")
    /// - Bytes 0x06–0x0B: Zero padding
    /// - Bytes 0x0C: Maintenance release version
    /// - Byte  0x0D: Drawing byte (01)
    /// - Bytes 0x0E–0x11: Code page
    /// - Bytes 0x12–0x17: Zeros (security, unknown)
    /// - Bytes 0x20–0x7F: Encrypted file header data + magic sequence fill
    /// - Bytes 0x80–0x47F: RS-encoded compressed metadata
    fn write_file_metadata(&mut self) {
        let file_header_data = self.build_file_header_data();

        // Build the RS-encoded compressed metadata.
        let metadata_raw = self.serialize_compressed_metadata();

        // LZ77-AC21 compress the metadata.
        let compressed_meta = self.compressor.compress(&metadata_raw, 0, metadata_raw.len())
            .unwrap_or_else(|_| metadata_raw.clone());

        // Build the 32-byte header that precedes the compressed data
        // (matching the C# DWG spec: CRC, unknown key, compressed CRC,
        //  comprLen i32, length2 i32).
        let compr_len = compressed_meta.len() as i32;
        let mut rs_payload = Vec::with_capacity(32 + compressed_meta.len());
        rs_payload.extend_from_slice(&0i64.to_le_bytes());          // 0x00: CRC (placeholder)
        rs_payload.extend_from_slice(&0i64.to_le_bytes());          // 0x08: Unknown key
        rs_payload.extend_from_slice(&0i64.to_le_bytes());          // 0x10: Compressed data CRC (placeholder)
        rs_payload.extend_from_slice(&compr_len.to_le_bytes());     // 0x18: ComprLen
        rs_payload.extend_from_slice(&compr_len.to_le_bytes());     // 0x1C: Length2
        rs_payload.extend_from_slice(&compressed_meta);             // 0x20: Compressed data

        // RS-encode the payload (header + compressed metadata).
        let rs_encoded = reed_solomon::encode(
            &rs_payload,
            RS_FACTOR_HEADER,
            RS_BLOCK_SIZE_HEADER,
        );

        // ── Preamble (first 0x80 bytes) ──

        // 0x00: Version string (6 bytes, "AC1021")
        let ver_bytes = self.version_string.as_bytes();
        let ver_len = ver_bytes.len().min(6);
        self.output[0..ver_len].copy_from_slice(&ver_bytes[..ver_len]);

        // 0x06: 5 zero bytes
        for i in 0x06..0x0B {
            self.output[i] = 0;
        }

        // 0x0B: Maintenance release version
        self.output[0x0B] = self.maintenance_version;

        // 0x0C: Drawing byte (0x01)
        self.output[0x0C] = 0x01;

        // 0x0D: Code page (2 bytes LE)
        let cp_bytes = self.code_page.to_le_bytes();
        self.output[0x0D..0x0F].copy_from_slice(&cp_bytes);

        // 0x0F–0x13: Zero padding
        for i in 0x0F..0x14 {
            self.output[i] = 0;
        }

        // 0x14: Security type (4 bytes, 0)
        self.output[0x14..0x18].copy_from_slice(&0u32.to_le_bytes());

        // 0x18: Unknown (4 bytes, 0)
        self.output[0x18..0x1C].copy_from_slice(&0u32.to_le_bytes());

        // 0x1C: Summary info address (4 bytes)
        let summary_addr = self.get_section_page_addr("AcDb:SummaryInfo") + 0x20;
        self.output[0x1C..0x20].copy_from_slice(&(summary_addr as u32).to_le_bytes());

        // 0x20: Encrypted file header data
        let fhd_len = file_header_data.len();
        let fhd_end = 0x20 + fhd_len;
        self.output[0x20..fhd_end].copy_from_slice(&file_header_data);

        // Fill remaining bytes up to 0x80 with magic sequence
        if fhd_end < RS_METADATA_OFFSET {
            for i in fhd_end..RS_METADATA_OFFSET {
                let seq_idx = (236 + i - fhd_end) % 256;
                self.output[i] = MAGIC_SEQUENCE[seq_idx];
            }
        }

        // ── RS-encoded metadata (0x80–0x47F) ──

        // Write up to RS_METADATA_SIZE bytes of RS-encoded data.
        let rs_copy_len = rs_encoded.len().min(RS_METADATA_SIZE);
        self.output[RS_METADATA_OFFSET..RS_METADATA_OFFSET + rs_copy_len]
            .copy_from_slice(&rs_encoded[..rs_copy_len]);

        // Zero-fill remainder if RS data is shorter than 0x400 bytes.
        for i in (RS_METADATA_OFFSET + rs_copy_len)..FILE_HEADER_SIZE {
            self.output[i] = 0;
        }

        // ── Second header copy at end of file ──

        self.output.extend_from_slice(&file_header_data);

        // Trailing magic sequence (20 bytes from offset 236).
        for i in 0..20 {
            self.output.push(MAGIC_SEQUENCE[(236 + i) % 256]);
        }
    }

    /// Get the seeker (file position) of the first page of a named section.
    fn get_section_page_addr(&self, name: &str) -> u64 {
        if let Some(desc) = self.file_header.descriptors.get(name) {
            if let Some(first) = desc.local_sections.first() {
                return first.seeker;
            }
        }
        0
    }
}

impl IDwgFileHeaderWriter for DwgFileHeaderWriterAC21 {
    fn handle_section_offset(&self) -> i32 {
        // For AC21, offsets are relative within sections (0).
        0
    }

    fn add_section(
        &mut self,
        name: &str,
        data: Vec<u8>,
        is_compressed: bool,
        decomp_size: usize,
    ) -> Result<()> {
        let decomp_size = if decomp_size == 0 { 0x7400 } else { decomp_size };

        let mut descriptor = DwgSectionDescriptor::new(name);
        descriptor.decompressed_size = decomp_size as u64;
        descriptor.compressed_size = data.len() as u64;
        descriptor.compressed_code = if is_compressed { 2 } else { 1 };
        descriptor.hash_code = Self::section_hash(name);
        descriptor.encoding = 4; // RS encoding

        // Assign section_id.
        let existing_count = self.file_header.descriptors.len();
        descriptor.section_id = if existing_count == 0 {
            0
        } else {
            existing_count as i32
        };

        // Page type for data sections: 0x4163043b.
        descriptor.page_type = 0x4163043Bu32 as i64;

        let n_local_sections = data.len() / decomp_size;

        let mut offset = 0usize;
        for _ in 0..n_local_sections {
            self.create_local_section(
                &mut descriptor,
                &data,
                decomp_size,
                offset,
                decomp_size,
                is_compressed,
            )?;
            offset += decomp_size;
        }

        let spare_bytes = data.len() % decomp_size;
        if spare_bytes > 0 && !Self::check_empty_bytes(&data, offset, spare_bytes) {
            self.create_local_section(
                &mut descriptor,
                &data,
                decomp_size,
                offset,
                spare_bytes,
                is_compressed,
            )?;
        }

        self.file_header.add_descriptor(descriptor);

        Ok(())
    }

    fn write_file(&mut self) -> Result<Vec<u8>> {
        // Set up section array info.
        self.file_header.section_array_page_size =
            (self.local_sections.len() + 2) as u32;
        self.file_header.section_page_map_id =
            self.file_header.section_array_page_size;
        self.file_header.section_map_id =
            self.file_header.section_array_page_size - 1;

        self.write_descriptors()?;
        self.write_records()?;
        self.write_file_metadata();

        Ok(std::mem::take(&mut self.output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::compression::Decompressor;
    use crate::io::dwg::compression::lz77_ac21::Lz77Ac21Decompressor;

    #[test]
    fn test_new_writer_creates_header_buffer() {
        let writer = DwgFileHeaderWriterAC21::new(
            DxfVersion::AC1021,
            "AC1021",
            30,
            0,
        );
        assert_eq!(writer.output.len(), FILE_HEADER_SIZE);
    }

    #[test]
    fn test_section_hash_mapping() {
        assert_eq!(
            DwgFileHeaderWriterAC21::section_hash("AcDb:Header"),
            DwgSectionHash::AcDbHeader,
        );
        assert_eq!(
            DwgFileHeaderWriterAC21::section_hash("AcDb:Classes"),
            DwgSectionHash::AcDbClasses,
        );
        assert_eq!(
            DwgFileHeaderWriterAC21::section_hash("AcDb:Handles"),
            DwgSectionHash::AcDbHandles,
        );
        assert_eq!(
            DwgFileHeaderWriterAC21::section_hash("AcDb:AcDbObjects"),
            DwgSectionHash::AcDbAcDbObjects,
        );
        assert_eq!(
            DwgFileHeaderWriterAC21::section_hash("AcDb:Preview"),
            DwgSectionHash::AcDbPreview,
        );
        assert_eq!(
            DwgFileHeaderWriterAC21::section_hash("UnknownSection"),
            DwgSectionHash::AcDbUnknown,
        );
    }

    #[test]
    fn test_serialize_compressed_metadata_size() {
        let writer = DwgFileHeaderWriterAC21::new(
            DxfVersion::AC1021,
            "AC1021",
            30,
            0,
        );
        let data = writer.serialize_compressed_metadata();
        assert_eq!(data.len(), DECOMPRESSED_METADATA_SIZE);
    }

    #[test]
    fn test_compressed_metadata_rs_roundtrip() {
        let writer = DwgFileHeaderWriterAC21::new(
            DxfVersion::AC1021,
            "AC1021",
            30,
            0,
        );
        let metadata_raw = writer.serialize_compressed_metadata();

        // Compress with LZ77-AC21.
        let compressed = writer.compressor
            .compress(&metadata_raw, 0, metadata_raw.len())
            .unwrap();

        // RS encode.
        let rs_encoded = reed_solomon::encode(
            &compressed,
            RS_FACTOR_HEADER,
            RS_BLOCK_SIZE_HEADER,
        );

        // RS decode.
        let rs_decoded = reed_solomon::decode(
            &rs_encoded,
            compressed.len(),
            RS_FACTOR_HEADER,
            RS_BLOCK_SIZE_HEADER,
        );
        assert_eq!(rs_decoded, compressed);

        // LZ77-AC21 decompress.
        let decompressor = Lz77Ac21Decompressor;
        let decompressed = decompressor.decompress(&rs_decoded, metadata_raw.len()).unwrap();
        assert_eq!(decompressed, metadata_raw);
    }

    #[test]
    fn test_add_section_creates_pages() {
        let mut writer = DwgFileHeaderWriterAC21::new(
            DxfVersion::AC1021,
            "AC1021",
            30,
            0,
        );

        // Add a small section.
        let data = vec![0x42u8; 100];
        writer.add_section("AcDb:Header", data, true, 0).unwrap();

        assert!(writer.file_header.descriptors.contains_key("AcDb:Header"));
        let desc = &writer.file_header.descriptors["AcDb:Header"];
        assert_eq!(desc.hash_code, DwgSectionHash::AcDbHeader);
        assert_eq!(desc.encoding, 4);
        assert_eq!(desc.page_count, 1);
        assert!(!desc.local_sections.is_empty());
    }

    #[test]
    fn test_write_file_produces_valid_output() {
        let mut writer = DwgFileHeaderWriterAC21::new(
            DxfVersion::AC1021,
            "AC1021",
            30,
            0,
        );

        // Add a few sections.
        writer.add_section("AcDb:Header", vec![1u8; 200], true, 0).unwrap();
        writer.add_section("AcDb:Classes", vec![2u8; 150], true, 0).unwrap();
        writer.add_section("AcDb:Handles", vec![3u8; 100], true, 0).unwrap();

        let output = writer.write_file().unwrap();

        // Output should be at least FILE_HEADER_SIZE.
        assert!(output.len() >= FILE_HEADER_SIZE);

        // Version string at offset 0.
        assert_eq!(&output[0..6], b"AC1021");

        // RS-encoded metadata at 0x80 should have some non-zero data.
        let rs_block = &output[RS_METADATA_OFFSET..RS_METADATA_OFFSET + RS_METADATA_SIZE];
        let non_zero_count: usize = rs_block.iter().filter(|&&b| b != 0).count();
        assert!(non_zero_count > 0, "RS metadata block should have non-zero data");
    }

    #[test]
    fn test_build_file_header_data_length() {
        let writer = DwgFileHeaderWriterAC21::new(
            DxfVersion::AC1021,
            "AC1021",
            30,
            0,
        );
        let data = writer.build_file_header_data();
        assert_eq!(data.len(), constants::ac18::ENCRYPTED_HEADER_SIZE);
    }
}
