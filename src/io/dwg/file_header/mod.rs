//! DWG file header structures.
//!
//! The DWG file header contains version information, section locators (AC15)
//! or section descriptors (AC18+), and other metadata.
//!
//! Mirrors ACadSharp's `FileHeaders/` directory.

mod compressed_metadata;
mod local_section_map;
mod section_descriptor;
mod section_locator;

pub use compressed_metadata::Dwg21CompressedMetadata;
pub use local_section_map::DwgLocalSectionMap;
pub use section_descriptor::DwgSectionDescriptor;
pub use section_locator::DwgSectionLocatorRecord;

use crate::error::{DxfError, Result};
use crate::types::DxfVersion;

use std::collections::HashMap;

use super::constants::sentinels;

/// DWG file header — version-dependent structure.
///
/// Mirrors ACadSharp `DwgFileHeader` and its subclasses.
#[derive(Debug, Clone)]
pub enum DwgFileHeader {
    /// R13–R2000 (AC1012/AC1014/AC1015) — simple record-based layout
    AC15(DwgFileHeaderAC15),
    /// R2004, R2010, R2013, R2018 (AC1018/AC1024/AC1027/AC1032) — page-based layout
    AC18(DwgFileHeaderAC18),
    /// R2007 (AC1021) — Reed-Solomon + page-based layout
    AC21(DwgFileHeaderAC21),
}

impl DwgFileHeader {
    /// Create the appropriate file header variant for a DXF version.
    pub fn create(version: DxfVersion) -> Result<Self> {
        match version {
            DxfVersion::AC1012 | DxfVersion::AC1014 | DxfVersion::AC1015 => {
                Ok(DwgFileHeader::AC15(DwgFileHeaderAC15::new(version)))
            }
            DxfVersion::AC1018 => {
                Ok(DwgFileHeader::AC18(DwgFileHeaderAC18::new(version)))
            }
            DxfVersion::AC1021 => {
                Ok(DwgFileHeader::AC21(DwgFileHeaderAC21::new(version)))
            }
            DxfVersion::AC1024 | DxfVersion::AC1027 | DxfVersion::AC1032 => {
                Ok(DwgFileHeader::AC18(DwgFileHeaderAC18::new(version)))
            }
            _ => Err(DxfError::UnsupportedVersion(version.to_string())),
        }
    }

    /// Get the version stored in this file header.
    pub fn version(&self) -> DxfVersion {
        match self {
            DwgFileHeader::AC15(h) => h.version,
            DwgFileHeader::AC18(h) => h.version,
            DwgFileHeader::AC21(h) => h.base.version,
        }
    }

    /// Get the preview address.
    pub fn preview_address(&self) -> i64 {
        match self {
            DwgFileHeader::AC15(h) => h.preview_address,
            DwgFileHeader::AC18(h) => h.preview_address,
            DwgFileHeader::AC21(h) => h.base.preview_address,
        }
    }

    /// Set the preview address.
    pub fn set_preview_address(&mut self, addr: i64) {
        match self {
            DwgFileHeader::AC15(h) => h.preview_address = addr,
            DwgFileHeader::AC18(h) => h.preview_address = addr,
            DwgFileHeader::AC21(h) => h.base.preview_address = addr,
        }
    }

    /// Get the maintenance version.
    pub fn maintenance_version(&self) -> u8 {
        match self {
            DwgFileHeader::AC15(h) => h.maintenance_version,
            DwgFileHeader::AC18(h) => h.maintenance_version,
            DwgFileHeader::AC21(h) => h.base.maintenance_version,
        }
    }

    /// Get a section descriptor by name (AC18+ only).
    pub fn get_descriptor(&self, name: &str) -> Option<&DwgSectionDescriptor> {
        match self {
            DwgFileHeader::AC15(_) => None,
            DwgFileHeader::AC18(h) => h.descriptors.get(name),
            DwgFileHeader::AC21(h) => h.base.descriptors.get(name),
        }
    }

    /// Get a section descriptor by name (AC18+, mutable).
    pub fn get_descriptor_mut(&mut self, name: &str) -> Option<&mut DwgSectionDescriptor> {
        match self {
            DwgFileHeader::AC15(_) => None,
            DwgFileHeader::AC18(h) => h.descriptors.get_mut(name),
            DwgFileHeader::AC21(h) => h.base.descriptors.get_mut(name),
        }
    }
}

// ---------------------------------------------------------------------------
// AC15 (R13–R2000) File Header
// ---------------------------------------------------------------------------

/// DWG file header for R13–R2000 (AC1012/AC1014/AC1015).
///
/// Contains section locator records that map section numbers to file offsets.
///
/// Mirrors ACadSharp `DwgFileHeaderAC15`.
#[derive(Debug, Clone)]
pub struct DwgFileHeaderAC15 {
    pub version: DxfVersion,
    pub preview_address: i64,
    pub maintenance_version: u8,
    pub drawing_code_page: u16,
    /// Section locator records: index → (offset, size)
    pub records: HashMap<usize, DwgSectionLocatorRecord>,
}

impl DwgFileHeaderAC15 {
    pub fn new(version: DxfVersion) -> Self {
        Self {
            version,
            preview_address: -1,
            maintenance_version: 0,
            drawing_code_page: 0,
            records: HashMap::new(),
        }
    }

    /// File header end sentinel for AC15
    pub fn end_sentinel() -> &'static [u8; 16] {
        &sentinels::FILE_HEADER_END_AC15
    }
}

