//! Dimension entity types

use crate::entities::EntityCommon;
use crate::types::Vector3;

/// Dimension type flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimensionType {
    /// Rotated, horizontal, or vertical linear dimension
    Linear = 0,
    /// Aligned dimension
    Aligned = 1,
    /// Angular 2 lines dimension
    Angular = 2,
    /// Diameter dimension
    Diameter = 3,
    /// Radius dimension
    Radius = 4,
    /// Angular 3 points dimension
    Angular3Point = 5,
    /// Ordinate dimension
    Ordinate = 6,
}

/// Attachment point type for dimension text
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentPointType {
    TopLeft = 1,
    TopCenter = 2,
    TopRight = 3,
    MiddleLeft = 4,
    MiddleCenter = 5,
    MiddleRight = 6,
    BottomLeft = 7,
    BottomCenter = 8,
    BottomRight = 9,
}

/// Base dimension entity
/// 
/// All dimension types share common properties and behavior.
/// Specific dimension types extend this base with additional properties.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionBase {
    pub common: EntityCommon,
    /// Definition point for the dimension line (in WCS)
    pub definition_point: Vector3,
    /// Middle point of dimension text (in WCS)
    pub text_middle_point: Vector3,
    /// Insertion point for clones of a dimension (in OCS)
    pub insertion_point: Vector3,
    /// Dimension type
    pub dimension_type: DimensionType,
    /// Attachment point
    pub attachment_point: AttachmentPointType,
    /// Dimension text explicitly entered by the user
    pub text: String,
    /// User text override (alternative to text field)
    pub user_text: Option<String>,
    /// Normal vector (extrusion direction)
    pub normal: Vector3,
    /// Rotation angle of dimension text
    pub text_rotation: f64,
    /// Horizontal direction for the dimension entity
    pub horizontal_direction: f64,
    /// Dimension style name
    pub style_name: String,
    /// Actual measurement (computed)
    pub actual_measurement: f64,
    /// Version number
    pub version: u8,
    /// Block name that contains the dimension geometry
    pub block_name: String,
    /// Line spacing factor
    pub line_spacing_factor: f64,
}

impl DimensionBase {
    /// Create a new dimension base
    pub fn new(dim_type: DimensionType) -> Self {
        Self {
            common: EntityCommon::default(),
            definition_point: Vector3::new(0.0, 0.0, 0.0),
            text_middle_point: Vector3::new(0.0, 0.0, 0.0),
            insertion_point: Vector3::new(0.0, 0.0, 0.0),
            dimension_type: dim_type,
            attachment_point: AttachmentPointType::MiddleCenter,
            text: String::new(),
            user_text: None,
            normal: Vector3::new(0.0, 0.0, 1.0),
            text_rotation: 0.0,
            horizontal_direction: 0.0,
            style_name: "Standard".to_string(),
            actual_measurement: 0.0,
            version: 0,
            block_name: String::new(),
            line_spacing_factor: 1.0,
        }
    }

    /// Builder: Set the text override
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    /// Builder: Set the style name
    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        self.style_name = style.into();
        self
    }

    /// Builder: Set the normal vector
    pub fn with_normal(mut self, normal: Vector3) -> Self {
        self.normal = normal;
        self
    }

    /// Check if this is an angular dimension
    pub fn is_angular(&self) -> bool {
        matches!(
            self.dimension_type,
            DimensionType::Angular | DimensionType::Angular3Point
        )
    }
}

impl Default for DimensionBase {
    fn default() -> Self {
        Self::new(DimensionType::Linear)
    }
}

/// Aligned dimension entity
/// 
/// Measures the distance between two points along a line parallel to those points.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionAligned {
    pub base: DimensionBase,
    /// First definition point (in WCS)
    pub first_point: Vector3,
    /// Second definition point (in WCS)
    pub second_point: Vector3,
    /// Definition point on dimension line
    pub definition_point: Vector3,
    /// Extension line rotation (optional)
    pub ext_line_rotation: f64,
}

