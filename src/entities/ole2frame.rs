//! OLE2FRAME entity — embedded OLE object in a drawing

use super::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transform, Transparency, Vector3};

/// OLE object type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum OleObjectType {
    /// Linked OLE object
    Link = 1,
    /// Embedded OLE object
    Embedded = 2,
    /// Static OLE object
    Static = 3,
}

impl OleObjectType {
    /// Create from DXF code value
    pub fn from_i16(v: i16) -> Self {
        match v {
            1 => OleObjectType::Link,
            3 => OleObjectType::Static,
            _ => OleObjectType::Embedded,
        }
    }
}

/// An embedded OLE2 object entity.
///
/// Stores the binary OLE data and bounding rectangle.
#[derive(Debug, Clone)]
pub struct Ole2Frame {
    /// Common entity data
    pub common: EntityCommon,
    /// OLE version (typically 2)
    pub version: i16,
    /// Name of the source application (e.g. "Excel.Sheet.12")
    pub source_application: String,
    /// Upper-left corner of the OLE frame
    pub upper_left_corner: Vector3,
    /// Lower-right corner of the OLE frame
    pub lower_right_corner: Vector3,
    /// Object type (link, embedded, static)
    pub ole_object_type: OleObjectType,
    /// Whether the object is in paper space
    pub is_paper_space: bool,
    /// Raw OLE binary data (code 310 chunks concatenated)
    pub binary_data: Vec<u8>,
}

impl Ole2Frame {
    /// Create a new OLE2FRAME with defaults
    pub fn new() -> Self {
        Ole2Frame {
            common: EntityCommon::new(),
            version: 2,
            source_application: String::new(),
            upper_left_corner: Vector3::new(1.0, 1.0, 0.0),
            lower_right_corner: Vector3::ZERO,
            ole_object_type: OleObjectType::Embedded,
            is_paper_space: false,
            binary_data: Vec::new(),
        }
    }
}

impl Default for Ole2Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Ole2Frame {
    fn handle(&self) -> Handle { self.common.handle }
    fn set_handle(&mut self, handle: Handle) { self.common.handle = handle; }
    fn layer(&self) -> &str { &self.common.layer }
    fn set_layer(&mut self, layer: String) { self.common.layer = layer; }
    fn color(&self) -> Color { self.common.color }
    fn set_color(&mut self, color: Color) { self.common.color = color; }
    fn line_weight(&self) -> LineWeight { self.common.line_weight }
    fn set_line_weight(&mut self, weight: LineWeight) { self.common.line_weight = weight; }
    fn transparency(&self) -> Transparency { self.common.transparency }
    fn set_transparency(&mut self, transparency: Transparency) { self.common.transparency = transparency; }
    fn is_invisible(&self) -> bool { self.common.invisible }
    fn set_invisible(&mut self, invisible: bool) { self.common.invisible = invisible; }
    fn bounding_box(&self) -> BoundingBox3D {
        BoundingBox3D::from_points(&[self.upper_left_corner, self.lower_right_corner])
            .unwrap_or_else(|| BoundingBox3D::from_point(self.upper_left_corner))
    }
    fn translate(&mut self, offset: Vector3) {
        self.upper_left_corner = self.upper_left_corner + offset;
        self.lower_right_corner = self.lower_right_corner + offset;
    }
    fn entity_type(&self) -> &'static str { "OLE2FRAME" }
    fn apply_transform(&mut self, transform: &Transform) {
        self.upper_left_corner = transform.apply(self.upper_left_corner);
        self.lower_right_corner = transform.apply(self.lower_right_corner);
    }
}
