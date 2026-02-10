//! DWG Application Info section reader.
//!
//! Reads application metadata from the `AcDb:AppInfo` section.
//! This section contains app name, version, and product information
//! used by the creating application.
//!
//! Mirrors ACadSharp's `DwgAppInfoReader`.

use crate::error::Result;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::io::dwg::reader::stream_reader_base::get_stream_handler;
use crate::io::dwg::section_io::SectionIO;
use crate::types::DxfVersion;

/// Application info data read from the `AcDb:AppInfo` section.
#[derive(Debug, Clone, Default)]
pub struct AppInfo {
    /// Application info name (e.g. "AppInfoDataList")
    pub info_name: String,
    /// Version string (e.g. "2.7.2.0")
    pub version: String,
    /// Comment string
    pub comment: String,
    /// Product XML element string
    pub product: String,
    /// Version data checksum (16 bytes)
    pub version_checksum: Vec<u8>,
    /// Comment data checksum (16 bytes)
    pub comment_checksum: Vec<u8>,
    /// Product data checksum (16 bytes, R2010+ only)
    pub product_checksum: Vec<u8>,
}

/// Reader for the DWG `AcDb:AppInfo` section.
///
/// Reads application identification and version metadata.
pub struct DwgAppInfoReader {
    /// Raw section data bytes (decompressed/decrypted by caller).
    data: Vec<u8>,
    /// DWG version being read.
    version: DxfVersion,
    /// Pre-computed version flags.
    sio: SectionIO,
}

impl DwgAppInfoReader {
    /// Create a new app info reader.
    ///
    /// # Arguments
    /// * `version` - The DWG version being read.
    /// * `data` - Raw section bytes (already decompressed/decrypted).
    pub fn new(version: DxfVersion, data: Vec<u8>) -> Self {
        Self {
            sio: SectionIO::new(version),
            data,
            version,
        }
    }

    /// Read application info from the section.
    ///
    /// Returns an `AppInfo` struct with the parsed metadata.
    pub fn read(&self) -> Result<AppInfo> {
        let mut info = AppInfo::default();
        let mut reader = get_stream_handler(self.version, self.data.clone());

        if !self.sio.r2007_plus {
            self.read_r18(&mut reader, &mut info)?;
        }

        // UInt32 4: Unknown (ODA writes 2)
        let _unknown1 = reader.read_raw_long()?;
        // String 2 + 2 * n + 2: App info name, ODA writes "AppInfoDataList"
        info.info_name = reader.read_text_unicode()?;
        // UInt32 4: Unknown (ODA writes 3)
        let _unknown2 = reader.read_raw_long()?;
        // Byte[] 16: Version data (checksum, ODA writes zeroes)
        info.version_checksum = reader.read_bytes(16)?;
        // String 2 + 2 * n + 2: Version
        info.version = reader.read_text_unicode()?;
        // Byte[] 16: Comment data (checksum, ODA writes zeroes)
        info.comment_checksum = reader.read_bytes(16)?;

        if !self.sio.r2010_plus {
            return Ok(info);
        }

        // String 2 + 2 * n + 2: Comment
        info.comment = reader.read_text_unicode()?;
        // Byte[] 16: Product data (checksum, ODA writes zeroes)
        info.product_checksum = reader.read_bytes(16)?;
        // String 2 + 2 * n + 2: Product
        info.product = reader.read_text_unicode()?;

        Ok(info)
    }

    /// Read the pre-R2007 (R18) format section.
    ///
    /// For R18, the values don't match the documentation exactly.
    fn read_r18(&self, reader: &mut dyn IDwgStreamReader, info: &mut AppInfo) -> Result<()> {
        // String 2 + n: App info name, ODA writes "AppInfoDataList"
        info.info_name = reader.read_variable_text()?;
        // UInt32 4: Unknown (ODA writes 2)
        let _unknown2 = reader.read_raw_long()?;
        // Unknown, ODA writes "4001"
        info.version = reader.read_variable_text()?;
        // String 2 + n: App info product XML element
        info.product = reader.read_variable_text()?;
        // String 2 + n: App info version, e.g. ODA writes "2.7.2.0"
        info.comment = reader.read_variable_text()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_info_reader_creation() {
        let reader = DwgAppInfoReader::new(DxfVersion::AC1015, vec![]);
        assert_eq!(reader.version, DxfVersion::AC1015);
    }

    #[test]
    fn test_app_info_default() {
        let info = AppInfo::default();
        assert!(info.info_name.is_empty());
        assert!(info.version.is_empty());
        assert!(info.comment.is_empty());
        assert!(info.product.is_empty());
    }
}
