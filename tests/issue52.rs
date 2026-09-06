//! Entity iteration APIs for issue #52: entities stored inside block
//! definitions are not part of the model-space drawable set, the way CAD
//! applications do not render unreferenced block geometry.

use acadrust::entities::{EntityType, Line};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn build_doc() -> CadDocument {
    let mut doc = CadDocument::new();

    // Model-space line (default routing).
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 10.0, 0.0, 0.0,
    )))
    .unwrap();

    // A line owned by *Paper_Space: kept out of model-space iteration.
    let ps_handle = doc.block_records.get("*Paper_Space").unwrap().handle;
    let mut ps_line = Line::from_coords(100.0, 100.0, 0.0, 110.0, 100.0, 0.0);
    ps_line.common.owner_handle = ps_handle;
    doc.add_entity(EntityType::Line(ps_line)).unwrap();

    doc
}

#[test]
fn model_space_entities_excludes_block_geometry() {
    let doc = build_doc();

    assert_eq!(
        doc.entities().count(),
        2,
        "flat storage holds both entities"
    );
    assert_eq!(
        doc.model_space_entities().count(),
        1,
        "only the model-space entity is drawable"
    );
    assert_eq!(doc.entities_in_block("*Model_Space").count(), 1);
    assert_eq!(doc.entities_in_block("*Paper_Space").count(), 1);
    assert_eq!(doc.entities_in_block("missing").count(), 0);

    let ms = doc.model_space_entities().next().unwrap();
    assert_eq!(
        ms.common().owner_handle,
        doc.block_records.get("*Model_Space").unwrap().handle
    );
}

#[test]
fn model_space_iteration_survives_roundtrip() {
    let doc = build_doc();
    let path = std::env::temp_dir().join("issue52_roundtrip.dxf");
    DxfWriter::new(&doc).write_to_file(&path).unwrap();

    let loaded = DxfReader::from_file(&path).unwrap().read().unwrap();
    assert_eq!(loaded.entities().count(), 2);
    assert_eq!(loaded.model_space_entities().count(), 1);
    assert_eq!(loaded.entities_in_block("*Paper_Space").count(), 1);
}
