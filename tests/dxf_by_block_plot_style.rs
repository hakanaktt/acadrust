//! Entity plot styles must survive DXF and DWG round trips in named mode.

use acadrust::entities::EntityType;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
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

fn assert_plot_style(doc: &CadDocument, stage: &str, flag: u8) {
    let line = doc
        .entities()
        .find(|e| matches!(e, EntityType::Line(_)))
        .unwrap_or_else(|| panic!("{stage}: the LINE"));
    assert_eq!(
        line.common().plotstyle_flags,
        flag,
        "{stage}: the LINE's plot style must match group 380"
    );
    assert_eq!(
        line.common().plotstyle_handle,
        None,
        "{stage}: ByBlock and explicit Normal have no handle"
    );
}

#[test]
fn dxf_reader_keeps_by_block_plot_style() {
    let doc = DxfReader::from_reader(Cursor::new(DXF_BY_BLOCK_PLOT_STYLE))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    assert_plot_style(&doc, "dxf", 1);
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
    assert_plot_style(&back, "dwg", 1);
}

#[test]
fn dxf_round_trip_keeps_by_block_and_explicit_normal() {
    for flag in [1, 2] {
        let source = DXF_BY_BLOCK_PLOT_STYLE.replace("380\n1\n", &format!("380\n{flag}\n"));
        let doc = DxfReader::from_reader(Cursor::new(source))
            .expect("open DXF")
            .read()
            .expect("read DXF");
        assert_plot_style(&doc, "input DXF", flag);

        let output = DxfWriter::new(&doc).write_to_vec().expect("write DXF");
        let ascii = std::str::from_utf8(&output).expect("read ASCII DXF");
        let lines: Vec<_> = ascii.lines().map(str::trim).collect();
        let expected = flag.to_string();
        assert!(lines
            .chunks_exact(2)
            .any(|pair| pair == ["380", expected.as_str()]));
        let back = DxfReader::from_reader(Cursor::new(output))
            .expect("open output DXF")
            .read()
            .expect("read output DXF");
        assert_plot_style(&back, "output DXF", flag);
    }
}
