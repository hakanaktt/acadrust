//! Graphical entity types.
//!
//! This module contains all 41 supported CAD entity types — from simple
//! primitives ([`Line`], [`Circle`], [`Arc`]) through complex objects
//! ([`Hatch`], [`Spline`], [`MultiLeader`], [`Mesh`]).
//!
//! Every entity carries [`EntityCommon`] data (layer, color, line weight,
//! handle, etc.) alongside its type-specific fields.
//!
//! Entities are stored in [`CadDocument`](crate::document::CadDocument) and
//! wrapped in the [`EntityType`] enum for heterogeneous collections.

use crate::types::{BoundingBox3D, Color, Handle, LineWeight, Transform, Transparency, Vector3};

pub mod acis;
pub mod arc;
pub mod attribute_definition;
pub mod attribute_entity;
pub mod block;
pub mod centerline;
pub mod circle;
pub mod dimension;
pub mod ellipse;
pub mod embedded_entity;
pub mod extended_entity;
pub mod face3d;
pub mod hatch;
pub mod helix;
pub mod insert;
pub mod leader;
pub mod light;
pub mod line;
pub mod lwpolyline;
pub mod mesh;
pub mod mline;
pub mod modification;
pub mod mtext;
pub mod mtext_format;
pub mod multileader;
pub mod ole2frame;
pub mod ole_presentation;
pub mod point;
pub mod polyface_mesh;
pub mod polygon_mesh;
pub mod polyline;
pub mod polyline3d;
pub mod proxy_graphics;
pub mod raster_image;
pub mod ray;
pub mod section_symbol;
pub mod seqend;
pub mod shape;
pub mod solid;
pub mod solid3d;
pub mod spline;
pub mod surface;
pub mod table;
pub mod text;
pub mod tolerance;
pub mod underlay;
pub mod unknown_entity;
pub mod view_border;
pub mod viewport;
pub mod wipeout;
pub mod xline;

// Preserve existing module paths for callers of the modification tools.
pub use modification::mirror::MirrorOptions;
pub use modification::mirror3d::MirrorPlane;
pub use modification::rotate::RotationAxis;
pub use modification::{explode, mirror, mirror3d, rotate, rotate3d, transform, translate};

