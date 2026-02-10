//! DWG Handle/Object Map section reader.
//!
//! Reads the handle-to-file-offset map from the `AcDb:Handles` section.
//! This map allows looking up any object's file position by its handle.
//!
//! The section is organized as a series of chunks, each containing
//! delta-encoded handle/offset pairs. Chunks are limited to 2032 bytes.
//!
//! Mirrors ACadSharp's `DwgHandleReader`.

use std::collections::HashMap;

use crate::error::Result;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::io::dwg::reader::stream_reader_base::get_stream_handler;
use crate::types::DxfVersion;

/// Reader for the DWG `AcDb:Handles` (object map) section.
///
/// Builds a map from object handles to their byte offsets in the file,
/// used to locate objects during later parsing.
pub struct DwgHandleReader {
    /// Raw section data bytes (decompressed/decrypted by caller).
    data: Vec<u8>,
    /// DWG version being read.
    version: DxfVersion,
}

impl DwgHandleReader {
    /// Create a new handle reader.
    ///
    /// # Arguments
    /// * `version` - The DWG version being read.
    /// * `data` - Raw section bytes (already decompressed/decrypted).
    pub fn new(version: DxfVersion, data: Vec<u8>) -> Self {
        Self { data, version }
    }

    /// Read the handle-to-offset map.
    ///
    /// Returns a `HashMap<u64, i64>` mapping each object's handle
    /// to its signed byte offset in the file.
    pub fn read(&self) -> Result<HashMap<u64, i64>> {
        let mut object_map: HashMap<u64, i64> = HashMap::new();
        let mut reader = get_stream_handler(self.version, self.data.clone());

        // Repeat until section size == 2 (the last empty section, except CRC):
        loop {
            // Set the "last handle" to all 0 and the "last loc" to 0L
            let mut last_handle: u64 = 0;
            let mut last_loc: i64 = 0;

            // Short: size of this section. Note this is in BIGENDIAN order (MSB first)
            let hi = reader.read_byte()? as i32;
            let lo = reader.read_byte()? as i32;
            let size = (hi << 8) | lo;

            if size == 2 {
                break;
            }

            let start_pos = reader.position() as i64;
            let mut max_section_offset = size - 2;
            // Note that each section is cut off at a maximum length of 2032
            if max_section_offset > 2032 {
                max_section_offset = 2032;
            }

            let last_position = start_pos + max_section_offset as i64;

            // Repeat until out of data for this section:
            while (reader.position() as i64) < last_position {
                // Offset of this handle from last handle as modular char
                let offset = reader.read_modular_char()?;
                last_handle += offset;

                // Offset of location in file from last loc as signed modular char.
                // (note that location offsets can be negative, if the terminating
                // byte has the 4 bit set).
                last_loc += reader.read_signed_modular_char()?;

                if offset > 0 {
                    object_map.insert(last_handle, last_loc);
                }
                // else: 0 offset, wrong reference — skip
            }

            // CRC (most significant byte followed by least significant byte)
            let _crc = ((reader.read_byte()? as u32) << 8) + reader.read_byte()? as u32;
        }

        Ok(object_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_reader_creation() {
        let reader = DwgHandleReader::new(DxfVersion::AC1015, vec![]);
        assert_eq!(reader.version, DxfVersion::AC1015);
    }

    #[test]
    fn test_handle_reader_empty_section() {
        // Size == 2 (big-endian): 0x00 0x02 → terminates immediately
        let data = vec![0x00, 0x02];
        let reader = DwgHandleReader::new(DxfVersion::AC1015, data);
        let map = reader.read().unwrap();
        assert!(map.is_empty());
    }
}
