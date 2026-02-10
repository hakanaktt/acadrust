//! DWG Classes section writer.
//!
//! Writes all DXF class definitions to the `AcDb:Classes` section.
//! The section is framed by start/end sentinels with CRC-8 protection.
//!
//! Mirrors ACadSharp's `DwgClassesWriter`.

use crate::classes::DxfClass;
use crate::error::Result;
use crate::io::dwg::constants::sentinels;
use crate::io::dwg::section_io::SectionIO;
use crate::io::dwg::writer::stream_writer::IDwgStreamWriter;
use crate::io::dwg::writer::stream_writer_base::DwgStreamWriterBase;
use crate::io::dwg::writer::merged_writer::DwgMergedStreamWriter;
use crate::types::DxfVersion;

use byteorder::{LittleEndian, WriteBytesExt};

/// Writer for the DWG `AcDb:Classes` section.
pub struct DwgClassesWriter {
    version: DxfVersion,
}

impl DwgClassesWriter {
    /// Create a new classes writer.
    pub fn new(version: DxfVersion) -> Self {
        Self { version }
    }

    /// Write the classes section, returning the raw section bytes.
    pub fn write(
        &self,
        classes: &[DxfClass],
        maintenance_version: u8,
    ) -> Result<Vec<u8>> {
        let sio = SectionIO::new(self.version);

        // Build inner data
        let inner_data: Vec<u8>;
        if sio.r2007_plus {
            let mut writer = DwgMergedStreamWriter::new(self.version);
            writer.save_position_for_size()?;
            Self::write_class_data(&sio, &mut writer, classes)?;
            writer.write_spear_shift()?;
            inner_data = writer.main_writer().data().to_vec();
        } else {
            let mut writer = DwgStreamWriterBase::new(self.version);
            Self::write_class_data(&sio, &mut writer, classes)?;
            writer.write_spear_shift()?;
            inner_data = writer.into_data();
        }

        // Wrap with sentinels + CRC
        self.write_size_and_crc(&inner_data, &sio, maintenance_version)
    }

    /// Write class definitions to the stream.
    fn write_class_data(
        sio: &SectionIO,
        writer: &mut dyn IDwgStreamWriter,
        classes: &[DxfClass],
    ) -> Result<()> {
        if sio.r2004_plus {
            // BS: Maximum class number
            let max_class_num = classes.iter().map(|c| c.class_number).max().unwrap_or(0);
            writer.write_bit_short(max_class_num)?;
            // RC: 0x00
            writer.write_byte(0)?;
            // RC: 0x00
            writer.write_byte(0)?;
            // B: true
            writer.write_bit(true)?;
        }

        for c in classes {
            // BS: classnum
            writer.write_bit_short(c.class_number)?;
            // BS: version (proxy flags)
            writer.write_bit_short(c.proxy_flags.0 as i16)?;
            // TV: appname
            writer.write_variable_text(&c.application_name)?;
            // TV: cplusplusclassname
            writer.write_variable_text(&c.cpp_class_name)?;
            // TV: classdxfname
            writer.write_variable_text(&c.dxf_name)?;
            // B: wasazombie
            writer.write_bit(c.was_zombie)?;
            // BS: itemclassid
            writer.write_bit_short(c.item_class_id)?;

            if sio.r2004_plus {
                // BL: Number of objects created of this type
                writer.write_bit_long(c.instance_count)?;
                // BL: Dwg Version
                writer.write_bit_long(c.dwg_version)?;
                // BL: Maintenance release version
                writer.write_bit_long(c.maintenance_version as i32)?;
                // BL: Unknown (normally 0)
                writer.write_bit_long(0)?;
                // BL: Unknown (normally 0)
                writer.write_bit_long(0)?;
            }
        }

        Ok(())
    }

    /// Write the section wrapper: start sentinel, size, CRC, end sentinel.
    fn write_size_and_crc(
        &self,
        inner_data: &[u8],
        sio: &SectionIO,
        maintenance_version: u8,
    ) -> Result<Vec<u8>> {
        let mut output = Vec::new();

        // Start sentinel
        output.extend_from_slice(&sentinels::CLASSES_START);

        // Build CRC-covered region
        let mut crc_region = Vec::new();
        // RL: size of class data area
        crc_region.write_i32::<LittleEndian>(inner_data.len() as i32)?;

        // R2010+ with maintenance > 3, or R2018+:
        if (sio.r2010_plus && maintenance_version > 3) || sio.r2018_plus {
            crc_region.write_i32::<LittleEndian>(0)?;
        }

        crc_region.extend_from_slice(inner_data);

        // CRC-8 over the region
        let crc = crate::io::dwg::crc::crc8(0xC0C1, &crc_region);
        output.extend_from_slice(&crc_region);

        // RS: CRC
        output.write_u16::<LittleEndian>(crc)?;

        // End sentinel
        output.extend_from_slice(&sentinels::CLASSES_END);

        // R2004+: 8 unknown trailing bytes (ODA writes 0)
        if sio.r2004_plus {
            output.write_i32::<LittleEndian>(0)?;
            output.write_i32::<LittleEndian>(0)?;
        }

        Ok(output)
    }
}
