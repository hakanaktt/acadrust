//! View table entry

use super::TableEntry;
use crate::types::{Handle, Vector3};

/// A view table entry
#[derive(Debug, Clone)]
pub struct View {
    /// Unique handle
    pub handle: Handle,
    /// View name
    pub name: String,
    /// View center point
    pub center: Vector3,
    /// View height
    pub height: f64,
    /// View width
    pub width: f64,
    /// View direction (from target)
    pub direction: Vector3,
    /// View target point
    pub target: Vector3,
    /// Lens length
    pub lens_length: f64,
    /// Front clipping plane offset
    pub front_clip: f64,
    /// Back clipping plane offset
    pub back_clip: f64,
    /// Twist angle
    pub twist_angle: f64,
}

impl View {
    /// Create a new view
    pub fn new(name: impl Into<String>) -> Self {
        View {
            handle: Handle::NULL,
            name: name.into(),
            center: Vector3::ZERO,
            height: 1.0,
            width: 1.0,
            direction: Vector3::UNIT_Z,
            target: Vector3::ZERO,
            lens_length: 50.0,
            front_clip: 0.0,
            back_clip: 0.0,
            twist_angle: 0.0,
        }
    }
}

impl TableEntry for View {
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
}


