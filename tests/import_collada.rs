//! Integration tests for COLLADA import.

#![cfg(feature = "import")]

use acadrust::entities::EntityType;
use acadrust::io::import::collada::ColladaImporter;
use acadrust::io::import::ImportConfig;

const TRIANGLE_DAE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
  <library_effects>
    <effect id="eff_red">
      <profile_COMMON><technique sid="t"><phong>
        <diffuse><color>1.0 0.0 0.0 1.0</color></diffuse>
      </phong></technique></profile_COMMON>
    </effect>
  </library_effects>
  <library_materials>
    <material id="mat_red" name="Red">
      <instance_effect url="#eff_red"/>
    </material>
  </library_materials>
  <library_geometries>
    <geometry id="geom0" name="Triangle">
      <mesh>
        <source id="pos0">
          <float_array id="pos0a" count="9">0 0 0 1 0 0 0 1 0</float_array>
          <technique_common>
            <accessor source="#pos0a" count="3" stride="3">
              <param name="X" type="float"/>
              <param name="Y" type="float"/>
              <param name="Z" type="float"/>
            </accessor>
          </technique_common>
        </source>
        <vertices id="verts0">
          <input semantic="POSITION" source="#pos0"/>
        </vertices>
        <triangles count="1" material="sym_red">
          <input semantic="VERTEX" source="#verts0" offset="0"/>
          <p>0 1 2</p>
        </triangles>
      </mesh>
    </geometry>
  </library_geometries>
  <library_visual_scenes>
    <visual_scene id="Scene">
      <node name="TriangleNode">
        <instance_geometry url="#geom0">
          <bind_material><technique_common>
            <instance_material symbol="sym_red" target="#mat_red"/>
          </technique_common></bind_material>
        </instance_geometry>
      </node>
    </visual_scene>
  </library_visual_scenes>
</COLLADA>"##;

#[test]
fn test_import_collada_single_triangle() {
    let importer = ColladaImporter::from_bytes(TRIANGLE_DAE.as_bytes().to_vec());
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    assert_eq!(entities.len(), 1, "Should produce 1 Mesh entity");

    if let EntityType::Mesh(mesh) = &entities[0] {
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.faces.len(), 1);
    } else {
        panic!("Expected Mesh entity");
    }
}

#[test]
fn test_collada_material_layer() {
    let importer = ColladaImporter::from_bytes(TRIANGLE_DAE.as_bytes().to_vec());
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    if let EntityType::Mesh(mesh) = &entities[0] {
        // Should be on a layer named after the material "Red"
        assert!(
            mesh.common.layer.contains("Red"),
            "Layer should contain material name 'Red', got: '{}'",
            mesh.common.layer
        );
    } else {
        panic!("Expected Mesh entity");
    }
}

// ─── Multi-material test ─────────────────────────────────────────────────

const TWO_MATERIAL_DAE: &str = r##"<?xml version="1.0" encoding="utf-8"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
  <library_effects>
    <effect id="eff_r">
      <profile_COMMON><technique sid="t"><phong>
        <diffuse><color>1.0 0.0 0.0 1.0</color></diffuse>
      </phong></technique></profile_COMMON>
    </effect>
    <effect id="eff_b">
      <profile_COMMON><technique sid="t"><phong>
        <diffuse><color>0.0 0.0 1.0 1.0</color></diffuse>
      </phong></technique></profile_COMMON>
    </effect>
  </library_effects>
  <library_materials>
    <material id="mat_r" name="RedMat">
      <instance_effect url="#eff_r"/>
    </material>
    <material id="mat_b" name="BlueMat">
      <instance_effect url="#eff_b"/>
    </material>
  </library_materials>
  <library_geometries>
    <geometry id="g0" name="Quad">
      <mesh>
        <source id="p0">
          <float_array id="p0a" count="12">0 0 0 1 0 0 1 1 0 0 1 0</float_array>
          <technique_common>
            <accessor source="#p0a" count="4" stride="3">
              <param name="X" type="float"/>
              <param name="Y" type="float"/>
              <param name="Z" type="float"/>
            </accessor>
          </technique_common>
        </source>
        <vertices id="v0">
          <input semantic="POSITION" source="#p0"/>
        </vertices>
        <triangles count="1" material="s_red">
          <input semantic="VERTEX" source="#v0" offset="0"/>
          <p>0 1 2</p>
        </triangles>
        <triangles count="1" material="s_blue">
          <input semantic="VERTEX" source="#v0" offset="0"/>
          <p>0 2 3</p>
        </triangles>
      </mesh>
    </geometry>
  </library_geometries>
  <library_visual_scenes>
    <visual_scene id="Scene">
      <node name="QuadNode">
        <instance_geometry url="#g0">
          <bind_material><technique_common>
            <instance_material symbol="s_red" target="#mat_r"/>
            <instance_material symbol="s_blue" target="#mat_b"/>
          </technique_common></bind_material>
        </instance_geometry>
      </node>
    </visual_scene>
  </library_visual_scenes>
