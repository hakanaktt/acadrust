use std::io::Cursor;

use acadrust::entities::polygon_mesh::{PolygonMesh, PolygonMeshVertex};
use acadrust::entities::EntityType;
use acadrust::tables::Layer;
use acadrust::types::{Color, LineWeight};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Handle, Vector3};

fn roundtrip(version: DxfVersion) -> PolygonMesh {
    let mut doc = CadDocument::with_version(version);
    doc.layers.add(Layer::new("VERTICES")).unwrap();
    let mut mesh = PolygonMesh::new();
    mesh.m_vertex_count = 2;
    mesh.n_vertex_count = 2;
    for i in 0..4 {
        let mut vertex =
            PolygonMeshVertex::at(Vector3::new((i / 2) as f64, (i % 2) as f64, i as f64));
        vertex.common.handle = Handle::new(0x1000 + i);
        vertex.common.layer = "VERTICES".into();
        vertex.common.color = Color::Index(2 + i as u8);
        vertex.common.line_weight = LineWeight::Value(50);
        vertex.common.linetype = "Continuous".into();
        vertex.common.linetype_scale = 2.5;
        vertex.flags = 64;
        mesh.vertices.push(vertex);
    }
    doc.header.handle_seed = 0x2000;
    doc.add_entity(EntityType::PolygonMesh(mesh)).unwrap();
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let mesh = read.entities()
        .find_map(|e| match e {
            EntityType::PolygonMesh(mesh) => Some(mesh.clone()),
            _ => None,
        })
        .expect("mesh");
    mesh
}

#[test]
fn polygon_mesh_vertex_ownership_and_flags_survive_dwg_read() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1024] {
        let mesh = roundtrip(version);
        assert_eq!(mesh.vertices.len(), 4);
        for (i, vertex) in mesh.vertices.iter().enumerate() {
            assert_eq!(vertex.common.handle, Handle::new(0x1000 + i as u64));
            assert_eq!(
                vertex.location,
                Vector3::new((i / 2) as f64, (i % 2) as f64, i as f64)
            );
            assert_eq!(
                (vertex.common.owner_handle, vertex.flags),
                (mesh.common.handle, 64),
                "{version:?}"
            );
        }
    }
}

#[test]
fn polygon_mesh_vertices_keep_their_own_display_properties() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1024] {
        let mesh = roundtrip(version);
        for (i, vertex) in mesh.vertices.iter().enumerate() {
            assert_eq!(vertex.common.layer, "VERTICES", "{version:?}");
            assert_eq!(vertex.common.color, Color::Index(2 + i as u8));
            assert_eq!(vertex.common.line_weight, LineWeight::Value(50));
            assert!(vertex.common.linetype.eq_ignore_ascii_case("Continuous"));
            assert_eq!(vertex.common.linetype_scale, 2.5);
        }
    }
}