impl DimensionAligned {
    /// Create a new aligned dimension
    pub fn new(first_point: Vector3, second_point: Vector3) -> Self {
        let mut base = DimensionBase::new(DimensionType::Aligned);

        // Calculate measurement
        base.actual_measurement = first_point.distance(&second_point);

        Self {
            base,
            first_point,
            second_point,
            definition_point: Vector3::ZERO,
            ext_line_rotation: 0.0,
        }
    }

    /// Get the measurement value
    pub fn measurement(&self) -> f64 {
        self.first_point.distance(&self.second_point)
    }

    /// Set the offset distance from the second point
    pub fn set_offset(&mut self, offset: f64) {
        let dir = self.second_point - self.first_point;
        let perpendicular = Vector3::new(-dir.y, dir.x, 0.0).normalize();
        self.definition_point = self.second_point + perpendicular * offset;
    }

    /// Get the offset distance
    pub fn offset(&self) -> f64 {
        self.second_point.distance(&self.definition_point)
    }
}

impl Default for DimensionAligned {
    fn default() -> Self {
        Self {
            base: DimensionBase::new(DimensionType::Aligned),
            first_point: Vector3::ZERO,
            second_point: Vector3::ZERO,
            definition_point: Vector3::ZERO,
            ext_line_rotation: 0.0,
        }
    }
}

/// Linear dimension entity
///
/// Measures the horizontal or vertical distance between two points,
/// or the distance along a rotated axis.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionLinear {
    pub base: DimensionBase,
    /// First definition point (in WCS)
    pub first_point: Vector3,
    /// Second definition point (in WCS)
    pub second_point: Vector3,
    /// Definition point on dimension line
    pub definition_point: Vector3,
    /// Rotation angle of the dimension line
    pub rotation: f64,
    /// Extension line rotation
    pub ext_line_rotation: f64,
}

impl DimensionLinear {
    /// Create a new linear dimension
    pub fn new(first_point: Vector3, second_point: Vector3) -> Self {
        let mut base = DimensionBase::new(DimensionType::Linear);
        base.actual_measurement = first_point.distance(&second_point);
        
        Self {
            base,
            first_point,
            second_point,
            definition_point: Vector3::ZERO,
            rotation: 0.0,
            ext_line_rotation: 0.0,
        }
    }

    /// Create a horizontal linear dimension
    pub fn horizontal(first_point: Vector3, second_point: Vector3) -> Self {
        Self::new(first_point, second_point)
    }

    /// Create a vertical linear dimension
    pub fn vertical(first_point: Vector3, second_point: Vector3) -> Self {
        let mut dim = Self::new(first_point, second_point);
        dim.rotation = std::f64::consts::FRAC_PI_2; // 90 degrees
        dim
    }

    /// Create a rotated linear dimension
    pub fn rotated(first_point: Vector3, second_point: Vector3, angle: f64) -> Self {
        let mut dim = Self::new(first_point, second_point);
        dim.rotation = angle;
        dim
    }

    /// Get the measurement value (projected onto rotation axis)
    pub fn measurement(&self) -> f64 {
        let angle_vec = Vector3::new(self.rotation.cos(), self.rotation.sin(), 0.0);
        let diff = self.second_point - self.first_point;
        let normalized = diff.normalize();
        let dot = angle_vec.dot(&normalized).abs();
        self.first_point.distance(&self.second_point) * dot
    }

    /// Set the offset distance
    pub fn set_offset(&mut self, offset: f64) {
        let axis_y = Vector3::new(-self.rotation.sin(), self.rotation.cos(), 0.0);
        self.definition_point = self.second_point + axis_y * offset;
    }
}

impl Default for DimensionLinear {
    fn default() -> Self {
        Self {
            base: DimensionBase::new(DimensionType::Linear),
            first_point: Vector3::ZERO,
            second_point: Vector3::ZERO,
            definition_point: Vector3::ZERO,
            rotation: 0.0,
            ext_line_rotation: 0.0,
        }
    }
}

