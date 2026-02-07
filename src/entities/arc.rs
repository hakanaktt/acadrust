//! Arc entity

use super::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transparency, Vector3};

/// An arc entity (portion of a circle)
#[derive(Debug, Clone)]
pub struct Arc {
    /// Common entity data
    pub common: EntityCommon,
    /// Center point of the arc
    pub center: Vector3,
    /// Radius of the arc
    pub radius: f64,
    /// Start angle in radians
    pub start_angle: f64,
    /// End angle in radians
    pub end_angle: f64,
    /// Thickness (extrusion in Z direction)
    pub thickness: f64,
    /// Normal vector
    pub normal: Vector3,
}

impl Arc {
    /// Create a new arc at the origin
    pub fn new() -> Self {
        Arc {
            common: EntityCommon::new(),
            center: Vector3::ZERO,
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::PI / 2.0, // 90 degrees
            thickness: 0.0,
            normal: Vector3::UNIT_Z,
        }
    }

    /// Create a new arc with center, radius, and angles
    pub fn from_center_radius_angles(
        center: Vector3,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> Self {
        Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..Self::new()
        }
    }

    /// Create a new arc from coordinates, radius, and angles
    pub fn from_coords(
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    ) -> Self {
        Arc::from_center_radius_angles(Vector3::new(x, y, z), radius, start_angle, end_angle)
    }

    /// Get the sweep angle (angular extent) in radians
    pub fn sweep_angle(&self) -> f64 {
        let mut sweep = self.end_angle - self.start_angle;
        if sweep < 0.0 {
            sweep += 2.0 * std::f64::consts::PI;
        }
        sweep
    }

    /// Get the arc length
    pub fn arc_length(&self) -> f64 {
        self.radius * self.sweep_angle()
    }

    /// Get the start point of the arc
    pub fn start_point(&self) -> Vector3 {
        Vector3::new(
            self.center.x + self.radius * self.start_angle.cos(),
            self.center.y + self.radius * self.start_angle.sin(),
            self.center.z,
        )
    }

    /// Get the end point of the arc
    pub fn end_point(&self) -> Vector3 {
        Vector3::new(
            self.center.x + self.radius * self.end_angle.cos(),
            self.center.y + self.radius * self.end_angle.sin(),
            self.center.z,
        )
    }

    /// Get the midpoint of the arc
    pub fn midpoint(&self) -> Vector3 {
        let mid_angle = self.start_angle + self.sweep_angle() / 2.0;
        Vector3::new(
            self.center.x + self.radius * mid_angle.cos(),
            self.center.y + self.radius * mid_angle.sin(),
            self.center.z,
        )
    }
}

impl Default for Arc {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Arc {
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
        // Simplified bounding box - full circle bounds
        // A proper implementation would calculate exact arc bounds
        BoundingBox3D::new(
            Vector3::new(
                self.center.x - self.radius,
                self.center.y - self.radius,
                self.center.z,
            ),
            Vector3::new(
                self.center.x + self.radius,
                self.center.y + self.radius,
                self.center.z,
            ),
        )
    }

    fn translate(&mut self, offset: Vector3) {
        self.center = self.center + offset;
    }

    fn entity_type(&self) -> &'static str {
        "ARC"
    }
    
    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        // Transform center point
        self.center = transform.apply(self.center);
        
        // Extract scale factor from transform
        let unit_x = Vector3::new(1.0, 0.0, 0.0);
        let transformed_unit = transform.apply_rotation(unit_x);
        let scale_factor = transformed_unit.length();
        
        // Scale the radius
        self.radius *= scale_factor;
        
        // Transform the normal vector
        self.normal = transform.apply_rotation(self.normal).normalize();
        
        // Note: start_angle and end_angle remain the same for rotation around normal
        // For general 3D transforms, angle recalculation would be needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_creation() {
        let arc = Arc::new();
        assert_eq!(arc.center, Vector3::ZERO);
        assert_eq!(arc.radius, 1.0);
        assert_eq!(arc.entity_type(), "ARC");
    }

    #[test]
    fn test_arc_sweep_angle() {
        let arc = Arc::from_coords(0.0, 0.0, 0.0, 5.0, 0.0, std::f64::consts::PI);
        assert!((arc.sweep_angle() - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_arc_length() {
        let arc = Arc::from_coords(0.0, 0.0, 0.0, 5.0, 0.0, std::f64::consts::PI);
        let expected = 5.0 * std::f64::consts::PI;
        assert!((arc.arc_length() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_arc_endpoints() {
        let arc = Arc::from_coords(0.0, 0.0, 0.0, 5.0, 0.0, std::f64::consts::PI / 2.0);
        let start = arc.start_point();
        let end = arc.end_point();
        assert!((start.x - 5.0).abs() < 1e-10);
        assert!((start.y - 0.0).abs() < 1e-10);
        assert!((end.x - 0.0).abs() < 1e-10);
        assert!((end.y - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_arc_translate() {
        let mut arc = Arc::from_coords(0.0, 0.0, 0.0, 5.0, 0.0, std::f64::consts::PI);
        arc.translate(Vector3::new(10.0, 20.0, 30.0));
        assert_eq!(arc.center, Vector3::new(10.0, 20.0, 30.0));
        assert_eq!(arc.radius, 5.0);
    }
}


