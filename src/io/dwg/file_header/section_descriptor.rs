//! DWG section descriptor for AC18+ (R2004+) file headers.
//!
//! Each section descriptor describes a named section (e.g., "AcDb:Header"),
//! including its page type, compression info, and local section maps.
//!
//! Mirrors ACadSharp `DwgSectionDescriptor`.

use super::local_section_map::DwgLocalSectionMap;
use crate::io::dwg::constants::DwgSectionHash;

/// Describes a section within an AC18+ DWG file.
#[derive(Debug, Clone)]
pub struct DwgSectionDescriptor {
    /// Page/section type (maps to DwgSectionHash values).
    pub page_type: i64,
    /// Section name (e.g., "AcDb:Header").
    pub name: String,
    /// Section number.
    pub section_number: i32,
    /// Section ID (starts at 0, first section is 0).
    pub section_id: i32,
    /// Decompressed data size.
    pub decompressed_size: u64,
    /// Compressed data size.
    pub compressed_size: u64,
    /// Compression type (1 = none, 2 = LZ77).
    pub compressed_code: i32,
    /// Whether the section is encrypted.
    pub is_encrypted: bool,
    /// Encrypted flag as integer (0 = no, 1 = yes, 2 = unknown).
    pub encrypted: i32,
    /// Hash value (section hash used in page map).
    pub hash_code: DwgSectionHash,
    /// Encoding for this section's data.
    pub encoding: u32,
    /// Number of pages written to file.
    pub page_count: i32,
    /// Associated local section maps (page data).
    pub local_sections: Vec<DwgLocalSectionMap>,
}

impl DwgSectionDescriptor {
    /// Create a new section descriptor with a given name.
    pub fn new(name: &str) -> Self {
        Self {
            page_type: 0,
            name: name.to_string(),
            section_number: 0,
            section_id: 0,
            decompressed_size: 0,
            compressed_size: 0,
            compressed_code: 2, // default to compression enabled
            is_encrypted: false,
            encrypted: 0,
            hash_code: DwgSectionHash::AcDbUnknown,
            encoding: 0,
            page_count: 0,
            local_sections: Vec::new(),
        }
    }

    /// The maximum decompressed size of any single page in this section.
    pub fn page_header_data_size(&self) -> u64 {
        self.decompressed_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let desc = DwgSectionDescriptor::new("AcDb:Header");
        assert_eq!(desc.name, "AcDb:Header");
        assert_eq!(desc.compressed_code, 2);
        assert!(!desc.is_encrypted);
        assert!(desc.local_sections.is_empty());
    }

    #[test]
    fn test_page_header_data_size() {
        let mut desc = DwgSectionDescriptor::new("test");
        desc.decompressed_size = 4096;
        assert_eq!(desc.page_header_data_size(), 4096);
    }
}