/// Radius dimension entity
///
/// Measures the radius of a circle or arc.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionRadius {
    pub base: DimensionBase,
    /// Definition point (point on arc/circle) - in WCS
    pub definition_point: Vector3,
    /// Center point of the arc/circle (in WCS)
    pub angle_vertex: Vector3,
    /// Leader length
    pub leader_length: f64,
}

impl DimensionRadius {
    /// Create a new radius dimension
    pub fn new(center: Vector3, point_on_arc: Vector3) -> Self {
        let mut base = DimensionBase::new(DimensionType::Radius);
        base.actual_measurement = center.distance(&point_on_arc);

        Self {
            base,
            definition_point: point_on_arc,
            angle_vertex: center,
            leader_length: 0.0,
        }
    }

    /// Get the radius measurement
    pub fn measurement(&self) -> f64 {
        self.definition_point.distance(&self.angle_vertex)
    }
}

impl Default for DimensionRadius {
    fn default() -> Self {
        Self {
            base: DimensionBase::new(DimensionType::Radius),
            definition_point: Vector3::ZERO,
            angle_vertex: Vector3::ZERO,
            leader_length: 0.0,
        }
    }
}

/// Diameter dimension entity
///
/// Measures the diameter of a circle or arc.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionDiameter {
    pub base: DimensionBase,
    /// Definition point (opposite side of diameter) - in WCS
    pub definition_point: Vector3,
    /// Point on arc/circle (in WCS)
    pub angle_vertex: Vector3,
    /// Leader length
    pub leader_length: f64,
}

impl DimensionDiameter {
    /// Create a new diameter dimension
    pub fn new(center: Vector3, point_on_arc: Vector3) -> Self {
        let mut base = DimensionBase::new(DimensionType::Diameter);
        base.actual_measurement = center.distance(&point_on_arc) * 2.0;

        Self {
            base,
            definition_point: point_on_arc,
            angle_vertex: center,
            leader_length: 0.0,
        }
    }

    /// Get the diameter measurement
    pub fn measurement(&self) -> f64 {
        self.definition_point.distance(&self.angle_vertex) * 2.0
    }

    /// Get the center point
    pub fn center(&self) -> Vector3 {
        (self.angle_vertex + self.definition_point) * 0.5
    }
}

impl Default for DimensionDiameter {
    fn default() -> Self {
        Self {
            base: DimensionBase::new(DimensionType::Diameter),
            definition_point: Vector3::ZERO,
            angle_vertex: Vector3::ZERO,
            leader_length: 0.0,
        }
    }
}

/// Angular 2-line dimension entity
///
/// Measures the angle between two lines.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionAngular2Ln {
    pub base: DimensionBase,
    /// Arc definition point (dimension arc location) - in WCS
    pub dimension_arc: Vector3,
    /// First point (line 1 start) - in WCS
    pub first_point: Vector3,
    /// Second point (line 1 end / angle vertex for line 1) - in WCS
    pub second_point: Vector3,
    /// Angle vertex (line 2 vertex) - in WCS
    pub angle_vertex: Vector3,
    /// Definition point (line 2 point defining angle) - in WCS
    pub definition_point: Vector3,
}

impl DimensionAngular2Ln {
    /// Create a new angular 2-line dimension
    pub fn new(vertex: Vector3, first_point: Vector3, second_point: Vector3) -> Self {
        let mut base = DimensionBase::new(DimensionType::Angular);

        // Calculate angle between the two lines
        let v1 = (first_point - vertex).normalize();
        let v2 = (second_point - vertex).normalize();
        let angle = v1.dot(&v2).acos();
        base.actual_measurement = angle.to_degrees();

        Self {
            base,
            dimension_arc: Vector3::ZERO,
            first_point,
            second_point,
            angle_vertex: vertex,
            definition_point: Vector3::ZERO,
        }
    }

