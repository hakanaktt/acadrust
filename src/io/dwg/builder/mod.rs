//! DWG document builder — resolves handle-based templates into a final [`CadDocument`].
//!
//! Mirrors ACadSharp's `CadDocumentBuilder` + `DwgDocumentBuilder`.
//!
//! # Architecture
//!
//! The DWG format stores all objects (entities, table entries, non-graphical
//! objects) in a flat heap indexed by handle.  While reading, the object reader
//! (Phase 5) creates *templates* — intermediate structures holding raw handle
//! values rather than resolved object references.  After every template has
//! been collected the builder performs multi-phase resolution:
//!
//! 1. **Create missing handles** — objects with handle=0 get unique handles.
//! 2. **Set blocks to records** — link Block/EndBlock entities to their
//!    `BlockRecord` table entries using the header handles collection.
//! 3. **Register tables** — set the 9 standard table handles in `CadDocument`.
//! 4. **Build tables** — for each table control object template, add its
//!    entries.
//! 5. **Build dictionaries** — resolve dictionary entry handles.
//! 6. **Build templates** — resolve entity & object handle references to
//!    concrete objects and insert them into the document.
//! 7. **Update header** — resolve header handle references.

use std::collections::HashMap;

use crate::document::{get_common_mut, CadDocument};
use crate::entities::EntityType;
use crate::notification::{Notification, NotificationType};
use crate::tables::*;
use crate::types::{DxfVersion, Handle};

use super::header_handles::DwgHeaderHandlesCollection;
use super::reader::object_reader::templates::{
    CadBlockRecordTemplateData, CadEntityTemplateData, CadLayerTemplateData,
    CadLineTypeTemplateData, CadTemplate, CadTemplateCommon, TableControlType,
};

/// The DWG document builder.
///
/// After the object reader has collected all templates, this builder
/// resolves handle references and assembles the final [`CadDocument`].
pub struct DwgDocumentBuilder {
    /// The document being built.
    pub document: CadDocument,

    /// DWG version.
    pub version: DxfVersion,

    /// Header handles collection (from header reader).
    pub header_handles: DwgHeaderHandlesCollection,

    // ------------------------------------------------------------------
    // Template storage
    // ------------------------------------------------------------------
    /// All templates by handle.
    templates_map: HashMap<u64, CadTemplate>,

    /// Highest handle encountered — used for missing-handle allocation.
    initial_hand_seed: u64,

    /// Templates with handle=0 that still need to be assigned.
    unassigned: Vec<CadTemplate>,

    /// Resolved block record names, keyed by block header handle.
    block_names: HashMap<u64, String>,

    // ------------------------------------------------------------------
    // Working state
    // ------------------------------------------------------------------
    /// Notifications collected during building.
    pub notifications: Vec<Notification>,

    /// Whether to keep unknown entities in the built document.
    pub keep_unknown_entities: bool,

    /// Whether to keep unknown non-graphical objects.
    pub keep_unknown_objects: bool,
}

impl DwgDocumentBuilder {
    /// Create a new builder for a specific DWG version.
    pub fn new(version: DxfVersion) -> Self {
        let mut document = CadDocument::new();
        document.version = version;

        Self {
            document,
            version,
            header_handles: DwgHeaderHandlesCollection::new(),
            templates_map: HashMap::new(),
            initial_hand_seed: 0,
            unassigned: Vec::new(),
            block_names: HashMap::new(),
            notifications: Vec::new(),
            keep_unknown_entities: false,
            keep_unknown_objects: false,
        }
    }

    // ------------------------------------------------------------------
    // Template registration
    // ------------------------------------------------------------------

    /// Register a template.
    pub fn add_template(&mut self, template: CadTemplate) {
        let handle = template.handle();

        if handle == 0 {
            self.unassigned.push(template);
            return;
        }

        if self.templates_map.contains_key(&handle) {
            self.notify(
                &format!("Repeated handle found {:#X}", handle),
                NotificationType::Warning,
            );
            self.unassigned.push(template);
            return;
        }

        if handle > self.initial_hand_seed {
            self.initial_hand_seed = handle;
        }

        self.templates_map.insert(handle, template);
    }

    /// Register a batch of templates from the object reader.
    pub fn add_templates(&mut self, templates: Vec<CadTemplate>) {
        for t in templates {
            self.add_template(t);
        }
    }

    // ------------------------------------------------------------------
    // Lookup
    // ------------------------------------------------------------------

    /// Look up a template by handle.
    pub fn get_template(&self, handle: u64) -> Option<&CadTemplate> {
        self.templates_map.get(&handle)
    }

