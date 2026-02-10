//! DWG Summary Info section reader.
//!
//! Reads document metadata (title, author, keywords, etc.) from the
//! `AcDb:SummaryInfo` section in a DWG file.
//!
//! Mirrors ACadSharp's `DwgSummaryInfoReader`.

use crate::error::Result;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::io::dwg::reader::stream_reader_base::get_stream_handler;
use crate::summary_info::CadSummaryInfo;
use crate::types::DxfVersion;

/// Reader for the DWG `AcDb:SummaryInfo` section.
///
/// Reads document metadata strings (title, subject, author, etc.)
/// and custom properties from the section data.
pub struct DwgSummaryInfoReader {
    /// Raw section data bytes (decompressed/decrypted by caller).
    data: Vec<u8>,
    /// DWG version being read.
    version: DxfVersion,
}

impl DwgSummaryInfoReader {
    /// Create a new summary info reader.
    ///
    /// # Arguments
    /// * `version` - The DWG version being read.
    /// * `data` - Raw section bytes (already decompressed/decrypted).
    pub fn new(version: DxfVersion, data: Vec<u8>) -> Self {
        Self { data, version }
    }

    /// Read document summary information.
    ///
    /// Returns a `CadSummaryInfo` struct containing all parsed metadata.
    /// On partial read failure, returns whatever was successfully read.
    pub fn read(&self) -> Result<CadSummaryInfo> {
        let mut summary = CadSummaryInfo::default();
        let mut reader = get_stream_handler(self.version, self.data.clone());
        let r2007_plus = self.version >= DxfVersion::AC1021;

        // Helper closure for reading strings:
        // Pre-R2021: 16-bit LE length + Windows-1252 encoded bytes (via readUnicodeString)
        // R2021+: ReadTextUnicode from bit-level reader

        // This section contains summary information about the drawing.
        // Strings are encoded as a 16-bit length, followed by the character bytes (0-terminated).

        // String 2 + n: Title
        summary.title = Self::read_string(&mut reader, r2007_plus)?;
        // String 2 + n: Subject
        summary.subject = Self::read_string(&mut reader, r2007_plus)?;
        // String 2 + n: Author
        summary.author = Self::read_string(&mut reader, r2007_plus)?;
        // String 2 + n: Keywords
        summary.keywords = Self::read_string(&mut reader, r2007_plus)?;
        // String 2 + n: Comments
        summary.comments = Self::read_string(&mut reader, r2007_plus)?;
        // String 2 + n: LastSavedBy
        summary.last_saved_by = Self::read_string(&mut reader, r2007_plus)?;
        // String 2 + n: RevisionNumber
        summary.revision_number = Self::read_string(&mut reader, r2007_plus)?;
        // String 2 + n: HyperlinkBase
        summary.hyperlink_base = Self::read_string(&mut reader, r2007_plus)?;

        // ? 8: Total editing time (ODA writes two zero Int32's)
        reader.read_raw_long()?;
        reader.read_raw_long()?;

        // Julian date 8: Create date time
        summary.created_date = reader.read_8bit_julian_date()?;

        // Julian date 8: Modified date time
        summary.modified_date = reader.read_8bit_julian_date()?;

        // Int16 2 + 2 * (2 + n): Property count, followed by PropertyCount key/value string pairs.
        let nproperties = reader.read_raw_short()?;
        for _ in 0..nproperties {
            let prop_name = Self::read_string(&mut reader, r2007_plus)?;
            let prop_value = Self::read_string(&mut reader, r2007_plus)?;

            // Add the property (ignore duplicates)
            if !prop_name.is_empty() {
                summary.properties.insert(prop_name, prop_value);
            }
        }

        // Int32 4: Unknown (write 0)
        let _ = reader.read_raw_long();
        // Int32 4: Unknown (write 0)
        let _ = reader.read_raw_long();

        Ok(summary)
    }

    /// Read a string using the version-appropriate method.
    ///
    /// Pre-R2007: LE i16 length + Windows-1252 encoded bytes
    /// R2007+: ReadTextUnicode (BS length + UTF-16LE bytes)
    fn read_string(reader: &mut dyn IDwgStreamReader, r2007_plus: bool) -> Result<String> {
        if r2007_plus {
            reader.read_text_unicode()
        } else {
            Self::read_unicode_string(reader)
        }
    }

    /// Read a pre-R2007 Unicode string.
    ///
    /// Format: LE Int16 length, then `length` bytes encoded as Windows-1252.
    /// Null characters are stripped from the result.
    fn read_unicode_string(reader: &mut dyn IDwgStreamReader) -> Result<String> {
        let text_length = reader.read_raw_short()?;
        if text_length == 0 {
            return Ok(String::new());
        }

        let bytes = reader.read_bytes(text_length as usize)?;
        let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
        Ok(decoded.replace('\0', ""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_info_reader_creation() {
        let reader = DwgSummaryInfoReader::new(DxfVersion::AC1015, vec![]);
        assert_eq!(reader.version, DxfVersion::AC1015);
    }
}
