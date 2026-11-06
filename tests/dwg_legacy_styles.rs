//! Style dictionaries follow the version that introduced their objects.
//!
//! TABLESTYLE is kept from R2004 (AC1018). MLEADERSTYLE is kept from R2007
//! (AC1021), and also on R2004 when the MLEADERSTYLE class is already
//! declared. Earlier saves drop the dictionary and the style it holds.

use acadrust::objects::ObjectType;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::collections::BTreeMap;

const FIXTURE: &str = "tests/fixtures/dynamic-blocks/point.dwg";
const STYLES: [&str; 2] = ["ACAD_TABLESTYLE", "ACAD_MLEADERSTYLE"];

/// Style counts plus the named-objects entries that reach them.
fn styles(doc: &CadDocument) -> (usize, usize, BTreeMap<String, u64>) {
    let tables = doc
        .objects
        .values()
        .filter(|o| matches!(o, ObjectType::TableStyle(_)))
        .count();
    let leaders = doc
        .objects
        .values()
        .filter(|o| matches!(o, ObjectType::MultiLeaderStyle(_)))
        .count();

    let mut entries = BTreeMap::new();
    let root = doc.header.named_objects_dict_handle;
    if let Some(ObjectType::Dictionary(dictionary)) = doc.objects.get(&root) {
        for (name, handle) in &dictionary.entries {
            if STYLES.contains(&name.as_str()) {
                entries.insert(name.clone(), handle.value());
            }
        }
    }
    (tables, leaders, entries)
}

fn read(path: &str) -> CadDocument {
    DwgReader::from_file(path)
        .and_then(|mut reader| reader.read())
        .expect("read fixture")
}

fn write_at(doc: &CadDocument, version: DxfVersion) -> CadDocument {
    let mut document = doc.clone();
    document.version = version;
    let bytes = DwgWriter::write_to_vec(&document).expect("write");

    let path = std::env::temp_dir().join(format!("iss18_legacy_styles_{version}.dwg"));
    std::fs::write(&path, &bytes).expect("spill written bytes");
    let mut reader = DwgReader::from_file(&path).expect("reopen written bytes");
    let back = reader.read().expect("read back");
    let _ = std::fs::remove_file(&path);
    back
}

fn keeps(version: DxfVersion, name: &str, mleader_class: bool) -> bool {
    match name {
        "ACAD_TABLESTYLE" => version >= DxfVersion::AC1018,
        "ACAD_MLEADERSTYLE" => {
            version >= DxfVersion::AC1021 || (version >= DxfVersion::AC1018 && mleader_class)
        }
        _ => false,
    }
}

#[test]
fn legacy_writes_keep_styles_only_from_their_introduction_version() {
    let input = read(FIXTURE);
    let (want_tables, want_leaders, want_entries) = styles(&input);
    assert!(
        want_tables > 0 && want_leaders > 0 && want_entries.len() == STYLES.len(),
        "fixture must carry both styles: tables={want_tables} leaders={want_leaders} entries={want_entries:?}"
    );
    let mleader_class = input.classes.get_by_name("MLEADERSTYLE").is_some();

    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let back = write_at(&input, version);
        let (tables, leaders, entries) = styles(&back);
        let keep_table = keeps(version, "ACAD_TABLESTYLE", mleader_class);
        let keep_leader = keeps(version, "ACAD_MLEADERSTYLE", mleader_class);

        assert_eq!(
            tables,
            if keep_table { want_tables } else { 0 },
            "{version:?}: TABLESTYLE"
        );
        assert_eq!(
            leaders,
            if keep_leader { want_leaders } else { 0 },
            "{version:?}: MLEADERSTYLE"
        );
        for name in STYLES {
            let kept = keeps(version, name, mleader_class);
            assert_eq!(
                entries.get(name).copied(),
                kept.then(|| want_entries[name]),
                "{version:?}: {name}"
            );
            if let Some(value) = entries.get(name) {
                assert!(
                    back.objects.keys().any(|handle| handle.value() == *value),
                    "{version:?}: {name} entry points at {value:X}, which is not an object"
                );
            }
        }
    }
}

/// Pre-R2010 MLEADERSTYLE records carry ACAD_MLEADERVER xdata (1070 = 2);
/// ZWCAD 2025 drops an R2004 style without it as an invalid entry.
#[test]
fn legacy_mleader_styles_carry_their_record_version() {
    let doc = CadDocument::new();
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1024] {
        let back = write_at(&doc, version);
        let styles: Vec<_> = back
            .objects
            .iter()
            .filter(|(_, object)| matches!(object, ObjectType::MultiLeaderStyle(_)))
            .map(|(handle, _)| *handle)
            .collect();
        assert!(
            !styles.is_empty(),
            "{version:?}: Standard MLEADERSTYLE written"
        );
        for handle in styles {
            let record = back
                .object_xdata
                .get(&handle)
                .and_then(|data| data.get_record("ACAD_MLEADERVER"));
            if version < DxfVersion::AC1024 {
                let record = record.unwrap_or_else(|| panic!("{version:?}: ACAD_MLEADERVER"));
                assert_eq!(
                    record.values,
                    vec![acadrust::xdata::XDataValue::Integer16(2)],
                    "{version:?}"
                );
            } else {
                assert!(
                    record.is_none(),
                    "{version:?}: R2010 records name their version"
                );
            }
        }
    }
}
