//! Constants, sentinel bytes, and magic numbers for the DWG file format.
//!
//! Mirrors ACadSharp's `DwgSectionDefinition`, `DwgFileHeaderAC15`, and
//! related constant definitions.

/// Section name constants (matching ACadSharp `DwgSectionDefinition`)
pub mod section_names {
    /// All entities, table entries, and objects
    pub const ACDB_OBJECTS: &str = "AcDb:AcDbObjects";
    /// Application information (AC18+)
    pub const APP_INFO: &str = "AcDb:AppInfo";
    /// Auxiliary header data (dates, version stamps)
    pub const AUX_HEADER: &str = "AcDb:AuxHeader";
    /// System variables (header variables)
    pub const HEADER: &str = "AcDb:Header";
    /// DXF class definitions
    pub const CLASSES: &str = "AcDb:Classes";
    /// Object map (handle → file offset)
    pub const HANDLES: &str = "AcDb:Handles";
    /// Free space information
    pub const OBJ_FREE_SPACE: &str = "AcDb:ObjFreeSpace";
    /// Template metadata
    pub const TEMPLATE: &str = "AcDb:Template";
    /// Document summary information (AC18+)
    pub const SUMMARY_INFO: &str = "AcDb:SummaryInfo";
    /// File dependency list (AC18+)
    pub const FILE_DEP_LIST: &str = "AcDb:FileDepList";
    /// Thumbnail preview image
    pub const PREVIEW: &str = "AcDb:Preview";
    /// Revision history (AC18+)
    pub const REV_HISTORY: &str = "AcDb:RevHistory";

    /// Get the AC15 section locator index for a section name.
    /// Returns `None` for sections not present in the AC15 locator table.
    pub fn get_section_locator_by_name(name: &str) -> Option<usize> {
        match name {
            HEADER => Some(0),
            CLASSES => Some(1),
            HANDLES => Some(2),
            OBJ_FREE_SPACE => Some(3),
            TEMPLATE => Some(4),
            AUX_HEADER => Some(5),
            _ => None,
        }
    }
}

/// Sentinel bytes for section boundaries (16-byte markers).
pub mod sentinels {
    /// AcDb:Header section start sentinel
    pub const HEADER_START: [u8; 16] = [
        0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D,
        0x33, 0x5F,
    ];
    /// AcDb:Header section end sentinel
    pub const HEADER_END: [u8; 16] = [
        0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92,
        0xCC, 0xA0,
    ];
    /// AcDb:Classes section start sentinel
    pub const CLASSES_START: [u8; 16] = [
        0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF,
        0xB6, 0x8A,
    ];
    /// AcDb:Classes section end sentinel
    pub const CLASSES_END: [u8; 16] = [
        0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30,
        0x49, 0x75,
    ];
    /// AcDb:Preview section start sentinel
    pub const PREVIEW_START: [u8; 16] = [
        0x1F, 0x25, 0x6D, 0x07, 0xD4, 0x36, 0x28, 0x28, 0x9D, 0x57, 0xCA, 0x3F, 0x9D, 0x44,
        0x10, 0x2B,
    ];
    /// AcDb:Preview section end sentinel
    pub const PREVIEW_END: [u8; 16] = [
        0xE0, 0xDA, 0x92, 0xF8, 0x2B, 0xC9, 0xD7, 0xD7, 0x62, 0xA8, 0x35, 0xC0, 0x62, 0xBB,
        0xEF, 0xD4,
    ];
    /// File header end sentinel (AC15 only)
    pub const FILE_HEADER_END_AC15: [u8; 16] = [
        0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A,
        0x4D, 0x00,
    ];

    /// Get the start sentinel bytes for a given section name, if known.
    pub fn start_sentinel(section_name: &str) -> Option<&'static [u8; 16]> {
        match section_name {
            super::section_names::HEADER => Some(&HEADER_START),
            super::section_names::CLASSES => Some(&CLASSES_START),
            super::section_names::PREVIEW => Some(&PREVIEW_START),
            _ => None,
        }
    }

    /// Get the end sentinel bytes for a given section name, if known.
    pub fn end_sentinel(section_name: &str) -> Option<&'static [u8; 16]> {
        match section_name {
            super::section_names::HEADER => Some(&HEADER_END),
            super::section_names::CLASSES => Some(&CLASSES_END),
            super::section_names::PREVIEW => Some(&PREVIEW_END),
            _ => None,
        }
    }
}

