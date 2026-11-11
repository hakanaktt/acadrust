//! Auxiliary object recovery must preserve source bytes without weakening bounds (#134).

use std::io::Cursor;

use acadrust::classes::DxfClass;
use acadrust::entities::{EntityType, Line};
use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::{DwgReadOptions, DwgVersion};
use acadrust::objects::{Dictionary, ObjectContextData, ObjectContextKind, ObjectType, Scale};
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfWriter};

const CONTEXT: &str = "ACDB_MLEADEROBJECTCONTEXTDATA_CLASS";

fn auxiliary_fixture(
    version: DxfVersion,
    name: &str,
    class_number: i16,
    bad_common: bool,
    wrong_identity: bool,
) -> (CadDocument, Handle, Handle, Vec<u8>, i64) {
    auxiliary_fixture_with_payload(
        version,
        name,
        class_number,
        bad_common,
        wrong_identity,
        |writer, scale| {
            if name == CONTEXT {
                writer.write_bit_short(4);
                writer.write_bit(true);
                if version >= DxfVersion::AC1024 {
                    writer.write_bit(true); // R2010+ has-file-to-xdict flag.
                }
                writer.write_handle(DwgReferenceType::HardPointer, scale.value());
                writer.write_bit_long(i32::MAX);
            }
        },
    )
}

fn auxiliary_fixture_with_payload(
    version: DxfVersion,
    name: &str,
    class_number: i16,
    bad_common: bool,
    wrong_identity: bool,
    payload: impl FnOnce(&mut DwgMergedWriter, Handle),
) -> (CadDocument, Handle, Handle, Vec<u8>, i64) {
    let mut document = CadDocument::with_version(version);
    document.dwg_source_version = Some(version);
    document.classes.clear();
    let mut class = DxfClass::new(name, "AcDbTestAuxiliary");
    class.class_number = class_number;
    if class_number >= 500 {
        document.classes.push_preserving(class);
    }
    let healthy = document
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
    let owner = document.header.named_objects_dict_handle;
    let auxiliary = document.allocate_handle();
    let extension = document.allocate_handle();
    let mut dictionary = Dictionary::new();
    dictionary.handle = extension;
    dictionary.owner = auxiliary;
    document
        .objects
        .insert(extension, ObjectType::Dictionary(dictionary));
    let scale_handle = document.allocate_handle();
    let mut scale = Scale::new("1:50", 1.0, 50.0);
    scale.handle = scale_handle;
    document
        .objects
        .insert(scale_handle, ObjectType::Scale(scale));
    let mut writer = DwgMergedWriter::new(DwgVersion::from_dxf_version(version).unwrap(), version);
    writer.write_object_type(class_number);
    if version < DxfVersion::AC1024 {
        writer.save_position_for_size();
    }
    writer
        .main_mut()
        .write_handle_undefined(auxiliary.value() + u64::from(wrong_identity));
    writer.write_bit_short(3);
    writer
        .main_mut()
        .write_handle_undefined(document.app_ids.get("ACAD").unwrap().handle.value());
    writer.write_bytes(&[70, 42, 0]); // EED integer16 (1070).
    writer.write_bit_short(0);
    writer.write_handle(DwgReferenceType::SoftPointer, owner.value());
    writer.write_bit_long(if bad_common { i32::MAX } else { 1 });
    writer.write_handle(DwgReferenceType::SoftPointer, owner.value());
    if version >= DxfVersion::AC1018 {
        writer.write_bit(false);
        writer.write_handle(DwgReferenceType::HardOwnership, extension.value());
    } else {
        writer.write_handle(DwgReferenceType::HardOwnership, extension.value());
    }
    if version >= DxfVersion::AC1027 {
        writer.write_bit(true);
    }
    payload(&mut writer, scale_handle);
    // Other classes have no body, forcing a bounded class-specific read failure.
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    document.objects.insert(
        auxiliary,
        ObjectType::Unknown {
            type_name: name.to_string(),
            handle: auxiliary,
            owner,
            raw_dxf_codes: None,
            raw_dwg_data: Some(raw.clone()),
            raw_dwg_handle_bits: handle_bits,
            raw_dwg_version: Some(version),
        },
    );
    if let Some(ObjectType::Dictionary(dictionary)) = document.objects.get_mut(&owner) {
        dictionary.add_entry("issue134", auxiliary);
    }
    (document, healthy, auxiliary, raw, handle_bits)
}

