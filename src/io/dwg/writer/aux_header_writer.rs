//! DWG Auxiliary Header section writer.
//!
//! Writes the auxiliary header section containing version stamps,
//! timestamps, maintenance version, handle seed, and save counts.
//!
//! Mirrors ACadSharp's `DwgAuxHeaderWriter`.

use crate::document::HeaderVariables;
use crate::error::Result;
use crate::io::dwg::section_io::SectionIO;
use crate::io::dwg::writer::stream_writer::IDwgStreamWriter;
use crate::io::dwg::writer::stream_writer_base::DwgStreamWriterBase;
use crate::types::DxfVersion;

/// Writer for the DWG AuxHeader section.
pub struct DwgAuxHeaderWriter {
    version: DxfVersion,
}

impl DwgAuxHeaderWriter {
    /// Create a new aux header writer.
    pub fn new(version: DxfVersion) -> Self {
        Self { version }
    }

    /// Write the AuxHeader section, returning the raw section bytes.
    pub fn write(
        &self,
        header: &HeaderVariables,
        maintenance_version: i16,
    ) -> Result<Vec<u8>> {
        let sio = SectionIO::new(self.version);
        let mut writer = DwgStreamWriterBase::new(self.version);

        // RC: 0xff 0x77 0x01
        writer.write_byte(0xFF)?;
        writer.write_byte(0x77)?;
        writer.write_byte(0x01)?;

        // RS: DWG version
        let ver_short = self.version as i16;
        writer.write_raw_short(ver_short)?;
        // RS: Maintenance version
        writer.write_raw_short(maintenance_version)?;

        // RL: Number of saves (starts at 1)
        writer.write_raw_long(1)?;
        // RL: -1
        writer.write_raw_long(-1)?;

        // RS: Number of saves part 1
        writer.write_raw_short(1)?;
        // RS: Number of saves part 2
        writer.write_raw_short(0)?;

        // RL: 0
        writer.write_raw_long(0)?;

        // RS: DWG version string
        writer.write_raw_short(ver_short)?;
        // RS: Maintenance version
        writer.write_raw_short(maintenance_version)?;

        // RS: DWG version string (again)
        writer.write_raw_short(ver_short)?;
        // RS: Maintenance version (again)
        writer.write_raw_short(maintenance_version)?;

        // RS: 0x0005
        writer.write_raw_short(0x0005)?;
        // RS: 0x0893
        writer.write_raw_short(0x0893)?;
        // RS: 0x0005
        writer.write_raw_short(0x0005)?;
        // RS: 0x0893
        writer.write_raw_short(0x0893)?;
        // RS: 0x0000
        writer.write_raw_short(0x0000)?;
        // RS: 0x0001
        writer.write_raw_short(0x0001)?;

        // RL: 0x0000 (×5)
        writer.write_raw_long(0)?;
        writer.write_raw_long(0)?;
        writer.write_raw_long(0)?;
        writer.write_raw_long(0)?;
        writer.write_raw_long(0)?;

        // TD: TDCREATE (creation datetime)
        let (jdate, ms) = Self::timestamp_to_julian(header.create_date_julian);
        writer.write_8bit_julian_date(jdate, ms)?;

        // TD: TDUPDATE (update datetime)
        let (jdate, ms) = Self::timestamp_to_julian(header.update_date_julian);
        writer.write_8bit_julian_date(jdate, ms)?;

        // RL: HANDSEED (if < 0x7fffffff, otherwise -1)
        let handseed: i32 = if header.handle_seed <= 0x7FFFFFFF {
            header.handle_seed as i32
        } else {
            -1
        };
        writer.write_raw_long(handseed)?;

        // RL: Educational plot stamp (default 0)
        writer.write_raw_long(0)?;

        // RS: 0
        writer.write_raw_short(0)?;
        // RS: Number of saves part 1 – number of saves part 2
        writer.write_raw_short(1)?;
        // RL: 0
        writer.write_raw_long(0)?;
        // RL: 0
        writer.write_raw_long(0)?;
        // RL: 0
        writer.write_raw_long(0)?;
        // RL: Number of saves
        writer.write_raw_long(1)?;
        // RL: 0 (×4)
        writer.write_raw_long(0)?;
        writer.write_raw_long(0)?;
        writer.write_raw_long(0)?;
        writer.write_raw_long(0)?;

        // R2018+:
        if sio.r2018_plus {
            writer.write_raw_short(0)?;
            writer.write_raw_short(0)?;
            writer.write_raw_short(0)?;
        }

        Ok(writer.into_data())
    }

    /// Convert a Unix timestamp (f64) back to (julian_day, milliseconds).
    fn timestamp_to_julian(timestamp: f64) -> (i32, i32) {
        let unix_days = timestamp / 86400.0;
        let jdate = (unix_days + 2440587.5) as i32;
        let remainder_secs = timestamp - (jdate as f64 - 2440587.5) * 86400.0;
        let ms = (remainder_secs * 1000.0) as i32;
        (jdate, ms)
    }
}
