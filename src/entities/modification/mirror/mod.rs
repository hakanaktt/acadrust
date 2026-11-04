//! Planar MIRROR, using a mirror line in the world XY plane.
//!
//! The public convenience methods are available on [`EntityType`] and
//! [`Entity`](crate::entities::Entity). Entity-specific behavior is grouped
//! alongside the corresponding groups in the explosion module.
//! Arbitrary reflection planes are exposed separately by [`super::mirror3d`].

use crate::entities::*;
use crate::types::{Transform, Vector3};

mod blocks;
mod curves;
mod hatch;
mod leaders;
mod meshes;
mod polylines;
mod primitives;
mod text;

pub(crate) use blocks::*;
pub(crate) use curves::*;
pub(crate) use hatch::*;
pub(crate) use leaders::*;
pub(crate) use meshes::*;
pub(crate) use polylines::*;
pub(crate) use primitives::*;
pub(crate) use text::*;

/// Text and hatch policies for MIRROR and MIRROR3D.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MirrorOptions {
    /// ACAD MIRRTEXT: false keeps text readable; true reflects glyphs.
    pub mirror_text: bool,
    /// ACAD MIRRHATCH: false preserves pattern direction; true reflects it.
    pub mirror_hatch: bool,
    /// Exact text advance width, including height and width factor.
    /// Aligned/Fit text uses its alignment points. Other text uses an estimate
    /// when this is absent. An INSERT applies this width to each attribute;
    /// attributes of different widths require individual font measurements.
    pub text_width: Option<f64>,
}

pub(crate) fn valid_transform(transform: &Transform) -> bool {
    !transform.is_identity() && transform.matrix.m.iter().flatten().all(|v| v.is_finite())
}

impl EntityType {
    /// Apply a mirror transform with entity-specific corrections.
    ///
    /// Entities that require post-processing (arc angle swaps, bulge
    /// negation, face winding reversal) are handled here. All other
    /// entities delegate to [`apply_transform`](EntityType::apply_transform).
    /// This low-level compatibility method accepts arbitrary reflection
    /// transforms; command entry points are [`Self::mirror`] and [`Self::mirror3d`].
    pub fn apply_mirror(&mut self, transform: &Transform) {
        self.apply_mirror_with_options(transform, MirrorOptions::default());
    }

    /// Apply a reflection with an explicit MIRRTEXT policy.
    /// This edits the entity in place; copying and source erasure are caller-owned.
    pub fn apply_mirror_with_options(&mut self, transform: &Transform, options: MirrorOptions) {
        if !valid_transform(transform) {
            return;
        }
        match self {
            EntityType::Arc(e) => mirror_arc(e, transform),
            EntityType::Ellipse(e) => mirror_ellipse(e, transform),
            EntityType::LwPolyline(e) => mirror_lwpolyline(e, transform),
            EntityType::Polyline2D(e) => mirror_polyline2d(e, transform),
            EntityType::MLine(e) => mirror_mline(e, transform),
            EntityType::Spline(e) => mirror_spline(e, transform),
            EntityType::Helix(e) => mirror_helix(e, transform),
            EntityType::Face3D(e) => mirror_face3d(e, transform),
            EntityType::Solid(e) => mirror_solid(e, transform),
            EntityType::Mesh(e) => mirror_mesh(e, transform),
            EntityType::PolyfaceMesh(e) => mirror_polyface_mesh(e, transform),
            EntityType::PolygonMesh(e) => mirror_polygon_mesh(e, transform),
            EntityType::Solid3D(e) => mirror_solid3d(e, transform),
            EntityType::Region(e) => mirror_region(e, transform),
            EntityType::Body(e) => mirror_body(e, transform),
            EntityType::Surface(e) => mirror_surface(e, transform),
            EntityType::Text(e) => mirror_text_with_options(e, transform, options),
            EntityType::MText(e) => mirror_mtext_with_options(e, transform, options),
            EntityType::AttributeDefinition(e) => {
                mirror_attribute_definition_with_options(e, transform, options)
            }
            EntityType::AttributeEntity(e) => {
                mirror_attribute_entity_with_options(e, transform, options)
            }
            EntityType::Insert(e) => mirror_insert_with_options(e, transform, options),
            EntityType::Underlay(e) => mirror_underlay(e, transform),
            EntityType::Leader(e) => mirror_leader(e, transform),
            EntityType::Shape(e) => mirror_shape(e, transform),
            EntityType::Hatch(e) => mirror_hatch_with_options(e, transform, options),
            // Carries its XDATA along itself.
            _ => return self.apply_transform(transform),
        }
        let _ = self.common_mut().extended_data.apply_transform(transform);
        if super::transform::transform_invalidates_raw_record(self) {
            self.common_mut().raw_record = None;
        }
    }

    /// Mirror across the YZ plane (negate X coordinates).
    pub fn mirror_x(&mut self) {
        self.apply_mirror(&Transform::from_mirror_x());
    }

    /// Mirror across the XZ plane (negate Y coordinates).
    pub fn mirror_y(&mut self) {
        self.apply_mirror(&Transform::from_mirror_y());
    }

    /// Mirror across the XY plane (negate Z coordinates).
    pub fn mirror_z(&mut self) {
        self.apply_mirror(&Transform::from_mirror_z());
    }

    /// Mirror across a line defined by two points (in the XY plane).
    pub fn mirror_about_line(&mut self, p1: Vector3, p2: Vector3) {
        self.mirror(p1, p2);
    }

    /// Planar MIRROR. Heights are preserved and the points' Z values are ignored.
    pub fn mirror(&mut self, first: Vector3, second: Vector3) {
        self.mirror_with_options(first, second, MirrorOptions::default());
    }

    /// Planar MIRROR with an explicit text policy.
    pub fn mirror_with_options(&mut self, first: Vector3, second: Vector3, options: MirrorOptions) {
        self.apply_mirror_with_options(&Transform::from_mirror_line(first, second), options);
    }

    /// Mirror across an arbitrary plane.
    pub fn mirror_about_plane(&mut self, point: Vector3, normal: Vector3) {
        self.mirror3d(super::mirror3d::MirrorPlane::PointNormal { point, normal });
    }
}

#[cfg(test)]
pub(crate) mod tests;
