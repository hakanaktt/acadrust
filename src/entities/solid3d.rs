//! Solid3D (3DSOLID) entity implementation.
//!
//! The Solid3D entity represents a 3D solid body with geometry stored
//! in ACIS/SAT format. It also provides wireframe and silhouette data
//! for visualization without parsing the full ACIS data.

use crate::entities::{Entity, EntityCommon};
use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transparency, Vector3};

// ============================================================================
// Wire Type
// ============================================================================

/// Wire type for wireframe display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum WireType {
    /// Unknown wire type.
    #[default]
    Unknown = 0,
    /// Silhouette edge.
    Silhouette = 1,
    /// Visible edge.
    VisibleEdge = 2,
    /// Hidden edge.
    HiddenEdge = 3,
    /// Isoparametric curve.
    Isoline = 4,
}

impl From<u8> for WireType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Silhouette,
            2 => Self::VisibleEdge,
            3 => Self::HiddenEdge,
            4 => Self::Isoline,
            _ => Self::Unknown,
        }
    }
}

// ============================================================================
// Wire Data
// ============================================================================

/// Wireframe edge data for visualization.
///
/// Provides display geometry without requiring full ACIS parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct Wire {
    /// ACIS entity index this wire belongs to.
    pub acis_index: i32,
    /// Wire type.
    pub wire_type: WireType,
    /// Selection marker for picking.
    pub selection_marker: i32,
    /// Wire color.
    pub color: Color,
    /// Points defining the wire path.
    pub points: Vec<Vector3>,

    // Transform data (when present)
    /// Whether transform data is present.
    pub has_transform: bool,
    /// Transform has rotation.
    pub has_rotation: bool,
    /// Transform has shear.
    pub has_shear: bool,
    /// Transform has reflection.
    pub has_reflection: bool,
    /// Transform scale factor.
    pub scale: f64,
    /// Transform translation.
    pub translation: Vector3,
    /// X axis of transform.
    pub x_axis: Vector3,
    /// Y axis of transform.
    pub y_axis: Vector3,
    /// Z axis of transform.
    pub z_axis: Vector3,
}

impl Wire {
    /// Creates a new wire with default transform.
    pub fn new() -> Self {
        Self {
            acis_index: 0,
            wire_type: WireType::Unknown,
            selection_marker: 0,
            color: Color::ByLayer,
            points: Vec::new(),
            has_transform: false,
            has_rotation: false,
            has_shear: false,
            has_reflection: false,
            scale: 1.0,
            translation: Vector3::ZERO,
            x_axis: Vector3::UNIT_X,
            y_axis: Vector3::UNIT_Y,
            z_axis: Vector3::UNIT_Z,
        }
    }

    /// Creates a wire from a list of points.
    pub fn from_points(points: Vec<Vector3>) -> Self {
        let mut wire = Self::new();
        wire.points = points;
        wire
    }

    /// Adds a point to the wire.
    pub fn add_point(&mut self, point: Vector3) {
        self.points.push(point);
    }

    /// Returns the number of points.
    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    /// Returns the bounding box of the wire points.
    pub fn bounding_box(&self) -> Option<BoundingBox3D> {
        if self.points.is_empty() {
            return None;
        }

        let first = self.points[0];
        let mut min = first;
        let mut max = first;

        for pt in &self.points[1..] {
            min.x = min.x.min(pt.x);
            min.y = min.y.min(pt.y);
            min.z = min.z.min(pt.z);
            max.x = max.x.max(pt.x);
            max.y = max.y.max(pt.y);
            max.z = max.z.max(pt.z);
        }

        Some(BoundingBox3D::new(min, max))
    }
}

