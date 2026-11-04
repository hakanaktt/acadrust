//! Transformation types for geometric operations
//!
//! Provides transform matrices and operations for rotating, scaling,
//! and translating CAD entities.

use crate::types::{Vector2, Vector3};
use std::ops::Mul;

/// 3x3 matrix for 2D transformations and rotations
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Matrix3 {
    /// Matrix elements stored in row-major order
    pub m: [[f64; 3]; 3],
}

impl Matrix3 {
    /// Create identity matrix
    pub fn identity() -> Self {
        Self {
            m: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Create zero matrix
    pub fn zero() -> Self {
        Self { m: [[0.0; 3]; 3] }
    }

    /// Create matrix from rows
    pub fn from_rows(row0: [f64; 3], row1: [f64; 3], row2: [f64; 3]) -> Self {
        Self {
            m: [row0, row1, row2],
        }
    }

    /// Create rotation matrix around Z axis
    pub fn rotation_z(angle: f64) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();
        Self {
            m: [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    /// Create scaling matrix
    pub fn scaling(sx: f64, sy: f64, sz: f64) -> Self {
        Self {
            m: [[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, sz]],
        }
    }

    /// Create arbitrary axis matrix for OCS to WCS conversion
    ///
    /// Implements the ACAD arbitrary axis algorithm
    pub fn arbitrary_axis(normal: Vector3) -> Self {
        const ARBITRARY_AXIS_THRESHOLD: f64 = 1.0 / 64.0;

        let normal = normal.normalize();

        // Choose reference axis (Ax) based on normal direction
        let ax = if normal.x.abs() < ARBITRARY_AXIS_THRESHOLD
            && normal.y.abs() < ARBITRARY_AXIS_THRESHOLD
        {
            Vector3::new(0.0, 1.0, 0.0) // Use Y axis
        } else {
            Vector3::new(0.0, 0.0, 1.0) // Use Z axis
        };

        // Calculate X direction (Ax × N normalized)
        let x_dir = ax.cross(&normal).normalize();

        // Calculate Y direction (N × X normalized)
        let y_dir = normal.cross(&x_dir).normalize();

        Self {
            m: [
                [x_dir.x, y_dir.x, normal.x],
                [x_dir.y, y_dir.y, normal.y],
                [x_dir.z, y_dir.z, normal.z],
            ],
        }
    }

    /// Transpose the matrix
    pub fn transpose(&self) -> Self {
        Self {
            m: [
                [self.m[0][0], self.m[1][0], self.m[2][0]],
                [self.m[0][1], self.m[1][1], self.m[2][1]],
                [self.m[0][2], self.m[1][2], self.m[2][2]],
            ],
        }
    }

    /// Calculate determinant
    pub fn determinant(&self) -> f64 {
        self.m[0][0] * (self.m[1][1] * self.m[2][2] - self.m[1][2] * self.m[2][1])
            - self.m[0][1] * (self.m[1][0] * self.m[2][2] - self.m[1][2] * self.m[2][0])
            + self.m[0][2] * (self.m[1][0] * self.m[2][1] - self.m[1][1] * self.m[2][0])
    }

    /// Invert the matrix (returns None if singular)
    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() < 1e-10 {
            return None;
        }

        let inv_det = 1.0 / det;

        Some(Self {
            m: [
                [
                    (self.m[1][1] * self.m[2][2] - self.m[1][2] * self.m[2][1]) * inv_det,
                    (self.m[0][2] * self.m[2][1] - self.m[0][1] * self.m[2][2]) * inv_det,
                    (self.m[0][1] * self.m[1][2] - self.m[0][2] * self.m[1][1]) * inv_det,
                ],
                [
                    (self.m[1][2] * self.m[2][0] - self.m[1][0] * self.m[2][2]) * inv_det,
                    (self.m[0][0] * self.m[2][2] - self.m[0][2] * self.m[2][0]) * inv_det,
                    (self.m[0][2] * self.m[1][0] - self.m[0][0] * self.m[1][2]) * inv_det,
                ],
                [
                    (self.m[1][0] * self.m[2][1] - self.m[1][1] * self.m[2][0]) * inv_det,
                    (self.m[0][1] * self.m[2][0] - self.m[0][0] * self.m[2][1]) * inv_det,
                    (self.m[0][0] * self.m[1][1] - self.m[0][1] * self.m[1][0]) * inv_det,
                ],
            ],
        })
    }

    /// Transform a Vector3
    pub fn transform_point(&self, v: Vector3) -> Vector3 {
        Vector3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }
}

impl Mul for Matrix3 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut result = Self::zero();
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    result.m[i][j] += self.m[i][k] * rhs.m[k][j];
                }
            }
        }
        result
    }
}

