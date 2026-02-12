//! DWG Summary Info section writer.
//!
//! Writes document metadata (title, author, keywords, etc.) to the
//! `AcDb:SummaryInfo` section of a DWG file.
//!
//! Mirrors ACadSharp's `DwgSummaryInfoWriter` and the reader's
//! `summary_info_reader.rs`.

use crate::error::Result;
use crate::summary_info::CadSummaryInfo;
use crate::types::DxfVersion;
use byteorder::{LittleEndian, WriteBytesExt};

/// Writer for the DWG `AcDb:SummaryInfo` section.
pub struct DwgSummaryInfoWriter {
    version: DxfVersion,
}

impl DwgSummaryInfoWriter {
    /// Create a new summary info writer.
    pub fn new(version: DxfVersion) -> Self {
        Self { version }
    }

    /// Write the summary info section and return the raw bytes.
    pub fn write(&self, info: &CadSummaryInfo) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        let r2007_plus = self.version >= DxfVersion::AC1021;

        // Strings: title, subject, author, keywords, comments,
        // last_saved_by, revision_number, hyperlink_base
        Self::write_string(&mut data, &info.title, r2007_plus)?;
        Self::write_string(&mut data, &info.subject, r2007_plus)?;
        Self::write_string(&mut data, &info.author, r2007_plus)?;
        Self::write_string(&mut data, &info.keywords, r2007_plus)?;
        Self::write_string(&mut data, &info.comments, r2007_plus)?;
        Self::write_string(&mut data, &info.last_saved_by, r2007_plus)?;
        Self::write_string(&mut data, &info.revision_number, r2007_plus)?;
        Self::write_string(&mut data, &info.hyperlink_base, r2007_plus)?;

        // Total editing time: two RL (Int32) = 0
        data.write_i32::<LittleEndian>(0)?;
        data.write_i32::<LittleEndian>(0)?;

        // Created date (8-bit Julian: two RL)
        Self::write_julian_date(&mut data, info.created_date)?;

        // Modified date (8-bit Julian: two RL)
        Self::write_julian_date(&mut data, info.modified_date)?;

        // Custom properties: Int16 count + pairs of strings
        let nprops = info.properties.len() as i16;
        data.write_i16::<LittleEndian>(nprops)?;
        for (key, value) in &info.properties {
            Self::write_string(&mut data, key, r2007_plus)?;
            Self::write_string(&mut data, value, r2007_plus)?;
        }

        // Two unknown Int32 = 0
        data.write_i32::<LittleEndian>(0)?;
        data.write_i32::<LittleEndian>(0)?;

        Ok(data)
    }

    /// Write a string in the appropriate format for the version.
    fn write_string(data: &mut Vec<u8>, value: &str, r2007_plus: bool) -> Result<()> {
        if r2007_plus {
            // R2007+: BS length (as u16) + UTF-16LE bytes
            let utf16: Vec<u16> = value.encode_utf16().collect();
            data.write_i16::<LittleEndian>(utf16.len() as i16)?;
            for cp in &utf16 {
                data.write_u16::<LittleEndian>(*cp)?;
            }
        } else {
            // Pre-R2007: LE Int16 length + Windows-1252/Latin-1 bytes
            let bytes = value.as_bytes(); // ASCII-ish for most CAD metadata
            data.write_i16::<LittleEndian>(bytes.len() as i16)?;
            data.extend_from_slice(bytes);
        }
        Ok(())
    }

    /// Write a Julian date as two 32-bit LE integers (day + milliseconds).
    fn write_julian_date(data: &mut Vec<u8>, jdate: f64) -> Result<()> {
        let days = jdate as i32;
        let frac = jdate - days as f64;
        let ms = (frac * 86_400_000.0) as i32;
        data.write_i32::<LittleEndian>(days)?;
        data.write_i32::<LittleEndian>(ms)?;
        Ok(())
    }
}
