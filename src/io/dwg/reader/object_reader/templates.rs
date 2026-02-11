//! DWG object templates — intermediate structures holding raw handle
//! values before resolution.
//!
//! Mirrors ACadSharp's `IO/Templates/` system.  The DWG object reader
//! creates a template for every object it reads.  After all objects
//! are read the builder resolves handle references and builds the
//! final `CadDocument`.

use std::collections::HashMap;

use crate::entities::EntityType;
use crate::types::{Color, Vector2, Vector3};
use crate::xdata::XDataValue;

// ---------------------------------------------------------------------------
// Extended Data template (EED)
// ---------------------------------------------------------------------------

/// A single extended-data record with its appid handle reference (unresolved).
#[derive(Debug, Clone)]
pub struct EDataTemplateEntry {
    /// Handle of the APPID object that registered the xdata.
    pub app_handle: u64,
    /// The xdata values.
    pub values: Vec<XDataValue>,
}

/// Container for extended-data read from a DWG object, keyed by appid handle.
#[derive(Debug, Clone, Default)]
pub struct EDataTemplate {
    pub entries: Vec<EDataTemplateEntry>,
}

impl EDataTemplate {
    pub fn add(&mut self, app_handle: u64, values: Vec<XDataValue>) {
        self.entries.push(EDataTemplateEntry { app_handle, values });
    }
}

// ---------------------------------------------------------------------------
// Common template header – shared by ALL templates
// ---------------------------------------------------------------------------

/// Fields common to every DWG template (entity AND non-entity alike).
#[derive(Debug, Clone, Default)]
pub struct CadTemplateCommon {
    /// The object's own handle (read from the object).
    pub handle: u64,
    /// Owner handle (soft pointer to owning object).
    pub owner_handle: u64,
    /// Reactor handles (soft pointer).
    pub reactor_handles: Vec<u64>,
    /// Extended dictionary handle (hard owner), 0 if absent.
    pub xdict_handle: u64,
    /// Extended data template (EED).
    pub edata: EDataTemplate,
}

// ---------------------------------------------------------------------------
// Entity-specific template fields
// ---------------------------------------------------------------------------

/// Template fields specific to graphical entities.
#[derive(Debug, Clone, Default)]
pub struct CadEntityTemplateData {
    // ---- Common entity data ----
    /// Entity mode (BB): 0=owner handle present, 1=pspace, 2=mspace, 3=n/a
    pub entity_mode: u8,
    /// Previous entity handle (for linked-list traversal, R13-R2000).
    pub prev_entity: u64,
    /// Next entity handle (for linked-list traversal, R13-R2000).
    pub next_entity: u64,
    /// Layer handle (hard pointer).
    pub layer_handle: u64,
    /// Linetype handle (hard pointer), 0 if bylayer.
    pub linetype_handle: u64,
    /// Linetype flags (BB): 00=bylayer, 01=byblock, 10=continuous, 11=handle present.
    pub ltype_flags: u8,
    /// Plotstyle handle (hard pointer), 0 if bylayer.
    pub plotstyle_handle: u64,
    /// Material handle (R2007+), 0 if bylayer.
    pub material_handle: u64,
    /// Color book color handle (R2004+), 0 if none.
    pub color_handle: u64,
}

// ---------------------------------------------------------------------------
// Specialised entity templates
// ---------------------------------------------------------------------------

/// Template for text/attribute entities (holds style handle).
#[derive(Debug, Clone, Default)]
pub struct CadTextEntityTemplateData {
    pub style_handle: u64,
}

/// Template for INSERT / MINSERT entities.
#[derive(Debug, Clone, Default)]
pub struct CadInsertTemplateData {
    pub block_header_handle: u64,
    pub has_atts: bool,
    pub owned_objects_count: i32,
    pub first_attribute_handle: u64,
    pub end_attribute_handle: u64,
    pub owned_objects_handles: Vec<u64>,
    pub seqend_handle: u64,
}

