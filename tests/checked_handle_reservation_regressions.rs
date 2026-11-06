use acadrust::{types::Handle, CadDocument};

fn reserve_for_reconstruction(document: &mut CadDocument) -> Handle {
    document.try_reserve_handles(1).unwrap().get(0).unwrap()
}

#[test]
fn reconstruction_does_not_reissue_a_directly_inserted_identity() {
    use acadrust::tables::Layer;
    let mut d = CadDocument::new();
    let occupied = Handle::new(d.next_handle());
    let mut layer = Layer::new("occupied");
    layer.handle = occupied;
    d.layers.add(layer).unwrap();
    assert_ne!(reserve_for_reconstruction(&mut d), occupied);
}

#[test]
fn reconstruction_does_not_bind_an_unrelated_dangling_xrecord_target() {
    use acadrust::objects::{ObjectType, XRecord, XRecordEntry};
    let mut d = CadDocument::new();
    let own = d.allocate_handle();
    let target = Handle::new(d.next_handle());
    let mut x = XRecord::named("guard");
    x.handle = own;
    x.owner = d.header.named_objects_dict_handle;
    x.add_entry(XRecordEntry::handle(340, target));
    d.objects.insert(own, ObjectType::XRecord(x));
    d.named_objects_dictionary_mut()
        .unwrap()
        .add_entry("guard", own);
    assert_eq!(d.audit().error_count(), 0);
    assert_ne!(reserve_for_reconstruction(&mut d), target);
}
