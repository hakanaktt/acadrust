//! Classes of registered-class records survive a DWG save (#319).
//!
//! A custom record read from DXF keeps its group codes as semantic
//! properties (`RegisteredClass`), and the DWG writer saves it as a proxy. The
//! proxy names its class by number, so the class has to be in the written
//! table: kept through the pre-R2013 rebuild of the table, and declared when
//! the document has no entry for it.

use acadrust::{
    classes::DxfClass,
    entities::{EntityType, ExtendedEntity, ExtendedEntityData, RegisteredClassEntityData},
    objects::{ObjectType, RegisteredClassObject, SemanticProperty, SemanticPropertyValue},
    types::DxfVersion,
    CadDocument, DwgReader, DwgWriter, Handle,
};
use std::io::Cursor;

const ENTITY_CLASS: (&str, &str) = ("SPDSNOTE", "mcsDbObjectNote");
const OBJECT_CLASS: (&str, &str) = ("SPDSDATACONTAINER", "mcsDbDataContainer");

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn properties() -> Vec<SemanticProperty> {
    vec![SemanticProperty {
        subclass: ENTITY_CLASS.1.to_string(),
        code: 1,
        value: SemanticPropertyValue::Text("Note".to_string()),
    }]
}

/// A drawing of `version` with a registered-class entity and object, both
/// with properties. `declared`: their classes are in the table.
fn document(version: DxfVersion, declared: bool) -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::with_version(version);
    if declared {
        doc.classes
            .add_or_update(DxfClass::new_entity(ENTITY_CLASS.0, ENTITY_CLASS.1));
        doc.classes
            .add_or_update(DxfClass::new(OBJECT_CLASS.0, OBJECT_CLASS.1));
    }
    let entity = doc
        .add_entity(EntityType::Extended(Box::new(ExtendedEntity {
            common: Default::default(),
            data: ExtendedEntityData::RegisteredClass(RegisteredClassEntityData {
                dxf_name: ENTITY_CLASS.0.to_string(),
                cpp_class_name: ENTITY_CLASS.1.to_string(),
                properties: properties(),
                ..Default::default()
            }),
        })))
        .unwrap();
    let object = doc.allocate_handle();
    doc.objects.insert(
        object,
        ObjectType::RegisteredClass(RegisteredClassObject {
            handle: object,
            owner: doc.header.named_objects_dict_handle,
            dxf_name: OBJECT_CLASS.0.to_string(),
            cpp_class_name: OBJECT_CLASS.1.to_string(),
            properties: properties(),
            ..Default::default()
        }),
    );
    (doc, entity, object)
}

/// The written drawing declares both classes and keeps both records.
fn assert_classes_written(version: DxfVersion, declared: bool) {
    let (doc, entity, object) = document(version, declared);
    let back = read(DwgWriter::write_to_vec(&doc).unwrap());
    for name in [ENTITY_CLASS.0, OBJECT_CLASS.0] {
        assert!(
            back.classes.contains(name),
            "{version:?} (declared: {declared}): class {name} is not in the written table"
        );
    }
    assert!(
        matches!(
            back.get_entity(entity),
            Some(EntityType::Extended(extended))
                if matches!(&extended.data, ExtendedEntityData::RegisteredClass(data)
                    if data.properties == properties())
        ),
        "{version:?}: entity lost"
    );
    assert!(
        matches!(
            back.objects.get(&object),
            Some(ObjectType::RegisteredClass(data)) if data.properties == properties()
        ),
        "{version:?}: object lost"
    );
}

#[test]
fn classes_of_registered_records_survive_a_legacy_save() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018] {
        assert_classes_written(version, true);
    }
}

#[test]
fn missing_classes_of_registered_records_are_declared() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1032] {
        assert_classes_written(version, false);
    }
}
