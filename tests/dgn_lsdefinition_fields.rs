//! The fields of a DGN line-style definition (`AcDbLSDefinition`) in the order
//! the file stores them (#372).
//!
//! After the version and the style number come the unit scale, the units type,
//! the continuous flag, the unit definition and four more flags; the 16-byte
//! component UID is last. The name is in the string stream from R2007 on, and
//! a TV between the flags and the UID in R2004 and older. The UID was read
//! right after the style number: it took the unit fields and the start of the
//! real UID, and the fields were read from its end.
//!
//! Bits that do not fit that order are kept as read in `undecoded_fields` and
//! written back unchanged.

use std::io::Cursor;

use acadrust::classes::DxfClass;
use acadrust::entities::{EntityType, Line};
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::dgn_linestyle::read_dgn_line_style_data;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::objects::{DgnLineStyleData, DgnLineStyleObject, Dictionary, ObjectType};
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

const UID: [u8; 16] = [
    0x5a, 0x31, 0x0c, 0x77, 0x9e, 0x42, 0x4b, 0x18, 0x8d, 0x06, 0x63, 0xf1, 0x27, 0xb4, 0x90, 0x3e,
];
const NAME: &str = "{ Synthetic Line }";
const ROOT: u64 = 0x2a;

const VERSIONS: [DxfVersion; 5] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// The definition the records below hold.
fn expected(root_component: Handle) -> DgnLineStyleData {
    DgnLineStyleData::Definition {
        description: NAME.to_string(),
        version: 1,
        style_number: 7,
        component_uid: UID,
        is_continuous: true,
        unit_definition: 1.0,
        unit_scale: 0.25,
        units_type: 3,
        is_element: false,
        is_physical: true,
        is_scale_independent: false,
        is_snappable: true,
        root_component,
        undecoded_fields: None,
        properties: Vec::new(),
    }
}

/// The definition's fields after the style number, as the file stores them.
fn file_fields(writer: &mut DwgMergedWriter) {
    writer.write_bit_double(0.25); // unit scale
    writer.write_bit_long(3); // units type
    writer.write_bit(true); // is continuous
    writer.write_bit_double(1.0); // unit definition
    writer.write_bit(false); // is element
    writer.write_bit(true); // is physical
    writer.write_bit(false); // is scale independent
    writer.write_bit(true); // is snappable
    writer.write_variable_text(NAME); // a TV here in R2004 and older
    writer.write_bytes(&UID);
}

/// The fields as the file stores them, then three bits no known layout has.
fn file_fields_and_more(writer: &mut DwgMergedWriter) {
    file_fields(writer);
    writer.write_bit(true);
    writer.write_bit(false);
    writer.write_bit(true);
}

/// The fields as the file stores them, with the record ending inside the UID.
fn short_uid(writer: &mut DwgMergedWriter) {
    writer.write_bit_double(0.25);
    writer.write_bit_long(3);
    writer.write_bit(true);
    writer.write_bit_double(1.0);
    for _ in 0..4 {
        writer.write_bit(false);
    }
    writer.write_variable_text(NAME);
    writer.write_bytes(&UID[..10]);
}

/// The body of a definition record: version 1, style 7, then `fields`, with
/// the root component in the handle stream. Returns a reader over it and the
/// bit where the main data ends.
fn definition_bits(
    fields: fn(&mut DwgMergedWriter),
    dxf_version: DxfVersion,
) -> (DwgMergedReader, i64) {
    let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
    let mut writer = DwgMergedWriter::new(version, dxf_version);
    writer.write_bit_long(1);
    writer.write_bit_long(7);
    fields(&mut writer);
    writer.write_handle(DwgReferenceType::HardPointer, ROOT);
    let end = writer.main().position_in_bits();
    let split_text = version.r2007_plus();
    let mut reader = DwgMergedReader::from_readers(
        DwgBitReader::new(writer.main().to_bytes_snapshot(), version, dxf_version),
        split_text
            .then(|| DwgBitReader::new(writer.text().to_bytes_snapshot(), version, dxf_version)),
        Some(DwgBitReader::new(
            writer.handle_writer().to_bytes_snapshot(),
            version,
            dxf_version,
        )),
        dxf_version,
    );
    reader.set_main_data_end(end);
    (reader, end)
}

