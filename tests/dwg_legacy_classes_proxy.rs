//! Proxies keep their class through a pre-R2013 DWG save (#318).
//!
//! On a save below AC1027 the writer rebuilds the class table: it keeps its
//! legacy classes and the classes named by record types, drops the rest and
//! renumbers what remains. A proxy names its class by number (`class_id`,
//! DXF 91), and that number used to be written unchanged: it then pointed past
//! the table or at another class, and the R2004+ class name fallback wrote the
//! name of that other class. ODA refuses such a proxy ("No ClassId") and
//! recovers the whole file.

use acadrust::{
    classes::{DxfClass, DxfClassCollection},
    entities::{
        extended_entity::{ExtendedEntity, ExtendedEntityData, ProxyEntityData},
        EntityType,
    },
    objects::{ObjectType, ProxyObject, ProxyPayload},
    types::DxfVersion,
    CadDocument, DwgReader, DwgWriter, Handle,
};
use std::io::Cursor;

/// An application class the legacy writer profile doesn't keep.
const APPLICATION: (&str, &str) = ("SPDSNOTE", "mcsDbObjectNote");
/// An application object class the legacy writer profile doesn't keep.
const APPLICATION_OBJECT: (&str, &str) = ("SPDSDATACONTAINER", "mcsDbDataContainer");

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn number(classes: &DxfClassCollection, name: &str) -> i32 {
    i32::from(classes.get_by_name(name).unwrap().class_number)
}

/// Name of the class with number `class_id` in `classes`.
fn class_name(classes: &DxfClassCollection, class_id: i32) -> Option<String> {
    classes
        .iter()
        .find(|class| i32::from(class.class_number) == class_id)
        .map(|class| class.dxf_name.clone())
}

fn proxy_entity(class_id: i32) -> EntityType {
    EntityType::Extended(Box::new(ExtendedEntity {
        common: Default::default(),
        data: ExtendedEntityData::Proxy(ProxyEntityData {
            proxy_id: 498,
            class_id,
            dxf_subclass: String::new(),
            version: 0,
            dwg_version: 0,
            maintenance_version: 0,
            from_dxf: false,
            // An empty graphics metafile: size 8, no records.
            graphics: ProxyPayload::from_bytes(&[8, 0, 0, 0, 0, 0, 0, 0]),
            payload: ProxyPayload::default(),
            text_payload: ProxyPayload::default(),
            object_ids: Vec::new(),
        }),
    }))
}

fn read_proxy_entity(doc: &CadDocument, handle: Handle) -> ProxyEntityData {
    match doc.get_entity(handle) {
        Some(EntityType::Extended(extended)) => match &extended.data {
            ExtendedEntityData::Proxy(proxy) => proxy.clone(),
            other => panic!("expected a proxy entity, read {other:?}"),
        },
        other => panic!("expected a proxy entity, read {other:?}"),
    }
}

struct Fixture {
    doc: CadDocument,
    /// Proxy of a class outside the legacy profile.
    application: Handle,
    /// Proxy of a class the profile keeps, whose number shifts.
    wipeout: Handle,
    /// Proxy object of a class outside the legacy profile.
    object: Handle,
}

/// A drawing saved to `version` with proxies of three classes. The class
/// table starts with the two application classes, which the legacy profile
/// drops, so every class it keeps moves when the table is rebuilt.
fn fixture(version: DxfVersion) -> Fixture {
    let mut doc = CadDocument::with_version(version);
    let defaults: Vec<DxfClass> = doc.classes.iter().cloned().collect();
    let application = [
        DxfClass::new_entity(APPLICATION.0, APPLICATION.1),
        DxfClass::new(APPLICATION_OBJECT.0, APPLICATION_OBJECT.1),
    ];
    doc.classes.clear();
    for (place, mut class) in application.into_iter().chain(defaults).enumerate() {
        class.class_number = 500 + place as i16;
        doc.classes.push_preserving(class);
    }
    let application = doc
        .add_entity(proxy_entity(number(&doc.classes, APPLICATION.0)))
        .unwrap();
    let wipeout = doc
        .add_entity(proxy_entity(number(&doc.classes, "WIPEOUT")))
        .unwrap();
    let object = doc.allocate_handle();
    doc.objects.insert(
        object,
        ObjectType::ProxyObject(ProxyObject {
            handle: object,
            owner: doc.header.named_objects_dict_handle,
            proxy_id: 499,
            class_id: number(&doc.classes, APPLICATION_OBJECT.0),
            ..Default::default()
        }),
    );
    Fixture {
        doc,
        application,
        wipeout,
        object,
    }
}

const LEGACY: [DxfVersion; 2] = [DxfVersion::AC1015, DxfVersion::AC1018];

#[test]
fn proxy_entity_keeps_its_application_class() {
    for version in LEGACY {
        let fixture = fixture(version);
        let back = read(DwgWriter::write_to_vec(&fixture.doc).unwrap());
        let proxy = read_proxy_entity(&back, fixture.application);
        assert_eq!(
            class_name(&back.classes, proxy.class_id).as_deref(),
            Some(APPLICATION.0),
            "{version:?}: class_id {} names another class or none",
            proxy.class_id
        );
    }
}

#[test]
fn proxy_entity_follows_its_renumbered_class() {
    for version in LEGACY {
        let fixture = fixture(version);
        let back = read(DwgWriter::write_to_vec(&fixture.doc).unwrap());
        let proxy = read_proxy_entity(&back, fixture.wipeout);
        assert_eq!(
            class_name(&back.classes, proxy.class_id).as_deref(),
            Some("WIPEOUT"),
            "{version:?}: class_id {} names another class or none",
            proxy.class_id
        );
    }
}

#[test]
fn proxy_object_keeps_its_application_class() {
    for version in LEGACY {
        let fixture = fixture(version);
        let back = read(DwgWriter::write_to_vec(&fixture.doc).unwrap());
        let Some(ObjectType::ProxyObject(proxy)) = back.objects.get(&fixture.object) else {
            panic!("{version:?}: proxy object lost");
        };
        assert_eq!(
            class_name(&back.classes, proxy.class_id).as_deref(),
            Some(APPLICATION_OBJECT.0),
            "{version:?}: class_id {} names another class or none",
            proxy.class_id
        );
    }
}

#[test]
fn r2004_class_name_fallback_names_the_proxy_class() {
    // R2004+ proxies carry the class name in the text stream; an empty
    // `dxf_subclass` is filled in from the table by `class_id`.
    let fixture = fixture(DxfVersion::AC1018);
    let back = read(DwgWriter::write_to_vec(&fixture.doc).unwrap());
    let proxy = read_proxy_entity(&back, fixture.application);
    assert_eq!(proxy.dxf_subclass, APPLICATION.0);
}

#[test]
fn same_version_table_is_left_as_it_is() {
    // A table read from a file of the target version is written as it is:
    // the numbers of its proxies stay.
    let fixture = fixture(DxfVersion::AC1018);
    let source = read(DwgWriter::write_to_vec(&fixture.doc).unwrap());
    let before = read_proxy_entity(&source, fixture.application).class_id;
    let back = read(DwgWriter::write_to_vec(&source).unwrap());
    assert_eq!(
        read_proxy_entity(&back, fixture.application).class_id,
        before
    );
}
