//! Line type table entry

use super::TableEntry;
use crate::types::Handle;

/// Line type element (dash, dot, space)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineTypeElement {
    /// Length of the element (positive = dash, negative = space, 0 = dot)
    pub length: f64,
}

impl LineTypeElement {
    /// Create a dash element
    pub fn dash(length: f64) -> Self {
        LineTypeElement { length: length.abs() }
    }

    /// Create a space element
    pub fn space(length: f64) -> Self {
        LineTypeElement { length: -length.abs() }
    }

    /// Create a dot element
    pub fn dot() -> Self {
        LineTypeElement { length: 0.0 }
    }

    /// Check if this is a dash
    pub fn is_dash(&self) -> bool {
        self.length > 0.0
    }

    /// Check if this is a space
    pub fn is_space(&self) -> bool {
        self.length < 0.0
    }

    /// Check if this is a dot
    pub fn is_dot(&self) -> bool {
        self.length == 0.0
    }
}

/// A line type table entry
#[derive(Debug, Clone)]
pub struct LineType {
    /// Unique handle
    pub handle: Handle,
    /// Line type name
    pub name: String,
    /// Description
    pub description: String,
    /// Pattern elements
    pub elements: Vec<LineTypeElement>,
    /// Total pattern length
    pub pattern_length: f64,
    /// Alignment (always 'A' for AutoCAD)
    pub alignment: char,
}

impl LineType {
    /// Create a new line type
    pub fn new(name: impl Into<String>) -> Self {
        LineType {
            handle: Handle::NULL,
            name: name.into(),
            description: String::new(),
            elements: Vec::new(),
            pattern_length: 0.0,
            alignment: 'A',
        }
    }

    /// Create the standard "Continuous" line type
    pub fn continuous() -> Self {
        LineType {
            handle: Handle::NULL,
            name: "Continuous".to_string(),
            description: "Solid line".to_string(),
            elements: Vec::new(),
            pattern_length: 0.0,
            alignment: 'A',
        }
    }

    /// Create the standard "ByLayer" line type
    pub fn by_layer() -> Self {
        LineType {
            handle: Handle::NULL,
            name: "ByLayer".to_string(),
            description: String::new(),
            elements: Vec::new(),
            pattern_length: 0.0,
            alignment: 'A',
        }
    }

    /// Create the standard "ByBlock" line type
    pub fn by_block() -> Self {
        LineType {
            handle: Handle::NULL,
            name: "ByBlock".to_string(),
            description: String::new(),
            elements: Vec::new(),
            pattern_length: 0.0,
            alignment: 'A',
        }
    }

    /// Create a dashed line type
    pub fn dashed() -> Self {
        let mut lt = LineType::new("Dashed");
        lt.description = "__ __ __ __ __ __".to_string();
        lt.add_element(LineTypeElement::dash(0.5));
        lt.add_element(LineTypeElement::space(0.25));
        lt.pattern_length = 0.75;
        lt
    }

    /// Create a dotted line type
    pub fn dotted() -> Self {
        let mut lt = LineType::new("Dotted");
        lt.description = ". . . . . . . .".to_string();
        lt.add_element(LineTypeElement::dot());
        lt.add_element(LineTypeElement::space(0.25));
        lt.pattern_length = 0.25;
        lt
    }

    /// Add an element to the pattern
    pub fn add_element(&mut self, element: LineTypeElement) {
        self.elements.push(element);
    }

    /// Get the number of elements
    pub fn element_count(&self) -> usize {
        self.elements.len()
    }

    /// Check if this is a continuous line type
    pub fn is_continuous(&self) -> bool {
        self.elements.is_empty()
    }
}

impl TableEntry for LineType {
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
        matches!(
            self.name.as_str(),
            "Continuous" | "ByLayer" | "ByBlock"
        )
    }
}

