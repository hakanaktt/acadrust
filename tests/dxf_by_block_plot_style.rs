//! A ByBlock plot style (group 380 = 1, `AcDb::PlotStyleNameType`) on an entity
//! must be read from DXF and survive the DWG round trip.
//!
//! The DXF reader has no arm for group 380: an entity's `plotstyle_flags` stay
//! 0 (ByLayer), so a DWG written from a DXF whose entities plot ByBlock
//! (`$PSTYLEMODE 0`) loses that and plots them ByLayer.

use acadrust::entities::EntityType;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader};
use std::io::Cursor;

/// Named plot styles on (`$PSTYLEMODE 0`); the LINE plots ByBlock (380 = 1).
const DXF_BY_BLOCK_PLOT_STYLE: &str = r#"  0
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
1
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
100
AcDbEntity
  8
0
380
1
100
AcDbLine
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
ENDSEC
  0
EOF
"#;

fn assert_by_block(doc: &CadDocument, stage: &str) {
    let line = doc
        .entities()
        .find(|e| matches!(e, EntityType::Line(_)))
        .unwrap_or_else(|| panic!("{stage}: the LINE"));
    assert_eq!(
        line.common().plotstyle_flags,
        0b01,
        "{stage}: the LINE's plot style (380 = 1) must be ByBlock"
    );
    assert_eq!(
        line.common().plotstyle_handle,
        None,
        "{stage}: a ByBlock plot style has no handle"
    );
}

#[test]
fn dxf_reader_keeps_by_block_plot_style() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_BY_BLOCK_PLOT_STYLE))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    assert_by_block(&doc, "dxf");
}

#[test]
fn dxf_to_dwg_keeps_by_block_plot_style() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_BY_BLOCK_PLOT_STYLE))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read dwg");
    assert_by_block(&back, "dwg");
}
