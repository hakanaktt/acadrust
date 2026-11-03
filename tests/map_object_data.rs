//! Map 3D Object Data: tables defined and records attached through the API
//! survive DXF and DWG, as typed values.

use acadrust::entities::{EntityType, Line};
use acadrust::objects::{
    ClassObject, ClassObjectData, ObjectDataField, ObjectDataTable, ObjectDataType,
    ObjectDataValue, ObjectType,
};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};
use std::io::Cursor;

fn table() -> ObjectDataTable {
    ObjectDataTable::new(
        "TEST",
        vec![
            ObjectDataField::new("VOLTAGE", ObjectDataType::Real),
            ObjectDataField::new("CIRCUITS", ObjectDataType::Integer),
            ObjectDataField::new("OWNER", ObjectDataType::Character),
        ],
    )
}

fn values(owner: &str) -> Vec<ObjectDataValue> {
    vec![
        ObjectDataValue::Real(230.0),
        ObjectDataValue::Integer(2),
        ObjectDataValue::Character(owner.into()),
    ]
}

/// A drawing with one line carrying a record, and the line's handle.
fn drawing(owner: &str) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let line = document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 0.0, 0.0,
        )))
        .unwrap();
    let table = document.add_object_data_table(&table()).unwrap();
    document
        .attach_object_data(line, table, &values(owner))
        .unwrap();
    (document, line)
}

fn through_dxf(document: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(document).write_to_vec().unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn through_dwg(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn line(document: &CadDocument) -> Handle {
    document
        .entities()
        .find(|entity| matches!(entity, EntityType::Line(_)))
        .unwrap()
        .common()
        .handle
}

fn assert_object_data(document: &CadDocument, owner: &str) {
    let tables = document.object_data_tables();
    assert_eq!(tables.len(), 1);
    let (table_handle, read) = &tables[0];
    assert_eq!(read, &table());
    let records = document.object_data(line(document));
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].table, *table_handle);
    assert_eq!(records[0].values, values(owner));
}

#[test]
fn api_records_read_back() {
    let (document, line) = drawing("hello");
    assert_object_data(&document, "hello");
    assert!(document
        .objects
        .contains_key(&document.object_data(line)[0].record));
}

#[test]
fn dxf_round_trip() {
    assert_object_data(&through_dxf(&drawing("hello").0), "hello");
}

#[test]
fn dwg_round_trip() {
    assert_object_data(&through_dwg(&drawing("hello").0), "hello");
}

#[test]
fn dxf_to_dwg_to_dxf() {
    let document = through_dxf(&through_dwg(&through_dxf(&drawing("hello").0)));
    assert_object_data(&document, "hello");
}

/// Map 3D splits a payload into DXF binary chunks of at most 127 bytes.
#[test]
fn long_values_span_dxf_chunks() {
    let owner = "Hé,".to_owned() + &"l".repeat(351) + "o\"'";
    let (document, _) = drawing(&owner);
    let text = String::from_utf8(DxfWriter::new(&document).write_to_vec().unwrap()).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let chunks: Vec<&str> = lines
        .windows(2)
        .filter(|pair| pair[0] == "1004")
        .map(|pair| pair[1])
        .collect();
    assert!(chunks.len() > 2);
    assert!(chunks.iter().all(|chunk| chunk.len() <= 254));
    assert_object_data(&through_dxf(&document), &owner);
    assert_object_data(&through_dwg(&document), &owner);
}

#[test]
fn second_table_and_record_share_the_entity() {
    let (mut document, line) = drawing("hello");
    let other = ObjectDataTable::new(
        "OTHER",
        vec![ObjectDataField::new("NAME", ObjectDataType::Character)],
    );
    let other_handle = document.add_object_data_table(&other).unwrap();
    let name = vec![ObjectDataValue::Character("x".into())];
    document
        .attach_object_data(line, other_handle, &name)
        .unwrap();

    let read = through_dwg(&document);
    let records = read.object_data(self::line(&read));
    assert_eq!(records.len(), 2);
    assert!(records.iter().any(|record| record.values == name));
}

#[test]
fn duplicate_table_and_mismatched_values_are_refused() {
    let (mut document, line) = drawing("hello");
    assert!(document.add_object_data_table(&table()).is_err());
    let table = document.object_data_tables()[0].0;
    assert!(document
        .attach_object_data(line, table, &[ObjectDataValue::Integer(1)])
        .is_err());
}

/// A descriptor this crate can't read as a table still round-trips.
#[test]
fn unread_descriptor_payload_is_kept() {
    let (mut document, _) = drawing("hello");
    let payload = vec![0x00, 0x30, 0x00, 0x00, 0xFF, 0xFF];
    let handle = document
        .objects
        .iter()
        .find_map(|(handle, object)| match object {
            ObjectType::ClassObject(ClassObject {
                data: ClassObjectData::ObjectDataDescriptor(descriptor),
                ..
            }) if descriptor.as_table().is_some() => Some(*handle),
            _ => None,
        })
        .unwrap();
    if let Some(ObjectType::ClassObject(object)) = document.objects.get_mut(&handle) {
        if let ClassObjectData::ObjectDataDescriptor(descriptor) = &mut object.data {
            descriptor.payload = payload.clone();
        }
    }
    for read in [through_dxf(&document), through_dwg(&document)] {
        let kept = read.objects.values().any(|object| {
            matches!(object, ObjectType::ClassObject(ClassObject {
                data: ClassObjectData::ObjectDataDescriptor(descriptor),
                ..
            }) if descriptor.payload == payload)
        });
        assert!(kept);
    }
}

/// Map never reuses a deleted entry's key, so new keys follow the highest.
#[test]
fn attach_after_a_deleted_record_takes_a_new_key() {
    let (mut document, line) = drawing("hello");
    let table = document.object_data_tables()[0].0;
    document
        .attach_object_data(line, table, &values("second"))
        .unwrap();
    let xdictionary = document
        .get_entity(line)
        .unwrap()
        .common()
        .xdictionary_handle
        .unwrap();
    let Some(ObjectType::Dictionary(dictionary)) = document.objects.get_mut(&xdictionary) else {
        panic!("no extension dictionary");
    };
    dictionary.entries.retain(|(key, _)| key != "*A1");
    document
        .attach_object_data(line, table, &values("third"))
        .unwrap();

    let Some(ObjectType::Dictionary(dictionary)) = document.objects.get(&xdictionary) else {
        panic!("no extension dictionary");
    };
    let keys: Vec<&str> = dictionary
        .entries
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    assert_eq!(keys, ["*A2", "*A3"]);
}

/// A type dictionary entry whose descriptor is gone is not written.
#[test]
fn missing_descriptor_is_not_referenced() {
    let (mut document, _) = drawing("hello");
    let table = document.object_data_tables()[0].0;
    document.objects.remove(&table);
    for read in [through_dxf(&document), through_dwg(&document)] {
        let types = read
            .objects
            .values()
            .find_map(|object| match object {
                ObjectType::ClassObject(ClassObject {
                    data: ClassObjectData::ObjectDataDictionary(types),
                    ..
                }) => Some(types.entries.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(types.len(), 5, "only the built-in types remain");
        assert!(types
            .iter()
            .all(|(_, handle)| read.objects.contains_key(handle)));
    }
}