/// Template for polyline entities (2D/3D/mesh/polyface).
#[derive(Debug, Clone, Default)]
pub struct CadPolylineTemplateData {
    pub first_vertex_handle: u64,
    pub last_vertex_handle: u64,
    pub owned_objects_handles: Vec<u64>,
    pub seqend_handle: u64,
}

/// Template for dimension entities.
#[derive(Debug, Clone, Default)]
pub struct CadDimensionTemplateData {
    pub style_handle: u64,
    pub block_handle: u64,
}

/// Template for leader entities.
#[derive(Debug, Clone, Default)]
pub struct CadLeaderTemplateData {
    pub annotation_handle: u64,
    pub dimstyle_handle: u64,
    pub dimasz: f64,
}

/// Template for multileader entities.
#[derive(Debug, Clone, Default)]
pub struct CadMultiLeaderTemplateData {
    pub leader_style_handle: u64,
    pub leader_line_type_handle: u64,
    pub arrowhead_handle: u64,
    pub mtext_style_handle: u64,
    pub block_content_handle: u64,
    pub arrowhead_handles: Vec<(u64, bool)>,
    pub block_attribute_handles: Vec<u64>,
    pub annot_context_data: CadMultiLeaderAnnotContextTemplateData,
}

/// Template data for a MultiLeader annotation context sub-object.
#[derive(Debug, Clone, Default)]
pub struct CadMultiLeaderAnnotContextTemplateData {
    pub mtext_style_handle: u64,
    pub block_table_handle: u64,
    pub arrowhead_handles: Vec<u64>,
    pub block_attribute_handles: Vec<u64>,
}

/// Template for shape entities.
#[derive(Debug, Clone, Default)]
pub struct CadShapeTemplateData {
    pub shape_file_handle: u64,
}

/// Template for viewport entities.
#[derive(Debug, Clone, Default)]
pub struct CadViewportTemplateData {
    pub viewport_header_handle: u64,
    pub frozen_layer_handles: Vec<u64>,
    pub boundary_handle: u64,
    pub named_ucs_handle: u64,
    pub base_ucs_handle: u64,
}

/// Template for spline entities.
#[derive(Debug, Clone, Default)]
pub struct CadSplineTemplateData {
    // No extra handles for spline, but keep for consistency.
}

/// Template for hatch entities.
#[derive(Debug, Clone, Default)]
pub struct CadHatchTemplateData {
    pub boundary_handles: Vec<Vec<u64>>,
}

/// Template for solid3d entities.
#[derive(Debug, Clone, Default)]
pub struct CadSolid3DTemplateData {
    pub history_handle: u64,
}

/// Template for raster image / wipeout entities.
#[derive(Debug, Clone, Default)]
pub struct CadImageTemplateData {
    pub img_def_handle: u64,
    pub img_def_reactor_handle: u64,
}

/// Template for polyface mesh entities.
#[derive(Debug, Clone, Default)]
pub struct CadPolyfaceMeshTemplateData {
    pub first_vertice_handle: u64,
    pub last_vertice_handle: u64,
    pub vertices_handles: Vec<u64>,
    pub seqend_handle: u64,
}

// ---------------------------------------------------------------------------
// Table-entry templates
// ---------------------------------------------------------------------------

/// Template for document table (control) objects.
#[derive(Debug, Clone, Default)]
pub struct CadTableTemplateData {
    /// Handles of entries owned by this table (soft owner).
    pub entry_handles: Vec<u64>,
}

/// Template for BLOCK_HEADER table entry.
#[derive(Debug, Clone, Default)]
pub struct CadBlockRecordTemplateData {
    pub first_entity_handle: u64,
    pub last_entity_handle: u64,
    pub owned_object_handles: Vec<u64>,
    pub insert_count: HashMap<u64, i32>,
    pub layout_handle: u64,
    pub block_entity_handle: u64,
    pub end_block_handle: u64,
}

