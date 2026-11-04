//! A pre-R2010 TABLESTYLE row must point at a real text style.
//!
//! The DXF reader only keeps the row's text style name (group 7), so a table
//! style read from a DXF file has no text style handle. The R2010+ writer
//! resolves the name; the legacy three-row writer wrote a null handle for
//! each row instead, which AutoCAD's AUDIT reports on the table style.

use std::io::Cursor;

use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn from_dxf(document: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(document).write_to_vec().expect("write DXF");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("open DXF")
        .read()
        .expect("read DXF")
}

fn row_text_styles(document: &CadDocument) -> Vec<Option<Handle>> {
    let style = document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::TableStyle(style) if style.name == "Standard" => Some(style),
            _ => None,
        })
        .expect("Standard table style");
    [
        &style.data_row_style,
        &style.title_row_style,
        &style.header_row_style,
    ]
    .iter()
    .map(|row| row.text_style_handle)
    .collect()
}

#[test]
fn legacy_table_style_rows_resolve_text_style_from_name() {
    let source = from_dxf(&CadDocument::with_version(DxfVersion::AC1032));
    let standard = source.text_styles.get("Standard").expect("Standard").handle;

    // R2000 has no TABLESTYLE class, so a cross-version save drops the
    // object there; the three-row layout is R2004 and R2007.
    for version in [DxfVersion::AC1018, DxfVersion::AC1021] {
        let mut document = source.clone();
        document.version = version;
        let bytes = DwgWriter::write_to_vec(&document).expect("write DWG");
        let back = DwgReader::from_stream(Cursor::new(bytes))
            .read()
            .expect("read DWG");

        assert_eq!(
            row_text_styles(&back),
            vec![Some(standard); 3],
            "{version:?}: legacy TABLESTYLE rows lost their text style"
        );
    }
}
