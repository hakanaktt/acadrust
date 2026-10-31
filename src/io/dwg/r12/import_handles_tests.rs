use super::*;

pub(super) fn record(kind: &str, pairs: &[(i32, &str)]) -> EntityRecord {
    EntityRecord {
        kind: kind.into(),
        pairs: pairs.iter().map(|(c, v)| (*c, (*v).into())).collect(),
        ..EntityRecord::default()
    }
}

pub(super) fn fixture(
    seed: u64,
    tables: &[Vec<EntityRecord>; 10],
    entities: &[EntityRecord],
    blocks: &[EntityRecord],
) -> Vec<u8> {
    fixture_with_references(seed, tables, entities, blocks, HeaderReferences::default())
}

fn fixture_with_references(
    seed: u64,
    tables: &[Vec<EntityRecord>; 10],
    entities: &[EntityRecord],
    blocks: &[EntityRecord],
    references: HeaderReferences,
) -> Vec<u8> {
    let refs = ensure_table_defaults(tables::entity_tables(tables));
    let entity_bytes =
        encode_entities(entities, &refs, header::ENTITIES_START, "ANSI_1252").unwrap();
    let entities_end = header::ENTITIES_START + entity_bytes.len() as u32;
    let blocks_start = entities_end + 32;
    let block_bytes = encode_entities(blocks, &refs, blocks_start, "ANSI_1252").unwrap();
    let extras_start = blocks_start + block_bytes.len() as u32 + 32;
    let mut address = extras_start + 16;
    let mut locators = [TableLocator::default(); 10];
    let mut bytes = Vec::new();
    for (i, (size, payload)) in
        tables::encode_tables(tables, &std::collections::HashMap::new(), "ANSI_1252")
            .unwrap()
            .into_iter()
            .enumerate()
    {
        if payload.is_empty() {
            continue;
        }
        locators[i] = TableLocator {
            record_size: size,
            count: (payload.len() / size as usize) as u16,
            flags: 0,
            address: address + 16,
        };
        bytes.extend_from_slice(&header::TABLE_BEGIN[i]);
        bytes.extend_from_slice(&payload);
        bytes.extend_from_slice(&tables::TABLE_END[i]);
        address += payload.len() as u32 + 32;
    }
    let layout = HeaderLayout {
        entities_start: header::ENTITIES_START,
        entities_end,
        blocks_start,
        blocks_size: block_bytes.len() as u32,
        extras_start,
        extras_size: 0,
    };
    let mut vars = crate::document::HeaderVariables::default();
    vars.handle_seed = seed;
    let mut out = write_header(&vars, layout, &locators, references, 0, None).unwrap();
    out[0x4ec..0x4ee].copy_from_slice(&u16::from(seed != 0).to_le_bytes()); // HANDLING disabled: every entity receives an import handle.
    let crc = crate::io::dwg::crc::crc16(
        crate::io::dwg::crc::CRC16_SEED,
        &out[..header::HEADER_VARIABLES_END],
    );
    out[header::HEADER_VARIABLES_END..header::HEADER_VARIABLES_END + 2]
        .copy_from_slice(&crc.to_le_bytes());
    out.extend(entity_bytes);
    out.extend(ENTITIES_END);
    out.extend(BLOCK_ENTITIES_BEGIN);
    out.extend(block_bytes);
    out.extend(BLOCK_ENTITIES_END);
    out.extend(EXTRA_ENTITIES_BEGIN);
    out.extend(EXTRA_ENTITIES_END);
    out.extend(bytes);
    out.extend(write_auxheader(layout, &locators, seed, address));
    out
}