impl Mul<Vector3> for Matrix3 {
    type Output = Vector3;

    fn mul(self, v: Vector3) -> Self::Output {
        self.transform_point(v)
    }
}

impl Default for Matrix3 {
    fn default() -> Self {
        Self::identity()
    }
}

/// 4x4 transformation matrix for 3D operations
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Matrix4 {
    /// Matrix elements stored in row-major order
    pub m: [[f64; 4]; 4],
}

impl Matrix4 {
    /// Create identity matrix
    pub fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create zero matrix
    pub fn zero() -> Self {
        Self { m: [[0.0; 4]; 4] }
    }

    /// Create translation matrix
    pub fn translation(tx: f64, ty: f64, tz: f64) -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, tx],
                [0.0, 1.0, 0.0, ty],
                [0.0, 0.0, 1.0, tz],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create scaling matrix
    pub fn scaling(sx: f64, sy: f64, sz: f64) -> Self {
        Self {
            m: [
                [sx, 0.0, 0.0, 0.0],
                [0.0, sy, 0.0, 0.0],
                [0.0, 0.0, sz, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create scaling matrix with origin
    pub fn scaling_with_origin(sx: f64, sy: f64, sz: f64, origin: Vector3) -> Self {
        // Translate to origin, scale, translate back
        Self::translation(origin.x, origin.y, origin.z)
            * Self::scaling(sx, sy, sz)
            * Self::translation(-origin.x, -origin.y, -origin.z)
    }

    /// Create rotation matrix around X axis
    pub fn rotation_x(angle: f64) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, cos, -sin, 0.0],
                [0.0, sin, cos, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create rotation matrix around Y axis
    pub fn rotation_y(angle: f64) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();
        Self {
            m: [
                [cos, 0.0, sin, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [-sin, 0.0, cos, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create rotation matrix around Z axis
    pub fn rotation_z(angle: f64) -> Self {
        let cos = angle.cos();
        let sin = angle.sin();
        Self {
            m: [
                [cos, -sin, 0.0, 0.0],
                [sin, cos, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create rotation matrix around arbitrary axis (Rodrigues' rotation formula)
    pub fn rotation(axis: Vector3, angle: f64) -> Self {
        let axis = axis.normalize();
        let cos = angle.cos();
        let sin = angle.sin();
        let one_minus_cos = 1.0 - cos;

        let x = axis.x;
        let y = axis.y;
        let z = axis.z;

        Self {
            m: [
                [
                    cos + x * x * one_minus_cos,
                    x * y * one_minus_cos - z * sin,
                    x * z * one_minus_cos + y * sin,
                    0.0,
                ],
                [
                    y * x * one_minus_cos + z * sin,
                    cos + y * y * one_minus_cos,
                    y * z * one_minus_cos - x * sin,
                    0.0,
                ],
                [
                    z * x * one_minus_cos - y * sin,
                    z * y * one_minus_cos + x * sin,
                    cos + z * z * one_minus_cos,
                    0.0,
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Create matrix from Matrix3 (3x3 rotation/scale portion)
    pub fn from_matrix3(m3: Matrix3) -> Self {
        Self {
            m: [
                [m3.m[0][0], m3.m[0][1], m3.m[0][2], 0.0],
                [m3.m[1][0], m3.m[1][1], m3.m[1][2], 0.0],
                [m3.m[2][0], m3.m[2][1], m3.m[2][2], 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Get the 3x3 rotation/scale portion
    pub fn to_matrix3(&self) -> Matrix3 {
        Matrix3 {
            m: [
                [self.m[0][0], self.m[0][1], self.m[0][2]],
                [self.m[1][0], self.m[1][1], self.m[1][2]],
                [self.m[2][0], self.m[2][1], self.m[2][2]],
            ],
        }
    }

    /// Transpose the matrix
    pub fn transpose(&self) -> Self {
        let mut result = Self::zero();
        for i in 0..4 {
            for j in 0..4 {
                result.m[i][j] = self.m[j][i];
            }
        }
        result
    }

    /// Transform a point (applies full transformation including translation)
    pub fn transform_point(&self, v: Vector3) -> Vector3 {
        let w = self.m[3][0] * v.x + self.m[3][1] * v.y + self.m[3][2] * v.z + self.m[3][3];
        let w = if w.abs() < 1e-10 { 1.0 } else { w };

        Vector3::new(
            (self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z + self.m[0][3]) / w,
            (self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z + self.m[1][3]) / w,
            (self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z + self.m[2][3]) / w,
        )
    }

    /// Transform a direction vector (ignores translation)
    pub fn transform_direction(&self, v: Vector3) -> Vector3 {
        Vector3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }
}

impl Mul for Matrix4 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut result = Self::zero();
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result.m[i][j] += self.m[i][k] * rhs.m[k][j];
                }
            }
        }
        result
    }
}

impl Default for Matrix4 {
    fn default() -> Self {
        Self::identity()
    }
}

/// Transform structure combining rotation, scaling, and translation
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transform {
    /// The 4x4 transformation matrix
    pub matrix: Matrix4,
}

impl Transform {
    /// Create identity transform
    pub fn identity() -> Self {
        Self {
            matrix: Matrix4::identity(),
        }
    }

    /// Create transform from matrix
    pub fn from_matrix(matrix: Matrix4) -> Self {
        Self { matrix }
    }

    /// Create rotation transform around arbitrary axis
    pub fn from_rotation(axis: Vector3, angle: f64) -> Self {
        Self {
            matrix: Matrix4::rotation(axis, angle),
        }
    }

    /// Create a rotation around an axis through `origin`.
    pub fn from_rotation_with_origin(axis: Vector3, angle: f64, origin: Vector3) -> Self {
        if !axis.x.is_finite()
            || !axis.y.is_finite()
            || !axis.z.is_finite()
            || !angle.is_finite()
            || !origin.x.is_finite()
            || !origin.y.is_finite()
            || !origin.z.is_finite()
            || axis.length_squared() <= f64::EPSILON
        {
            return Self::identity();
        }
        Self::from_translation(-origin)
            .then(&Self::from_rotation(axis, angle))
            .then(&Self::from_translation(origin))
    }

    /// Create translation transform
    pub fn from_translation(translation: Vector3) -> Self {
        Self {
            matrix: Matrix4::translation(translation.x, translation.y, translation.z),
        }
    }

    /// Create uniform scaling transform
    pub fn from_scale(scale: f64) -> Self {
        Self {
            matrix: Matrix4::scaling(scale, scale, scale),
        }
    }

    /// Create non-uniform scaling transform
    pub fn from_scaling(scale: Vector3) -> Self {
        Self {
            matrix: Matrix4::scaling(scale.x, scale.y, scale.z),
        }
    }

    /// Create scaling transform with origin
    pub fn from_scaling_with_origin(scale: Vector3, origin: Vector3) -> Self {
        Self {
            matrix: Matrix4::scaling_with_origin(scale.x, scale.y, scale.z, origin),
        }
    }

    /// Apply transform to a point
    pub fn apply(&self, point: Vector3) -> Vector3 {
        self.matrix.transform_point(point)
    }

    /// Apply only the rotation portion
    pub fn apply_rotation(&self, direction: Vector3) -> Vector3 {
        self.matrix.transform_direction(direction)
    }

    /// Combine with another transform (this transform applied first)
    pub fn then(&self, other: &Transform) -> Transform {
        Transform {
            matrix: other.matrix * self.matrix,
        }
    }

    /// Combine with another transform (other transform applied first)
    pub fn compose(&self, other: &Transform) -> Transform {
        Transform {
            matrix: self.matrix * other.matrix,
        }
    }

    /// Create a mirror transform across the YZ plane (negate X)
    pub fn from_mirror_x() -> Self {
        Self {
            matrix: Matrix4::scaling(-1.0, 1.0, 1.0),
        }
    }

    /// Create a mirror transform across the XZ plane (negate Y)
    pub fn from_mirror_y() -> Self {
        Self {
            matrix: Matrix4::scaling(1.0, -1.0, 1.0),
        }
    }

    /// Create a mirror transform across the XY plane (negate Z)
    pub fn from_mirror_z() -> Self {
        Self {
            matrix: Matrix4::scaling(1.0, 1.0, -1.0),
        }
    }

    /// Create a mirror transform across an arbitrary plane defined by a point and normal
    ///
    /// The plane passes through `point` with the given `normal` direction.
    /// Uses the Householder reflection: P' = P - 2(n·P + d)n.
    /// A zero normal, non-finite input, or unrepresentable translation
    /// produces the identity transform.
    pub fn from_mirror_plane(point: Vector3, normal: Vector3) -> Self {
        if ![point.x, point.y, point.z, normal.x, normal.y, normal.z]
            .iter()
            .all(|value| value.is_finite())
        {
            return Self::identity();
        }
        // Scale before normalizing to avoid overflow and underflow in the
        // squared length, including subnormal but nonzero normals.
        let magnitude = normal.x.abs().max(normal.y.abs()).max(normal.z.abs());
        if magnitude == 0.0 {
            return Self::identity();
        }
        let scaled = Vector3::new(
            normal.x / magnitude,
            normal.y / magnitude,
            normal.z / magnitude,
        );
        let n = scaled * (1.0 / scaled.length());
        let point_scale = point.x.abs().max(point.y.abs()).max(point.z.abs());
        let projection = if point_scale > 0.0 {
            n.x * (point.x / point_scale)
                + n.y * (point.y / point_scale)
                + n.z * (point.z / point_scale)
        } else {
            0.0
        };
        let translation = Vector3::new(
            (2.0 * n.x * projection) * point_scale,
            (2.0 * n.y * projection) * point_scale,
            (2.0 * n.z * projection) * point_scale,
        );
        if ![translation.x, translation.y, translation.z]
            .iter()
            .all(|value| value.is_finite())
        {
            return Self::identity();
        }

        Self {
            matrix: Matrix4 {
                m: [
                    [
                        1.0 - 2.0 * n.x * n.x,
                        -2.0 * n.x * n.y,
                        -2.0 * n.x * n.z,
                        translation.x,
                    ],
                    [
                        -2.0 * n.y * n.x,
                        1.0 - 2.0 * n.y * n.y,
                        -2.0 * n.y * n.z,
                        translation.y,
                    ],
                    [
                        -2.0 * n.z * n.x,
                        -2.0 * n.z * n.y,
                        1.0 - 2.0 * n.z * n.z,
                        translation.z,
                    ],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            },
        }
    }

    /// Create a mirror transform across a line defined by two points (in the XY plane)
    ///
    /// The mirror line passes through the XY projections of `p1` and `p2`;
    /// reflected points retain their Z coordinate. A zero XY direction or
    /// non-finite input produces the identity transform.
    pub fn from_mirror_line(p1: Vector3, p2: Vector3) -> Self {
        if ![p1.x, p1.y, p1.z, p2.x, p2.y, p2.z]
            .iter()
            .all(|value| value.is_finite())
        {
            return Self::identity();
        }
        let mut dx = p2.x - p1.x;
        let mut dy = p2.y - p1.y;
        if !dx.is_finite() || !dy.is_finite() {
            let magnitude = p1.x.abs().max(p1.y.abs()).max(p2.x.abs()).max(p2.y.abs());
            dx = p2.x / magnitude - p1.x / magnitude;
            dy = p2.y / magnitude - p1.y / magnitude;
        }
        Self::from_mirror_plane(Vector3::new(p1.x, p1.y, 0.0), Vector3::new(-dy, dx, 0.0))
    }

    /// Check if transform is identity
    pub fn is_identity(&self) -> bool {
        *self == Self::identity()
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

impl Mul for Transform {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        self.compose(&rhs)
    }
}

/// Helper for 2D rotation (utility function)
pub fn rotate_point_2d(point: Vector2, center: Vector2, angle: f64) -> Vector2 {
    let cos = angle.cos();
    let sin = angle.sin();
    let dx = point.x - center.x;
    let dy = point.y - center.y;
    Vector2::new(
        center.x + dx * cos - dy * sin,
        center.y + dx * sin + dy * cos,
    )
}

/// Helper to check if angle is effectively zero
pub fn is_zero_angle(angle: f64) -> bool {
    angle.abs() < 1e-10
}

/// Normalize an angle to the range [0, 2π)
pub fn normalize_angle(angle: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    let mut a = angle % two_pi;
    if a < 0.0 {
        a += two_pi;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_matrix3_identity() {
        let m = Matrix3::identity();
        let v = Vector3::new(1.0, 2.0, 3.0);
        let result = m * v;
        assert!((result.x - v.x).abs() < 1e-10);
        assert!((result.y - v.y).abs() < 1e-10);
        assert!((result.z - v.z).abs() < 1e-10);
    }

    #[test]
    fn test_matrix3_rotation_z() {
        let m = Matrix3::rotation_z(PI / 2.0);
        let v = Vector3::new(1.0, 0.0, 0.0);
        let result = m * v;
        assert!(result.x.abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix4_translation() {
        let m = Matrix4::translation(1.0, 2.0, 3.0);
        let v = Vector3::new(0.0, 0.0, 0.0);
        let result = m.transform_point(v);
        assert!((result.x - 1.0).abs() < 1e-10);
        assert!((result.y - 2.0).abs() < 1e-10);
        assert!((result.z - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix4_rotation() {
        let m = Matrix4::rotation(Vector3::new(0.0, 0.0, 1.0), PI / 2.0);
        let v = Vector3::new(1.0, 0.0, 0.0);
        let result = m.transform_point(v);
        assert!(result.x.abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix4_rotation_with_origin() {
        let transform = Transform::from_rotation_with_origin(
            Vector3::UNIT_Z,
            PI / 2.0,
            Vector3::new(10.0, 10.0, 0.0),
        );
        let result = transform.apply(Vector3::new(10.0, 0.0, 0.0));
        assert!((result.x - 20.0).abs() < 1e-10);
        assert!((result.y - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_transform_composition() {
        let t1 = Transform::from_translation(Vector3::new(1.0, 0.0, 0.0));
        let t2 = Transform::from_translation(Vector3::new(0.0, 1.0, 0.0));
        let combined = t1.then(&t2);

        let origin = Vector3::new(0.0, 0.0, 0.0);
        let result = combined.apply(origin);

        assert!((result.x - 1.0).abs() < 1e-10);
        assert!((result.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_arbitrary_axis() {
        // Test with standard Z normal
        let m = Matrix3::arbitrary_axis(Vector3::new(0.0, 0.0, 1.0));
        let det = m.determinant();
        assert!((det - 1.0).abs() < 1e-10); // Should be orthonormal
    }

    #[test]
    fn test_mirror_x() {
        let t = Transform::from_mirror_x();
        let p = Vector3::new(3.0, 5.0, 7.0);
        let r = t.apply(p);
        assert!((r.x - (-3.0)).abs() < 1e-10);
        assert!((r.y - 5.0).abs() < 1e-10);
        assert!((r.z - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_y() {
        let t = Transform::from_mirror_y();
        let p = Vector3::new(3.0, 5.0, 7.0);
        let r = t.apply(p);
        assert!((r.x - 3.0).abs() < 1e-10);
        assert!((r.y - (-5.0)).abs() < 1e-10);
        assert!((r.z - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_z() {
        let t = Transform::from_mirror_z();
        let p = Vector3::new(3.0, 5.0, 7.0);
        let r = t.apply(p);
        assert!((r.x - 3.0).abs() < 1e-10);
        assert!((r.y - 5.0).abs() < 1e-10);
        assert!((r.z - (-7.0)).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_plane_through_origin() {
        // Mirror across YZ plane through origin = same as mirror_x
        let t = Transform::from_mirror_plane(Vector3::ZERO, Vector3::UNIT_X);
        let p = Vector3::new(3.0, 5.0, 7.0);
        let r = t.apply(p);
        assert!((r.x - (-3.0)).abs() < 1e-10);
        assert!((r.y - 5.0).abs() < 1e-10);
        assert!((r.z - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_plane_offset() {
        // Mirror across plane x=2 (normal X, point (2,0,0))
        let t = Transform::from_mirror_plane(Vector3::new(2.0, 0.0, 0.0), Vector3::UNIT_X);
        let p = Vector3::new(5.0, 3.0, 1.0);
        let r = t.apply(p);
        assert!((r.x - (-1.0)).abs() < 1e-10); // 2 - (5-2) = -1
        assert!((r.y - 3.0).abs() < 1e-10);
        assert!((r.z - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_line() {
        // Mirror across the X axis (line from (0,0,0) to (1,0,0))
        let t =
            Transform::from_mirror_line(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        let p = Vector3::new(3.0, 5.0, 0.0);
        let r = t.apply(p);
        assert!((r.x - 3.0).abs() < 1e-10);
        assert!((r.y - (-5.0)).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_line_diagonal() {
        // Mirror across the line y=x (45-degree line through origin)
        let t = Transform::from_mirror_line(Vector3::ZERO, Vector3::new(1.0, 1.0, 0.0));
        let p = Vector3::new(3.0, 0.0, 0.0);
        let r = t.apply(p);
        // Point (3,0) mirrored across y=x should give (0,3)
        assert!((r.x - 0.0).abs() < 1e-10);
        assert!((r.y - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_is_involution() {
        // Applying a mirror twice should return the original point
        let t =
            Transform::from_mirror_plane(Vector3::new(1.0, 2.0, 3.0), Vector3::new(1.0, 1.0, 0.0));
        let p = Vector3::new(7.0, -2.0, 4.0);
        let r = t.apply(t.apply(p));
        assert!((r.x - p.x).abs() < 1e-10);
        assert!((r.y - p.y).abs() < 1e-10);
        assert!((r.z - p.z).abs() < 1e-10);
    }

    #[test]
    fn test_mirror_plane_normal_magnitude_does_not_change_reflection() {
        let point = Vector3::new(2.0, -3.0, 5.0);
        let sample = Vector3::new(-4.0, 8.0, 1.0);
        let normal = Vector3::new(1.0, 2.0, -1.0);
        let expected = Transform::from_mirror_plane(point, normal).apply(sample);
        for magnitude in [1e-307, 1e307, f64::from_bits(1)] {
            let mirror = Transform::from_mirror_plane(point, normal * magnitude);
            let reflected = mirror.apply(sample);
            assert!((reflected - expected).length() < 1e-12);
            assert!((mirror.apply(reflected) - sample).length() < 1e-12);
            assert!((mirror.apply(point) - point).length() < 1e-12);
        }
    }

    #[test]
    fn test_mirror_plane_invalid_inputs_are_identity() {
        assert!(Transform::from_mirror_plane(Vector3::ZERO, Vector3::ZERO).is_identity());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                Transform::from_mirror_plane(Vector3::new(invalid, 1.0, 2.0), Vector3::UNIT_X,)
                    .is_identity()
            );
            assert!(
                Transform::from_mirror_plane(Vector3::ZERO, Vector3::new(1.0, invalid, 0.0),)
                    .is_identity()
            );
        }
        assert!(
            Transform::from_mirror_plane(Vector3::new(f64::MAX, 0.0, 0.0), Vector3::UNIT_X,)
                .is_identity()
        );
    }

    #[test]
    fn test_mirror_line_uses_xy_projection_and_preserves_z() {
        let mirror = Transform::from_mirror_line(
            Vector3::new(2.0, 3.0, -1e300),
            Vector3::new(3.0, 4.0, 1e300),
        );
        let reflected = mirror.apply(Vector3::new(5.0, 4.0, 7.0));
        assert!((reflected - Vector3::new(3.0, 6.0, 7.0)).length() < 1e-12);
        let vertical =
            Transform::from_mirror_line(Vector3::new(2.0, 3.0, 1.0), Vector3::new(2.0, 3.0, 8.0));
        assert!(vertical.is_identity());
    }

    #[test]
    fn test_mirror_line_handles_extreme_finite_directions() {
        for end in [
            Vector3::new(1e-300, 1e-300, 0.0),
            Vector3::new(1e300, 1e300, 0.0),
        ] {
            let mirror = Transform::from_mirror_line(Vector3::ZERO, end);
            let reflected = mirror.apply(Vector3::new(3.0, 5.0, 7.0));
            assert!((reflected - Vector3::new(5.0, 3.0, 7.0)).length() < 1e-12);
        }
        let mirror = Transform::from_mirror_line(
            Vector3::new(f64::MAX, f64::MAX, 0.0),
            Vector3::new(-f64::MAX, -f64::MAX, 0.0),
        );
        assert!(
            (mirror.apply(Vector3::new(3.0, 5.0, 7.0)) - Vector3::new(5.0, 3.0, 7.0)).length()
                < 1e-12
        );
        assert!(
            Transform::from_mirror_line(Vector3::ZERO, Vector3::new(f64::NAN, 1.0, 0.0))
                .is_identity()
        );
    }

    #[test]
    fn test_normalize_angle() {
        assert!((normalize_angle(0.0)).abs() < 1e-10);
        assert!((normalize_angle(PI) - PI).abs() < 1e-10);
        assert!((normalize_angle(-PI / 2.0) - 3.0 * PI / 2.0).abs() < 1e-10);
        assert!((normalize_angle(3.0 * PI) - PI).abs() < 1e-10);
    }

    #[test]
    fn test_is_zero_angle() {
        assert!(is_zero_angle(0.0));
        assert!(is_zero_angle(1e-11));
        assert!(is_zero_angle(-1e-11));
        assert!(!is_zero_angle(1e-9));
        assert!(!is_zero_angle(-1e-9));
        assert!(!is_zero_angle(PI));
    }

    #[test]
    fn test_rotate_point_2d() {
        let origin = Vector2::ZERO;

        // Zero rotation returns the same point
        let p = Vector2::new(3.0, 4.0);
        let r0 = rotate_point_2d(p, origin, 0.0);
        assert!((r0.x - p.x).abs() < 1e-10);
        assert!((r0.y - p.y).abs() < 1e-10);

        // 90 degrees CCW rotation around origin: (1, 0) -> (0, 1)
        let r90 = rotate_point_2d(Vector2::UNIT_X, origin, PI / 2.0);
        assert!(r90.x.abs() < 1e-10);
        assert!((r90.y - 1.0).abs() < 1e-10);

        // 180 degrees rotation around origin: (1, 0) -> (-1, 0)
        let r180 = rotate_point_2d(Vector2::UNIT_X, origin, PI);
        assert!((r180.x - (-1.0)).abs() < 1e-10);
        assert!(r180.y.abs() < 1e-10);

        // 270 degrees rotation around origin: (1, 0) -> (0, -1)
        let r270 = rotate_point_2d(Vector2::UNIT_X, origin, 3.0 * PI / 2.0);
        assert!(r270.x.abs() < 1e-10);
        assert!((r270.y - (-1.0)).abs() < 1e-10);

        // 360 degrees full rotation returns to initial point
        let r360 = rotate_point_2d(p, origin, 2.0 * PI);
        assert!((r360.x - p.x).abs() < 1e-10);
        assert!((r360.y - p.y).abs() < 1e-10);

        // Rotation around an arbitrary pivot point:
        // Pivot at (2, 3), point at (5, 3) (offset +3 along X).
        // Rotated 90 degrees CCW should be at (2, 6) (offset +3 along Y).
        let pivot = Vector2::new(2.0, 3.0);
        let pt = Vector2::new(5.0, 3.0);
        let rotated = rotate_point_2d(pt, pivot, PI / 2.0);
        assert!((rotated.x - 2.0).abs() < 1e-10);
        assert!((rotated.y - 6.0).abs() < 1e-10);

        // Rotating the pivot point itself should leave it unchanged
        let rotated_pivot = rotate_point_2d(pivot, pivot, 1.234);
        assert!((rotated_pivot.x - pivot.x).abs() < 1e-10);
        assert!((rotated_pivot.y - pivot.y).abs() < 1e-10);

        // Distance from center is preserved
        let arbitrary_pt = Vector2::new(7.0, -11.0);
        let arbitrary_center = Vector2::new(-3.0, 5.0);
        let res = rotate_point_2d(arbitrary_pt, arbitrary_center, 0.75);
        let orig_dist = (arbitrary_pt - arbitrary_center).length();
        let new_dist = (res - arbitrary_center).length();
        assert!((orig_dist - new_dist).abs() < 1e-10);
    }
}
