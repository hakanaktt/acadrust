use acadrust::{
    entities::{EntityType, Line},
    types::Vector3,
    CadDocument, DwgReader, DwgWriter,
};
use std::{io::Cursor, mem::size_of};

#[test]
fn entity_heavy_drawings_keep_non_entity_storage_bounded() {
    let mut source = CadDocument::new();
    for x in 0..5_000 {
        source
            .add_entity(EntityType::Line(Line::from_points(
                Vector3::new(f64::from(x), 0.0, 0.0),
                Vector3::new(f64::from(x), 1.0, 0.0),
            )))
            .unwrap();
    }
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    assert_eq!(loaded.model_space_entities().count(), 5_000);
    assert!(!loaded.objects.is_empty());
    // Empty hash buckets contain a full ObjectType value. A small object table
    // must not retain megabytes solely because the drawing has many lines.
    let object_storage_bytes =
        loaded.objects.capacity() * size_of::<acadrust::objects::ObjectType>();
    assert!(
        object_storage_bytes < 1_048_576,
        "{} objects reserve at least {object_storage_bytes} bytes across {} slots",
        loaded.objects.len(),
        loaded.objects.capacity(),
    );
}

#[test]
fn polyline_vertex_arrays_read_from_dwg_hold_only_their_vertices() {
    use acadrust::entities::{
        polygon_mesh::{PolygonMesh, PolygonMeshVertex},
        PolyfaceMesh, PolyfaceVertex, Polyline2D, Polyline3D, Vertex2D,
    };

    // 17 and 18 are not powers of two, so a vector that grew by doubling, or
    // one that kept a larger buffer, has spare capacity.
    let mut source = CadDocument::new();
    let mut polyline2d = Polyline2D::new();
    let mut polyline3d = Polyline3D::new();
    let mut polyface = PolyfaceMesh::new();
    for i in 0..17 {
        let x = f64::from(i);
        polyline2d.add_vertex(Vertex2D::new(Vector3::new(x, 0.0, 0.0)));
        polyline3d.add_vertex(Vector3::new(x, 0.0, x));
        polyface.add_vertex(PolyfaceVertex::from_xyz(x, x % 2.0, 0.0));
    }
    for i in 1..=15 {
        polyface.add_triangle(i, i + 1, i + 2);
    }
    let mut mesh = PolygonMesh::new();
    mesh.m_vertex_count = 3;
    mesh.n_vertex_count = 6;
    for i in 0..18 {
        mesh.vertices.push(PolygonMeshVertex::at(Vector3::new(
            f64::from(i / 6),
            f64::from(i % 6),
            0.0,
        )));
    }
    for entity in [
        EntityType::Polyline2D(polyline2d),
        EntityType::Polyline3D(polyline3d),
        EntityType::PolyfaceMesh(polyface),
        EntityType::PolygonMesh(mesh),
    ] {
        source.add_entity(entity).unwrap();
    }

    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let mut arrays = Vec::new();
    for entity in loaded.entities() {
        match entity {
            EntityType::Polyline2D(p) => arrays.push((
                "2D polyline vertices",
                p.vertices.len(),
                p.vertices.capacity(),
            )),
            EntityType::Polyline3D(p) => arrays.push((
                "3D polyline vertices",
                p.vertices.len(),
                p.vertices.capacity(),
            )),
            EntityType::PolyfaceMesh(p) => {
                arrays.push(("polyface vertices", p.vertices.len(), p.vertices.capacity()));
                arrays.push(("polyface faces", p.faces.len(), p.faces.capacity()));
            }
            EntityType::PolygonMesh(p) => {
                arrays.push(("mesh vertices", p.vertices.len(), p.vertices.capacity()))
            }
            _ => {}
        }
    }
    assert_eq!(
        arrays.iter().map(|(_, len, _)| *len).collect::<Vec<_>>(),
        [17, 17, 17, 15, 18],
        "{arrays:?}"
    );
    // A drawing can hold hundreds of thousands of vertices, so spare capacity
    // left over from reading multiplies into a large share of its memory.
    for (array, len, capacity) in arrays {
        assert_eq!(capacity, len, "{array}: capacity {capacity} for {len}");
    }
}
