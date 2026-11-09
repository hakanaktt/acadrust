use acadrust::entities::proxy_graphics::{
    CheckedProxyGraphicRecord, CheckedProxyGraphics, ProxyGraphicRecord, ProxyGraphics,
};

fn stream(record_type: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for word in [
        16 + payload.len() as u32,
        1,
        8 + payload.len() as u32,
        record_type,
    ] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn checked_unknown_roundtrip_preserves_future_records() {
    let bytes = stream(9001, &[3, 2, 1, 0]);
    let graphics = CheckedProxyGraphics::decode_checked(&bytes).unwrap();
    assert!(
        matches!(&graphics.records[0], CheckedProxyGraphicRecord::Unknown { record_type: 9001, data } if data == &[3,2,1,0])
    );
    assert_eq!(graphics.encode_checked().unwrap(), bytes);
}

#[test]
fn checked_framing_requires_exact_complete_stream() {
    let valid = stream(21, &[]);
    for end in 0..valid.len() {
        assert!(CheckedProxyGraphics::decode_checked(&valid[..end]).is_err());
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(CheckedProxyGraphics::decode_checked(&trailing).is_err());
    // Existing permissive API keeps accepting a framed prefix.
    assert!(ProxyGraphics::decode(&trailing).is_some());
    let mut impossible_count = valid;
    impossible_count[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(CheckedProxyGraphics::decode_checked(&impossible_count).is_err());
}

#[test]
fn checked_known_record_cannot_fall_back_to_unknown() {
    let bytes = stream(36, &[0; 8]);
    assert!(CheckedProxyGraphics::decode_checked(&bytes).is_err());
    assert!(matches!(
        ProxyGraphics::decode(&bytes).unwrap().records[0],
        ProxyGraphicRecord::Unknown { .. }
    ));
}

#[test]
fn sample_graphics_records_are_typed_and_roundtrip() {
    let source = include_bytes!("fixtures/proxy_graphics/sample_ac1032_mleader_b1e.bin");
    let graphics = CheckedProxyGraphics::decode_checked(source).unwrap();
    let unsupported: Vec<_> = graphics
        .records
        .iter()
        .filter_map(|record| match record {
            CheckedProxyGraphicRecord::Unknown { record_type, .. } => Some(*record_type),
            _ => None,
        })
        .collect();
    assert!(
        unsupported.is_empty(),
        "source records still opaque: {unsupported:?}"
    );
    let encoded = graphics.encode_checked().unwrap();
    assert_eq!(
        CheckedProxyGraphics::decode_checked(&encoded).unwrap(),
        graphics
    );
}

#[test]
fn checked_alignment_padding_is_not_semantic_string_data() {
    let mut payload = vec![0u8; 100];
    for offset in [40, 48, 72, 80] {
        payload[offset..offset + 8].copy_from_slice(&1.0f64.to_le_bytes());
    }
    payload[98..100].copy_from_slice(&[0xcd, 0xec]);
    let graphics = CheckedProxyGraphics::decode_checked(&stream(36, &payload)).unwrap();
    assert_eq!(
        CheckedProxyGraphics::decode_checked(&graphics.encode_checked().unwrap()).unwrap(),
        graphics
    );
}

#[test]
fn known_polyline_count_cannot_escape_its_payload() {
    let mut payload = 2u32.to_le_bytes().to_vec();
    payload.extend_from_slice(&[0; 24]);
    assert!(CheckedProxyGraphics::decode_checked(&stream(6, &payload)).is_err());
}

#[test]
fn trait_values_require_valid_closed_enums_and_finite_scalars() {
    for value in [1u32, 2] {
        assert!(CheckedProxyGraphics::decode_checked(&stream(20, &value.to_le_bytes())).is_ok());
    }
    assert!(CheckedProxyGraphics::decode_checked(&stream(20, &0u32.to_le_bytes())).is_err());
    assert!(CheckedProxyGraphics::decode_checked(&stream(24, &f64::NAN.to_le_bytes())).is_err());
    assert!(CheckedProxyGraphics::decode_checked(&stream(25, &(-3.5f64).to_le_bytes())).is_ok());
}

#[test]
fn compatibility51_only_recognizes_the_zero_word() {
    let zero = CheckedProxyGraphics::decode_checked(&stream(51, &0u32.to_le_bytes())).unwrap();
    assert!(!matches!(
        zero.records[0],
        CheckedProxyGraphicRecord::Unknown { .. }
    ));
    let future = stream(51, &1u32.to_le_bytes());
    let graphics = CheckedProxyGraphics::decode_checked(&future).unwrap();
    assert!(matches!(
        graphics.records[0],
        CheckedProxyGraphicRecord::Unknown { .. }
    ));
    assert_eq!(graphics.encode_checked().unwrap(), future);
}

#[test]
fn parametric_geometry_and_transform_records_are_typed() {
    let doubles = |values: &[f64]| {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>()
    };
    let points = [1., 2., 3., 4., 5., 6., 7., 8., 9.];
    let mut arc = doubles(&[1., 2., 3., 2.5, 0., 0., 1., 1., 0., 0., 1.25]);
    arc.extend_from_slice(&2i32.to_le_bytes());
    let mut arc3 = doubles(&points);
    arc3.extend_from_slice(&1i32.to_le_bytes());
    let mut normal_polyline = 2u32.to_le_bytes().to_vec();
    normal_polyline.extend_from_slice(&doubles(&points));
    let matrix = doubles(&[
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 12., 13., 14., 1.,
    ]);
    for (kind, payload) in [
        (1, doubles(&points[..6])),
        (2, doubles(&[1., 2., 3., 2.5, 0., 0., 1.])),
        (3, doubles(&points)),
        (4, arc),
        (5, arc3),
        (29, matrix),
        (31, vec![]),
        (32, normal_polyline),
    ] {
        let graphics = CheckedProxyGraphics::decode_checked(&stream(kind, &payload)).unwrap();
        let CheckedProxyGraphicRecord::Command(command) = &graphics.records[0] else {
            panic!("record {kind} remains untyped")
        };
        assert_eq!(command.record_type(), kind);
        assert_eq!(
            CheckedProxyGraphics::decode_checked(&graphics.encode_checked().unwrap()).unwrap(),
            graphics
        );
        let mut truncated = payload;
        if !truncated.is_empty() {
            truncated.pop();
            assert!(CheckedProxyGraphics::decode_checked(&stream(kind, &truncated)).is_err());
        }
    }
}

#[test]
fn styled_unicode_preserves_non_bmp_text_empty_font_requests_and_source_length() {
    use acadrust::entities::proxy_graphics::ProxyGraphicCommand;
    let source = include_bytes!("fixtures/proxy_graphics/sample_ac1032_mleader_b1e.bin");
    let graphics = CheckedProxyGraphics::decode_checked(source).unwrap();
    let mut text = graphics
        .records
        .into_iter()
        .find_map(|record| match record {
            CheckedProxyGraphicRecord::Command(ProxyGraphicCommand::StyledUnicodeText(text)) => {
                Some(text)
            }
            _ => None,
        })
        .unwrap();
    text.text = "🙂".into();
    text.declared_length = 2;
    text.family.clear();
    text.font_file.clear();
    text.big_font_file.clear();
    text.text_flags[2] = true;
    let valid = CheckedProxyGraphics {
        records: vec![CheckedProxyGraphicRecord::Command(
            ProxyGraphicCommand::StyledUnicodeText(text.clone()),
        )],
    };
    assert_eq!(
        CheckedProxyGraphics::decode_checked(&valid.encode_checked().unwrap()).unwrap(),
        valid
    );
    text.declared_length = 1;
    let invalid = CheckedProxyGraphics {
        records: vec![CheckedProxyGraphicRecord::Command(
            ProxyGraphicCommand::StyledUnicodeText(text),
        )],
    };
    assert!(
        invalid.encode_checked().is_err(),
        "declared source length must count UTF16 units, not Unicode scalars"
    );
}

#[test]
fn legacy_text_records_preserve_source_codepage_bytes() {
    let frame = [0., 0., 0., 0., 0., 1., 1., 0., 0.]
        .iter()
        .flat_map(|value: &f64| value.to_le_bytes())
        .collect::<Vec<_>>();
    let mut plain = frame.clone();
    for value in [2.0f64, 1.0, 0.0] {
        plain.extend_from_slice(&value.to_le_bytes());
    }
    plain.extend_from_slice(&[0x80, 0, 0xcd, 0xec]); // Codepage byte, terminator, nonsemantic padding.
    let mut styled = frame;
    styled.extend_from_slice(&[0x80, 0, 0, 0]);
    for value in [1i32, 1] {
        styled.extend_from_slice(&value.to_le_bytes());
    }
    for value in [2.0f64, 1.0, 0.0, 1.0] {
        styled.extend_from_slice(&value.to_le_bytes());
    }
    for value in [0u32, 0, 1, 0, 0] {
        styled.extend_from_slice(&value.to_le_bytes());
    }
    styled.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]); // Two unspecified font requests.
    for (kind, payload) in [(10, plain), (11, styled)] {
        let graphics = CheckedProxyGraphics::decode_checked(&stream(kind, &payload)).unwrap();
        let CheckedProxyGraphicRecord::Command(command) = &graphics.records[0] else {
            panic!("legacy text {kind} is opaque")
        };
        assert_eq!(command.record_type(), kind);
        assert_eq!(
            CheckedProxyGraphics::decode_checked(&graphics.encode_checked().unwrap()).unwrap(),
            graphics
        );
        let mut truncated = payload;
        truncated.pop();
        assert!(CheckedProxyGraphics::decode_checked(&stream(kind, &truncated)).is_err());
    }
}

#[test]
fn reconstructed_snapshot_survives_production_dwg_and_both_dxf_writers() {
    use acadrust::entities::{EntityType, MultiLeader};
    use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter, Vector3};
    use std::io::Cursor;
    let source = include_bytes!("fixtures/proxy_graphics/sample_ac1032_mleader_b1e.bin");
    let expected = CheckedProxyGraphics::decode_checked(source).unwrap();
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut leader = MultiLeader::with_text(
        "graphics fixture",
        Vector3::new(3., 3., 0.),
        vec![Vector3::ZERO, Vector3::new(2., 2., 0.)],
    );
    leader.common.graphic_data = Some(expected.encode_checked().unwrap());
    doc.add_entity(EntityType::MultiLeader(Box::new(leader)))
        .unwrap();
    let ascii = DxfReader::from_reader(Cursor::new(DxfWriter::new(&doc).write_to_vec().unwrap()))
        .unwrap()
        .read()
        .unwrap();
    let binary = DxfReader::from_reader(Cursor::new(
        DxfWriter::new_binary(&doc).write_to_vec().unwrap(),
    ))
    .unwrap()
    .read()
    .unwrap();
    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    for (format, drawing) in [("ASCII DXF", ascii), ("binary DXF", binary), ("DWG", dwg)] {
        let bytes = drawing
            .entities()
            .find_map(|entity| match entity {
                EntityType::MultiLeader(leader) => leader.common.graphic_data.as_deref(),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{format} lost saved graphics"));
        assert_eq!(
            CheckedProxyGraphics::decode_checked(bytes).unwrap(),
            expected,
            "{format}"
        );
    }
}

/// H1 is a deliberately provisional self-generated wire specimen, not evidence
/// of how AutoCAD/ARES encode owned tolerance content. No DIMSTYLE is inferred.
#[test]
fn experimental_tolerance_h1_preserves_tag_and_text_only() {
    use acadrust::entities::{multileader::LeaderContentType, EntityType, MultiLeader};
    use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Vector3};
    use std::io::Cursor;
    let mut document = CadDocument::new();
    let mut value = MultiLeader::with_text(
        "{\\Fgdt;j}%%v0.1%%vA",
        Vector3::new(10., 10., 0.),
        vec![Vector3::ZERO, Vector3::new(10., 10., 0.)],
    );
    value.content_type = LeaderContentType::Tolerance;
    document
        .add_entity(EntityType::MultiLeader(Box::new(value)))
        .unwrap();
    for format in ["ascii", "binary", "dwg"] {
        let bytes = match format {
            "dwg" => DwgWriter::write_to_vec(&document).unwrap(),
            "binary" => DxfWriter::new_binary(&document).write_to_vec().unwrap(),
            _ => DxfWriter::new(&document).write_to_vec().unwrap(),
        };
        let outcome = if format == "dwg" {
            DwgReader::from_stream(Cursor::new(bytes))
                .read_with_stats()
                .unwrap()
        } else {
            DxfReader::from_reader(Cursor::new(bytes))
                .unwrap()
                .read_with_stats()
                .unwrap()
        };
        assert_eq!(outcome.stats.skipped_source_records, 0, "{format}");
        assert_eq!(outcome.stats.recovered_errors, 0, "{format}");
        let value = outcome
            .document
            .model_space_entities()
            .find_map(|e| match e {
                EntityType::MultiLeader(v) => Some(v),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            value.content_type,
            LeaderContentType::Tolerance,
            "{format}: type3 may not normalize to None/MText"
        );
        assert!(value.context.has_text_contents, "{format}");
        assert_eq!(
            value.context.text_string, "{\\Fgdt;j}%%v0.1%%vA",
            "{format}"
        );
    }
}
