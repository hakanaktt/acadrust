//! Multi-line text entity

use super::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transparency, Vector3};

/// Attachment point for MText
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AttachmentPoint {
    /// Top left
    TopLeft = 1,
    /// Top center
    TopCenter = 2,
    /// Top right
    TopRight = 3,
    /// Middle left
    MiddleLeft = 4,
    /// Middle center
    MiddleCenter = 5,
    /// Middle right
    MiddleRight = 6,
    /// Bottom left
    BottomLeft = 7,
    /// Bottom center
    BottomCenter = 8,
    /// Bottom right
    BottomRight = 9,
}

/// Drawing direction for MText
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DrawingDirection {
    /// Left to right
    LeftToRight = 1,
    /// Top to bottom
    TopToBottom = 2,
    /// By style
    ByStyle = 3,
}

/// A multi-line text entity
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MText {
    /// Common entity data
    pub common: EntityCommon,
    /// Text content (may contain formatting codes)
    pub value: String,
    /// Insertion point
    pub insertion_point: Vector3,
    /// Text height
    pub height: f64,
    /// Reference rectangle width
    pub rectangle_width: f64,
    /// Reference rectangle height (optional)
    pub rectangle_height: Option<f64>,
    /// Rotation angle in radians
    pub rotation: f64,
    /// Text style name
    pub style: String,
    /// Attachment point
    pub attachment_point: AttachmentPoint,
    /// Drawing direction
    pub drawing_direction: DrawingDirection,
    /// Line spacing factor
    pub line_spacing_factor: f64,
    /// Normal vector
    pub normal: Vector3,
    /// Whether this entity participates in annotation scaling (group code 293; default true)
    pub is_annotative: bool,
}

impl MText {
    /// Create a new MText entity
    pub fn new() -> Self {
        MText {
            common: EntityCommon::new(),
            value: String::new(),
            insertion_point: Vector3::ZERO,
            height: 1.0,
            rectangle_width: 10.0,
            rectangle_height: None,
            rotation: 0.0,
            style: "STANDARD".to_string(),
            attachment_point: AttachmentPoint::TopLeft,
            drawing_direction: DrawingDirection::LeftToRight,
            line_spacing_factor: 1.0,
            normal: Vector3::UNIT_Z,
            is_annotative: true,
        }
    }

    /// Create a new MText with value and position
    pub fn with_value(value: impl Into<String>, position: Vector3) -> Self {
        MText {
            value: value.into(),
            insertion_point: position,
            ..Self::new()
        }
    }

    /// Set the text height
    pub fn with_height(mut self, height: f64) -> Self {
        self.height = height;
        self
    }

    /// Set the rectangle width
    pub fn with_width(mut self, width: f64) -> Self {
        self.rectangle_width = width;
        self
    }
}

impl Default for MText {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for MText {
    fn handle(&self) -> Handle {
        self.common.handle
    }

    fn set_handle(&mut self, handle: Handle) {
        self.common.handle = handle;
    }

    fn layer(&self) -> &str {
        &self.common.layer
    }

    fn set_layer(&mut self, layer: String) {
        self.common.layer = layer;
    }

    fn color(&self) -> Color {
        self.common.color
    }

    fn set_color(&mut self, color: Color) {
        self.common.color = color;
    }

    fn line_weight(&self) -> LineWeight {
        self.common.line_weight
    }

    fn set_line_weight(&mut self, weight: LineWeight) {
        self.common.line_weight = weight;
    }

    fn transparency(&self) -> Transparency {
        self.common.transparency
    }

    fn set_transparency(&mut self, transparency: Transparency) {
        self.common.transparency = transparency;
    }

    fn is_invisible(&self) -> bool {
        self.common.invisible
    }

    fn set_invisible(&mut self, invisible: bool) {
        self.common.invisible = invisible;
    }

    fn bounding_box(&self) -> BoundingBox3D {
        let height = self.rectangle_height.unwrap_or(self.height * 2.0);
        BoundingBox3D::new(
            self.insertion_point,
            Vector3::new(
                self.insertion_point.x + self.rectangle_width,
                self.insertion_point.y + height,
                self.insertion_point.z,
            ),
        )
    }

    fn translate(&mut self, offset: Vector3) {
        super::translate::translate_mtext(self, offset);
    }

    fn entity_type(&self) -> &'static str {
        "MTEXT"
    }
    
    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        super::transform::transform_mtext(self, transform);
    }
    
    fn apply_mirror(&mut self, transform: &crate::types::Transform) {
        super::mirror::mirror_mtext(self, transform);
    }
}


