//! The DWG writer turns each entity's layer name into the layer handle it
//! writes. The name decides, not a handle the entity may still carry from the
//! file it was read from; the match ignores case as table lookups do; and a
//! name no layer has, or an empty one, writes layer "0" (issue #80).
//!
//! Each test reads the written file back and checks the handle the entity
//! points at, not only the name: the reader turns a NULL layer pointer into
//! layer "0" by name, but leaves `layer_handle` empty.

use std::collections::HashMap;
use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::tables::Layer;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// R14 writes the layer in the R13–R14 part of the entity header, R2000 and
/// R2018 after the colour.
const VERSIONS: [DxfVersion; 3] = [DxfVersion::AC1014, DxfVersion::AC1015, DxfVersion::AC1032];

fn document(version: DxfVersion, layers: &[&str]) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    for name in layers {
        doc.layers.add(Layer::new(*name)).unwrap();
    }
    doc
}

fn add_line(doc: &mut CadDocument, i: usize, layer: &str) -> Handle {
    let x = i as f64;
    let mut line = Line::from_coords(x, 0.0, 0.0, x, 1.0, 0.0);
    line.common.layer = layer.to_string();
    doc.add_entity(EntityType::Line(line)).unwrap()
}

fn roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("write");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read")
}

/// Every entity in `expected` is on the named layer of `doc`, by name and by
/// the handle it was written with.
fn assert_on_layers(doc: &CadDocument, expected: &HashMap<Handle, &str>, label: &str) {
    for (handle, layer) in expected {
        let entity = doc
            .get_entity(*handle)
            .unwrap_or_else(|| panic!("{label}: entity {handle:?} missing"));
        let table = doc
            .layers
            .get(layer)
            .unwrap_or_else(|| panic!("{label}: layer {layer:?} missing"));
        assert_eq!(
            (entity.common().layer.as_str(), entity.common().layer_handle),
            (table.name.as_str(), Some(table.handle)),
            "{label}: entity {handle:?}"
        );
    }
}

#[test]
fn entities_on_several_layers_point_at_their_layers() {
    let layers = ["0", "Walls", "Doors", "A-ANNO-TEXT"];
    for version in VERSIONS {
        let mut doc = document(version, &layers[1..]);
        let mut expected = HashMap::new();
        // Long enough for the parallel entity writer to take over.
        for i in 0..3_000 {
            let layer = layers[(i * 7 / 3) % layers.len()];
            expected.insert(add_line(&mut doc, i, layer), layer);
        }
        assert_on_layers(&roundtrip(&doc), &expected, &format!("{version:?}"));
    }
}

#[test]
fn layer_name_wins_over_a_stale_layer_handle() {
    for version in VERSIONS {
        let mut doc = document(version, &["A", "B"]);
        let moved = add_line(&mut doc, 0, "A");
        let kept = add_line(&mut doc, 1, "A");
        let mut read = roundtrip(&doc);
        let a = read.layers.get("A").unwrap().handle;
        assert_eq!(
            read.get_entity(moved).unwrap().common().layer_handle,
            Some(a)
        );

        // Move one entity by name only: it still carries A's handle.
        read.get_entity_mut(moved).unwrap().common_mut().layer = "B".to_string();
        assert_eq!(
            read.get_entity(moved).unwrap().common().layer_handle,
            Some(a)
        );

        let expected = HashMap::from([(moved, "B"), (kept, "A")]);
        assert_on_layers(&roundtrip(&read), &expected, &format!("{version:?}"));
    }
}

#[test]
fn a_document_changed_between_writes_writes_its_new_layers() {
    for version in VERSIONS {
        let mut doc = document(version, &["Walls", "Doors"]);
        let walls = add_line(&mut doc, 0, "Walls");
        let doors = add_line(&mut doc, 1, "Doors");
        let expected = HashMap::from([(walls, "Walls"), (doors, "Doors")]);
        assert_on_layers(&roundtrip(&doc), &expected, &format!("{version:?} first"));

        doc.rename_layer("Walls", "Facade").unwrap();
        doc.get_entity_mut(doors).unwrap().common_mut().layer = "Facade".to_string();
        let expected = HashMap::from([(walls, "Facade"), (doors, "Facade")]);
        let read = roundtrip(&doc);
        assert_on_layers(&read, &expected, &format!("{version:?} second"));
        assert!(!read.layers.contains("Walls"));
    }
}

#[test]
fn unknown_and_empty_layer_names_write_layer_zero() {
    for version in VERSIONS {
        let mut doc = document(version, &["Walls"]);
        let mut expected = HashMap::new();
        for (i, (layer, on)) in [
            ("Walls", "Walls"),
            ("GHOST", "0"),
            ("", "0"),
            ("Walls", "Walls"),
            ("GHOST", "0"),
            ("", "0"),
        ]
        .into_iter()
        .enumerate()
        {
            expected.insert(add_line(&mut doc, i, layer), on);
        }
        assert_on_layers(&roundtrip(&doc), &expected, &format!("{version:?}"));
    }
}

#[test]
fn layer_names_match_regardless_of_case() {
    for version in VERSIONS {
        let mut doc = document(version, &["Walls", "Équipe", "Straße"]);
        let mut expected = HashMap::new();
        for (i, (layer, on)) in [
            ("Walls", "Walls"),
            ("WALLS", "Walls"),
            ("walls", "Walls"),
            ("wALLS", "Walls"),
            ("équipe", "Équipe"),
            ("ÉQUIPE", "Équipe"),
            ("STRAßE", "Straße"),
            // "ß" has no one-letter upper case, so it is not matched by "SS".
            ("STRASSE", "0"),
        ]
        .into_iter()
        .enumerate()
        {
            expected.insert(add_line(&mut doc, i, layer), on);
        }
        assert_on_layers(&roundtrip(&doc), &expected, &format!("{version:?}"));
    }
}
