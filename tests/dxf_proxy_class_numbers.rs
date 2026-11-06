//! A proxy read from DXF names its class in the document's class table.
//!
//! In DXF a proxy entity or object names its class by number (group 91):
//! 500 + the place of its `CLASS` record in the file's CLASSES section. The
//! reader merges the file's classes into the default table by name, so a
//! file class sits at another place in the document than in the file. The
//! proxy's number has to follow its class.

use acadrust::{
    entities::{EntityType, ExtendedEntityData},
    objects::ObjectType,
    CadDocument, DxfReader, DxfWriter,
};
use std::io::Cursor;

/// An AC1018 DXF whose CLASSES section holds `classes` (DXF name, C++
/// name, entity flag), with a proxy entity and a proxy object numbered
/// `class_id`.
fn dxf(classes: &[(&str, &str, bool)], class_id: i32) -> Vec<u8> {
    let mut pairs: Vec<(i32, String)> = vec![
        (0, "SECTION".into()),
        (2, "HEADER".into()),
        (9, "$ACADVER".into()),
        (1, "AC1018".into()),
        (9, "$HANDSEED".into()),
        (5, "400".into()),
        (0, "ENDSEC".into()),
        (0, "SECTION".into()),
        (2, "CLASSES".into()),
    ];
    for (name, cpp, entity) in classes {
        pairs.extend([
            (0, "CLASS".into()),
            (1, (*name).into()),
            (2, (*cpp).into()),
            (3, "nanoCAD".into()),
            (90, "0".into()),
            (91, "1".into()),
            (280, "0".into()),
            (281, if *entity { "1" } else { "0" }.into()),
        ]);
    }
    let class_id = class_id.to_string();
    pairs.extend([
        (0, "ENDSEC".into()),
        (0, "SECTION".into()),
        (2, "ENTITIES".into()),
        (0, "ACAD_PROXY_ENTITY".into()),
        (5, "1F0".into()),
        (330, "1F".into()),
        (100, "AcDbEntity".into()),
        (8, "0".into()),
        (100, "AcDbProxyEntity".into()),
        (90, "498".into()),
        (91, class_id.clone()),
        (95, "25".into()),
        (70, "0".into()),
        (92, "8".into()),
        (310, "0800000000000000".into()),
        (93, "3".into()),
        (310, "A0".into()),
        (94, "0".into()),
        (0, "ENDSEC".into()),
        (0, "SECTION".into()),
        (2, "OBJECTS".into()),
        (0, "ACAD_PROXY_OBJECT".into()),
        (5, "1F1".into()),
        (330, "C".into()),
        (100, "AcDbProxyObject".into()),
        (90, "499".into()),
        (91, class_id),
        (95, "25".into()),
        (70, "0".into()),
        (93, "0".into()),
        (0, "ENDSEC".into()),
        (0, "EOF".into()),
    ]);
    pairs
        .iter()
        .map(|(code, value)| format!("{code}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

fn read(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

/// Class numbers of the proxy entity and the proxy object.
fn proxy_class_ids(document: &CadDocument) -> (i32, i32) {
    let entity = document
        .entities()
        .find_map(|entity| match entity {
            EntityType::Extended(extended) => match &extended.data {
                ExtendedEntityData::Proxy(proxy) => Some(proxy.class_id),
                _ => None,
            },
            _ => None,
        })
        .expect("proxy entity read");
    let object = document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::ProxyObject(proxy) => Some(proxy.class_id),
            _ => None,
        })
        .expect("proxy object read");
    (entity, object)
}

/// The DXF name of the class numbered `number` in the document's table.
fn class_name(document: &CadDocument, number: i32) -> Option<&str> {
    document
        .classes
        .iter()
        .find(|class| i32::from(class.class_number) == number)
        .map(|class| class.dxf_name.as_str())
}

fn assert_proxies_name(document: &CadDocument, expected: &str) {
    let (entity, object) = proxy_class_ids(document);
    assert_eq!(class_name(document, entity), Some(expected), "proxy entity");
    assert_eq!(class_name(document, object), Some(expected), "proxy object");
}

const NOTE: (&str, &str, bool) = ("SPDSNOTE", "mcsDbObjectNote", true);
const DICTIONARY: (&str, &str, bool) = ("ACDBDICTIONARYWDFLT", "AcDbDictionaryWithDefault", false);

#[test]
fn proxy_names_its_file_class() {
    // SPDSNOTE is the file's first class (500); the defaults hold another
    // class at 500, and SPDSNOTE is appended after them.
    assert_proxies_name(&read(dxf(&[NOTE], 500)), "SPDSNOTE");
}

#[test]
fn proxy_of_a_default_class_names_it_at_its_default_place() {
    // The file's second class (501) is one the defaults know: it stays at
    // its default place.
    assert_proxies_name(&read(dxf(&[NOTE, DICTIONARY], 501)), "ACDBDICTIONARYWDFLT");
}

#[test]
fn proxy_keeps_its_class_through_a_dxf_round_trip() {
    let document = read(dxf(&[NOTE], 500));
    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    assert_proxies_name(&read(bytes), "SPDSNOTE");
}

#[test]
fn number_naming_no_file_class_names_no_class() {
    // 501 is past the file's only class; with no CLASSES section 500 names
    // nothing either.
    assert_eq!(proxy_class_ids(&read(dxf(&[NOTE], 501))), (0, 0));
    let text = String::from_utf8(dxf(&[], 500)).unwrap();
    let text = text.replace("0\nSECTION\n2\nCLASSES\n0\nENDSEC\n", "");
    assert_eq!(proxy_class_ids(&read(text.into_bytes())), (0, 0));
}

#[test]
fn number_below_the_first_class_is_kept() {
    // Control: below 500 a number names no class and is left as read.
    assert_eq!(proxy_class_ids(&read(dxf(&[NOTE], 0))), (0, 0));
}