#[test]
fn named_auxiliary_failures_are_preserved_in_strict_and_failsafe_reads() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        for (name, class_number) in [
            (CONTEXT, 540),
            ("ACDBASSOCGEOMDEPENDENCY", 501),
            ("ASSOCGEOMDEPENDENCY", 541),
            ("ACDBASSOC2DCONSTRAINTGROUP", 861),
            ("ASSOC2DCONSTRAINTGROUP", 536),
            ("ACDBASSOCEDGEACTIONPARAM", 562),
            ("ASSOCEDGEACTIONPARAM", 542),
            ("LSDEFINITION", 543),
            ("LSSTROKEPATTERNCOMPONENT", 544),
            ("ACDBASSOCARRAYACTIONBODY", 545),
        ] {
            let (source, healthy, auxiliary, raw, handle_bits) =
                auxiliary_fixture(version, name, class_number, false, false);
            let bytes = DwgWriter::write_to_vec(&source).unwrap();
            for options in [DwgReadOptions::default(), DwgReadOptions::failsafe()] {
                let outcome =
                    DwgReader::from_stream_with_options(Cursor::new(bytes.clone()), options)
                        .read_with_stats()
                        .unwrap_or_else(|error| panic!("{version:?} {name}: {error}"));
                assert!(outcome.document.get_entity(healthy).is_some());
                assert_eq!(
                    outcome.stats.skipped_source_records, 0,
                    "{version:?} {name}"
                );
                assert!(
                    outcome.stats.recovered(),
                    "{version:?} {name}: {:?}",
                    outcome.stats.diagnostics
                );
                let diagnostics: Vec<_> = outcome
                    .stats
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.code == "auxiliary-object-preserved")
                    .collect();
                assert_eq!(diagnostics.len(), 1, "{version:?} {name}");
                let diagnostic = diagnostics[0];
                assert_eq!(diagnostic.record_handle, Some(auxiliary.value()));
                assert_eq!(diagnostic.record_type.as_deref(), Some(name));
                assert!(diagnostic.source_offset.is_some());
                assert!(diagnostic.message.contains(name));
                assert_eq!(
                    outcome.document.object_xdata[&auxiliary]
                        .get_record("ACAD")
                        .unwrap()
                        .values,
                    vec![acadrust::xdata::XDataValue::Integer16(42)]
                );
                if name == CONTEXT {
                    let scale = outcome.document.context_scales[&auxiliary];
                    assert!(matches!(
                        outcome.document.objects.get(&scale),
                        Some(ObjectType::Scale(_))
                    ));
                }
                match outcome.document.objects.get(&auxiliary).unwrap() {
                    ObjectType::Unknown {
                        type_name,
                        handle,
                        owner,
                        raw_dwg_data,
                        raw_dwg_handle_bits,
                        raw_dwg_version,
                        ..
                    } => {
                        assert_eq!(type_name, name);
                        assert_eq!(*handle, auxiliary);
                        assert_eq!(*owner, source.header.named_objects_dict_handle);
                        assert_eq!(raw_dwg_data.as_ref().unwrap(), &raw);
                        if version >= DxfVersion::AC1024 {
                            assert_eq!(*raw_dwg_handle_bits, handle_bits);
                        }
                        assert_eq!(*raw_dwg_version, Some(version));
                    }
                    object => panic!("fabricated typed object: {object:?}"),
                }
                let saved = DwgWriter::write_to_vec(&outcome.document).unwrap();
                let saved = DwgReader::from_stream(Cursor::new(saved)).read().unwrap();
                assert!(matches!(saved.objects.get(&auxiliary),
                    Some(ObjectType::Unknown { raw_dwg_data: Some(data), .. }) if data == &raw));
            }
        }
    }
}

