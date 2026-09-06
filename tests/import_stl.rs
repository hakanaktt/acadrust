//! Integration tests for STL import.

#![cfg(feature = "import")]

use acadrust::entities::EntityType;
use acadrust::io::import::stl::StlImporter;
use acadrust::io::import::ImportConfig;
use acadrust::types::Vector3;

// ─── ASCII STL tests ─────────────────────────────────────────────────────

const ASCII_CUBE_STL: &str = r#"solid cube
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 0 1 0
      vertex 1 1 0
    endloop
  endfacet
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 1 1 0
      vertex 1 0 0
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex 0 0 1
      vertex 1 1 1
      vertex 0 1 1
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex 0 0 1
      vertex 1 0 1
      vertex 1 1 1
    endloop
  endfacet
  facet normal 0 -1 0
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 1 0 1
    endloop
  endfacet
  facet normal 0 -1 0
    outer loop
      vertex 0 0 0
      vertex 1 0 1
      vertex 0 0 1
    endloop
  endfacet
  facet normal 1 0 0
    outer loop
      vertex 1 0 0
      vertex 1 1 0
      vertex 1 1 1
    endloop
  endfacet
  facet normal 1 0 0
    outer loop
      vertex 1 0 0
      vertex 1 1 1
      vertex 1 0 1
    endloop
  endfacet
  facet normal 0 1 0
    outer loop
      vertex 0 1 0
      vertex 0 1 1
      vertex 1 1 1
    endloop
  endfacet
  facet normal 0 1 0
    outer loop
      vertex 0 1 0
      vertex 1 1 1
      vertex 1 1 0
    endloop
  endfacet
  facet normal -1 0 0
    outer loop
      vertex 0 0 0
      vertex 0 0 1
      vertex 0 1 1
    endloop
  endfacet
  facet normal -1 0 0
    outer loop
      vertex 0 0 0
      vertex 0 1 1
      vertex 0 1 0
    endloop
  endfacet
endsolid cube
"#;

#[test]
fn test_import_ascii_stl_cube() {
    let importer = StlImporter::from_bytes(ASCII_CUBE_STL.as_bytes().to_vec());
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    assert_eq!(entities.len(), 1, "Should produce exactly one Mesh entity");

    if let EntityType::Mesh(mesh) = &entities[0] {
        assert_eq!(mesh.faces.len(), 12, "A cube has 12 triangles");
        // With vertex merging, a cube has only 8 unique vertices
        assert_eq!(mesh.vertices.len(), 8, "A cube has 8 unique vertices");
    } else {
        panic!("Expected Mesh entity, got {:?}", entities[0]);
    }
}

#[test]
fn test_import_ascii_stl_no_merge() {
    let mut config = ImportConfig::default();
    config.merge_vertices = false;

    let importer = StlImporter::from_bytes(ASCII_CUBE_STL.as_bytes().to_vec()).with_config(config);
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    if let EntityType::Mesh(mesh) = &entities[0] {
        assert_eq!(mesh.faces.len(), 12);
        // Without merging: 12 triangles × 3 vertices = 36
        assert_eq!(mesh.vertices.len(), 36);
    } else {
        panic!("Expected Mesh entity");
    }
}

#[test]
fn test_import_stl_with_scale() {
    let stl = b"solid s\nfacet normal 0 0 1\nouter loop\nvertex 1 2 3\nvertex 4 5 6\nvertex 7 8 9\nendloop\nendfacet\nendsolid s\n";

    let mut config = ImportConfig::default();
    config.scale_factor = 2.5;
    config.merge_vertices = false;

    let importer = StlImporter::from_bytes(stl.to_vec()).with_config(config);
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    if let EntityType::Mesh(mesh) = &entities[0] {
        assert_eq!(mesh.vertices[0], Vector3::new(2.5, 5.0, 7.5));
        assert_eq!(mesh.vertices[1], Vector3::new(10.0, 12.5, 15.0));
    } else {
        panic!("Expected Mesh entity");
    }
}

// ─── Binary STL tests ───────────────────────────────────────────────────

fn make_binary_stl_data(triangles: &[([f32; 3], [[f32; 3]; 3])]) -> Vec<u8> {
    let mut buf = Vec::new();
    // 80-byte header
    let header = b"binary stl test";
    buf.extend_from_slice(header);
    buf.extend_from_slice(&vec![0u8; 80 - header.len()]);
    // Triangle count
    buf.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
    // Triangles
    for (normal, verts) in triangles {
        for &n in normal {
            buf.extend_from_slice(&n.to_le_bytes());
        }
        for v in verts {
            for &c in v {
                buf.extend_from_slice(&c.to_le_bytes());
            }
        }
        buf.extend_from_slice(&0u16.to_le_bytes()); // attribute
    }
    buf
}

#[test]
fn test_import_binary_stl() {
    let triangles = vec![
        (
            [0.0f32, 0.0, 1.0],
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        ),
        (
            [0.0, 0.0, 1.0],
            [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        ),
    ];

    let data = make_binary_stl_data(&triangles);
    let importer = StlImporter::from_bytes(data);
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    assert_eq!(entities.len(), 1);
    if let EntityType::Mesh(mesh) = &entities[0] {
        assert_eq!(mesh.faces.len(), 2);
        // Shared edge: 4 unique vertices
        assert_eq!(mesh.vertices.len(), 4);
    } else {
        panic!("Expected Mesh entity");
    }
}

#[test]
fn test_import_empty_stl() {
    let data = make_binary_stl_data(&[]);
    let importer = StlImporter::from_bytes(data);
    let doc = importer.import().unwrap();
    assert_eq!(doc.entities().count(), 0);
}

// ─── Layer naming tests ─────────────────────────────────────────────────

#[test]
fn test_stl_layer_named_from_solid() {
    let stl = b"solid MyModel\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid MyModel\n";

    let importer = StlImporter::from_bytes(stl.to_vec());
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    if let EntityType::Mesh(mesh) = &entities[0] {
        assert!(
            mesh.common.layer.contains("MyModel"),
            "Layer should contain solid name, got: {}",
            mesh.common.layer
        );
    } else {
        panic!("Expected Mesh entity");
    }
}

// ─── Roundtrip: STL → DXF → re-read ────────────────────────────────────

#[test]
fn test_stl_roundtrip_through_dxf() {
    let stl = b"solid rt\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 10 0 0\nvertex 0 10 0\nendloop\nendfacet\nendsolid rt\n";

    let importer = StlImporter::from_bytes(stl.to_vec());
    let doc = importer.import().unwrap();

    // Write to in-memory DXF
    let mut dxf_buf = Vec::new();
    acadrust::DxfWriter::new(&doc)
        .write_to_writer(&mut dxf_buf)
        .expect("DXF write should succeed");

    // Read back
    let doc2 = acadrust::DxfReader::from_reader(std::io::Cursor::new(dxf_buf))
        .expect("DXF reader creation should succeed")
        .read()
        .expect("DXF read should succeed");

    let entities2: Vec<_> = doc2.entities().collect();
    assert_eq!(
        entities2.len(),
        1,
        "Should still have 1 entity after roundtrip"
    );
    if let EntityType::Mesh(mesh) = &entities2[0] {
        assert_eq!(mesh.faces.len(), 1, "Should still have 1 face");
    } else {
        panic!("Expected Mesh entity after roundtrip");
    }
}
