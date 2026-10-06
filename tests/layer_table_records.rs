use acadrust::{tables::Layer, types::Color, CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

#[test]
fn dwg_preserves_duplicate_empty_layer_names_and_properties() {
    let mut doc = CadDocument::new();
    let mut expected = Vec::new();
    for (name, color) in [("", 1), ("", 2), ("Visible", 3)] {
        let mut layer = Layer::new(name);
        layer.handle = doc.allocate_handle();
        layer.color = Color::Index(color);
        expected.push((layer.handle, name, color));
        doc.layers.add_allow_duplicate(layer);
    }
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    assert_eq!(read.layers.len(), 4);
    for (handle, name, color) in expected {
        let layer = read.layers.iter().find(|l| l.handle == handle).unwrap();
        assert_eq!(layer.name, name);
        assert_eq!(layer.color, Color::Index(color));
    }
}
