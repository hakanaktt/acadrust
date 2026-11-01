//! ATTRIB field length (73) and the line spacing factor (41) of ARC_DIMENSION
//! and LARGE_RADIAL_DIMENSION survive a DXF write and read.
//!
//! The writer stores both; the reader had no 73 arm for ATTRIB and read the
//! line spacing factor of these two dimension entities from 44 instead of
//! the AcDbDimension 41.

use acadrust::entities::{
    AttributeEntity, Dimension, DimensionArc, DimensionLargeRadial, EntityType, Insert,
};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("write dxf");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf")
}

#[test]
fn attrib_field_length_survives_dxf() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut record = acadrust::tables::BlockRecord::new("ATTBLK");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    doc.block_records.add(record).unwrap();
    let mut insert = Insert::new("ATTBLK", Vector3::new(1.0, 2.0, 0.0));
    let mut attrib = AttributeEntity::new("TAG".into(), "value".into());
    attrib.field_length = 5;
    insert.attributes.push(attrib);
    let handle = doc.add_entity(EntityType::Insert(insert)).unwrap();
    match round_trip(&doc).get_entity(handle) {
        Some(EntityType::Insert(insert)) => {
            assert_eq!(insert.attributes.len(), 1);
            assert_eq!(insert.attributes[0].field_length, 5);
        }
        other => panic!("insert lost: {other:?}"),
    }
}

#[test]
fn arc_and_jogged_dimension_line_spacing_factor_survives_dxf() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut arc = DimensionArc::default();
    arc.center_point = Vector3::new(0.0, 0.0, 0.0);
    arc.first_extension_point = Vector3::new(10.0, 0.0, 0.0);
    arc.second_extension_point = Vector3::new(0.0, 10.0, 0.0);
    arc.definition_point = Vector3::new(8.0, 8.0, 0.0);
    arc.arc_end_parameter = 1.5;
    arc.base.line_spacing_factor = 1.5;
    let mut jogged = DimensionLargeRadial::default();
    jogged.chord_point = Vector3::new(10.0, 0.0, 0.0);
    jogged.base.line_spacing_factor = 0.75;
    let arc = doc
        .add_entity(EntityType::Dimension(Box::new(Dimension::Arc(arc))))
        .unwrap();
    let jogged = doc
        .add_entity(EntityType::Dimension(Box::new(Dimension::LargeRadial(
            jogged,
        ))))
        .unwrap();
    let back = round_trip(&doc);
    for (handle, factor) in [(arc, 1.5), (jogged, 0.75)] {
        match back.get_entity(handle) {
            Some(EntityType::Dimension(d)) => {
                assert_eq!(d.base().line_spacing_factor, factor, "{d:?}")
            }
            other => panic!("dimension lost: {other:?}"),
        }
    }
    // The arc's own 41 (arc end parameter) is not taken for the factor.
    match back.get_entity(arc) {
        Some(EntityType::Dimension(d)) => match d.as_ref() {
            Dimension::Arc(arc) => assert_eq!(arc.arc_end_parameter, 1.5),
            other => panic!("not an arc dimension: {other:?}"),
        },
        other => panic!("dimension lost: {other:?}"),
    }
}