pub use arc::Arc;
pub use attribute_definition::{
    AttributeDefinition, AttributeFlags, HorizontalAlignment, MTextFlag, VerticalAlignment,
};
pub use attribute_entity::AttributeEntity;
pub use block::{Block, BlockEnd};
pub use centerline::{
    CenterLineAssociation, CenterLineSource, CenterLineSourceKind, CenterMarkAssociation,
    CenterMarkSource, CenterMarkSourceKind, CENTERLINE_XDATA_APPLICATION,
    CENTERMARK_XDATA_APPLICATION,
};
pub use circle::Circle;
pub use dimension::*;
pub use ellipse::Ellipse;
pub use embedded_entity::EmbeddedEntity;
pub use extended_entity::*;
pub use face3d::{Face3D, InvisibleEdgeFlags};
pub use hatch::*;
pub use helix::{Helix, HelixConstraint};
pub use insert::Insert;
pub use leader::{HooklineDirection, Leader, LeaderCreationType, LeaderPathType};
pub use light::{Light, LightPhotometricData};
pub use line::Line;
pub use lwpolyline::{LwPolyline, LwVertex};
pub use mesh::{Mesh, MeshBuilder, MeshEdge, MeshFace};
pub use mline::{
    MLine, MLineBuilder, MLineFlags, MLineJustification, MLineSegment, MLineStyle,
    MLineStyleElement, MLineStyleFlags, MLineVertex,
};
pub use mtext::{AttachmentPoint, DrawingDirection, MText, MTextColumnData};
pub use multileader::{
    BlockAttribute, BlockContentConnectionType, FlowDirectionType, LeaderContentType, LeaderLine,
    LeaderLinePropertyOverrideFlags, LeaderRoot, LineSpacingStyle, MultiLeader,
    MultiLeaderAnnotContext, MultiLeaderArrowheadOverride, MultiLeaderBuilder, MultiLeaderPathType,
    MultiLeaderPropertyOverrideFlags, StartEndPointPair, TextAlignmentType, TextAngleType,
    TextAttachmentDirectionType, TextAttachmentPointType, TextAttachmentType,
};
pub use ole2frame::{Ole2Frame, OleFrameEnvelope, OleObjectType};
pub use ole_presentation::{extract_presentation, ole_user_type, OlePresentation};
pub use point::Point;
pub use polyface_mesh::{
    PolyfaceFace, PolyfaceMesh, PolyfaceMeshFlags, PolyfaceSmoothType, PolyfaceVertex,
    PolyfaceVertexFlags,
};
pub use polygon_mesh::{
    PolygonMesh as PolygonMeshEntity, PolygonMeshFlags, PolygonMeshVertex, SurfaceSmoothType,
};
pub use polyline::{
    Polyline, Polyline2D, PolylineFlags, SmoothSurfaceType, Vertex2D, Vertex3D, VertexFlags,
};
pub use polyline3d::{Polyline3D, Polyline3DFlags, Vertex3DPolyline};
pub use proxy_graphics::{ProxyGraphicRecord, ProxyGraphics, ProxyUnicodeText};
pub use raster_image::{
    ClipBoundary, ClipMode, ClipType, ImageDefinition, ImageDisplayFlags, ImageDisplayQuality,
    RasterImage, RasterImageBuilder, ResolutionUnit,
};
pub use ray::Ray;
pub use section_symbol::{SectionSymbol, SectionSymbolPoint, SectionViewStyle};
pub use seqend::Seqend;
pub use shape::{gdt_shapes, standard_shapes, Shape};
pub use solid::Solid;
pub use solid3d::{
    AcisData, AcisMaterial, AcisVersion, Body, Region, Silhouette, Solid3D, Wire, WireType,
};
pub use spline::{Spline, SplineFlags};
pub use surface::{Surface, SurfaceData, SurfaceKind, SurfaceSweepOptions};
pub use table::{
    BorderPropertyFlags, BorderType, BreakFlowDirection, BreakOptionFlags, CellBorder, CellContent,
    CellContentGeometry, CellEdgeFlags, CellRange, CellStateFlags, CellStyle,
    CellStylePropertyFlags, CellStyleType, CellType, CellValue, CellValueType, ContentLayoutFlags,
    LegacyBorderOverrides, LegacyTableStyleOverride, Table, TableAttribute, TableBreakData,
    TableBreakRange, TableBuilder, TableCell, TableCellContentType, TableColumn, TableCustomData,
    TableRow, ValueUnitType,
};
pub use text::{Text, TextHorizontalAlignment, TextVerticalAlignment};
pub use tolerance::{gdt_symbols, Tolerance};
pub use underlay::{
    DgnUnderlay, DgnUnderlayDefinition, DwfUnderlay, DwfUnderlayDefinition, PdfUnderlay,
    PdfUnderlayDefinition, Underlay, UnderlayDefinition, UnderlayDisplayFlags, UnderlayType,
};
pub use unknown_entity::UnknownEntity;
pub use view_border::ViewBorder;
pub use viewport::{GridFlags, StandardView, Viewport, ViewportRenderMode, ViewportStatusFlags};
pub use wipeout::{Wipeout, WipeoutClipMode, WipeoutClipType, WipeoutDisplayFlags};
pub use xline::XLine;

/// Base trait for all CAD entities.
///
/// Common fields live on [`EntityCommon`]. Implement [`Self::common`] and
/// [`Self::common_mut`]; the handle, layer, color, line weight, transparency,
/// and visibility methods forward to those.
pub trait Entity {
    /// Common entity fields.
    fn common(&self) -> &EntityCommon;

