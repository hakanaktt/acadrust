//! R13 and R14 proxy records as ODA writes them.
//!
//! From R2000 a proxy record starts with its class number (DXF 91), the data
//! format (DXF 95) and the origin bit (DXF 70). In R13 and R14 it starts with
//! two BL, the record type (DXF 90: 498 for an entity, 499 for an object) and
//! the class number, and the data follows (ODA specification 20.4.90 marks
//! the class number, the format and the bit R2000+). acadrust read and wrote
//! the R2000 fields in R13 and R14 as well: the class number came back as 498
//! or 499, and ODA read the data format of a written proxy as its class
//! ("No ClassId") and recovered the whole file.

use acadrust::{
    classes::DxfClassCollection,
    entities::{
        extended_entity::{ExtendedEntityData, ProxyEntityData},
        EntityType,
    },
    objects::{ObjectType, ProxyObject, ProxyPayload},
    types::DxfVersion,
    CadDocument, DwgReader, DwgWriter, Handle,
};
use std::io::Cursor;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/proxy_r13_r14");

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn fixture(name: &str) -> CadDocument {
    read(std::fs::read(format!("{FIXTURES}/{name}")).unwrap())
}

/// Name of the class with number `class_id` in `classes`.
fn class_name(classes: &DxfClassCollection, class_id: i32) -> Option<String> {
    classes
        .iter()
        .find(|class| i32::from(class.class_number) == class_id)
        .map(|class| class.dxf_name.clone())
}

fn proxy_entity(doc: &CadDocument, handle: u64) -> ProxyEntityData {
    match doc.get_entity(Handle::new(handle)) {
        Some(EntityType::Extended(extended)) => match &extended.data {
            ExtendedEntityData::Proxy(proxy) => proxy.clone(),
            other => panic!("{handle:X}: expected a proxy entity, read {other:?}"),
        },
        other => panic!("{handle:X}: expected a proxy entity, read {other:?}"),
    }
}

fn proxy_object(doc: &CadDocument, handle: u64) -> ProxyObject {
    match doc.objects.get(&Handle::new(handle)) {
        Some(ObjectType::ProxyObject(proxy)) => proxy.clone(),
        other => panic!("{handle:X}: expected a proxy object, read {other:?}"),
    }
}

fn data(payload: &ProxyPayload) -> (u32, Vec<u8>) {
    (payload.bit_count, payload.data().to_vec())
}

#[test]
fn oda_r13_proxies_name_their_class() {
    let doc = fixture("oda_r13.dwg");
    for handle in [0x32, 0x34] {
        let proxy = proxy_entity(&doc, handle);
        assert_eq!(
            class_name(&doc.classes, proxy.class_id).as_deref(),
            Some("ACAD_PROXY_ENTITY_WRAPPER"),
            "entity {handle:X}: class_id {}",
            proxy.class_id
        );
    }
    let object = proxy_object(&doc, 0x33);
    assert_eq!(
        class_name(&doc.classes, object.class_id).as_deref(),
        Some("ACAD_PROXY_OBJECT_WRAPPER"),
        "object: class_id {}",
        object.class_id
    );
    assert_eq!(
        data(&proxy_entity(&doc, 0x32).payload),
        (24, vec![0xA5, 0x5A, 0xC3])
    );
    assert_eq!(data(&object.payload), (24, vec![0x3C, 0xC3, 0x99]));
}

#[test]
fn oda_r14_proxies_name_their_class() {
    let doc = fixture("oda_r14.dwg");
    let rtext = proxy_entity(&doc, 0x34);
    assert_eq!(
        class_name(&doc.classes, rtext.class_id).as_deref(),
        Some("ACAD_PROXY_ENTITY_WRAPPER"),
        "entity: class_id {}",
        rtext.class_id
    );
    assert_eq!(rtext.payload.bit_count, 260);
    assert_eq!(rtext.object_ids.len(), 1);
    let object = proxy_object(&doc, 0x33);
    assert_eq!(
        class_name(&doc.classes, object.class_id).as_deref(),
        Some("ACAD_PROXY_OBJECT_WRAPPER"),
        "object: class_id {}",
        object.class_id
    );
    assert_eq!(data(&object.payload), (24, vec![0x3C, 0xC3, 0x99]));
}

#[test]
fn r13_proxies_keep_their_class_through_a_save() {
    // The same version: the table is written as it is, so the numbers stay.
    for (name, version) in [
        ("oda_r13.dwg", DxfVersion::AC1012),
        ("oda_r14.dwg", DxfVersion::AC1014),
    ] {
        let doc = fixture(name);
        assert_eq!(doc.version, version, "{name}");
        let back = read(DwgWriter::write_to_vec(&doc).unwrap());
        let object = proxy_object(&back, 0x33);
        assert_eq!(
            class_name(&back.classes, object.class_id).as_deref(),
            Some("ACAD_PROXY_OBJECT_WRAPPER"),
            "{name}: object class_id {}",
            object.class_id
        );
        assert_eq!(
            data(&object.payload),
            (24, vec![0x3C, 0xC3, 0x99]),
            "{name}"
        );
    }
}
