//! CAD entity types and traits

use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transform, Transparency, Vector3};

pub mod point;
pub mod line;
pub mod circle;
pub mod arc;
pub mod ellipse;
pub mod polyline;
pub mod polyline3d;
pub mod lwpolyline;
pub mod text;
pub mod mtext;
pub mod spline;
pub mod dimension;
pub mod hatch;
pub mod solid;
pub mod face3d;
pub mod insert;
pub mod block;
pub mod ray;
pub mod xline;
pub mod viewport;
pub mod attribute_definition;
pub mod attribute_entity;
pub mod leader;
pub mod multileader;
pub mod mline;
pub mod mesh;
pub mod raster_image;
pub mod solid3d;
pub mod table;
pub mod tolerance;
pub mod polyface_mesh;
pub mod wipeout;
pub mod shape;
pub mod underlay;

pub use point::Point;
pub use line::Line;
pub use circle::Circle;
pub use arc::Arc;
pub use ellipse::Ellipse;
pub use polyline::{Polyline, Polyline2D, Vertex2D, Vertex3D, PolylineFlags, VertexFlags, SmoothSurfaceType};
pub use polyline3d::{Polyline3D, Vertex3DPolyline, Polyline3DFlags};
pub use lwpolyline::{LwPolyline, LwVertex};
pub use text::{Text, TextHorizontalAlignment, TextVerticalAlignment};
pub use mtext::{MText, AttachmentPoint, DrawingDirection};
pub use spline::{Spline, SplineFlags};
pub use dimension::*;
pub use hatch::*;
pub use solid::Solid;
pub use face3d::{Face3D, InvisibleEdgeFlags};
pub use insert::Insert;
pub use block::{Block, BlockEnd};
pub use ray::Ray;
pub use xline::XLine;
pub use viewport::{Viewport, ViewportStatusFlags, ViewportRenderMode, StandardView, GridFlags};
pub use attribute_definition::{AttributeDefinition, AttributeFlags, HorizontalAlignment, VerticalAlignment, MTextFlag};
pub use attribute_entity::AttributeEntity;
pub use leader::{Leader, LeaderPathType, LeaderCreationType, HooklineDirection};
pub use multileader::{
    MultiLeader, MultiLeaderBuilder, MultiLeaderAnnotContext,
    LeaderRoot, LeaderLine, BlockAttribute, StartEndPointPair,
    LeaderContentType, MultiLeaderPathType, TextAttachmentType, TextAngleType,
    BlockContentConnectionType, TextAttachmentDirectionType, TextAttachmentPointType,
    TextAlignmentType, FlowDirectionType, LineSpacingStyle,
    MultiLeaderPropertyOverrideFlags, LeaderLinePropertyOverrideFlags,
};
pub use mline::{
    MLine, MLineBuilder, MLineVertex, MLineSegment,
    MLineStyle, MLineStyleElement, MLineJustification, MLineFlags, MLineStyleFlags,
};
pub use mesh::{Mesh, MeshBuilder, MeshEdge, MeshFace};
pub use raster_image::{
    RasterImage, RasterImageBuilder, ImageDefinition, ClipBoundary,
    ClipMode, ClipType, ImageDisplayFlags, ImageDisplayQuality, ResolutionUnit,
};
pub use solid3d::{
    Solid3D, Region, Body, Wire, Silhouette, AcisData,
    WireType, AcisVersion,
};
pub use table::{
    Table, TableBuilder, TableCell, TableRow, TableColumn,
    CellContent, CellValue, CellStyle, CellBorder, CellRange,
    CellType, CellValueType, ValueUnitType, BorderType,
    TableCellContentType, CellStyleType, BreakFlowDirection,
    CellEdgeFlags, CellStateFlags, CellStylePropertyFlags,
    BorderPropertyFlags, ContentLayoutFlags, BreakOptionFlags,
};
pub use tolerance::{Tolerance, gdt_symbols};
pub use polyface_mesh::{
    PolyfaceMesh, PolyfaceVertex, PolyfaceFace,
    PolyfaceMeshFlags, PolyfaceVertexFlags, PolyfaceSmoothType,
};
pub use wipeout::{
    Wipeout, WipeoutDisplayFlags, WipeoutClipType, WipeoutClipMode,
};
pub use shape::{Shape, standard_shapes, gdt_shapes};
pub use underlay::{
    Underlay, UnderlayDefinition, UnderlayType, UnderlayDisplayFlags,
    PdfUnderlay, DwfUnderlay, DgnUnderlay,
    PdfUnderlayDefinition, DwfUnderlayDefinition, DgnUnderlayDefinition,
};

/// Base trait for all CAD entities
pub trait Entity {
    /// Get the entity's unique handle
    fn handle(&self) -> Handle;

