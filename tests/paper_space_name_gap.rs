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

#[test]
fn add_layout_after_delete_goes_after_last_tab() {
    use acadrust::objects::ObjectType;
    let mut doc = CadDocument::new();
    for name in ["Layout2", "Layout3"] {
        doc.add_layout(name).unwrap();
    }
    // Drop Layout2 the way a caller deletes a layout: three tabs become two.
    let (handle, record) = doc
        .objects
        .iter()
        .find_map(|(h, o)| match o {
            ObjectType::Layout(l) if l.name == "Layout2" => Some((*h, l.block_record)),
            _ => None,
        })
        .unwrap();
    doc.objects.remove(&handle);
    let record = doc
        .block_records
        .iter()
        .find(|b| b.handle == record)
        .map(|b| b.name.clone())
        .unwrap();
    doc.block_records.remove(&record);

    doc.add_layout("Layout4").unwrap();
    let order = |name: &str| {
        doc.objects.values().find_map(|o| match o {
            ObjectType::Layout(l) if l.name == name => Some(l.tab_order),
            _ => None,
        })
    };
    assert!(order("Layout4") > order("Layout3"));
}
