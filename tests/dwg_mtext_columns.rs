//! MTEXT columns survive a DWG save in every version that has them.
//!
//! R2018 stores the columns in the MTEXT record. R2007 to R2013 have no such
//! fields: AutoCAD keeps the column layout in ACAD extended data
//! (`ACAD_MTEXT_COLUMN_INFO_BEGIN` ... `ACAD_MTEXT_COLUMN_INFO_END`), as the
//! DXF of those versions does. The DWG writer used to drop the columns there.

use acadrust::entities::{EntityType, MText, MTextColumnData};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

const VERSIONS: [DxfVersion; 4] = [
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

fn columns_after_dwg_save(version: DxfVersion, columns: &MTextColumnData) -> MTextColumnData {
    let mut doc = CadDocument::with_version(version);
    let mut mtext = MText::new();
    mtext.value = "first\\Psecond\\Pthird".into();
    mtext.insertion_point = Vector3::new(0.0, 100.0, 0.0);
    mtext.height = 2.5;
    mtext.rectangle_width = 50.0;
    mtext.column_data = columns.clone();
    let handle = doc.add_entity(EntityType::MText(Box::new(mtext))).unwrap();
    let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let back = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg");
    match back.get_entity(handle) {
        Some(EntityType::MText(m)) => m.column_data.clone(),
        other => panic!("{version:?}: MTEXT lost, read {other:?}"),
    }
}

#[test]
fn static_columns_survive_dwg_save() {
    let columns = MTextColumnData {
        column_type: 1,
        column_count: 3,
        flow_reversed: false,
        auto_height: false,
        width: 50.0,
        gutter: 5.0,
        heights: Vec::new(),
    };
    for version in VERSIONS {
        assert_eq!(
            columns_after_dwg_save(version, &columns),
            columns,
            "{version:?}"
        );
    }
}

#[test]
fn dynamic_columns_with_heights_survive_dwg_save() {
    let columns = MTextColumnData {
        column_type: 2,
        column_count: 2,
        flow_reversed: false,
        auto_height: false,
        width: 40.0,
        gutter: 4.0,
        heights: vec![20.0, 30.0],
    };
    for version in VERSIONS {
        assert_eq!(
            columns_after_dwg_save(version, &columns),
            columns,
            "{version:?}"
        );
    }
}
