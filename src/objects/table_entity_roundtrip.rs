//! A DXF file keeps a table's modern content (cell value types, data
//! formats, content formats) outside the ACAD_TABLE record: in a TABLECONTENT
//! object reached through the table's extension dictionary
//! (`ACAD_XREC_ROUNDTRIP` → XRECORD `ACAD_ROUNDTRIP_2008_TABLE_ENTITY` → 360).
//! The table record itself carries only the older cell layout, which has no
//! data type. DWG files keep that content inside the table record.

use crate::document::CadDocument;
use crate::entities::table::{CellStylePropertyFlags as P, Table};
use crate::entities::EntityType;
use crate::objects::{
    DataObject, DataObjectData, ObjectType, TableGeometry, TableGeometryCell, XRecordValue,
};
use crate::types::Handle;

const RECORD_KEY: &str = "ACAD_XREC_ROUNDTRIP";
const MARKER: &str = "ACAD_ROUNDTRIP_2008_TABLE_ENTITY";

/// A table's roundtrip record.
fn roundtrip_record(document: &CadDocument, table: Handle) -> Option<Handle> {
    let dictionary = document.extension_dictionary_handle(table)?;
    let Some(ObjectType::Dictionary(entries)) = document.objects.get(&dictionary) else {
        return None;
    };
    let record = entries.get(RECORD_KEY)?;
    matches!(document.objects.get(&record), Some(ObjectType::XRecord(_))).then_some(record)
}

/// The handle a table's roundtrip record keeps under `code` (360 the
/// content object, 361 the geometry object).
fn roundtrip_entry(document: &CadDocument, table: Handle, code: i32) -> Option<Handle> {
    let Some(ObjectType::XRecord(record)) = document.objects.get(&roundtrip_record(document, table)?)
    else {
        return None;
    };
    record
        .entries
        .iter()
        .skip_while(|entry| entry.value.as_string() != Some(MARKER))
        .find(|entry| entry.code == code)
        .and_then(|entry| match entry.value {
            XRecordValue::Handle(handle) => Some(handle),
            _ => None,
        })
}

/// The table content object a table's roundtrip record points to.
fn roundtrip_content(document: &CadDocument, table: Handle) -> Option<Handle> {
    let content = roundtrip_entry(document, table, 360)?;
    matches!(document.objects.get(&content), Some(ObjectType::TableContent(_))).then_some(content)
}

/// The table geometry object a table's roundtrip record points to.
fn roundtrip_geometry(document: &CadDocument, table: Handle) -> Option<Handle> {
    let geometry = roundtrip_entry(document, table, 361)?;
    matches!(
        document.objects.get(&geometry),
        Some(ObjectType::DataObject(DataObject { data: DataObjectData::TableGeometry(_), .. }))
    )
    .then_some(geometry)
}

/// Where the contents of each cell sit, row by row, as the geometry object
/// keeps it.
fn table_geometry(table: &Table) -> TableGeometry {
    TableGeometry {
        rows: table.rows.len() as i32,
        columns: table.columns.len() as i32,
        cells: table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .map(|cell| TableGeometryCell {
                geometry_data_flag: cell.geometry_data_flag,
                width_with_gap: cell.geometry_width_with_gap,
                height_with_gap: cell.geometry_height_with_gap,
                table_geometry: Handle::NULL,
                geometry: cell.geometries.clone(),
            })
            .collect(),
    }
}

fn table_handles(document: &CadDocument) -> Vec<Handle> {
    document
        .entities()
        .filter_map(|entity| match entity {
            EntityType::Table(table) => Some(table.common.handle),
            _ => None,
        })
        .collect()
}

/// Takes the data type and format a DXF table's content object keeps for
/// each cell into the table record read from the same file.
pub(crate) fn restore_table_entity_roundtrip(document: &mut CadDocument) {
    for handle in table_handles(document) {
        let Some(content) = roundtrip_content(document, handle) else {
            continue;
        };
        let Some(ObjectType::TableContent(content)) = document.objects.get(&content).cloned() else {
            continue;
        };
        let geometry = roundtrip_geometry(document, handle).and_then(|geometry| {
            match document.objects.get(&geometry) {
                Some(ObjectType::DataObject(DataObject {
                    data: DataObjectData::TableGeometry(geometry),
                    ..
                })) => Some(geometry.clone()),
                _ => None,
            }
        });
        let Some(EntityType::Table(table)) = document.get_entity_mut(handle) else {
            continue;
        };
        merge_formats(table, &content);
        if let Some(geometry) = geometry {
            merge_geometry(table, &geometry);
        }
    }
}

fn merge_geometry(table: &mut Table, geometry: &TableGeometry) {
    if geometry.columns as usize != table.columns.len() {
        return;
    }
    let cells = table.rows.iter_mut().flat_map(|row| row.cells.iter_mut());
    for (cell, from) in cells.zip(&geometry.cells) {
        if cell.geometry_width_with_gap == 0.0 && cell.geometries.is_empty() {
            cell.geometry_data_flag = from.geometry_data_flag;
            cell.geometry_width_with_gap = from.width_with_gap;
            cell.geometry_height_with_gap = from.height_with_gap;
            cell.geometries = from.geometry.clone();
        }
    }
}