#[test]
fn handleless_text_matches_independent_imports() {
    let mut tables: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    tables[1] = vec![
        record("LAYER", &[(2, "0"), (62, "7"), (6, "CONTINUOUS")]),
        record("LAYER", &[(2, "PROBE"), (62, "2"), (6, "CONTINUOUS")]),
    ];
    tables[2] = [
        ("STANDARD", "txt"),
        ("ALPHA", "simplex"),
        ("BRAVO", "simplex"),
        ("CHARLIE", "txt"),
        ("DELTA", "iso"),
    ]
    .map(|(n, f)| record("STYLE", &[(2, n), (3, f)]))
    .to_vec();
    tables[3] = vec![record("LTYPE", &[(2, "CONTINUOUS")])];
    tables[7] = vec![record("APPID", &[(2, "ACAD")])];
    let styles = ["ALPHA", "BRAVO", "STANDARD", "CHARLIE", "DELTA", "ALPHA"];
    let entities = styles
        .iter()
        .map(|s| {
            record(
                "TEXT",
                &[
                    (8, "0"),
                    (1, "Probe"),
                    (7, s),
                    (40, "2"),
                    (10, "0"),
                    (20, "0"),
                ],
            )
        })
        .collect::<Vec<_>>();
    let doc = read_document(&fixture(0, &tables, &entities, &[]), None, None).unwrap();
    let handles = doc
        .entities()
        .filter(|e| matches!(e, crate::EntityType::Text(_)))
        .map(|e| e.common().handle.value())
        .collect::<Vec<_>>();
    assert_eq!(handles, [0x7c, 0x7e, 0x7f, 0x81, 0x82, 0x84]);
    assert_eq!(doc.layers.get("PROBE").unwrap().handle.value(), 0x76);
    assert_eq!(
        doc.block_records
            .get("*Model_Space")
            .unwrap()
            .handle
            .value(),
        0x6f
    );
    assert_eq!(
        doc.block_records
            .get("*Model_Space")
            .unwrap()
            .block_entity_handle
            .value(),
        0x79
    );
    assert_eq!(
        doc.block_records
            .get("*Model_Space")
            .unwrap()
            .layout
            .value(),
        0x72
    );
}

#[test]
fn legacy_font_substitution_shares_the_transient_identity() {
    let mut tables: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    tables[1] = vec![
        record("LAYER", &[(2, "0"), (62, "7"), (6, "CONTINUOUS")]),
        record("LAYER", &[(2, "PROBE"), (62, "2"), (6, "CONTINUOUS")]),
    ];
    tables[2] = [
        ("STANDARD", "txt"),
        ("ALPHA", "simplex"),
        ("BRAVO", "simplex"),
        ("CHARLIE", "txt"),
        ("DELTA", "unavailable_indium_probe_font"),
    ]
    .map(|(n, f)| record("STYLE", &[(2, n), (3, f)]))
    .to_vec();
    tables[3] = vec![record("LTYPE", &[(2, "CONTINUOUS")])];
    tables[7] = vec![record("APPID", &[(2, "ACAD")])];
    let entities = ["DELTA", "STANDARD", "ALPHA", "CHARLIE"].map(|s| {
        record(
            "TEXT",
            &[
                (8, "0"),
                (1, "Probe"),
                (7, s),
                (40, "2"),
                (10, "0"),
                (20, "0"),
            ],
        )
    });
    let bytes = fixture(0, &tables, &entities, &[]);
    let mut reader =
        crate::DwgReader::from_stream(Cursor::new(bytes)).with_legacy_font_resolver(|font| {
            if font == "unavailable_indium_probe_font" {
                "SIMPLEX".into()
            } else {
                font.to_ascii_uppercase()
            }
        });
    let doc = reader.read().unwrap();
    assert_eq!(
        doc.entities()
            .filter(|e| matches!(e, crate::EntityType::Text(_)))
            .map(|e| e.common().handle.value())
            .collect::<Vec<_>>(),
        [0x7c, 0x7e, 0x80, 0x81]
    );
}

#[test]
fn stored_compound_handles_and_owners_survive_import() {
    use crate::{EntityType, Handle};
    let mut tables: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    tables[0] = vec![record("BLOCK_RECORD", &[(2, "PART")])];
    tables[1] = vec![record("LAYER", &[(2, "0"), (62, "7"), (6, "CONTINUOUS")])];
    tables[2] = vec![record("STYLE", &[(2, "STANDARD"), (3, "txt")])];
    tables[3] = vec![record("LTYPE", &[(2, "CONTINUOUS")])];
    tables[7] = vec![record("APPID", &[(2, "ACAD")])];
    let blocks = [
        record("BLOCK", &[(2, "PART"), (5, "11"), (8, "0")]),
        record(
            "ATTDEF",
            &[
                (5, "12"),
                (8, "0"),
                (2, "TAG"),
                (1, "default"),
                (3, "Prompt"),
                (40, "2"),
            ],
        ),
        record("ENDBLK", &[(5, "13"), (8, "0")]),
    ];
    let entities = [
        record("INSERT", &[(5, "20"), (8, "0"), (2, "PART"), (66, "1")]),
        record(
            "ATTRIB",
            &[(5, "21"), (8, "0"), (2, "TAG"), (1, "value"), (40, "2")],
        ),
        record("SEQEND", &[(5, "22"), (8, "0")]),
    ];
    let doc = read_document(&fixture(0x1000, &tables, &entities, &blocks), None, None).unwrap();
    let part = doc.block_records.get("PART").unwrap();
    assert_eq!(part.handle.value(), 0x1072);
    assert_eq!(part.block_entity_handle.value(), 0x11);
    assert_eq!(part.block_end_handle.value(), 0x13);
    assert_eq!(
        doc.get_entity(Handle::new(0x12))
            .unwrap()
            .common()
            .owner_handle,
        part.handle
    );
    let EntityType::Insert(insert) = doc.get_entity(Handle::new(0x20)).unwrap() else {
        panic!("missing insert")
    };
    assert_eq!(
        insert.common.owner_handle,
        doc.header.model_space_block_handle
    );
    assert_eq!(insert.attributes[0].common.handle.value(), 0x21);
    assert_eq!(insert.seqend_handle, Some(Handle::new(0x22)));
    assert!(doc.next_handle() > 0x1072);
}

