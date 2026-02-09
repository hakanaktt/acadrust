//! DWG local section map for AC18+ (R2004+) file headers.
//!
//! Each local section map describes one "page" of a section
//! (its offset, compressed size, etc.).
//!
//! Mirrors ACadSharp `DwgLocalSectionMap`.

/// A single page within a section.
#[derive(Debug, Clone, Default)]
pub struct DwgLocalSectionMap {
    /// Whether this is the start of a new section sequence.
    pub is_empty: bool,
    /// Page number within the section.
    pub page_number: i32,
    /// Compressed data size for this page.
    pub compressed_size: u64,
    /// Decompressed data size for this page.
    pub decompressed_size: u64,
    /// Offset into the section's decompressed stream.
    pub offset: u64,
    /// Page count.
    pub page_count: u32,
    /// Compression type (1 = none, 2 = LZ77).
    pub compression_type: i32,
    /// Section number.
    pub section_number: i32,
    /// Decompressed page size (max for this section).
    pub decompressed_page_size: u64,
    /// Checksum value.
    pub checksum: u64,
    /// CRC value.
    pub crc: u64,
    /// Absolute byte offset in the DWG file to this page.
    pub seeker: u64,
    /// Page size (may differ from compressed_size with padding).
    pub page_size: u64,
    /// Size of the section's ODA size field.
    pub oda_size: u64,
}

impl DwgLocalSectionMap {
    /// Create a new empty local section map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a "gap" entry (used in some AC18+ section maps).
    pub fn gap() -> Self {
        Self {
            is_empty: true,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let map = DwgLocalSectionMap::new();
        assert!(!map.is_empty);
        assert_eq!(map.page_number, 0);
        assert_eq!(map.seeker, 0);
    }

    #[test]
    fn test_gap() {
        let map = DwgLocalSectionMap::gap();
        assert!(map.is_empty);
    }
}
