use std::io::Cursor;

use acadrust::entities::polygon_mesh::{PolygonMesh, PolygonMeshVertex};
use acadrust::entities::{EntityType, Polyline3D, Vertex3DPolyline};
use acadrust::tables::Layer;
use acadrust::{CadDocument, Color, DwgReader, DwgWriter, DxfVersion, DxfWriter, Vector3};

const VERSIONS: [DxfVersion; 4] = [
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1032,
];

fn document(version: DxfVersion, mesh: bool) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    doc.layers.add(Layer::new("Walls")).unwrap();
    doc.layers.add(Layer::new("VERTICES")).unwrap();
    let points = vec![
        Vector3::ZERO,
        Vector3::UNIT_X,
        Vector3::UNIT_Y,
        Vector3::new(1., 1., 0.),
    ];
    let entity = if mesh {
        let mut mesh = PolygonMesh::new();
        mesh.m_vertex_count = 2;
        mesh.n_vertex_count = 2;
        mesh.vertices = points.into_iter().map(PolygonMeshVertex::at).collect();
        EntityType::PolygonMesh(mesh)
    } else {
        // Exercise from_points, new/from_xyz, and add_vertex together.
        let mut polyline = Polyline3D::from_points(points[..2].to_vec());
        polyline.add_vertex(points[2]);
        polyline.add_vertex_full(Vertex3DPolyline::from_xyz(1., 1., 0.));
        EntityType::Polyline3D(polyline)
    };
    doc.add_entity(entity).unwrap();
    doc
}

fn set_parent(doc: &mut CadDocument) {
    for e in doc.entities_mut() {
        e.common_mut().layer = "Walls".into();
        e.common_mut().color = Color::Index(1);
    }
}

fn dwg_read(doc: &CadDocument) -> CadDocument {
    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(doc).unwrap()))
        .read()
        .unwrap()
}

fn assert_dwg_vertices(doc: &CadDocument, layer: &str, color: Color) {
    let read = dwg_read(doc);
    for entity in read.entities() {
        match entity {
            EntityType::PolygonMesh(mesh) => {
                assert_eq!(mesh.vertices.len(), 4);
                for v in &mesh.vertices {
                    assert_eq!(v.common.layer, layer, "{:?}", doc.version);
                    assert_eq!(v.common.color, color);
                }
            }
            EntityType::Polyline3D(polyline) => {
                assert_eq!(polyline.vertices.len(), 4);
                for v in &polyline.vertices {
                    assert_eq!(v.layer, layer, "{:?}", doc.version);
                }
            }
            _ => {}
        }
    }
}

// Inspect emitted DXF groups independently: the DXF reader's compound-vertex
// model does not retain all display properties.
fn assert_dxf_vertices(doc: &CadDocument, layer: &str, color_index: i16) {
    let text = String::from_utf8(DxfWriter::new(doc).write_to_vec().unwrap()).unwrap();
    let lines: Vec<_> = text.lines().collect();
    let mut vertex = None;
    let mut count = 0;
    for pair in lines.chunks_exact(2) {
        let code: i16 = pair[0].trim().parse().unwrap();
        let value = pair[1].trim();
        if code == 0 {
            if let Some((actual_layer, actual_color)) = vertex.take() {
                assert_eq!(actual_layer, layer, "{:?}", doc.version);
                assert_eq!(actual_color, color_index);
                count += 1;
            }
            if value == "VERTEX" {
                vertex = Some((String::new(), 256));
            }
        } else if let Some((ref mut actual_layer, ref mut actual_color)) = vertex {
            if code == 8 {
                *actual_layer = value.to_owned();
            }
            if code == 62 {
                *actual_color = value.parse().unwrap();
            }
        }
    }
    assert_eq!(count, 4);
}

#[test]
fn api_mesh_defaults_inherit_parent_in_dwg() {
    for version in VERSIONS {
        let mut doc = document(version, true);
        set_parent(&mut doc);
        assert_dwg_vertices(&doc, "Walls", Color::Index(1));
    }
}

#[test]
fn api_polyline3d_defaults_inherit_parent_in_dwg() {
    for version in VERSIONS {
        let mut doc = document(version, false);
        set_parent(&mut doc);
        assert_dwg_vertices(&doc, "Walls", Color::Index(1));
    }
}

#[test]
fn api_compound_defaults_inherit_parent_in_dxf() {
    for version in VERSIONS {
        for mesh in [true, false] {
            let mut doc = document(version, mesh);
            set_parent(&mut doc);
            assert_dxf_vertices(&doc, "Walls", 1);
        }
    }
}

#[test]
fn imported_layer_zero_and_bylayer_are_not_unset_defaults() {
    for version in VERSIONS {
        for mesh in [true, false] {
            let mut doc = dwg_read(&document(version, mesh));
            set_parent(&mut doc);
            assert_dwg_vertices(&doc, "0", Color::ByLayer);
            // Polyline3D has no per-vertex color field; its existing color
            // inheritance remains outside this regression fix.
            assert_dxf_vertices(&doc, "0", if mesh { 256 } else { 1 });
        }
    }
}

#[test]
fn explicit_api_vertex_display_properties_survive_both_writers() {
    for version in VERSIONS {
        let mut doc = document(version, true);
        set_parent(&mut doc);
        for entity in doc.entities_mut() {
            if let EntityType::PolygonMesh(mesh) = entity {
                for vertex in &mut mesh.vertices {
                    vertex.common.layer = "VERTICES".into();
                    vertex.common.color = Color::Index(6);
                }
            }
        }
        assert_dwg_vertices(&doc, "VERTICES", Color::Index(6));
        assert_dxf_vertices(&doc, "VERTICES", 6);
    }
}
