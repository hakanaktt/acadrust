use acadrust::tables::BlockRecord;
use acadrust::CadDocument;

#[test]
fn add_layout_skips_taken_paper_space_name() {
    let mut doc = CadDocument::new();
    // Gap: *Paper_Space exists, *Paper_Space0 does not, *Paper_Space1 does.
    let mut stray = BlockRecord::new("*Paper_Space1");
    stray.handle = doc.allocate_handle();
    stray.block_entity_handle = doc.allocate_handle();
    stray.block_end_handle = doc.allocate_handle();
    doc.block_records.add(stray).unwrap();

    doc.add_layout("Layout2")
        .expect("add_layout must pick a free *Paper_Space name");
    assert!(doc.block_records.get("*Paper_Space0").is_some());
}