/// Template for LAYER table entry.
#[derive(Debug, Clone, Default)]
pub struct CadLayerTemplateData {
    pub name: String,
    pub color: Color,
    pub is_on: bool,
    pub frozen: bool,
    pub locked: bool,
    pub is_plottable: bool,
    pub line_weight_raw: u8,
    pub linetype_handle: u64,
    pub plotstyle_handle: u64,
    pub material_handle: u64,
}

/// Template for LTYPE table entry.
#[derive(Debug, Clone, Default)]
pub struct CadLineTypeTemplateData {
    pub name: String,
    pub description: String,
    pub alignment: u8,
    pub dash_lengths: Vec<f64>,
    pub ltype_control_handle: u64,
    pub total_len: f64,
    pub segment_handles: Vec<u64>,
}

/// Template data for a single linetype segment.
#[derive(Debug, Clone, Default)]
pub struct CadLineTypeSegmentTemplateData {
    pub style_handle: u64,
}

/// Template for TEXT_STYLE table entry.
#[derive(Debug, Clone, Default)]
pub struct CadTextStyleTemplateData {
    pub name: String,
    pub height: f64,
    pub width_factor: f64,
    pub oblique_angle: f64,
    pub gen_flags: u8,
    pub font_file: String,
    pub big_font_file: String,
    pub is_vertical: bool,
    pub is_shape_file: bool,
    pub last_height: f64,
    pub style_control_handle: u64,
}

/// Template for APPID table entry.
#[derive(Debug, Clone, Default)]
pub struct CadAppIdTemplateData {
    pub name: String,
    pub appid_control_handle: u64,
}

/// Template for UCS table entry.
#[derive(Debug, Clone, Default)]
pub struct CadUcsTemplateData {
    pub name: String,
    pub origin: Vector3,
    pub x_dir: Vector3,
    pub y_dir: Vector3,
    pub ucs_control_handle: u64,
}

/// Template for DIMSTYLE table entry.
#[derive(Debug, Clone, Default)]
pub struct CadDimStyleTemplateData {
    pub name: String,
    pub dimtxsty_handle: u64,
    pub dimldrblk_handle: u64,
    pub dimblk_handle: u64,
    pub dimblk1_handle: u64,
    pub dimblk2_handle: u64,
    pub dimltype_handle: u64,
    pub dimltex1_handle: u64,
    pub dimltex2_handle: u64,
    pub dimblk_name: String,
    pub dimblk1_name: String,
    pub dimblk2_name: String,
}

/// Template for VIEW table entry.
#[derive(Debug, Clone, Default)]
pub struct CadViewTemplateData {
    pub name: String,
    pub height: f64,
    pub width: f64,
    pub center: Vector2,
    pub target: Vector3,
    pub direction: Vector3,
    pub twist_angle: f64,
    pub lens_length: f64,
    pub front_clip: f64,
    pub back_clip: f64,
    pub ucs_handle: u64,
    pub named_ucs_handle: u64,
}

/// Template for VPORT table entry.
#[derive(Debug, Clone, Default)]
pub struct CadVPortTemplateData {
    pub name: String,
    pub vport_control_handle: u64,
    pub background_handle: u64,
    pub style_handle: u64,
    pub sun_handle: u64,
    pub named_ucs_handle: u64,
    pub base_ucs_handle: u64,
}

// ---------------------------------------------------------------------------
// Non-graphical object templates
// ---------------------------------------------------------------------------

/// Template for DICTIONARY objects.
#[derive(Debug, Clone, Default)]
pub struct CadDictionaryTemplateData {
    /// Entry name → handle pairs (soft owner).
    pub entries: Vec<(String, u64)>,
}

/// Template for DICTIONARY_WITH_DEFAULT objects.
#[derive(Debug, Clone, Default)]
pub struct CadDictWithDefaultTemplateData {
    /// Base dictionary entries.
    pub dict_data: CadDictionaryTemplateData,
    /// Default entry handle (hard pointer).
    pub default_entry_handle: u64,
}

