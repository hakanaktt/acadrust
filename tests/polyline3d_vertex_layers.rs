use acadrust::entities::{EntityType, Polyline3D, Vertex3DPolyline};
use acadrust::tables::Layer;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion};
use std::io::Cursor;

fn assert_vertex_layer(source_layer: &str, expected_layer: &str) {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1024] {
        let mut doc = CadDocument::with_version(version);
        doc.layers.add(Layer::new("PARENT")).unwrap();
        doc.layers.add(Layer::new("VERTICES")).unwrap();
        let mut polyline = Polyline3D::new();
        polyline.common.layer = "PARENT".into();
        for i in 0..3 {
            let mut vertex = Vertex3DPolyline::from_xyz(i as f64, i as f64 * 2.0, 3.0);
            vertex.layer = source_layer.into();
            polyline.add_vertex_full(vertex);
        }
        doc.add_entity(EntityType::Polyline3D(polyline)).unwrap();
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let result = read
            .entities()
            .find_map(|e| match e {
                EntityType::Polyline3D(p) => Some(p),
                _ => None,
            })
            .unwrap();
        assert_eq!(result.vertices.len(), 3);
        for vertex in &result.vertices {
            assert_eq!(vertex.layer, expected_layer, "{version:?}");
        }
    }
}

#[test]
fn polyline3d_keeps_decoded_vertex_layer_names() {
    assert_vertex_layer("PARENT", "PARENT");
}

#[test]
fn polyline3d_writes_distinct_vertex_layers_and_empty_layer_fallback() {
    assert_vertex_layer("VERTICES", "VERTICES");
    assert_vertex_layer("", "PARENT");
}
