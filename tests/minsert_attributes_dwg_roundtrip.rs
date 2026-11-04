use std::io::Cursor;

use acadrust::entities::{AttributeDefinition, AttributeEntity, Circle, EntityType, Insert};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

const VERSIONS: [DxfVersion; 4] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1024,
    DxfVersion::AC1032,
];

/// A block with a circle and one attribute definition, and one reference to
/// it carrying a filled-in attribute. `array` turns the reference into a
/// 2 column x 3 row MINSERT.
fn tagged_reference(version: DxfVersion, array: bool) -> (CadDocument, Handle) {
    let mut doc = CadDocument::with_version(version);
    let mut record = BlockRecord::new("Tagged");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let circle = Circle::from_center_radius(Vector3::ZERO, 1.0);
    record
        .entity_handles
        .push(doc.add_entity(EntityType::Circle(circle)).unwrap());
    let definition = AttributeDefinition::new("TAG".into(), "Tag".into(), "default".into());
    record.entity_handles.push(
        doc.add_entity(EntityType::AttributeDefinition(definition))
            .unwrap(),
    );
    doc.block_records.add(record).unwrap();

    let mut insert = Insert::new("Tagged", Vector3::new(5.0, 5.0, 0.0));
    if array {
        insert = insert.with_array(2, 3, 15.0, 10.0);
    }
    insert
        .attributes
        .push(AttributeEntity::simple("TAG", "value"));
    let handle = doc.add_entity(EntityType::Insert(insert)).unwrap();
    (doc, handle)
}

fn dwg_roundtrip(doc: &CadDocument, handle: Handle) -> Insert {
    let bytes = DwgWriter::write_to_vec(doc).expect("DWG write failed");
    let read = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed");
    let Some(EntityType::Insert(insert)) = read.get_entity(handle) else {
        panic!("{:?}: the block reference is missing", doc.version);
    };
    insert.clone()
}

#[test]
fn an_attributed_minsert_keeps_its_array_through_a_dwg_roundtrip() {
    for version in VERSIONS {
        let (doc, handle) = tagged_reference(version, true);
        let insert = dwg_roundtrip(&doc, handle);
        assert!(insert.is_minsert(), "{version:?}: no longer a MINSERT");
        assert_eq!(
            (
                insert.column_count,
                insert.row_count,
                insert.column_spacing,
                insert.row_spacing
            ),
            (2, 3, 15.0, 10.0),
            "{version:?}: array fields (columns, rows, column spacing, row spacing)"
        );
        assert_eq!(insert.attributes.len(), 1, "{version:?}: attributes");
        assert_eq!(insert.attributes[0].tag, "TAG", "{version:?}");
        assert_eq!(insert.attributes[0].value, "value", "{version:?}");
    }
}

#[test]
fn an_attributed_insert_survives_a_dwg_roundtrip_in_every_version() {
    for version in VERSIONS {
        let (doc, handle) = tagged_reference(version, false);
        let insert = dwg_roundtrip(&doc, handle);
        assert!(!insert.is_minsert(), "{version:?}: became a MINSERT");
        assert_eq!(
            insert.insert_point,
            Vector3::new(5.0, 5.0, 0.0),
            "{version:?}"
        );
        assert_eq!(insert.attributes.len(), 1, "{version:?}: attributes");
        assert_eq!(insert.attributes[0].tag, "TAG", "{version:?}");
        assert_eq!(insert.attributes[0].value, "value", "{version:?}");
    }
}
