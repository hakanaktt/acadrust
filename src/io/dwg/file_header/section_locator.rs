//! DWG section locator record.
//!
//! Used in AC15 (R13–R2000) file headers to locate sections.
//! Mirrors ACadSharp `DwgSectionLocatorRecord`.

/// A record indicating the file offset and size of a section in AC15 drawings.
#[derive(Debug, Clone, Default)]
pub struct DwgSectionLocatorRecord {
    /// Section number (index).
    pub number: i32,
    /// Byte offset into the DWG file.
    pub seeker: i64,
    /// Size of the section in bytes.
    pub size: i64,
}

impl DwgSectionLocatorRecord {
    /// Create a new section locator record.
    pub fn new(number: i32, seeker: i64, size: i64) -> Self {
        Self {
            number,
            seeker,
            size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let rec = DwgSectionLocatorRecord::default();
        assert_eq!(rec.number, 0);
        assert_eq!(rec.seeker, 0);
        assert_eq!(rec.size, 0);
    }

    #[test]
    fn test_new() {
        let rec = DwgSectionLocatorRecord::new(2, 0x1234, 0x5678);
        assert_eq!(rec.number, 2);
        assert_eq!(rec.seeker, 0x1234);
        assert_eq!(rec.size, 0x5678);
    }
}
