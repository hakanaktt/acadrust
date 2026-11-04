//! Table entries and objects keep their `{ACAD_REACTORS` group through a DXF
//! read and write.
//!
//! SCALE, MLINESTYLE and ACDBDICTIONARYWDFLT have no reactor list in the
//! model, and the reader kept reactors only for BLOCK_RECORD, so AutoCAD's
//! reactors on these objects and on DIMSTYLE (and other table entries) were
//! lost.

use acadrust::{DxfReader, DxfWriter};
use std::io::Cursor;

/// The fixture written by AutoCAD (see `tests/datatable/README.md`).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/datatable/point_object_id.dxf"
);

type Pairs = Vec<(i32, String)>;

fn pairs(text: &str) -> Pairs {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap_or(-1), c[1].trim().to_string()))
        .collect()
}

/// The reactors of the record `kind` whose handle (5 or 105) is `handle`.
fn reactors(text: &str, kind: &str, handle: &str) -> Vec<String> {
    let pairs = pairs(text);
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, kind.into()) && matches!(w[1].0, 5 | 105) && w[1].1 == handle)
        .unwrap_or_else(|| panic!("{kind} {handle}"));
    pairs[start + 1..]
        .iter()
        .take_while(|(code, _)| *code != 0)
        .skip_while(|p| **p != (102, "{ACAD_REACTORS".into()))
        .skip(1)
        .take_while(|p| **p != (102, "}".into()))
        .map(|(_, v)| v.clone())
        .collect()
}

fn round_trip(text: &str) -> String {
    let doc = DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    String::from_utf8(
        DxfWriter::new(&doc)
            .with_version(doc.version)
            .write_to_vec()
            .expect("write dxf"),
    )
    .unwrap()
}

#[test]
fn autocad_object_reactors_survive_a_dxf_round_trip() {
    let source = std::fs::read_to_string(FIXTURE).expect("fixture");
    let written = round_trip(&source);
    for (kind, handle) in [
        ("SCALE", "43"),
        ("MLINESTYLE", "60"),
        ("ACDBDICTIONARYWDFLT", "E"),
    ] {
        let before = reactors(&source, kind, handle);
        assert!(!before.is_empty(), "fixture {kind} {handle} has reactors");
        assert_eq!(reactors(&written, kind, handle), before, "{kind} {handle}");
    }
}

#[test]
fn dimstyle_reactors_survive_a_dxf_round_trip() {
    // AutoCAD writes a DIMSTYLE's reactors (its dimensions) after 105, as on
    // other table entries; give the fixture's DIMSTYLE 58 one, on the scale
    // list dictionary 42 (any handle that exists in the file).
    // Normalise line endings first: a Windows checkout has CRLF.
    let source = std::fs::read_to_string(FIXTURE)
        .expect("fixture")
        .replace("\r\n", "\n")
        .replacen(
            "DIMSTYLE\n105\n58\n",
            "DIMSTYLE\n105\n58\n102\n{ACAD_REACTORS\n330\n42\n102\n}\n",
            1,
        );
    assert_eq!(reactors(&source, "DIMSTYLE", "58"), ["42"], "fixture edit");
    let written = round_trip(&source);
    assert_eq!(reactors(&written, "DIMSTYLE", "58"), ["42"]);
}

#[test]
fn reactors_survive_on_a_table_entry_with_a_default_object_handle() {
    // AutoCAD's DIMSTYLE Standard is often 27, a handle the reader's default
    // objects also use until the OBJECTS section replaces them; the entry is
    // still a table entry and keeps its dimension (here a line) as reactor.
    let pairs: &[(i32, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1032"),
        (9, "$HANDSEED"),
        (5, "200"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "DIMSTYLE"),
        (5, "A"),
        (330, "0"),
        (100, "AcDbSymbolTable"),
        (70, "1"),
        (100, "AcDbDimStyleTable"),
        (71, "0"),
        (0, "DIMSTYLE"),
        (105, "27"),
        (102, "{ACAD_REACTORS"),
        (330, "100"),
        (102, "}"),
        (330, "A"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbDimStyleTableRecord"),
        (2, "Standard"),
        (70, "0"),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "LINE"),
        (5, "100"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbLine"),
        (10, "0"),
        (20, "0"),
        (30, "0"),
        (11, "1"),
        (21, "0"),
        (31, "0"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "OBJECTS"),
        (0, "DICTIONARY"),
        (5, "C"),
        (330, "0"),
        (100, "AcDbDictionary"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    let source: String = pairs.iter().map(|(c, v)| format!("{c}\n{v}\n")).collect();
    assert_eq!(reactors(&source, "DIMSTYLE", "27"), ["100"], "source");
    let written = round_trip(&source);
    assert_eq!(reactors(&written, "DIMSTYLE", "27"), ["100"]);
}
