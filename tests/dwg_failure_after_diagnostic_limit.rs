//! A record failure must stay in the read diagnostics, and name the strict read error,
//! when preserved auxiliary objects alone would fill the diagnostic limit.

use std::io::Cursor;

use acadrust::classes::DxfClass;
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::{DwgReadOptions, DwgVersion};
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, MAX_READ_DIAGNOSTICS};

const VERSION: DxfVersion = DxfVersion::AC1032;
const CLASS: &str = "LSSTROKEPATTERNCOMPONENT";
const CLASS_NUMBER: i16 = 544;

/// A raw object record of `CLASS` with framed common data and no class body.
/// `reactors` is the common reactor count; `i32::MAX` breaks the common data.
fn raw_object(document: &mut CadDocument, handle: Handle, reactors: i32) {
    let mut writer = DwgMergedWriter::new(DwgVersion::from_dxf_version(VERSION).unwrap(), VERSION);
    writer.write_object_type(CLASS_NUMBER);
    writer.main_mut().write_handle_undefined(handle.value());
    writer.write_bit_short(0); // no EED
    writer.write_handle(DwgReferenceType::SoftPointer, 0);
    writer.write_bit_long(reactors);
    writer.write_bit(true); // no extension dictionary
    writer.write_bit(false); // no data-store data
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    document.objects.insert(
        handle,
        ObjectType::Unknown {
            type_name: CLASS.into(),
            handle,
            owner: Handle::NULL,
            raw_dxf_codes: None,
            raw_dwg_data: Some(raw),
            raw_dwg_handle_bits: handle_bits,
            raw_dwg_version: Some(VERSION),
        },
    );
}

/// A drawing whose `MAX_READ_DIAGNOSTICS` auxiliary objects fail to decode but
/// are preserved, followed (by handle) by one object whose common data is broken.
fn drawing() -> (Vec<u8>, Handle) {
    let mut document = CadDocument::with_version(VERSION);
    document.dwg_source_version = Some(VERSION);
    document.classes.clear();
    let mut class = DxfClass::new(CLASS, "AcDbTestAuxiliary");
    class.class_number = CLASS_NUMBER;
    document.classes.push_preserving(class);
    for _ in 0..MAX_READ_DIAGNOSTICS {
        let handle = document.allocate_handle();
        raw_object(&mut document, handle, 0);
    }
    let broken = document.allocate_handle();
    raw_object(&mut document, broken, i32::MAX);
    (DwgWriter::write_to_vec(&document).unwrap(), broken)
}

#[test]
fn strict_error_names_the_failed_record_after_the_diagnostic_limit() {
    let (bytes, broken) = drawing();
    let error = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .unwrap_err()
        .to_string();
    assert!(error.contains(&format!("{:#X}", broken.value())), "{error}");
    assert!(!error.contains("safety limit"), "{error}");
}

#[test]
fn failsafe_diagnostics_keep_the_failed_record_after_the_diagnostic_limit() {
    let (bytes, broken) = drawing();
    let outcome =
        DwgReader::from_stream_with_options(Cursor::new(bytes), DwgReadOptions::failsafe())
            .read_with_stats()
            .unwrap();
    assert_eq!(outcome.stats.skipped_source_records, 1);
    let diagnostics = &outcome.stats.diagnostics;
    assert!(diagnostics.len() <= MAX_READ_DIAGNOSTICS);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "record-decode-failed"
                && diagnostic.record_handle == Some(broken.value())
        }),
        "the failed record is not reported"
    );
    // The preserved objects still use the rest of the budget.
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "auxiliary-object-preserved"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "diagnostic-limit-reached"));
}
