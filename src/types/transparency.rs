//! Transparency representation for CAD entities

use std::fmt;

/// Represents transparency in AutoCAD
///
/// Transparency is represented as an alpha value where:
/// - 0 = fully opaque (0% transparent)
/// - 255 = fully transparent (100% transparent)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Transparency(u8);

impl Transparency {
    /// Fully opaque (0% transparent)
    pub const OPAQUE: Transparency = Transparency(0);

    /// Fully transparent (100% transparent)
    pub const TRANSPARENT: Transparency = Transparency(255);

    /// Transparency ByLayer - uses the layer's transparency
    pub const BY_LAYER: Transparency = Transparency(0);

    /// Create a new transparency from an alpha value (0-255)
    pub const fn new(alpha: u8) -> Self {
        Transparency(alpha)
    }

    /// Create transparency from a percentage (0.0 = opaque, 1.0 = transparent)
    pub fn from_percent(percent: f64) -> Self {
        let alpha = (percent.clamp(0.0, 1.0) * 255.0) as u8;
        Transparency(alpha)
    }

    /// Create transparency from DWG alpha value (32-bit format)
    ///
    /// The first byte represents the transparency type:
    /// - 0 = BYLAYER
    /// - 1 = BYBLOCK  
    /// - 3 = the transparency value in the last byte
    pub fn from_alpha_value(value: u32) -> Self {
        let type_byte = (value >> 24) as u8;
        match type_byte {
            0 => Transparency::BY_LAYER,
            1 => Transparency::OPAQUE, // BYBLOCK = opaque for now
            3 => Transparency((value & 0xFF) as u8),
            _ => Transparency::OPAQUE,
        }
    }

    /// Get the raw alpha value (0-255)
    pub const fn alpha(&self) -> u8 {
        self.0
    }

    /// Get transparency as a percentage (0.0 = opaque, 1.0 = transparent)
    pub fn as_percent(&self) -> f64 {
        self.0 as f64 / 255.0
    }

    /// Check if fully opaque
    pub const fn is_opaque(&self) -> bool {
        self.0 == 0
    }

    /// Check if fully transparent
    pub const fn is_transparent(&self) -> bool {
        self.0 == 255
    }

    /// Common transparency values
    pub const T_10: Transparency = Transparency(26);   // 10% transparent
    pub const T_20: Transparency = Transparency(51);   // 20% transparent
    pub const T_30: Transparency = Transparency(77);   // 30% transparent
    pub const T_40: Transparency = Transparency(102);  // 40% transparent
    pub const T_50: Transparency = Transparency(128);  // 50% transparent
    pub const T_60: Transparency = Transparency(153);  // 60% transparent
    pub const T_70: Transparency = Transparency(179);  // 70% transparent
    pub const T_80: Transparency = Transparency(204);  // 80% transparent
    pub const T_90: Transparency = Transparency(230);  // 90% transparent
    
    /// Convert to DWG alpha value (32-bit format)
    ///
    /// The first byte represents the transparency type:
    /// - 0 = BYLAYER
    /// - 1 = BYBLOCK
    /// - 3 = the transparency value in the last byte
    pub fn to_alpha_value(&self) -> i32 {
        if self.0 == 0 {
            // Fully opaque, use BYLAYER type
            0
        } else {
            // Type 3 = explicit value
            ((3u32 << 24) | self.0 as u32) as i32
        }
    }
}

impl Default for Transparency {
    fn default() -> Self {
        Transparency::OPAQUE
    }
}

impl From<u8> for Transparency {
    fn from(alpha: u8) -> Self {
        Transparency(alpha)
    }
}

impl From<Transparency> for u8 {
    fn from(transparency: Transparency) -> Self {
        transparency.0
    }
}

impl fmt::Display for Transparency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.as_percent() * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transparency_creation() {
        let t = Transparency::new(128);
        assert_eq!(t.alpha(), 128);
    }

    #[test]
    fn test_transparency_from_percent() {
        let t = Transparency::from_percent(0.5);
        // 0.5 * 255.0 = 127.5, which rounds down to 127
        assert_eq!(t.alpha(), 127);

        let t = Transparency::from_percent(0.0);
        assert_eq!(t.alpha(), 0);

        let t = Transparency::from_percent(1.0);
        assert_eq!(t.alpha(), 255);
    }

    #[test]
    fn test_transparency_as_percent() {
        assert_eq!(Transparency::OPAQUE.as_percent(), 0.0);
        assert_eq!(Transparency::TRANSPARENT.as_percent(), 1.0);
        assert!((Transparency::T_50.as_percent() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_transparency_checks() {
        assert!(Transparency::OPAQUE.is_opaque());
        assert!(!Transparency::OPAQUE.is_transparent());
        assert!(Transparency::TRANSPARENT.is_transparent());
        assert!(!Transparency::TRANSPARENT.is_opaque());
    }

    #[test]
    fn test_transparency_display() {
        assert_eq!(Transparency::OPAQUE.to_string(), "0.0%");
        assert_eq!(Transparency::TRANSPARENT.to_string(), "100.0%");
    }

    #[test]
    fn test_transparency_conversion() {
        let alpha: u8 = 100;
        let t: Transparency = alpha.into();
        let back: u8 = t.into();
        assert_eq!(alpha, back);
    }

    #[test]
    fn test_default_transparency() {
        assert_eq!(Transparency::default(), Transparency::OPAQUE);
    }
}