    /// Mutable common entity fields.
    fn common_mut(&mut self) -> &mut EntityCommon;

    /// Get the entity's unique handle
    fn handle(&self) -> Handle {
        self.common().handle
    }

    /// Set the entity's handle
    fn set_handle(&mut self, handle: Handle) {
        self.common_mut().handle = handle;
    }

    /// Get the entity's layer name
    fn layer(&self) -> &str {
        &self.common().layer
    }

    /// Set the entity's layer name
    fn set_layer(&mut self, layer: String) {
        self.common_mut().layer = layer;
    }

    /// Get the entity's color
    fn color(&self) -> Color {
        self.common().color
    }

    /// Set the entity's color
    fn set_color(&mut self, color: Color) {
        self.common_mut().color = color;
    }

    /// Get the entity's line weight
    fn line_weight(&self) -> LineWeight {
        self.common().line_weight
    }

    /// Set the entity's line weight
    fn set_line_weight(&mut self, weight: LineWeight) {
        self.common_mut().line_weight = weight;
    }

    /// Get the entity's transparency
    fn transparency(&self) -> Transparency {
        self.common().transparency
    }

    /// Set the entity's transparency
    fn set_transparency(&mut self, transparency: Transparency) {
        self.common_mut().transparency = transparency;
    }

    /// Check if the entity is invisible
    fn is_invisible(&self) -> bool {
        self.common().invisible
    }

    /// Set the entity's visibility
    fn set_invisible(&mut self, invisible: bool) {
        self.common_mut().invisible = invisible;
    }

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

    /// Rotate around `center` in the world XY plane.
    fn rotate(&mut self, center: Vector3, angle: f64) {
        self.rotate_about_axis(center, Vector3::UNIT_Z, angle);
    }

    /// Rotate around the world-space line through `center` parallel to `axis`.
    fn rotate_about_axis(&mut self, center: Vector3, axis: Vector3, angle: f64) {
        if !center.x.is_finite()
            || !center.y.is_finite()
            || !center.z.is_finite()
            || !axis.x.is_finite()
            || !axis.y.is_finite()
            || !axis.z.is_finite()
            || !angle.is_finite()
            || axis.length_squared() <= f64::EPSILON
            || angle == 0.0
        {
            return;
        }
        self.apply_transform(&Transform::from_rotation_with_origin(axis, angle, center));
    }

    /// Rotate around the 3D line passing through `first` and `second`.
    fn rotate_about_line(&mut self, first: Vector3, second: Vector3, angle: f64) {
        let axis = second - first;
        if first.x.is_finite()
            && first.y.is_finite()
            && first.z.is_finite()
            && second.x.is_finite()
            && second.y.is_finite()
            && second.z.is_finite()
            && angle.is_finite()
            && axis.length_squared() > f64::EPSILON
        {
            self.rotate_about_axis(first, axis, angle);
        }
    }

    /// Rotate using one of the CAD axis-selection modes.
    fn rotate_with_axis(&mut self, axis: crate::entities::rotate::RotationAxis, angle: f64) {
        match axis {
            crate::entities::rotate::RotationAxis::TwoPoint { first, second } => {
                self.rotate_about_line(first, second, angle)
            }
            crate::entities::rotate::RotationAxis::Entity { origin, direction }
            | crate::entities::rotate::RotationAxis::Last { origin, direction }
            | crate::entities::rotate::RotationAxis::View { origin, direction } => {
                self.rotate_about_axis(origin, direction, angle)
            }
            crate::entities::rotate::RotationAxis::XAxis { origin } => {
                self.rotate_about_axis(origin, Vector3::UNIT_X, angle)
            }
            crate::entities::rotate::RotationAxis::YAxis { origin } => {
                self.rotate_about_axis(origin, Vector3::UNIT_Y, angle)
            }
            crate::entities::rotate::RotationAxis::ZAxis { origin } => {
                self.rotate_about_axis(origin, Vector3::UNIT_Z, angle)
            }
        }
    }

