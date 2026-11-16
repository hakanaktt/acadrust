use std::io::Cursor;

use acadrust::entities::Line;
use acadrust::objects::{Dictionary, DictionaryWithDefault, ObjectType};
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter, EntityType};

#[test]
fn test_dictionary_collection_manipulation() {
    let mut dict = Dictionary::new();
    assert!(dict.is_empty());
    assert_eq!(dict.len(), 0);

    // insert
    assert_eq!(dict.insert("Layer1", Handle::new(0x10)), None);
    assert!(dict.contains_key("layer1"));
    assert!(dict.contains_key("LAYER1"));
    assert_eq!(dict.get("LAYER1"), Some(Handle::new(0x10)));
    assert_eq!(dict.len(), 1);

    // insert replacement returns old handle
    assert_eq!(
        dict.insert("LAYER1", Handle::new(0x20)),
        Some(Handle::new(0x10))
    );
    assert_eq!(dict.get("layer1"), Some(Handle::new(0x20)));
    assert_eq!(dict.len(), 1);

    // set alias
    assert_eq!(dict.set("Layer2", Handle::new(0x30)), None);
    assert_eq!(dict.len(), 2);

    // hard-owner entry tracking and synchronization
    dict.set_entry_hard_owner("Layer1", true);
    assert!(dict.is_entry_hard_owner("LAYER1"));

    // remove
    assert_eq!(dict.remove("LAYER1"), Some(Handle::new(0x20)));
    assert!(!dict.contains_key("Layer1"));
    assert!(!dict.is_entry_hard_owner("Layer1"));
    assert_eq!(dict.len(), 1);

    // remove_entry
    assert_eq!(
        dict.remove_entry("layer2"),
        Some(("Layer2".to_string(), Handle::new(0x30)))
    );
    assert!(dict.is_empty());

    // iterators
    dict.insert("Alpha", Handle::new(1));
    dict.insert("Beta", Handle::new(2));
    let keys: Vec<&str> = dict.keys().collect();
    assert_eq!(keys, vec!["Alpha", "Beta"]);
    let values: Vec<Handle> = dict.values().collect();
    assert_eq!(values, vec![Handle::new(1), Handle::new(2)]);
    let pairs: Vec<(&str, Handle)> = dict.iter().collect();
    assert_eq!(
        pairs,
        vec![("Alpha", Handle::new(1)), ("Beta", Handle::new(2))]
    );

    // clear
    dict.clear();
    assert!(dict.is_empty());
}

#[test]
fn test_dictionary_with_default_manipulation() {
    let mut dict = DictionaryWithDefault::new();
    let default_h = Handle::new(0x100);
    dict.set_default(default_h);
    assert_eq!(dict.default_handle, default_h);

    assert!(dict.is_empty());
    assert_eq!(dict.len(), 0);

    // get_or_default
    assert_eq!(dict.get("Style1"), None);
    assert_eq!(dict.get_or_default("Style1"), default_h);

    // insert and get
    assert_eq!(dict.insert("Style1", Handle::new(0x200)), None);
    assert!(dict.contains_key("style1"));
    assert_eq!(dict.get("STYLE1"), Some(Handle::new(0x200)));
    assert_eq!(dict.get_or_default("STYLE1"), Handle::new(0x200));

    // replacement
    assert_eq!(
        dict.insert("STYLE1", Handle::new(0x300)),
        Some(Handle::new(0x200))
    );
    assert_eq!(dict.get("style1"), Some(Handle::new(0x300)));

    // remove
    assert_eq!(dict.remove("style1"), Some(Handle::new(0x300)));
    assert_eq!(dict.get_or_default("Style1"), default_h);
}

#[test]
fn test_document_subdictionary_helpers() {
    let mut doc = CadDocument::new();

    // ensure_subdictionary
    let custom_h = doc.ensure_subdictionary("MY_CUSTOM_DICT");
    assert_ne!(custom_h, Handle::NULL);

    // get_subdictionary
    assert_eq!(doc.get_subdictionary("MY_CUSTOM_DICT"), Some(custom_h));
    assert_eq!(doc.get_subdictionary("my_custom_dict"), Some(custom_h));

    // ensure_subdictionary is idempotent
    assert_eq!(doc.ensure_subdictionary("my_custom_dict"), custom_h);

    // verify ownership
    let root_h = doc.header.named_objects_dict_handle;
    let ObjectType::Dictionary(sub_dict) = doc.objects.get(&custom_h).unwrap() else {
        panic!("expected Dictionary");
    };
    assert_eq!(sub_dict.owner, root_h);

    // remove_subdictionary
    let removed = doc.remove_subdictionary("my_custom_dict");
    assert_eq!(removed, Some(custom_h));
    assert_eq!(doc.get_subdictionary("MY_CUSTOM_DICT"), None);
    assert!(!doc.objects.contains_key(&custom_h));
}