#[test]
fn import_handle_overflow_is_rejected() {
    let mut tables = std::array::from_fn(|_| Vec::new());
    assert!(import_handles::ImportHandles::assign(
        u64::MAX,
        &mut tables,
        &mut [],
        &mut [],
        None,
        None
    )
    .is_err());
}

#[test]
fn duplicated_anonymous_block_names_keep_distinct_owned_contents() {
    let mut tables: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    tables[0] = vec![
        record("BLOCK", &[(2, "*X"), (70, "1"), (90, "0")]),
        record("BLOCK", &[(2, "*X"), (70, "1"), (90, "0")]),
    ];
    tables[1] = vec![record("LAYER", &[(2, "0")])];
    tables[3] = vec![record("LTYPE", &[(2, "CONTINUOUS")])];
    let blocks = vec![
        record("BLOCK", &[(2, "*X"), (5, "201"), (8, "0")]),
        record("LINE", &[(5, "202"), (8, "0"), (10, "1"), (11, "2")]),
        record("ENDBLK", &[(5, "203"), (8, "0")]),
        record("BLOCK", &[(2, "*X"), (5, "204"), (8, "0")]),
        record("LINE", &[(5, "205"), (8, "0"), (10, "3"), (11, "4")]),
        record("ENDBLK", &[(5, "206"), (8, "0")]),
    ];
    let refs = ensure_table_defaults(tables::entity_tables(&tables));
    let offset = encode_entities(&blocks[..3], &refs, 0, "ANSI_1252")
        .unwrap()
        .len();
    tables[0][1].pairs.retain(|(code, _)| *code != 90);
    tables[0][1].pairs.push((90, offset.to_string()));
    let inserts = [record("INSERT", &[(2, "*X"), (5, "301"), (8, "0")])];
    let doc =
        crate::DwgReader::from_stream(Cursor::new(fixture(0x1000, &tables, &inserts, &blocks)))
            .read()
            .unwrap();
    let first = doc.get_entity(Handle::new(0x202)).unwrap();
    let second = doc.get_entity(Handle::new(0x205)).unwrap();
    let owners = (first.common().owner_handle, second.common().owner_handle);
    assert_ne!(
        owners.0, doc.header.model_space_block_handle,
        "first block content escaped into model space"
    );
    assert_ne!(
        owners.1, doc.header.model_space_block_handle,
        "second block content escaped into model space"
    );
    assert_ne!(owners.0, owners.1, "distinct anonymous blocks were merged");
    for (owner, child) in [
        (owners.0, Handle::new(0x202)),
        (owners.1, Handle::new(0x205)),
    ] {
        let block = doc
            .block_records
            .iter()
            .find(|b| b.handle == owner)
            .unwrap();
        assert!(block.entity_handles.contains(&child));
    }
    let EntityType::Insert(insert) = doc.get_entity(Handle::new(0x301)).unwrap() else {
        panic!("insert");
    };
    assert_eq!(
        doc.block_records.get(&insert.block_name).unwrap().handle,
        owners.0
    );
}

