//! The current text style, linetype and dimension style of a DWG come back as
//! names, the way the DXF reader gives them.
//!
//! The DWG header holds each of them as a handle. The reader resolved the
//! current layer and left the other three at their defaults ("Standard" and
//! "ByLayer"), so a drawing whose current dimension style is not Standard read as Standard.

use acadrust::tables::{DimStyle, LineType, TextStyle};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, TableEntry};

fn document(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);

    let mut text = TextStyle::new("MyText");
    text.set_handle(doc.allocate_handle());
    doc.header.current_text_style_handle = text.handle;
    doc.text_styles.add(text).unwrap();

    let mut dim = DimStyle::new("MyDim");
    dim.set_handle(doc.allocate_handle());
    doc.header.current_dimstyle_handle = dim.handle;
    doc.dim_styles.add(dim).unwrap();

    let mut lt = LineType::new("MyDash");
    lt.set_handle(doc.allocate_handle());
    doc.header.current_linetype_handle = lt.handle;
    doc.line_types.add(lt).unwrap();

    doc
}

fn roundtrip(version: DxfVersion) {
    let bytes = DwgWriter::write_to_vec(&document(version)).expect("write dwg");
    let loaded = DwgReader::from_stream(std::io::Cursor::new(bytes))
        .read()
        .expect("read dwg");
    let h = &loaded.header;
    assert_eq!(h.current_dimstyle_name, "MyDim", "{version:?} $DIMSTYLE");
    assert_eq!(h.current_text_style_name, "MyText", "{version:?} $TEXTSTYLE");
    assert_eq!(h.current_linetype_name, "MyDash", "{version:?} $CELTYPE");
}

#[test]
fn ac1015_current_styles_are_names() {
    roundtrip(DxfVersion::AC1015);
}

#[test]
fn ac1018_current_styles_are_names() {
    roundtrip(DxfVersion::AC1018);
}

#[test]
fn ac1032_current_styles_are_names() {
    roundtrip(DxfVersion::AC1032);
}
