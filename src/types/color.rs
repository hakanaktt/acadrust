//! Color representation for CAD entities

use std::fmt;

/// Represents a color in AutoCAD
///
/// Colors can be represented in multiple ways:
/// - By index (0-256): AutoCAD Color Index (ACI)
/// - By RGB values: True color
/// - By layer: Use the layer's color
/// - By block: Use the block's color
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// Color by layer (index 256)
    #[default]
    ByLayer,
    /// Color by block (index 0)
    ByBlock,
    /// AutoCAD Color Index (1-255)
    Index(u8),
    /// True color with RGB values
    Rgb { r: u8, g: u8, b: u8 },
}

impl Color {
    /// Create a color from an AutoCAD Color Index
    pub fn from_index(index: i16) -> Self {
        match index {
            0 => Color::ByBlock,
            256 => Color::ByLayer,
            1..=255 => Color::Index(index as u8),
            _ if index < 0 => Color::Index((-index).min(255) as u8),  // Negative means layer is off
            _ => Color::Index(7), // Default to white
        }
    }

    /// Create a true color from RGB values
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Rgb { r, g, b }
    }

    /// Get the color index (if applicable)
    pub fn index(&self) -> Option<u16> {
        match self {
            Color::ByBlock => Some(0),
            Color::Index(i) => Some(*i as u16),
            Color::ByLayer => Some(256),
            Color::Rgb { .. } => None,
        }
    }

    /// Get RGB values (if applicable)
    pub fn rgb(&self) -> Option<(u8, u8, u8)> {
        match self {
            Color::Rgb { r, g, b } => Some((*r, *g, *b)),
            _ => None,
        }
    }

    /// Common color constants
    pub const RED: Color = Color::Index(1);
    pub const YELLOW: Color = Color::Index(2);
    pub const GREEN: Color = Color::Index(3);
    pub const CYAN: Color = Color::Index(4);
    pub const BLUE: Color = Color::Index(5);
    pub const MAGENTA: Color = Color::Index(6);
    pub const WHITE: Color = Color::Index(7);
    pub const GRAY: Color = Color::Index(8);
    pub const LIGHT_GRAY: Color = Color::Index(9);
    
    /// Approximate a true color to the nearest ACI index
    pub fn approximate_index(&self) -> i16 {
        match self {
            Color::ByBlock => 0,
            Color::ByLayer => 256,
            Color::Index(i) => *i as i16,
            Color::Rgb { r, g, b } => {
                // Simple approximation to ACI color
                // This is a rough approximation - a full implementation would use color tables
                let brightness = ((*r as u16) + (*g as u16) + (*b as u16)) / 3;
                if brightness < 32 {
                    8 // dark gray
                } else if brightness > 224 {
                    7 // white
                } else if *r > *g && *r > *b {
                    1 // red
                } else if *g > *r && *g > *b {
                    3 // green
                } else if *b > *r && *b > *g {
                    5 // blue
                } else if *r > 128 && *g > 128 {
                    2 // yellow
                } else if *g > 128 && *b > 128 {
                    4 // cyan
                } else if *r > 128 && *b > 128 {
                    6 // magenta
                } else {
                    7 // white
                }
            }
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::ByLayer => write!(f, "ByLayer"),
            Color::ByBlock => write!(f, "ByBlock"),
            Color::Index(i) => write!(f, "Index({})", i),
            Color::Rgb { r, g, b } => write!(f, "RGB({}, {}, {})", r, g, b),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_index() {
        assert_eq!(Color::from_index(0), Color::ByBlock);
        assert_eq!(Color::from_index(256), Color::ByLayer);
        assert_eq!(Color::from_index(1), Color::Index(1));
    }

    #[test]
    fn test_color_rgb() {
        let color = Color::from_rgb(255, 128, 64);
        assert_eq!(color.rgb(), Some((255, 128, 64)));
        assert_eq!(color.index(), None);
    }

    #[test]
    fn test_color_index() {
        let color = Color::Index(5);
        assert_eq!(color.index(), Some(5));
        assert_eq!(color.rgb(), None);
    }

    #[test]
    fn test_color_constants() {
        assert_eq!(Color::RED, Color::Index(1));
        assert_eq!(Color::BLUE, Color::Index(5));
    }

    #[test]
    fn test_color_display() {
        assert_eq!(Color::ByLayer.to_string(), "ByLayer");
        assert_eq!(Color::from_rgb(255, 0, 0).to_string(), "RGB(255, 0, 0)");
    }

    #[test]
    fn test_default_color() {
        assert_eq!(Color::default(), Color::ByLayer);
    }
}