    /// Look up a template by handle (mutable).
    pub fn get_template_mut(&mut self, handle: u64) -> Option<&mut CadTemplate> {
        self.templates_map.get_mut(&handle)
    }

    // ------------------------------------------------------------------
    // Building
    // ------------------------------------------------------------------

    /// Main entry-point: build the final `CadDocument` from all templates.
    pub fn build_document(&mut self) {
        self.create_missing_handles();
        self.set_blocks_to_records();
        self.register_tables();
        self.build_tables();
        self.build_dictionaries();
        self.build_object_templates();
        self.update_header();
    }

    // ------------------------------------------------------------------
    // Phase 1 — Create missing handles
    // ------------------------------------------------------------------

    fn create_missing_handles(&mut self) {
        let mut templates = std::mem::take(&mut self.unassigned);
        for mut t in templates.drain(..) {
            self.initial_hand_seed += 1;
            set_template_handle(&mut t, self.initial_hand_seed);
            self.templates_map.insert(self.initial_hand_seed, t);
        }
    }

    // ------------------------------------------------------------------
    // Phase 2 — Set blocks to records
    // ------------------------------------------------------------------

    fn set_blocks_to_records(&mut self) {
        let block_header_handles: Vec<u64> = self
            .templates_map
            .iter()
            .filter_map(|(h, t)| {
                if matches!(t, CadTemplate::BlockHeader { .. }) {
                    Some(*h)
                } else {
                    None
                }
            })
            .collect();

        let model_handle = self.header_handles.model_space().unwrap_or(0);
        let paper_handle = self.header_handles.paper_space().unwrap_or(0);

        let mut block_names: HashMap<u64, String> = HashMap::new();
        for handle in &block_header_handles {
            if let Some(CadTemplate::BlockHeader { block_data, .. }) =
                self.templates_map.get(handle)
            {
                if block_data.block_entity_handle != 0 {
                    if let Some(name) = self.get_block_entity_name(block_data.block_entity_handle) {
                        block_names.insert(*handle, name);
                    }
                }
            }
        }

        if model_handle != 0 {
            block_names.insert(model_handle, "*Model_Space".to_string());
        }
        if paper_handle != 0 {
            block_names.insert(paper_handle, "*Paper_Space".to_string());
        }

        self.block_names = block_names;
    }