/// A drawing holding one line and one LSDEFINITION stored as the raw record
/// `fields` gives, as a DWG read from a file keeps a record it could not
/// decode. Returns the drawing and the definition's handle.
fn drawing_with_definition(
    version: DxfVersion,
    fields: fn(&mut DwgMergedWriter),
) -> (Vec<u8>, Handle) {
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
    writer.write_bit_long(1);
    writer.write_bit_long(7);
    fields(&mut writer);
    // The root component; nothing here needs it to resolve.
    writer.write_handle(DwgReferenceType::HardPointer, ROOT);
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

/// The bits `fields` writes after the style number, as `undecoded_fields` keeps them.
fn bits_after_style_number(fields: fn(&mut DwgMergedWriter), dxf_version: DxfVersion) -> String {
    let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
    let mut writer = DwgMergedWriter::new(version, dxf_version);
    fields(&mut writer);
    let count = writer.main().position_in_bits() as usize;
    let bytes = writer.main().to_bytes_snapshot();
    (0..count)
        .map(|bit| {
            if bytes[bit / 8] & (0x80 >> (bit % 8)) != 0 {
                '1'
            } else {
                '0'
            }
        })
        .collect()
}

fn payload_bits(data: &DgnLineStyleData) -> Option<String> {
    let DgnLineStyleData::Definition {
        undecoded_fields, ..
    } = data
    else {
        panic!("not a definition: {data:?}");
    };
    undecoded_fields.as_ref().map(|payload| {
        let bytes = payload.data();
        (0..payload.bit_count as usize)
            .map(|bit| {
                if bytes[bit / 8] & (0x80 >> (bit % 8)) != 0 {
                    '1'
                } else {
                    '0'
                }
            })
            .collect()
    })
}

#[test]
fn a_definition_reads_in_the_file_layout() {
    for dxf_version in VERSIONS {
        let (mut reader, end) = definition_bits(file_fields, dxf_version);
        let data = read_dgn_line_style_data(&mut reader, "LSDEFINITION").unwrap();
        reader
            .check_error()
            .unwrap_or_else(|error| panic!("{dxf_version:?}: {error}"));
        assert_eq!(data, expected(Handle::from(ROOT)), "{dxf_version:?}");
        assert_eq!(reader.position_in_bits(), end, "{dxf_version:?}");
    }
}

#[test]
fn a_drawing_with_a_definition_opens_with_its_fields() {
    for version in VERSIONS {
        let (bytes, handle) = drawing_with_definition(version, file_fields);
        assert_eq!(
            read_definition(bytes, handle, version),
            expected(Handle::from(ROOT)),
            "{version:?}"
        );
    }
}

#[test]
fn a_definition_survives_dwg_save_in_the_file_layout() {
    for version in VERSIONS {
        let (bytes, handle) = drawing_with_definition(version, file_fields);
        let outcome = DwgReader::from_stream(Cursor::new(bytes))
            .read_with_stats()
            .unwrap();
        let saved = DwgWriter::write_to_vec(&outcome.document).unwrap();
        assert_eq!(
            read_definition(saved, handle, version),
            expected(Handle::from(ROOT)),
            "{version:?}"
        );
    }
}

#[test]
fn bits_that_do_not_fit_the_file_layout_are_kept_as_read() {
    for fields in [file_fields_and_more, short_uid] {
        for dxf_version in VERSIONS {
            let (mut reader, end) = definition_bits(fields, dxf_version);
            let data = read_dgn_line_style_data(&mut reader, "LSDEFINITION").unwrap();
            reader
                .check_error()
                .unwrap_or_else(|error| panic!("{dxf_version:?}: {error}"));
            assert_eq!(reader.position_in_bits(), end, "{dxf_version:?}");
            assert_eq!(
                payload_bits(&data),
                Some(bits_after_style_number(fields, dxf_version)),
                "{dxf_version:?}"
            );
            let DgnLineStyleData::Definition {
                version,
                style_number,
                component_uid,
                root_component,
                ..
            } = data
            else {
                unreachable!();
            };
            assert_eq!((version, style_number), (1, 7), "{dxf_version:?}");
            assert_eq!(component_uid, [0; 16], "{dxf_version:?}");
            assert_eq!(root_component, Handle::from(ROOT), "{dxf_version:?}");
        }
    }
}

#[test]
fn bits_that_do_not_fit_survive_dwg_save() {
    for fields in [file_fields_and_more, short_uid] {
        for version in VERSIONS {
            let (bytes, handle) = drawing_with_definition(version, fields);
            let first = read_definition(bytes.clone(), handle, version);
            assert_eq!(
                payload_bits(&first),
                Some(bits_after_style_number(fields, version)),
                "{version:?}"
            );
            let document = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
            let saved = DwgWriter::write_to_vec(&document).unwrap();
            assert_eq!(
                read_definition(saved, handle, version),
                first,
                "{version:?}"
            );
        }
    }
}
