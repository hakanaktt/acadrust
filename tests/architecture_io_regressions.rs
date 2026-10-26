use acadrust::classes::arch::{ArchEntityData, ArchObject};
use acadrust::classes::DxfClass;
use acadrust::entities::{EntityCommon, EntityType, ExtendedEntity, ExtendedEntityData};
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::{DwgReferenceType, DwgVersion};
use acadrust::objects::{Dictionary, ObjectType};
use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion, Handle, Transparency};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

// A small opaque class body exercises the actual DWG streams without a local
// ACAD installation or an ignored sample drawing.
fn drawing() -> (CadDocument, Handle) {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    let class = DxfClass::new_entity("AEC_WALL", "AecDbWall");
    doc.classes.add_or_update(class);
    let class_number = doc.classes.get_by_name("AEC_WALL").unwrap().class_number;
    let original = Handle::from(0x1000);
    let target = doc.header.model_space_block_handle;
    let mut writer = DwgMergedWriter::new(
        DwgVersion::from_dxf_version(doc.version).unwrap(),
        doc.version,
    );
    writer.write_object_type(class_number);
    writer.main_mut().write_handle_undefined(original.value());
    writer.write_bit_short(0); // EED terminator.
    writer.write_bit(false); // No proxy graphics.
    writer.write_2bits(2); // Model space.
    writer.write_bit_long(0); // Reactors.
    writer.write_bit(true); // No extension dictionary.
    writer.write_bit(false); // No AcDs payload.
    writer.write_en_color(&Color::ByLayer, &Transparency::default());
    writer.write_bit_double(1.0);
    writer.write_handle(
        DwgReferenceType::HardPointer,
        doc.layers.get("0").unwrap().handle.value(),
    );
    writer.write_2bits(0); // By-layer linetype.
    writer.write_2bits(0); // By-layer material.
    writer.write_byte(0); // Shadow flags.
    writer.write_2bits(0); // By-layer plot style.
    for _ in 0..3 {
        writer.write_bit(false); // Visual-style overrides.
    }
    writer.write_bit_short(0); // Visible.
    writer.write_byte(29); // Default lineweight.
    writer.write_raw_long(0x12345678);
    writer.write_bit_double(123.75);
    writer.write_variable_text("opaque AEC state");
    writer.write_handle_relative(original.value(), target.value());
    writer.write_handle(DwgReferenceType::HardPointer, target.value());
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    let mut data = ArchEntityData::from_parts("AEC_WALL", "AecDbWall", None);
    data.raw_dwg_data = Some(raw);
    data.dwg_handle_bits = handle_bits;
    data.dwg_type_code = class_number;
    data.dwg_source_version = Some(doc.version);
    let mut common = EntityCommon::default();
    common.handle = original;
    let handle = doc
        .add_entity(EntityType::Extended(Box::new(ExtendedEntity {
            common,
            data: ExtendedEntityData::Arch(data),
        })))
        .unwrap();
    (doc, handle)
}

fn roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn framed_record(raw: &[u8], handle_bits: i64) -> Vec<u8> {
    assert!(raw.len() < 0x8000);
    let mut framed = (raw.len() as u16).to_le_bytes().to_vec();
    let mut remaining = handle_bits as u64;
    loop {
        let byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        framed.push(byte | if remaining != 0 { 0x80 } else { 0 });
        if remaining == 0 {
            break;
        }
    }
    framed.extend_from_slice(raw);
    framed
}

fn assert_class_body(doc: &CadDocument, handle: Handle) {
    let EntityType::Extended(entity) = doc.get_entity(handle).unwrap() else {
        panic!()
    };
    let ExtendedEntityData::Arch(data) = &entity.data else {
        panic!()
    };
    let raw = data.raw_dwg_data.as_ref().unwrap();
    let framed = framed_record(raw, data.dwg_handle_bits);
    let objects = DwgObjectReader::new(framed, doc.version, Default::default()).unwrap();
    let (type_code, mut reader) = objects.read_record_at(0).unwrap();
    let common = objects.read_common_entity_data(&mut reader, type_code);
    assert_eq!(common.common.handle, handle.value());
    assert_eq!(reader.read_raw_long(), 0x12345678);
    assert_eq!(reader.read_bit_double(), 123.75);
    assert_eq!(reader.read_variable_text(), "opaque AEC state");
    assert_eq!(
        reader.read_handle(),
        doc.header.model_space_block_handle.value()
    );
    let (target, kind) = reader.read_typed_handle();
    assert_eq!(target, doc.header.model_space_block_handle.value());
    assert_eq!(kind, DwgReferenceType::HardPointer);
    reader.check_error().unwrap();
}

