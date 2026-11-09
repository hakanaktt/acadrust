//! Seqend entity — end-of-sequence marker for polyline vertices and insert attributes

use super::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Transform, Vector3};

/// Marker entity signaling the end of a vertex or attribute sequence.
///
/// Seqend has no geometry or entity-specific data. It only carries
/// the inherited common entity fields (handle, owner, layer, etc.).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Seqend {
    /// Common entity data
    pub common: EntityCommon,
}

impl Seqend {
    /// Create a new Seqend marker
    pub fn new() -> Self {
        Seqend {
            common: EntityCommon::new(),
        }
    }
}

impl Default for Seqend {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Seqend {
    fn common(&self) -> &super::EntityCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::EntityCommon {
        &mut self.common
    }

    fn bounding_box(&self) -> BoundingBox3D {
        BoundingBox3D::from_point(Vector3::ZERO)
    }
    fn translate(&mut self, _offset: Vector3) {
        super::translate::translate_seqend(self, _offset);
    }
    fn entity_type(&self) -> &'static str {
        "SEQEND"
    }
    fn apply_transform(&mut self, _transform: &Transform) {
        super::transform::transform_seqend(self, _transform);
    }
}