    /// Set the entity's handle
    fn set_handle(&mut self, handle: Handle);

    /// Get the entity's layer name
    fn layer(&self) -> &str;

    /// Set the entity's layer name
    fn set_layer(&mut self, layer: String);

    /// Get the entity's color
    fn color(&self) -> Color;

    /// Set the entity's color
    fn set_color(&mut self, color: Color);

    /// Get the entity's line weight
    fn line_weight(&self) -> LineWeight;

    /// Set the entity's line weight
    fn set_line_weight(&mut self, weight: LineWeight);

    /// Get the entity's transparency
    fn transparency(&self) -> Transparency;

    /// Set the entity's transparency
    fn set_transparency(&mut self, transparency: Transparency);

    /// Check if the entity is invisible
    fn is_invisible(&self) -> bool;

    /// Set the entity's visibility
    fn set_invisible(&mut self, invisible: bool);

    /// Get the bounding box of the entity
    fn bounding_box(&self) -> BoundingBox3D;

    /// Transform the entity by a translation vector
    fn translate(&mut self, offset: Vector3);

    /// Get the entity type name
    fn entity_type(&self) -> &'static str;
    
    /// Apply a general transform to the entity
    /// 
    /// This is the main transformation method. Default implementation
    /// only supports translation for backward compatibility.
    fn apply_transform(&mut self, transform: &Transform) {
        // Default: extract translation and apply
        let origin = Vector3::ZERO;
        let translated = transform.apply(origin);
        self.translate(translated);
    }
    
    /// Apply rotation around an axis
    fn apply_rotation(&mut self, axis: Vector3, angle: f64) {
        self.apply_transform(&Transform::from_rotation(axis, angle));
    }
    
    /// Apply uniform scaling
    fn apply_scaling(&mut self, scale: f64) {
        self.apply_transform(&Transform::from_scale(scale));
    }
    
    /// Apply non-uniform scaling
    fn apply_scaling_xyz(&mut self, scale: Vector3) {
        self.apply_transform(&Transform::from_scaling(scale));
    }
    
    /// Apply scaling with a specific origin point
    fn apply_scaling_with_origin(&mut self, scale: Vector3, origin: Vector3) {
        self.apply_transform(&Transform::from_scaling_with_origin(scale, origin));
    }
}

/// Common entity data shared by all entities
#[derive(Debug, Clone, PartialEq)]
pub struct EntityCommon {
    /// Unique handle
    pub handle: Handle,
    /// Layer name
    pub layer: String,
    /// Color
    pub color: Color,
    /// Line weight
    pub line_weight: LineWeight,
    /// Transparency
    pub transparency: Transparency,
    /// Visibility flag
    pub invisible: bool,
    /// Extended data (XDATA)
    pub extended_data: crate::xdata::ExtendedData,
}

impl EntityCommon {
    /// Create new common entity data with defaults
    pub fn new() -> Self {
        EntityCommon {
            handle: Handle::NULL,
            layer: "0".to_string(),
            color: Color::ByLayer,
            line_weight: LineWeight::ByLayer,
            transparency: Transparency::OPAQUE,
            invisible: false,
            extended_data: crate::xdata::ExtendedData::new(),
        }
    }

    /// Create with a specific layer
    pub fn with_layer(layer: impl Into<String>) -> Self {
        EntityCommon {
            layer: layer.into(),
            ..Self::new()
        }
    }
}

impl Default for EntityCommon {
    fn default() -> Self {
        Self::new()
    }
}

/// Enumeration of all entity types for type-safe storage
#[derive(Debug, Clone)]
pub enum EntityType {
    /// Point entity
    Point(Point),
    /// Line entity
    Line(Line),
    /// Circle entity
    Circle(Circle),
    /// Arc entity
    Arc(Arc),
    /// Ellipse entity
    Ellipse(Ellipse),
    /// 3D Polyline entity
    Polyline(Polyline),
    /// 2D Polyline entity (heavy polyline)
    Polyline2D(Polyline2D),
    /// 3D Polyline entity (new style)
    Polyline3D(Polyline3D),
    /// Lightweight polyline entity
    LwPolyline(LwPolyline),
    /// Text entity
    Text(Text),
    /// Multi-line text entity
    MText(MText),
    /// Spline entity
    Spline(Spline),
    /// Dimension entity
    Dimension(Dimension),
    /// Hatch entity
    Hatch(Hatch),
    /// Solid entity
    Solid(Solid),
    /// 3D Face entity
    Face3D(Face3D),
    /// Insert entity (block reference)
    Insert(Insert),
    /// Block entity (block definition start)
    Block(Block),
    /// BlockEnd entity (block definition end)
    BlockEnd(BlockEnd),
    /// Ray entity (semi-infinite line)
    Ray(Ray),
    /// XLine entity (construction line, infinite)
    XLine(XLine),
    /// Viewport entity (paper space viewport)
    Viewport(Viewport),
    /// Attribute definition entity
    AttributeDefinition(AttributeDefinition),
    /// Attribute entity (block attribute instance)
    AttributeEntity(AttributeEntity),
    /// Leader entity
    Leader(Leader),
    /// MultiLeader entity
    MultiLeader(MultiLeader),
    /// MLine (multiline) entity
    MLine(MLine),
    /// Mesh entity
    Mesh(Mesh),
    /// RasterImage entity
    RasterImage(RasterImage),
    /// Solid3D entity
    Solid3D(Solid3D),
    /// Region entity
    Region(Region),
    /// Body entity
    Body(Body),
    /// Table entity
    Table(Table),
    /// Tolerance entity (geometric tolerancing)
    Tolerance(Tolerance),
    /// PolyfaceMesh entity
    PolyfaceMesh(PolyfaceMesh),
    /// Wipeout entity
    Wipeout(Wipeout),
    /// Shape entity
    Shape(Shape),
    /// Underlay entity (PDF, DWF, DGN)
    Underlay(Underlay),
}

