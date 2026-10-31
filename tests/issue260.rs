//! Repro for issue #260: in a pre-2007 DXF, a `\U+XXXX` escape for a character
//! the drawing's code page can hold is kept as text, though AutoCAD decodes it.
//! Some writers escape every non-ASCII character to keep the file pure ASCII;
//! AutoCAD 2023 opens `escaped_ac1015.dxf` with the layer `Électricité` current
//! and the text `Cote 94°`.

use acadrust::entities::{EntityType, Text};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/issue260")
        .join(name)
}

fn read_fixture(name: &str) -> CadDocument {
    DxfReader::from_file(fixture(name)).unwrap().read().unwrap()
}

fn first_text(doc: &CadDocument) -> &Text {
    doc.entities()
        .find_map(|e| match e {
            EntityType::Text(t) => Some(t),
            _ => None,
        })
        .expect("a TEXT entity")
}

fn dwg_round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

/// The values AutoCAD shows for the escaped drawing, as read here.
fn as_json(doc: &CadDocument) -> serde_json::Value {
    let text = first_text(doc);
    serde_json::json!({
        "current_layer": doc.header.current_layer_name,
        "layers": doc.layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
        "text": text.value,
        "text_layer": text.common.layer,
    })
}

fn expected() -> serde_json::Value {
    let json = std::fs::read_to_string(fixture("expected.json")).unwrap();
    serde_json::from_str(&json).unwrap()
}

#[test]
fn issue_260_ac1015_current_layer_resolves_to_the_decoded_layer() {
    let doc = read_fixture("escaped_ac1015.dxf");
    assert_eq!(doc.version, DxfVersion::AC1015);
    let layer = doc.layers.get(&doc.header.current_layer_name);
    assert_eq!(layer.map(|l| l.name.as_str()), Some("Électricité"));
}

#[test]
fn issue_260_ac1018_current_layer_resolves_to_the_decoded_layer() {
    let doc = read_fixture("escaped_ac1018.dxf");
    assert_eq!(doc.version, DxfVersion::AC1018);
    let layer = doc.layers.get(&doc.header.current_layer_name);
    assert_eq!(layer.map(|l| l.name.as_str()), Some("Électricité"));
}

#[test]
fn issue_260_text_reads_decoded() {
    for name in ["escaped_ac1015.dxf", "escaped_ac1018.dxf"] {
        assert_eq!(first_text(&read_fixture(name)).value, "Cote 94°", "{name}");
    }
}

#[test]
fn issue_260_layer_lookup_ignores_case_of_decoded_letters() {
    for name in ["escaped_ac1015.dxf", "escaped_ac1018.dxf"] {
        let doc = read_fixture(name);
        for key in ["Survey_ä", "Survey_Ä"] {
            assert_eq!(
                doc.layers.get(key).map(|l| l.name.as_str()),
                Some("Survey_Ä"),
                "{name}: {key}"
            );
        }
        // The TEXT names its layer in the other case; it finds it too.
        let on = &first_text(&doc).common.layer;
        assert!(doc.layers.get(on).is_some(), "{name}: {on:?}");
    }
}

#[test]
fn issue_260_reads_as_autocad_shows_it() {
    for name in ["escaped_ac1015.dxf", "escaped_ac1018.dxf"] {
        assert_eq!(as_json(&read_fixture(name)), expected(), "{name}");
    }
}

#[test]
fn issue_260_dwg_from_the_escaped_dxf_holds_the_characters() {
    for (name, version) in [
        ("escaped_ac1015.dxf", DxfVersion::AC1015),
        ("escaped_ac1018.dxf", DxfVersion::AC1018),
    ] {
        let back = dwg_round_trip(&read_fixture(name));
        assert_eq!(back.version, version, "{name}");
        let text = &first_text(&back).value;
        assert_eq!(text, "Cote 94°", "{name}");
        assert!(!text.contains("\\U+"), "{name}: {text:?}");
        assert_eq!(back.header.current_layer_name, "Électricité", "{name}");
    }
}

/// Controls, green before and after the fix.
fn dxf_text_round_trip(value: &str) -> String {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.header.code_page = "ANSI_1252".to_string();
    doc.add_entity(EntityType::Text(Text::with_value(
        value,
        Vector3::new(0.0, 0.0, 0.0),
    )))
    .unwrap();
    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    first_text(&back).value.clone()
}

#[test]
fn issue_260_control_a_character_the_page_cannot_hold_still_round_trips() {
    assert_eq!(dxf_text_round_trip("\u{2205}45,6"), "\u{2205}45,6");
}

#[test]
fn issue_260_control_an_escape_the_page_cannot_hold_is_decoded() {
    let dxf = std::fs::read_to_string(fixture("escaped_ac1015.dxf"))
        .unwrap()
        .replace("Cote 94\\U+00B0", "\\U+2205 x");
    let doc = DxfReader::from_reader(Cursor::new(dxf.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(first_text(&doc).value, "\u{2205} x");
}

#[test]
fn issue_260_control_a_malformed_escape_is_left_alone() {
    let dxf = std::fs::read_to_string(fixture("escaped_ac1015.dxf"))
        .unwrap()
        .replace("Cote 94\\U+00B0", "Cote 94\\U+00B");
    let doc = DxfReader::from_reader(Cursor::new(dxf.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(first_text(&doc).value, "Cote 94\\U+00B");
}
