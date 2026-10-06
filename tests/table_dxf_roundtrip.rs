//! Synthetic DXF round-trip for ACAD_TABLE cell content (fixture-independent).
//!
//! Builds a small table with text cells, column widths and row heights, writes
//! it to DXF and reads it back, asserting the structure and every cell's text
//! survive. This exercises both the DXF writer's cell emission and the DXF
//! reader's cell parser.

use std::io::Cursor;

use acadrust::entities::{EntityType, Table, TableCell};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

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
