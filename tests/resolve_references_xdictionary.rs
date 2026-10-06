use acadrust::entities::Line;
use acadrust::objects::{Dictionary, ObjectType};
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, EntityType};

#[test]
fn remaps_entity_xdictionary_reference_with_colliding_object() {
    let mut document = CadDocument::new();
    let colliding = Handle::new(0x100);

    let mut line = Line::from_points(Vector3::ZERO, Vector3::UNIT_X);
    line.common.handle = colliding;
    line.common.xdictionary_handle = Some(colliding);
    document.add_entity(EntityType::Line(line)).unwrap();

    let mut extension_dictionary = Dictionary::new();
    extension_dictionary.handle = colliding;
    extension_dictionary.owner = colliding;
    document
        .objects
        .insert(colliding, ObjectType::Dictionary(extension_dictionary));

    document.resolve_references();

    let line = document
        .entities()
        .find_map(|entity| match entity {
            EntityType::Line(line) if line.common.handle == colliding => Some(line),
            _ => None,
        })
        .unwrap();
    let remapped_dictionary = line.common.xdictionary_handle.unwrap();

    assert_ne!(remapped_dictionary, colliding);
    assert!(matches!(
        document.objects.get(&remapped_dictionary),
        Some(ObjectType::Dictionary(dictionary))
            if dictionary.handle == remapped_dictionary
    ));
}
