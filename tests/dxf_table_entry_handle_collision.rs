//! Two entries of different symbol tables that share a handle in a DXF are
//! given distinct handles on read.

use std::collections::HashMap;
use std::io::Cursor;

use acadrust::{CadDocument, DxfReader, DxfWriter};

/// R2000 DXF as a third-party teaching program writes it: the APPID `ACAD` and
/// the DIMSTYLE `ISO-25` both carry handle 10C, and no record has a group 330.
fn dxf(appid_handle: &str, dimstyle_handle: &str) -> String {
    let rows = [
        "0",
        "SECTION",
        "2",
        "HEADER",
        "9",
        "$ACADVER",
        "1",
        "AC1015",
        "9",
        "$HANDSEED",
        "5",
        "540",
        "0",
        "ENDSEC",
        "0",
        "SECTION",
        "2",
        "TABLES",
        "0",
        "TABLE",
        "2",
        "APPID",
        "5",
        "10B",
        "100",
        "AcDbSymbolTable",
        "70",
        "1",
        "0",
        "APPID",
        "5",
        appid_handle,
        "100",
        "AcDbSymbolTableRecord",
        "100",
        "AcDbRegAppTableRecord",
        "2",
        "ACAD",
        "70",
        "0",
        "0",
        "ENDTAB",
        "0",
        "TABLE",
        "2",
        "DIMSTYLE",
        "5",
        "10D",
        "100",
        "AcDbSymbolTable",
        "70",
        "1",
        "100",
        "AcDbDimStyleTable",
        "71",
        "1",
        "0",
        "DIMSTYLE",
        "105",
        dimstyle_handle,
        "100",
        "AcDbSymbolTableRecord",
        "100",
        "AcDbDimStyleTableRecord",
        "2",
        "ISO-25",
        "70",
        "0",
        "0",
        "ENDTAB",
        "0",
        "ENDSEC",
        "0",
        "EOF",
    ];
    rows.iter().map(|row| format!("{row}\n")).collect()
}

fn read(text: &[u8]) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.to_vec()))
        .unwrap()
        .read()
        .unwrap()
}

fn table_handles(document: &CadDocument) -> Vec<(String, u64)> {
    let mut all = Vec::new();
    all.extend(
        document
            .layers
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .line_types
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .text_styles
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .dim_styles
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .app_ids
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .views
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .vports
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .ucss
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all.extend(
        document
            .block_records
            .iter()
            .map(|e| (e.name.clone(), e.handle.value())),
    );
    all
}

fn assert_unique(document: &CadDocument, stage: &str) {
    let mut seen: HashMap<u64, String> = HashMap::new();
    for (name, handle) in table_handles(document) {
        if let Some(other) = seen.insert(handle, name.clone()) {
            panic!("{stage}: {other} and {name} share handle {handle:X}");
        }
    }
}

#[test]
fn appid_and_dimstyle_with_one_handle_are_told_apart() {
    let document = read(dxf("10C", "10C").as_bytes());
    let appid = document.app_ids.get("ACAD").unwrap().handle.value();
    let dimstyle = document.dim_styles.get("ISO-25").unwrap().handle.value();
    assert_ne!(appid, dimstyle);
    assert_eq!(dimstyle, 0x10C, "the DIMSTYLE keeps its handle");
    assert_unique(&document, "read");
}

#[test]
fn written_dxf_has_no_repeated_table_handles() {
    let document = read(dxf("10C", "10C").as_bytes());
    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    assert_unique(&read(&bytes), "written and read back");
}

#[test]
fn handles_without_a_repeat_stay_as_they_are() {
    let document = read(dxf("10C", "10E").as_bytes());
    assert_eq!(document.app_ids.get("ACAD").unwrap().handle.value(), 0x10C);
    assert_eq!(
        document.dim_styles.get("ISO-25").unwrap().handle.value(),
        0x10E
    );
    assert_unique(&document, "read");
}
