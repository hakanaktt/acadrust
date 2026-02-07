//! Viewport table entry

use super::TableEntry;
use crate::types::{Handle, Vector2, Vector3};

/// A viewport table entry
#[derive(Debug, Clone)]
pub struct VPort {
    /// Unique handle
    pub handle: Handle,
    /// Viewport name
    pub name: String,
    /// Lower-left corner
    pub lower_left: Vector2,
    /// Upper-right corner
    pub upper_right: Vector2,
    /// View center point
    pub view_center: Vector2,
    /// Snap base point
    pub snap_base: Vector2,
    /// Snap spacing
    pub snap_spacing: Vector2,
    /// Grid spacing
    pub grid_spacing: Vector2,
    /// View direction
    pub view_direction: Vector3,
    /// View target
    pub view_target: Vector3,
    /// View height
    pub view_height: f64,
    /// Aspect ratio
    pub aspect_ratio: f64,
    /// Lens length
    pub lens_length: f64,
}

impl VPort {
    /// Create a new viewport
    pub fn new(name: impl Into<String>) -> Self {
        VPort {
            handle: Handle::NULL,
            name: name.into(),
            lower_left: Vector2::ZERO,
            upper_right: Vector2::new(1.0, 1.0),
            view_center: Vector2::ZERO,
            snap_base: Vector2::ZERO,
            snap_spacing: Vector2::new(0.5, 0.5),
            grid_spacing: Vector2::new(10.0, 10.0),
            view_direction: Vector3::UNIT_Z,
            view_target: Vector3::ZERO,
            view_height: 10.0,
            aspect_ratio: 1.0,
            lens_length: 50.0,
        }
    }

    /// Create the standard "*Active" viewport
    pub fn active() -> Self {
        Self::new("*Active")
    }
}

impl TableEntry for VPort {
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
        self.name == "*Active"
    }
}


