//! Synthetic DWG round-trip for ACAD_TABLE (fixture-independent).
//!
//! Writes a table with text cells, column widths and row heights, reads it
//! back, and checks the structure and every cell's text survive — at both a
//! pre-R2010 version (flat format) and an R2010+ version (inline table
//! content), exercising both writer and reader paths.

use std::io::Cursor;

use acadrust::entities::{EntityType, Table, TableCell};
use acadrust::objects::{
    CellStyleMap, DataObject, DataObjectData, NamedTableCellStyle, ObjectType, TableCellStyleData,
};
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn sample_table() -> Table {
    let mut t = Table::new(Vector3::new(1.0, 2.0, 0.0), 2, 3);
    t.columns[0].width = 10.0;
    t.columns[1].width = 20.0;
    t.columns[2].width = 30.0;
    t.rows[0].height = 5.0;
    t.rows[1].height = 7.0;
    t.rows[0].cells[0] = TableCell::text("Name");
    t.rows[0].cells[1] = TableCell::text("Qty");
    t.rows[0].cells[2] = TableCell::text("Cost");
    t.rows[1].cells[0] = TableCell::text("Bolt");
    t.rows[1].cells[1] = TableCell::text("10");
    t.rows[1].cells[2] = TableCell::text("2.50");
    t
}

fn roundtrip(version: DxfVersion) -> Table {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::Table(Box::new(sample_table()))).unwrap();
    let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write");
    let rt = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read");
    let found = rt.entities().find_map(|e| match e {
        EntityType::Table(t) => Some((**t).clone()),
        _ => None,
    });
    found.expect("table missing after DWG roundtrip")
}

fn assert_table(t: &Table, label: &str) {
    assert_eq!(t.rows.len(), 2, "{label}: rows");
    assert_eq!(t.columns.len(), 3, "{label}: columns");
    assert_eq!(t.columns[1].width, 20.0, "{label}: column width");
    assert_eq!(t.rows[1].height, 7.0, "{label}: row height");
    let text = |r: usize| -> Vec<String> {
        t.rows[r]
            .cells
            .iter()
            .map(|c| c.text_value().to_string())
            .collect()
    };
    assert_eq!(text(0), vec!["Name", "Qty", "Cost"], "{label}: header");
    assert_eq!(text(1), vec!["Bolt", "10", "2.50"], "{label}: data");
}

#[test]
fn table_dwg_roundtrip_flat_r2007() {
    // AC1021 = R2007 → pre-R2010 flat cell format.
    let t = roundtrip(DxfVersion::AC1021);
    assert_table(&t, "R2007");
}

#[test]
fn table_dwg_roundtrip_content_r2018() {
    // AC1032 = R2018 → R2010+ inline table content.
    let t = roundtrip(DxfVersion::AC1032);
    assert_table(&t, "R2018");
}

#[test]
fn table_r2010_header_defaults_are_distinct_from_r2013() {
    let r2010 = roundtrip(DxfVersion::AC1024);
    assert_table(&r2010, "R2010");
    assert_eq!(r2010.dwg_r2010_unknown_bit, Some(true));
    let r2013 = roundtrip(DxfVersion::AC1027);
    assert_table(&r2013, "R2013");
    assert_eq!(r2013.dwg_unknown_long2, 0);
    assert_eq!(r2013.dwg_r2010_unknown_bit, None);
}

#[test]
fn table_r2010_explicit_header_bit_is_preserved() {
    for bit in [false, true] {
        let mut doc = CadDocument::with_version(DxfVersion::AC1024);
        let mut table = sample_table();
        table.dwg_r2010_unknown_bit = Some(bit);
        let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let Some(EntityType::Table(table)) = loaded.get_entity(handle) else {
            panic!()
        };
        assert_eq!(table.dwg_r2010_unknown_bit, Some(bit));
    }
}

#[test]
fn r2018_standard_table_style_has_valid_legacy_row_text_styles() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let standard_text_style = doc.text_styles.get("Standard").unwrap().handle;

    let standard_table_style = doc
        .objects
        .values_mut()
        .find_map(|object| match object {
            ObjectType::TableStyle(style) if style.name == "Standard" => Some(style),
            _ => None,
        })
        .expect("Standard table style");
    standard_table_style.set_all_text_styles("Standard", None);

    let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write");
    let rt = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read");
    let standard_table_style = rt
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::TableStyle(style) if style.name == "Standard" => Some(style),
            _ => None,
        })
        .expect("round-tripped Standard table style");

    let expected = [
        (1, 1, 1, "_TITLE"),
        (2, 2, 1, "_HEADER"),
        (3, 3, 2, "_DATA"),
    ];
    assert_eq!(standard_table_style.modern_overrides.len(), expected.len());
    for ((key, style), (expected_key, expected_id, expected_type, expected_name)) in
        standard_table_style.modern_overrides.iter().zip(expected)
    {
        assert_eq!(*key, expected_key);
        assert_eq!(style.id, expected_id);
        assert_eq!(style.style_type, expected_type);
        assert_eq!(style.name, expected_name);
        assert_eq!(
            style.cell_style.content_format.text_style,
            standard_text_style
        );
    }
}

#[test]
fn r2018_cell_style_map_preserves_inherited_text_style() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut object = DataObject::new(DataObjectData::CellStyleMap(CellStyleMap {
        cells: vec![NamedTableCellStyle {
            cell_style: TableCellStyleData {
                style_type: 5,
                data_flags: 1,
                ..TableCellStyleData::default()
            },
            id: 1,
            style_type: 1,
            name: "Inherited".to_string(),
        }],
    }));
    object.handle = doc.allocate_handle();
    doc.objects
        .insert(object.handle, ObjectType::DataObject(object));

    let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write");
    let rt = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read");
    let cell = rt
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::DataObject(DataObject {
                data: DataObjectData::CellStyleMap(style_map),
                ..
            }) => style_map.cells.first(),
            _ => None,
        })
        .expect("round-tripped cell style map");

    assert_eq!(cell.cell_style.content_format.text_style, Handle::NULL);
}
