//! DWG file header writer for R13–R2000 (AC15).
//!
//! Writes a simple sequential file layout: file header, then each section
//! in order, protected by CRC-8.
//!
//! Layout:
//! ```text
//! FILE HEADER (0x61 bytes)
//! DWG HEADER VARIABLES (section 0)
//! CLASS DEFINITIONS (section 1)
//! OBJ FREE SPACE (section 3)
//! TEMPLATE (section 4)
//! AUX HEADER (section 5)
//! OBJECT DATA (no section number)
//! OBJECT MAP (section 2)
//! PREVIEW (no section number)
//! ```
//!
//! Mirrors ACadSharp's `DwgFileHeaderWriterAC15`.

use crate::error::Result;
use crate::io::dwg::constants::section_names;
use crate::io::dwg::constants::sentinels;
use crate::io::dwg::crc;
use crate::types::DxfVersion;

use byteorder::{LittleEndian, WriteBytesExt};

use super::IDwgFileHeaderWriter;

/// File header size for AC15.
const FILE_HEADER_SIZE: usize = 0x61;

/// AC15 file header writer.
#[allow(dead_code)]
pub struct DwgFileHeaderWriterAC15 {
    version: DxfVersion,
    version_string: String,
    code_page: u16,
    maintenance_version: u8,
    /// Ordered sections: name → (record_number option, data)
    sections: Vec<(String, Option<u8>, Vec<u8>)>,
}

impl DwgFileHeaderWriterAC15 {
    /// Create a new AC15 file header writer.
    pub fn new(
        version: DxfVersion,
        version_string: &str,
        code_page: u16,
        maintenance_version: u8,
    ) -> Self {
        Self {
            version,
            version_string: version_string.to_string(),
            code_page,
            maintenance_version,
            sections: Vec::new(),
        }
    }

    /// Register a section with a record number.
    fn register_section(&mut self, name: &str, record_number: Option<u8>, data: Vec<u8>) {
        self.sections.push((name.to_string(), record_number, data));
    }

    /// Build the file header bytes (0x61 bytes).
    fn build_file_header(&self, records: &[(u8, i64, i64)], preview_seeker: i64) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        // 0x00: "ACXXXX" version string (6 bytes)
        let version_bytes = self.version_string.as_bytes();
        let mut vb = [0u8; 6];
        let copy_len = version_bytes.len().min(6);
        vb[..copy_len].copy_from_slice(&version_bytes[..copy_len]);
        buf.extend_from_slice(&vb);

        // 0x06: 5 zero bytes, then ACADMAINTVER, then 0x01
        buf.push(0);
        buf.push(0);
        buf.push(0);
        buf.push(0);
        buf.push(0);
        buf.push(self.maintenance_version);
        buf.push(1);

        // 0x0D: Preview seeker (4 bytes, little-endian i32)
        buf.write_i32::<LittleEndian>(preview_seeker as i32)?;

        // 0x11: 0x1B (app DWG version)
        buf.push(0x1B);
        // 0x12: 0x19 (app maintenance version)
        buf.push(0x19);

        // 0x13: Code page (2 bytes, little-endian u16)
        buf.write_u16::<LittleEndian>(self.code_page)?;

        // 0x15: Number of section records (4 bytes, little-endian i32)
        buf.write_i32::<LittleEndian>(records.len() as i32)?;

        // Section records: each has record_number(1) + seeker(4) + size(4) = 9 bytes
        for &(number, seeker, size) in records {
            buf.push(number);
            buf.write_i32::<LittleEndian>(seeker as i32)?;
            buf.write_i32::<LittleEndian>(size as i32)?;
        }

        // Pad with zeros to reach the CRC position
        // The CRC is just before the end sentinel at offset FILE_HEADER_SIZE - 18
        // (16 for sentinel + 2 for CRC)
        let crc_offset = FILE_HEADER_SIZE - 18; // 0x61 - 18 = 79
        while buf.len() < crc_offset {
            buf.push(0);
        }

        // CRC-8 over all bytes so far
        let crc_val = crc::crc8(0xC0C1, &buf);
        buf.write_u16::<LittleEndian>(crc_val)?;

        // End sentinel (16 bytes)
        buf.extend_from_slice(&sentinels::FILE_HEADER_END_AC15);

        Ok(buf)
    }
}

impl IDwgFileHeaderWriter for DwgFileHeaderWriterAC15 {
    fn handle_section_offset(&self) -> i32 {
        // The handle section offset is the sum of file header + all data sections
        // up to (but not including) the objects section
        let mut offset = FILE_HEADER_SIZE as i32;
        for (name, _, data) in &self.sections {
            if name == section_names::ACDB_OBJECTS {
                break;
            }
            offset += data.len() as i32;
        }
        offset
    }

    fn add_section(
        &mut self,
        name: &str,
        data: Vec<u8>,
        _is_compressed: bool,
        _decomp_size: usize,
    ) -> Result<()> {
        // Map section names to record numbers for AC15
        let record_number = match name {
            section_names::HEADER => Some(0u8),
            section_names::CLASSES => Some(1),
            section_names::HANDLES => Some(2),
            section_names::OBJ_FREE_SPACE => Some(3),
            section_names::TEMPLATE => Some(4),
            section_names::AUX_HEADER => Some(5),
            _ => None, // ACDB_OBJECTS, PREVIEW have no record number
        };
        self.register_section(name, record_number, data);
        Ok(())
    }

    fn write_file(&mut self) -> Result<Vec<u8>> {
        // Calculate seekers for each section
        let mut curr_offset = FILE_HEADER_SIZE as i64;
        let mut records: Vec<(u8, i64, i64)> = Vec::new();
        let mut preview_seeker: i64 = -1;

        for (name, record_number, data) in &self.sections {
            let seeker = curr_offset;
            let size = data.len() as i64;

            if let Some(num) = record_number {
                records.push((*num, seeker, size));
            }

            if name == section_names::PREVIEW {
                preview_seeker = seeker;
            }

            curr_offset += size;
        }

        // Build the file header
        let file_header = self.build_file_header(&records, preview_seeker)?;

        // Assemble the complete file: header + all section data
        let mut output = file_header;
        for (_, _, data) in &self.sections {
            output.extend_from_slice(data);
        }

        Ok(output)
    }
}