    /// Get the angle measurement in radians
    pub fn measurement_radians(&self) -> f64 {
        let v1 = (self.first_point - self.angle_vertex).normalize();
        let v2 = (self.second_point - self.angle_vertex).normalize();
        v1.dot(&v2).acos()
    }

    /// Get the angle measurement in degrees
    pub fn measurement_degrees(&self) -> f64 {
        self.measurement_radians().to_degrees()
    }
}

impl Default for DimensionAngular2Ln {
    fn default() -> Self {
        Self {
            base: DimensionBase::new(DimensionType::Angular),
            dimension_arc: Vector3::ZERO,
            first_point: Vector3::ZERO,
            second_point: Vector3::ZERO,
            angle_vertex: Vector3::ZERO,
            definition_point: Vector3::ZERO,
        }
    }
}

/// Type alias for backward compatibility
pub type DimensionAngular2Line = DimensionAngular2Ln;

/// Angular 3-point dimension entity
///
/// Measures the angle defined by three points.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionAngular3Pt {
    pub base: DimensionBase,
    /// Definition point (arc location) - in WCS
    pub definition_point: Vector3,
    /// First point on first line - in WCS
    pub first_point: Vector3,
    /// Second point on second line - in WCS
    pub second_point: Vector3,
    /// Angle vertex - in WCS
    pub angle_vertex: Vector3,
}

impl DimensionAngular3Pt {
    /// Create a new angular 3-point dimension
    pub fn new(vertex: Vector3, first_point: Vector3, second_point: Vector3) -> Self {
        let mut base = DimensionBase::new(DimensionType::Angular3Point);

        // Calculate angle
        let v1 = (first_point - vertex).normalize();
        let v2 = (second_point - vertex).normalize();
        let angle = v1.dot(&v2).acos();
        base.actual_measurement = angle.to_degrees();

        Self {
            base,
            definition_point: Vector3::ZERO,
            angle_vertex: vertex,
            first_point,
            second_point,
        }
    }

    /// Get the angle measurement in radians
    pub fn measurement_radians(&self) -> f64 {
        let v1 = (self.first_point - self.angle_vertex).normalize();
        let v2 = (self.second_point - self.angle_vertex).normalize();
        v1.dot(&v2).acos()
    }

    /// Get the angle measurement in degrees
    pub fn measurement_degrees(&self) -> f64 {
        self.measurement_radians().to_degrees()
    }
}

impl Default for DimensionAngular3Pt {
    fn default() -> Self {
        Self {
            base: DimensionBase::new(DimensionType::Angular3Point),
            definition_point: Vector3::ZERO,
            first_point: Vector3::ZERO,
            second_point: Vector3::ZERO,
            angle_vertex: Vector3::ZERO,
        }
    }
}

/// Type alias for backward compatibility
pub type DimensionAngular3Point = DimensionAngular3Pt;

/// Ordinate dimension entity
///
/// Measures the X or Y ordinate of a point.
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionOrdinate {
    pub base: DimensionBase,
    /// Definition point (origin) - in WCS
    pub definition_point: Vector3,
    /// Feature location point (in WCS)
    pub feature_location: Vector3,
    /// Leader endpoint (in WCS)
    pub leader_endpoint: Vector3,
    /// True if this is an X-ordinate, false for Y-ordinate
    pub is_ordinate_type_x: bool,
}

impl DimensionOrdinate {
    /// Create a new ordinate dimension
    pub fn new(feature_location: Vector3, leader_endpoint: Vector3, is_x_type: bool) -> Self {
        let mut base = DimensionBase::new(DimensionType::Ordinate);
        base.actual_measurement = if is_x_type {
            feature_location.x
        } else {
            feature_location.y
        };

        Self {
            base,
            definition_point: Vector3::ZERO,
            feature_location,
            leader_endpoint,
            is_ordinate_type_x: is_x_type,
        }
    }

    /// Create a new X-ordinate dimension
    pub fn x_ordinate(feature_location: Vector3, leader_endpoint: Vector3) -> Self {
        Self::new(feature_location, leader_endpoint, true)
    }

