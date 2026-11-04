use std::io::Cursor;

use acadrust::entities::EntityType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, SemanticEntityV1, SemanticPartV1};

/// A drawing without an `AcadAnnotative` application id (older DWGs carry
/// none) whose *Paper_Space ENDBLK sits on the handle a fresh document gives
/// that default entry (acadrust #136). The reader seeds the default before the
/// file is read.
fn drawing_with_end_marker_on_a_seeded_default_handle(
    version: DxfVersion,
) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let seeded = document.app_ids.get("AcadAnnotative").unwrap().handle;
    document.app_ids.remove("AcadAnnotative").unwrap();
    let paper = document.header.paper_space_block_handle;
    let record = document
        .block_records
        .iter_mut()
        .find(|record| record.handle == paper)
        .unwrap();
    record.block_end_handle = seeded;
    (document, seeded)
}

#[test]
fn paper_space_keeps_its_own_end_marker() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1032] {
        let (document, end) = drawing_with_end_marker_on_a_seeded_default_handle(version);
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

        let paper = read.header.paper_space_block_handle;
        let record = read
            .block_records
            .iter()
            .find(|record| record.handle == paper)
            .unwrap();
        assert_eq!(record.block_end_handle, end, "{version:?}: record end marker");

        let mut ends = Vec::new();
        read.semantic_inventory_v1().visit(|part| {
            if let SemanticPartV1::Entity(SemanticEntityV1::Typed(EntityType::BlockEnd(marker))) =
                part
            {
                if marker.common.owner_handle == paper {
                    ends.push(marker.common.handle);
                }
            }
        });
        assert_eq!(ends, vec![end], "{version:?}: the stored ENDBLK it owns");
    }
}
