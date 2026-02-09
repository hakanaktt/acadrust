//! Unknown entity type for round-trip preservation.
//!
//! When the reader encounters an entity type that is not directly supported,
//! it can still capture the common entity properties (handle, layer, color, …)
//! wrapped in this type.  The entity-specific codes are discarded — this
//! matches the ACadSharp `UnknownEntity` behavior.
//!
//! Unknown entities are **never written back** to DXF output, following the
//! same convention as ACadSharp.

use crate::entities::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transform, Transparency, Vector3};

/// An entity whose type is not directly supported by the library.
///
/// Preserves the DXF type name (e.g. `"ACAD_PROXY_ENTITY"`) and common entity
/// properties.  Entity-specific codes are discarded.
#[derive(Debug, Clone)]
pub struct UnknownEntity {
    /// Common entity data (handle, layer, color, reactors, …).
    pub common: EntityCommon,
    /// The DXF type name as it appeared in the file (e.g. `"ACAD_PROXY_ENTITY"`).
    pub dxf_name: String,
}

impl UnknownEntity {
    /// Create a new unknown entity with the given DXF type name.
    pub fn new(dxf_name: impl Into<String>) -> Self {
        Self {
            common: EntityCommon::new(),
            dxf_name: dxf_name.into(),
        }
    }
}

impl Entity for UnknownEntity {
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
    fn bounding_box(&self) -> BoundingBox3D { BoundingBox3D::from_point(Vector3::ZERO) }
    fn translate(&mut self, _offset: Vector3) { /* no geometry */ }
    fn entity_type(&self) -> &'static str { "UNKNOWN" }
    fn apply_transform(&mut self, _transform: &Transform) { /* no geometry */ }
}