/// Template for LAYOUT objects.
#[derive(Debug, Clone, Default)]
pub struct CadLayoutTemplateData {
    pub plot_view_handle: u64,
    pub visual_style_handle: u64,
    pub associated_tab_handle: u64,
    pub viewport_handle: u64,
    pub base_ucs_handle: u64,
    pub named_ucs_handle: u64,
    pub block_record_handle: u64,
}

/// Template for GROUP objects.
#[derive(Debug, Clone, Default)]
pub struct CadGroupTemplateData {
    pub entity_handles: Vec<u64>,
}

/// Template for MLINESTYLE objects.
#[derive(Debug, Clone, Default)]
pub struct CadMLineStyleTemplateData {
    pub element_linetype_handles: Vec<u64>,
}

/// Template for XRECORD objects.
#[derive(Debug, Clone, Default)]
pub struct CadXRecordTemplateData {
    // No extra handles; xrecord data is self-contained.
}

/// Template for IMAGEDEF objects.
#[derive(Debug, Clone, Default)]
pub struct CadImageDefTemplateData {
    // Reactor handles stored in common.reactor_handles.
}

/// Template for IMAGEDEF_REACTOR objects.
#[derive(Debug, Clone, Default)]
pub struct CadImageDefReactorTemplateData {
    pub image_handle: u64,
}

/// Template for SORTENTSTABLE objects.
#[derive(Debug, Clone, Default)]
pub struct CadSortEntsTableTemplateData {
    pub block_owner_handle: u64,
    pub sort_handle_pairs: Vec<(u64, u64)>,
}

/// Template for SCALE objects.
#[derive(Debug, Clone, Default)]
pub struct CadScaleTemplateData {
    // No extra handles.
}

/// Template for MLEADERSTYLE objects.
#[derive(Debug, Clone, Default)]
pub struct CadMLeaderStyleTemplateData {
    pub leader_line_type_handle: u64,
    pub arrowhead_handle: u64,
    pub mtext_style_handle: u64,
    pub block_content_handle: u64,
}

/// Template for PLOTSETTINGS objects.
#[derive(Debug, Clone, Default)]
pub struct CadPlotSettingsTemplateData {
    // Plot settings data is self-contained after reading.
}

/// Template data for TABLESTYLE objects.
#[derive(Debug, Clone, Default)]
pub struct CadTableStyleTemplateData {
    // No extra handles.
}

// ---------------------------------------------------------------------------
// The unified CadTemplate enum
// ---------------------------------------------------------------------------

/// A template wrapping an entity, table entry, or non-graphical object
/// with its unresolved handle references.
///
/// After the DWG object reader finishes reading, the document builder
/// iterates over all templates, resolves handle references, and
/// constructs the final `CadDocument`.
#[derive(Debug, Clone)]
pub enum CadTemplate {
    // ---- Entities ----
    Entity {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        entity: EntityType,
    },
    TextEntity {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        text_data: CadTextEntityTemplateData,
        entity: EntityType,
    },
    Insert {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        insert_data: CadInsertTemplateData,
        entity: EntityType,
    },
    Polyline {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        polyline_data: CadPolylineTemplateData,
        entity: EntityType,
    },
    Dimension {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        dim_data: CadDimensionTemplateData,
        entity: EntityType,
    },
    Leader {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        leader_data: CadLeaderTemplateData,
        entity: EntityType,
    },
    MultiLeader {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        mleader_data: CadMultiLeaderTemplateData,
        entity: EntityType,
    },
    Shape {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        shape_data: CadShapeTemplateData,
        entity: EntityType,
    },
    Viewport {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        viewport_data: CadViewportTemplateData,
        entity: EntityType,
    },
    Hatch {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        hatch_data: CadHatchTemplateData,
        entity: EntityType,
    },
    Solid3D {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        solid3d_data: CadSolid3DTemplateData,
        entity: EntityType,
    },
    Image {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        image_data: CadImageTemplateData,
        entity: EntityType,
    },
    PolyfaceMesh {
        common: CadTemplateCommon,
        entity_data: CadEntityTemplateData,
        polyface_data: CadPolyfaceMeshTemplateData,
        entity: EntityType,
    },

