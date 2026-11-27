//! Multi-view block records decode from the corpus drawing and rebuild
//! through the writers with their class bodies bit-exact (corpus gap #7,
//! Worker-RT-CORPUS pilot 2026-11-23: "21 in oracle (plant/mixing blocks),
//! none rebuilt").
//!
//! Oracle: Sections-Project-Objects.dwg — one `AEC_MVBLOCK_DEF` ("Eastern
//! White Pine", two views) and 21 `AEC_MVBLOCK_REF` placements.

use acadrust::classes::arch::{
    add_mv_block_def, add_mv_block_ref, MvBlockDef, MvBlockRef,
};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::objects::ObjectType;
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
    let path = std::env::temp_dir().join(format!("rt-arch-mvblocks-{}.dwg", std::process::id()));
    DwgWriter::write_to_file(&path, doc).expect("write the scratch DWG");
    let back = DwgReader::from_file(&path)
        .expect("reopen the scratch DWG")
        .read()
        .expect("parse the scratch DWG");
    let _ = std::fs::remove_file(&path);
    back
}

fn oracle_refs(doc: &CadDocument) -> Vec<MvBlockRef> {
    let mut v: Vec<_> = doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) => MvBlockRef::decode(a),
                _ => None,
            },
            _ => None,
        })
        .collect();
    v.sort_by_key(|r| r.guid.clone());
    v
}

fn oracle_defs(doc: &CadDocument) -> Vec<MvBlockDef> {
    let mut v: Vec<_> = doc
        .objects
        .values()
        .filter_map(|o| match o {
            ObjectType::Arch(a) => MvBlockDef::decode(a),
            _ => None,
        })
        .collect();
    v.sort_by_key(|d| d.name.clone());
    v
}

/// The definition and all 21 references decode with their recorded structure:
/// name, GUID, placement, the definition and view-block handles, texts, and
/// the recorded class-body run.
#[test]
fn the_oracle_mv_block_records_decode() {
    let Some(doc) = oracle() else { return };
    let defs = oracle_defs(&doc);
    assert_eq!(defs.len(), 1, "one definition");
    let def = &defs[0];
    assert_eq!(def.name, "Eastern White Pine");
    assert_eq!(def.views.len(), 2, "model and plan views");
    assert_eq!(def.texts.len(), 11, "the definition's text stream");
    assert!(def.body.bits >= 64, "the recorded class body");

    let refs = oracle_refs(&doc);
    assert_eq!(refs.len(), 21, "every reference decodes");
    for r in &refs {
        assert_eq!(r.name, "Eastern White Pine", "the definition name");
        assert!(r.guid.starts_with('{') && r.guid.ends_with('}'), "a GUID identifier");
        assert!(r.placement.rigid, "world-aligned placement");
        let known = matches!(doc.objects.get(&r.definition), Some(ObjectType::Arch(a)) if a.dxf_name == "AEC_MVBLOCK_DEF");
        assert!(known, "the reference names the definition record");
        assert_eq!(r.view_blocks.len(), 2, "plan and model view blocks");
        for block in &r.view_blocks {
            let name = doc
                .block_records
                .iter()
                .find(|record| record.handle == *block)
                .map(|record| record.name.clone());
            assert!(
                matches!(name.as_deref(), Some("I_TREE_T13_P") | Some("I_TREE_3D9_M")),
                "the view block resolves to the tree records, got {name:?}"
            );
        }
        assert_eq!(r.texts.len(), 9, "the reference's text stream");
        assert!(r.body.bits >= 64, "the recorded class body");
    }
    // The placements sit in the corridor's planting band, world-aligned.
    for r in &refs {
        assert!(r.placement.origin.x > 4000.0 && r.placement.origin.y > 3900.0);
        assert_eq!(r.placement.z_axis, acadrust::types::Vector3::new(0.0, 0.0, 1.0));
    }
}

