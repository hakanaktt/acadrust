//! DWG handle reference types and resolution.
//!
//! In the DWG format, object handles are encoded with a reference code that
//! determines how the handle value is resolved relative to a reference handle.
//!
//! Handle encoding: `|CODE (4 bits)|COUNTER (4 bits)|HANDLE bytes (N)|`
//!
//! Mirrors ACadSharp's handle reference system.

/// DWG handle reference code.
///
/// The handle code determines how the raw handle value is interpreted
/// relative to the parent (reference) handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DwgReferenceType {
    /// Undefined reference (code 0)
    Undefined = 0,
    /// Soft ownership reference (code 2) — absolute handle
    SoftOwnership = 2,
    /// Hard ownership reference (code 3) — absolute handle
    HardOwnership = 3,
    /// Soft pointer reference (code 4) — absolute handle
    SoftPointer = 4,
    /// Hard pointer reference (code 5) — absolute handle
    HardPointer = 5,
    /// Offset +1 from reference handle (code 6)
    HardOwnershipPlus1 = 6,
    /// Offset -1 from reference handle (code 8)
    HardOwnershipMinus1 = 8,
    /// Offset +N from reference handle (code 0xA)
    SoftPointerPlusOffset = 0xA,
    /// Offset -N from reference handle (code 0xC)
    SoftPointerMinusOffset = 0xC,
}

impl DwgReferenceType {
    /// Try to create a reference type from a raw code value.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(DwgReferenceType::Undefined),
            2 => Some(DwgReferenceType::SoftOwnership),
            3 => Some(DwgReferenceType::HardOwnership),
            4 => Some(DwgReferenceType::SoftPointer),
            5 => Some(DwgReferenceType::HardPointer),
            6 => Some(DwgReferenceType::HardOwnershipPlus1),
            8 => Some(DwgReferenceType::HardOwnershipMinus1),
            0xA => Some(DwgReferenceType::SoftPointerPlusOffset),
            0xC => Some(DwgReferenceType::SoftPointerMinusOffset),
            _ => None,
        }
    }

    /// Whether this reference type uses an absolute handle value.
    pub fn is_absolute(&self) -> bool {
        matches!(
            self,
            DwgReferenceType::Undefined
                | DwgReferenceType::SoftOwnership
                | DwgReferenceType::HardOwnership
                | DwgReferenceType::SoftPointer
                | DwgReferenceType::HardPointer
        )
    }

    /// Whether this reference type uses an offset from the reference handle.
    pub fn is_offset(&self) -> bool {
        !self.is_absolute()
    }
}

/// A raw handle reference as read from the DWG file.
///
/// Contains the reference code, byte count, and raw handle value.
/// Must be resolved against a reference handle to get the absolute handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandleReference {
    /// The reference code (upper 4 bits of the first handle byte).
    pub code: u8,
    /// Number of handle bytes (lower 4 bits of the first handle byte).
    pub counter: u8,
    /// The raw handle value (assembled from `counter` bytes).
    pub handle: u64,
}

impl HandleReference {
    /// Create a new handle reference.
    pub fn new(code: u8, counter: u8, handle: u64) -> Self {
        Self {
            code,
            counter,
            handle,
        }
    }

    /// Resolve the absolute handle value given a reference (parent) handle.
    ///
    /// For absolute reference types (codes 0, 2, 3, 4, 5) the raw handle
    /// is the absolute value. For offset types:
    /// - Code 6: reference_handle + 1
    /// - Code 8: reference_handle - 1
    /// - Code 0xA: reference_handle + handle
    /// - Code 0xC: reference_handle - handle
    pub fn resolve(&self, reference_handle: u64) -> u64 {
        match self.code {
            // Absolute handle types
            0 | 2 | 3 | 4 | 5 => self.handle,
            // +1 offset
            6 => reference_handle.wrapping_add(1),
            // -1 offset
            8 => reference_handle.wrapping_sub(1),
            // +N offset
            0xA => reference_handle.wrapping_add(self.handle),
            // -N offset
            0xC => reference_handle.wrapping_sub(self.handle),
            // Unknown code — treat as absolute
            _ => self.handle,
        }
    }

    /// Get the reference type enum, if the code is recognized.
    pub fn reference_type(&self) -> Option<DwgReferenceType> {
        DwgReferenceType::from_code(self.code)
    }
}

impl Default for HandleReference {
    fn default() -> Self {
        Self {
            code: 0,
            counter: 0,
            handle: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_type_from_code() {
        assert_eq!(
            DwgReferenceType::from_code(2),
            Some(DwgReferenceType::SoftOwnership)
        );
        assert_eq!(
            DwgReferenceType::from_code(5),
            Some(DwgReferenceType::HardPointer)
        );
        assert_eq!(DwgReferenceType::from_code(1), None);
        assert_eq!(DwgReferenceType::from_code(7), None);
    }

    #[test]
    fn test_is_absolute() {
        assert!(DwgReferenceType::SoftOwnership.is_absolute());
        assert!(DwgReferenceType::HardPointer.is_absolute());
        assert!(!DwgReferenceType::HardOwnershipPlus1.is_absolute());
        assert!(!DwgReferenceType::SoftPointerMinusOffset.is_absolute());
    }

    #[test]
    fn test_resolve_absolute() {
        let href = HandleReference::new(4, 2, 0x1A);
        assert_eq!(href.resolve(0x50), 0x1A);
    }

    #[test]
    fn test_resolve_plus1() {
        let href = HandleReference::new(6, 0, 0);
        assert_eq!(href.resolve(0x10), 0x11);
    }

    #[test]
    fn test_resolve_minus1() {
        let href = HandleReference::new(8, 0, 0);
        assert_eq!(href.resolve(0x10), 0x0F);
    }

    #[test]
    fn test_resolve_plus_offset() {
        let href = HandleReference::new(0xA, 1, 5);
        assert_eq!(href.resolve(0x10), 0x15);
    }

    #[test]
    fn test_resolve_minus_offset() {
        let href = HandleReference::new(0xC, 1, 3);
        assert_eq!(href.resolve(0x10), 0x0D);
    }
}
