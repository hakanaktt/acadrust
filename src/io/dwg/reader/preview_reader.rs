//! DWG Preview (thumbnail) section reader.
//!
//! Reads the embedded preview image from the `AcDb:Preview` section.
//! The section is bounded by start/end sentinels and can contain
//! BMP, WMF, or PNG image data.
//!
//! Mirrors ACadSharp's `DwgPreviewReader`.

use crate::error::Result;
use crate::io::dwg::constants::sentinels;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::io::dwg::reader::stream_reader_base::get_stream_handler;
use crate::io::dwg::section_io::SectionIO;
use crate::preview::{DwgPreview, PreviewType};
use crate::types::DxfVersion;

/// Reader for the DWG `AcDb:Preview` (thumbnail image) section.
///
/// Reads the embedded preview image data from the section, including
/// a raw header and the image body bytes.
pub struct DwgPreviewReader {
    /// Raw section data bytes (decompressed/decrypted by caller).
    data: Vec<u8>,
    /// DWG version being read.
    version: DxfVersion,
}

impl DwgPreviewReader {
    /// Create a new preview reader.
    ///
    /// # Arguments
    /// * `version` - The DWG version being read.
    /// * `data` - Raw section bytes (already decompressed/decrypted).
    pub fn new(version: DxfVersion, data: Vec<u8>) -> Self {
        Self { data, version }
    }

    /// Read the preview image from the section.
    ///
    /// Returns a `DwgPreview` containing the image type, raw header,
    /// and body bytes. Returns an empty preview if no image data is present.
    pub fn read(&self) -> Result<DwgPreview> {
        let mut reader = get_stream_handler(self.version, self.data.clone());

        // Start sentinel: {0x1F,0x25,0x6D,0x07,0xD4,0x36,0x28,0x28,0x9D,0x57,0xCA,0x3F,0x9D,0x44,0x10,0x2B}
        let sentinel = reader.read_sentinel()?;
        if let Some(expected) = sentinels::start_sentinel("AcDb:Preview") {
            SectionIO::check_sentinel(&sentinel, expected);
        }

        // overall size RL: overall size of image area
        let _overall_size = reader.read_raw_long()?;

        // images present RC: counter indicating what is present here
        let images_present = reader.read_raw_char()?;

        let mut _header_data_start: Option<i32> = None;
        let mut header_data_size: Option<i32> = None;
        let mut _start_of_image: Option<i32> = None;
        let mut size_image: Option<i32> = None;
        let mut preview_code = PreviewType::Unknown;

        for _ in 0..images_present {
            // Code RC: code indicating what follows
            let code = reader.read_raw_char()?;
            match code {
                1 => {
                    // Header data start RL: start of header data
                    _header_data_start = Some(reader.read_raw_long()?);
                    // Header data size RL: size of header data
                    header_data_size = Some(reader.read_raw_long()?);
                }
                _ => {
                    preview_code = PreviewType::from_code(code);
                    _start_of_image = Some(reader.read_raw_long()?);
                    size_image = Some(reader.read_raw_long()?);
                }
            }
        }

        // Read header bytes
        let header = if let Some(size) = header_data_size {
            reader.read_bytes(size as usize)?
        } else {
            Vec::new()
        };

        // Read body (image) bytes
        let body = if let Some(size) = size_image {
            reader.read_bytes(size as usize)?
        } else {
            Vec::new()
        };

        // End sentinel: 0xE0,0xDA,0x92,0xF8,0x2B,0xC9,0xD7,0xD7,0x62,0xA8,0x35,0xC0,0x62,0xBB,0xEF,0xD4
        let sentinel = reader.read_sentinel()?;
        if let Some(expected) = sentinels::end_sentinel("AcDb:Preview") {
            SectionIO::check_sentinel(&sentinel, expected);
        }

        Ok(DwgPreview::new(preview_code, header, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_reader_creation() {
        let reader = DwgPreviewReader::new(DxfVersion::AC1015, vec![]);
        assert_eq!(reader.version, DxfVersion::AC1015);
    }
}