fn merge_formats(table: &mut Table, content: &Table) {
    for (row, source_row) in table.rows.iter_mut().zip(&content.rows) {
        for (cell, source) in row.cells.iter_mut().zip(&source_row.cells) {
            for (value, from) in cell.contents.iter_mut().zip(&source.contents) {
                let unset = value.format_property_flags == 0
                    && value.format_override_flags == 0
                    && value.format_value_data_type == 0;
                if unset {
                    value.format_property_flags = from.format_property_flags;
                    value.format_override_flags = from.format_override_flags;
                    value.format_value_data_type = from.format_value_data_type;
                    value.format_value_unit_type = from.format_value_unit_type;
                    value.value_format = from.value_format.clone();
                }
            }
            let Some(from) = source.style.as_ref() else {
                continue;
            };
            for property in [P::DATA_TYPE, P::DATA_FORMAT] {
                if !from.sets(property) || cell.style.as_ref().is_some_and(|s| s.sets(property)) {
                    continue;
                }
                let style = cell.style.get_or_insert_with(Default::default);
                if property == P::DATA_TYPE {
                    style.value_data_type = from.value_data_type;
                    style.value_unit_type = from.value_unit_type;
                } else {
                    style.value_format = from.value_format.clone();
                }
                style.property_flags.insert(property);
            }
        }
    }
}

/// Before a DXF write: every table record gets its content object and its
/// geometry object (where each cell's content sits), written from the
/// table as it is now (objects read from a file are brought up to date with
/// edits made since).
pub(crate) fn prepare_table_entity_roundtrip(document: &mut std::borrow::Cow<'_, CadDocument>) {
    let tables = table_handles(document);
    if tables.is_empty() {
        return;
    }
    let document = document.to_mut();
    document.classes.ensure_default("TABLECONTENT");
    document.classes.ensure_default("TABLEGEOMETRY");
    for handle in tables {
        let Some(EntityType::Table(table)) = document.get_entity(handle).cloned() else {
            continue;
        };
        let content = match roundtrip_content(document, handle) {
            Some(content) => content,
            None => {
                let record = document.ensure_xrecord(handle, RECORD_KEY);
                let content = document.allocate_handle();
                if let Some(ObjectType::XRecord(xrecord)) = document.objects.get_mut(&record) {
                    // The record may carry other roundtrip data: replace only
                    // this table's section (a stale one, if any).
                    if let Some(start) = xrecord
                        .entries
                        .iter()
                        .position(|entry| entry.value.as_string() == Some(MARKER))
                    {
                        let end = xrecord.entries[start + 1..]
                            .iter()
                            .position(|entry| entry.code == 102)
                            .map_or(xrecord.entries.len(), |offset| start + 1 + offset);
                        xrecord.entries.drain(start..end);
                    }
                    xrecord.add_string(102, MARKER);
                    xrecord.add_handle(360, content);
                    xrecord.add_int16(70, 2);
                    xrecord.add_int32(90, 1);
                    xrecord.add_point3d(10, 0.0, 0.0, 0.0);
                    xrecord.add_int32(90, 0);
                    xrecord.add_int32(90, table.rows.len().saturating_sub(1) as i32);
                }
                let mut placeholder = Table::default();
                placeholder.common.handle = content;
                placeholder.common.owner_handle = record;
                placeholder.common.reactors = vec![record];
                document
                    .objects
                    .insert(content, ObjectType::TableContent(placeholder));
                content
            }
        };
        if let Some(ObjectType::TableContent(object)) = document.objects.get_mut(&content) {
            let mut written = table.clone();
            written.common.handle = object.common.handle;
            written.common.owner_handle = object.common.owner_handle;
            written.common.reactors = object.common.reactors.clone();
            written.common.xdictionary_handle = object.common.xdictionary_handle;
            *object = *written;
        }
        let geometry = match roundtrip_geometry(document, handle) {
            Some(geometry) => geometry,
            None => {
                let Some(record) = roundtrip_record(document, handle) else {
                    continue;
                };
                let geometry = document.allocate_handle();
                let mut object = DataObject::new(DataObjectData::TableGeometry(TableGeometry::default()));
                object.handle = geometry;
                object.owner = record;
                document.objects.insert(geometry, ObjectType::DataObject(object));
                if let Some(ObjectType::XRecord(xrecord)) = document.objects.get_mut(&record) {
                    xrecord.entries.retain(|entry| entry.code != 361);
                    xrecord.add_handle(361, geometry);
                }
                geometry
            }
        };
        if let Some(ObjectType::DataObject(object)) = document.objects.get_mut(&geometry) {
            object.data = DataObjectData::TableGeometry(table_geometry(&table));
        }
        // The record's last 90 is the table's last row index.
        if let Some(record) = roundtrip_record(document, handle) {
            if let Some(ObjectType::XRecord(xrecord)) = document.objects.get_mut(&record) {
                let last_row = table.rows.len().saturating_sub(1) as i32;
                if let Some(entry) = xrecord
                    .entries
                    .iter_mut()
                    .skip_while(|entry| entry.value.as_string() != Some(MARKER))
                    .filter(|entry| entry.code == 90)
                    .nth(2)
                {
                    entry.value = XRecordValue::Int32(last_row);
                }
            }
        }
    }
}