    /// Create a new Y-ordinate dimension
    pub fn y_ordinate(feature_location: Vector3, leader_endpoint: Vector3) -> Self {
        Self::new(feature_location, leader_endpoint, false)
    }

    /// Get the ordinate measurement
    pub fn measurement(&self) -> f64 {
        if self.is_ordinate_type_x {
            self.feature_location.x
        } else {
            self.feature_location.y
        }
    }
}

impl Default for DimensionOrdinate {
    fn default() -> Self {
        Self {
            base: DimensionBase::new(DimensionType::Ordinate),
            definition_point: Vector3::ZERO,
            feature_location: Vector3::ZERO,
            leader_endpoint: Vector3::ZERO,
            is_ordinate_type_x: true,
        }
    }
}

/// Unified dimension enum for all dimension types
#[derive(Debug, Clone, PartialEq)]
pub enum Dimension {
    Aligned(DimensionAligned),
    Linear(DimensionLinear),
    Radius(DimensionRadius),
    Diameter(DimensionDiameter),
    Angular2Ln(DimensionAngular2Ln),
    Angular3Pt(DimensionAngular3Pt),
    Ordinate(DimensionOrdinate),
}

impl Dimension {
    /// Get the base dimension data
    pub fn base(&self) -> &DimensionBase {
        match self {
            Dimension::Aligned(d) => &d.base,
            Dimension::Linear(d) => &d.base,
            Dimension::Radius(d) => &d.base,
            Dimension::Diameter(d) => &d.base,
            Dimension::Angular2Ln(d) => &d.base,
            Dimension::Angular3Pt(d) => &d.base,
            Dimension::Ordinate(d) => &d.base,
        }
    }

    /// Get mutable base dimension data
    pub fn base_mut(&mut self) -> &mut DimensionBase {
        match self {
            Dimension::Aligned(d) => &mut d.base,
            Dimension::Linear(d) => &mut d.base,
            Dimension::Radius(d) => &mut d.base,
            Dimension::Diameter(d) => &mut d.base,
            Dimension::Angular2Ln(d) => &mut d.base,
            Dimension::Angular3Pt(d) => &mut d.base,
            Dimension::Ordinate(d) => &mut d.base,
        }
    }

    /// Get the measurement value
    pub fn measurement(&self) -> f64 {
        match self {
            Dimension::Aligned(d) => d.measurement(),
            Dimension::Linear(d) => d.measurement(),
            Dimension::Radius(d) => d.measurement(),
            Dimension::Diameter(d) => d.measurement(),
            Dimension::Angular2Ln(d) => d.measurement_degrees(),
            Dimension::Angular3Pt(d) => d.measurement_degrees(),
            Dimension::Ordinate(d) => d.measurement(),
        }
    }
}

impl super::Entity for Dimension {
    fn handle(&self) -> crate::types::Handle {
        self.base().common.handle
    }

    fn set_handle(&mut self, handle: crate::types::Handle) {
        self.base_mut().common.handle = handle;
    }

    fn layer(&self) -> &str {
        &self.base().common.layer
    }

    fn set_layer(&mut self, layer: String) {
        self.base_mut().common.layer = layer;
    }

    fn color(&self) -> crate::types::Color {
        self.base().common.color
    }

    fn set_color(&mut self, color: crate::types::Color) {
        self.base_mut().common.color = color;
    }

    fn line_weight(&self) -> crate::types::LineWeight {
        self.base().common.line_weight
    }

    fn set_line_weight(&mut self, weight: crate::types::LineWeight) {
        self.base_mut().common.line_weight = weight;
    }

    fn transparency(&self) -> crate::types::Transparency {
        self.base().common.transparency
    }

    fn set_transparency(&mut self, transparency: crate::types::Transparency) {
        self.base_mut().common.transparency = transparency;
    }

    fn is_invisible(&self) -> bool {
        self.base().common.invisible
    }

