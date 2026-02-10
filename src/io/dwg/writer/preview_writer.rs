//! DWG Preview (thumbnail image) section writer.
//!
//! Writes the preview/thumbnail section framed by start/end sentinels.
//!
//! Mirrors ACadSharp's `DwgPreviewWriter`.

use crate::error::Result;
use crate::io::dwg::constants::sentinels;
use crate::types::DxfVersion;

use byteorder::{LittleEndian, WriteBytesExt};

/// Writer for the DWG preview/thumbnail section.
#[allow(dead_code)]
pub struct DwgPreviewWriter {
    version: DxfVersion,
}

impl DwgPreviewWriter {
    /// Create a new preview writer.
    pub fn new(version: DxfVersion) -> Self {
        Self { version }
    }

    /// Write an empty preview section (no thumbnail).
    pub fn write_empty(&self) -> Result<Vec<u8>> {
        let mut output = Vec::new();

        // Start sentinel
        output.extend_from_slice(&sentinels::PREVIEW_START);

        // RL: overall size of image area
        output.write_i32::<LittleEndian>(1)?;
        // RC: images present counter (0 = none)
        output.push(0);

        // End sentinel
        output.extend_from_slice(&sentinels::PREVIEW_END);

        Ok(output)
    }

    /// Write a preview section with raw BMP header and image data.
    ///
    /// `raw_header` — BMP header bytes.
    /// `raw_image` — BMP image data bytes.
    /// `image_code` — image type code (typically 2 for BMP, 3 for WMF).
    /// `start_pos` — the absolute file position where this section will be written.
    pub fn write_with_image(
        &self,
        raw_header: &[u8],
        raw_image: &[u8],
        image_code: u8,
        start_pos: i64,
    ) -> Result<Vec<u8>> {
        let size = raw_header.len() + raw_image.len() + 19;
        let mut output = Vec::new();

        // Start sentinel
        output.extend_from_slice(&sentinels::PREVIEW_START);

        // RL: overall size of image area
        output.write_i32::<LittleEndian>(size as i32)?;

        // RC: images present counter (2 = header + image)
        output.push(2);

        // -- Header entry --
        // RC: code indicating what follows (1 = header)
        output.push(1);

        // RL: header data start
        let header_offset = start_pos + output.len() as i64 + 12 + 5 + 32;
        output.write_i32::<LittleEndian>(header_offset as i32)?;

        // RL: header data size
        output.write_i32::<LittleEndian>(raw_header.len() as i32)?;

        // -- Image entry --
        // RC: code indicating what follows (image code)
        output.push(image_code);

        // RL: image data start
        let image_offset = header_offset + raw_header.len() as i64;
        output.write_i32::<LittleEndian>(image_offset as i32)?;

        // RL: image data size
        output.write_i32::<LittleEndian>(raw_image.len() as i32)?;

        // Actual data
        output.extend_from_slice(raw_header);
        output.extend_from_slice(raw_image);

        // End sentinel
        output.extend_from_slice(&sentinels::PREVIEW_END);

        Ok(output)
    }
}