impl Default for Wire {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Silhouette
// ============================================================================

/// Silhouette data for a specific viewport.
///
/// Provides pre-computed silhouette curves for different view directions.
#[derive(Debug, Clone, PartialEq)]
pub struct Silhouette {
    /// Viewport identifier.
    pub viewport_id: i64,
    /// Viewport view direction.
    pub view_direction: Vector3,
    /// Viewport up vector.
    pub up_vector: Vector3,
    /// Viewport target point.
    pub target: Vector3,
    /// Whether viewport uses perspective.
    pub is_perspective: bool,
    /// Silhouette wires for this viewport.
    pub wires: Vec<Wire>,
}

impl Silhouette {
    /// Creates a new silhouette for the given viewport.
    pub fn new(viewport_id: i64) -> Self {
        Self {
            viewport_id,
            view_direction: Vector3::new(0.0, 0.0, 1.0),
            up_vector: Vector3::new(0.0, 1.0, 0.0),
            target: Vector3::ZERO,
            is_perspective: false,
            wires: Vec::new(),
        }
    }

    /// Creates a silhouette with view direction.
    pub fn with_view(viewport_id: i64, view_direction: Vector3, up_vector: Vector3) -> Self {
        Self {
            viewport_id,
            view_direction: view_direction.normalize(),
            up_vector: up_vector.normalize(),
            target: Vector3::ZERO,
            is_perspective: false,
            wires: Vec::new(),
        }
    }

    /// Adds a wire to the silhouette.
    pub fn add_wire(&mut self, wire: Wire) {
        self.wires.push(wire);
    }

    /// Returns the number of wires.
    pub fn wire_count(&self) -> usize {
        self.wires.len()
    }
}

impl Default for Silhouette {
    fn default() -> Self {
        Self::new(0)
    }
}

// ============================================================================
// ACIS Data
// ============================================================================

/// ACIS/SAT data format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AcisVersion {
    /// Version 1: SAT data with character encoding.
    #[default]
    Version1 = 1,
    /// Version 2: Text SAT or binary SAB.
    Version2 = 2,
}

impl From<u8> for AcisVersion {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Version1,
            2 => Self::Version2,
            _ => Self::Version1,
        }
    }
}

/// Container for ACIS/SAT solid data.
///
/// The ACIS data represents the actual 3D solid geometry in Spatial
/// Corporation's proprietary format.
#[derive(Debug, Clone, PartialEq)]
pub struct AcisData {
    /// Version of the modeler format.
    pub version: AcisVersion,
    /// Raw SAT text data (for Version 1 or text Version 2).
    pub sat_data: String,
    /// Binary SAB data (for binary Version 2).
    pub sab_data: Vec<u8>,
    /// Whether this is binary SAB format.
    pub is_binary: bool,
}

impl AcisData {
    /// Creates empty ACIS data.
    pub fn new() -> Self {
        Self {
            version: AcisVersion::Version1,
            sat_data: String::new(),
            sab_data: Vec::new(),
            is_binary: false,
        }
    }

    /// Creates ACIS data from SAT text.
    pub fn from_sat(sat: &str) -> Self {
        Self {
            version: AcisVersion::Version1,
            sat_data: sat.to_string(),
            sab_data: Vec::new(),
            is_binary: false,
        }
    }

    /// Creates ACIS data from binary SAB.
    pub fn from_sab(sab: Vec<u8>) -> Self {
        Self {
            version: AcisVersion::Version2,
            sat_data: String::new(),
            sab_data: sab,
            is_binary: true,
        }
    }

    /// Returns true if this contains valid data.
    pub fn has_data(&self) -> bool {
        !self.sat_data.is_empty() || !self.sab_data.is_empty()
    }

    /// Returns the data size in bytes.
    pub fn size(&self) -> usize {
        if self.is_binary {
            self.sab_data.len()
        } else {
            self.sat_data.len()
        }
    }
}