    /// Apply the 3D CAD rotation axis selector.
    fn rotate3d(&mut self, axis: crate::entities::rotate::RotationAxis, angle: f64) {
        self.rotate_with_axis(axis, angle);
    }

    /// Apply a rotation around `axis` through `origin`.
    fn apply_rotation_with_origin(&mut self, axis: Vector3, angle: f64, origin: Vector3) {
        self.rotate_about_axis(origin, axis, angle);
    }

    /// Rotate from a reference point to a target point around `center` in the
    /// world XY plane.
    fn rotate_by_points(&mut self, center: Vector3, reference: Vector3, target: Vector3) {
        if let Some(angle) =
            crate::entities::rotate::angle_from_points(center, Vector3::UNIT_Z, reference, target)
        {
            self.rotate_about_axis(center, Vector3::UNIT_Z, angle);
        }
    }

    /// Rotate from a reference point to a target point around an arbitrary axis.
    fn rotate_about_axis_by_points(
        &mut self,
        center: Vector3,
        axis: Vector3,
        reference: Vector3,
        target: Vector3,
    ) {
        if let Some(angle) =
            crate::entities::rotate::angle_from_points(center, axis, reference, target)
        {
            self.rotate_about_axis(center, axis, angle);
        }
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

    /// Apply a mirror transform with entity-specific corrections
    ///
    /// Override this for entities that need post-processing after mirroring
    /// (e.g., arc angle swaps, bulge negation, face winding reversal).
    fn apply_mirror(&mut self, transform: &Transform) {
        if mirror::valid_transform(transform) {
            self.apply_transform(transform);
        }
    }

    /// Apply a reflection with explicit text and hatch policies.
    fn apply_mirror_with_options(&mut self, transform: &Transform, _options: MirrorOptions) {
        self.apply_mirror(transform);
    }

    /// Mirror the entity across the YZ plane (negate X coordinates)
    fn mirror_x(&mut self) {
        self.apply_mirror(&Transform::from_mirror_x());
    }

    /// Mirror the entity across the XZ plane (negate Y coordinates)
    fn mirror_y(&mut self) {
        self.apply_mirror(&Transform::from_mirror_y());
    }

    /// Mirror the entity across the XY plane (negate Z coordinates)
    fn mirror_z(&mut self) {
        self.apply_mirror(&Transform::from_mirror_z());
    }

    /// Mirror the entity across a line defined by two points (in the XY plane)
    fn mirror_about_line(&mut self, p1: Vector3, p2: Vector3) {
        self.mirror(p1, p2);
    }

    /// Planar MIRROR through the points' XY projections, preserving height.
    fn mirror(&mut self, first: Vector3, second: Vector3) {
        self.mirror_with_options(first, second, MirrorOptions::default());
    }

    /// Planar MIRROR with explicit text and hatch policies.
    fn mirror_with_options(&mut self, first: Vector3, second: Vector3, options: MirrorOptions) {
        self.apply_mirror_with_options(&Transform::from_mirror_line(first, second), options);
    }

    /// MIRROR3D across the selected world-space plane.
    fn mirror3d(&mut self, plane: MirrorPlane) {
        self.mirror3d_with_options(plane, MirrorOptions::default());
    }

    /// MIRROR3D with explicit text and hatch policies.
    fn mirror3d_with_options(&mut self, plane: MirrorPlane, options: MirrorOptions) {
        self.apply_mirror_with_options(&plane.transform(), options);
    }

    /// Mirror the entity across an arbitrary plane
    fn mirror_about_plane(&mut self, point: Vector3, normal: Vector3) {
        self.mirror3d(MirrorPlane::PointNormal { point, normal });
    }
}

/// Verbatim DWG record bytes of an entity (see [`EntityCommon::raw_record`]).
#[derive(Debug, Clone, PartialEq)]
pub struct RawRecord {
    /// Payload between the ModularShort length prefix and the CRC-16 trailer.
    pub data: Vec<u8>,
    /// R2010+ MC handle-stream size framing value.
    pub handle_bits: i64,
    /// Version the bytes were encoded for; passthrough is only valid for this exact target.
    pub version: crate::types::DxfVersion,
}

/// Native AC1009 fields which do not have a corresponding DXF entity field.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct R12EntityMetadata {
    /// Unsigned native viewport reference, including an explicitly stored zero.
    pub viewport: Option<u16>,
    /// Viewport references of ordered VERTEX/ATTRIB children followed by SEQEND.
    /// The final entry belongs to SEQEND and remains last when children are added.
    pub child_viewports: Vec<Option<u16>>,
}