#[test]
fn anonymous_block_fallback_names_do_not_collide_with_existing_names() {
    use super::entities::PositionedRecord;
    let mut rows = vec![
        record("BLOCK", &[(2, "*X"), (70, "1"), (90, "0")]),
        record("BLOCK", &[(2, "*X"), (70, "1"), (90, "10")]),
        record("BLOCK", &[(2, "*X0"), (70, "1"), (90, "20")]),
    ];
    let blocks = SectionRecords {
        base_address: 100,
        records: [0, 10, 20]
            .into_iter()
            .map(|offset| PositionedRecord {
                offset,
                address: 100 + offset,
                size: 10,
                record: Some(record(
                    "BLOCK",
                    &[(2, if offset == 20 { "*X0" } else { "*X" })],
                )),
                jump_address: None,
                sequence_owner: None,
            })
            .collect(),
        ..Default::default()
    };
    adopt_distinct_block_names(&mut rows, &blocks, 100);
    let names: std::collections::HashSet<_> = rows.iter().map(|r| r.string(2)).collect();
    assert_eq!(names.len(), 3, "anonymous definitions need distinct names");
    assert_eq!(
        rows[2].string(2),
        "*X0",
        "an existing unique name must stay unchanged"
    );
}

#[test]
fn anonymous_block_fallback_does_not_claim_a_later_marker_name() {
    use super::entities::PositionedRecord;
    let mut rows = vec![
        record("BLOCK", &[(2, "*X"), (70, "1"), (90, "0")]),
        record("BLOCK", &[(2, "*X"), (70, "1"), (90, "10")]),
    ];
    let blocks = SectionRecords {
        base_address: 100,
        records: [(0, "*X"), (10, "*X0")]
            .into_iter()
            .map(|(offset, name)| PositionedRecord {
                offset,
                address: 100 + offset,
                size: 10,
                record: Some(record("BLOCK", &[(2, name)])),
                jump_address: None,
                sequence_owner: None,
            })
            .collect(),
        ..Default::default()
    };
    adopt_distinct_block_names(&mut rows, &blocks, 100);
    assert_eq!(rows[1].string(2), "*X0", "preserve the distinct BLOCK marker name");
    assert_ne!(rows[0].string(2), "*X0");
}

#[test]
fn legacy_header_indices_resolve_to_imported_table_handles() {
    let mut tables: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    tables[1] = vec![
        record("LAYER", &[(2, "0")]),
        record("LAYER", &[(2, "WORK")]),
    ];
    tables[2] = vec![
        record("STYLE", &[(2, "STANDARD")]),
        record("STYLE", &[(2, "LABEL")]),
    ];
    tables[3] = vec![
        record("LTYPE", &[(2, "CONTINUOUS")]),
        record("LTYPE", &[(2, "DASHED")]),
    ];
    for index in [1, -1, -2] {
        let refs = HeaderReferences {
            layer_index: 1,
            text_style_index: 1,
            linetype_index: index,
            ..Default::default()
        };
        let doc = read_document(
            &fixture_with_references(0, &tables, &[], &[], refs),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            doc.header.current_layer_handle,
            doc.layers.get("WORK").unwrap().handle
        );
        assert_eq!(
            doc.header.current_text_style_handle,
            doc.text_styles.get("LABEL").unwrap().handle
        );
        let name = match index {
            -1 => "ByLayer",
            -2 => "ByBlock",
            _ => "DASHED",
        };
        assert_eq!(
            doc.header.current_linetype_handle,
            doc.line_types.get(name).unwrap().handle
        );
    }
}

#[test]
fn adopt_only_splits_duplicated_block_table_names() {
    use super::entities::PositionedRecord;
    use super::traversal::SectionRecords;

    let mut rows = vec![
        record("BLOCK", &[(2, "KEEP"), (90, "0")]),
        record("BLOCK", &[(2, "*D"), (90, "10")]),
        record("BLOCK", &[(2, "*D"), (90, "20")]),
        record("BLOCK", &[(2, "*D"), (90, "30")]),
    ];
    let blocks = SectionRecords {
        records: ["RENAMED", "*D0", "KEEP", "*D1"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| PositionedRecord {
                offset: index as u32 * 10,
                address: index as u32 * 10,
                size: 0,
                record: Some(record("BLOCK", &[(2, name)])),
                jump_address: None,
                sequence_owner: None,
            })
            .collect(),
        ..SectionRecords::default()
    };
    super::adopt_distinct_block_names(&mut rows, &blocks, 0);
    assert_eq!(rows[0].string(2), "KEEP");
    assert_eq!(rows[1].string(2), "*D0");
    assert_eq!(rows[2].string(2), "*D");
    assert_eq!(rows[3].string(2), "*D1");
}

