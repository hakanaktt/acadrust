use acadrust::{
    entities::{EntityType, Line},
    types::Vector3,
    CadDocument, DwgReader, DwgWriter,
};
use std::{io::Cursor, mem::size_of};

#[test]
fn entity_heavy_drawings_keep_non_entity_storage_bounded() {
    let mut source = CadDocument::new();
    for x in 0..5_000 {
        source
            .add_entity(EntityType::Line(Line::from_points(
                Vector3::new(f64::from(x), 0.0, 0.0),
                Vector3::new(f64::from(x), 1.0, 0.0),
            )))
            .unwrap();
    }
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let loaded = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    assert_eq!(loaded.model_space_entities().count(), 5_000);
    assert!(!loaded.objects.is_empty());
    // Empty hash buckets contain a full ObjectType value. A small object table
    // must not retain megabytes solely because the drawing has many lines.
    let object_storage_bytes =
        loaded.objects.capacity() * size_of::<acadrust::objects::ObjectType>();
    assert!(
        object_storage_bytes < 1_048_576,
        "{} objects reserve at least {object_storage_bytes} bytes across {} slots",
        loaded.objects.len(),
        loaded.objects.capacity(),
    );
}