// ---------------------------------------------------------------------------
// AC18 (R2004+) File Header
// ---------------------------------------------------------------------------

/// DWG file header for R2004+ (AC1018/AC1024/AC1027/AC1032).
///
/// Contains page-based section descriptors and encrypted metadata.
///
/// Mirrors ACadSharp `DwgFileHeaderAC18`.
#[derive(Debug, Clone)]
pub struct DwgFileHeaderAC18 {
    pub version: DxfVersion,
    pub preview_address: i64,
    pub maintenance_version: u8,
    pub drawing_code_page: u16,

    /// DWG version byte
    pub dwg_version: u8,
    /// Application release version byte
    pub app_release_version: u8,

    /// Summary info address
    pub summary_info_addr: i64,
    /// Security type
    pub security_type: i64,
    /// VBA project address
    pub vba_project_addr: i64,

    /// Root tree node gap
    pub root_tree_node_gap: i32,
    /// Gap array size
    pub gap_array_size: u32,
    /// CRC seed
    pub crc_seed: u32,
    /// Last page ID
    pub last_page_id: i32,
    /// Last section address
    pub last_section_addr: u64,
    /// Second header address
    pub second_header_addr: u64,
    /// Gap amount
    pub gap_amount: u32,
    /// Section amount
    pub section_amount: u32,
    /// Section page map ID
    pub section_page_map_id: u32,
    /// Page map address
    pub page_map_address: u64,
    /// Section map ID
    pub section_map_id: u32,
    /// Section array page size
    pub section_array_page_size: u32,
    /// Right gap
    pub right_gap: i32,
    /// Left gap
    pub left_gap: i32,

    /// Section descriptors by name
    pub descriptors: HashMap<String, DwgSectionDescriptor>,

    /// Section locator records (inherited from AC15 base)
    pub records: HashMap<usize, DwgSectionLocatorRecord>,
}

impl DwgFileHeaderAC18 {
    pub fn new(version: DxfVersion) -> Self {
        Self {
            version,
            preview_address: -1,
            maintenance_version: 0,
            drawing_code_page: 0,
            dwg_version: 0,
            app_release_version: 0,
            summary_info_addr: 0,
            security_type: 0,
            vba_project_addr: 0,
            root_tree_node_gap: 0,
            gap_array_size: 0,
            crc_seed: 0,
            last_page_id: 0,
            last_section_addr: 0,
            second_header_addr: 0,
            gap_amount: 0,
            section_amount: 0,
            section_page_map_id: 0,
            page_map_address: 0,
            section_map_id: 0,
            section_array_page_size: 0,
            right_gap: 0,
            left_gap: 0,
            descriptors: HashMap::new(),
            records: HashMap::new(),
        }
    }

    /// Add a named section descriptor.
    pub fn add_section(&mut self, name: &str) {
        self.descriptors
            .insert(name.to_string(), DwgSectionDescriptor::new(name));
    }

    /// Add a pre-built section descriptor.
    pub fn add_descriptor(&mut self, descriptor: DwgSectionDescriptor) {
        self.descriptors
            .insert(descriptor.name.clone(), descriptor);
    }
}

// ---------------------------------------------------------------------------
// AC21 (R2007) File Header
// ---------------------------------------------------------------------------

/// DWG file header for R2007 (AC1021).
///
/// Extends AC18 with Reed-Solomon encoded compressed metadata.
///
/// Mirrors ACadSharp `DwgFileHeaderAC21`.
#[derive(Debug, Clone)]
pub struct DwgFileHeaderAC21 {
    pub base: DwgFileHeaderAC18,
    pub compressed_metadata: Option<Dwg21CompressedMetadata>,
}

impl DwgFileHeaderAC21 {
    pub fn new(version: DxfVersion) -> Self {
        Self {
            base: DwgFileHeaderAC18::new(version),
            compressed_metadata: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ac15() {
        let header = DwgFileHeader::create(DxfVersion::AC1015).unwrap();
        assert!(matches!(header, DwgFileHeader::AC15(_)));
        assert_eq!(header.version(), DxfVersion::AC1015);
    }

    #[test]
    fn test_create_ac18() {
        let header = DwgFileHeader::create(DxfVersion::AC1018).unwrap();
        assert!(matches!(header, DwgFileHeader::AC18(_)));
    }

    #[test]
    fn test_create_ac21() {
        let header = DwgFileHeader::create(DxfVersion::AC1021).unwrap();
        assert!(matches!(header, DwgFileHeader::AC21(_)));
    }

    #[test]
    fn test_create_ac1024_uses_ac18() {
        let header = DwgFileHeader::create(DxfVersion::AC1024).unwrap();
        assert!(matches!(header, DwgFileHeader::AC18(_)));
    }

    #[test]
    fn test_create_unsupported() {
        let result = DwgFileHeader::create(DxfVersion::Unknown);
        assert!(result.is_err());
    }

    #[test]
    fn test_preview_address_default() {
        let header = DwgFileHeader::create(DxfVersion::AC1015).unwrap();
        assert_eq!(header.preview_address(), -1);
    }

    #[test]
    fn test_ac18_add_section() {
        let mut header = DwgFileHeaderAC18::new(DxfVersion::AC1018);
        header.add_section("AcDb:Header");
        assert!(header.descriptors.contains_key("AcDb:Header"));
    }
}
