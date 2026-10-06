use acadrust::objects::{Dictionary, Material, ObjectType, XRecord, XRecordEntry};
use acadrust::{CadDocument, Handle};

fn checker_record(name: &str, handle: Handle, owner: Handle) -> XRecord {
    let mut record = XRecord::named(name);
    record.handle = handle;
    record.owner = owner;
    record.entries = vec![
        XRecordEntry::string(300, "Container"),
        XRecordEntry::string(300, "Shader"),
        XRecordEntry::string(301, "Checker"),
        XRecordEntry::string(300, "Map1"),
        XRecordEntry::string(300, "Color"),
        XRecordEntry::int32(420, 0x102030),
        XRecordEntry::string(300, "Map2"),
        XRecordEntry::string(300, "Color"),
        XRecordEntry::int32(420, 0xD0E0F0),
        XRecordEntry::string(300, "Mapper"),
    ];
    record
}

#[test]
fn resolves_checker_asset_records_into_material_maps() {
    let material_handle = Handle::new(0x10);
    let dictionary_handle = Handle::new(0x20);
    let diffuse_handle = Handle::new(0x21);
    let opacity_handle = Handle::new(0x22);
    let mut document = CadDocument::new();

    let mut material = Material::new();
    material.handle = material_handle;
    material.xdictionary_handle = Some(dictionary_handle);

    let mut dictionary = Dictionary::new();
    dictionary.handle = dictionary_handle;
    dictionary.add_entry("DIFFUSE", diffuse_handle);
    dictionary.add_entry("OPACITY", opacity_handle);

    document
        .objects
        .insert(material_handle, ObjectType::Material(material));
    document
        .objects
        .insert(dictionary_handle, ObjectType::Dictionary(dictionary));
    document.objects.insert(
        diffuse_handle,
        ObjectType::XRecord(checker_record("DIFFUSE", diffuse_handle, dictionary_handle)),
    );
    document.objects.insert(
        opacity_handle,
        ObjectType::XRecord(checker_record("OPACITY", opacity_handle, dictionary_handle)),
    );

    document.resolve_xrecord_backed_properties();

    let ObjectType::Material(material) = document
        .objects
        .get(&material_handle)
        .expect("material object")
    else {
        panic!("expected material");
    };
    for map in [&material.diffuse_map, &material.opacity_map] {
        assert_eq!(map.source, 2);
        assert!(map.file_name.is_empty());
        let texture = map.texture.as_ref().expect("checker texture");
        assert_eq!(texture.color1.rgb, Some(0x102030));
        assert_eq!(texture.color2.rgb, Some(0xD0E0F0));
    }
}
