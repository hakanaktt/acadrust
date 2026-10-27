//! Tests that DWG write preserves current text and dimension styles ($TEXTSTYLE, $DIMSTYLE)
//! when given as names (e.g. from DXF or set programmatically on CadDocument).
//!
//! Issue #227: DWG write and read lose the current text and dimension styles ($TEXTSTYLE, $DIMSTYLE):
//! they become Standard.

use acadrust::tables::{DimStyle, TextStyle};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader};
use std::io::Cursor;

const DXF_CUSTOM_STYLES: &str = r#"  0
SECTION
  2
HEADER
  9
$ACADVER
  1
AC1032
  9
$TEXTSTYLE
  7
MYTXT
  9
$DIMSTYLE
  2
MYDIM
  0
ENDSEC
  0
SECTION
  2
TABLES
  0
TABLE
  2
STYLE
 70
1
  0
STYLE
  2
MYTXT
 70
0
 40
0.0
 41
1.0
 50
0.0
 71
0
 42
2.5
  3
txt
  4

  0
ENDTAB
  0
TABLE
  2
DIMSTYLE
 70
1
  0
DIMSTYLE
  2
MYDIM
 70
0
  0
ENDTAB
  0
ENDSEC
  0
SECTION
  2
BLOCKS
  0
ENDSEC
  0
SECTION
  2
ENTITIES
  0
ENDSEC
  0
SECTION
  2
OBJECTS
  0
ENDSEC
  0
EOF
"#;

#[test]
fn dxf_to_dwg_keeps_current_styles() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_CUSTOM_STYLES))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    assert_eq!(doc.header.current_text_style_name, "MYTXT");
    assert_eq!(doc.header.current_dimstyle_name, "MYDIM");

    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read dwg");

    assert_eq!(
        back.header.current_text_style_name, "MYTXT",
        "current text style name must survive DWG roundtrip"
    );
    assert_eq!(
        back.header.current_dimstyle_name, "MYDIM",
        "current dimstyle name must survive DWG roundtrip"
    );
}

#[test]
fn programmatic_style_names_kept_in_dwg() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);

    let mut text = TextStyle::new("MYTXT");
    text.handle = doc.allocate_handle();
    doc.text_styles.add(text).expect("add text style");

    let mut dim = DimStyle::new("MYDIM");
    dim.handle = doc.allocate_handle();
    doc.dim_styles.add(dim).expect("add dim style");

    doc.header.current_text_style_name = "MYTXT".to_string();
    doc.header.current_dimstyle_name = "MYDIM".to_string();

    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read dwg");

    assert_eq!(
        back.header.current_text_style_name, "MYTXT",
        "programmatic current text style name must survive DWG roundtrip"
    );
    assert_eq!(
        back.header.current_dimstyle_name, "MYDIM",
        "programmatic current dimstyle name must survive DWG roundtrip"
    );
}

#[test]
fn case_insensitive_style_name_matching_in_dwg() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);

    let mut text = TextStyle::new("MYTXT");
    text.handle = doc.allocate_handle();
    doc.text_styles.add(text).expect("add text style");

    let mut dim = DimStyle::new("MYDIM");
    dim.handle = doc.allocate_handle();
    doc.dim_styles.add(dim).expect("add dim style");

    doc.header.current_text_style_name = "mytxt".to_string();
    doc.header.current_dimstyle_name = "mydim".to_string();

    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read dwg");

    assert!(
        back.header.current_text_style_name.eq_ignore_ascii_case("MYTXT"),
        "expected MYTXT (case-insensitive), got {}",
        back.header.current_text_style_name
    );
    assert!(
        back.header.current_dimstyle_name.eq_ignore_ascii_case("MYDIM"),
        "expected MYDIM (case-insensitive), got {}",
        back.header.current_dimstyle_name
    );
}
