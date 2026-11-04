//! A 3D polyline read from DWG keeps the layer of its VERTEX records, so the
//! DXF written from it names a layer on every vertex. AutoCAD refuses a DXF
//! with an empty layer (group 8) on a VERTEX: "Bad entity layer".

use std::io::Cursor;

use acadrust::entities::{EntityType, Polyline3D};
use acadrust::tables::Layer;
use acadrust::types::Vector3;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfWriter};

#[test]
fn dwg_polyline3d_vertices_keep_their_layer_in_dxf() {
    let mut doc = CadDocument::new();
    doc.layers.add(Layer::new("P3D")).unwrap();
    let mut poly = Polyline3D::from_points(vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 5.0),
        Vector3::new(20.0, 10.0, 10.0),
    ]);
    poly.common.layer = "P3D".to_string();
    doc.add_entity(EntityType::Polyline3D(poly)).unwrap();

    let bytes = DwgWriter::write_to_vec(&doc).expect("dwg write");
    let read = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("dwg read");
    let poly = read
        .entities()
        .find_map(|e| match e {
            EntityType::Polyline3D(p) => Some(p),
            _ => None,
        })
        .expect("3D polyline read back");
    assert_eq!(poly.vertices.len(), 3);
    for v in &poly.vertices {
        assert_eq!(v.layer, "P3D", "vertex layer after DWG read");
    }

    let dxf = DxfWriter::new(&read).write_to_vec().expect("dxf write");
    let text = String::from_utf8_lossy(&dxf);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut vertices = 0;
    let mut i = 0;
    while i + 1 < lines.len() {
        if lines[i] == "0" && lines[i + 1] == "VERTEX" {
            vertices += 1;
            let layer = lines[i + 2..]
                .chunks(2)
                .take_while(|p| p[0] != "0")
                .find(|p| p[0] == "8")
                .map(|p| p[1]);
            assert_eq!(layer, Some("P3D"), "VERTEX layer in DXF");
        }
        i += 2;
    }
    assert_eq!(vertices, 3);
}