#[test]
fn broken_common_metadata_and_wrong_handles_still_fail_strict_reads() {
    for name in [
        CONTEXT,
        "ACDBASSOCGEOMDEPENDENCY",
        "ACDBASSOC2DCONSTRAINTGROUP",
        "ASSOC2DCONSTRAINTGROUP",
        "ACDBASSOCEDGEACTIONPARAM",
        "LSDEFINITION",
        "LSSTROKEPATTERNCOMPONENT",
        "ACDBASSOCARRAYACTIONBODY",
    ] {
        for (bad_common, wrong_identity) in [(true, false), (false, true)] {
            let (source, healthy, auxiliary, ..) =
                auxiliary_fixture(DxfVersion::AC1032, name, 540, bad_common, wrong_identity);
            let bytes = DwgWriter::write_to_vec(&source).unwrap();
            assert!(DwgReader::from_stream(Cursor::new(bytes.clone()))
                .read()
                .is_err());
            let recovered =
                DwgReader::from_stream_with_options(Cursor::new(bytes), DwgReadOptions::failsafe())
                    .read_with_stats()
                    .unwrap();
            assert!(recovered.document.get_entity(healthy).is_some());
            assert!(!recovered.document.objects.contains_key(&auxiliary));
            assert_eq!(recovered.stats.skipped_source_records, 1);
            assert!(recovered
                .stats
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "auxiliary-object-preserved"));
        }
    }
}

#[test]
fn failsafe_keeps_a_framed_object_and_returns_the_decode_error() {
    let (source, healthy, auxiliary, ..) =
        auxiliary_fixture(DxfVersion::AC1032, "CONTEXTDATAMANAGER", 540, false, false);
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let strict = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        DwgReader::from_stream(Cursor::new(bytes.clone())).read()
    }))
    .expect("strict read must not panic");
    let error = strict
        .expect_err("strict read reports the bad record")
        .to_string();
    assert!(error.starts_with("Parse error:"), "{error}");
    assert!(!error.contains("panicked"), "{error}");

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        DwgReader::from_stream_with_options(Cursor::new(bytes), DwgReadOptions::failsafe())
            .read_with_stats()
    }))
    .expect("failsafe read must not panic")
    .expect("failsafe returns the drawing");
    assert!(outcome.document.get_entity(healthy).is_some());
    assert!(matches!(
        outcome.document.objects.get(&auxiliary),
        Some(ObjectType::Unknown { type_name, raw_dwg_data: Some(_), .. })
            if type_name == "CONTEXTDATAMANAGER"
    ));
    assert!(outcome.stats.diagnostics.iter().any(|diagnostic| {
        diagnostic.record_handle == Some(auxiliary.value())
            && diagnostic.message.contains("CONTEXTDATAMANAGER")
            && !diagnostic.message.contains("panicked")
    }));
    assert_eq!(outcome.stats.skipped_source_records, 0);
}

#[test]
fn other_known_object_decode_failures_still_fail_strict_reads() {
    for (name, type_code) in [
        ("CONTEXTDATAMANAGER", 540),
        (
            "LAYER",
            acadrust::io::dwg::dwg_stream_readers::object_reader::common::OBJ_LAYER,
        ),
    ] {
        let (source, ..) = auxiliary_fixture(DxfVersion::AC1032, name, type_code, false, false);
        let bytes = DwgWriter::write_to_vec(&source).unwrap();
        assert!(
            DwgReader::from_stream(Cursor::new(bytes)).read().is_err(),
            "{name}"
        );
    }
}

