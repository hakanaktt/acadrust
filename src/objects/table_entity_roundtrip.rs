//! A DXF file keeps a table's modern content (cell value types, data
//! formats, content formats) outside the ACAD_TABLE record: in a TABLECONTENT
//! object reached through the table's extension dictionary
//! (`ACAD_XREC_ROUNDTRIP` → XRECORD `ACAD_ROUNDTRIP_2008_TABLE_ENTITY` → 360).
//! The table record itself carries only the older cell layout, which has no
//! data type. DWG files keep that content inside the table record.

use crate::document::CadDocument;
use crate::entities::table::{CellStylePropertyFlags as P, CellType, Table, TableCellContentType};
use crate::entities::EntityType;
use crate::io::read::{push_read_diagnostic, ReadDiagnostic, ReadStage};
use crate::objects::{
    DataObject, DataObjectData, ObjectType, TableGeometry, TableGeometryCell, XRecord, XRecordValue,
};
use crate::types::Handle;

const RECORD_KEY: &str = "ACAD_XREC_ROUNDTRIP";
const MARKER: &str = "ACAD_ROUNDTRIP_2008_TABLE_ENTITY";

fn table_section(record: &XRecord) -> Option<std::ops::Range<usize>> {
    let start = record
        .entries
        .iter()
        .position(|entry| entry.code == 102 && entry.value.as_string() == Some(MARKER))?;
    let end = record.entries[start + 1..]
        .iter()
        .position(|entry| entry.code == 102)
        .map_or(record.entries.len(), |offset| start + 1 + offset);
    Some(start..end)
}

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
    let Some(ObjectType::XRecord(record)) =
        document.objects.get(&roundtrip_record(document, table)?)
    else {
        return None;
    };
    record.entries[table_section(record)?]
        .iter()
        .find(|entry| entry.code == code)
        .and_then(|entry| match entry.value {
            XRecordValue::Handle(handle) => Some(handle),
            _ => None,
        })
}

/// The table content object a table's roundtrip record points to.
fn roundtrip_content(document: &CadDocument, table: Handle) -> Option<Handle> {
    let content = roundtrip_entry(document, table, 360)?;
    matches!(
        document.objects.get(&content),
        Some(ObjectType::TableContent(_))
    )
    .then_some(content)
}

