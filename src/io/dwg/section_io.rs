//! Base section I/O helpers with version-conditional logic.
//!
//! Mirrors ACadSharp's `DwgSectionIO` abstract class which provides
//! version flags and sentinel checking for all section readers/writers.

use crate::notification::{Notification, NotificationType};
use crate::types::DxfVersion;

use super::constants::sentinels;

/// Base helpers for section read/write with version-conditional logic.
///
/// Each section reader/writer creates a `SectionIO` to get pre-computed
/// version flags, making version-conditional reads concise.
pub struct SectionIO {
    version: DxfVersion,

    /// R13-R14 only
    pub r13_14_only: bool,
    /// R13-R15 only
    pub r13_15_only: bool,
    /// R2000+ (AC1015+)
    pub r2000_plus: bool,
    /// Pre-R2004
    pub r2004_pre: bool,
    /// Pre-R2007
    pub r2007_pre: bool,
    /// R2004+ (AC1018+)
    pub r2004_plus: bool,
    /// R2007+ (AC1021+)
    pub r2007_plus: bool,
    /// R2010+ (AC1024+)
    pub r2010_plus: bool,
    /// R2013+ (AC1027+)
    pub r2013_plus: bool,
    /// R2018+ (AC1032+)
    pub r2018_plus: bool,
}

impl SectionIO {
    /// Create a new `SectionIO` with pre-computed version flags.
    ///
    /// Mirrors ACadSharp `DwgSectionIO` constructor.
    pub fn new(version: DxfVersion) -> Self {
        Self {
            r13_14_only: version == DxfVersion::AC1014 || version == DxfVersion::AC1012,
            r13_15_only: version >= DxfVersion::AC1012 && version <= DxfVersion::AC1015,
            r2000_plus: version >= DxfVersion::AC1015,
            r2004_pre: version < DxfVersion::AC1018,
            r2007_pre: version <= DxfVersion::AC1021,
            r2004_plus: version >= DxfVersion::AC1018,
            r2007_plus: version >= DxfVersion::AC1021,
            r2010_plus: version >= DxfVersion::AC1024,
            r2013_plus: version >= DxfVersion::AC1027,
            r2018_plus: version >= DxfVersion::AC1032,
            version,
        }
    }

    /// Get the DXF version.
    pub fn version(&self) -> DxfVersion {
        self.version
    }

    /// Check if two 16-byte sentinel arrays match.
    ///
    /// Returns `true` if all 16 bytes are identical.
    pub fn check_sentinel(actual: &[u8; 16], expected: &[u8; 16]) -> bool {
        actual == expected
    }

    /// Validate a sentinel, returning a `Notification` on mismatch.
    ///
    /// This is a convenience method that creates a warning notification
    /// if the sentinel does not match the expected value.
    pub fn validate_sentinel(
        actual: &[u8; 16],
        expected: &[u8; 16],
        section_name: &str,
    ) -> Option<Notification> {
        if Self::check_sentinel(actual, expected) {
            None
        } else {
            Some(Notification::new(
                NotificationType::Warning,
                format!("Invalid section sentinel found in {}", section_name),
            ))
        }
    }

    /// Get the start sentinel for a section by name, if known.
    pub fn start_sentinel(section_name: &str) -> Option<&'static [u8; 16]> {
        sentinels::start_sentinel(section_name)
    }

    /// Get the end sentinel for a section by name, if known.
    pub fn end_sentinel(section_name: &str) -> Option<&'static [u8; 16]> {
        sentinels::end_sentinel(section_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_flags_ac1012() {
        let sio = SectionIO::new(DxfVersion::AC1012);
        assert!(sio.r13_14_only);
        assert!(sio.r13_15_only);
        assert!(!sio.r2000_plus);
        assert!(sio.r2004_pre);
        assert!(!sio.r2004_plus);
        assert!(!sio.r2007_plus);
        assert!(!sio.r2010_plus);
        assert!(!sio.r2013_plus);
        assert!(!sio.r2018_plus);
    }

    #[test]
    fn test_version_flags_ac1015() {
        let sio = SectionIO::new(DxfVersion::AC1015);
        assert!(!sio.r13_14_only);
        assert!(sio.r13_15_only);
        assert!(sio.r2000_plus);
        assert!(sio.r2004_pre);
        assert!(!sio.r2004_plus);
    }

    #[test]
    fn test_version_flags_ac1018() {
        let sio = SectionIO::new(DxfVersion::AC1018);
        assert!(!sio.r13_14_only);
        assert!(!sio.r13_15_only);
        assert!(sio.r2000_plus);
        assert!(!sio.r2004_pre);
        assert!(sio.r2004_plus);
        assert!(!sio.r2007_plus);
    }

    #[test]
    fn test_version_flags_ac1021() {
        let sio = SectionIO::new(DxfVersion::AC1021);
        assert!(sio.r2004_plus);
        assert!(sio.r2007_plus);
        assert!(sio.r2007_pre); // AC1021 is <= AC1021
        assert!(!sio.r2010_plus);
    }

    #[test]
    fn test_version_flags_ac1032() {
        let sio = SectionIO::new(DxfVersion::AC1032);
        assert!(sio.r2000_plus);
        assert!(sio.r2004_plus);
        assert!(sio.r2007_plus);
        assert!(sio.r2010_plus);
        assert!(sio.r2013_plus);
        assert!(sio.r2018_plus);
        assert!(!sio.r2004_pre);
        assert!(!sio.r13_14_only);
    }

    #[test]
    fn test_check_sentinel_match() {
        let a = [0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F];
        let b = [0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F];
        assert!(SectionIO::check_sentinel(&a, &b));
    }

    #[test]
    fn test_check_sentinel_mismatch() {
        let a = [0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F];
        let b = [0x00; 16];
        assert!(!SectionIO::check_sentinel(&a, &b));
    }

    #[test]
    fn test_validate_sentinel_ok() {
        let sentinel = sentinels::HEADER_START;
        assert!(SectionIO::validate_sentinel(&sentinel, &sentinels::HEADER_START, "Header").is_none());
    }

    #[test]
    fn test_validate_sentinel_mismatch() {
        let bad = [0u8; 16];
        let notif = SectionIO::validate_sentinel(&bad, &sentinels::HEADER_START, "Header");
        assert!(notif.is_some());
        assert_eq!(notif.unwrap().notification_type, NotificationType::Warning);
    }
}
