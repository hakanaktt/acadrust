//! Header variables the DWG reader read and threw away, and the DXF reader
//! skipped: the orthographic UCS origins of both spaces, the R2010 dimension
//! zero suppression variables and text direction, and the casting shadow mode.

use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader};
use std::io::Cursor;

fn v(n: f64) -> Vector3 {
    Vector3::new(n, n + 1.0, n + 2.0)
}

fn set(doc: &mut CadDocument) {
    let h = &mut doc.header;
    h.dim_text_direction = true;
    h.dim_alt_mzf = 12.5;
    h.dim_alt_mzs = "alt".to_string();
    h.dim_mzf = 7.5;
    h.dim_mzs = "mz".to_string();
    h.cast_shadow = 3;
    for (i, o) in h.ucs_ortho_origins.iter_mut().enumerate() {
        *o = v(10.0 * (i as f64 + 1.0));
    }
    for (i, o) in h.paper_ucs_ortho_origins.iter_mut().enumerate() {
        *o = v(100.0 * (i as f64 + 1.0));
    }
}

fn roundtrip(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    set(&mut doc);
    let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg")
}

fn check_origins(h: &acadrust::document::HeaderVariables, version: DxfVersion) {
    for (i, o) in h.ucs_ortho_origins.iter().enumerate() {
        assert_eq!(*o, v(10.0 * (i as f64 + 1.0)), "{version:?} UCSORG #{i}");
    }
    for (i, o) in h.paper_ucs_ortho_origins.iter().enumerate() {
        assert_eq!(*o, v(100.0 * (i as f64 + 1.0)), "{version:?} PUCSORG #{i}");
    }
}

#[test]
fn ac1015_orthographic_origins() {
    let doc = roundtrip(DxfVersion::AC1015);
    check_origins(&doc.header, DxfVersion::AC1015);
}

#[test]
fn ac1021_orthographic_origins_and_shadow_mode() {
    let doc = roundtrip(DxfVersion::AC1021);
    check_origins(&doc.header, DxfVersion::AC1021);
    assert_eq!(doc.header.cast_shadow, 3);
}

#[test]
fn ac1032_zero_suppression_and_text_direction() {
    let doc = roundtrip(DxfVersion::AC1032);
    let h = &doc.header;
    check_origins(h, DxfVersion::AC1032);
    assert!(h.dim_text_direction);
    assert_eq!(h.dim_alt_mzf, 12.5);
    assert_eq!(h.dim_alt_mzs, "alt");
    assert_eq!(h.dim_mzf, 7.5);
    assert_eq!(h.dim_mzs, "mz");
    assert_eq!(h.cast_shadow, 3);
}

#[test]
fn the_dxf_reader_takes_them_too() {
    let text = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1032\n\
  9\n$DIMMZS\n  6\nxx\n  9\n$DIMMZF\n 40\n1.5\n  9\n$DIMALTMZS\n  6\nyy\n  9\n$DIMALTMZF\n 40\n2.5\n\
  9\n$DIMTXTDIRECTION\n 70\n     1\n  9\n$CSHADOW\n280\n     2\n\
  9\n$UCSORGTOP\n 10\n1.0\n 20\n2.0\n 30\n3.0\n  9\n$PUCSORGBACK\n 10\n4.0\n 20\n5.0\n 30\n6.0\n\
  0\nENDSEC\n  0\nEOF\n";
    let h = DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("open")
        .read()
        .expect("read")
        .header;
    assert_eq!(h.dim_mzs, "xx");
    assert_eq!(h.dim_mzf, 1.5);
    assert_eq!(h.dim_alt_mzs, "yy");
    assert_eq!(h.dim_alt_mzf, 2.5);
    assert!(h.dim_text_direction);
    assert_eq!(h.cast_shadow, 2);
    assert_eq!(h.ucs_ortho_origins[0], Vector3::new(1.0, 2.0, 3.0));
    assert_eq!(h.paper_ucs_ortho_origins[5], Vector3::new(4.0, 5.0, 6.0));
}
