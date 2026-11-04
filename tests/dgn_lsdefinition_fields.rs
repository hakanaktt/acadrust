//! A DGN line-style definition whose fields after the UID do not fit the order
//! they are read in.
//!
//! A drawing was found whose `AcDbLSDefinition` keeps 11 bits after the UID:
//! `11011011011`. Read as B, BD, BD, BL and four B, the record ends inside the
//! BL: the read failed, the definition was kept as a raw record, and the
//! drawing opened in recovery.

use std::io::Cursor;

use acadrust::classes::DxfClass;
use acadrust::entities::{EntityType, Line};
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::dgn_linestyle::read_dgn_line_style_data;
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::objects::{DgnLineStyleData, DgnLineStyleObject, Dictionary, ObjectType};
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

const UID: [u8; 16] = [
    0x5a, 0x31, 0x0c, 0x77, 0x9e, 0x42, 0x4b, 0x18, 0x8d, 0x06, 0x63, 0xf1, 0x27, 0xb4, 0x90, 0x3e,
];
const NAME: &str = "{ Synthetic Line }";
/// The bits found after the UID.
const FOUND: &str = "11011011011";

const VERSIONS: [DxfVersion; 4] = [
    DxfVersion::AC1018,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

fn write_found(main: &mut DwgBitWriter) {
    for bit in FOUND.chars() {
        main.write_bit(bit == '1');
    }
}

/// The written order: B, BD, BD, BL, four B.
fn write_fields_in_order(main: &mut DwgBitWriter) {
    main.write_bit(true);
    main.write_bit_double(2.5);
    main.write_bit_double(1.0);
    main.write_bit_long(3);
    main.write_bit(false);
    main.write_bit(true);
    main.write_bit(false);
    main.write_bit(true);
}

/// A definition record body: name, version 1, style 0, the UID, then `tail`,
/// with a null root component in the handle stream.
fn definition_bits(tail: fn(&mut DwgBitWriter), dxf_version: DxfVersion) -> (DwgMergedReader, i64) {
    let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
    let mut main = DwgBitWriter::new(version, dxf_version);
    let mut text = DwgBitWriter::new(version, dxf_version);
    let mut handles = DwgBitWriter::new(version, dxf_version);
    let split_text = version.r2007_plus();
    if split_text {
        text.write_variable_text(NAME);
    } else {
        main.write_variable_text(NAME);
    }
    main.write_bit_long(1);
    main.write_bit_long(0);
    main.write_bytes(&UID);
    tail(&mut main);
    handles.write_handle(DwgReferenceType::HardPointer, 0);
    let end = main.position_in_bits();
    let mut reader = DwgMergedReader::from_readers(
        DwgBitReader::new(main.into_bytes(), version, dxf_version),
        split_text.then(|| DwgBitReader::new(text.into_bytes(), version, dxf_version)),
        Some(DwgBitReader::new(
            handles.into_bytes(),
            version,
            dxf_version,
        )),
        dxf_version,
    );
    reader.set_main_data_end(end);
    (reader, end)
}

/// A drawing holding one line and one LSDEFINITION stored as the raw record
/// `tail` gives, as a DWG read from a file keeps a record it could not decode.
fn drawing_with_definition(version: DxfVersion, tail: fn(&mut DwgBitWriter)) -> (Vec<u8>, Handle) {
    let mut document = CadDocument::with_version(version);
    document.dwg_source_version = Some(version);
    document.classes.clear();
    let mut class = DxfClass::new("LSDEFINITION", "AcDbLSDefinition");
    class.class_number = 543;
    document.classes.push_preserving(class);
    document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
    let owner = document.header.named_objects_dict_handle;
    let definition = document.allocate_handle();
    let extension = document.allocate_handle();
    let mut dictionary = Dictionary::new();
    dictionary.handle = extension;
    dictionary.owner = definition;
    document
        .objects
        .insert(extension, ObjectType::Dictionary(dictionary));

    let mut writer = DwgMergedWriter::new(DwgVersion::from_dxf_version(version).unwrap(), version);
    writer.write_object_type(543);
    if version < DxfVersion::AC1024 {
        writer.save_position_for_size();
    }
    writer.main_mut().write_handle_undefined(definition.value());
    writer.write_bit_short(0); // no EED
    writer.write_handle(DwgReferenceType::SoftPointer, owner.value());
    writer.write_bit_long(1);
    writer.write_handle(DwgReferenceType::SoftPointer, owner.value());
    if version >= DxfVersion::AC1018 {
        writer.write_bit(false);
    }
    writer.write_handle(DwgReferenceType::HardOwnership, extension.value());
    if version >= DxfVersion::AC1027 {
        writer.write_bit(false);
    }
    writer.write_variable_text(NAME);
    writer.write_bit_long(1);
    writer.write_bit_long(0);
    writer.write_bytes(&UID);
    tail(writer.main_mut());
    writer.write_handle(DwgReferenceType::HardPointer, 0);
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    document.objects.insert(
        definition,
        ObjectType::Unknown {
            type_name: "LSDEFINITION".to_string(),
            handle: definition,
            owner,
            raw_dxf_codes: None,
            raw_dwg_data: Some(raw),
            raw_dwg_handle_bits: handle_bits,
            raw_dwg_version: Some(version),
        },
    );
    if let Some(ObjectType::Dictionary(dictionary)) = document.objects.get_mut(&owner) {
        dictionary.add_entry("lsdefinition", definition);
    }
    (DwgWriter::write_to_vec(&document).unwrap(), definition)
}

/// Reads `bytes` strictly and returns the definition at `handle`, failing if
/// the read needed recovery.
fn read_definition(bytes: Vec<u8>, handle: Handle, version: DxfVersion) -> DgnLineStyleData {
    let outcome = DwgReader::from_stream(Cursor::new(bytes))
        .read_with_stats()
        .unwrap_or_else(|error| panic!("{version:?}: {error}"));
    assert!(
        !outcome.stats.recovered(),
        "{version:?}: the drawing needed recovery: {:?}",
        outcome.stats.diagnostics
    );
    match outcome.document.objects.get(&handle) {
        Some(ObjectType::DgnLineStyle(DgnLineStyleObject { data, .. })) => data.clone(),
        other => panic!("{version:?}: no line-style definition, read {other:?}"),
    }
}

#[test]
fn a_definition_with_the_found_bits_reads_whole() {
    for dxf_version in VERSIONS {
        let (mut reader, end) = definition_bits(write_found, dxf_version);
        let data = read_dgn_line_style_data(&mut reader, "LSDEFINITION").unwrap();
        reader
            .check_error()
            .unwrap_or_else(|error| panic!("{dxf_version:?}: {error}"));
        assert_eq!(reader.position_in_bits(), end, "{dxf_version:?}");
        let DgnLineStyleData::Definition {
            description,
            version,
            style_number,
            component_uid,
            ..
        } = data
        else {
            panic!("{dxf_version:?}: not a definition: {data:?}");
        };
        assert_eq!(description, NAME, "{dxf_version:?}");
        assert_eq!((version, style_number), (1, 0), "{dxf_version:?}");
        assert_eq!(component_uid, UID, "{dxf_version:?}");
    }
}

#[test]
fn a_drawing_with_the_found_bits_opens_without_recovery() {
    for version in VERSIONS {
        let (bytes, handle) = drawing_with_definition(version, write_found);
        let data = read_definition(bytes, handle, version);
        let DgnLineStyleData::Definition { description, .. } = data else {
            panic!("{version:?}: not a definition: {data:?}");
        };
        assert_eq!(description, NAME, "{version:?}");
    }
}

#[test]
fn the_found_bits_survive_dwg_save() {
    for version in VERSIONS {
        let (bytes, handle) = drawing_with_definition(version, write_found);
        let outcome = DwgReader::from_stream(Cursor::new(bytes))
            .read_with_stats()
            .unwrap();
        let first = match outcome.document.objects.get(&handle) {
            Some(ObjectType::DgnLineStyle(DgnLineStyleObject { data, .. })) => data.clone(),
            other => panic!("{version:?}: no line-style definition, read {other:?}"),
        };
        let saved = DwgWriter::write_to_vec(&outcome.document).unwrap();
        assert_eq!(
            read_definition(saved, handle, version),
            first,
            "{version:?}"
        );
    }
}

#[test]
fn fields_in_the_written_order_read_as_before() {
    for dxf_version in VERSIONS {
        let (mut reader, end) = definition_bits(write_fields_in_order, dxf_version);
        let data = read_dgn_line_style_data(&mut reader, "LSDEFINITION").unwrap();
        reader.check_error().unwrap();
        assert_eq!(reader.position_in_bits(), end, "{dxf_version:?}");
        let DgnLineStyleData::Definition {
            is_continuous,
            unit_definition,
            unit_scale,
            units_type,
            is_element,
            is_physical,
            is_scale_independent,
            is_snappable,
            ..
        } = data
        else {
            panic!("{dxf_version:?}: not a definition: {data:?}");
        };
        assert!(is_continuous, "{dxf_version:?}");
        assert_eq!(
            (unit_definition, unit_scale, units_type),
            (2.5, 1.0, 3),
            "{dxf_version:?}"
        );
        assert_eq!(
            (is_element, is_physical, is_scale_independent, is_snappable),
            (false, true, false, true),
            "{dxf_version:?}"
        );
    }
}