    // ---- Table control objects ----
    TableControl {
        common: CadTemplateCommon,
        table_data: CadTableTemplateData,
        table_type: TableControlType,
    },

    // ---- Table entries ----
    BlockHeader {
        common: CadTemplateCommon,
        block_data: CadBlockRecordTemplateData,
    },
    LayerEntry {
        common: CadTemplateCommon,
        layer_data: CadLayerTemplateData,
    },
    LineTypeEntry {
        common: CadTemplateCommon,
        ltype_data: CadLineTypeTemplateData,
    },
    DimStyleEntry {
        common: CadTemplateCommon,
        dimstyle_data: CadDimStyleTemplateData,
    },
    ViewEntry {
        common: CadTemplateCommon,
        view_data: CadViewTemplateData,
    },
    VPortEntry {
        common: CadTemplateCommon,
        vport_data: CadVPortTemplateData,
    },
    TextStyleEntry {
        common: CadTemplateCommon,
        textstyle_data: CadTextStyleTemplateData,
    },
    AppIdEntry {
        common: CadTemplateCommon,
        appid_data: CadAppIdTemplateData,
    },
    UcsEntry {
        common: CadTemplateCommon,
        ucs_data: CadUcsTemplateData,
    },
    /// Generic table entry (VP_ENT_HDR, etc.)
    GenericTableEntry {
        common: CadTemplateCommon,
    },

    // ---- Non-graphical objects ----
    DictionaryObj {
        common: CadTemplateCommon,
        dict_data: CadDictionaryTemplateData,
    },
    DictWithDefault {
        common: CadTemplateCommon,
        dict_default_data: CadDictWithDefaultTemplateData,
    },
    LayoutObj {
        common: CadTemplateCommon,
        layout_data: CadLayoutTemplateData,
    },
    GroupObj {
        common: CadTemplateCommon,
        group_data: CadGroupTemplateData,
    },
    MLineStyleObj {
        common: CadTemplateCommon,
        mls_data: CadMLineStyleTemplateData,
    },
    ImageDefObj {
        common: CadTemplateCommon,
        imgdef_data: CadImageDefTemplateData,
    },
    ImageDefReactorObj {
        common: CadTemplateCommon,
        reactor_data: CadImageDefReactorTemplateData,
    },
    SortEntsTableObj {
        common: CadTemplateCommon,
        sort_data: CadSortEntsTableTemplateData,
    },
    MLeaderStyleObj {
        common: CadTemplateCommon,
        mls_style_data: CadMLeaderStyleTemplateData,
    },
    PlotSettingsObj {
        common: CadTemplateCommon,
    },
    ScaleObj {
        common: CadTemplateCommon,
    },
    XRecordObj {
        common: CadTemplateCommon,
    },
    /// Dictionary variable, plain object template, etc.
    GenericObject {
        common: CadTemplateCommon,
    },
}

