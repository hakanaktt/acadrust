//! Synthetic DXF round-trip for ACAD_TABLE cell content (fixture-independent).
//!
//! Builds a small table with text cells, column widths and row heights, writes
//! it to DXF and reads it back, asserting the structure and every cell's text
//! survive. This exercises both the DXF writer's cell emission and the DXF
//! reader's cell parser.

use std::io::Cursor;

use acadrust::entities::table::TableCellContentType;
use acadrust::entities::{CellContent, EntityType, Table, TableCell};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

#[test]
fn table_keeps_ordered_block_and_value_items_in_both_dxf_encodings() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut block = BlockRecord::new("symbol");
    block.handle = doc.allocate_handle();
    block.block_entity_handle = doc.allocate_handle();
    block.block_end_handle = doc.allocate_handle();
    let block_handle = block.handle;
    doc.block_records.add(block).unwrap();

    let mut table = Table::new(Vector3::ZERO, 1, 2);
    table.name = "Symbol schedule".into();
    table.columns[0].name = "Symbol".into();
    table.rows[0].style_id = 2;
    table.rows[0].cells[0] = TableCell::block(block_handle);
    table.rows[0].cells[0].style_id = 3;
    table.rows[0].cells[0].tooltip = "Two symbols and a label".into();
    table.rows[0].cells[0].contents.extend([
        CellContent::block(block_handle),
        CellContent::text("after the symbols"),
    ]);
    doc.add_entity(EntityType::Table(Box::new(table))).unwrap();

    for binary in [false, true] {
        let mut writer = DxfWriter::new(&doc);
        writer.set_binary(binary);
        let rt = DxfReader::from_reader(Cursor::new(writer.write_to_vec().unwrap()))
            .unwrap()
            .read()
            .unwrap();
        let table = rt
            .entities()
            .find_map(|e| match e {
                EntityType::Table(t) => Some(t),
                _ => None,
            })
            .unwrap();
        let block = rt.block_records.get("symbol").unwrap().handle;
        let contents = &table.rows[0].cells[0].contents;
        assert_eq!(contents.len(), 3, "binary={binary}");
        assert_eq!(contents[0].block_handle, Some(block));
        assert_eq!(contents[1].block_handle, Some(block));
        assert_eq!(contents[2].value.text, "after the symbols");
        assert!(table.rows[0].cells[1].contents.is_empty());
        assert_eq!(table.name, "Symbol schedule", "binary={binary}");
        assert_eq!(table.columns[0].name, "Symbol");
        assert_eq!(table.rows[0].style_id, 2);
        assert_eq!(table.rows[0].cells[0].style_id, 3);
        assert_eq!(table.rows[0].cells[0].tooltip, "Two symbols and a label");
    }
}

#[test]
fn invalid_table_content_target_is_diagnosed_at_its_table() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut table = Table::new(Vector3::ZERO, 1, 1);
    table.rows[0].cells[0] = TableCell::text("legacy cell");
    let table_handle = document
        .add_entity(EntityType::Table(Box::new(table)))
        .unwrap();
    let ascii = String::from_utf8(DxfWriter::new(&document).write_to_vec().unwrap()).unwrap();
    let mut lines: Vec<_> = ascii.lines().collect();
    let marker = lines
        .iter()
        .position(|line| line.trim() == "ACAD_ROUNDTRIP_2008_TABLE_ENTITY")
        .unwrap();
    assert_eq!(lines[marker + 1].trim(), "360");
    lines[marker + 2] = "FFFFFFFE";
    let outcome = DxfReader::from_reader(Cursor::new(lines.join("\r\n").into_bytes()))
        .unwrap()
        .read_with_stats()
        .unwrap();
    assert!(outcome.stats.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "invalid-table-adjunct"
            && diagnostic.record_handle == Some(table_handle.value())
            && diagnostic.message.contains("Invalid TABLECONTENT target")
    }));
    let Some(EntityType::Table(table)) = outcome.document.get_entity(table_handle) else {
        panic!("missing table after fallback");
    };
    assert_eq!(table.rows[0].cells[0].text_value(), "legacy cell");
}