#[test]
fn invalid_constraint_counts_keep_source_records_without_silent_dxf_loss() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        for (class_number, count) in [(861, -49), (536, 1_652_754_708), (824, 1_619_069_204)] {
            let (source, healthy, group, raw, _) = auxiliary_fixture_with_payload(
                version,
                "ACDBASSOC2DCONSTRAINTGROUP",
                class_number,
                false,
                false,
                |writer, _| {
                    writer.write_bit_short(1); // action class version
                    for _ in 0..4 {
                        writer.write_bit_long(0); // status, indexes, dependencies
                    }
                    writer.write_handle(DwgReferenceType::SoftPointer, 0);
                    writer.write_handle(DwgReferenceType::HardOwnership, 0);
                    writer.write_bit_long(2); // group version
                    writer.write_bit(false);
                    for point in [
                        acadrust::types::Vector3::ZERO,
                        acadrust::types::Vector3::UNIT_X,
                        acadrust::types::Vector3::UNIT_Y,
                    ] {
                        writer.write_3bit_double(point);
                    }
                    writer.write_handle(DwgReferenceType::HardOwnership, 0);
                    writer.write_bit_long(count); // impossible owned-action count
                },
            );
            let bytes = DwgWriter::write_to_vec(&source).unwrap();
            for options in [DwgReadOptions::default(), DwgReadOptions::failsafe()] {
                let outcome = DwgReader::from_stream_with_options(Cursor::new(&bytes), options)
                    .read_with_stats()
                    .unwrap();
                assert_eq!(outcome.stats.skipped_source_records, 0);
                assert!(outcome.document.get_entity(healthy).is_some());
                assert!(outcome.stats.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "auxiliary-object-preserved"
                        && diagnostic.record_handle == Some(group.value())
                        && diagnostic
                            .message
                            .contains(&format!("Invalid DWG array count {count}"))
                }));
                let saved = DwgWriter::write_to_vec(&outcome.document).unwrap();
                let saved = DwgReader::from_stream(Cursor::new(saved)).read().unwrap();
                assert!(matches!(saved.objects.get(&group),
                    Some(ObjectType::Unknown { raw_dwg_data: Some(data), .. }) if data == &raw));
                for writer in [
                    DxfWriter::new(&outcome.document),
                    DxfWriter::new_binary(&outcome.document),
                ] {
                    let mut output = Vec::new();
                    let error = writer.write_to_writer(&mut output).unwrap_err().to_string();
                    assert!(error.contains("undecoded constraint group"), "{error}");
                    assert!(error.contains(&group.to_string()), "{error}");
                    assert!(output.is_empty());
                }
            }
        }
    }
}

#[test]
fn strict_error_identifies_the_failed_record_after_auxiliary_recovery() {
    let (mut source, _, recovered, ..) =
        auxiliary_fixture(DxfVersion::AC1032, CONTEXT, 540, false, false);
    let broken = source.allocate_handle();
    let mut writer = DwgMergedWriter::new(DwgVersion::AC24, DxfVersion::AC1032);
    writer.write_object_type(540);
    writer.main_mut().write_handle_undefined(broken.value());
    writer.write_bit_short(0);
    writer.write_handle(DwgReferenceType::SoftPointer, 0);
    writer.write_bit_long(i32::MAX); // Invalid common reactor count.
    writer.write_bit(true);
    writer.write_bit(false);
    let raw = writer.merge();
    let handle_bits = raw.len() as i64 * 8 - writer.handle_start_bits();
    source.objects.insert(
        broken,
        ObjectType::Unknown {
            type_name: CONTEXT.into(),
            handle: broken,
            owner: Handle::NULL,
            raw_dxf_codes: None,
            raw_dwg_data: Some(raw),
            raw_dwg_handle_bits: handle_bits,
            raw_dwg_version: Some(DxfVersion::AC1032),
        },
    );
    assert!(broken > recovered);
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let error = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .unwrap_err()
        .to_string();
    assert!(error.contains(&format!("{:#X}", broken.value())), "{error}");
    assert!(!error.contains("Preserved auxiliary object"), "{error}");
}

