use acadrust::entities::{solid3d::Solid3D, EntityType};
use acadrust::objects::{
    SolidHistoryBox, SolidHistoryBrep, SolidHistoryFillet, SolidHistoryNodeBase,
    SolidHistoryOperation,
};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

fn box_step(step_id: i32) -> SolidHistoryOperation {
    SolidHistoryOperation::Box(SolidHistoryBox {
        base: SolidHistoryNodeBase::new(step_id),
        length: 2.0,
        width: 3.0,
        height: 4.0,
        ..SolidHistoryBox::default()
    })
}

fn fillet_step() -> SolidHistoryOperation {
    SolidHistoryOperation::Fillet(SolidHistoryFillet {
        base: SolidHistoryNodeBase::new(0),
        radii: vec![0.25],
        ..SolidHistoryFillet::default()
    })
}

#[test]
fn appended_history_is_returned_root_to_active() {
    let mut document = CadDocument::new();
    let entity = document
        .add_entity(EntityType::Solid3D(Solid3D::new()))
        .unwrap();
    document.create_solid_history(entity, box_step(1)).unwrap();
    document
        .append_solid_history(entity, fillet_step())
        .unwrap();

    let operations = document.solid_history_operations(entity).unwrap();
    assert_eq!(operations.len(), 2);
    assert!(matches!(operations[0], SolidHistoryOperation::Box(_)));
    assert!(matches!(operations[1], SolidHistoryOperation::Fillet(_)));
    assert_eq!(operations[0].base().unwrap().eval.parent_id, SolidHistoryNodeBase::ROOT_PARENT);
    // Nodes are linked through the evaluation graph; every node stores the
    // root parent id.
    assert_eq!(operations[1].base().unwrap().eval.parent_id, SolidHistoryNodeBase::ROOT_PARENT);
    assert!(document.solid_history_graph(entity).unwrap().evaluation_graph.is_some());
}

#[test]
fn updating_a_step_preserves_its_graph_identity() {
    let mut document = CadDocument::new();
    let entity = document
        .add_entity(EntityType::Solid3D(Solid3D::new()))
        .unwrap();
    document.create_solid_history(entity, box_step(1)).unwrap();
    document
        .append_solid_history(entity, fillet_step())
        .unwrap();

    let mut replacement = document.solid_history_operations(entity).unwrap()[0].clone();
    let base = replacement.base_mut().unwrap();
    base.eval.parent_id = 99;
    if let SolidHistoryOperation::Box(value) = &mut replacement {
        value.length = 8.0;
    }
    document
        .update_solid_history_step(entity, replacement)
        .unwrap();

    let operations = document.solid_history_operations(entity).unwrap();
    assert_eq!(operations[0].base().unwrap().eval.parent_id, SolidHistoryNodeBase::ROOT_PARENT);
    assert_eq!(operations[1].base().unwrap().eval.parent_id, SolidHistoryNodeBase::ROOT_PARENT);
    assert!(matches!(
        &operations[0],
        SolidHistoryOperation::Box(value) if value.length == 8.0
    ));
}

#[test]
fn binary_brep_history_survives_r2018_dwg_roundtrip() {
    let sat = acadrust::entities::acis::primitives::build_planar_body(
        &[
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        &[
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 1, 5, 4],
            vec![3, 7, 6, 2],
            vec![1, 2, 6, 5],
            vec![0, 4, 7, 3],
        ],
    )
    .unwrap();
    let sab = acadrust::SabWriter::write(&sat);
    let operation = SolidHistoryOperation::Brep(SolidHistoryBrep {
        base: SolidHistoryNodeBase::new(1),
        acis_data: acadrust::entities::AcisData::from_sab(sab.clone()),
        ..SolidHistoryBrep::default()
    });
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let entity = document
        .add_entity(EntityType::Solid3D(Solid3D::new()))
        .unwrap();
    document.create_solid_history(entity, operation).unwrap();

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let roundtrip = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let operations = roundtrip.solid_history_operations(entity).unwrap();

    assert_eq!(operations.len(), 1);
    let SolidHistoryOperation::Brep(value) = &operations[0] else {
        panic!("history operation changed type: {:?}", operations[0]);
    };
    assert_eq!(value.acis_data.sab_data, sab);
}