    fn set_invisible(&mut self, invisible: bool) {
        self.base_mut().common.invisible = invisible;
    }

    fn bounding_box(&self) -> crate::types::BoundingBox3D {
        use crate::types::BoundingBox3D;
        match self {
            Dimension::Aligned(d) => BoundingBox3D::from_points(&[d.first_point, d.second_point, d.definition_point]).unwrap_or_default(),
            Dimension::Linear(d) => BoundingBox3D::from_points(&[d.first_point, d.second_point, d.definition_point]).unwrap_or_default(),
            Dimension::Radius(d) => BoundingBox3D::from_points(&[d.angle_vertex, d.definition_point]).unwrap_or_default(),
            Dimension::Diameter(d) => BoundingBox3D::from_points(&[d.angle_vertex, d.definition_point]).unwrap_or_default(),
            Dimension::Angular2Ln(d) => BoundingBox3D::from_points(&[d.angle_vertex, d.first_point, d.second_point, d.definition_point]).unwrap_or_default(),
            Dimension::Angular3Pt(d) => BoundingBox3D::from_points(&[d.angle_vertex, d.first_point, d.second_point, d.definition_point]).unwrap_or_default(),
            Dimension::Ordinate(d) => BoundingBox3D::from_points(&[d.feature_location, d.leader_endpoint, d.definition_point]).unwrap_or_default(),
        }
    }

    fn translate(&mut self, offset: Vector3) {
        match self {
            Dimension::Aligned(d) => {
                d.definition_point = d.definition_point + offset;
                d.base.text_middle_point = d.base.text_middle_point + offset;
                d.first_point = d.first_point + offset;
                d.second_point = d.second_point + offset;
            }
            Dimension::Linear(d) => {
                d.definition_point = d.definition_point + offset;
                d.base.text_middle_point = d.base.text_middle_point + offset;
                d.first_point = d.first_point + offset;
                d.second_point = d.second_point + offset;
            }
            Dimension::Radius(d) => {
                d.definition_point = d.definition_point + offset;
                d.base.text_middle_point = d.base.text_middle_point + offset;
                d.angle_vertex = d.angle_vertex + offset;
            }
            Dimension::Diameter(d) => {
                d.definition_point = d.definition_point + offset;
                d.base.text_middle_point = d.base.text_middle_point + offset;
                d.angle_vertex = d.angle_vertex + offset;
            }
            Dimension::Angular2Ln(d) => {
                d.definition_point = d.definition_point + offset;
                d.base.text_middle_point = d.base.text_middle_point + offset;
                d.angle_vertex = d.angle_vertex + offset;
                d.first_point = d.first_point + offset;
                d.second_point = d.second_point + offset;
            }
            Dimension::Angular3Pt(d) => {
                d.definition_point = d.definition_point + offset;
                d.base.text_middle_point = d.base.text_middle_point + offset;
                d.angle_vertex = d.angle_vertex + offset;
                d.first_point = d.first_point + offset;
                d.second_point = d.second_point + offset;
            }
            Dimension::Ordinate(d) => {
                d.definition_point = d.definition_point + offset;
                d.base.text_middle_point = d.base.text_middle_point + offset;
                d.feature_location = d.feature_location + offset;
                d.leader_endpoint = d.leader_endpoint + offset;
            }
        }
    }

    fn entity_type(&self) -> &'static str {
        match self {
            Dimension::Aligned(_) => "DIMENSION_ALIGNED",
            Dimension::Linear(_) => "DIMENSION_LINEAR",
            Dimension::Radius(_) => "DIMENSION_RADIUS",
            Dimension::Diameter(_) => "DIMENSION_DIAMETER",
            Dimension::Angular2Ln(_) => "DIMENSION_ANGULAR_2LINE",
            Dimension::Angular3Pt(_) => "DIMENSION_ANGULAR_3POINT",
            Dimension::Ordinate(_) => "DIMENSION_ORDINATE",
        }
    }
}