#[test]
fn native_mleader_base_flags_and_context_payload_decode_from_oda_layout() {
    use acadrust::types::Vector3;

    // ODA DWG specification §20.4.89 (pp. 219): BS version,
    // B file-to-xdict, B default. The following MLeader context fields are
    // encoded in the order from §20.4.86 (pp. 214-216).
    // With the default flag set, the same bits also decode without an error
    // as a native one-bit record with no leader roots, which stops far short
    // of the end of the record.
    for is_default in [false, true] {
        let (source, healthy, auxiliary, _raw, _) = auxiliary_fixture_with_payload(
            DxfVersion::AC1032,
            CONTEXT,
            540,
            false,
            false,
            move |writer, scale| {
                writer.write_bit_short(4);
                writer.write_bit(true);
                writer.write_bit(is_default);
                writer.write_handle(DwgReferenceType::HardPointer, scale.value());
                writer.write_bit_long(1); // One leader root.
                writer.write_bit(true);
                writer.write_bit(false);
                writer.write_3bit_double(Vector3::ZERO);
                writer.write_3bit_double(Vector3::UNIT_X);
                writer.write_bit_long(0); // No break pairs.
                writer.write_bit_long(0); // Root index.
                writer.write_bit_double(5.0);
                writer.write_bit_long(0); // No leader lines.
                writer.write_bit_short(0);
                writer.write_bit_double(1.0); // Context scale.
                writer.write_3bit_double(Vector3::new(1.0, 2.0, 0.0));
                for value in [2.5, 1.0, 0.5] {
                    writer.write_bit_double(value);
                }
                for value in [1, 1, 0, 0] {
                    writer.write_bit_short(value);
                }
                writer.write_bit(false); // No text contents.
                writer.write_bit(false); // No block contents.
                for vector in [
                    // Raw doubles: a real record is long enough for the misread
                    // to finish inside it.
                    Vector3::new(58.5, 50.25, 0.125),
                    Vector3::new(0.6, 0.8, 0.0),
                    Vector3::new(-0.8, 0.6, 0.0),
                ] {
                    writer.write_3bit_double(vector);
                }
                writer.write_bit(false);
                writer.write_bit_short(8); // Distinct top attachment value.
                writer.write_bit_short(9);
            },
        );
        let outcome =
            DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&source).unwrap()))
                .read_with_stats()
                .unwrap();
        assert_eq!(outcome.stats.skipped_source_records, 0);
        assert!(outcome.document.get_entity(healthy).is_some());
        assert!(outcome.stats.diagnostics.is_empty());
        let Some(ObjectType::ObjectContextData(context)) = outcome.document.objects.get(&auxiliary)
        else {
            panic!("ODA-layout MLeader context should be modeled");
        };
        assert_eq!(context.class_version, 4);
        assert!(context.has_file_to_extension_dictionary);
        assert!(!context.uses_legacy_mleader_context_base);
        assert_eq!(context.is_default, is_default);
        assert_eq!(context.scale, outcome.document.context_scales[&auxiliary]);
        let source_kind = context.kind.clone();
        let ObjectContextKind::MLeader(context) = &context.kind else {
            panic!("ODA-layout MLeader payload should decode as MLeader context");
        };
        assert_eq!(context.leader_roots.len(), 1);
        assert_eq!(
            context.text_top_attachment,
            acadrust::entities::TextAttachmentType::from(8)
        );
        assert_eq!(
            context.text_bottom_attachment,
            acadrust::entities::TextAttachmentType::from(9)
        );

        // A save always writes the native one-bit base.
        let saved = DwgReader::from_stream(Cursor::new(
            DwgWriter::write_to_vec(&outcome.document).unwrap(),
        ))
        .read_with_stats()
        .unwrap();
        assert!(saved.stats.diagnostics.is_empty());
        let Some(ObjectType::ObjectContextData(saved_context)) =
            saved.document.objects.get(&auxiliary)
        else {
            panic!("saved MLeader context should be modeled");
        };
        assert!(saved_context.uses_legacy_mleader_context_base);
        assert_eq!(saved_context.is_default, is_default);
        assert_eq!(saved_context.kind, source_kind);
    }
}

