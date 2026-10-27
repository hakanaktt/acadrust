//! Symbol names compare as AutoCAD compares them.
//!
//! AutoCAD compares symbol table names without regard to case, as Windows compares names: each
//! letter is upper-cased to a single letter, and a letter outside ASCII never becomes an ASCII
//! one. So `ölçü` and `ÖLÇÜ` are one layer, but the Turkish dotless `ı` stays apart from `I` (a
//! drawing can hold the layers `YAZI` and `Yazı`, or the text styles `ARIAL` and `ARıAL`, side by
//! side) and `ß` stays apart from `SS` (`STRASSE` and `Straße` are two layers).

use std::io::Cursor;

use acadrust::entities::{EntityType, Line, Text};
use acadrust::tables::{BlockRecord, Layer, TextStyle};
use acadrust::types::{Color, DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn dxf_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc)
        .write_to_vec()
        .expect("DXF write failed");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

fn dwg_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("DWG write failed");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed")
}

/// Layers `YAZI` (red) and `Yazı` (green), with a line on each.
fn two_layers() -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for (name, color, y) in [("YAZI", 1, 0.0), ("Yazı", 3, 10.0)] {
        let mut layer = Layer::new(name);
        layer.color = Color::Index(color);
        doc.layers.add_allow_duplicate(layer);
        let mut line = Line::from_points(Vector3::new(0.0, y, 0.0), Vector3::new(100.0, y, 0.0));
        line.common.layer = name.to_string();
        doc.add_entity(EntityType::Line(line)).unwrap();
    }
    doc
}

fn assert_two_layers(read: &CadDocument) {
    let names: Vec<&str> = read.layers.iter().map(|l| l.name.as_str()).collect();
    for (name, color) in [("YAZI", 1), ("Yazı", 3)] {
        let layer = read
            .layers
            .iter()
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("layer {name} missing from {names:?}"));
        assert_eq!(layer.color, Color::Index(color), "colour of {name}");
    }
    let mut on: Vec<&str> = read
        .model_space_entities()
        .map(|e| e.common().layer.as_str())
        .collect();
    on.sort_unstable();
    assert_eq!(on, ["YAZI", "Yazı"], "each line stays on its own layer");
}

#[test]
fn a_table_takes_names_that_differ_only_by_dotless_i() {
    let mut doc = CadDocument::new();
    doc.layers.add(Layer::new("YAZI")).unwrap();
    doc.layers
        .add(Layer::new("Yazı"))
        .expect("Yazı is not YAZI");
    assert_eq!(doc.layers.get("YAZI").unwrap().name, "YAZI");
    assert_eq!(doc.layers.get("yazı").unwrap().name, "Yazı");

    doc.text_styles.add(TextStyle::new("ARIAL")).unwrap();
    doc.text_styles
        .add(TextStyle::new("ARıAL"))
        .expect("ARıAL is not ARIAL");
    assert_eq!(doc.text_styles.get("arial").unwrap().name, "ARIAL");
    assert_eq!(doc.text_styles.get("ARıAL").unwrap().name, "ARıAL");
}

#[test]
fn other_letters_still_fold_as_autocad_folds_them() {
    let mut doc = CadDocument::new();
    doc.layers.add(Layer::new("ÖLÇÜ")).unwrap();
    assert!(doc.layers.add(Layer::new("ölçü")).is_err(), "ölçü is ÖLÇÜ");
    assert_eq!(doc.layers.get("ölçü").unwrap().name, "ÖLÇÜ");
    doc.layers.add(Layer::new("Duvar")).unwrap();
    assert!(
        doc.layers.add(Layer::new("DUVAR")).is_err(),
        "DUVAR is Duvar"
    );
    assert_eq!(doc.layers.get("duvar").unwrap().name, "Duvar");
}

#[test]
fn dxf_keeps_layers_that_differ_only_by_dotless_i() {
    assert_two_layers(&dxf_roundtrip(&two_layers()));
}

#[test]
fn dwg_keeps_layers_that_differ_only_by_dotless_i() {
    assert_two_layers(&dwg_roundtrip(&two_layers()));
}

#[test]
fn dwg_keeps_text_styles_that_differ_only_by_dotless_i() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut wide = TextStyle::new("ARIAL");
    wide.width_factor = 1.0;
    doc.text_styles.add_allow_duplicate(wide);
    let mut narrow = TextStyle::new("ARıAL");
    narrow.width_factor = 0.8;
    doc.text_styles.add_allow_duplicate(narrow);
    for (style, y) in [("ARIAL", 0.0), ("ARıAL", 10.0)] {
        let mut text = Text::new();
        text.value = style.to_string();
        text.insertion_point = Vector3::new(0.0, y, 0.0);
        text.style = style.to_string();
        doc.add_entity(EntityType::Text(text)).unwrap();
    }

    let read = dwg_roundtrip(&doc);
    for (name, width) in [("ARIAL", 1.0), ("ARıAL", 0.8)] {
        let style = read
            .text_styles
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("text style {name} missing"));
        assert!((style.width_factor - width).abs() < 1e-9, "width of {name}");
    }
    for entity in read.model_space_entities() {
        if let EntityType::Text(text) = entity {
            assert_eq!(text.style, text.value, "a text keeps its own style");
        }
    }
}

#[test]
fn dwg_keeps_block_names_that_differ_only_by_dotless_i() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for name in ["YAZI", "Yazı"] {
        let mut record = BlockRecord::new(name);
        record.handle = doc.allocate_handle();
        record.block_entity_handle = doc.allocate_handle();
        record.block_end_handle = doc.allocate_handle();
        let line = Line::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        let handle = doc.add_entity(EntityType::Line(line)).unwrap();
        record.entity_handles.push(handle);
        doc.block_records.add_allow_duplicate(record);
    }

    let read = dwg_roundtrip(&doc);
    let names: Vec<&str> = read.block_records.iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"YAZI"), "YAZI missing from {names:?}");
    assert!(names.contains(&"Yazı"), "Yazı missing from {names:?}");
}

#[test]
fn a_table_takes_names_that_differ_only_by_sharp_s() {
    let mut doc = CadDocument::new();
    doc.layers.add(Layer::new("STRASSE")).unwrap();
    doc.layers
        .add(Layer::new("Straße"))
        .expect("Straße is not STRASSE");
    assert_eq!(doc.layers.get("strasse").unwrap().name, "STRASSE");
    assert_eq!(doc.layers.get("STRAßE").unwrap().name, "Straße");
}

#[test]
fn dxf_keeps_layers_that_differ_only_by_sharp_s() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for name in ["STRASSE", "Straße"] {
        doc.layers.add_allow_duplicate(Layer::new(name));
    }
    let read = dxf_roundtrip(&doc);
    let names: Vec<&str> = read.layers.iter().map(|l| l.name.as_str()).collect();
    assert!(names.contains(&"STRASSE"), "STRASSE missing from {names:?}");
    assert!(names.contains(&"Straße"), "Straße missing from {names:?}");
}
