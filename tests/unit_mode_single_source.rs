//! $UNITMODE must have one source of truth, `header.unit_mode`. It used to live in
//! both `header.unit_mode` and `header.legacy.unit_mode`, and the readers and
//! writers each picked a different one.

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

fn dwg(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("write dwg");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg")
}

fn dxf(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("write dxf");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf")
}

#[test]
fn dxf_writer_emits_unitmode_without_legacy_state() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.header.unit_mode = 1;
    assert!(doc.header.legacy.is_none());
    assert_eq!(dxf(&doc).header.unit_mode, 1);
}

#[test]
fn dwg_r2000_plus_to_dxf_keeps_unitmode() {
    for v in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let mut doc = CadDocument::with_version(v);
        doc.header.unit_mode = 1;
        let from_dwg = dwg(&doc);
        assert_eq!(from_dwg.header.unit_mode, 1, "{v:?} dwg read");
        assert_eq!(dxf(&from_dwg).header.unit_mode, 1, "{v:?} dwg to dxf");
    }
}

#[test]
fn r12_dwg_to_modern_dwg_keeps_unitmode() {
    let mut r12 = CadDocument::with_version(DxfVersion::AC1009);
    r12.header.unit_mode = 1;
    r12.header.legacy.get_or_insert_with(Default::default).unit_mode = 1;
    let read = dwg(&r12);
    assert_eq!(read.header.unit_mode, 1, "r12 reader mirrors into unit_mode");

    let mut modern = read;
    modern.version = DxfVersion::AC1032;
    assert_eq!(dwg(&modern).header.unit_mode, 1, "r12 to modern dwg");
}
