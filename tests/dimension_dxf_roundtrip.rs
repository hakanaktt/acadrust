//! DXF round-trip of dimension geometry/units: dimension-line rotation
//! (degrees<->radians), radial centre/arc point group codes, and the ordinate
//! X/Y datum bit. Regression tests for the reader/writer pairing.

use acadrust::entities::{Dimension, DimensionLinear, DimensionOrdinate, DimensionRadius};
use acadrust::types::Vector3;
use acadrust::{CadDocument, DxfReader, DxfWriter, EntityType};

fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
    let path = std::env::temp_dir().join(format!("acadrust_dim_rt_{tag}.dxf"));
    DxfWriter::new(doc).write_to_file(&path).expect("write dxf");
    let loaded = DxfReader::from_file(&path)
        .expect("open")
        .read()
        .expect("read");
    let _ = std::fs::remove_file(&path);
    loaded
}

#[test]
fn linear_rotation_survives_dxf_roundtrip() {
    let mut doc = CadDocument::new();
    let mut d =
        DimensionLinear::vertical(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 10.0, 0.0));
    d.definition_point = Vector3::new(5.0, 5.0, 0.0);
    d.base.definition_point = d.definition_point;
    doc.add_entity(EntityType::Dimension(Dimension::Linear(d)))
        .unwrap();
    let loaded = roundtrip(&doc, "lin");
    let dim = loaded
        .entities()
        .find_map(|e| match e {
            EntityType::Dimension(Dimension::Linear(d)) => Some(d),
            _ => None,
        })
        .expect("linear dim");
    assert!(
        (dim.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-6,
        "rotation {} should round-trip as PI/2 (was flattened by a degrees/radians bug)",
        dim.rotation
    );
}

#[test]
fn block_name_survives_dxf_roundtrip() {
    // The anonymous block holding the baked dimension picture (group code 2)
    // must round-trip. The reader previously ignored code 2, so every DXF
    // dimension came back with an empty block_name and lost its geometry link —
    // consumers then recomputed the picture and the text drifted.
    let mut doc = CadDocument::new();
    let mut d =
        DimensionLinear::vertical(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.0, 10.0, 0.0));
    d.base.block_name = "*D42".to_string();
    doc.add_entity(EntityType::Dimension(Dimension::Linear(d)))
        .unwrap();
    let loaded = roundtrip(&doc, "blk");
    let dim = loaded
        .entities()
        .find_map(|e| match e {
            EntityType::Dimension(Dimension::Linear(d)) => Some(d),
            _ => None,
        })
        .expect("linear dim");
    assert_eq!(
        dim.base.block_name, "*D42",
        "dimension block_name (DXF code 2) must survive the round-trip"
    );
}

#[test]
fn radius_points_survive_dxf_roundtrip() {
    let mut doc = CadDocument::new();
    let center = Vector3::new(3.0, 4.0, 0.0);
    let arc = Vector3::new(8.0, 4.0, 0.0);
    let mut d = DimensionRadius::new(center, arc);
    d.base.definition_point = d.definition_point; // group 10 = arc point
    doc.add_entity(EntityType::Dimension(Dimension::Radius(d)))
        .unwrap();
    let loaded = roundtrip(&doc, "rad");
    let dim = loaded
        .entities()
        .find_map(|e| match e {
            EntityType::Dimension(Dimension::Radius(d)) => Some(d),
            _ => None,
        })
        .expect("radius dim");
    assert!(
        (dim.angle_vertex.x - 3.0).abs() < 1e-6 && (dim.definition_point.x - 8.0).abs() < 1e-6,
        "radius centre/arc collapsed: centre={:?} arc={:?}",
        dim.angle_vertex,
        dim.definition_point
    );
}

#[test]
fn ordinate_xy_and_elbow_survive_dxf_roundtrip() {
    let mut doc = CadDocument::new();
    let mut d =
        DimensionOrdinate::y_ordinate(Vector3::new(2.0, 3.0, 0.0), Vector3::new(2.0, 9.0, 0.0));
    d.definition_point = Vector3::new(2.0, 7.0, 0.0); // leader elbow
    d.base.definition_point = d.definition_point;
    doc.add_entity(EntityType::Dimension(Dimension::Ordinate(d)))
        .unwrap();
    let loaded = roundtrip(&doc, "ord");
    let dim = loaded
        .entities()
        .find_map(|e| match e {
            EntityType::Dimension(Dimension::Ordinate(d)) => Some(d),
            _ => None,
        })
        .expect("ordinate dim");
    assert!(!dim.is_ordinate_type_x, "Y-ordinate reloaded as X");
    assert!(
        (dim.definition_point.y - 7.0).abs() < 1e-6,
        "ordinate leader elbow lost on reload: {:?}",
        dim.definition_point
    );
}