/// The table geometry object a table's roundtrip record points to.
fn roundtrip_geometry(document: &CadDocument, table: Handle) -> Option<Handle> {
    let geometry = roundtrip_entry(document, table, 361)?;
    matches!(
        document.objects.get(&geometry),
        Some(ObjectType::DataObject(DataObject {
            data: DataObjectData::TableGeometry(_),
            ..
        }))
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

/// Completes the legacy DXF entity from its associated content and geometry.
pub(crate) fn restore_table_entity_roundtrip(
    document: &mut CadDocument,
    diagnostics: &mut Vec<ReadDiagnostic>,
) {
    for handle in table_handles(document) {
        let content = roundtrip_entry(document, handle, 360).and_then(|target| {
            if let Some(ObjectType::TableContent(content)) = document.objects.get(&target) {
                return Some(content.clone());
            }
            diagnose(
                diagnostics,
                handle,
                format!("Invalid TABLECONTENT target {target:?}"),
            );
            None
        });
        let geometry = roundtrip_entry(document, handle, 361).and_then(|target| {
            if let Some(ObjectType::DataObject(DataObject {
                data: DataObjectData::TableGeometry(geometry),
                ..
            })) = document.objects.get(&target)
            {
                return Some(geometry.clone());
            }
            diagnose(
                diagnostics,
                handle,
                format!("Invalid TABLEGEOMETRY target {target:?}"),
            );
            None
        });
        let Some(EntityType::Table(table)) = document.get_entity_mut(handle) else {
            continue;
        };
        if let Some(content) = content {
            if content.columns.len() == table.columns.len()
                && content.rows.len() == table.rows.len()
                && content
                    .rows
                    .iter()
                    .zip(&table.rows)
                    .all(|(source, target)| source.cells.len() == target.cells.len())
            {
                merge_contents(table, &content);
            } else {
                diagnose(
                    diagnostics,
                    handle,
                    "TABLECONTENT dimensions do not match ACAD_TABLE",
                );
            }
        }
        if let Some(geometry) = geometry {
            if geometry.columns as usize == table.columns.len()
                && geometry.rows as usize == table.rows.len()
                && geometry.cells.len()
                    == table.rows.iter().map(|row| row.cells.len()).sum::<usize>()
            {
                merge_geometry(table, &geometry);
            } else {
                diagnose(
                    diagnostics,
                    handle,
                    "TABLEGEOMETRY dimensions do not match ACAD_TABLE",
                );
            }
        }
    }
}

fn diagnose(diagnostics: &mut Vec<ReadDiagnostic>, table: Handle, message: impl Into<String>) {
    let mut diagnostic = ReadDiagnostic::new("invalid-table-adjunct", ReadStage::Objects, message);
    diagnostic.section = Some("OBJECTS".into());
    diagnostic.record_type = Some("ACAD_TABLE".into());
    diagnostic.record_handle = Some(table.value());
    push_read_diagnostic(diagnostics, diagnostic);
}

fn merge_geometry(table: &mut Table, geometry: &TableGeometry) {
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

fn merge_contents(table: &mut Table, content: &Table) {
    table.name.clone_from(&content.name);
    table.description.clone_from(&content.description);
    table.field_handles.clone_from(&content.field_handles);
    table.base_style.clone_from(&content.base_style);
    for (column, source) in table.columns.iter_mut().zip(&content.columns) {
        column.name.clone_from(&source.name);
        column.style.clone_from(&source.style);
        column.style_id = source.style_id;
        column.custom_data = source.custom_data;
        column
            .custom_data_items
            .clone_from(&source.custom_data_items);
    }
    for (row, source_row) in table.rows.iter_mut().zip(&content.rows) {
        row.style.clone_from(&source_row.style);
        row.style_id = source_row.style_id;
        row.custom_data = source_row.custom_data;
        row.custom_data_items
            .clone_from(&source_row.custom_data_items);
        for (cell, source) in row.cells.iter_mut().zip(&source_row.cells) {
            cell.style_id = source.style_id;
            cell.tooltip.clone_from(&source.tooltip);
            cell.state |= source.state;
            cell.custom_data = source.custom_data;
            cell.custom_data_items.clone_from(&source.custom_data_items);
            cell.has_linked_data = source.has_linked_data;
            cell.data_link_handle = source.data_link_handle;
            cell.data_link_rows = source.data_link_rows;
            cell.data_link_columns = source.data_link_columns;
            cell.data_link_unknown = source.data_link_unknown;
            // The table record gives every cell an empty text; the content
            // object tells which cells hold no content at all.
            if source.contents.is_empty()
                && cell.contents.iter().all(|c| {
                    c.block_handle.is_none()
                        && c.field_handle.is_none()
                        && c.value.display().is_empty()
                })
            {
                cell.contents.clear();
            }
            // The legacy entity has only one slot and may put a generated
            // display block there. The modern record holds the authored items.
            if source.contents.len() > 1
                || source.contents.first().is_some_and(|item| {
                    matches!(
                        item.content_type,
                        TableCellContentType::Block | TableCellContentType::Field
                    )
                })
            {
                cell.contents.clone_from(&source.contents);
                cell.cell_type = if cell.contents[0].content_type == TableCellContentType::Block {
                    CellType::Block
                } else {
                    CellType::Text
                };
            }
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
                    // The rest of the content's format (text style, height,
                    // color, rotation, alignment) is kept there too.
                    value.text_style_handle = from.text_style_handle;
                    value.text_style_name = from.text_style_name.clone();
                    value.color = from.color.clone();
                    value.rotation = from.rotation;
                    value.scale = from.scale;
                    value.text_height = from.text_height;
                    value.alignment = from.alignment;
                }
            }
            let Some(from) = source.style.as_ref() else {
                continue;
            };
            if cell.style.is_none() {
                cell.style = Some(from.clone());
                continue;
            }
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
                    if let Some(section) = table_section(xrecord) {
                        xrecord.entries.drain(section);
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
                let mut object =
                    DataObject::new(DataObjectData::TableGeometry(TableGeometry::default()));
                object.handle = geometry;
                object.owner = record;
                document
                    .objects
                    .insert(geometry, ObjectType::DataObject(object));
                if let Some(ObjectType::XRecord(xrecord)) = document.objects.get_mut(&record) {
                    if let Some(section) = table_section(xrecord) {
                        if let Some(entry) = xrecord.entries[section.clone()]
                            .iter_mut()
                            .find(|entry| entry.code == 361)
                        {
                            entry.value = XRecordValue::Handle(geometry);
                        } else {
                            xrecord.add_handle(361, geometry);
                            let entry = xrecord.entries.pop().expect("added geometry entry");
                            xrecord.entries.insert(section.end, entry);
                        }
                    }
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
                if let Some(section) = table_section(xrecord) {
                    if let Some(entry) = xrecord.entries[section]
                        .iter_mut()
                        .filter(|entry| entry.code == 90)
                        .nth(2)
                    {
                        entry.value = XRecordValue::Int32(last_row);
                    }
                }
            }
        }
    }
}