impl Default for AcisData {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Solid3D Entity
// ============================================================================

/// 3D Solid (3DSOLID) entity.
///
/// Represents a 3D solid body with geometry stored in ACIS/SAT format.
/// The wireframe and silhouette data provide visualization hints without
/// requiring full ACIS parsing.
///
/// # Example
///
/// ```ignore
/// use acadrust::entities::Solid3D;
/// use acadrust::types::Vector3;
///
/// // Create a 3D solid (typically from DXF/DWG import)
/// let mut solid = Solid3D::new();
///
/// // Set ACIS data (from file import)
/// solid.set_sat_data("ACIS data here...");
///
/// // Access wireframe for visualization
/// for wire in &solid.wires {
///     for point in &wire.points {
///         // Draw wire segment
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Solid3D {
    /// Common entity data.
    pub common: EntityCommon,
    /// Unique identifier within the file.
    pub uid: String,
    /// Point of reference (typically origin).
    pub point_of_reference: Vector3,
    /// ACIS/SAT solid data.
    pub acis_data: AcisData,
    /// Wireframe edges for visualization.
    pub wires: Vec<Wire>,
    /// Silhouette data for viewports.
    pub silhouettes: Vec<Silhouette>,
    /// Handle to edit history object (R2007+).
    pub history_handle: Option<Handle>,
}

impl Solid3D {
    /// Creates a new empty 3D solid.
    pub fn new() -> Self {
        Self {
            common: EntityCommon::default(),
            uid: String::new(),
            point_of_reference: Vector3::ZERO,
            acis_data: AcisData::new(),
            wires: Vec::new(),
            silhouettes: Vec::new(),
            history_handle: None,
        }
    }

    /// Creates a 3D solid from SAT text data.
    pub fn from_sat(sat: &str) -> Self {
        let mut solid = Self::new();
        solid.acis_data = AcisData::from_sat(sat);
        solid
    }

    /// Creates a 3D solid from binary SAB data.
    pub fn from_sab(sab: Vec<u8>) -> Self {
        let mut solid = Self::new();
        solid.acis_data = AcisData::from_sab(sab);
        solid
    }

    /// Sets the SAT data.
    pub fn set_sat_data(&mut self, sat: &str) {
        self.acis_data = AcisData::from_sat(sat);
    }

    /// Sets the SAB data.
    pub fn set_sab_data(&mut self, sab: Vec<u8>) {
        self.acis_data = AcisData::from_sab(sab);
    }

    /// Returns true if this solid has valid ACIS data.
    pub fn has_acis_data(&self) -> bool {
        self.acis_data.has_data()
    }

    /// Returns the ACIS data size.
    pub fn acis_size(&self) -> usize {
        self.acis_data.size()
    }

    /// Adds a wireframe edge.
    pub fn add_wire(&mut self, wire: Wire) {
        self.wires.push(wire);
    }

    /// Creates and adds a simple wire from points.
    pub fn add_wire_from_points(&mut self, points: Vec<Vector3>) -> &mut Wire {
        self.wires.push(Wire::from_points(points));
        self.wires.last_mut().unwrap()
    }

    /// Returns the number of wireframe edges.
    pub fn wire_count(&self) -> usize {
        self.wires.len()
    }

    /// Adds silhouette data.
    pub fn add_silhouette(&mut self, silhouette: Silhouette) {
        self.silhouettes.push(silhouette);
    }

    /// Returns silhouette for a viewport.
    pub fn silhouette_for_viewport(&self, viewport_id: i64) -> Option<&Silhouette> {
        self.silhouettes.iter().find(|s| s.viewport_id == viewport_id)
    }

    /// Clears all visualization data (wires and silhouettes).
    pub fn clear_visualization(&mut self) {
        self.wires.clear();
        self.silhouettes.clear();
    }

    /// Calculates bounding box from wireframe data.
    fn wireframe_bounding_box(&self) -> BoundingBox3D {
        if self.wires.is_empty() {
            return BoundingBox3D::default();
        }

        let mut min = Vector3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max = Vector3::new(f64::MIN, f64::MIN, f64::MIN);
        let mut has_points = false;

        for wire in &self.wires {
            for pt in &wire.points {
                has_points = true;
                min.x = min.x.min(pt.x);
                min.y = min.y.min(pt.y);
                min.z = min.z.min(pt.z);
                max.x = max.x.max(pt.x);
                max.y = max.y.max(pt.y);
                max.z = max.z.max(pt.z);
            }
        }

        if has_points {
            BoundingBox3D::new(min, max)
        } else {
            BoundingBox3D::default()
        }
    }
}

impl Default for Solid3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Solid3D {
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

    fn set_line_weight(&mut self, line_weight: LineWeight) {
        self.common.line_weight = line_weight;
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
        self.wireframe_bounding_box()
    }

