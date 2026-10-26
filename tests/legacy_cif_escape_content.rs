//! In a pre-2007 drawing, `\U+XXXX` is a transport escape for a character the
//! drawing's code page cannot hold. When the page can hold the character, the
//! writer never escapes it, so a `\U+XXXX` the file does contain is the text
//! itself and has to come back unchanged.

use acadrust::entities::{EntityType, Text};
use acadrust::io::dxf::{DxfReader, DxfWriter};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

fn doc_with(text: &str, page: &str, version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    doc.header.code_page = page.to_string();
    doc.header.dim_post = text.to_string();
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

fn dwg_round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn dxf_round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

const HELD: [(&str, &str); 3] = [
    ("ANSI_1252", "94\\U+00B0"),
    ("ANSI_1252", "94\\U+00b0"),
    ("ANSI_1250", "A\\U+0104B"),
];

#[test]
fn a_dwg_escape_the_page_can_hold_is_content() {
    for (page, text) in HELD {
        let back = dwg_round_trip(&doc_with(text, page, DxfVersion::AC1015));
        assert_eq!(first_text(&back), text, "{page}");
    }
}

#[test]
fn a_dwg_escape_the_page_cannot_hold_is_decoded() {
    let back = dwg_round_trip(&doc_with("\\U+2205 x", "ANSI_1252", DxfVersion::AC1015));
    assert_eq!(first_text(&back), "\u{2205} x");
}

#[test]
fn the_dwg_header_reads_the_page_the_entities_do() {
    for (value, want) in [("94\\U+00B0", "94\\U+00B0"), ("\\U+2205 x", "\u{2205} x")] {
        let back = dwg_round_trip(&doc_with(value, "ANSI_1252", DxfVersion::AC1015));
        assert_eq!(back.header.dim_post, want, "header: {value:?}");
        assert_eq!(first_text(&back), want, "entity: {value:?}");
    }
}

#[test]
fn a_dxf_escape_the_page_can_hold_is_content() {
    for (page, text) in HELD {
        let back = dxf_round_trip(&doc_with(text, page, DxfVersion::AC1015));
        assert_eq!(first_text(&back), text, "{page}");
    }
}

#[test]
fn a_dxf_escape_the_page_cannot_hold_is_decoded() {
    let back = dxf_round_trip(&doc_with("\\U+2205 x", "ANSI_1252", DxfVersion::AC1015));
    assert_eq!(first_text(&back), "\u{2205} x");
}

#[test]
fn a_character_the_page_cannot_hold_still_round_trips() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018] {
        let doc = doc_with("\u{2205}45,6", "ANSI_1252", version);
        assert_eq!(first_text(&dwg_round_trip(&doc)), "\u{2205}45,6");
        assert_eq!(first_text(&dxf_round_trip(&doc)), "\u{2205}45,6");
    }
}
