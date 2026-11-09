use std::io::Cursor;

use acadrust::entities::leader::LeaderPathType;
use acadrust::entities::{EntityType, Leader, MultiLeader, RawRecord};
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::io::dwg::{DwgReadOptions, DwgVersion};
use acadrust::objects::{MultiLeaderStyle, ObjectType};
use acadrust::{
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfReaderConfiguration, DxfVersion, DxfWriter,
    ReadStats, Vector3,
};

fn replace_dxf_values(drawing: &CadDocument, record: &str, changes: &[(i32, &str)]) -> Vec<u8> {
    let text = String::from_utf8(DxfWriter::new(drawing).write_to_vec().unwrap()).unwrap();
    let lines: Vec<_> = text.lines().collect();
    let mut pairs: Vec<_> = lines
        .chunks_exact(2)
        .map(|p| (p[0].trim().parse::<i32>().unwrap(), p[1]))
        .collect();
    let start = pairs
        .iter()
        .position(|(c, v)| *c == 0 && v.trim() == record)
        .unwrap();
    let end = start
        + 1
        + pairs[start + 1..]
            .iter()
            .position(|(c, _)| *c == 0)
            .unwrap();
    for &(code, value) in changes {
        let position = (start..end).rev().find(|i| pairs[*i].0 == code).unwrap();
        pairs[position].1 = value;
    }
    pairs
        .into_iter()
        .map(|(code, value)| format!("{code}\r\n{value}\r\n"))
        .collect::<String>()
        .into_bytes()
}

fn assert_warned(stats: &ReadStats, handle: u64, values: &[&str]) {
    assert_eq!(stats.skipped_source_records, 0);
    assert_eq!(stats.recovered_errors, 0);
    for value in values {
        assert!(
            stats.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "unknown-leader-enum"
                    && diagnostic.record_handle == Some(handle)
                    && diagnostic.message.contains(&format!("value {value}"))
            }),
            "missing diagnostic for {handle:X}, value {value}: {:?}",
            stats.diagnostics
        );
    }
}

#[test]
fn dxf_unknown_leader_enums_warn_without_dropping_entities_or_styles() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let leader = document
        .add_entity(EntityType::Leader(Leader::two_point(
            Vector3::ZERO,
            Vector3::UNIT_X,
        )))
        .unwrap();
    let multileader = document
        .add_entity(EntityType::MultiLeader(Box::new(MultiLeader::with_text(
            "preserved",
            Vector3::UNIT_X,
            vec![Vector3::ZERO, Vector3::UNIT_X],
        ))))
        .unwrap();
    let style = document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::MultiLeaderStyle(style) => Some(style.handle),
            _ => None,
        })
        .unwrap();

    for (record, handle, changes) in [
        ("LEADER", leader, &[(72, "99")][..]),
        (
            "MULTILEADER",
            multileader,
            &[(272, "4786"), (273, "32"), (172, "99")],
        ),
        ("MLEADERSTYLE", style, &[(170, "99")]),
    ] {
        let bytes = replace_dxf_values(&document, record, changes);
        let values: Vec<_> = changes.iter().map(|(_, value)| *value).collect();
        for failsafe in [false, true] {
            let outcome = DxfReader::from_reader(Cursor::new(bytes.clone()))
                .unwrap()
                .with_configuration(DxfReaderConfiguration {
                    failsafe,
                    ..Default::default()
                })
                .read_with_stats()
                .unwrap();
            assert_warned(&outcome.stats, handle.value(), &values);
            assert!(matches!(
                outcome.document.get_entity(leader),
                Some(EntityType::Leader(_))
            ));
            assert!(
                matches!(outcome.document.get_entity(multileader), Some(EntityType::MultiLeader(ml)) if ml.context.text_string == "preserved")
            );
            assert!(matches!(
                outcome.document.objects.get(&style),
                Some(ObjectType::MultiLeaderStyle(_))
            ));
        }
    }
}

#[test]
fn dxf_unlisted_mleader_style_uses_group_3_for_its_name() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = MultiLeaderStyle::new("");
    style.handle = document.allocate_handle();
    style.owner_handle = document.header.named_objects_dict_handle;
    style.description = "Unlisted style".into();
    let handle = style.handle;
    document
        .objects
        .insert(handle, ObjectType::MultiLeaderStyle(style));
    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    assert!(
        matches!(back.objects.get(&handle), Some(ObjectType::MultiLeaderStyle(style))
        if style.name == "Unlisted style" && style.description == "Unlisted style")
    );
}

#[test]
fn dwg_unknown_leader_enum_warns_and_preserves_later_fields() {
    let version = DxfVersion::AC1032;
    let dwg_version = DwgVersion::from_dxf_version(version).unwrap();
    let mut document = CadDocument::with_version(version);
    let mut leader = Leader::two_point(Vector3::ZERO, Vector3::new(3.0, 4.0, 0.0));
    leader.path_type = LeaderPathType::Spline;
    let vertices = leader.vertices.clone();
    let handle = document.add_entity(EntityType::Leader(leader)).unwrap();
    let (bytes, handles, _, _) = DwgObjectWriter::new(&document).unwrap().write();
    let offset = handles
        .iter()
        .find(|(h, _)| *h == handle.value())
        .unwrap()
        .1;
    let objects = DwgObjectReader::new(
        bytes,
        version,
        handles
            .into_iter()
            .map(|(h, p)| (h, i64::from(p)))
            .collect(),
    )
    .unwrap();
    let (kind, mut reader) = objects.read_record_at(offset as usize).unwrap();
    objects.read_common_entity_data(&mut reader, kind);
    reader.read_bit();
    reader.read_bit_short();
    let path_position = reader.position_in_bits();
    assert_eq!(reader.read_bit_short(), 1);
    let count_position = reader.position_in_bits();
    assert_eq!(reader.read_bit_long(), 2);
    let mut raw = DwgBitWriter::new(dwg_version, version);
    raw.write_bytes(&reader.raw_merged_data());
    raw.set_position_in_bits(path_position);
    raw.write_bit_short(99); // Same bit-short width as the source value 1.
    let raw_record = |raw: &DwgBitWriter| {
        Some(
            RawRecord {
                data: raw.to_bytes_snapshot(),
                handle_bits: reader.get_handle_bits(),
                version,
            }
            .into(),
        )
    };
    document
        .get_entity_mut(handle)
        .unwrap()
        .common_mut()
        .raw_record = raw_record(&raw);

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    for options in [DwgReadOptions::default(), DwgReadOptions::failsafe()] {
        let outcome = DwgReader::from_stream_with_options(Cursor::new(bytes.as_slice()), options)
            .read_with_stats()
            .unwrap();
        assert_warned(&outcome.stats, handle.value(), &["99"]);
        let Some(EntityType::Leader(leader)) = outcome.document.get_entity(handle) else {
            panic!("LEADER with unknown path type was dropped");
        };
        assert_eq!(leader.path_type, LeaderPathType::StraightLine);
        assert_eq!(leader.vertices, vertices);
    }

    // Claim more vertices than the record holds, after the unknown path value.
    raw.set_position_in_bits(count_position);
    raw.write_bit_long(255);
    document
        .get_entity_mut(handle)
        .unwrap()
        .common_mut()
        .raw_record = raw_record(&raw);
    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let error = DwgReader::from_stream(Cursor::new(bytes))
        .read_with_stats()
        .unwrap_err()
        .to_string();
    assert!(error.contains("Invalid record at handle"), "{error}");
    assert!(!error.contains("Unknown read_leader.path_type"), "{error}");
}