fn object_drawing() -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    doc.classes
        .add_or_update(DxfClass::new("AEC_WALL_STYLE", "AecDbWallStyle"));
    let class_number = doc
        .classes
        .get_by_name("AEC_WALL_STYLE")
        .unwrap()
        .class_number;
    let handle = doc.allocate_handle();
    let xdictionary = doc.allocate_handle();
    let root = doc.header.named_objects_dict_handle;
    let mut dictionary = Dictionary::new();
    dictionary.handle = xdictionary;
    dictionary.owner = handle;
    doc.objects
        .insert(xdictionary, ObjectType::Dictionary(dictionary));
    let mut writer = DwgMergedWriter::new(DwgVersion::AC24, doc.version);
    writer.write_object_type(class_number);
    writer.main_mut().write_handle_undefined(handle.value());
    writer.write_bit_short(0);
    writer.write_handle(DwgReferenceType::SoftPointer, root.value());
    writer.write_bit_long(1);
    writer.write_handle(DwgReferenceType::SoftPointer, root.value());
    writer.write_bit(false);
    writer.write_handle(DwgReferenceType::HardOwnership, xdictionary.value());
    writer.write_bit(false);
    writer.write_raw_long(0x12345678);
    writer.write_bit_double(123.75);
    writer.write_variable_text("opaque AEC object state");
    writer.write_handle_relative(handle.value(), root.value());
    writer.write_handle(DwgReferenceType::HardPointer, root.value());
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    let mut object = ArchObject::new("AEC_WALL_STYLE", handle, root);
    object.reactors.push(root);
    object.xdictionary_handle = Some(xdictionary);
    object.raw_dwg_data = Some(raw);
    object.raw_dwg_handle_bits = handle_bits;
    object.raw_dwg_version = Some(doc.version);
    doc.objects.insert(handle, ObjectType::Arch(object));
    let ObjectType::Dictionary(dictionary) = doc.objects.get_mut(&root).unwrap() else {
        panic!()
    };
    dictionary.add_entry("AEC test style", handle);
    (doc, handle, xdictionary)
}

fn assert_object_body(doc: &CadDocument, handle: Handle, owner: Handle) {
    let ObjectType::Arch(object) = doc.objects.get(&handle).unwrap() else {
        panic!()
    };
    assert_eq!(object.handle, handle);
    assert_eq!(object.owner, owner);
    let raw = object.raw_dwg_data.as_ref().unwrap();
    let framed = framed_record(raw, object.raw_dwg_handle_bits);
    let objects = DwgObjectReader::new(framed, doc.version, Default::default()).unwrap();
    let (type_code, mut reader) = objects.read_record_at(0).unwrap();
    let common = objects.read_common_non_entity_data(&mut reader, type_code);
    assert_eq!(common.common.handle, handle.value());
    assert_eq!(common.owner_handle, owner.value());
    assert_eq!(reader.read_raw_long(), 0x12345678);
    assert_eq!(reader.read_bit_double(), 123.75);
    assert_eq!(reader.read_variable_text(), "opaque AEC object state");
    assert_eq!(
        reader.read_handle(),
        doc.header.named_objects_dict_handle.value()
    );
    let (target, kind) = reader.read_typed_handle();
    assert_eq!(target, doc.header.named_objects_dict_handle.value());
    assert_eq!(kind, DwgReferenceType::HardPointer);
    reader.check_error().unwrap();
}

#[test]
fn aec_common_edits_and_opaque_class_streams_survive_dwg_save() {
    let (doc, handle) = drawing();
    let mut doc = roundtrip(&doc);
    doc.layers.add(Layer::new("AEC edited layer")).unwrap();
    let common = doc.get_entity_mut(handle).unwrap().common_mut();
    common.invisible = true;
    common.layer = "AEC edited layer".into();
    common.color = Color::Index(3);
    let saved = roundtrip(&doc);
    let common = saved.get_entity(handle).unwrap().common();
    assert!(common.invisible);
    assert_eq!(common.layer, "AEC edited layer");
    assert_eq!(common.color, Color::Index(3));
    assert_class_body(&saved, handle);
}

#[test]
fn cloned_aec_entity_rewrites_its_handle_and_relative_class_references() {
    let (doc, original) = drawing();
    let mut doc = roundtrip(&doc);
    let mut entity = doc.get_entity(original).unwrap().clone();
    entity.common_mut().handle = Handle::NULL;
    let cloned = doc.add_entity(entity).unwrap();
    assert_ne!(cloned, original);
    let saved = roundtrip(&doc);
    assert_class_body(&saved, original);
    assert_class_body(&saved, cloned);
}

