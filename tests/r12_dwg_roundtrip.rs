use std::io::Cursor;

use acadrust::entities::{Face3D, Solid};
use acadrust::io::dwg::DwgVersion;
use acadrust::tables::BlockRecord;
use acadrust::tables::Layer;
use acadrust::{
    Arc, CadDocument, Circle, DwgReader, DwgWriter, DxfVersion, EntityType, Line, Point, Text,
    Vector3,
};

#[test]
fn ac1009_writer_roundtrips_a_line() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write AC1009 DWG");
    assert_eq!(&bytes[..6], b"AC1009");
    let info = DwgReader::from_stream(Cursor::new(bytes.clone()))
        .read_file_header()
        .expect("read AC1009 file header");
    assert_eq!(info.version, DwgVersion::AC9);
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read AC1009 DWG");
    assert_eq!(restored.version, DxfVersion::AC1009);
    assert!(restored
        .entities()
        .any(|entity| matches!(entity, EntityType::Line(_))));
}

#[test]
fn ac1009_roundtrips_the_native_entity_set() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::Point(Point::from_coords(1.0, 2.0, 0.0)))
        .unwrap();
    document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
    document
        .add_entity(EntityType::Circle(Circle::from_coords(4.0, 5.0, 0.0, 2.0)))
        .unwrap();
    document
        .add_entity(EntityType::Arc(Arc::from_coords(
            4.0,
            5.0,
            0.0,
            2.0,
            0.0,
            std::f64::consts::PI,
        )))
        .unwrap();
    document
        .add_entity(EntityType::Solid(Solid::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        )))
        .unwrap();
    document
        .add_entity(EntityType::Face3D(Face3D::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0),
            Vector3::new(0.0, 1.0, 1.0),
        )))
        .unwrap();
    document
        .add_entity(EntityType::Text(Text::with_value(
            "R12",
            Vector3::new(0.0, 0.0, 0.0),
        )))
        .unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write native R12 entities");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read native R12 entities");
    let kinds: Vec<&'static str> = restored
        .entities()
        .map(|entity| match entity {
            EntityType::Point(_) => "POINT",
            EntityType::Line(_) => "LINE",
            EntityType::Circle(_) => "CIRCLE",
            EntityType::Arc(_) => "ARC",
            EntityType::Solid(_) => "SOLID",
            EntityType::Face3D(_) => "3DFACE",
            EntityType::Text(_) => "TEXT",
            _ => "OTHER",
        })
        .collect();
    for expected in ["POINT", "LINE", "CIRCLE", "ARC", "SOLID", "3DFACE", "TEXT"] {
        assert!(kinds.contains(&expected), "missing {expected} in {kinds:?}");
    }
}

#[test]
fn ac1009_rejects_corrupt_entity_crc() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 2.0, 3.0, 0.0,
        )))
        .unwrap();
    let mut bytes = DwgWriter::write_to_vec(&document).unwrap();
    let end = u32::from_le_bytes(bytes[0x18..0x1c].try_into().unwrap()) as usize;
    bytes[end - 1] ^= 0x40;
    let error = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .unwrap_err();
    assert!(error.to_string().contains("CRC"));
}

#[test]
fn ac1009_roundtrips_block_entities() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    let mut block = BlockRecord::new("Door");
    block.handle = document.allocate_handle();
    block.block_entity_handle = document.allocate_handle();
    block.block_end_handle = document.allocate_handle();
    let entity = document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
        )))
        .unwrap();
    block.entity_handles.push(entity);
    document.block_records.add(block).unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write AC1009 block");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read AC1009 block");
    assert!(restored.block_records.get("Door").is_some());
    assert!(restored
        .entities()
        .any(|entity| matches!(entity, EntityType::Line(_))));
}

#[test]
fn ac1009_preserves_custom_layer_names() {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document.layers.add(Layer::new("CUT")).unwrap();
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    line.common.layer = "CUT".into();
    document.add_entity(EntityType::Line(line)).unwrap();
    let bytes = DwgWriter::write_to_vec(&document).expect("write custom layer");
    let restored = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read custom layer");
    let EntityType::Line(line) = restored
        .entities()
        .find(|entity| matches!(entity, EntityType::Line(_)))
        .unwrap()
    else {
        unreachable!()
    };
    assert_eq!(line.common.layer, "CUT");
}