impl CadTemplate {
    /// Access the common template data.
    pub fn common(&self) -> &CadTemplateCommon {
        match self {
            CadTemplate::Entity { common, .. }
            | CadTemplate::TextEntity { common, .. }
            | CadTemplate::Insert { common, .. }
            | CadTemplate::Polyline { common, .. }
            | CadTemplate::Dimension { common, .. }
            | CadTemplate::Leader { common, .. }
            | CadTemplate::MultiLeader { common, .. }
            | CadTemplate::Shape { common, .. }
            | CadTemplate::Viewport { common, .. }
            | CadTemplate::Hatch { common, .. }
            | CadTemplate::Solid3D { common, .. }
            | CadTemplate::Image { common, .. }
            | CadTemplate::PolyfaceMesh { common, .. }
            | CadTemplate::TableControl { common, .. }
            | CadTemplate::BlockHeader { common, .. }
            | CadTemplate::LayerEntry { common, .. }
            | CadTemplate::LineTypeEntry { common, .. }
            | CadTemplate::DimStyleEntry { common, .. }
            | CadTemplate::ViewEntry { common, .. }
            | CadTemplate::VPortEntry { common, .. }
            | CadTemplate::TextStyleEntry { common, .. }
            | CadTemplate::AppIdEntry { common, .. }
            | CadTemplate::UcsEntry { common, .. }
            | CadTemplate::GenericTableEntry { common, .. }
            | CadTemplate::DictionaryObj { common, .. }
            | CadTemplate::DictWithDefault { common, .. }
            | CadTemplate::LayoutObj { common, .. }
            | CadTemplate::GroupObj { common, .. }
            | CadTemplate::MLineStyleObj { common, .. }
            | CadTemplate::ImageDefObj { common, .. }
            | CadTemplate::ImageDefReactorObj { common, .. }
            | CadTemplate::SortEntsTableObj { common, .. }
            | CadTemplate::MLeaderStyleObj { common, .. }
            | CadTemplate::PlotSettingsObj { common, .. }
            | CadTemplate::ScaleObj { common, .. }
            | CadTemplate::XRecordObj { common, .. }
            | CadTemplate::GenericObject { common, .. } => common,
        }
    }

    /// The handle of the object that this template represents.
    pub fn handle(&self) -> u64 {
        self.common().handle
    }