#[test]
fn aec_object_owner_and_common_edits_survive_dwg_save() {
    let (doc, handle, xdictionary) = object_drawing();
    let mut doc = roundtrip(&doc);
    let ObjectType::Arch(object) = doc.objects.get_mut(&handle).unwrap() else {
        panic!()
    };
    object.owner = xdictionary;
    object.reactors = vec![xdictionary];
    object.xdictionary_handle = None;
    let saved = roundtrip(&doc);
    assert_object_body(&saved, handle, xdictionary);
    let ObjectType::Arch(object) = saved.objects.get(&handle).unwrap() else {
        panic!()
    };
    assert_eq!(object.reactors, vec![xdictionary]);
    assert_eq!(object.xdictionary_handle, None);
    let mut doc = saved;
    let ObjectType::Arch(object) = doc.objects.get_mut(&handle).unwrap() else {
        panic!()
    };
    object.reactors.clear();
    let saved = roundtrip(&doc);
    let ObjectType::Arch(object) = saved.objects.get(&handle).unwrap() else {
        panic!()
    };
    assert!(object.reactors.is_empty());
}

#[test]
fn cloned_aec_object_rewrites_its_handle_and_relative_class_references() {
    let (doc, original, _) = object_drawing();
    let mut doc = roundtrip(&doc);
    let ObjectType::Arch(mut cloned) = doc.objects.get(&original).unwrap().clone() else {
        panic!()
    };
    let handle = doc.allocate_handle();
    let root = doc.header.named_objects_dict_handle;
    cloned.handle = handle;
    cloned.xdictionary_handle = None;
    doc.objects.insert(handle, ObjectType::Arch(cloned));
    let ObjectType::Dictionary(dictionary) = doc.objects.get_mut(&root).unwrap() else {
        panic!()
    };
    dictionary.add_entry("AEC cloned style", handle);
    let saved = roundtrip(&doc);
    assert_object_body(&saved, original, root);
    assert_object_body(&saved, handle, root);
}

#[test]
fn dxf_export_omits_an_arch_entity_without_graphics_and_keeps_the_rest() {
    let (mut doc, wall) = drawing();
    let line = doc
        .add_entity(acadrust::entities::EntityType::Line(
            acadrust::entities::Line::from_points(
                acadrust::types::Vector3::ZERO,
                acadrust::types::Vector3::new(10.0, 0.0, 0.0),
            ),
        ))
        .unwrap();
    for binary in [false, true] {
        let mut writer = DxfWriter::new(&doc);
        writer.set_binary(binary);
        let omitted = writer.omitted_architecture_records();
        assert_eq!(omitted.len(), 1);
        assert_eq!(omitted[0].handle, wall);
        assert_eq!(omitted[0].name, "AEC_WALL");
        let output = writer.write_to_vec().unwrap();
        assert!(!output.is_empty());
        let saved = DxfReader::from_reader(Cursor::new(output))
            .unwrap()
            .read()
            .unwrap();
        assert!(saved.get_entity(wall).is_none());
        assert!(saved.get_entity(line).is_some());
    }
    let mut output = Vec::new();
    let error = DxfWriter::new(&doc)
        .with_version(DxfVersion::AC1009)
        .write_to_writer(&mut output)
        .unwrap_err();
    assert!(error.to_string().contains("Arch entity AEC_WALL"));
    assert!(output.is_empty());
}