/// Common entity data shared by all entities.
///
/// With the `serde` feature, native references and flags are retained alongside
/// the public appearance fields. Older field-named input (such as JSON) without
/// these fields defaults optional references and `entity_mode` to `None`, and
/// flags to zero. Positional formats require a separate compatibility strategy.
/// Raw DWG records, graphic bytes and the derived data-store bit remain excluded.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityCommon {
    /// Unique handle
    pub handle: Handle,
    /// Layer name
    pub layer: String,
    /// DWG layer handle as read from the file. Preserved so consumers can tell
    /// an entity on layer "0" from one whose layer reference did not resolve
    /// (the DWG reader falls back to `layer = "0"` in that case).
    #[cfg_attr(feature = "serde", serde(default))]
    pub layer_handle: Option<Handle>,
    /// Color
    pub color: Color,
    /// Line weight
    pub line_weight: LineWeight,
    /// Linetype name (empty string = "ByLayer")
    pub linetype: String,
    /// DWG linetype handle. Preserved so R13/R14 entities can round-trip
    /// non-ByLayer linetypes even when the table name cannot be resolved.
    #[cfg_attr(feature = "serde", serde(default))]
    pub linetype_handle: Option<Handle>,
    /// Linetype scale factor (default 1.0)
    pub linetype_scale: f64,
    /// Transparency
    #[cfg_attr(feature = "serde", serde(default))]
    pub transparency: Transparency,
    /// Named/color-book identity from DXF group code 430.
    #[cfg_attr(feature = "serde", serde(default))]
    pub color_name: Option<String>,
    /// Visibility flag
    pub invisible: bool,
    /// Extended data (XDATA)
    pub extended_data: crate::xdata::ExtendedData,
    /// Raw entity graphic data bytes (stored for DWG round-trip; None otherwise).
    #[cfg_attr(feature = "serde", serde(skip))]
    pub graphic_data: Option<Vec<u8>>,
    /// Reactor handles — objects attached as reactors ({ACAD_REACTORS})
    pub reactors: Vec<Handle>,
    /// Extended dictionary handle ({ACAD_XDICTIONARY}) — hard-owner handle to a Dictionary
    pub xdictionary_handle: Option<Handle>,
    /// Owner handle (soft pointer, code 330)
    pub owner_handle: Handle,

    // ── Native reference/round-trip fields ──
    /// AcDbColor object handle for a color-book color — R2004+.
    #[cfg_attr(feature = "serde", serde(default))]
    pub color_book_handle: Option<Handle>,
    /// Full visual-style override handle — R2010+; DXF code 348.
    pub full_visual_style_handle: Option<Handle>,
    /// Face visual-style override handle — R2010+.
    #[cfg_attr(feature = "serde", serde(default))]
    pub face_visual_style_handle: Option<Handle>,
    /// Edge visual-style override handle — R2010+.
    #[cfg_attr(feature = "serde", serde(default))]
    pub edge_visual_style_handle: Option<Handle>,
    /// Material flags (BB: 00=bylayer, 01=byblock, 10=reserved, 11=handle) — R2007+
    #[cfg_attr(feature = "serde", serde(default))]
    pub material_flags: u8,
    /// Material handle (only valid when material_flags == 0b11) — R2007+
    #[cfg_attr(feature = "serde", serde(default))]
    pub material_handle: Option<Handle>,
    /// Shadow flags (RC) — R2007+
    #[cfg_attr(feature = "serde", serde(default))]
    pub shadow_flags: u8,
    /// Plotstyle flags (BB: 00=bylayer, 01=byblock, 10=reserved, 11=handle) — R2000+
    #[cfg_attr(feature = "serde", serde(default))]
    pub plotstyle_flags: u8,
    /// Plotstyle handle (only valid when plotstyle_flags == 0b11) — R2000+
    #[cfg_attr(feature = "serde", serde(default))]
    pub plotstyle_handle: Option<Handle>,
    /// Entity mode (0=owned, 1=paper, 2=model) — raw DWG value for round-trip
    #[cfg_attr(feature = "serde", serde(default))]
    pub entity_mode: Option<u8>,
    /// Native AC1009 viewport references retained across edits and DWG writes.
    #[cfg_attr(feature = "serde", serde(default))]
    pub r12_metadata: Option<R12EntityMetadata>,
    /// R2013+ `has_ds_data` bit: the entity's modeler geometry (3DSOLID/REGION/
    /// BODY/SURFACE) is stored as a SAB blob in the `AcDb:AcDsPrototype_1b`
    /// data-store section rather than inline. Set by the DWG reader; the writer
    /// re-derives it from the presence of ACIS data. Not part of the logical
    /// model, so skipped for serde.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub has_ds_data: bool,
    /// The exact merged-stream bytes this entity was decoded from (DWG only),
    /// with the framing the writer needs to re-emit them verbatim.
    /// Set by the DWG reader; cleared by every mutable access (`get_entity_mut`,
    /// `entities_mut`) and by `add_entity`, so it is only ever present on an
    /// entity that is byte-for-byte what the source file contained. The writer
    /// copies such records instead of re-encoding them (same target version only).
    #[cfg_attr(feature = "serde", serde(skip))]
    pub raw_record: Option<std::sync::Arc<RawRecord>>,
}