    fn get_block_entity_name(&self, handle: u64) -> Option<String> {
        if let Some(template) = self.templates_map.get(&handle) {
            if let CadTemplate::Entity { entity, .. } = template {
                if let EntityType::Block(block) = entity {
                    return Some(block.name.clone());
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Phase 3 — Register tables
    // ------------------------------------------------------------------

    fn register_tables(&mut self) {
        if let Some(h) = self.header_handles.block_control_object() {
            self.document.block_records.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.layer_control_object() {
            self.document.layers.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.style_control_object() {
            self.document.text_styles.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.linetype_control_object() {
            self.document.line_types.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.view_control_object() {
            self.document.views.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.ucs_control_object() {
            self.document.ucss.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.vport_control_object() {
            self.document.vports.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.appid_control_object() {
            self.document.app_ids.set_handle(Handle::new(h));
        }
        if let Some(h) = self.header_handles.dimstyle_control_object() {
            self.document.dim_styles.set_handle(Handle::new(h));
        }
    }

    // ------------------------------------------------------------------
    // Phase 4 — Build tables
    // ------------------------------------------------------------------

    fn build_tables(&mut self) {
        let table_controls: Vec<(u64, TableControlType, Vec<u64>)> = self
            .templates_map
            .iter()
            .filter_map(|(h, t)| {
                if let CadTemplate::TableControl {
                    table_data,
                    table_type,
                    ..
                } = t
                {
                    Some((*h, *table_type, table_data.entry_handles.clone()))
                } else {
                    None
                }
            })
            .collect();

        for (_control_handle, table_type, entry_handles) in table_controls {
            for entry_handle in &entry_handles {
                self.build_table_entry(*entry_handle, table_type);
            }
        }
    }

    fn build_table_entry(&mut self, handle: u64, table_type: TableControlType) {
        let template = match self.templates_map.get(&handle) {
            Some(t) => t.clone(),
            None => {
                self.notify(
                    &format!("Table entry handle {:#X} not found", handle),
                    NotificationType::Warning,
                );
                return;
            }
        };

        match table_type {
            TableControlType::BlockControl => {
                if let CadTemplate::BlockHeader { common, block_data } = &template {
                    self.build_block_record(handle, common, block_data);
                }
            }
            TableControlType::LayerControl => {
                if let CadTemplate::LayerEntry { common, layer_data } = &template {
                    self.build_layer(handle, common, layer_data);
                }
            }
            TableControlType::LineTypeControl => {
                if let CadTemplate::LineTypeEntry {
                    common, ltype_data, ..
                } = &template
                {
                    self.build_linetype(handle, common, ltype_data);
                }
            }
            _ => {
                self.build_generic_table_entry(handle, &template, table_type);
            }
        }
    }

    fn build_block_record(
        &mut self,
        handle: u64,
        _common: &CadTemplateCommon,
        data: &CadBlockRecordTemplateData,
    ) {
        let name = self
            .block_names
            .get(&handle)
            .cloned()
            .unwrap_or_else(|| format!("*U{}", handle));

        let mut record = BlockRecord::new(&name);
        record.handle = Handle::new(handle);

        let entity_handles = if !data.owned_object_handles.is_empty() {
            data.owned_object_handles.clone()
        } else if data.first_entity_handle != 0 {
            self.walk_entity_chain(data.first_entity_handle, data.last_entity_handle)
        } else {
            Vec::new()
        };

        for entity_handle in &entity_handles {
            if let Some(entity) = self.build_entity(*entity_handle) {
                record.entities.push(entity);
            }
        }

        record.layout = Handle::new(data.layout_handle);
        record.block_entity_handle = Handle::new(data.block_entity_handle);
        record.block_end_handle = Handle::new(data.end_block_handle);

        self.document.block_records.remove(&name);
        let _ = self.document.block_records.add(record);
    }

    fn build_layer(
        &mut self,
        handle: u64,
        _common: &CadTemplateCommon,
        _data: &CadLayerTemplateData,
    ) {
        // Full field population requires extending the template system to
        // embed the actual Layer struct read by the object reader.
        self.notify(
            &format!(
                "Layer entry {:#X} — full field population deferred to extended templates",
                handle
            ),
            NotificationType::NotImplemented,
        );
    }

    fn build_linetype(
        &mut self,
        handle: u64,
        _common: &CadTemplateCommon,
        _data: &CadLineTypeTemplateData,
    ) {
        self.notify(
            &format!("LineType entry {:#X} build deferred", handle),
            NotificationType::NotImplemented,
        );
    }

    fn build_generic_table_entry(
        &mut self,
        handle: u64,
        _template: &CadTemplate,
        table_type: TableControlType,
    ) {
        self.notify(
            &format!(
                "{:?} table entry {:#X} build deferred",
                table_type, handle
            ),
            NotificationType::NotImplemented,
        );
    }

    // ------------------------------------------------------------------
    // Phase 5 — Build dictionaries
    // ------------------------------------------------------------------

    fn build_dictionaries(&mut self) {
        let dict_handles: Vec<u64> = self
            .templates_map
            .iter()
            .filter_map(|(h, t)| {
                if matches!(
                    t,
                    CadTemplate::DictionaryObj { .. } | CadTemplate::DictWithDefault { .. }
                ) {
                    Some(*h)
                } else {
                    None
                }
            })
            .collect();

        for handle in &dict_handles {
            self.build_dictionary(*handle);
        }

        if let Some(rh) = self.header_handles.dictionary_named_objects() {
            self.document.header.named_objects_dict_handle = Handle::new(rh);
        }
    }

    fn build_dictionary(&mut self, handle: u64) {
        let template = match self.templates_map.get(&handle) {
            Some(t) => t.clone(),
            None => return,
        };

        match &template {
            CadTemplate::DictionaryObj { .. } | CadTemplate::DictWithDefault { .. } => {
                // Dictionary objects stored; entry resolution is deferred
                // until the full object model is implemented.
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Phase 6 — Build remaining object templates
    // ------------------------------------------------------------------

    fn build_object_templates(&mut self) {
        // Entities owned by block records were already consumed in Phase 4.
        // Remaining templates are non-graphical objects (layouts, groups, etc.)
        // which will be fully built when the object model is extended.
    }

    // ------------------------------------------------------------------
    // Phase 7 — Update header
    // ------------------------------------------------------------------

    fn update_header(&mut self) {
        // Current settings handles.
        if let Some(h) = self.header_handles.clayer() {
            self.document.header.current_layer_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.textstyle() {
            self.document.header.current_text_style_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.celtype() {
            self.document.header.current_linetype_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimstyle() {
            self.document.header.current_dimstyle_handle = Handle::new(h);
        }

        // Standard linetypes.
        if let Some(h) = self.header_handles.bylayer() {
            self.document.header.bylayer_linetype_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.byblock() {
            self.document.header.byblock_linetype_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.continuous() {
            self.document.header.continuous_linetype_handle = Handle::new(h);
        }

        // Block handles.
        if let Some(h) = self.header_handles.model_space() {
            self.document.header.model_space_block_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.paper_space() {
            self.document.header.paper_space_block_handle = Handle::new(h);
        }

        // Table control handles.
        if let Some(h) = self.header_handles.block_control_object() {
            self.document.header.block_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.layer_control_object() {
            self.document.header.layer_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.style_control_object() {
            self.document.header.style_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.linetype_control_object() {
            self.document.header.linetype_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.view_control_object() {
            self.document.header.view_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.ucs_control_object() {
            self.document.header.ucs_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.vport_control_object() {
            self.document.header.vport_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.appid_control_object() {
            self.document.header.appid_control_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimstyle_control_object() {
            self.document.header.dimstyle_control_handle = Handle::new(h);
        }

        // Dictionary handles.
        if let Some(h) = self.header_handles.dictionary_named_objects() {
            self.document.header.named_objects_dict_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dictionary_acad_group() {
            self.document.header.acad_group_dict_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dictionary_acad_mlinestyle() {
            self.document.header.acad_mlinestyle_dict_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dictionary_layouts() {
            self.document.header.acad_layout_dict_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dictionary_plotsettings() {
            self.document.header.acad_plotsettings_dict_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dictionary_plotstyles() {
            self.document.header.acad_plotstylename_dict_handle = Handle::new(h);
        }

        // Dimension handles.
        if let Some(h) = self.header_handles.dimtxsty() {
            self.document.header.dim_text_style_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimblk() {
            self.document.header.dim_arrow_block_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimblk1() {
            self.document.header.dim_arrow_block1_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimblk2() {
            self.document.header.dim_arrow_block2_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimltype() {
            self.document.header.dim_linetype_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimltex1() {
            self.document.header.dim_linetype1_handle = Handle::new(h);
        }
        if let Some(h) = self.header_handles.dimltex2() {
            self.document.header.dim_linetype2_handle = Handle::new(h);
        }

        // UCS handles.
        if let Some(h) = self.header_handles.ucsorthoref() {
            self.document.header.ucs_ortho_ref = Handle::new(h);
        }
        if let Some(h) = self.header_handles.pucsorthoref() {
            self.document.header.paper_ucs_ortho_ref = Handle::new(h);
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn build_entity(&mut self, handle: u64) -> Option<EntityType> {
        let template = self.templates_map.get(&handle)?.clone();
        match template {
            CadTemplate::Entity {
                entity,
                common,
                entity_data,
            }
            | CadTemplate::TextEntity {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Insert {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Polyline {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Dimension {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Leader {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::MultiLeader {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Shape {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Viewport {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Hatch {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Solid3D {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::Image {
                entity,
                common,
                entity_data,
                ..
            }
            | CadTemplate::PolyfaceMesh {
                entity,
                common,
                entity_data,
                ..
            } => {
                let mut entity = entity;
                self.apply_entity_common(&mut entity, handle, &common, &entity_data);
                Some(entity)
            }
            _ => None,
        }
    }

    fn apply_entity_common(
        &self,
        entity: &mut EntityType,
        handle: u64,
        common: &CadTemplateCommon,
        _entity_data: &CadEntityTemplateData,
    ) {
        let ec = get_common_mut(entity);
        ec.handle = Handle::new(handle);
        ec.owner_handle = Handle::new(common.owner_handle);

        // Set reactors from common template data.
        ec.reactors = common
            .reactor_handles
            .iter()
            .map(|&h| Handle::new(h))
            .collect();

        // Set xdictionary handle.
        if common.xdict_handle != 0 {
            ec.xdictionary_handle = Some(Handle::new(common.xdict_handle));
        }
    }

    fn walk_entity_chain(&self, first: u64, last: u64) -> Vec<u64> {
        let mut result = Vec::new();
        let mut current = first;
        let mut iterations = 0;
        let max_iterations = 100_000;

        while current != 0 && iterations < max_iterations {
            result.push(current);
            if current == last {
                break;
            }
            if let Some(template) = self.templates_map.get(&current) {
                match template {
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
                        current = entity_data.next_entity;
                    }
                    _ => break,
                }
            } else {
                break;
            }
            iterations += 1;
        }

        result
    }

    fn notify(&mut self, message: &str, ntype: NotificationType) {
        self.notifications.push(Notification::new(ntype, message));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Set the handle inside a CadTemplate.
fn set_template_handle(template: &mut CadTemplate, handle: u64) {
    match template {
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
        | CadTemplate::GenericObject { common, .. } => {
            common.handle = handle;
        }
    }
}
