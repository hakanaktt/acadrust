//! Insert entity (block reference)

use crate::entities::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transparency, Vector3};

/// Insert entity - a reference to a block definition
///
/// An Insert entity places an instance of a block at a specified location
/// with optional scaling, rotation, and array properties.
#[derive(Debug, Clone, PartialEq)]
pub struct Insert {
    pub common: EntityCommon,
    /// Block name (references a BlockRecord)
    pub block_name: String,
    /// Insertion point (in WCS)
    pub insert_point: Vector3,
    /// X scale factor
    pub x_scale: f64,
    /// Y scale factor
    pub y_scale: f64,
    /// Z scale factor
    pub z_scale: f64,
    /// Rotation angle in radians
    pub rotation: f64,
    /// Normal vector (extrusion direction)
    pub normal: Vector3,
    /// Column count (for array inserts)
    pub column_count: u16,
    /// Row count (for array inserts)
    pub row_count: u16,
    /// Column spacing (for array inserts)
    pub column_spacing: f64,
    /// Row spacing (for array inserts)
    pub row_spacing: f64,
}

impl Insert {
    /// Create a new insert entity
    pub fn new(block_name: impl Into<String>, insert_point: Vector3) -> Self {
        Self {
            common: EntityCommon::default(),
            block_name: block_name.into(),
            insert_point,
            x_scale: 1.0,
            y_scale: 1.0,
            z_scale: 1.0,
            rotation: 0.0,
            normal: Vector3::new(0.0, 0.0, 1.0),
            column_count: 1,
            row_count: 1,
            column_spacing: 0.0,
            row_spacing: 0.0,
        }
    }

    /// Builder: Set the scale factors
    pub fn with_scale(mut self, x: f64, y: f64, z: f64) -> Self {
        self.x_scale = x;
        self.y_scale = y;
        self.z_scale = z;
        self
    }

    /// Builder: Set uniform scale
    pub fn with_uniform_scale(mut self, scale: f64) -> Self {
        self.x_scale = scale;
        self.y_scale = scale;
        self.z_scale = scale;
        self
    }

    /// Builder: Set the rotation angle
    pub fn with_rotation(mut self, angle: f64) -> Self {
        self.rotation = angle;
        self
    }

    /// Builder: Set the normal vector
    pub fn with_normal(mut self, normal: Vector3) -> Self {
        self.normal = normal;
        self
    }

    /// Builder: Set array properties
    pub fn with_array(mut self, columns: u16, rows: u16, col_spacing: f64, row_spacing: f64) -> Self {
        self.column_count = columns;
        self.row_count = rows;
        self.column_spacing = col_spacing;
        self.row_spacing = row_spacing;
        self
    }

    /// Check if this is an array insert
    pub fn is_array(&self) -> bool {
        self.column_count > 1 || self.row_count > 1
    }

    /// Get the total number of instances in the array
    pub fn instance_count(&self) -> usize {
        (self.column_count as usize) * (self.row_count as usize)
    }

    /// Get all insertion points for array instances
    pub fn array_points(&self) -> Vec<Vector3> {
        let mut points = Vec::with_capacity(self.instance_count());

        for row in 0..self.row_count {
            for col in 0..self.column_count {
                let offset_x = col as f64 * self.column_spacing;
                let offset_y = row as f64 * self.row_spacing;

                // Apply rotation to the offset
                let cos_r = self.rotation.cos();
                let sin_r = self.rotation.sin();
                let rotated_x = offset_x * cos_r - offset_y * sin_r;
                let rotated_y = offset_x * sin_r + offset_y * cos_r;

                let point = self.insert_point + Vector3::new(rotated_x, rotated_y, 0.0);
                points.push(point);
            }
        }

        points
    }

    /// Check if the insert has uniform scale
    pub fn has_uniform_scale(&self) -> bool {
        (self.x_scale - self.y_scale).abs() < 1e-10 && (self.y_scale - self.z_scale).abs() < 1e-10
    }

    /// Get the uniform scale factor (if uniform)
    pub fn uniform_scale(&self) -> Option<f64> {
        if self.has_uniform_scale() {
            Some(self.x_scale)
        } else {
            None
        }
    }
}

impl Entity for Insert {
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
        // For now, return a bounding box at the insertion point
        // In a full implementation, this would need to reference the block definition
        BoundingBox3D::from_point(self.insert_point)
    }

    fn translate(&mut self, offset: Vector3) {
        self.insert_point = self.insert_point + offset;
    }

    fn entity_type(&self) -> &'static str {
        "INSERT"
    }
    
    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        // Transform the insertion point
        self.insert_point = transform.apply(self.insert_point);
        
        // Extract scale factor from transform and apply to scale factors
        let unit_x = Vector3::new(1.0, 0.0, 0.0);
        let transformed_unit = transform.apply_rotation(unit_x);
        let scale_factor = transformed_unit.length();
        
        self.x_scale *= scale_factor;
        self.y_scale *= scale_factor;
        self.z_scale *= scale_factor;
        
        // Transform the normal vector
        self.normal = transform.apply_rotation(self.normal).normalize();
        
        // Note: rotation angle and array spacings may need adjustment for complex transforms
    }
}

