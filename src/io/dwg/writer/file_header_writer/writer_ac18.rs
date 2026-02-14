//! DWG file header writer for R2004+ (AC18).
//!
//! Writes a page-based file layout with LZ77 compression, encrypted page
//! headers, section descriptors, and a CRC-32 protected file header.
//!
//! Mirrors ACadSharp's `DwgFileHeaderWriterAC18`.

use crate::error::Result;
use crate::io::dwg::checksum::{self, MAGIC_SEQUENCE};
use crate::io::dwg::compression::{Compressor, lz77_ac18::Lz77Ac18Compressor};
use crate::io::dwg::crc;
use crate::io::dwg::file_header::{
    DwgFileHeaderAC18, DwgLocalSectionMap, DwgSectionDescriptor,
};
use crate::types::DxfVersion;



use super::IDwgFileHeaderWriter;

/// Size of the file header preamble (bytes 0x00–0xFF).
const FILE_HEADER_SIZE: usize = 0x100;

/// AC18 file header writer.
#[allow(dead_code)]
pub struct DwgFileHeaderWriterAC18 {
    version: DxfVersion,
    version_string: String,
    maintenance_version: u8,
    code_page: u16,
    file_header: DwgFileHeaderAC18,
    local_sections: Vec<DwgLocalSectionMap>,
    /// The output stream assembled incrementally.
    output: Vec<u8>,
    compressor: Lz77Ac18Compressor,
}

