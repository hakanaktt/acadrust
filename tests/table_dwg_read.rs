//! Verifies the ACAD_TABLE DWG reader against a real, AutoCAD-authored fixture.
//!
//! A table is INSERT-derived: its insert base positions it and links it to the
//! block that renders its cells, and the inline table body carries the columns,
//! rows and cell contents. This reads both — the placement and the actual cell
//! text — from a real R2007 drawing (two schedules).

use std::io::Cursor;

use acadrust::entities::EntityType;
use acadrust::types::Handle;
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn load_doc() -> Option<CadDocument> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/roundtrip/blocks_and_tables_metric.dwg"
    );
    if !std::path::Path::new(path).exists() {
        eprintln!("fixture blocks_and_tables_metric.dwg missing — skipping");
        return None;
    }
    let mut reader = DwgReader::from_file(path).expect("open fixture");
    Some(reader.read().expect("read fixture"))
}

fn tables_of(doc: &CadDocument) -> Vec<acadrust::entities::Table> {
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Table(t) => Some((**t).clone()),
            _ => None,
        })
        .collect()
}

fn load_tables() -> Option<Vec<acadrust::entities::Table>> {
    load_doc().map(|d| tables_of(&d))
}

#[test]
fn table_base_is_positioned_and_block_linked() {
    let Some(tables) = load_tables() else { return };
    assert_eq!(tables.len(), 2, "expected two ACAD_TABLE entities");
    for t in &tables {
        let block = t.block_record_handle.expect("missing block_record_handle");
        assert_ne!(block, Handle::NULL, "null block handle");
        assert!(t.insertion_point.x.is_finite() && t.insertion_point.y.is_finite());
    }
}

#[test]
fn table_cell_content_is_parsed() {
    let Some(tables) = load_tables() else { return };

    // Collect each table's first-row title so order doesn't matter.
    let title = |t: &acadrust::entities::Table| -> String {
        t.rows
            .first()
            .and_then(|r| r.cells.first())
            .map(|c| c.text_value().to_string())
            .unwrap_or_default()
    };
    let titles: Vec<String> = tables.iter().map(title).collect();
    assert!(
        titles.iter().any(|s| s == "DOOR SCHEDULE"),
        "missing DOOR SCHEDULE table, got {titles:?}"
    );
    assert!(
        titles.iter().any(|s| s == "WINDOW SCHEDULE"),
        "missing WINDOW SCHEDULE table, got {titles:?}"
    );

    let door = tables.iter().find(|t| title(t) == "DOOR SCHEDULE").unwrap();
    assert_eq!(door.columns.len(), 9, "door schedule column count");
    assert!(door.rows.len() >= 3, "door schedule row count");

    // Header row.
    let header: Vec<String> = door.rows[1]
        .cells
        .iter()
        .map(|c| c.text_value().to_string())
        .collect();
    assert_eq!(
        header,
        vec![
            "SYM.",
            "WIDTH",
            "HEIGHT",
            "STYLE",
            "REF#",
            "MANUFACTURER",
            "QTY",
            "COST",
            "TOTAL",
        ],
        "door schedule header row"
    );

    // First data row.
    assert_eq!(door.rows[2].cells[0].text_value(), "1");
    assert_eq!(door.rows[2].cells[3].text_value(), "TWO PANEL");
    assert_eq!(door.rows[2].cells[5].text_value(), "TRU STYLE");
}

#[test]
fn real_tables_survive_a_dwg_write() {
    // Read the real R2007 drawing, write it back to DWG and re-read: the tables
    // and their cell text must survive the writer.
    let Some(doc) = load_doc() else { return };

    let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write");
    let rt = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG re-read");
    let tables = tables_of(&rt);

    let door = tables
        .iter()
        .find(|t| {
            t.rows
                .first()
                .and_then(|r| r.cells.first())
                .map(|c| c.text_value() == "DOOR SCHEDULE")
                .unwrap_or(false)
        })
        .expect("DOOR SCHEDULE lost on DWG write");

    let header: Vec<String> = door.rows[1]
        .cells
        .iter()
        .map(|c| c.text_value().to_string())
        .collect();
    assert_eq!(header[0], "SYM.");
    assert_eq!(header[5], "MANUFACTURER");
    assert_eq!(door.rows[2].cells[3].text_value(), "TWO PANEL");
}