#[test]
#[ignore = "requires the separately supplied sample_AC1032 DXF pair"]
fn sample_ac1032_keeps_the_authored_items_of_both_block_cells() {
    let dir = std::path::PathBuf::from(std::env::var_os("ACADRUST_TABLE_SAMPLE_DIR").unwrap());
    for name in ["sample_AC1032_ascii.dxf", "sample_AC1032_binary.dxf"] {
        let mut document = DxfReader::from_file(dir.join(name))
            .unwrap()
            .read()
            .unwrap();
        let table = document
            .entities()
            .find_map(|entity| match entity {
                EntityType::Table(table) if table.rows.len() == 4 && table.columns.len() == 5 => {
                    Some(table)
                }
                _ => None,
            })
            .expect("the 4-by-5 table");
        let block = document.block_records.get("my_block").unwrap().handle;
        let d2 = &table.rows[1].cells[3].contents;
        let e2 = &table.rows[1].cells[4].contents;
        assert_eq!(d2.len(), 1, "{name} D2");
        assert_eq!(d2[0].content_type, TableCellContentType::Block, "{name} D2");
        assert_eq!(d2[0].block_handle, Some(block), "{name} D2");
        assert_eq!(e2.len(), 3, "{name} E2");
        assert_eq!(e2[0].block_handle, Some(block), "{name} E2 first item");
        assert_eq!(e2[1].block_handle, Some(block), "{name} E2 second item");
        assert_eq!(
            e2[2].value.text, "Cell with 2 blocks",
            "{name} E2 third item"
        );
        let handle = table.common.handle;
        let Some(EntityType::Table(table)) = document.get_entity_mut(handle) else {
            panic!("missing table {handle:?}");
        };
        table.rows[1].cells[4].contents[2].value.text = "Edited cell label".into();
        let mut writer = DxfWriter::new(&document);
        writer.set_binary(name.contains("binary"));
        let readback = DxfReader::from_reader(Cursor::new(writer.write_to_vec().unwrap()))
            .unwrap()
            .read()
            .unwrap();
        let table = readback
            .entities()
            .find_map(|entity| match entity {
                EntityType::Table(table) if table.rows.len() == 4 && table.columns.len() == 5 => {
                    Some(table)
                }
                _ => None,
            })
            .expect("the rewritten 4-by-5 table");
        let block = readback.block_records.get("my_block").unwrap().handle;
        let e2 = &table.rows[1].cells[4].contents;
        assert_eq!(e2.len(), 3, "rewritten {name} E2");
        assert_eq!(e2[0].block_handle, Some(block));
        assert_eq!(e2[1].block_handle, Some(block));
        assert_eq!(e2[2].value.text, "Edited cell label");
    }
}

#[test]
fn table_cell_content_dxf_roundtrip() {
    let mut table = Table::new(Vector3::new(1.0, 2.0, 0.0), 2, 3);
    table.columns[0].width = 10.0;
    table.columns[1].width = 20.0;
    table.columns[2].width = 30.0;
    table.rows[0].height = 5.0;
    table.rows[1].height = 7.0;
    table.rows[0].cells[0] = TableCell::text("Name");
    table.rows[0].cells[1] = TableCell::text("Qty");
    table.rows[0].cells[2] = TableCell::text("Cost");
    table.rows[1].cells[0] = TableCell::text("Bolt");
    table.rows[1].cells[1] = TableCell::text("10");
    table.rows[1].cells[2] = TableCell::text("2.50");

    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Table(Box::new(table))).unwrap();

    let bytes = DxfWriter::new(&doc).write_to_vec().expect("DXF write");
    let rt = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init")
        .read()
        .expect("DXF read");

    let t = rt
        .entities()
        .find_map(|e| match e {
            EntityType::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("table missing after DXF roundtrip");

    assert_eq!(t.rows.len(), 2, "row count");
    assert_eq!(t.columns.len(), 3, "column count");
    assert_eq!(t.columns[1].width, 20.0, "column width");
    assert_eq!(t.rows[1].height, 7.0, "row height");

    let row_text = |r: usize| -> Vec<String> {
        t.rows[r]
            .cells
            .iter()
            .map(|c| c.text_value().to_string())
            .collect()
    };
    assert_eq!(row_text(0), vec!["Name", "Qty", "Cost"], "header row");
    assert_eq!(row_text(1), vec!["Bolt", "10", "2.50"], "data row");
}
