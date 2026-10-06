
use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn dxf_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("DXF write failed");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

fn door(description: &str) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut record = BlockRecord::new("Door");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    record.base_point = Vector3::new(2.0, 3.0, 4.0);
    record.description = description.to_string();
    let line = Line::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
    let handle = doc.add_entity(EntityType::Line(line)).unwrap();
    record.entity_handles.push(handle);
    doc.block_records.add(record).unwrap();
    doc
}

#[test]
fn block_description_survives_a_dxf_roundtrip() {
    let doc = door("Entrance door");
    let text = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    assert!(
        text.lines().any(|line| line.trim() == "Entrance door"),
        "the description is written as BLOCK group 4"
    );
    let record = dxf_roundtrip(&doc).block_records.get("Door").cloned();
    assert_eq!(record.expect("Door missing").description, "Entrance door");
}

#[test]
fn a_unicode_description_survives_and_an_empty_one_stays_empty() {
    let record = dxf_roundtrip(&door("Kapı — dış cephe"))
        .block_records
        .get("Door")
        .cloned()
        .expect("Door missing");
    assert_eq!(record.description, "Kapı — dış cephe");

    let record = dxf_roundtrip(&door(""))
        .block_records
        .get("Door")
        .cloned()
        .expect("Door missing");
    assert!(record.description.is_empty(), "no description, none invented");
}

#[test]
fn the_dwg_block_marker_agrees_with_its_record_on_the_base_point() {
    let bytes = DwgWriter::write_to_vec(&door("Entrance door")).expect("DWG write failed");
    let read = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed");
    let record = read.block_records.get("Door").expect("Door missing").clone();
    assert_eq!(record.base_point, Vector3::new(2.0, 3.0, 4.0));
    let Some(EntityType::Block(marker)) = read.get_entity(record.block_entity_handle) else {
        panic!("the BLOCK marker is missing from the document");
    };
    assert_eq!(marker.name, record.name);
    assert_eq!(
        marker.base_point, record.base_point,
        "the marker and its record are two views of one definition"
    );
}

#[test]
fn an_explicit_attribute_flag_survives_without_attribute_definitions() {
    let mut doc = door("Synthetic block");
    doc.block_records.get_mut("Door").unwrap().flags.has_attributes = true;
    let saved = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    assert!(saved.block_records.get("Door").unwrap().flags.has_attributes);
    assert_eq!(doc.entities().count(), saved.entities().count());
}