#[test]
fn dxf_export_writes_arch_proxy_graphics_as_a_proxy_entity() {
    let (mut doc, wall) = drawing();
    let EntityType::Extended(entity) = doc.get_entity_mut(wall).unwrap() else {
        panic!()
    };
    entity.common.graphic_data = Some(vec![1, 2, 3, 4, 5, 6, 7, 8]);
    let writer = DxfWriter::new(&doc);
    assert!(writer.omitted_architecture_records().is_empty());
    let bytes = writer.write_to_vec().unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("ACAD_PROXY_ENTITY"));
    let saved = DxfReader::from_reader(Cursor::new(text.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let EntityType::Extended(entity) = saved.get_entity(wall).unwrap() else {
        panic!("proxy entity was dropped")
    };
    let ExtendedEntityData::Proxy(data) = &entity.data else {
        panic!("expected a proxy entity, got {:?}", entity.data);
    };
    assert_eq!(data.graphics.data(), &[1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn dxf_export_omits_arch_objects_without_a_dxf_body() {
    let mut doc = CadDocument::new();
    let handle = Handle::from(0x1000);
    let object = ArchObject::new("AEC_WALL_STYLE", handle, Handle::NULL);
    doc.objects.insert(handle, ObjectType::Arch(object));
    let root = doc.header.named_objects_dict_handle;
    let ObjectType::Dictionary(dictionary) = doc.objects.get_mut(&root).unwrap() else {
        panic!()
    };
    dictionary.add_entry("AEC test style", handle);
    let writer = DxfWriter::new(&doc);
    let omitted = writer.omitted_architecture_records();
    assert_eq!(omitted.len(), 1);
    assert_eq!(omitted[0].name, "AEC_WALL_STYLE");
    let output = writer.write_to_vec().unwrap();
    assert!(!output.is_empty());
    let text = String::from_utf8(output.clone()).unwrap();
    assert!(!text.contains("AEC_WALL_STYLE"));
    let saved = DxfReader::from_reader(Cursor::new(output))
        .unwrap()
        .read()
        .unwrap();
    assert!(saved.objects.get(&handle).is_none());
    let ObjectType::Dictionary(dictionary) = saved.objects.get(&root).unwrap() else {
        panic!()
    };
    assert!(dictionary.get("AEC test style").is_none());
}

#[test]
fn dxf_export_drops_xdata_handles_that_point_at_omitted_arch_objects() {
    let mut doc = CadDocument::new();
    let handle = Handle::from(0x1000);
    doc.objects.insert(
        handle,
        ObjectType::Arch(ArchObject::new("AEC_WALL_STYLE", handle, Handle::NULL)),
    );
    let layer = doc.layers.get("0").unwrap().handle;
    let mut line = acadrust::entities::Line::from_points(
        acadrust::types::Vector3::ZERO,
        acadrust::types::Vector3::new(1.0, 0.0, 0.0),
    );
    let mut record = acadrust::xdata::ExtendedDataRecord::new("AECBASE");
    record
        .values
        .push(acadrust::xdata::XDataValue::Integer16(10000));
    record
        .values
        .push(acadrust::xdata::XDataValue::Handle(handle));
    record
        .values
        .push(acadrust::xdata::XDataValue::Handle(layer));
    line.common.extended_data.add_record(record);
    doc.add_entity(EntityType::Line(line)).unwrap();
    let text = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    let handles: Vec<_> = lines
        .windows(2)
        .filter(|pair| pair[0] == "1005")
        .map(|pair| pair[1])
        .collect();
    assert!(
        !handles.iter().any(|value| *value == "1000"),
        "omitted Arch handle was written: {handles:?}"
    );
    assert!(
        handles
            .iter()
            .any(|value| value.eq_ignore_ascii_case(&format!("{:X}", layer.value()))),
        "layer handle should stay, got {handles:?}"
    );
}

#[test]
fn rejected_r12_aec_export_does_not_truncate_existing_file() {
    let (doc, _) = drawing();
    let path = std::env::temp_dir().join(format!("acadrust-aec-export-{}.dxf", std::process::id()));
    std::fs::write(&path, b"existing drawing").unwrap();
    assert!(DxfWriter::new(&doc)
        .with_version(DxfVersion::AC1009)
        .write_to_file(&path)
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"existing drawing");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn native_aec_dxf_body_still_roundtrips() {
    let (mut doc, handle) = drawing();
    let EntityType::Extended(entity) = doc.get_entity_mut(handle).unwrap() else {
        panic!()
    };
    let ExtendedEntityData::Arch(data) = &mut entity.data else {
        panic!()
    };
    data.raw_dxf_codes = Some(vec![
        (1, "opaque AEC DXF state".into()),
        (40, "123.75".into()),
    ]);
    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    let saved = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    let EntityType::Extended(entity) = saved.get_entity(handle).unwrap() else {
        panic!()
    };
    let ExtendedEntityData::Arch(data) = &entity.data else {
        panic!()
    };
    assert!(data
        .raw_dxf_codes
        .as_ref()
        .unwrap()
        .contains(&(1, "opaque AEC DXF state".into())));

    let mut output = Vec::new();
    assert!(DxfWriter::new(&doc)
        .with_version(DxfVersion::AC1009)
        .write_to_writer(&mut output)
        .is_err());
    assert!(output.is_empty());
}

#[test]
fn malformed_aec_dwg_record_is_rejected_before_writing() {
    let (mut doc, handle) = drawing();
    let EntityType::Extended(entity) = doc.get_entity_mut(handle).unwrap() else {
        panic!()
    };
    let ExtendedEntityData::Arch(data) = &mut entity.data else {
        panic!()
    };
    data.raw_dwg_data = Some(vec![0]);
    let mut output = Cursor::new(Vec::new());
    assert!(DwgWriter::write_to_writer(&mut output, &doc).is_err());
    assert!(output.get_ref().is_empty());
}

#[test]
fn malformed_aec_object_is_rejected_before_writing() {
    let (mut doc, handle, _) = object_drawing();
    let ObjectType::Arch(object) = doc.objects.get_mut(&handle).unwrap() else {
        panic!()
    };
    object.raw_dwg_data = Some(vec![0]);
    let mut output = Cursor::new(Vec::new());
    assert!(DwgWriter::write_to_writer(&mut output, &doc).is_err());
    assert!(output.get_ref().is_empty());
}
