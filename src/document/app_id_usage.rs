//! Which registered applications (APPID records) nothing in the drawing uses.

use super::CadDocument;
use crate::entities::EntityType;
use crate::objects::ObjectType;
use crate::types::Handle;
use crate::xdata::ExtendedData;
use std::collections::HashSet;

/// Applications the writers derive extended data for from modelled fields
/// (dimension overrides, annotative styles, layer transparency and
/// description), so they stay registered whether or not a record names them.
const WRITER_APPS: [&str; 4] = [
    "ACAD",
    "AcadAnnotative",
    "AcCmTransparency",
    crate::tables::layer::LAYER_DESCRIPTION_APP,
];

impl CadDocument {
    /// The APPID records that no extended data in the drawing names: the
    /// entities (attributes and mesh vertices included), the verbatim DWG
    /// extended data of objects and table records, their DXF extended data,
    /// and the DXF groups kept verbatim for unsupported records; a record
    /// that owns objects is in use too. These can be removed without leaving
    /// a reference to a missing application or owner.
    pub fn unused_app_ids(&self) -> Vec<Handle> {
        let mut handles = HashSet::new();
        let mut names = HashSet::new();
        let mut note = |xdata: &ExtendedData| {
            handles.extend(xdata.raw_dwg_eed.iter().map(|(app, _)| *app));
            names.extend(
                xdata
                    .records()
                    .iter()
                    .map(|record| record.application_name.to_uppercase()),
            );
        };
        for entity in &self.entities {
            note(&entity.common().extended_data);
            match entity.as_ref() {
                EntityType::Insert(insert) => insert
                    .attributes
                    .iter()
                    .for_each(|a| note(&a.common.extended_data)),
                EntityType::PolyfaceMesh(mesh) => {
                    mesh.vertices
                        .iter()
                        .for_each(|v| note(&v.common.extended_data));
                    mesh.faces.iter().for_each(|f| note(&f.common.extended_data));
                }
                EntityType::PolygonMesh(mesh) => mesh
                    .vertices
                    .iter()
                    .for_each(|v| note(&v.common.extended_data)),
                _ => {}
            }
        }
        for xdata in self.object_xdata.values() {
            note(xdata);
        }
        let mut raw_names = Vec::new();
        for object in self.objects.values() {
            match object {
                ObjectType::Field(field) => note(&field.xdata),
                ObjectType::Unknown {
                    raw_dxf_codes: Some(codes),
                    ..
                } => raw_names.extend(codes.iter()),
                ObjectType::Arch(arch) => raw_names.extend(arch.raw_dxf_codes.iter().flatten()),
                _ => {}
            }
        }
        for entity in &self.entities {
            match entity.as_ref() {
                EntityType::Unknown(unknown) => {
                    raw_names.extend(unknown.raw_dxf_codes.iter().flatten())
                }
                EntityType::Extended(extended) => {
                    if let crate::entities::ExtendedEntityData::Arch(data) = &extended.data {
                        raw_names.extend(data.raw_dxf_codes.iter().flatten());
                    }
                }
                _ => {}
            }
        }
        names.extend(
            raw_names
                .into_iter()
                .filter(|(code, _)| *code == 1001)
                .map(|(_, name)| name.to_uppercase()),
        );
        for blocks in self.eed_by_handle.values() {
            handles.extend(blocks.iter().map(|(app, _)| *app));
        }
        names.extend(WRITER_APPS.iter().map(|name| name.to_uppercase()));
        // A record that owns objects (its extension dictionary) stays, or
        // they would name a missing owner.
        handles.extend(self.xdic_by_handle.keys().map(|owner| owner.value()));
        handles.extend(
            self.objects
                .keys()
                .filter_map(|h| self.object_owner(*h))
                .map(|owner| owner.value()),
        );
        self.app_ids
            .iter()
            .filter(|app| {
                !handles.contains(&app.handle.value()) && !names.contains(&app.name.to_uppercase())
            })
            .map(|app| app.handle)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::document::CadDocument;
    use crate::entities::{EntityType, Line};
    use crate::tables::{AppId, TableEntry};
    use crate::types::Handle;
    use crate::xdata::ExtendedDataRecord;

    #[test]
    fn only_unnamed_applications_are_unused() {
        let mut doc = CadDocument::new();
        for name in ["USED", "RAW", "IDLE"] {
            let mut app = AppId::new(name);
            app.set_handle(doc.allocate_handle());
            doc.app_ids.add(app).unwrap();
        }
        let raw = doc.app_ids.get("RAW").unwrap().handle;
        let mut line = Line::default();
        line.common
            .extended_data
            .add_record(ExtendedDataRecord::new("used"));
        doc.add_entity(EntityType::Line(line)).unwrap();
        doc.eed_by_handle.insert(Handle::new(0x99), vec![(raw.value(), vec![])]);
        let idle = doc.app_ids.get("IDLE").unwrap().handle;
        assert_eq!(doc.unused_app_ids(), vec![idle]);
    }

}