/// Rebuilding the definition and all 21 references through the writers (into
/// the drawing, so the recorded handles stay the stored ones) and saving
/// gives records whose decode is the oracle's, class bodies bit for bit.
#[test]
fn the_oracle_mv_block_records_rebuild_bit_exact() {
    let Some(mut doc) = oracle() else { return };
    let defs = oracle_defs(&doc);
    let refs = oracle_refs(&doc);
    let _added_def = add_mv_block_def(&mut doc, &defs[0]).expect("write the definition");
    for r in &refs {
        add_mv_block_ref(&mut doc, r, None).expect("write the reference");
    }

    let back = roundtrip(&doc);
    // The rebuilt definition decodes as the oracle's (same name, views,
    // texts, body); it is a second record besides the stored one.
    let rebuilt = oracle_defs(&back);
    assert_eq!(rebuilt.len(), 2, "stored + rebuilt definition");
    assert!(rebuilt.contains(&defs[0]), "the rebuilt definition decodes as the oracle's");

    let mut back_refs = oracle_refs(&back);
    assert_eq!(back_refs.len(), 42, "stored + rebuilt references");
    for oracle_ref in &refs {
        let index = back_refs
            .iter()
            .position(|r| r.guid == oracle_ref.guid && r.body == oracle_ref.body)
            .unwrap_or_else(|| panic!("rebuilt reference {}", oracle_ref.guid));
        let rebuilt = back_refs.swap_remove(index);
        assert_eq!(&rebuilt, oracle_ref, "the rebuilt reference decodes as the oracle's");
        let known = matches!(back.objects.get(&rebuilt.definition), Some(ObjectType::Arch(a)) if a.dxf_name == "AEC_MVBLOCK_DEF");
        assert!(known, "the rebuilt reference's definition handle resolves");
    }
    // The rebuilt definition is listed under its name in the definitions
    // dictionary.
    let definitions = back.named_objects_dictionary().unwrap().get("AEC_MVBLOCK_DEFS").unwrap();
    let listed = match back.objects.get(&definitions) {
        Some(ObjectType::Dictionary(dict)) => dict
            .get("Eastern White Pine")
            .is_some_and(|h| matches!(back.objects.get(&h), Some(ObjectType::Arch(a)) if a.dxf_name == "AEC_MVBLOCK_DEF")),
        _ => false,
    };
    assert!(listed, "the definitions dictionary lists a definition");
}

/// A reference the typed fields cannot describe is refused, not invented: an
/// unknown definition, an unknown view block, a name/GUID that contradicts
/// the texts, or a class body that is not a recorded run.
#[test]
fn a_reference_outside_the_recorded_grammar_is_refused() {
    let Some(mut doc) = oracle() else { return };
    let defs = oracle_defs(&doc);
    let refs = oracle_refs(&doc);
    let real = &refs[0];

    let mut unknown_definition = real.clone();
    unknown_definition.definition = acadrust::types::Handle::new(0xDEAD);
    assert!(add_mv_block_ref(&mut doc, &unknown_definition, None).is_err());

    let mut unknown_block = real.clone();
    unknown_block.view_blocks[0] = acadrust::types::Handle::new(0xDEAD);
    assert!(add_mv_block_ref(&mut doc, &unknown_block, None).is_err());

    let mut wrong_texts = real.clone();
    wrong_texts.texts[1] = "{00000000-0000-0000-0000-000000000000}".into();
    assert!(add_mv_block_ref(&mut doc, &wrong_texts, None).is_err());

    let mut short_body = real.clone();
    short_body.body = acadrust::classes::arch::ClassBody { bytes: vec![0; 4], bits: 32 };
    assert!(add_mv_block_ref(&mut doc, &short_body, None).is_err());

    let mut unknown_block_def = defs[0].clone();
    unknown_block_def.views[0].block = acadrust::types::Handle::new(0xDEAD);
    assert!(add_mv_block_def(&mut doc, &unknown_block_def).is_err());

    let mut no_views = defs[0].clone();
    no_views.views.clear();
    assert!(add_mv_block_def(&mut doc, &no_views).is_err());

    // The stored records still rebuild after the refusals.
    assert!(add_mv_block_def(&mut doc, &defs[0]).is_ok());
    assert!(add_mv_block_ref(&mut doc, real, None).is_ok());
    let _ = &defs;
}