    /// Collect all handle references from this template for enqueuing.
    ///
    /// Returns handles from the common data (owner, reactors, xdict),
    /// entity data (layer, ltype, prev/next), and type-specific data
    /// (table entry handles, dictionary entries, block record entities, etc.).
    pub fn all_handles(&self) -> Vec<u64> {
        let mut handles = Vec::new();
        let c = self.common();
        handles.push(c.owner_handle);
        handles.extend_from_slice(&c.reactor_handles);
        handles.push(c.xdict_handle);

        // Entity data handles.
        match self {
            CadTemplate::Entity { entity_data, .. }
            | CadTemplate::TextEntity { entity_data, .. }
            | CadTemplate::Insert { entity_data, .. }
            | CadTemplate::Polyline { entity_data, .. }
            | CadTemplate::Dimension { entity_data, .. }
            | CadTemplate::Leader { entity_data, .. }
            | CadTemplate::MultiLeader { entity_data, .. }
            | CadTemplate::Shape { entity_data, .. }
            | CadTemplate::Viewport { entity_data, .. }
            | CadTemplate::Hatch { entity_data, .. }
            | CadTemplate::Solid3D { entity_data, .. }
            | CadTemplate::Image { entity_data, .. }
            | CadTemplate::PolyfaceMesh { entity_data, .. } => {
                handles.push(entity_data.prev_entity);
                handles.push(entity_data.next_entity);
                handles.push(entity_data.layer_handle);
                handles.push(entity_data.linetype_handle);
                handles.push(entity_data.plotstyle_handle);
                handles.push(entity_data.material_handle);
                handles.push(entity_data.color_handle);
            }
            _ => {}
        }

        // Type-specific handles.
        match self {
            CadTemplate::TableControl { table_data, .. } => {
                handles.extend_from_slice(&table_data.entry_handles);
            }
            CadTemplate::BlockHeader { block_data, .. } => {
                handles.push(block_data.first_entity_handle);
                handles.push(block_data.last_entity_handle);
                handles.extend_from_slice(&block_data.owned_object_handles);
                handles.push(block_data.layout_handle);
                handles.push(block_data.block_entity_handle);
                handles.push(block_data.end_block_handle);
            }
            CadTemplate::DictionaryObj { dict_data, .. } => {
                for (_, h) in &dict_data.entries {
                    handles.push(*h);
                }
            }
            CadTemplate::DictWithDefault { dict_default_data, .. } => {
                for (_, h) in &dict_default_data.dict_data.entries {
                    handles.push(*h);
                }
                handles.push(dict_default_data.default_entry_handle);
            }
            CadTemplate::LayerEntry { layer_data, .. } => {
                handles.push(layer_data.linetype_handle);
                handles.push(layer_data.plotstyle_handle);
                handles.push(layer_data.material_handle);
            }
            CadTemplate::LineTypeEntry { ltype_data, .. } => {
                handles.push(ltype_data.ltype_control_handle);
                handles.extend_from_slice(&ltype_data.segment_handles);
            }
            CadTemplate::DimStyleEntry { dimstyle_data, .. } => {
                handles.push(dimstyle_data.dimtxsty_handle);
                handles.push(dimstyle_data.dimldrblk_handle);
                handles.push(dimstyle_data.dimblk_handle);
                handles.push(dimstyle_data.dimblk1_handle);
                handles.push(dimstyle_data.dimblk2_handle);
            }
            CadTemplate::VPortEntry { vport_data, .. } => {
                handles.push(vport_data.vport_control_handle);
                handles.push(vport_data.background_handle);
                handles.push(vport_data.style_handle);
                handles.push(vport_data.named_ucs_handle);
                handles.push(vport_data.base_ucs_handle);
            }
            CadTemplate::LayoutObj { layout_data, .. } => {
                handles.push(layout_data.plot_view_handle);
                handles.push(layout_data.visual_style_handle);
                handles.push(layout_data.associated_tab_handle);
                handles.push(layout_data.viewport_handle);
                handles.push(layout_data.base_ucs_handle);
                handles.push(layout_data.named_ucs_handle);
                handles.push(layout_data.block_record_handle);
            }
            CadTemplate::GroupObj { group_data, .. } => {
                handles.extend_from_slice(&group_data.entity_handles);
            }
            CadTemplate::TextEntity { text_data, .. } => {
                handles.push(text_data.style_handle);
            }
            CadTemplate::Insert { insert_data, .. } => {
                handles.push(insert_data.block_header_handle);
                handles.push(insert_data.first_attribute_handle);
                handles.push(insert_data.end_attribute_handle);
                handles.push(insert_data.seqend_handle);
            }
            CadTemplate::Polyline { polyline_data, .. } => {
                handles.push(polyline_data.first_vertex_handle);
                handles.push(polyline_data.last_vertex_handle);
                handles.push(polyline_data.seqend_handle);
                handles.extend_from_slice(&polyline_data.owned_objects_handles);
            }
            CadTemplate::Dimension { dim_data, .. } => {
                handles.push(dim_data.block_handle);
                handles.push(dim_data.style_handle);
            }
            CadTemplate::Leader { leader_data, .. } => {
                handles.push(leader_data.dimstyle_handle);
                handles.push(leader_data.annotation_handle);
            }
            CadTemplate::MLineStyleObj { mls_data, .. } => {
                handles.extend_from_slice(&mls_data.element_linetype_handles);
            }
            CadTemplate::MLeaderStyleObj { mls_style_data, .. } => {
                handles.push(mls_style_data.leader_line_type_handle);
                handles.push(mls_style_data.arrowhead_handle);
                handles.push(mls_style_data.mtext_style_handle);
                handles.push(mls_style_data.block_content_handle);
            }
            CadTemplate::TextStyleEntry { textstyle_data, .. } => {
                handles.push(textstyle_data.style_control_handle);
            }
            CadTemplate::AppIdEntry { appid_data, .. } => {
                handles.push(appid_data.appid_control_handle);
            }
            CadTemplate::UcsEntry { ucs_data, .. } => {
                handles.push(ucs_data.ucs_control_handle);
            }
            _ => {}
        }

        // Filter out zeros.
        handles.retain(|&h| h != 0);
        handles
    }
}

/// Which type of table control object this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableControlType {
    BlockControl,
    LayerControl,
    TextStyleControl,
    LineTypeControl,
    ViewControl,
    UcsControl,
    VPortControl,
    AppIdControl,
    DimStyleControl,
    ViewportEntityHeaderControl,
}
