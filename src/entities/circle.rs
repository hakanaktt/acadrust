//! Circle entity

use super::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transform, Transparency, Vector3};

/// A circle entity
#[derive(Debug, Clone)]
pub struct Circle {
    /// Common entity data
    pub common: EntityCommon,
    /// Center point of the circle
    pub center: Vector3,
    /// Radius of the circle
    pub radius: f64,
    /// Thickness (extrusion in Z direction)
    pub thickness: f64,
    /// Normal vector
    pub normal: Vector3,
}

impl Circle {
    /// Create a new circle at the origin with radius 1
    pub fn new() -> Self {
        Circle {
            common: EntityCommon::new(),
            center: Vector3::ZERO,
            radius: 1.0,
            thickness: 0.0,
            normal: Vector3::UNIT_Z,
        }
    }

    /// Create a new circle with center and radius
    pub fn from_center_radius(center: Vector3, radius: f64) -> Self {
        Circle {
            center,
            radius,
            ..Self::new()
        }
    }

    /// Create a new circle from coordinates and radius
    pub fn from_coords(x: f64, y: f64, z: f64, radius: f64) -> Self {
        Circle::from_center_radius(Vector3::new(x, y, z), radius)
    }

    /// Get the diameter of the circle
    pub fn diameter(&self) -> f64 {
        self.radius * 2.0
    }

    /// Get the circumference of the circle
    pub fn circumference(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius
    }

    /// Get the area of the circle
    pub fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }
}

impl Default for Circle {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Circle {
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
        "CIRCLE"
    }
    
    fn apply_transform(&mut self, transform: &Transform) {
        // Transform center point
        self.center = transform.apply(self.center);
        
        // For scaling, we need to extract the scale factor
        // Apply to a unit vector to determine scale
        let unit_x = Vector3::new(1.0, 0.0, 0.0);
        let transformed_unit = transform.apply_rotation(unit_x);
        let scale_factor = transformed_unit.length();
        
        // Scale the radius (uses uniform scaling assumption)
        self.radius *= scale_factor;
        
        // Transform the normal vector
        self.normal = transform.apply_rotation(self.normal).normalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle_creation() {
        let circle = Circle::new();
        assert_eq!(circle.center, Vector3::ZERO);
        assert_eq!(circle.radius, 1.0);
        assert_eq!(circle.entity_type(), "CIRCLE");
    }

    #[test]
    fn test_circle_from_center_radius() {
        let circle = Circle::from_center_radius(Vector3::new(5.0, 5.0, 0.0), 10.0);
        assert_eq!(circle.center, Vector3::new(5.0, 5.0, 0.0));
        assert_eq!(circle.radius, 10.0);
        assert_eq!(circle.diameter(), 20.0);
    }

    #[test]
    fn test_circle_area() {
        let circle = Circle::from_coords(0.0, 0.0, 0.0, 5.0);
        assert!((circle.area() - 78.53981633974483).abs() < 1e-10);
    }

    #[test]
    fn test_circle_circumference() {
        let circle = Circle::from_coords(0.0, 0.0, 0.0, 5.0);
        assert!((circle.circumference() - 31.41592653589793).abs() < 1e-10);
    }

    #[test]
    fn test_circle_translate() {
        let mut circle = Circle::from_coords(0.0, 0.0, 0.0, 5.0);
        circle.translate(Vector3::new(10.0, 20.0, 30.0));
        assert_eq!(circle.center, Vector3::new(10.0, 20.0, 30.0));
        assert_eq!(circle.radius, 5.0);
    }
}

