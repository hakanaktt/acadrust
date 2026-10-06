use acadrust::entities::{
    EntityCommon, EntityType, ExtendedEntity, ExtendedEntityData, SectionObjectData,
};
use acadrust::objects::{ClassObject, ClassObjectData, ObjectType, SectionManager};
use acadrust::types::{Color, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};
use std::io::Cursor;

#[test]
fn dxf_preserves_section_manager_handles() {
    let mut document = CadDocument::new();
    let handle = document.allocate_handle();
    let section = Handle::new(0x1234);
    let mut object = ClassObject::new(ClassObjectData::SectionManager(SectionManager {
        is_live: true,
        sections: vec![section],
    }));
    object.handle = handle;
    object.owner = document.header.named_objects_dict_handle;
    document
        .objects
        .insert(handle, ObjectType::ClassObject(object));

    let bytes = DxfWriter::new(&document).write_to_vec().expect("write DXF");
    let roundtripped = DxfReader::from_reader(Cursor::new(bytes))
        .expect("create DXF reader")
        .read()
        .expect("read DXF");
    let manager = roundtripped
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::ClassObject(object) => match &object.data {
                ClassObjectData::SectionManager(manager) => Some(manager),
                _ => None,
            },
            _ => None,
        })
        .expect("section manager should round-trip");

    assert!(manager.is_live);
    assert_eq!(manager.sections, vec![section]);
}

#[test]
fn dwg_preserves_section_object_entity() {
    let mut document = CadDocument::new();
    document
        .add_entity(EntityType::Extended(Box::new(ExtendedEntity {
            common: EntityCommon::new(),
            data: ExtendedEntityData::SectionObject(SectionObjectData {
                state: 1,
                flags: 3,
                name: "Section 1".to_string(),
                vertical_direction: Vector3::new(0.0, 0.0, 1.0),
                top_height: 2.0,
                bottom_height: 3.0,
                indicator_alpha: 63,
                indicator_color: Color::from_index(1),
                vertices: vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(5.0, 0.0, 0.0)],
                back_line_vertices: Vec::new(),
                settings_handle: Handle::new(0x1234),
            }),
        })))
        .expect("add section object");
    let bytes = DwgWriter::write_to_vec(&document).expect("write DWG");
    let roundtripped = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read DWG");
    let section = roundtripped.entities().find_map(|entity| match entity {
        EntityType::Extended(extended) => match &extended.data {
            ExtendedEntityData::SectionObject(section) => Some(section),
            _ => None,
        },
        _ => None,
    });

    assert!(section.is_some(), "section object should round-trip");
}
