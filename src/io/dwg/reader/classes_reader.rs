//! DWG Classes section reader.
//!
//! Reads DXF class definitions from the `AcDb:Classes` section in a DWG file.
//! The section contains type information for all custom object/entity classes
//! used in the drawing beyond the built-in ones.
//!
//! Mirrors ACadSharp's `DwgClassesReader`.

use crate::classes::{DxfClass, DxfClassCollection, ProxyFlags};
use crate::error::Result;
use crate::io::dwg::constants::sentinels;
use crate::io::dwg::reader::merged_reader::DwgMergedReader;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::io::dwg::reader::stream_reader_base::{get_stream_handler, DwgStreamReaderBase};
use crate::io::dwg::section_io::SectionIO;
use crate::types::DxfVersion;

/// Reader for the DWG `AcDb:Classes` section.
///
/// Reads class definitions that map DXF names to C++ class names and
/// proxy capability flags.
pub struct DwgClassesReader {
    /// Raw section data bytes (decompressed/decrypted by caller).
    data: Vec<u8>,
    /// DWG version being read.
    version: DxfVersion,
    /// Version from the file header.
    file_header_version: DxfVersion,
    /// Maintenance version from the file header.
    file_header_maintenance_version: u8,
}

impl DwgClassesReader {
    /// Create a new classes reader.
    ///
    /// # Arguments
    /// * `version` - The DWG version being read.
    /// * `data` - Raw section bytes (already decompressed/decrypted).
    /// * `file_header_version` - Version from the DWG file header.
    /// * `file_header_maintenance_version` - Maintenance version from the file header.
    pub fn new(
        version: DxfVersion,
        data: Vec<u8>,
        file_header_version: DxfVersion,
        file_header_maintenance_version: u8,
    ) -> Self {
        Self {
            data,
            version,
            file_header_version,
            file_header_maintenance_version,
        }
    }

    /// Read all class definitions from the section.
    ///
    /// Returns a `DxfClassCollection` containing all class definitions
    /// found in the section data.
    pub fn read(&self) -> Result<DxfClassCollection> {
        let mut classes = DxfClassCollection::new();
        let mut reader = get_stream_handler(self.version, self.data.clone());
        let sio = SectionIO::new(self.version);

        // SN: 0x8D 0xA1 0xC4 0xB8 0xC4 0xA9 0xF8 0xC5 0xC0 0xDC 0xF4 0x5F 0xE7 0xCF 0xB6 0x8A
        let sentinel = reader.read_sentinel()?;
        if let Some(expected) = sentinels::start_sentinel("AcDb:Classes") {
            SectionIO::check_sentinel(&sentinel, expected);
        }

        // RL: size of class data area
        let size = reader.read_raw_long()? as i64;

        // R2010+ (only present if the maintenance version is greater than 3!) or R2018+
        if (self.file_header_version >= DxfVersion::AC1024
            && self.file_header_maintenance_version > 3)
            || self.file_header_version > DxfVersion::AC1027
        {
            // RL: unknown, possibly the high 32 bits of a 64-bit size?
            let _unknown = reader.read_raw_long()?;
        }

        let flag_pos: i64;

        // +R2007 Only:
        if sio.r2007_plus {
            // Setup readers
            flag_pos = reader.position_in_bits() + reader.read_raw_long()? as i64 - 1;
            let saved_offset = reader.position_in_bits();
            let end_section = reader.set_position_by_flag(flag_pos)?;

            reader.set_position_in_bits(saved_offset);

            // Setup the text reader for versions 2007 and above
            // Create a copy of the stream data for the text reader
            let mut text_reader = get_stream_handler(self.version, self.data.clone());
            // Set the position and use the flag
            text_reader.set_position_in_bits(end_section);

            // Create merged reader (main + text, no handle reader needed for classes)
            let handle_reader = DwgStreamReaderBase::new(vec![], self.version);
            let mut merged = DwgMergedReader::new(reader, text_reader, handle_reader, self.version);

            // BL: 0x00
            merged.read_bit_long()?;
            // B: flag - to find the data string at the end of the section
            merged.read_bit()?;

            // We read sets of these until we exhaust the data
            while merged.position_in_bits() < end_section {
                let class = Self::read_class(&mut merged, sio.r2004_plus)?;
                classes.add_or_update(class);
            }

            merged.set_position_in_bits(flag_pos + 1);

            // RS: CRC
            merged.reset_shift()?;

            // SN: 0x72 0x5E 0x3B 0x47 0x3B 0x56 0x07 0x3A 0x3F 0x23 0x0B 0xA0 0x18 0x30 0x49 0x75
            let sentinel = merged.read_sentinel()?;
            if let Some(expected) = sentinels::end_sentinel("AcDb:Classes") {
                SectionIO::check_sentinel(&sentinel, expected);
            }
        } else {
            let end_section = reader.position() as i64 + size;

            if self.version == DxfVersion::AC1018 {
                // BS: Maximum class number
                reader.read_bit_short()?;
                // RC: 0x00
                reader.read_raw_char()?;
                // RC: 0x00
                reader.read_raw_char()?;
                // B: true
                reader.read_bit()?;
            }

            // We read sets of these until we exhaust the data
            while (reader.position() as i64) < end_section {
                let class = Self::read_class(&mut reader, sio.r2004_plus)?;
                classes.add_or_update(class);
            }

            // RS: CRC
            reader.reset_shift()?;

            // SN: 0x72 0x5E 0x3B 0x47 0x3B 0x56 0x07 0x3A 0x3F 0x23 0x0B 0xA0 0x18 0x30 0x49 0x75
            let sentinel = reader.read_sentinel()?;
            if let Some(expected) = sentinels::end_sentinel("AcDb:Classes") {
                SectionIO::check_sentinel(&sentinel, expected);
            }
        }

        Ok(classes)
    }

    /// Read a single DXF class definition from the stream.
    fn read_class(reader: &mut dyn IDwgStreamReader, r2004_plus: bool) -> Result<DxfClass> {
        let mut class = DxfClass::default();

        // BS: classnum
        class.class_number = reader.read_bit_short()?;
        // BS: version – in R14, becomes a flag indicating whether objects can be moved, edited, etc.
        class.proxy_flags = ProxyFlags(reader.read_bit_short()? as u16);

        // TV: appname
        class.application_name = reader.read_variable_text()?;
        // TV: cplusplusclassname
        class.cpp_class_name = reader.read_variable_text()?;
        // TV: classdxfname
        class.dxf_name = reader.read_variable_text()?;

        // B: wasazombie
        class.was_zombie = reader.read_bit()?;
        // BS: itemclassid -- 0x1F2 for classes which produce entities, 0x1F3 for classes which produce objects.
        class.item_class_id = reader.read_bit_short()?;
        class.is_an_entity = class.item_class_id == 0x1F2;

        if r2004_plus {
            // BL: Number of objects created of this type in the current DB (DXF 91).
            class.instance_count = reader.read_bit_long()?;

            // BL: Dwg Version
            class.dwg_version = reader.read_bit_long()?;
            // BL: Maintenance release version.
            class.maintenance_version = reader.read_bit_long()? as i16;

            // BL: Unknown (normally 0L)
            reader.read_bit_long()?;
            // BL: Unknown (normally 0L)
            reader.read_bit_long()?;
        }

        Ok(class)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classes_reader_creation() {
        let reader = DwgClassesReader::new(
            DxfVersion::AC1015,
            vec![0u8; 64],
            DxfVersion::AC1015,
            0,
        );
        assert_eq!(reader.version, DxfVersion::AC1015);
    }
}