    fn translate(&mut self, offset: Vector3) {
        // Translate point of reference
        self.point_of_reference = self.point_of_reference + offset;

        // Translate all wireframe points
        for wire in &mut self.wires {
            for pt in &mut wire.points {
                *pt = *pt + offset;
            }
            wire.translation = wire.translation + offset;
        }

        // Translate silhouette targets
        for silhouette in &mut self.silhouettes {
            silhouette.target = silhouette.target + offset;
            for wire in &mut silhouette.wires {
                for pt in &mut wire.points {
                    *pt = *pt + offset;
                }
            }
        }
    }

    fn entity_type(&self) -> &'static str {
        "3DSOLID"
    }
    
    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        // Transform point of reference
        self.point_of_reference = transform.apply(self.point_of_reference);
        
        // Transform all wireframe points
        for wire in &mut self.wires {
            for pt in &mut wire.points {
                *pt = transform.apply(*pt);
            }
            wire.translation = transform.apply(wire.translation);
        }
        
        // Transform silhouette data
        for silhouette in &mut self.silhouettes {
            silhouette.target = transform.apply(silhouette.target);
            silhouette.view_direction = transform.apply_rotation(silhouette.view_direction).normalize();
            silhouette.up_vector = transform.apply_rotation(silhouette.up_vector).normalize();
            for wire in &mut silhouette.wires {
                for pt in &mut wire.points {
                    *pt = transform.apply(*pt);
                }
            }
        }
    }
}

// ============================================================================
// Region Entity (similar structure, 2D enclosed area)
// ============================================================================

/// Region entity.
///
/// Represents a 2D enclosed area stored in ACIS format.
/// Similar to Solid3D but for 2D geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// Common entity data.
    pub common: EntityCommon,
    /// Unique identifier within the file.
    pub uid: String,
    /// Point of reference.
    pub point_of_reference: Vector3,
    /// ACIS/SAT region data.
    pub acis_data: AcisData,
    /// Wireframe edges for visualization.
    pub wires: Vec<Wire>,
    /// Silhouette data for viewports.
    pub silhouettes: Vec<Silhouette>,
}

impl Region {
    /// Creates a new empty region.
    pub fn new() -> Self {
        Self {
            common: EntityCommon::default(),
            uid: String::new(),
            point_of_reference: Vector3::ZERO,
            acis_data: AcisData::new(),
            wires: Vec::new(),
            silhouettes: Vec::new(),
        }
    }

    /// Creates a region from SAT text data.
    pub fn from_sat(sat: &str) -> Self {
        let mut region = Self::new();
        region.acis_data = AcisData::from_sat(sat);
        region
    }

    /// Returns true if this region has valid ACIS data.
    pub fn has_acis_data(&self) -> bool {
        self.acis_data.has_data()
    }

    /// Adds a wireframe edge.
    pub fn add_wire(&mut self, wire: Wire) {
        self.wires.push(wire);
    }
}

impl Default for Region {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Region {
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

    fn set_line_weight(&mut self, line_weight: LineWeight) {
        self.common.line_weight = line_weight;
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
        if self.wires.is_empty() {
            return BoundingBox3D::default();
        }

        let mut min = Vector3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max = Vector3::new(f64::MIN, f64::MIN, f64::MIN);

        for wire in &self.wires {
            for pt in &wire.points {
                min.x = min.x.min(pt.x);
                min.y = min.y.min(pt.y);
                min.z = min.z.min(pt.z);
                max.x = max.x.max(pt.x);
                max.y = max.y.max(pt.y);
                max.z = max.z.max(pt.z);
            }
        }

        BoundingBox3D::new(min, max)
    }

    fn translate(&mut self, offset: Vector3) {
        self.point_of_reference = self.point_of_reference + offset;
        for wire in &mut self.wires {
            for pt in &mut wire.points {
                *pt = *pt + offset;
            }
        }
    }

    fn entity_type(&self) -> &'static str {
        "REGION"
    }
    
    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        // Transform point of reference
        self.point_of_reference = transform.apply(self.point_of_reference);
        
        // Transform all wireframe points
        for wire in &mut self.wires {
            for pt in &mut wire.points {
                *pt = transform.apply(*pt);
            }
        }
    }
}

