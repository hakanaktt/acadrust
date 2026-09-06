//! Regression test for issue #268 (via OpenCADStudio): an xref block must
//! round-trip through DWG as an *external reference*, not as baked-in geometry.
//!
//! Some hosts (e.g. OCS) merge the resolved contents of an xref into its block
//! record so they can be displayed. Those entities must never be serialized as
//! owned entities of the xref block — doing so binds/explodes the xref into the
//! host file, so on the next open the reference is gone and only loose objects
//! remain. The writer now skips owned-entity output for `is_xref` block records
//! (and no longer writes their R2004+ owned-handle list, which also desynced
//! the handle stream).

use std::io::Cursor;

use acadrust::entities::*;
use acadrust::tables::BlockRecord;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn dwg_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("DWG write failed");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed")
}

#[test]
fn xref_block_does_not_bind_owned_entities() {
    for version in [
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut doc = CadDocument::with_version(version);

        // A genuine model-space entity that must survive untouched — a canary
        // for handle-stream desync corruption around the xref block.
        doc.add_entity(EntityType::Circle(Circle::from_coords(5.0, 5.0, 0.0, 3.0)))
            .unwrap();

        // An xref block record with the flag + path, exactly as XATTACH creates
        // it before resolution.
        let mut br = BlockRecord::new("XREF_UNIT");
        br.handle = doc.allocate_handle();
        br.block_entity_handle = doc.allocate_handle();
        br.block_end_handle = doc.allocate_handle();
        br.flags.is_xref = true;
        br.xref_path = "./OCS_xref_file.dwg".to_string();
        let br_handle = br.handle;
        doc.block_records.add(br).unwrap();

        // Simulate the host merging the resolved xref geometry into the block
        // record for display: several entities owned by the xref block.
        for i in 0..5 {
            let mut e =
                EntityType::Line(Line::from_coords(0.0, i as f64, 0.0, 10.0, i as f64, 0.0));
            e.common_mut().owner_handle = br_handle;
            doc.add_entity(e).unwrap();
        }

        // Precondition: the merge actually populated the block's owned list.
        let owned_before = doc
            .block_records
            .iter()
            .find(|b| b.name == "XREF_UNIT")
            .expect("xref block missing before save")
            .entity_handles
            .len();
        assert_eq!(
            owned_before, 5,
            "{version:?}: precondition — merged entities should be present before save"
        );

        let rt = dwg_roundtrip(&doc);

        // The xref block must survive as a reference.
        let xref = rt
            .block_records
            .iter()
            .find(|b| b.name == "XREF_UNIT")
            .unwrap_or_else(|| panic!("{version:?}: xref block record lost on roundtrip"));
        assert!(
            xref.flags.is_xref,
            "{version:?}: is_xref flag lost — the xref was bound/exploded"
        );
        assert_eq!(
            xref.xref_path, "./OCS_xref_file.dwg",
            "{version:?}: xref path not preserved"
        );
        assert!(
            xref.entity_handles.is_empty(),
            "{version:?}: xref block still owns {} entities — resolved geometry was baked into the file",
            xref.entity_handles.len()
        );

        // None of the merged lines may have been serialized anywhere.
        let leaked = rt
            .entities()
            .filter(|e| matches!(e, EntityType::Line(_)))
            .count();
        assert_eq!(
            leaked, 0,
            "{version:?}: {leaked} resolved xref line(s) leaked into the saved file"
        );

        // The genuine model-space circle must survive intact (no desync).
        let circles = rt
            .entities()
            .filter(|e| matches!(e, EntityType::Circle(_)))
            .count();
        assert_eq!(
            circles, 1,
            "{version:?}: model-space circle lost — the handle stream desynced"
        );
    }
}
