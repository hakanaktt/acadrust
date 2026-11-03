//! Named plot styles (group 390) on entities and LAYER records must be read
//! from DXF and survive the DWG roundtrip.
//!
//! The DXF reader ignores group 390 everywhere: an entity's `plotstyle_flags`
//! stay 0 (ByLayer) and a layer's `plotstyle_handle` stays null, so a DWG
//! written from a named-plot-style DXF (`$PSTYLEMODE 0`) loses every plot
//! style assignment, although the ACDBPLACEHOLDER objects are kept.

use acadrust::entities::EntityType;
use acadrust::objects::ObjectType;
use acadrust::types::Handle;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

/// Root dictionary C, ACAD_PLOTSTYLENAME dictionary E with placeholders
/// F (Normal) and 8A (Review); layer PS_LAYER and a LINE both use Review.
const DXF_NAMED_PLOT_STYLES: &str = r#"  0
SECTION
  2
HEADER
  9
$ACADVER
  1
AC1032
  9
$PSTYLEMODE
290
0
  0
ENDSEC
  0
SECTION
  2
TABLES
  0
TABLE
  2
LAYER
  5
2
 70
2
  0
LAYER
  5
10
  2
0
 70
0
 62
7
  6
Continuous
390
F
  0
LAYER
  5
11
  2
PS_LAYER
 70
0
 62
3
  6
Continuous
390
8A
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
LINE
  5
100
  8
PS_LAYER
390
8A
 10
0.0
 20
0.0
 30
0.0
 11
10.0
 21
10.0
 31
0.0
  0
ENDSEC
  0
SECTION
  2
OBJECTS
  0
DICTIONARY
  5
C
330
0
100
AcDbDictionary
281
1
  3
ACAD_PLOTSTYLENAME
350
E
  0
ACDBDICTIONARYWDFLT
  5
E
330
C
100
AcDbDictionary
281
1
  3
Normal
350
F
  3
Review
350
8A
100
AcDbDictionaryWithDefault
340
F
  0
ACDBPLACEHOLDER
  5
F
330
E
  0
ACDBPLACEHOLDER
  5
8A
330
E
  0
ENDSEC
  0
EOF
"#;

const REVIEW: Handle = Handle::new(0x8A);

fn assert_review_everywhere(doc: &CadDocument, stage: &str) {
    let layer = doc
        .layers
        .iter()
        .find(|l| l.name == "PS_LAYER")
        .unwrap_or_else(|| panic!("{stage}: layer PS_LAYER"));
    assert_eq!(
        layer.plotstyle_handle, REVIEW,
        "{stage}: PS_LAYER's plot style (390) must be the Review placeholder"
    );

    let line = doc
        .entities()
        .find(|e| matches!(e, EntityType::Line(_)))
        .unwrap_or_else(|| panic!("{stage}: the LINE"));
    assert_eq!(
        line.common().plotstyle_flags,
        0b11,
        "{stage}: the LINE must carry a plot style handle"
    );
    assert_eq!(
        line.common().plotstyle_handle,
        Some(REVIEW),
        "{stage}: the LINE's plot style (390) must be the Review placeholder"
    );

    assert!(
        matches!(doc.objects.get(&REVIEW), Some(ObjectType::PlaceHolder(_))),
        "{stage}: the Review placeholder must exist"
    );
}

#[test]
fn dxf_reader_keeps_plot_style_handles() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_NAMED_PLOT_STYLES))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    assert_review_everywhere(&doc, "dxf");
}

#[test]
fn dxf_to_dwg_keeps_plot_style_handles() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_NAMED_PLOT_STYLES))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read dwg");
    assert_review_everywhere(&back, "dwg");
}

/// What a DXF consumer sees of `doc` after the DXF writer: a DWG reaches
/// DXF-based tools (CADCraft's bridge among them) as DwgReader + DxfWriter.
fn through_dxf_writer(doc: &CadDocument) -> CadDocument {
    let dxf = DxfWriter::new(doc).write_to_vec().expect("write dxf");
    DxfReader::from_reader(Cursor::new(dxf))
        .expect("open written dxf")
        .read()
        .expect("read written dxf")
}

#[test]
fn dxf_writer_keeps_plot_style_handles() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_NAMED_PLOT_STYLES))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    assert_review_everywhere(&through_dxf_writer(&doc), "dxf -> dxf");
}

#[test]
fn dwg_to_dxf_keeps_plot_style_handles() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_NAMED_PLOT_STYLES))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read dwg");
    assert_review_everywhere(&through_dxf_writer(&back), "dwg -> dxf");
}
