//! The native block records of the corpus drawing round-trip: definitions,
//! member entities, and inserts are written back as stored (corpus gap #6,
//! Worker-RT-CORPUS pilot 2026-11-23: "13 block refs in oracle, none rebuilt"
//! — the writer path was untested, not absent).
//!
//! Oracle: Sections-Project-Objects.dwg — 64 block records, 19 inserts in
//! model space, 659 member entities.

use acadrust::entities::EntityType;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn oracle() -> Option<CadDocument> {
    let dir = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR")?;
    let path = std::path::Path::new(&dir).join("Sections-Project-Objects.dwg");
    let mut doc = DwgReader::from_file(&path)
        .expect("open the oracle")
        .read()
        .expect("parse the oracle");
    doc.version = DxfVersion::AC1032;
    Some(doc)
}

fn roundtrip(doc: &CadDocument) -> CadDocument {
    let path = std::env::temp_dir().join(format!("rt-native-blocks-{}.dwg", std::process::id()));
    DwgWriter::write_to_file(&path, doc).expect("write the scratch DWG");
    let back = DwgReader::from_file(&path)
        .expect("reopen the scratch DWG")
        .read()
        .expect("parse the scratch DWG");
    let _ = std::fs::remove_file(&path);
    back
}

/// Block records: name, member count, base point — as stored, sorted.
fn record_snapshot(doc: &CadDocument) -> Vec<(String, usize, u64, u64, u64)> {
    let mut v: Vec<_> = doc
        .block_records
        .iter()
        .map(|r| {
            (
                r.name.clone(),
                r.entity_handles.len(),
                r.base_point.x.to_bits(),
                r.base_point.y.to_bits(),
                r.base_point.z.to_bits(),
            )
        })
        .collect();
    v.sort();
    v
}

/// Inserts: handle, block, placement and scale bits — as stored, sorted.
fn insert_snapshot(doc: &CadDocument) -> Vec<(u64, String, u64, u64, u64, u64, u64, u64, u64)> {
    let mut v: Vec<_> = doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Insert(i) => Some((
                i.common.handle.value(),
                i.block_name.clone(),
                i.insert_point.x.to_bits(),
                i.insert_point.y.to_bits(),
                i.insert_point.z.to_bits(),
                i.rotation.to_bits(),
                i.x_scale().to_bits(),
                i.y_scale().to_bits(),
                i.z_scale().to_bits(),
            )),
            _ => None,
        })
        .collect();
    v.sort();
    v
}

/// Member entities of every block record: handle and type — as stored.
fn member_snapshot(doc: &CadDocument) -> Vec<(u64, String)> {
    let mut v: Vec<_> = Vec::new();
    for r in doc.block_records.iter() {
        for &h in &r.entity_handles {
            let name = match doc.get_entity(h) {
                Some(e) => match e {
                    EntityType::Extended(x) => format!("EXT:{}", x.class_name()),
                    other => other.as_entity().entity_type().to_string(),
                },
                None => "<missing>".to_string(),
            };
            v.push((h.value(), name));
        }
    }
    v.sort();
    v
}

#[test]
fn the_oracle_block_definitions_and_inserts_roundtrip() {
    let Some(oracle) = oracle() else { return };
    assert_eq!(oracle.block_records.len(), 64, "the oracle's block records");
    let inserts = insert_snapshot(&oracle);
    assert_eq!(inserts.len(), 19, "the oracle's inserts in model space");
    let members = member_snapshot(&oracle);
    assert_eq!(members.len(), 659, "member entities across the records");

    let back = roundtrip(&oracle);
    assert_eq!(
        record_snapshot(&oracle),
        record_snapshot(&back),
        "block records round-trip: names, member counts, base points bit-exact"
    );
    assert_eq!(inserts, insert_snapshot(&back), "inserts round-trip bit-exact");
    assert_eq!(members, member_snapshot(&back), "member entities round-trip");

    // The multi-view block view blocks the corpus references point at.
    for name in ["I_TREE_T13_P", "I_TREE_3D9_M"] {
        let record = back
            .block_records
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("view block {name} survives"));
        assert!(!record.entity_handles.is_empty(), "view block {name} keeps its members");
    }
}

/// The AEC records the same save writes back keep their stored bodies: the
/// multi-view block references and definition survive byte for byte except
/// the trailing pad byte of the handle stream (the crate zero-pads, the
/// oracle's last byte is 0xFF/0x87) — decode-neutral.
#[test]
fn the_aec_mv_block_records_survive_the_same_save() {
    let Some(oracle) = oracle() else { return };
    let back = roundtrip(&oracle);

    let raws = |doc: &CadDocument| -> Vec<(u64, Vec<u8>)> {
        let mut v: Vec<_> = doc
            .entities()
            .filter_map(|e| match e {
                EntityType::Extended(x) => match &x.data {
                    acadrust::entities::ExtendedEntityData::Arch(a)
                        if a.dxf_name == "AEC_MVBLOCK_REF" =>
                    {
                        Some((x.common.handle.value(), a.raw_dwg_data.clone().unwrap_or_default()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        v.sort();
        v
    };
    let (oracle_refs, back_refs) = (raws(&oracle), raws(&back));
    assert_eq!(oracle_refs.len(), 21, "the oracle's multi-view block references");
    assert_eq!(oracle_refs.len(), back_refs.len(), "all references survive the save");
    for ((h, a), (_, b)) in oracle_refs.iter().zip(back_refs.iter()) {
        assert_eq!(a.len(), b.len(), "ref {h:X} keeps its length");
        assert_eq!(&a[..a.len() - 1], &b[..b.len() - 1], "ref {h:X} keeps its body");
        assert_eq!(a[a.len() - 1] & 0x80, b[b.len() - 1] & 0x80, "ref {h:X} pad bit");
    }

    let def = |doc: &CadDocument| -> Vec<u8> {
        doc.objects
            .values()
            .find_map(|o| match o {
                acadrust::objects::ObjectType::Arch(a) if a.dxf_name == "AEC_MVBLOCK_DEF" => {
                    a.raw_dwg_data.clone()
                }
                _ => None,
            })
            .expect("the definition survives")
    };
    let (a, b) = (def(&oracle), def(&back));
    assert_eq!(a.len(), b.len(), "the definition keeps its length");
    assert_eq!(&a[..a.len() - 1], &b[..b.len() - 1], "the definition keeps its body");
}