impl DwgFileHeaderWriterAC18 {
    /// Create a new AC18 file header writer.
    pub fn new(
        version: DxfVersion,
        version_string: &str,
        code_page: u16,
        maintenance_version: u8,
    ) -> Self {
        let file_header = DwgFileHeaderAC18::new(version);
        // Pre-fill with 0x100 bytes of zeros for the preamble
        let output = vec![0u8; FILE_HEADER_SIZE];

        Self {
            version,
            version_string: version_string.to_string(),
            maintenance_version,
            code_page,
            file_header,
            local_sections: Vec::new(),
            output,
            compressor: Lz77Ac18Compressor::new(),
        }
    }

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
            // Pad to decompressed_size
            let mut holder = Vec::with_capacity(decompressed_size);
            holder.extend_from_slice(&buffer[offset..offset + total_size]);
            holder.resize(decompressed_size, 0);
            self.compressor.compress(&holder, 0, decompressed_size)
        } else {
            let mut data = Vec::with_capacity(decompressed_size);
            data.extend_from_slice(&buffer[offset..offset + total_size]);
            data.resize(decompressed_size, 0);
            Ok(data)
        }
    }

    /// Create a local section page: compress data, write page header + data.
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
        local_map.decompressed_size = total_size as u64;
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

    /// Compress data and compute its checksum, then write to output.
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

        // Write page header and compressed data
        let header_buf = Self::build_page_header_data(section);
        self.output.extend_from_slice(&header_buf);
        self.output.extend_from_slice(&compressed);

        Ok(())
    }

    /// Write section descriptors (the section map).
    fn write_descriptors(&mut self) -> Result<()> {
        let descriptors: Vec<DwgSectionDescriptor> =
            self.file_header.descriptors.values().cloned().collect();

        let mut stream = Vec::new();

        // Number of section descriptions
        stream.extend_from_slice(&(descriptors.len() as i32).to_le_bytes());
        // 0x02
        stream.extend_from_slice(&2i32.to_le_bytes());
        // 0x7400
        stream.extend_from_slice(&0x7400i32.to_le_bytes());
        // 0x00
        stream.extend_from_slice(&0i32.to_le_bytes());
        // NumDescriptions (again)
        stream.extend_from_slice(&(descriptors.len() as i32).to_le_bytes());

        for desc in &descriptors {
            // Size of section (8 bytes)
            stream.extend_from_slice(&desc.compressed_size.to_le_bytes());
            // Page count
            stream.extend_from_slice(&desc.page_count.to_le_bytes());
            // Max decompressed size
            stream.extend_from_slice(&(desc.decompressed_size as i32).to_le_bytes());
            // Unknown (1)
            stream.extend_from_slice(&1i32.to_le_bytes());
            // Compressed code
            stream.extend_from_slice(&desc.compressed_code.to_le_bytes());
            // Section Id
            stream.extend_from_slice(&desc.section_id.to_le_bytes());
            // Encrypted
            stream.extend_from_slice(&desc.encrypted.to_le_bytes());

            // Section Name (64 bytes, zero-padded)
            let mut name_arr = [0u8; 64];
            let name_bytes = desc.name.as_bytes();
            let copy_len = name_bytes.len().min(64);
            name_arr[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
            stream.extend_from_slice(&name_arr);

            for local_map in &desc.local_sections {
                if local_map.page_number > 0 {
                    // Page number
                    stream.extend_from_slice(&local_map.page_number.to_le_bytes());
                    // Data size (compressed)
                    stream.extend_from_slice(&(local_map.compressed_size as i32).to_le_bytes());
                    // Start offset (8 bytes)
                    stream.extend_from_slice(&local_map.offset.to_le_bytes());
                }
            }
        }

        // Section map page type: 0x4163003b
        let mut section_holder = DwgLocalSectionMap::new();
        section_holder.section_map = 0x4163003Bu32 as i32;

        self.write_magic_number();
        section_holder.seeker = self.output.len() as u64;

        self.compress_checksum(&mut section_holder, &stream)?;

        let diff = checksum::compression_padding(
            self.output.len() - section_holder.seeker as usize,
        );
        for i in 0..diff {
            self.output.push(MAGIC_SEQUENCE[i % 256]);
        }
        section_holder.size = (self.output.len() - section_holder.seeker as usize) as u64;

        section_holder.page_number = self.local_sections.len() as i32 + 1;
        self.local_sections.push(section_holder);

        Ok(())
    }

    /// Write the page map records (section page map).
    fn write_records(&mut self) -> Result<()> {
        self.write_magic_number();

        let mut section = DwgLocalSectionMap::new();
        section.section_map = 0x41630E3Bu32 as i32;
        section.page_number = self.local_sections.len() as i32 + 1;
        self.local_sections.push(section.clone());

        let counter = self.local_sections.len() * 8;
        let seeker = self.output.len();
        let size = counter + checksum::compression_padding(counter);

        // Build the page map data
        let mut stream = Vec::new();
        for item in &self.local_sections {
            // Page number (4 bytes)
            stream.extend_from_slice(&item.page_number.to_le_bytes());
            // Section size (4 bytes)
            stream.extend_from_slice(&(item.size as i32).to_le_bytes());
        }

        // Get the last entry's index to update it
        let last_idx = self.local_sections.len() - 1;
        self.local_sections[last_idx].seeker = seeker as u64;
        self.local_sections[last_idx].size = size as u64;

        // Compress and write
        let mut section_map = self.local_sections[last_idx].clone();
        self.compress_checksum(&mut section_map, &stream)?;
        self.local_sections[last_idx] = section_map;

        // Update file header fields
        let last = &self.local_sections[self.local_sections.len() - 1];
        self.file_header.gap_amount = 0;
        self.file_header.last_page_id = last.page_number;
        self.file_header.last_section_addr =
            last.seeker + size as u64 - 256;
        self.file_header.section_amount = (self.local_sections.len() - 1) as u32;
        self.file_header.page_map_address = seeker as u64;

        Ok(())
    }

    /// Build the encrypted 0x6C-byte file header data with CRC-32.
    fn build_file_header_data(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // 0x00: "AcFssFcAJMB\0" (12 bytes)
        buf.extend_from_slice(b"AcFssFcAJMB");
        buf.push(0);

        // 0x0C: 0x00
        buf.extend_from_slice(&0i32.to_le_bytes());
        // 0x10: 0x6c
        buf.extend_from_slice(&0x6Ci32.to_le_bytes());
        // 0x14: 0x04
        buf.extend_from_slice(&0x04i32.to_le_bytes());
        // 0x18: Root tree node gap
        buf.extend_from_slice(&self.file_header.root_tree_node_gap.to_le_bytes());
        // 0x1C: Left gap
        buf.extend_from_slice(&self.file_header.left_gap.to_le_bytes());
        // 0x20: Right gap
        buf.extend_from_slice(&self.file_header.right_gap.to_le_bytes());
        // 0x24: Unknown (1)
        buf.extend_from_slice(&1i32.to_le_bytes());
        // 0x28: Last section page Id
        buf.extend_from_slice(&self.file_header.last_page_id.to_le_bytes());

        // 0x2C: Last section page end address (8 bytes)
        buf.extend_from_slice(&self.file_header.last_section_addr.to_le_bytes());
        // 0x34: Second header data address (8 bytes)
        buf.extend_from_slice(&self.file_header.second_header_addr.to_le_bytes());

        // 0x3C: Gap amount
        buf.extend_from_slice(&self.file_header.gap_amount.to_le_bytes());
        // 0x40: Section page amount
        buf.extend_from_slice(&self.file_header.section_amount.to_le_bytes());

        // 0x44: 0x20
        buf.extend_from_slice(&0x20i32.to_le_bytes());
        // 0x48: 0x80
        buf.extend_from_slice(&0x80i32.to_le_bytes());
        // 0x4C: 0x40
        buf.extend_from_slice(&0x40i32.to_le_bytes());

        // 0x50: Section Page Map Id
        buf.extend_from_slice(&self.file_header.section_page_map_id.to_le_bytes());
        // 0x54: Section Page Map address - 0x100 (8 bytes)
        let pma = self.file_header.page_map_address.wrapping_sub(256);
        buf.extend_from_slice(&pma.to_le_bytes());
        // 0x5C: Section Map Id
        buf.extend_from_slice(&self.file_header.section_map_id.to_le_bytes());
        // 0x60: Section page array size
        buf.extend_from_slice(&self.file_header.section_array_page_size.to_le_bytes());
        // 0x64: Gap array size
        buf.extend_from_slice(&self.file_header.gap_array_size.to_le_bytes());

        // 0x68: CRC-32 (initially zero, computed over all 0x6C bytes)
        let crc_pos = buf.len();
        buf.extend_from_slice(&0u32.to_le_bytes());

        // Compute CRC-32 (seed = 0, over entire buffer including the zero CRC bytes)
        let seed = crc::crc32(0, &buf);
        buf[crc_pos..crc_pos + 4].copy_from_slice(&seed.to_le_bytes());

        // Apply magic sequence XOR
        Self::apply_magic_sequence(&mut buf);

        buf
    }

    /// Write the file metadata: second header copy and preamble fields.
    fn write_file_metadata(&mut self) {
        // Record second header address
        self.file_header.second_header_addr = self.output.len() as u64;

        // Build file header data (needs second_header_addr set first)
        let file_header_data = self.build_file_header_data();

        // Write the second copy of the file header at end of file
        self.output.extend_from_slice(&file_header_data);

        // Now write the preamble at the beginning (bytes 0x00–0xFF)
        // 0x00: "ACXXXX" (6 bytes)
        let version_bytes = self.version_string.as_bytes();
        let copy_len = version_bytes.len().min(6);
        self.output[..copy_len].copy_from_slice(&version_bytes[..copy_len]);

        // 0x06: 5 bytes of 0x00
        for i in 6..11 {
            self.output[i] = 0;
        }

        // 0x0B: Maintenance release version
        self.output[0x0B] = self.maintenance_version;

        // 0x0C: 0x03 (byte)
        self.output[0x0C] = 3;

        // 0x0D: Preview address (4 bytes)
        // Points to the preview section page + page header (0x20)
        let preview_addr = self.get_section_page_addr("AcDb:Preview") + 0x20;
        let preview_bytes = (preview_addr as u32).to_le_bytes();
        self.output[0x0D..0x11].copy_from_slice(&preview_bytes);

        // 0x11: Dwg version (33 for AC1032)
        self.output[0x11] = 33;
        // 0x12: App maintenance release version
        self.output[0x12] = self.maintenance_version;

        // 0x13: Codepage (2 bytes)
        let cp_bytes = self.code_page.to_le_bytes();
        self.output[0x13..0x15].copy_from_slice(&cp_bytes);

        // 0x15: 3 zero bytes
        self.output[0x15] = 0;
        self.output[0x16] = 0;
        self.output[0x17] = 0;

        // 0x18: SecurityType (4 bytes, 0)
        self.output[0x18..0x1C].copy_from_slice(&0i32.to_le_bytes());
        // 0x1C: Unknown long (0)
        self.output[0x1C..0x20].copy_from_slice(&0i32.to_le_bytes());

        // 0x20: Summary info address (4 bytes)
        let summary_addr = self.get_section_page_addr("AcDb:SummaryInfo") + 0x20;
        self.output[0x20..0x24].copy_from_slice(&(summary_addr as u32).to_le_bytes());

        // 0x24: VBA Project Addr (0)
        self.output[0x24..0x28].copy_from_slice(&0u32.to_le_bytes());
        // 0x28: 0x80
        self.output[0x28..0x2C].copy_from_slice(&0x80i32.to_le_bytes());

        // 0x2C: App info address (4 bytes)
        let appinfo_addr = self.get_section_page_addr("AcDb:AppInfo") + 0x20;
        self.output[0x2C..0x30].copy_from_slice(&(appinfo_addr as u32).to_le_bytes());

        // 0x30: 0x80 zero bytes
        for i in 0x30..0x80 {
            self.output[i] = 0;
        }

        // 0x80: Second copy of the file header data
        let fhd_len = file_header_data.len();
        self.output[0x80..0x80 + fhd_len].copy_from_slice(&file_header_data);

        // Remaining bytes: fill with magic sequence fragment
        let fhd_end = 0x80 + fhd_len;
        if fhd_end < FILE_HEADER_SIZE {
            for i in fhd_end..FILE_HEADER_SIZE {
                let seq_idx = (236 + i - fhd_end) % 256;
                self.output[i] = MAGIC_SEQUENCE[seq_idx];
            }
        }

        // Write the second header data at end of file as well
        self.output.extend_from_slice(&file_header_data);

        // Trailing magic sequence (20 bytes from offset 236)
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

impl IDwgFileHeaderWriter for DwgFileHeaderWriterAC18 {
    fn handle_section_offset(&self) -> i32 {
        // For AC18+, offsets are relative within sections (0)
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

        // Assign section_id: first section is 0, then descending from (total - 1)
        let existing_count = self.file_header.descriptors.len();
        descriptor.section_id = if existing_count == 0 {
            0
        } else {
            existing_count as i32
        };

        // Page type for data sections: 0x4163043b
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
        // Set up section array info
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
