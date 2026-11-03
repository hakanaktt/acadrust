//! Two DWG classes may share one DXF name (AEC drawings carry two
//! `AEC_LIST_DEF` entries). An Arch object must keep the class number its
//! record was read with; resolving it again by name gives every object the
//! same class, and AutoCAD refuses the file (ErrorStatus 53).

use acadrust::classes::arch::ArchObject;
use acadrust::classes::DxfClass;
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::{DwgReferenceType, DwgVersion};
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

const NAME: &str = "AEC_LIST_DEF";

fn add_class(doc: &mut CadDocument) -> i16 {
    let mut class = DxfClass::new(NAME, "AecDbListDefinition");
    class.class_number = 500 + doc.classes.len() as i16;
    let number = class.class_number;
    doc.classes.push_preserving(class);
    number
}

fn add_object(doc: &mut CadDocument, class_number: i16, marker: i32) -> Handle {
    let handle = doc.allocate_handle();
    let root = doc.header.named_objects_dict_handle;
    let mut writer = DwgMergedWriter::new(
        DwgVersion::from_dxf_version(doc.version).unwrap(),
        doc.version,
    );
    writer.write_object_type(class_number);
    writer.main_mut().write_handle_undefined(handle.value());
    writer.write_bit_short(0); // EED terminator.
    writer.write_handle(DwgReferenceType::SoftPointer, root.value());
    writer.write_bit_long(0); // No reactors.
    writer.write_bit(true); // No extension dictionary.
    writer.write_bit(false); // No DS data.
    writer.write_raw_long(marker);
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    let mut object = ArchObject::new(NAME, handle, root);
    object.raw_dwg_data = Some(raw);
    object.raw_dwg_handle_bits = handle_bits;
    object.raw_dwg_version = Some(doc.version);
    doc.objects.insert(handle, ObjectType::Arch(object));
    let ObjectType::Dictionary(dictionary) = doc.objects.get_mut(&root).unwrap() else {
        panic!()
    };
    dictionary.add_entry(format!("AEC list {marker}"), handle);
    handle
}

fn type_code(doc: &CadDocument, handle: Handle) -> i16 {
    let ObjectType::Arch(object) = doc.objects.get(&handle).unwrap() else {
        panic!("{handle:?} is not an Arch object")
    };
    let raw = object.raw_dwg_data.as_ref().unwrap();
    let objects = DwgObjectReader::new(Vec::new(), doc.version, Default::default()).unwrap();
    let (type_code, _) = objects
        .read_raw_record(raw, object.raw_dwg_handle_bits)
        .unwrap();
    type_code
}

#[test]
fn arch_objects_keep_their_class_when_two_classes_share_a_dxf_name() {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    let first = add_class(&mut doc);
    let second = add_class(&mut doc);
    let a = add_object(&mut doc, first, 1);
    let b = add_object(&mut doc, second, 2);

    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let saved = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

    let numbers: Vec<i16> = saved
        .classes
        .iter()
        .filter(|class| class.dxf_name == NAME)
        .map(|class| class.class_number)
        .collect();
    assert_eq!(numbers, vec![first, second]);
    assert_eq!(type_code(&saved, a), first, "object of the first class");
    assert_eq!(type_code(&saved, b), second, "object of the second class");
}