fn with_point(mut record: EntityRecord, code: i32, x: f64, y: f64, z: f64) -> EntityRecord {
    record.pairs.push((code, x.to_string()));
    record.pairs.push((code + 10, y.to_string()));
    record.pairs.push((code + 20, z.to_string()));
    record
}

fn collapse_block_table_names(bytes: &mut [u8]) {
    let size = u16::from_le_bytes(bytes[0x2c..0x2e].try_into().unwrap()) as usize;
    let count = u16::from_le_bytes(bytes[0x2e..0x30].try_into().unwrap()) as usize;
    let address = u32::from_le_bytes(bytes[0x32..0x36].try_into().unwrap()) as usize;
    for index in 0..count {
        let start = address + index * size;
        let record = &mut bytes[start..start + size];
        record[1..33].fill(0);
        record[1..3].copy_from_slice(b"*D");
        let checksum = crate::io::dwg::crc::crc16(0xC0C1, &record[..size - 2]);
        record[size - 2..].copy_from_slice(&checksum.to_le_bytes());
    }
}

#[test]
fn duplicate_r11_dimension_block_names_keep_entity_identity() {
    let blocks = vec![
        with_point(record("BLOCK", &[(2, "*D0")]), 10, 0.0, 0.0, 0.0),
        with_point(
            with_point(record("LINE", &[(8, "0")]), 10, 0.0, 0.0, 0.0),
            11,
            1.0,
            0.0,
            0.0,
        ),
        record("ENDBLK", &[]),
        with_point(record("BLOCK", &[(2, "*D1")]), 10, 0.0, 0.0, 0.0),
        with_point(
            with_point(record("LINE", &[(8, "0")]), 10, 2.0, 0.0, 0.0),
            11,
            3.0,
            0.0,
            0.0,
        ),
        record("ENDBLK", &[]),
    ];
    let mut tables: [Vec<EntityRecord>; 10] = std::array::from_fn(|_| Vec::new());
    tables[0] = vec![
        record("BLOCK", &[(2, "*D0"), (70, "1")]),
        record("BLOCK", &[(2, "*D1"), (70, "1")]),
    ];
    let refs = ensure_table_defaults(tables::entity_tables(&tables));
    let encoded = entities::encode_entities(&blocks, &refs, 0, "ANSI_1252").unwrap();
    let mut offsets = Vec::new();
    let mut position = 0;
    while position < encoded.len() {
        if encoded[position] & 127 == 12 {
            offsets.push(position as u32);
        }
        let size = u16::from_le_bytes([encoded[position + 2], encoded[position + 3]]) as usize;
        position += size;
    }
    assert_eq!(offsets.len(), 2);
    for (row, offset) in tables[0].iter_mut().zip(offsets) {
        row.pairs.push((90, (offset | 0x4000_0000).to_string()));
    }
    let mut dimension = with_point(
        record("DIMENSION", &[(2, "*D1"), (8, "0"), (70, "1")]),
        10,
        5.0,
        5.0,
        0.0,
    );
    dimension = with_point(dimension, 11, 6.0, 6.0, 0.0);
    let mut bytes = fixture(0x1000, &tables, &[dimension], &blocks);
    collapse_block_table_names(&mut bytes);

    let doc = read_document(&bytes, None, None).unwrap();
    let names: Vec<_> = doc
        .block_records
        .iter()
        .map(|block| block.name.clone())
        .collect();
    assert!(names.iter().any(|name| name == "*D0"), "{names:?}");
    assert!(names.iter().any(|name| name == "*D1"), "{names:?}");
    let dimension = doc
        .entities()
        .find(|entity| matches!(entity, EntityType::Dimension(_)))
        .expect("dimension");
    let EntityType::Dimension(dimension) = dimension else {
        unreachable!()
    };
    assert_eq!(dimension.base().block_name, "*D1");
    assert_eq!(
        doc.audit()
            .count(crate::audit::AuditIssueCode::DuplicateHandle),
        0
    );
    let picture = doc.entities_in_block("*D1").collect::<Vec<_>>();
    assert_eq!(picture.len(), 1);
    let EntityType::Line(line) = picture[0] else {
        panic!("dimension picture lost its line");
    };
    assert_eq!(line.start, crate::types::Vector3::new(2.0, 0.0, 0.0));
}
