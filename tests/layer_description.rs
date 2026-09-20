//! A layer's description survives a round-trip.
//!
//! The format keeps it in the layer's extended data under the
//! `AcAecLayerStandard` application, as two strings of which the second is the
//! text -- the same place, and on DWG the same mechanism, the layer
//! transparency comes from. Until now it was parsed and dropped on both
//! readers, so a description was invisible to every caller.
//!
//! The fixtures are written by the crate itself: no sample drawing is needed
//! and none is referenced.

use acadrust::tables::Layer;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

const DESCRIPTION: &str = "Roadways: profile geometry points";

fn document() -> CadDocument {
    let mut doc = CadDocument::new();
    let mut described = Layer::new("C-ROAD");
    described.description = DESCRIPTION.to_string();
    doc.layers.add(described).unwrap();
    doc.layers.add(Layer::new("C-PLAIN")).unwrap();
    doc
}

fn assert_carried(loaded: &CadDocument, format: &str) {
    let described = loaded.layers.get("C-ROAD").expect("the described layer");
    assert_eq!(
        described.description, DESCRIPTION,
        "{format}: the description did not survive"
    );
    let plain = loaded.layers.get("C-PLAIN").expect("the plain layer");
    assert!(
        plain.description.is_empty(),
        "{format}: a layer that states no description came back with {:?} -- the \
         empty placeholder string that precedes the text must not be mistaken for it",
        plain.description
    );
}

#[test]
fn a_layer_description_survives_a_dxf_roundtrip() {
    let path = std::env::temp_dir().join("acadrust_layer_description.dxf");
    DxfWriter::new(&document())
        .write_to_file(&path)
        .expect("write dxf");
    let loaded = DxfReader::from_file(&path)
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let _ = std::fs::remove_file(&path);
    assert_carried(&loaded, "dxf");
}

#[test]
fn a_layer_description_survives_a_dwg_roundtrip() {
    let path = std::env::temp_dir().join("acadrust_layer_description.dwg");
    DwgWriter::write_to_file(&path, &document()).expect("write dwg");
    let loaded = DwgReader::from_file(&path)
        .expect("open dwg")
        .read()
        .expect("read dwg");
    let _ = std::fs::remove_file(&path);
    assert_carried(&loaded, "dwg");
}
