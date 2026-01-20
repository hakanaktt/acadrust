//! Bounding box types for geometric entities

use super::{Vector2, Vector3};
use std::fmt;

/// 2D bounding box
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox2D {
    /// Minimum point (lower-left corner)
    pub min: Vector2,
    /// Maximum point (upper-right corner)
    pub max: Vector2,
}

impl BoundingBox2D {
    /// Create a new bounding box from min and max points
    pub fn new(min: Vector2, max: Vector2) -> Self {
        BoundingBox2D { min, max }
    }

    /// Create a bounding box from a single point
    pub fn from_point(point: Vector2) -> Self {
        BoundingBox2D {
            min: point,
            max: point,
        }
    }

    /// Create a bounding box that contains all given points
    pub fn from_points(points: &[Vector2]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        let mut min_x = points[0].x;
        let mut min_y = points[0].y;
        let mut max_x = points[0].x;
        let mut max_y = points[0].y;

        for point in points.iter().skip(1) {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }

        Some(BoundingBox2D {
            min: Vector2::new(min_x, min_y),
            max: Vector2::new(max_x, max_y),
        })
    }

    /// Get the width of the bounding box
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Get the height of the bounding box
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Get the center point of the bounding box
    pub fn center(&self) -> Vector2 {
        Vector2::new(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }

    /// Check if this bounding box contains a point
    pub fn contains(&self, point: Vector2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Expand the bounding box to include another point
    pub fn expand_to_include(&mut self, point: Vector2) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }

    /// Merge with another bounding box
    pub fn merge(&self, other: &BoundingBox2D) -> BoundingBox2D {
        BoundingBox2D {
            min: Vector2::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
            ),
            max: Vector2::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
            ),
        }
    }
}

impl fmt::Display for BoundingBox2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BBox2D[{} -> {}]", self.min, self.max)
    }
}

/// 3D bounding box
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox3D {
    /// Minimum point (lower-left-back corner)
    pub min: Vector3,
    /// Maximum point (upper-right-front corner)
    pub max: Vector3,
}

impl Default for BoundingBox3D {
    fn default() -> Self {
        BoundingBox3D {
            min: Vector3::new(0.0, 0.0, 0.0),
            max: Vector3::new(0.0, 0.0, 0.0),
        }
    }
}

impl BoundingBox3D {
    /// Create a new bounding box from min and max points
    pub fn new(min: Vector3, max: Vector3) -> Self {
        BoundingBox3D { min, max }
    }

    /// Create a bounding box from a single point
    pub fn from_point(point: Vector3) -> Self {
        BoundingBox3D {
            min: point,
            max: point,
        }
    }

    /// Create a bounding box that contains all given points
    pub fn from_points(points: &[Vector3]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        let mut min_x = points[0].x;
        let mut min_y = points[0].y;
        let mut min_z = points[0].z;
        let mut max_x = points[0].x;
        let mut max_y = points[0].y;
        let mut max_z = points[0].z;

        for point in points.iter().skip(1) {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            min_z = min_z.min(point.z);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
            max_z = max_z.max(point.z);
        }

        Some(BoundingBox3D {
            min: Vector3::new(min_x, min_y, min_z),
            max: Vector3::new(max_x, max_y, max_z),
        })
    }

    /// Get the width of the bounding box (X dimension)
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }

    /// Get the height of the bounding box (Y dimension)
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }

    /// Get the depth of the bounding box (Z dimension)
    pub fn depth(&self) -> f64 {
        self.max.z - self.min.z
    }

    /// Get the center point of the bounding box
    pub fn center(&self) -> Vector3 {
        Vector3::new(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
            (self.min.z + self.max.z) / 2.0,
        )
    }

    /// Check if this bounding box contains a point
    pub fn contains(&self, point: Vector3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Expand the bounding box to include another point
    pub fn expand_to_include(&mut self, point: Vector3) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    /// Merge with another bounding box
    pub fn merge(&self, other: &BoundingBox3D) -> BoundingBox3D {
        BoundingBox3D {
            min: Vector3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vector3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }
}

impl fmt::Display for BoundingBox3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BBox3D[{} -> {}]", self.min, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox2d_from_points() {
        let points = vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(10.0, 5.0),
            Vector2::new(-5.0, 3.0),
        ];
        let bbox = BoundingBox2D::from_points(&points).unwrap();
        assert_eq!(bbox.min, Vector2::new(-5.0, 0.0));
        assert_eq!(bbox.max, Vector2::new(10.0, 5.0));
    }

    #[test]
    fn test_bbox2d_dimensions() {
        let bbox = BoundingBox2D::new(Vector2::new(0.0, 0.0), Vector2::new(10.0, 5.0));
        assert_eq!(bbox.width(), 10.0);
        assert_eq!(bbox.height(), 5.0);
        assert_eq!(bbox.center(), Vector2::new(5.0, 2.5));
    }

    #[test]
    fn test_bbox2d_contains() {
        let bbox = BoundingBox2D::new(Vector2::new(0.0, 0.0), Vector2::new(10.0, 10.0));
        assert!(bbox.contains(Vector2::new(5.0, 5.0)));
        assert!(!bbox.contains(Vector2::new(15.0, 5.0)));
    }

    #[test]
    fn test_bbox3d_from_points() {
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 5.0, 3.0),
            Vector3::new(-5.0, 3.0, -2.0),
        ];
        let bbox = BoundingBox3D::from_points(&points).unwrap();
        assert_eq!(bbox.min, Vector3::new(-5.0, 0.0, -2.0));
        assert_eq!(bbox.max, Vector3::new(10.0, 5.0, 3.0));
    }

    #[test]
    fn test_bbox3d_dimensions() {
        let bbox = BoundingBox3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 5.0, 3.0),
        );
        assert_eq!(bbox.width(), 10.0);
        assert_eq!(bbox.height(), 5.0);
        assert_eq!(bbox.depth(), 3.0);
        assert_eq!(bbox.center(), Vector3::new(5.0, 2.5, 1.5));
    }
}

