//! DWG writer drops reactors that point at handles which will not be in the
//! written file (the DXF writer already does this, see `reactor_handles_dxf.rs`).
//! Typical source: an entity is deleted after load, and the object that listed
//! it as a reactor (a DIMSTYLE, a LAYOUT, an associative dictionary) is kept;
//! readers then report "Object handle not found" / "Object was erased".

use acadrust::entities::{Circle, EntityType, Line};
use acadrust::io::dwg::{DwgReader, DwgWriter};
use acadrust::types::{Handle, Vector3};
use acadrust::CadDocument;

#[test]
fn dwg_entity_reactors_to_missing_handles_are_dropped() {
    let mut document = CadDocument::new();
    let line1 = Line::from_points(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    let line1_handle = document.add_entity(EntityType::Line(line1)).unwrap();
    let line2 = Line::from_points(Vector3::new(10.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0));
    let line2_handle = document.add_entity(EntityType::Line(line2)).unwrap();
    let doomed = Line::from_points(Vector3::ZERO, Vector3::new(0.0, 10.0, 0.0));
    let doomed_handle = document.add_entity(EntityType::Line(doomed)).unwrap();

    let mut circle = Circle::from_center_radius(Vector3::ZERO, 5.0);
    circle.common.reactors.push(line1_handle);
    circle.common.reactors.push(doomed_handle);
    circle.common.reactors.push(Handle::new(0xDEADBEEF));
    circle.common.reactors.push(line2_handle);
    let circle_handle = document.add_entity(EntityType::Circle(circle)).unwrap();
    document.remove_entity(doomed_handle).unwrap();

    let path = std::env::temp_dir().join(format!("acadrust-reactors-{}.dwg", std::process::id()));
    DwgWriter::write_to_file(&path, &document).unwrap();
    let back = DwgReader::from_file(&path).unwrap().read().unwrap();
    let _ = std::fs::remove_file(&path);

    let reactors = &back.get_entity(circle_handle).expect("circle survives").common().reactors;
    assert_eq!(reactors, &vec![line1_handle, line2_handle]);
}
