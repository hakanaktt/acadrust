//! An ATTRIB or ATTDEF hidden by its own attribute flag (DXF group 70,
//! bit 1) is invisible through the `Entity` trait, as the entity-level
//! invisible flag (group 60) already is.

use std::io::Cursor;

use acadrust::entities::{AttributeDefinition, AttributeEntity, Insert};
use acadrust::{
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter, Entity, EntityType,
    Handle, Vector3,
};

/// Every combination of the entity flag (group 60) and the attribute flag
/// (group 70, bit 1), with the visibility it must report.
const COMBINATIONS: [(bool, bool, bool); 4] = [
    (false, false, false),
    (true, false, true),
    (false, true, true),
    (true, true, true),
];

#[test]
fn an_attribute_is_invisible_when_either_flag_hides_it() {
    for (entity_flag, attribute_flag, invisible) in COMBINATIONS {
        let mut attribute = AttributeEntity::simple("TAG", "value");
        attribute.common.invisible = entity_flag;
        attribute.flags.invisible = attribute_flag;
        let entity = EntityType::AttributeEntity(attribute);

        assert_eq!(
            entity.as_entity().is_invisible(),
            invisible,
            "entity flag {entity_flag}, attribute flag {attribute_flag}"
        );
    }
}

#[test]
fn an_attribute_definition_is_invisible_when_either_flag_hides_it() {
    for (entity_flag, attribute_flag, invisible) in COMBINATIONS {
        let mut definition = AttributeDefinition::simple("TAG");
        definition.common.invisible = entity_flag;
        definition.flags.invisible = attribute_flag;
        let entity = EntityType::AttributeDefinition(definition);

        assert_eq!(
            entity.as_entity().is_invisible(),
            invisible,
            "entity flag {entity_flag}, attribute flag {attribute_flag}"
        );
    }
}

/// A drawing with one INSERT carrying a flag-hidden ATTRIB and one flag-hidden
/// ATTDEF, plus the handles to find both again after a round trip.
fn drawing_with_hidden_attributes() -> (CadDocument, Handle, Handle) {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut attribute = AttributeEntity::simple("TAG", "value");
    attribute.flags.invisible = true;
    let mut insert = Insert::new("*Model_Space", Vector3::ZERO);
    insert.attributes.push(attribute);
    let insert = document.add_entity(EntityType::Insert(insert)).unwrap();
    let definition = document
        .add_entity(EntityType::AttributeDefinition(
            AttributeDefinition::invisible("TAG", "default"),
        ))
        .unwrap();

    (document, insert, definition)
}

fn assert_hidden_attributes(document: &CadDocument, insert: Handle, definition: Handle) {
    let EntityType::Insert(insert) = document.get_entity(insert).unwrap() else {
        panic!("the INSERT did not come back");
    };
    assert!(insert.attributes[0].flags.invisible);
    assert!(insert.attributes[0].is_invisible());

    let definition = document.get_entity(definition).unwrap();
    assert!(definition.as_entity().is_invisible());
}

#[test]
fn flag_hidden_attributes_read_from_a_dxf_are_invisible() {
    let (document, insert, definition) = drawing_with_hidden_attributes();
    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let loaded = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();

    assert_hidden_attributes(&loaded, insert, definition);
}

#[test]
fn flag_hidden_attributes_read_from_a_dwg_are_invisible() {
    let (document, insert, definition) = drawing_with_hidden_attributes();
    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

    assert_hidden_attributes(&loaded, insert, definition);
}
