//! The dimension text style and linetypes of the header come back as names
//! (hakanaktt/acadrust#217), from a DWG (where the header holds handles) and
//! from a DXF (where it holds names).

use acadrust::tables::{LineType, TextStyle};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, TableEntry};
use std::io::Cursor;

fn document(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);

    let mut text = TextStyle::new("MyDimText");
    text.set_handle(doc.allocate_handle());
    doc.header.dim_text_style_handle = text.handle;
    doc.text_styles.add(text).unwrap();

    for (name, slot) in [("BATTING", 0), ("TRACKS", 1), ("HIDDEN2", 2)] {
        let mut lt = LineType::new(name);
        lt.set_handle(doc.allocate_handle());
        match slot {
            0 => doc.header.dim_linetype_handle = lt.handle,
            1 => doc.header.dim_linetype1_handle = lt.handle,
            _ => doc.header.dim_linetype2_handle = lt.handle,
        }
        doc.line_types.add(lt).unwrap();
    }
    doc
}

fn dwg_roundtrip(version: DxfVersion) {
    let bytes = DwgWriter::write_to_vec(&document(version)).expect("write dwg");
    let h = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg")
        .header;
    assert_eq!(h.dim_text_style_name, "MyDimText", "{version:?} $DIMTXSTY");
    // The DWG header holds the linetype handles from R2007 on.
    if version < DxfVersion::AC1021 {
        return;
    }
    assert_eq!(h.dim_linetype_name, "BATTING", "{version:?} $DIMLTYPE");
    assert_eq!(h.dim_linetype1_name, "TRACKS", "{version:?} $DIMLTEX1");
    assert_eq!(h.dim_linetype2_name, "HIDDEN2", "{version:?} $DIMLTEX2");
}

#[test]
fn ac1015_dwg_gives_the_dimension_names() {
    dwg_roundtrip(DxfVersion::AC1015);
}

#[test]
fn ac1021_dwg_gives_the_dimension_names() {
    dwg_roundtrip(DxfVersion::AC1021);
}

#[test]
fn ac1032_dwg_gives_the_dimension_names() {
    dwg_roundtrip(DxfVersion::AC1032);
}

#[test]
fn the_dxf_reader_takes_the_dimension_names() {
    let text = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1032\n\
  9\n$DIMTXSTY\n  7\nMyDimText\n  9\n$DIMLTYPE\n  6\nBATTING\n  9\n$DIMLTEX1\n  6\nTRACKS\n\
  9\n$DIMLTEX2\n  6\nHIDDEN2\n  0\nENDSEC\n  0\nEOF\n";
    let h = DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("init")
        .read()
        .expect("read dxf")
        .header;
    assert_eq!(h.dim_text_style_name, "MyDimText");
    assert_eq!(h.dim_linetype_name, "BATTING");
    assert_eq!(h.dim_linetype1_name, "TRACKS");
    assert_eq!(h.dim_linetype2_name, "HIDDEN2");
}

#[test]
fn a_new_document_starts_with_autocads_names() {
    let h = CadDocument::new().header;
    assert_eq!(h.dim_text_style_name, "Standard");
    assert_eq!(h.dim_linetype_name, "ByBlock");
    assert_eq!(h.dim_linetype1_name, "ByBlock");
    assert_eq!(h.dim_linetype2_name, "ByBlock");
}
