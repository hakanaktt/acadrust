//! Line weight representation for CAD entities

use std::fmt;

/// Represents line weight in AutoCAD
///
/// Line weights are specified in millimeters (mm) or can be special values
/// like ByLayer, ByBlock, or Default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LineWeight {
    /// Use the layer's line weight
    #[default]
    ByLayer,
    /// Use the block's line weight
    ByBlock,
    /// Default line weight
    Default,
    /// Specific line weight in 1/100 mm
    /// Value range: 0-211 (representing 0.00mm to 2.11mm)
    Value(i16),
}

impl LineWeight {
    /// Create a line weight from a raw value
    pub fn from_value(value: i16) -> Self {
        match value {
            -1 => LineWeight::ByLayer,
            -2 => LineWeight::ByBlock,
            -3 => LineWeight::Default,
            v => LineWeight::Value(v),
        }
    }

    /// Get the raw value
    pub fn value(&self) -> i16 {
        match self {
            LineWeight::ByLayer => -1,
            LineWeight::ByBlock => -2,
            LineWeight::Default => -3,
            LineWeight::Value(v) => *v,
        }
    }

    /// Get the raw value as i16 (alias for value())
    pub fn as_i16(&self) -> i16 {
        self.value()
    }

    /// Get the line weight in millimeters
    pub fn millimeters(&self) -> Option<f64> {
        match self {
            LineWeight::Value(v) => Some(*v as f64 / 100.0),
            _ => None,
        }
    }

    /// Common line weight constants (in 1/100 mm)
    pub const W0_00: LineWeight = LineWeight::Value(0);
    pub const W0_05: LineWeight = LineWeight::Value(5);
    pub const W0_09: LineWeight = LineWeight::Value(9);
    pub const W0_13: LineWeight = LineWeight::Value(13);
    pub const W0_15: LineWeight = LineWeight::Value(15);
    pub const W0_18: LineWeight = LineWeight::Value(18);
    pub const W0_20: LineWeight = LineWeight::Value(20);
    pub const W0_25: LineWeight = LineWeight::Value(25);
    pub const W0_30: LineWeight = LineWeight::Value(30);
    pub const W0_35: LineWeight = LineWeight::Value(35);
    pub const W0_40: LineWeight = LineWeight::Value(40);
    pub const W0_50: LineWeight = LineWeight::Value(50);
    pub const W0_53: LineWeight = LineWeight::Value(53);
    pub const W0_60: LineWeight = LineWeight::Value(60);
    pub const W0_70: LineWeight = LineWeight::Value(70);
    pub const W0_80: LineWeight = LineWeight::Value(80);
    pub const W0_90: LineWeight = LineWeight::Value(90);
    pub const W1_00: LineWeight = LineWeight::Value(100);
    pub const W1_06: LineWeight = LineWeight::Value(106);
    pub const W1_20: LineWeight = LineWeight::Value(120);
    pub const W1_40: LineWeight = LineWeight::Value(140);
    pub const W1_58: LineWeight = LineWeight::Value(158);
    pub const W2_00: LineWeight = LineWeight::Value(200);
    pub const W2_11: LineWeight = LineWeight::Value(211);
}

impl fmt::Display for LineWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineWeight::ByLayer => write!(f, "ByLayer"),
            LineWeight::ByBlock => write!(f, "ByBlock"),
            LineWeight::Default => write!(f, "Default"),
            LineWeight::Value(v) => write!(f, "{:.2}mm", *v as f64 / 100.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_weight_from_value() {
        assert_eq!(LineWeight::from_value(-1), LineWeight::ByLayer);
        assert_eq!(LineWeight::from_value(-2), LineWeight::ByBlock);
        assert_eq!(LineWeight::from_value(-3), LineWeight::Default);
        assert_eq!(LineWeight::from_value(25), LineWeight::Value(25));
    }

    #[test]
    fn test_line_weight_value() {
        assert_eq!(LineWeight::ByLayer.value(), -1);
        assert_eq!(LineWeight::Value(50).value(), 50);
    }

    #[test]
    fn test_line_weight_millimeters() {
        assert_eq!(LineWeight::Value(25).millimeters(), Some(0.25));
        assert_eq!(LineWeight::Value(100).millimeters(), Some(1.0));
        assert_eq!(LineWeight::ByLayer.millimeters(), None);
    }

    #[test]
    fn test_line_weight_constants() {
        assert_eq!(LineWeight::W0_25.millimeters(), Some(0.25));
        assert_eq!(LineWeight::W1_00.millimeters(), Some(1.0));
    }

    #[test]
    fn test_line_weight_display() {
        assert_eq!(LineWeight::ByLayer.to_string(), "ByLayer");
        assert_eq!(LineWeight::W0_25.to_string(), "0.25mm");
    }

    #[test]
    fn test_default_line_weight() {
        assert_eq!(LineWeight::default(), LineWeight::ByLayer);
    }
}

