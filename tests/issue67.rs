//! Regression test for issue #67: legacy space aliases on BLOCK markers
//! must resolve to the canonical BLOCK_RECORD entries.

use std::collections::HashSet;
use std::io::Cursor;

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DxfReader, DxfWriter};

#[derive(Debug)]
struct DxfPair {
    code: i32,
    value: String,
}

fn parse_ascii_dxf(bytes: Vec<u8>) -> Vec<DxfPair> {
    let text = String::from_utf8(bytes).expect("ASCII DXF is valid UTF-8");
    let mut lines = text.lines();
    let mut pairs = Vec::new();

    while let Some(code) = lines.next() {
        let value = lines.next().expect("group code has a value line");
        pairs.push(DxfPair {
            code: code.trim().parse().expect("valid DXF group code"),
            value: value.to_string(),
        });
    }

    pairs
}

fn serialize_ascii_dxf(pairs: &[DxfPair]) -> Vec<u8> {
    let mut text = String::new();
    for pair in pairs {
        text.push_str(&format!("{:>6}\r\n{}\r\n", pair.code, pair.value));
    }
    text.into_bytes()
}

fn blocks_section_bounds(pairs: &[DxfPair]) -> (usize, usize) {
    let start = pairs
        .windows(2)
        .position(|window| {
            window[0].code == 0
                && window[0].value == "SECTION"
                && window[1].code == 2
                && window[1].value == "BLOCKS"
        })
        .expect("BLOCKS section")
        + 2;
    let end = pairs[start..]
        .iter()
        .position(|pair| pair.code == 0 && pair.value == "ENDSEC")
        .expect("end of BLOCKS section")
        + start;
    (start, end)
}

fn make_legacy_space_block_fixture() -> Vec<u8> {
    let source = CadDocument::with_version(DxfVersion::AC1015);
    assert_eq!(source.block_records.len(), 2);
    assert!(source.block_records.get("*Model_Space").is_some());
    assert!(source.block_records.get("*Paper_Space").is_some());

    let mut pairs = parse_ascii_dxf(
        DxfWriter::new(&source)
            .write_to_vec()
            .expect("write canonical source DXF"),
    );
    let (start, end) = blocks_section_bounds(&pairs);
    let mut source_marker = "";
    let mut source_block_handles = Vec::new();
    for pair in &pairs[start..end] {
        if pair.code == 0 {
            source_marker = match pair.value.as_str() {
                "BLOCK" => "BLOCK",
                "ENDBLK" => "ENDBLK",
                _ => "",
            };
        } else if source_marker == "BLOCK" && pair.code == 5 {
            source_block_handles.push(pair.value.clone());
        }
    }
    assert_eq!(source_block_handles.len(), 2);

    let mut marker_type = "";
    let mut block_number = 0usize;
    let mut alias_count = 0;
    let mut removed_handle_count = 0;

    let mut mutated = Vec::with_capacity(pairs.len());
    for (index, mut pair) in pairs.drain(..).enumerate() {
        if (start..end).contains(&index) && pair.code == 0 {
            marker_type = match pair.value.as_str() {
                "BLOCK" => {
                    block_number += 1;
                    "BLOCK"
                }
                "ENDBLK" => "ENDBLK",
                _ => "",
            };
        }

        if (start..end).contains(&index) && marker_type == "BLOCK" && pair.code == 2 {
            pair.value = match pair.value.as_str() {
                "*Model_Space" => {
                    alias_count += 1;
                    "$MODEL_SPACE".to_string()
                }
                "*Paper_Space" => {
                    alias_count += 1;
                    "$PAPER_SPACE".to_string()
                }
                _ => pair.value,
            };
        }

        if (start..end).contains(&index) && marker_type == "BLOCK" && pair.code == 5 {
            removed_handle_count += 1;
            continue;
        }
        if (start..end).contains(&index)
            && marker_type == "ENDBLK"
            && block_number == 1
            && pair.code == 5
        {
            // Reuse the now-absent paper-space BLOCK handle for the model-space
            // ENDBLK, exactly like the reporter's valid source. A reader that
            // retains the synthesized paper BLOCK handle creates a duplicate.
            pair.value = source_block_handles[1].clone();
        }
        mutated.push(pair);
    }

    assert_eq!(alias_count, 2, "mutated both space BLOCK names");
    assert_eq!(
        removed_handle_count, 2,
        "removed the BLOCK handles for both spaces"
    );

    let (start, end) = blocks_section_bounds(&mutated);
    let alternate_names: Vec<_> = mutated[start..end]
        .iter()
        .filter(|pair| pair.code == 3)
        .map(|pair| pair.value.as_str())
        .collect();
    assert_eq!(alternate_names, ["*Model_Space", "*Paper_Space"]);

    serialize_ascii_dxf(&mutated)
}

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("create DXF reader")
        .read()
        .expect("read DXF")
}

fn assert_canonical_space_records(document: &CadDocument) {
    assert_eq!(
        document.block_records.len(),
        2,
        "legacy BLOCK aliases must not create duplicate block records"
    );
    assert!(document.block_records.get("$MODEL_SPACE").is_none());
    assert!(document.block_records.get("$PAPER_SPACE").is_none());

    let mut identities = HashSet::new();
    for name in ["*Model_Space", "*Paper_Space"] {
        let record = document
            .block_records
            .get(name)
            .unwrap_or_else(|| panic!("missing canonical {name} block record"));
        assert!(
            !record.block_entity_handle.is_null(),
            "{name} BLOCK marker needs a repaired handle"
        );
        assert!(
            !record.block_end_handle.is_null(),
            "{name} ENDBLK marker needs a repaired handle"
        );
        for (kind, handle) in [
            ("BLOCK_RECORD", record.handle),
            ("BLOCK", record.block_entity_handle),
            ("ENDBLK", record.block_end_handle),
        ] {
            assert!(
                identities.insert(handle),
                "duplicate {kind} identity {handle:?} in {name}"
            );
        }
    }
}

#[test]
fn legacy_space_block_aliases_and_missing_block_handles_round_trip_canonically() {
    let loaded = read_dxf(make_legacy_space_block_fixture());
    assert_canonical_space_records(&loaded);

    let output = DxfWriter::new(&loaded)
        .write_to_vec()
        .expect("write normalized DXF");
    let output_pairs = parse_ascii_dxf(output.clone());
    let (blocks_start, blocks_end) = blocks_section_bounds(&output_pairs);
    assert_eq!(
        output_pairs[blocks_start..blocks_end]
            .iter()
            .filter(|pair| pair.code == 0 && pair.value == "BLOCK")
            .count(),
        2,
        "writer must emit only the canonical space definitions"
    );

    let mut output_handles = HashSet::new();
    for pair in output_pairs
        .iter()
        .filter(|pair| matches!(pair.code, 5 | 105))
    {
        let value = u64::from_str_radix(pair.value.trim(), 16).expect("valid output handle");
        assert_ne!(value, 0, "writer emitted a null identity handle");
        assert!(
            output_handles.insert(value),
            "writer emitted duplicate identity handle #{value:X}"
        );
    }

    assert!(
        !output_pairs
            .iter()
            .any(|pair| pair.code == 5 && pair.value.trim() == "0"),
        "writer emitted a null handle as group code 5/0"
    );
    assert!(
        !output_pairs.iter().any(|pair| {
            pair.value.eq_ignore_ascii_case("$MODEL_SPACE")
                || pair.value.eq_ignore_ascii_case("$PAPER_SPACE")
        }),
        "writer preserved a legacy space alias"
    );

    assert_canonical_space_records(&read_dxf(output));
}