impl EntityCommon {
    fn preserve_storage_data_from(&mut self, source: &Self) {
        self.graphic_data = source.graphic_data.clone();
        self.r12_metadata = source.r12_metadata.clone();
        self.has_ds_data = source.has_ds_data;
    }

    /// Create new common entity data with defaults
    pub fn new() -> Self {
        EntityCommon {
            raw_record: None,
            handle: Handle::NULL,
            layer: "0".to_string(),
            layer_handle: None,
            color: Color::ByLayer,
            line_weight: LineWeight::ByLayer,
            linetype: String::new(),
            linetype_handle: None,
            linetype_scale: 1.0,
            transparency: Transparency::BY_LAYER,
            color_name: None,
            invisible: false,
            extended_data: crate::xdata::ExtendedData::new(),
            graphic_data: None,
            reactors: Vec::new(),
            xdictionary_handle: None,
            owner_handle: Handle::NULL,
            color_book_handle: None,
            full_visual_style_handle: None,
            face_visual_style_handle: None,
            edge_visual_style_handle: None,
            material_flags: 0,
            material_handle: None,
            shadow_flags: 0,
            plotstyle_flags: 0,
            plotstyle_handle: None,
            entity_mode: None,
            r12_metadata: None,
            has_ds_data: false,
        }
    }

    /// Create with a specific layer
    pub fn with_layer(layer: impl Into<String>) -> Self {
        EntityCommon {
            layer: layer.into(),
            ..Self::new()
        }
    }

    /// Check whether a linetype name is set (not empty and not "ByLayer")
    pub fn has_linetype(&self) -> bool {
        !self.linetype.is_empty() && self.linetype != "ByLayer"
    }
}

impl Default for EntityCommon {
    fn default() -> Self {
        Self::new()
    }
}

