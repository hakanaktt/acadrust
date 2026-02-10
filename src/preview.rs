//! DWG preview/thumbnail image data.
//!
//! Stores the thumbnail information to generate the preview for a CAD document.

/// Type of media stored in the preview image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PreviewType {
    /// Unknown or unsupported preview format.
    Unknown = 0,
    /// BMP bitmap image.
    Bmp = 2,
    /// Windows Metafile (WMF) image.
    Wmf = 3,
    /// PNG image.
    Png = 6,
}

impl PreviewType {
    /// Create from a raw byte code.
    pub fn from_code(code: u8) -> Self {
        match code {
            2 => Self::Bmp,
            3 => Self::Wmf,
            6 => Self::Png,
            _ => Self::Unknown,
        }
    }
}

/// Preview/thumbnail image data from a DWG file.
///
/// Corresponds to the `AcDb:Preview` section.
#[derive(Debug, Clone)]
pub struct DwgPreview {
    /// Code that specifies the type of media stored in the preview.
    pub code: PreviewType,
    /// Raw header bytes for the preview section.
    ///
    /// Usually formed by an empty array of 80 zeros.
    pub raw_header: Vec<u8>,
    /// Raw image bytes conforming the thumbnail.
    pub raw_image: Vec<u8>,
}

impl Default for DwgPreview {
    fn default() -> Self {
        Self {
            code: PreviewType::Unknown,
            raw_header: Vec::new(),
            raw_image: Vec::new(),
        }
    }
}

impl DwgPreview {
    /// Create a new preview with the given data.
    pub fn new(code: PreviewType, raw_header: Vec<u8>, raw_image: Vec<u8>) -> Self {
        Self {
            code,
            raw_header,
            raw_image,
        }
    }

    /// Returns `true` if the preview is empty (no image data).
    pub fn is_empty(&self) -> bool {
        self.raw_image.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_default() {
        let preview = DwgPreview::default();
        assert_eq!(preview.code, PreviewType::Unknown);
        assert!(preview.is_empty());
    }

    #[test]
    fn test_preview_type_from_code() {
        assert_eq!(PreviewType::from_code(0), PreviewType::Unknown);
        assert_eq!(PreviewType::from_code(2), PreviewType::Bmp);
        assert_eq!(PreviewType::from_code(3), PreviewType::Wmf);
        assert_eq!(PreviewType::from_code(6), PreviewType::Png);
        assert_eq!(PreviewType::from_code(99), PreviewType::Unknown);
    }

    #[test]
    fn test_preview_new() {
        let header = vec![0u8; 80];
        let image = vec![0xFF; 100];
        let preview = DwgPreview::new(PreviewType::Bmp, header.clone(), image.clone());
        assert_eq!(preview.code, PreviewType::Bmp);
        assert_eq!(preview.raw_header.len(), 80);
        assert_eq!(preview.raw_image.len(), 100);
        assert!(!preview.is_empty());
    }
}
