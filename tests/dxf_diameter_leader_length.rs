//! A diameter dimension keeps its leader length (group 40) through a DXF
//! write and read, as a radius dimension does, and a radius dimension's
//! base keeps its chord point as the DWG reader keeps it (#603).

use acadrust::entities::{Dimension, DimensionDiameter, DimensionRadius, EntityType};
use acadrust::io::dxf::{DxfReader, DxfWriter};
use acadrust::types::Vector3;
use acadrust::CadDocument;
use std::io::Cursor;

#[test]
fn diameter_and_radius_keep_their_leader_length_in_dxf() {
    let mut doc = CadDocument::new();
    let mut d = DimensionDiameter::new(Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0));
    d.leader_length = 1.5;
    doc.add_entity(EntityType::Dimension(Box::new(Dimension::Diameter(d)))).unwrap();
    let mut r = DimensionRadius::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(2.0, 0.0, 0.0));
    r.leader_length = 2.5;
    doc.add_entity(EntityType::Dimension(Box::new(Dimension::Radius(r)))).unwrap();

    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    let read = DxfReader::from_reader(Cursor::new(bytes)).unwrap().read().unwrap();
    let mut seen = (false, false);
    for e in read.entities() {
        if let EntityType::Dimension(dim) = e {
            match &**dim {
                Dimension::Diameter(d) => {
                    assert_eq!(d.leader_length, 1.5);
                    seen.0 = true;
                }
                Dimension::Radius(r) => {
                    assert_eq!(r.leader_length, 2.5);
                    assert_eq!(r.base.definition_point, Vector3::new(2.0, 0.0, 0.0));
                    seen.1 = true;
                }
                _ => {}
            }
        }
    }
    assert_eq!(seen, (true, true));
}