/// Enumeration of all entity types for type-safe storage
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    MText(Box<MText>),
    /// Spline entity
    Spline(Spline),
    /// Helix entity (spline-derived 3D spiral)
    Helix(Box<Helix>),
    /// Dimension entity
    Dimension(Box<Dimension>),
    /// Hatch entity
    Hatch(Box<Hatch>),
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
    Viewport(Box<Viewport>),
    /// Attribute definition entity
    AttributeDefinition(AttributeDefinition),
    /// Attribute entity (block attribute instance)
    AttributeEntity(AttributeEntity),
    /// Leader entity
    Leader(Leader),
    /// MultiLeader entity
    MultiLeader(Box<MultiLeader>),
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
    /// Surface entity (ACAD_SURFACE family: lofted/swept/extruded/etc.)
    Surface(Box<Surface>),
    /// Table entity
    Table(Box<Table>),
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
    /// End-of-sequence marker
    Seqend(Seqend),
    /// OLE2 embedded object
    Ole2Frame(Box<Ole2Frame>),
    /// Polygon mesh (3D surface mesh)
    PolygonMesh(PolygonMeshEntity),
    /// Light entity (point / spot / distant light source)
    Light(Box<Light>),
    SectionSymbol(SectionSymbol),
    ViewBorder(ViewBorder),
    /// Structured class-based and legacy entities.
    Extended(Box<ExtendedEntity>),
    /// Unknown / unsupported entity type (common fields only)
    Unknown(UnknownEntity),
}