/// Section hash values for AC21 (R2007) paged section identification.
///
/// These are hash values used to identify sections in the section map
/// for AC1021 (R2007) format files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DwgSectionHash {
    /// Unknown section
    AcDbUnknown = 0x00000000,
    /// Security section
    AcDbSecurity = 0x4A0204EA,
    /// File dependency list section
    AcDbFileDepList = 0x6C4205CA,
    /// VBA project section
    AcDbVbaProject = 0x586E0544,
    /// Application info section
    AcDbAppInfo = 0x3FA0043E,
    /// Preview (thumbnail) section
    AcDbPreview = 0x40AA0473,
    /// Summary info section
    AcDbSummaryInfo = 0x717A060F,
    /// Revision history section
    AcDbRevHistory = 0x60A205B3,
    /// Main objects section
    AcDbAcDbObjects = 0x674C05A9,
    /// Free space section
    AcDbObjFreeSpace = 0x77E2061F,
    /// Template section
    AcDbTemplate = 0x4A1404CE,
    /// Handle/object map section
    AcDbHandles = 0x3F6E0450,
    /// Classes section
    AcDbClasses = 0x3F54045F,
    /// Auxiliary header section
    AcDbAuxHeader = 0x54F0050A,
    /// Header (system variables) section
    AcDbHeader = 0x32B803D9,
    /// Digital signature section
    AcDbSignature = 0xFFFFFFFF,
}

/// AC18+ file format constants
pub mod ac18 {
    /// Size of the encrypted header metadata block at offset 0x80
    pub const ENCRYPTED_HEADER_SIZE: usize = 0x6C; // 108 bytes
    /// File identification string for AC18+ format
    pub const FILE_ID: &[u8] = b"AcFssFcAJMB\0";
    /// XOR mask for data page header decryption
    pub const DECRYPTION_MASK: u32 = 0x4164536B;
    /// Maximum page payload size (29696 bytes)
    pub const MAX_PAGE_SIZE: usize = 0x7400;
    /// Data page type marker
    pub const PAGE_TYPE_DATA: u32 = 0x4163043B;
    /// Page map page type marker
    pub const PAGE_TYPE_PAGE_MAP: u32 = 0x41630E3B;
    /// Section map page type marker
    pub const PAGE_TYPE_SECTION_MAP: u32 = 0x4163003B;
}

/// AC21 (R2007) file format constants
pub mod ac21 {
    /// Base file offset where data pages begin
    pub const DATA_PAGE_BASE_OFFSET: u64 = 0x480;
    /// Size of the Reed-Solomon encoded block at offset 0x80
    pub const RS_ENCODED_BLOCK_SIZE: usize = 0x400; // 1024 bytes
    /// Size of the decompressed header metadata
    pub const DECOMPRESSED_HEADER_SIZE: usize = 0x110; // 272 bytes
    /// Reed-Solomon data block size
    pub const RS_BLOCK_SIZE: usize = 239;
}

/// Handle section constants
pub mod handle_section {
    /// Maximum chunk size for handle section entries
    pub const MAX_CHUNK_SIZE: usize = 2032;
}

/// Section locator indices for AC15 (R13–R2000) file header records
pub mod section_locator {
    /// AcDb:Header section locator index
    pub const HEADER: usize = 0;
    /// AcDb:Classes section locator index
    pub const CLASSES: usize = 1;
    /// AcDb:Handles (object map) section locator index
    pub const HANDLES: usize = 2;
    /// AcDb:ObjFreeSpace section locator index
    pub const OBJ_FREE_SPACE: usize = 3;
    /// AcDb:Template section locator index
    pub const TEMPLATE: usize = 4;
    /// AcDb:AuxHeader section locator index
    pub const AUX_HEADER: usize = 5;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_locator_by_name() {
        assert_eq!(
            section_names::get_section_locator_by_name(section_names::HEADER),
            Some(0)
        );
        assert_eq!(
            section_names::get_section_locator_by_name(section_names::CLASSES),
            Some(1)
        );
        assert_eq!(
            section_names::get_section_locator_by_name(section_names::HANDLES),
            Some(2)
        );
        assert_eq!(
            section_names::get_section_locator_by_name(section_names::OBJ_FREE_SPACE),
            Some(3)
        );
        assert_eq!(
            section_names::get_section_locator_by_name(section_names::TEMPLATE),
            Some(4)
        );
        assert_eq!(
            section_names::get_section_locator_by_name(section_names::AUX_HEADER),
            Some(5)
        );
        assert_eq!(
            section_names::get_section_locator_by_name("Unknown"),
            None
        );
    }

    #[test]
    fn test_sentinel_lookup() {
        assert_eq!(
            sentinels::start_sentinel(section_names::HEADER),
            Some(&sentinels::HEADER_START)
        );
        assert_eq!(
            sentinels::end_sentinel(section_names::CLASSES),
            Some(&sentinels::CLASSES_END)
        );
        assert_eq!(sentinels::start_sentinel("NonExistent"), None);
    }
}