// ============================================================================
// Body Entity (similar structure, 3D body)
// ============================================================================

/// Body entity.
///
/// Represents a 3D body stored in ACIS format.
/// Similar to Solid3D but a different entity type.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    /// Common entity data.
    pub common: EntityCommon,
    /// Unique identifier within the file.
    pub uid: String,
    /// Point of reference.
    pub point_of_reference: Vector3,
    /// ACIS/SAT body data.
    pub acis_data: AcisData,
    /// Wireframe edges for visualization.
    pub wires: Vec<Wire>,
    /// Silhouette data for viewports.
    pub silhouettes: Vec<Silhouette>,
}

impl Body {
    /// Creates a new empty body.
    pub fn new() -> Self {
        Self {
            common: EntityCommon::default(),
            uid: String::new(),
            point_of_reference: Vector3::ZERO,
            acis_data: AcisData::new(),
            wires: Vec::new(),
            silhouettes: Vec::new(),
        }
    }

    /// Creates a body from SAT text data.
    pub fn from_sat(sat: &str) -> Self {
        let mut body = Self::new();
        body.acis_data = AcisData::from_sat(sat);
        body
    }

    /// Returns true if this body has valid ACIS data.
    pub fn has_acis_data(&self) -> bool {
        self.acis_data.has_data()
    }

    /// Adds a wireframe edge.
    pub fn add_wire(&mut self, wire: Wire) {
        self.wires.push(wire);
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::new()
    }
}

impl Entity for Body {
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

    fn set_line_weight(&mut self, line_weight: LineWeight) {
        self.common.line_weight = line_weight;
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
        if self.wires.is_empty() {
            return BoundingBox3D::default();
        }

        let mut min = Vector3::new(f64::MAX, f64::MAX, f64::MAX);
        let mut max = Vector3::new(f64::MIN, f64::MIN, f64::MIN);

        for wire in &self.wires {
            for pt in &wire.points {
                min.x = min.x.min(pt.x);
                min.y = min.y.min(pt.y);
                min.z = min.z.min(pt.z);
                max.x = max.x.max(pt.x);
                max.y = max.y.max(pt.y);
                max.z = max.z.max(pt.z);
            }
        }

        BoundingBox3D::new(min, max)
    }

    fn translate(&mut self, offset: Vector3) {
        self.point_of_reference = self.point_of_reference + offset;
        for wire in &mut self.wires {
            for pt in &mut wire.points {
                *pt = *pt + offset;
            }
        }
    }