#[test]
fn test_document_group_helpers_and_reactors() {
    let mut doc = CadDocument::new();

    // Add entities
    let line1 = doc
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::ZERO,
            Vector3::UNIT_X,
        )))
        .unwrap();
    let line2 = doc
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::UNIT_X,
            Vector3::UNIT_Y,
        )))
        .unwrap();

    // create_group
    let group_h = doc.create_group("SteelBeams", &[line1, line2]).unwrap();
    assert_ne!(group_h, Handle::NULL);

    // verify entity reactors contain group handle
    assert!(doc
        .get_entity(line1)
        .unwrap()
        .common()
        .reactors
        .contains(&group_h));
    assert!(doc
        .get_entity(line2)
        .unwrap()
        .common()
        .reactors
        .contains(&group_h));

    // verify group lookup
    let group = doc.get_group("steelbeams").expect("group should exist");
    assert_eq!(group.name, "SteelBeams");
    assert_eq!(group.entities, vec![line1, line2]);
    assert_eq!(group.owner, doc.header.acad_group_dict_handle);

    // verify get_group_by_handle
    assert_eq!(doc.get_group_by_handle(group_h).unwrap().name, "SteelBeams");

    // add_entity_to_group
    let line3 = doc
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::UNIT_Y,
            Vector3::ZERO,
        )))
        .unwrap();
    doc.add_entity_to_group("SteelBeams", line3).unwrap();
    assert!(doc.get_group("SteelBeams").unwrap().contains(line3));
    assert!(doc
        .get_entity(line3)
        .unwrap()
        .common()
        .reactors
        .contains(&group_h));

    // remove_entity_from_group
    doc.remove_entity_from_group("SteelBeams", line2).unwrap();
    assert!(!doc.get_group("SteelBeams").unwrap().contains(line2));
    assert!(!doc
        .get_entity(line2)
        .unwrap()
        .common()
        .reactors
        .contains(&group_h));

    // create_unnamed_group
    let unnamed_h = doc.create_unnamed_group(&[line1]).unwrap();
    let unnamed_group = doc.get_group_by_handle(unnamed_h).unwrap();
    assert!(unnamed_group.is_unnamed());
    assert!(unnamed_group.name.starts_with("*A"));

    // remove_group
    let removed = doc.remove_group("SteelBeams").unwrap();
    assert_eq!(removed.name, "SteelBeams");
    assert_eq!(doc.get_group("SteelBeams"), None);
    assert!(!doc.objects.contains_key(&group_h));
    // entity reactors for line1 and line3 no longer contain group_h
    assert!(!doc
        .get_entity(line1)
        .unwrap()
        .common()
        .reactors
        .contains(&group_h));
    assert!(!doc
        .get_entity(line3)
        .unwrap()
        .common()
        .reactors
        .contains(&group_h));
}

#[test]
fn test_group_and_subdictionary_dxf_roundtrip() {
    let mut doc = CadDocument::new();

    let line = doc
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::ZERO,
            Vector3::UNIT_X,
        )))
        .unwrap();

    let _group_h = doc.create_group("GroupRoundtrip", &[line]).unwrap();
    let _subdict_h = doc.ensure_subdictionary("SubdictRoundtrip");

    // Write to DXF
    let bytes = DxfWriter::new(&doc)
        .write_to_vec()
        .expect("DXF write failed");

    // Read back from DXF
    let read_doc = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed");

    // Verify sub-dictionary roundtrip
    let rt_subdict = read_doc.get_subdictionary("SubdictRoundtrip");
    assert!(
        rt_subdict.is_some(),
        "sub-dictionary should survive roundtrip"
    );

    // Verify group roundtrip
    let rt_group = read_doc.get_group("GroupRoundtrip");
    assert!(rt_group.is_some(), "group should survive roundtrip");
    let rt_group = rt_group.unwrap();
    assert_eq!(rt_group.name, "GroupRoundtrip");
    assert_eq!(rt_group.entities.len(), 1);
}
