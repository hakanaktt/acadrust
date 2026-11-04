//! BLOCK records carry their AcDbEntity groups once, as AutoCAD writes them.
//!
//! The writer emitted the common entity data (which already holds `8`) and
//! then a second `8 0`, plus the paper-space `67 1` after the first `8`
//! (AutoCAD: `67` before `8`, one `8`).

use acadrust::{DxfReader, DxfWriter};
use std::io::Cursor;

/// The fixture written by AutoCAD (see `tests/datatable/README.md`).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/datatable/point_object_id.dxf"
);

/// For every BLOCK record: its handle and the pairs from `100 AcDbEntity` up to
/// `100 AcDbBlockBegin`.
fn block_entity_groups(text: &str) -> Vec<(String, Vec<(i32, String)>)> {
    let lines: Vec<&str> = text.lines().collect();
    let pairs: Vec<(i32, String)> = lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap_or(-1), c[1].trim().to_string()))
        .collect();
    let mut out = Vec::new();
    for (i, pair) in pairs.iter().enumerate() {
        if *pair != (0, "BLOCK".to_string()) {
            continue;
        }
        let record: Vec<(i32, String)> = pairs[i + 1..]
            .iter()
            .take_while(|(code, _)| *code != 0)
            .cloned()
            .collect();
        let handle = record
            .iter()
            .find(|(code, _)| *code == 5)
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        let entity: Vec<(i32, String)> = record
            .iter()
            .skip_while(|p| **p != (100, "AcDbEntity".to_string()))
            .take_while(|p| **p != (100, "AcDbBlockBegin".to_string()))
            .cloned()
            .collect();
        out.push((handle, entity));
    }
    out
}

#[test]
fn block_records_write_layer_and_paper_space_flag_once_like_autocad() {
    let source = std::fs::read_to_string(FIXTURE).expect("fixture");
    let doc = DxfReader::from_reader(Cursor::new(source.clone().into_bytes()))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let written = String::from_utf8(
        DxfWriter::new(&doc)
            .with_version(doc.version)
            .write_to_vec()
            .expect("write dxf"),
    )
    .unwrap();
    let before = block_entity_groups(&source);
    assert!(
        before
            .iter()
            .any(|(_, groups)| groups.iter().any(|g| *g == (67, "1".into()))),
        "fixture has a paper-space block"
    );
    assert_eq!(block_entity_groups(&written), before);
}
