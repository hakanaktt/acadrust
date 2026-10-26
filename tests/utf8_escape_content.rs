//! R2007 and later text is UTF-8 in a DXF and UTF-16 in a DWG. Either can hold
//! every character, so the writer never escapes one and a `\U+XXXX` the file
//! does contain is the text itself. Only the double-byte `\M+` escapes are
//! transport there.

use acadrust::entities::{EntityType, Text};
use acadrust::io::dxf::{DxfReader, DxfWriter};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

const LATE: [DxfVersion; 2] = [DxfVersion::AC1021, DxfVersion::AC1032];

fn doc_with(text: &str, version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    doc.header.code_page = "ANSI_1252".to_string();
    doc.add_entity(EntityType::Text(Text::with_value(
        text,
        Vector3::new(0.0, 0.0, 0.0),
    )))
    .unwrap();
    doc
}

fn first_text(doc: &CadDocument) -> String {
    doc.entities()
        .find_map(|e| match e {
            EntityType::Text(t) => Some(t.value.clone()),
            _ => None,
        })
        .expect("a TEXT entity")
}

fn dxf_round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn dwg_round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

#[test]
fn a_utf8_dxf_keeps_a_cif_escape_as_content() {
    for version in LATE {
        for text in ["94\\U+00B0", "94\\U+00b0", "\\U+2205 x"] {
            let back = dxf_round_trip(&doc_with(text, version));
            assert_eq!(first_text(&back), text, "{version:?} {text:?}");
        }
    }
}

#[test]
fn a_utf16_dwg_keeps_a_cif_escape_as_content() {
    for version in LATE {
        for text in ["94\\U+00B0", "94\\U+00b0", "\\U+2205 x"] {
            let back = dwg_round_trip(&doc_with(text, version));
            assert_eq!(first_text(&back), text, "{version:?} {text:?}");
        }
    }
}

#[test]
fn a_late_drawing_still_decodes_a_mif_escape() {
    for version in LATE {
        let doc = doc_with("\\M+5BCFE", version);
        assert_eq!(first_text(&dxf_round_trip(&doc)), "\u{4ef6}", "dxf {version:?}");
        assert_eq!(first_text(&dwg_round_trip(&doc)), "\u{4ef6}", "dwg {version:?}");
    }
}

#[test]
fn a_real_character_still_round_trips_in_a_late_drawing() {
    for version in LATE {
        let doc = doc_with("\u{2205}45,6 94\u{b0}", version);
        assert_eq!(first_text(&dxf_round_trip(&doc)), "\u{2205}45,6 94\u{b0}");
        assert_eq!(first_text(&dwg_round_trip(&doc)), "\u{2205}45,6 94\u{b0}");
    }
}