</COLLADA>"##;

#[test]
fn test_collada_multi_material() {
    let importer = ColladaImporter::from_bytes(TWO_MATERIAL_DAE.as_bytes().to_vec());
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    // Should produce 2 Mesh entities — one per material
    assert_eq!(
        entities.len(),
        2,
        "Should produce 2 Mesh entities for 2 materials"
    );

    let mut layers: Vec<String> = entities
        .iter()
        .map(|e| match e {
            EntityType::Mesh(m) => m.common.layer.clone(),
            _ => panic!("Expected Mesh entity"),
        })
        .collect();
    layers.sort();

    // One layer should contain "Blue", the other "Red"
    assert!(
        layers.iter().any(|l| l.contains("Blue")),
        "Should have a Blue layer, got: {:?}",
        layers
    );
    assert!(
        layers.iter().any(|l| l.contains("Red")),
        "Should have a Red layer, got: {:?}",
        layers
    );
}

// ─── Scale factor test ───────────────────────────────────────────────────

#[test]
fn test_collada_scale_factor() {
    let mut config = ImportConfig::default();
    config.scale_factor = 100.0;
    config.merge_vertices = false;

    let importer =
        ColladaImporter::from_bytes(TRIANGLE_DAE.as_bytes().to_vec()).with_config(config);
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    if let EntityType::Mesh(mesh) = &entities[0] {
        // vertex (1, 0, 0) × 100 = (100, 0, 0)
        let has_scaled = mesh.vertices.iter().any(|v| (v.x - 100.0).abs() < 0.001);
        assert!(has_scaled, "Vertices should be scaled by 100");
    } else {
        panic!("Expected Mesh entity");
    }
}

// ─── Geometry without visual scene ───────────────────────────────────────

#[test]
fn test_collada_no_visual_scene() {
    let dae = r##"<?xml version="1.0" encoding="utf-8"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
  <library_geometries>
    <geometry id="g0" name="Bare">
      <mesh>
        <source id="p0">
          <float_array id="p0a" count="9">0 0 0 1 0 0 0 1 0</float_array>
          <technique_common>
            <accessor source="#p0a" count="3" stride="3">
              <param name="X" type="float"/>
              <param name="Y" type="float"/>
              <param name="Z" type="float"/>
            </accessor>
          </technique_common>
        </source>
        <vertices id="v0">
          <input semantic="POSITION" source="#p0"/>
        </vertices>
        <triangles count="1">
          <input semantic="VERTEX" source="#v0" offset="0"/>
          <p>0 1 2</p>
        </triangles>
      </mesh>
    </geometry>
  </library_geometries>
</COLLADA>"##;

    let importer = ColladaImporter::from_bytes(dae.as_bytes().to_vec());
    let doc = importer.import().unwrap();

    let entities: Vec<_> = doc.entities().collect();
    assert_eq!(
        entities.len(),
        1,
        "Should import geometry even without visual_scene"
    );
}