impl EntityType {
    /// Restore storage-only data omitted from serde after editing the public
    /// representation of an entity. Serialized common references and flags keep
    /// their edited values, including `None` and zero. For legacy R12 editing,
    /// source `r12_metadata` is still restored even though it is serialized.
    /// The source `raw_record` replay cache is not restored.
    /// The source and destination must have the same variant and identity;
    /// callers remain responsible for that check.
    pub fn preserve_storage_data_from(&mut self, source: &Self) {
        self.common_mut()
            .preserve_storage_data_from(source.common());
        match (self, source) {
            (Self::Extended(value), Self::Extended(source)) => {
                value.data.preserve_storage_data_from(&source.data);
            }
            (Self::Unknown(value), Self::Unknown(source)) => {
                value.raw_dwg_data = source.raw_dwg_data.clone();
                value.raw_dxf_codes = source.raw_dxf_codes.clone();
                value.dwg_source_version = source.dwg_source_version;
            }
            _ => {}
        }
    }

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
            EntityType::MText(e) => e.as_ref(),
            EntityType::Spline(e) => e,
            EntityType::Helix(e) => e.as_ref(),
            EntityType::Dimension(e) => e.as_ref(),
            EntityType::Hatch(e) => e.as_ref(),
            EntityType::Solid(e) => e,
            EntityType::Face3D(e) => e,
            EntityType::Insert(e) => e,
            EntityType::Block(e) => e,
            EntityType::BlockEnd(e) => e,
            EntityType::Ray(e) => e,
            EntityType::XLine(e) => e,
            EntityType::Viewport(e) => e.as_ref(),
            EntityType::AttributeDefinition(e) => e,
            EntityType::AttributeEntity(e) => e,
            EntityType::Leader(e) => e,
            EntityType::MultiLeader(e) => e.as_ref(),
            EntityType::MLine(e) => e,
            EntityType::Mesh(e) => e,
            EntityType::RasterImage(e) => e,
            EntityType::Solid3D(e) => e,
            EntityType::Region(e) => e,
            EntityType::Body(e) => e,
            EntityType::Surface(e) => e.as_ref(),
            EntityType::Table(e) => e.as_ref(),
            EntityType::Tolerance(e) => e,
            EntityType::PolyfaceMesh(e) => e,
            EntityType::Wipeout(e) => e,
            EntityType::Shape(e) => e,
            EntityType::Underlay(e) => e,
            EntityType::Seqend(e) => e,
            EntityType::Ole2Frame(e) => e.as_ref(),
            EntityType::PolygonMesh(e) => e,
            EntityType::Light(e) => e.as_ref(),
            EntityType::SectionSymbol(e) => e,
            EntityType::ViewBorder(e) => e,
            EntityType::Extended(e) => e.as_ref(),
            EntityType::Unknown(e) => e,
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
            EntityType::MText(e) => e.as_mut(),
            EntityType::Text(e) => e,
            EntityType::Spline(e) => e,
            EntityType::Helix(e) => e.as_mut(),
            EntityType::Dimension(e) => e.as_mut(),
            EntityType::Hatch(e) => e.as_mut(),
            EntityType::Solid(e) => e,
            EntityType::Face3D(e) => e,
            EntityType::Insert(e) => e,
            EntityType::Block(e) => e,
            EntityType::BlockEnd(e) => e,
            EntityType::Ray(e) => e,
            EntityType::XLine(e) => e,
            EntityType::Viewport(e) => e.as_mut(),
            EntityType::AttributeDefinition(e) => e,
            EntityType::AttributeEntity(e) => e,
            EntityType::Leader(e) => e,
            EntityType::MultiLeader(e) => e.as_mut(),
            EntityType::MLine(e) => e,
            EntityType::Mesh(e) => e,
            EntityType::RasterImage(e) => e,
            EntityType::Solid3D(e) => e,
            EntityType::Region(e) => e,
            EntityType::Body(e) => e,
            EntityType::Surface(e) => e.as_mut(),
            EntityType::Table(e) => e.as_mut(),
            EntityType::Tolerance(e) => e,
            EntityType::PolyfaceMesh(e) => e,
            EntityType::Wipeout(e) => e,
            EntityType::Shape(e) => e,
            EntityType::Underlay(e) => e,
            EntityType::Seqend(e) => e,
            EntityType::Ole2Frame(e) => e.as_mut(),
            EntityType::PolygonMesh(e) => e,
            EntityType::Light(e) => e.as_mut(),
            EntityType::SectionSymbol(e) => e,
            EntityType::ViewBorder(e) => e,
            EntityType::Extended(e) => e.as_mut(),
            EntityType::Unknown(e) => e,
        }
    }

    /// Get a reference to the entity's common data
    pub fn common(&self) -> &EntityCommon {
        self.as_entity().common()
    }

    /// Get a mutable reference to the entity's common data
    pub fn common_mut(&mut self) -> &mut EntityCommon {
        self.as_entity_mut().common_mut()
    }
}

#[cfg(test)]
mod storage_data_tests {
    use super::*;

    #[test]
    fn public_record_edit_can_preserve_opaque_entity_data() {
        let mut source = UnknownEntity::new("CUSTOM_ENTITY");
        source.common.graphic_data = Some(vec![1, 2, 3]);
        source.raw_dwg_data = Some(vec![4, 5, 6]);
        source.raw_dxf_codes = Some(vec![(100, "payload".into())]);
        let source = EntityType::Unknown(source);

        let mut edited = source.clone();
        if let EntityType::Unknown(value) = &mut edited {
            value.common.layer = "Edited".into();
            value.common.graphic_data = None;
            value.raw_dwg_data = None;
            value.raw_dxf_codes = None;
        }
        edited.preserve_storage_data_from(&source);

        let EntityType::Unknown(edited) = edited else {
            unreachable!()
        };
        assert_eq!(edited.common.layer, "Edited");
        assert_eq!(edited.common.graphic_data, Some(vec![1, 2, 3]));
        assert_eq!(edited.raw_dwg_data, Some(vec![4, 5, 6]));
        assert_eq!(edited.raw_dxf_codes, Some(vec![(100, "payload".into())]));
    }

    #[test]
    fn entity_type_stays_narrow() {
        let size = std::mem::size_of::<EntityType>();
        assert!(size <= 856, "EntityType is {size} bytes");
    }
}