    fn entity_type(&self) -> &'static str {
        "BODY"
    }
    
    fn apply_transform(&mut self, transform: &crate::types::Transform) {
        // Transform point of reference
        self.point_of_reference = transform.apply(self.point_of_reference);
        
        // Transform all wireframe points
        for wire in &mut self.wires {
            for pt in &mut wire.points {
                *pt = transform.apply(*pt);
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solid3d_creation() {
        let solid = Solid3D::new();
        assert!(solid.uid.is_empty());
        assert_eq!(solid.point_of_reference, Vector3::ZERO);
        assert!(!solid.has_acis_data());
    }

    #[test]
    fn test_solid3d_from_sat() {
        let sat = "400 0 1 0\n16 ASM-BODY 1.0 0\n";
        let solid = Solid3D::from_sat(sat);
        assert!(solid.has_acis_data());
        assert!(!solid.acis_data.is_binary);
        assert_eq!(solid.acis_data.sat_data, sat);
    }

    #[test]
    fn test_solid3d_from_sab() {
        let sab = b"ACIS BinaryFile".to_vec();
        let solid = Solid3D::from_sab(sab.clone());
        assert!(solid.has_acis_data());
        assert!(solid.acis_data.is_binary);
        assert_eq!(solid.acis_data.sab_data, sab);
    }

    #[test]
    fn test_wire_creation() {
        let wire = Wire::new();
        assert_eq!(wire.wire_type, WireType::Unknown);
        assert!(wire.points.is_empty());
        assert!(!wire.has_transform);
    }

    #[test]
    fn test_wire_from_points() {
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
        ];
        let wire = Wire::from_points(points.clone());
        assert_eq!(wire.point_count(), 3);
        assert_eq!(wire.points, points);
    }

    #[test]
    fn test_wire_bounding_box() {
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 5.0, 3.0),
            Vector3::new(5.0, 10.0, 1.0),
        ];
        let wire = Wire::from_points(points);
        let bbox = wire.bounding_box().unwrap();
        assert_eq!(bbox.min, Vector3::new(0.0, 0.0, 0.0));
        assert_eq!(bbox.max, Vector3::new(10.0, 10.0, 3.0));
    }

    #[test]
    fn test_silhouette_creation() {
        let silhouette = Silhouette::new(42);
        assert_eq!(silhouette.viewport_id, 42);
        assert!(!silhouette.is_perspective);
        assert_eq!(silhouette.wire_count(), 0);
    }

    #[test]
    fn test_silhouette_with_view() {
        let silhouette = Silhouette::with_view(
            1,
            Vector3::new(0.0, 0.0, -1.0),
            Vector3::new(0.0, 1.0, 0.0),
        );
        assert_eq!(silhouette.viewport_id, 1);
        assert_eq!(silhouette.view_direction.z, -1.0);
    }

    #[test]
    fn test_solid3d_add_wire() {
        let mut solid = Solid3D::new();
        solid.add_wire(Wire::from_points(vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
        ]));
        assert_eq!(solid.wire_count(), 1);
    }

    #[test]
    fn test_solid3d_translate() {
        let mut solid = Solid3D::new();
        solid.point_of_reference = Vector3::new(1.0, 2.0, 3.0);
        solid.add_wire(Wire::from_points(vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
        ]));

        solid.translate(Vector3::new(10.0, 20.0, 30.0));

        assert_eq!(solid.point_of_reference, Vector3::new(11.0, 22.0, 33.0));
        assert_eq!(solid.wires[0].points[0], Vector3::new(10.0, 20.0, 30.0));
        assert_eq!(solid.wires[0].points[1], Vector3::new(11.0, 20.0, 30.0));
    }

    #[test]
    fn test_acis_data() {
        let mut data = AcisData::new();
        assert!(!data.has_data());
        assert_eq!(data.size(), 0);

        data.sat_data = "test".to_string();
        assert!(data.has_data());
        assert_eq!(data.size(), 4);
    }

    #[test]
    fn test_acis_version() {
        assert_eq!(AcisVersion::from(1), AcisVersion::Version1);
        assert_eq!(AcisVersion::from(2), AcisVersion::Version2);
        assert_eq!(AcisVersion::from(99), AcisVersion::Version1);
    }

    #[test]
    fn test_wire_type() {
        assert_eq!(WireType::from(1), WireType::Silhouette);
        assert_eq!(WireType::from(2), WireType::VisibleEdge);
        assert_eq!(WireType::from(3), WireType::HiddenEdge);
        assert_eq!(WireType::from(4), WireType::Isoline);
        assert_eq!(WireType::from(99), WireType::Unknown);
    }

    #[test]
    fn test_region_creation() {
        let region = Region::new();
        assert!(!region.has_acis_data());
        assert_eq!(region.entity_type(), "REGION");
    }

    #[test]
    fn test_body_creation() {
        let body = Body::new();
        assert!(!body.has_acis_data());
        assert_eq!(body.entity_type(), "BODY");
    }

    #[test]
    fn test_solid3d_silhouette_for_viewport() {
        let mut solid = Solid3D::new();
        solid.add_silhouette(Silhouette::new(1));
        solid.add_silhouette(Silhouette::new(2));

        assert!(solid.silhouette_for_viewport(1).is_some());
        assert!(solid.silhouette_for_viewport(2).is_some());
        assert!(solid.silhouette_for_viewport(3).is_none());
    }

    #[test]
    fn test_solid3d_clear_visualization() {
        let mut solid = Solid3D::new();
        solid.add_wire(Wire::new());
        solid.add_silhouette(Silhouette::new(1));

        solid.clear_visualization();

        assert_eq!(solid.wire_count(), 0);
        assert!(solid.silhouettes.is_empty());
    }
}

