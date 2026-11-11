//! DWG record diagnostics name the record type, as the DXF reader does,
//! instead of carrying the reader's internal type code.

use std::io::Cursor;

use acadrust::classes::DxfClass;
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::{DwgReadOptions, DwgVersion};
use acadrust::io::read::ReadDiagnostic;
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

const VERSION: DxfVersion = DxfVersion::AC1032;

/// Classes of the drawing: (dxf name, class number, is an entity).
const CLASSES: [(&str, i16, bool); 3] = [
    ("ACAD_TABLE", 600, true),
    ("ACIDBLOCKREFERENCE", 601, true),
    ("CONTEXTDATAMANAGER", 602, false),
];

/// A record of `type_code` with object common data and no body, which every
/// decoder below rejects.
fn raw_record(document: &mut CadDocument, type_code: i16) -> Handle {
    let handle = document.allocate_handle();
    let mut writer = DwgMergedWriter::new(DwgVersion::from_dxf_version(VERSION).unwrap(), VERSION);
    writer.write_object_type(type_code);
    writer.main_mut().write_handle_undefined(handle.value());
    writer.write_bit_short(0); // no EED
    writer.write_handle(DwgReferenceType::SoftPointer, 0);
    writer.write_bit_long(0); // no reactors
    writer.write_bit(true); // no extension dictionary
    writer.write_bit(false); // no data-store data
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    document.objects.insert(
        handle,
        ObjectType::Unknown {
            type_name: format!("TYPE_{type_code}"),
            handle,
            owner: Handle::NULL,
            raw_dxf_codes: None,
            raw_dwg_data: Some(raw),
            raw_dwg_handle_bits: handle_bits,
            raw_dwg_version: Some(VERSION),
        },
    );
    handle
}

/// The failsafe read diagnostics of a drawing with one undecodable record per
/// `type_code`, and the handle of each record.
fn read_records(type_codes: &[i16]) -> (Vec<ReadDiagnostic>, Vec<Handle>) {
    let mut document = CadDocument::with_version(VERSION);
    document.dwg_source_version = Some(VERSION);
    document.classes.clear();
    for (name, number, entity) in CLASSES {
        let mut class = DxfClass::new(name, "AcDbTestRecord");
        class.class_number = number;
        class.is_an_entity = entity;
        class.item_class_id = if entity { 0x1F2 } else { 0x1F3 };
        document.classes.push_preserving(class);
    }
    let handles = type_codes
        .iter()
        .map(|&type_code| raw_record(&mut document, type_code))
        .collect();
    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let outcome =
        DwgReader::from_stream_with_options(Cursor::new(bytes), DwgReadOptions::failsafe())
            .read_with_stats()
            .unwrap();
    (outcome.stats.diagnostics, handles)
}

fn record_type(diagnostics: &[ReadDiagnostic], handle: Handle) -> Option<&str> {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.record_handle == Some(handle.value()))
        .unwrap_or_else(|| panic!("no diagnostic for {handle:?}: {diagnostics:?}"))
        .record_type
        .as_deref()
}

#[test]
fn fixed_type_records_are_named() {
    // 3DSOLID (38), DICTIONARY (42) and the LAYER table record (51).
    let (diagnostics, handles) = read_records(&[38, 42, 51]);
    let names: Vec<_> = handles
        .into_iter()
        .map(|handle| record_type(&diagnostics, handle))
        .collect();
    assert_eq!(names, [Some("3DSOLID"), Some("DICTIONARY"), Some("LAYER")]);
}

#[test]
fn class_records_are_named_by_their_class() {
    // ACAD_TABLE resolves to an internal negative code, ACIDBLOCKREFERENCE to
    // INSERT; CONTEXTDATAMANAGER has no internal code and is preserved raw.
    let (diagnostics, handles) = read_records(&[600, 601, 602]);
    let names: Vec<_> = handles
        .into_iter()
        .map(|handle| record_type(&diagnostics, handle))
        .collect();
    assert_eq!(
        names,
        [
            Some("ACAD_TABLE"),
            Some("ACIDBLOCKREFERENCE"),
            Some("CONTEXTDATAMANAGER")
        ]
    );
}
