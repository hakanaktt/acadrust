//! UNITMODE, DIMUNIT, DIMFIT and `$ACADMAINTVER` are read from the header and kept.
//! The DWG header reader read UNITMODE and DIMUNIT and dropped them (the writer put
//! back 0), the DXF reader merged `$DIMFIT` into `$DIMATFIT` and skipped
//! `$ACADMAINTVER`.

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader};
use std::io::Cursor;

fn dwg_roundtrip(version: DxfVersion, set: impl FnOnce(&mut CadDocument)) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    set(&mut doc);
    let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg")
}

fn dxf_header(vars: &str) -> acadrust::document::HeaderVariables {
    let text = format!("  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1032\n{vars}  0\nENDSEC\n  0\nEOF\n");
    DxfReader::from_reader(Cursor::new(text.into_bytes()))
        .expect("open")
        .read()
        .expect("read")
        .header
}

#[test]
fn dwg_keeps_unitmode_in_every_version() {
    for v in [DxfVersion::AC1014, DxfVersion::AC1015, DxfVersion::AC1032] {
        let doc = dwg_roundtrip(v, |d| d.header.unit_mode = 1);
        assert_eq!(doc.header.unit_mode, 1, "{v:?} UNITMODE");
    }
}

#[test]
fn dwg_r14_keeps_dimunit() {
    let doc = dwg_roundtrip(DxfVersion::AC1014, |d| d.header.dim_unit = 4);
    assert_eq!(doc.header.dim_unit, 4);
}

#[test]
fn dwg_r14_reads_dimfit_as_the_r13_variable_too() {
    let doc = dwg_roundtrip(DxfVersion::AC1014, |d| d.header.dim_fit = 1);
    assert_eq!(doc.header.dim_fit_r13, 1);
}

#[test]
fn dxf_reads_unitmode_and_dimunit() {
    let h = dxf_header("  9\n$UNITMODE\n 70\n     1\n  9\n$DIMUNIT\n 70\n     3\n");
    assert_eq!(h.unit_mode, 1);
    assert_eq!(h.dim_unit, 3);
}

#[test]
fn dxf_keeps_dimfit_apart_from_dimatfit() {
    let h = dxf_header("  9\n$DIMATFIT\n 70\n     2\n  9\n$DIMFIT\n 70\n     1\n");
    assert_eq!(h.dim_fit, 2, "$DIMATFIT wins over the R13 variable");
    assert_eq!(h.dim_fit_r13, 1);
}

#[test]
fn dxf_dimfit_alone_still_feeds_dimfit() {
    let h = dxf_header("  9\n$DIMFIT\n 70\n     1\n");
    assert_eq!(h.dim_fit, 1);
    assert_eq!(h.dim_fit_r13, 1);
}

#[test]
fn dxf_reads_acadmaintver() {
    let h = dxf_header("  9\n$ACADMAINTVER\n 90\n   329\n");
    assert_eq!(h.acad_maintenance_version, 329);
}
