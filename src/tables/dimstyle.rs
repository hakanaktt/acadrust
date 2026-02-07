//! Dimension style table entry

use super::TableEntry;
use crate::types::Handle;

/// A dimension style table entry
#[derive(Debug, Clone)]
pub struct DimStyle {
    /// Unique handle
    pub handle: Handle,
    /// Style name
    pub name: String,
    /// Dimension line color
    pub dimclrd: i16,
    /// Extension line color
    pub dimclre: i16,
    /// Dimension text color
    pub dimclrt: i16,
    /// Dimension scale factor
    pub dimscale: f64,
    /// Dimension text height
    pub dimtxt: f64,
    /// Arrow size
    pub dimasz: f64,
    /// Extension line extension
    pub dimexe: f64,
    /// Extension line offset
    pub dimexo: f64,
    /// Text style name
    pub dimtxsty: String,
}

impl DimStyle {
    /// Create a new dimension style
    pub fn new(name: impl Into<String>) -> Self {
        DimStyle {
            handle: Handle::NULL,
            name: name.into(),
            dimclrd: 0,
            dimclre: 0,
            dimclrt: 0,
            dimscale: 1.0,
            dimtxt: 0.18,
            dimasz: 0.18,
            dimexe: 0.18,
            dimexo: 0.0625,
            dimtxsty: "Standard".to_string(),
        }
    }

    /// Create the standard dimension style
    pub fn standard() -> Self {
        Self::new("Standard")
    }
}

impl TableEntry for DimStyle {
    fn handle(&self) -> Handle {
        self.handle
    }

    fn set_handle(&mut self, handle: Handle) {
        self.handle = handle;
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn is_standard(&self) -> bool {
        self.name == "Standard"
    }
}