#[test]
fn auxiliary_class_names_cannot_bypass_entity_or_record_framing_validation() {
    for bad_framing in [false, true] {
        let (mut source, _, auxiliary, ..) =
            auxiliary_fixture(DxfVersion::AC1032, CONTEXT, 540, false, false);
        if bad_framing {
            if let ObjectType::Unknown {
                raw_dwg_data: Some(raw),
                raw_dwg_handle_bits,
                ..
            } = source.objects.get_mut(&auxiliary).unwrap()
            {
                *raw_dwg_handle_bits = raw.len() as i64 * 8 + 1;
            }
        } else {
            let mut class = source.classes.get_by_name(CONTEXT).unwrap().clone();
            class.is_an_entity = true;
            class.item_class_id = 498;
            source.classes.clear();
            source.classes.push_preserving(class);
        }
        let bytes = DwgWriter::write_to_vec(&source).unwrap();
        assert!(
            DwgReader::from_stream(Cursor::new(bytes)).read().is_err(),
            "bad_framing={bad_framing}"
        );
    }
}

#[test]
fn opaque_auxiliary_objects_cannot_silently_disappear_on_version_change() {
    let (source, _, auxiliary, ..) =
        auxiliary_fixture(DxfVersion::AC1032, CONTEXT, 540, false, false);
    let mut read = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&source).unwrap()))
        .read()
        .unwrap();
    read.version = DxfVersion::AC1027;
    let mut output = Cursor::new(Vec::new());
    let error = DwgWriter::write_to_writer(&mut output, &read)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains(CONTEXT) && error.contains(&auxiliary.to_string()),
        "{error}"
    );
    assert!(output.into_inner().is_empty());
}

#[test]
fn valid_mleader_context_still_decodes_to_fields_without_recovery() {
    let mut source = CadDocument::with_version(DxfVersion::AC1032);
    source.register_object_context_class(CONTEXT);
    let scale_handle = source.allocate_handle();
    let mut scale = Scale::new("1:50", 1.0, 50.0);
    scale.handle = scale_handle;
    source
        .objects
        .insert(scale_handle, ObjectType::Scale(scale));
    let auxiliary = source.allocate_handle();
    let mut context = acadrust::entities::MultiLeaderAnnotContext::new();
    context.standalone_uses_root_flags = true;
    context.set_text_content("issue134", acadrust::types::Vector3::new(1.0, 2.0, 3.0));
    context.text_top_attachment = acadrust::entities::TextAttachmentType::from(8);
    context.text_bottom_attachment = acadrust::entities::TextAttachmentType::from(9);
    let object = ObjectContextData {
        handle: auxiliary,
        owner_handle: Handle::NULL,
        reactors: Vec::new(),
        xdictionary_handle: None,
        class_version: 4,
        has_file_to_extension_dictionary: true,
        uses_legacy_mleader_context_base: false,
        is_default: true,
        scale: scale_handle,
        kind: ObjectContextKind::MLeader(context),
    };
    source
        .objects
        .insert(auxiliary, ObjectType::ObjectContextData(object.clone()));
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let outcome = DwgReader::from_stream(Cursor::new(bytes))
        .read_with_stats()
        .unwrap();
    assert!(outcome.stats.diagnostics.is_empty());
    // The writer emits the native one-bit base whatever the flag says, so
    // the context reads back marked as native.
    let mut native_object = object;
    native_object.uses_legacy_mleader_context_base = true;
    assert_eq!(
        outcome.document.objects[&auxiliary],
        ObjectType::ObjectContextData(native_object)
    );
    assert_eq!(outcome.document.context_scales[&auxiliary], scale_handle);

    // Native AutoCAD captures use a historical MLeader base with only the
    // default flag. Preserve and re-emit that layout independently of the
    // generic ODA R2010+ base above.
    let ObjectType::ObjectContextData(mut legacy_object) = source.objects[&auxiliary].clone()
    else {
        panic!("source context object should be modeled");
    };
    legacy_object.uses_legacy_mleader_context_base = true;
    source.objects.insert(
        auxiliary,
        ObjectType::ObjectContextData(legacy_object.clone()),
    );
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    let legacy_outcome = DwgReader::from_stream(Cursor::new(bytes))
        .read_with_stats()
        .unwrap();
    assert!(legacy_outcome.stats.diagnostics.is_empty());
    assert_eq!(
        legacy_outcome.document.objects[&auxiliary],
        ObjectType::ObjectContextData(legacy_object)
    );
}