impl EntityType {
    /// Get a reference to the entity trait object
    pub fn as_entity(&self) -> &dyn Entity {
        match self {
            EntityType::Point(e) => e,
            EntityType::Line(e) => e,
            EntityType::Circle(e) => e,
            EntityType::Arc(e) => e,
            EntityType::Ellipse(e) => e,
            EntityType::Polyline(e) => e,
            EntityType::Polyline2D(e) => e,
            EntityType::Polyline3D(e) => e,
            EntityType::LwPolyline(e) => e,
            EntityType::Text(e) => e,
            EntityType::MText(e) => e,
            EntityType::Spline(e) => e,
            EntityType::Dimension(e) => e,
            EntityType::Hatch(e) => e,
            EntityType::Solid(e) => e,
            EntityType::Face3D(e) => e,
            EntityType::Insert(e) => e,
            EntityType::Block(e) => e,
            EntityType::BlockEnd(e) => e,
            EntityType::Ray(e) => e,
            EntityType::XLine(e) => e,
            EntityType::Viewport(e) => e,
            EntityType::AttributeDefinition(e) => e,
            EntityType::AttributeEntity(e) => e,
            EntityType::Leader(e) => e,
            EntityType::MultiLeader(e) => e,
            EntityType::MLine(e) => e,
            EntityType::Mesh(e) => e,
            EntityType::RasterImage(e) => e,
            EntityType::Solid3D(e) => e,
            EntityType::Region(e) => e,
            EntityType::Body(e) => e,
            EntityType::Table(e) => e,
            EntityType::Tolerance(e) => e,
            EntityType::PolyfaceMesh(e) => e,
            EntityType::Wipeout(e) => e,
            EntityType::Shape(e) => e,
            EntityType::Underlay(e) => e,
        }
    }

    /// Get a mutable reference to the entity trait object
    pub fn as_entity_mut(&mut self) -> &mut dyn Entity {
        match self {
            EntityType::Point(e) => e,
            EntityType::Line(e) => e,
            EntityType::Circle(e) => e,
            EntityType::Arc(e) => e,
            EntityType::Ellipse(e) => e,
            EntityType::Polyline(e) => e,
            EntityType::Polyline2D(e) => e,
            EntityType::Polyline3D(e) => e,
            EntityType::LwPolyline(e) => e,
            EntityType::MText(e) => e,
            EntityType::Text(e) => e,
            EntityType::Spline(e) => e,
            EntityType::Dimension(e) => e,
            EntityType::Hatch(e) => e,
            EntityType::Solid(e) => e,
            EntityType::Face3D(e) => e,
            EntityType::Insert(e) => e,
            EntityType::Block(e) => e,
            EntityType::BlockEnd(e) => e,
            EntityType::Ray(e) => e,
            EntityType::XLine(e) => e,
            EntityType::Viewport(e) => e,
            EntityType::AttributeDefinition(e) => e,
            EntityType::AttributeEntity(e) => e,
            EntityType::Leader(e) => e,
            EntityType::MultiLeader(e) => e,
            EntityType::MLine(e) => e,
            EntityType::Mesh(e) => e,
            EntityType::RasterImage(e) => e,
            EntityType::Solid3D(e) => e,
            EntityType::Region(e) => e,
            EntityType::Body(e) => e,
            EntityType::Table(e) => e,
            EntityType::Tolerance(e) => e,
            EntityType::PolyfaceMesh(e) => e,
            EntityType::Wipeout(e) => e,
            EntityType::Shape(e) => e,
            EntityType::Underlay(e) => e,
        }
    }
}

