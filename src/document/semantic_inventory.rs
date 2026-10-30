//! Versioned traversal of a document's logical content.
//!
//! The V1 inventory emits semantic payloads and relationships once. It omits
//! caches (`entity_index`, decoded side views, notifications), source context
//! (`source_path`), handle allocation, file-version metadata, and stored
//! DWG/DXF bytes. Those omitted values are derived or encoding-only state.
//! Storage fields remain available through the ordinary model API for
//! compatibility, but they are not inventory components and must not be
//! counted as semantic content.

use super::{CadDocument, HeaderVariables, Preview, SummaryInfo};
use crate::classes::DxfClass;
use crate::entities::{EntityCommon, EntityType};
use crate::objects::ObjectType;
use crate::tables::{
    AppId, BlockRecord, DimStyle, Layer, LineType, TableEntry, TextStyle, Ucs, VPort, View,
    VxTableRecord,
};
use crate::types::{DxfVersion, Handle};
use crate::xdata::XDataValue;

/// Latest semantic-inventory contract supported by this crate.
pub const SEMANTIC_INVENTORY_VERSION: u16 = 1;

/// A symbol-table record in the V1 semantic inventory.
#[derive(Debug, Clone, Copy)]
pub enum SemanticTableRecordV1<'a> {
    Layer(&'a Layer),
    LineType(&'a LineType),
    TextStyle(&'a TextStyle),
    BlockRecord(&'a BlockRecord),
    DimStyle(&'a DimStyle),
    AppId(&'a AppId),
    View(&'a View),
    VPort(&'a VPort),
    Ucs(&'a Ucs),
    Vx(&'a VxTableRecord),
}

/// A handle-independent node resolved from a document relationship.
#[derive(Debug, Clone, Copy)]
pub enum SemanticNodeV1<'a> {
    Document,
    Entity(&'a EntityType),
    Object(&'a ObjectType),
    TableRecord(SemanticTableRecordV1<'a>),
}

/// Result of resolving one endpoint of a semantic relationship.
#[derive(Debug, Clone, Copy)]
pub enum SemanticReferenceV1<'a> {
    Resolved(SemanticNodeV1<'a>),
    /// The source contained a dangling relationship.
    Unresolved,
}

/// Semantic relationship kinds emitted separately from payload objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticRelationshipKindV1 {
    Ownership,
    ExtensionDictionary,
    Reactor,
}

/// Entity content exposed without an unsupported entity's stored bytes.
#[derive(Debug)]
pub enum SemanticEntityV1<'a> {
    Typed(&'a EntityType),
    Unsupported {
        type_name: &'a str,
        common: &'a EntityCommon,
    },
}

/// Object content exposed without an unsupported object's stored bytes.
#[derive(Debug)]
pub enum SemanticObjectV1<'a> {
    Typed(&'a ObjectType),
    Unsupported { type_name: &'a str },
}

/// One logical component in the V1 inventory.
#[derive(Debug)]
pub enum SemanticPartV1<'a> {
    Header(&'a HeaderVariables),
    TableRecord(SemanticTableRecordV1<'a>),
    Class(&'a DxfClass),
    Entity(SemanticEntityV1<'a>),
    Object(SemanticObjectV1<'a>),
    SummaryInfo(&'a SummaryInfo),
    Preview(&'a Preview),
    Relationship {
        kind: SemanticRelationshipKindV1,
        source: SemanticReferenceV1<'a>,
        target: SemanticReferenceV1<'a>,
    },
    /// Structured non-entity extended data. `None` means the stored payload
    /// could not be decoded; no encoding bytes are exposed as semantics.
    NonEntityExtendedData {
        owner: SemanticReferenceV1<'a>,
        application: Option<&'a AppId>,
        values: Option<Vec<XDataValue>>,
    },
}

/// Read-only V1 view over all logical document content.
pub struct SemanticInventoryV1<'a> {
    document: &'a CadDocument,
}

impl CadDocument {
    /// Return the current versioned semantic inventory.
    pub fn semantic_inventory_v1(&self) -> SemanticInventoryV1<'_> {
        SemanticInventoryV1 { document: self }
    }
}

impl<'a> SemanticInventoryV1<'a> {
    /// Resolve a document handle to a semantic node. Numeric handles are lookup
    /// keys only; callers do not need to retain them as semantic identity.
    pub fn resolve(&self, handle: Handle) -> Option<SemanticNodeV1<'a>> {
        if handle.is_null() {
            return Some(SemanticNodeV1::Document);
        }
        if let Some(entity) = self.document.get_entity(handle) {
            return Some(SemanticNodeV1::Entity(entity));
        }
        if let Some(object) = self.document.objects.get(&handle) {
            return Some(SemanticNodeV1::Object(object));
        }

        macro_rules! resolve_table {
            ($table:expr, $variant:ident) => {
                if let Some(record) = $table.iter().find(|record| record.handle() == handle) {
                    return Some(SemanticNodeV1::TableRecord(
                        SemanticTableRecordV1::$variant(record),
                    ));
                }
            };
        }
        resolve_table!(self.document.layers, Layer);
        resolve_table!(self.document.line_types, LineType);
        resolve_table!(self.document.text_styles, TextStyle);
        resolve_table!(self.document.block_records, BlockRecord);
        resolve_table!(self.document.dim_styles, DimStyle);
        resolve_table!(self.document.app_ids, AppId);
        resolve_table!(self.document.views, View);
        resolve_table!(self.document.vports, VPort);
        resolve_table!(self.document.ucss, Ucs);
        resolve_table!(self.document.vx_table, Vx);
        None
    }

    fn reference(&self, handle: Handle) -> SemanticReferenceV1<'a> {
        self.resolve(handle)
            .map(SemanticReferenceV1::Resolved)
            .unwrap_or(SemanticReferenceV1::Unresolved)
    }

    /// Visit every V1 semantic component exactly once.
    ///
    /// Entity/object ownership, extension dictionaries, and reactors are
    /// emitted as resolved relationships. Raw records, page data, caches, and
    /// duplicate decoded side views are intentionally excluded.
    pub fn visit(&self, mut visitor: impl FnMut(SemanticPartV1<'a>)) {
        let CadDocument {
            version,
            maintenance_version: _,
            header,
            layers,
            line_types,
            text_styles,
            block_records,
            dim_styles,
            app_ids,
            views,
            vports,
            ucss,
            vx_table,
            vx_control_entries: _,
            classes,
            notifications: _,
            entities,
            entity_index: _,
            objects,
            block_visibility_params: _,
            context_scales: _,
            block_representations: _,
            fields: _,
            summary_info,
            source_path: _,
            dgn_ls_definitions: _,
            dgn_ls_components: _,
            eed_by_handle,
            object_xdata,
            xdic_by_handle,
            reactors_by_handle,
            block_entity_handles: _,
            dwg_source_version,
            preview,
            acis_sab_handles: _,
            raw_acds_data: _,
            r12_header_raw: _,
            r12_table_usage: _,
            r12_block_tags: _,
            raw_acds_fingerprint: _,
            raw_records: _,
            original_objects: _,
            raw_record_owners: _,
            dwg_data_store_handles: _,
            section_view_style: _,
            view_rep_refs: _,
            section_view_reps: _,
            next_handle: _,
        } = self.document;

        visitor(SemanticPartV1::Header(header));
        macro_rules! visit_table {
            ($table:expr, $variant:ident) => {
                for record in $table.iter() {
                    visitor(SemanticPartV1::TableRecord(
                        SemanticTableRecordV1::$variant(record),
                    ));
                }
            };
        }
        visit_table!(layers, Layer);
        visit_table!(line_types, LineType);
        visit_table!(text_styles, TextStyle);
        visit_table!(block_records, BlockRecord);
        visit_table!(dim_styles, DimStyle);
        visit_table!(app_ids, AppId);
        visit_table!(views, View);
        visit_table!(vports, VPort);
        visit_table!(ucss, Ucs);
        visit_table!(vx_table, Vx);
        for class in classes.iter() {
            visitor(SemanticPartV1::Class(class));
        }

        for entity in entities {
            let entity = entity.as_ref();
            visitor(SemanticPartV1::Entity(classify_entity(entity)));
            let common = entity.common();
            self.visit_relationship(
                &mut visitor,
                SemanticRelationshipKindV1::Ownership,
                SemanticReferenceV1::Resolved(SemanticNodeV1::Entity(entity)),
                common.owner_handle,
            );
            if let Some(dictionary) = common.xdictionary_handle {
                self.visit_relationship(
                    &mut visitor,
                    SemanticRelationshipKindV1::ExtensionDictionary,
                    SemanticReferenceV1::Resolved(SemanticNodeV1::Entity(entity)),
                    dictionary,
                );
            }
            for reactor in &common.reactors {
                self.visit_relationship(
                    &mut visitor,
                    SemanticRelationshipKindV1::Reactor,
                    SemanticReferenceV1::Resolved(SemanticNodeV1::Entity(entity)),
                    *reactor,
                );
            }
        }

        for (handle, object) in objects {
            visitor(SemanticPartV1::Object(classify_object(object)));
            if let Some(owner) = self.document.object_owner(*handle) {
                self.visit_relationship(
                    &mut visitor,
                    SemanticRelationshipKindV1::Ownership,
                    SemanticReferenceV1::Resolved(SemanticNodeV1::Object(object)),
                    owner,
                );
            }
            if let ObjectType::ImageDefinition(definition) = object {
                for reactor in &definition.reactors {
                    self.visit_relationship(
                        &mut visitor,
                        SemanticRelationshipKindV1::Reactor,
                        SemanticReferenceV1::Resolved(SemanticNodeV1::Object(object)),
                        *reactor,
                    );
                }
            }
        }

        visitor(SemanticPartV1::SummaryInfo(summary_info));
        if let Some(preview) = preview {
            visitor(SemanticPartV1::Preview(preview));
        }

        for (owner, dictionary) in xdic_by_handle {
            self.visit_relationship(
                &mut visitor,
                SemanticRelationshipKindV1::ExtensionDictionary,
                self.reference(*owner),
                *dictionary,
            );
        }
        for (owner, reactors) in reactors_by_handle {
            for reactor in reactors {
                self.visit_relationship(
                    &mut visitor,
                    SemanticRelationshipKindV1::Reactor,
                    self.reference(*owner),
                    *reactor,
                );
            }
        }

        let wide = dwg_source_version.unwrap_or(*version) >= DxfVersion::AC1021;
        for (owner, records) in eed_by_handle {
            for (application_handle, payload) in records {
                let application = app_ids
                    .iter()
                    .find(|entry| entry.handle.value() == *application_handle);
                let values = crate::io::dwg::eed_codec::decode_values(payload, wide, |handle| {
                    layers
                        .iter()
                        .find(|layer| layer.handle.value() == handle)
                        .map(|layer| layer.name.clone())
                });
                visitor(SemanticPartV1::NonEntityExtendedData {
                    owner: self.reference(*owner),
                    application,
                    values,
                });
            }
        }
        // Records without a verbatim EED block (read from DXF).
        for (owner, xdata) in object_xdata {
            for record in xdata.records() {
                let application = app_ids.get(&record.application_name);
                let has_raw = application.is_some_and(|app| {
                    eed_by_handle
                        .get(owner)
                        .is_some_and(|blocks| blocks.iter().any(|(a, _)| *a == app.handle.value()))
                });
                if has_raw {
                    continue;
                }
                visitor(SemanticPartV1::NonEntityExtendedData {
                    owner: self.reference(*owner),
                    application,
                    values: Some(record.values.clone()),
                });
            }
        }
    }

    fn visit_relationship(
        &self,
        visitor: &mut impl FnMut(SemanticPartV1<'a>),
        kind: SemanticRelationshipKindV1,
        source: SemanticReferenceV1<'a>,
        target: Handle,
    ) {
        visitor(SemanticPartV1::Relationship {
            kind,
            source,
            target: self.reference(target),
        });
    }
}

fn classify_entity(entity: &EntityType) -> SemanticEntityV1<'_> {
    match entity {
        EntityType::Unknown(value) => SemanticEntityV1::Unsupported {
            type_name: &value.dxf_name,
            common: &value.common,
        },
        EntityType::Point(_)
        | EntityType::Line(_)
        | EntityType::Circle(_)
        | EntityType::Arc(_)
        | EntityType::Ellipse(_)
        | EntityType::Polyline(_)
        | EntityType::Polyline2D(_)
        | EntityType::Polyline3D(_)
        | EntityType::LwPolyline(_)
        | EntityType::Text(_)
        | EntityType::MText(_)
        | EntityType::Spline(_)
        | EntityType::Helix(_)
        | EntityType::Dimension(_)
        | EntityType::Hatch(_)
        | EntityType::Solid(_)
        | EntityType::Face3D(_)
        | EntityType::Insert(_)
        | EntityType::Block(_)
        | EntityType::BlockEnd(_)
        | EntityType::Ray(_)
        | EntityType::XLine(_)
        | EntityType::Viewport(_)
        | EntityType::AttributeDefinition(_)
        | EntityType::AttributeEntity(_)
        | EntityType::Leader(_)
        | EntityType::MultiLeader(_)
        | EntityType::MLine(_)
        | EntityType::Mesh(_)
        | EntityType::RasterImage(_)
        | EntityType::Solid3D(_)
        | EntityType::Region(_)
        | EntityType::Body(_)
        | EntityType::Surface(_)
        | EntityType::Table(_)
        | EntityType::Tolerance(_)
        | EntityType::PolyfaceMesh(_)
        | EntityType::Wipeout(_)
        | EntityType::Shape(_)
        | EntityType::Underlay(_)
        | EntityType::Seqend(_)
        | EntityType::Ole2Frame(_)
        | EntityType::PolygonMesh(_)
        | EntityType::Light(_)
        | EntityType::SectionSymbol(_)
        | EntityType::ViewBorder(_)
        | EntityType::Extended(_) => SemanticEntityV1::Typed(entity),
    }
}

fn classify_object(object: &ObjectType) -> SemanticObjectV1<'_> {
    match object {
        ObjectType::Unknown { type_name, .. } => SemanticObjectV1::Unsupported { type_name },
        ObjectType::Dictionary(_)
        | ObjectType::Layout(_)
        | ObjectType::XRecord(_)
        | ObjectType::Group(_)
        | ObjectType::MLineStyle(_)
        | ObjectType::ImageDefinition(_)
        | ObjectType::UnderlayDefinition(_)
        | ObjectType::PlotSettings(_)
        | ObjectType::MultiLeaderStyle(_)
        | ObjectType::TableStyle(_)
        | ObjectType::TableContent(_)
        | ObjectType::TableTemplate(_)
        | ObjectType::Scale(_)
        | ObjectType::ObjectContextData(_)
        | ObjectType::SortEntitiesTable(_)
        | ObjectType::DictionaryVariable(_)
        | ObjectType::VisualStyle(_)
        | ObjectType::Material(_)
        | ObjectType::ImageDefinitionReactor(_)
        | ObjectType::GeoData(_)
        | ObjectType::SpatialFilter(_)
        | ObjectType::RasterVariables(_)
        | ObjectType::BookColor(_)
        | ObjectType::PlaceHolder(_)
        | ObjectType::DictionaryWithDefault(_)
        | ObjectType::WipeoutVariables(_)
        | ObjectType::BlockVisibilityParameter(_)
        | ObjectType::DynamicBlock(_)
        | ObjectType::Associative(_)
        | ObjectType::ClassObject(_)
        | ObjectType::DataObject(_)
        | ObjectType::Field(_)
        | ObjectType::FieldList(_)
        | ObjectType::RegisteredClass(_)
        | ObjectType::DgnLineStyle(_)
        | ObjectType::ProxyObject(_)
        | ObjectType::Arch(_) => SemanticObjectV1::Typed(object),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{EntityType, Line};
    use crate::objects::{Dictionary, ObjectType};
    use crate::xdata::XDataValue;

    #[test]
    fn inventory_exposes_private_relationships_without_raw_bytes() {
        let mut document = CadDocument::new();
        let entity = document.add_entity(EntityType::Line(Line::new())).unwrap();
        let mut dictionary = Dictionary::new();
        dictionary.handle = document.allocate_handle();
        let dictionary_handle = dictionary.handle;
        document
            .objects
            .insert(dictionary_handle, ObjectType::Dictionary(dictionary));
        document.xdic_by_handle.insert(entity, dictionary_handle);
        let application_handle = document.app_ids.iter().next().unwrap().handle.value();
        document
            .eed_by_handle
            .insert(entity, vec![(application_handle, vec![70, 7, 0])]);

        let mut extension_dictionary_seen = false;
        let mut eed_seen = false;
        document.semantic_inventory_v1().visit(|part| match part {
            SemanticPartV1::Relationship {
                kind: SemanticRelationshipKindV1::ExtensionDictionary,
                source: SemanticReferenceV1::Resolved(SemanticNodeV1::Entity(_)),
                target: SemanticReferenceV1::Resolved(SemanticNodeV1::Object(_)),
            } => extension_dictionary_seen = true,
            SemanticPartV1::NonEntityExtendedData {
                application: Some(application),
                values: Some(values),
                ..
            } if application.handle.value() == application_handle => {
                assert_eq!(values, vec![XDataValue::Integer16(7)]);
                eed_seen = true;
            }
            _ => {}
        });

        assert!(extension_dictionary_seen);
        assert!(eed_seen);
    }
}
