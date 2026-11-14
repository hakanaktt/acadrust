//! What `CadDocument::add_entity` does with each entity it is given: the
//! handle it gets, the block record that lists it, and the order the document
//! keeps. Reserving room first with `CadDocument::reserve_entities` must not
//! change any of it.

use acadrust::entities::{AttributeEntity, EntityType, Line};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::CadDocument;
use std::ops::Range;

fn line(i: usize) -> Line {
    let x = i as f64;
    Line::from_points(Vector3::new(x, 0.0, 0.0), Vector3::new(x, 1.0, 0.0))
}

/// A named block record with its own handles, as an application builds one.
fn add_block(doc: &mut CadDocument, name: &str) -> Handle {
    let mut record = BlockRecord::new(name);
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let handle = record.handle;
    doc.block_records.add(record).unwrap();
    handle
}

/// Everything `add_entity` decides, in the order the document keeps it.
#[derive(Debug, PartialEq)]
struct Layout {
    /// `(handle, owner)` of every entity, in document order.
    entities: Vec<(Handle, Handle)>,
    /// `(record name, listed entity handles)` of every block record.
    records: Vec<(String, Vec<Handle>)>,
}

fn layout(doc: &CadDocument) -> Layout {
    Layout {
        entities: doc
            .entities()
            .map(|e| (e.common().handle, e.common().owner_handle))
            .collect(),
        records: doc
            .block_records
            .iter()
            .map(|r| (r.name.clone(), r.entity_handles.clone()))
            .collect(),
    }
}

/// A mixed run of entities: model space, paper space, two named blocks, an
/// owner no record has, an entity with its own handle, and an attribute.
fn add_mixed(doc: &mut CadDocument, a: Handle, b: Handle, range: Range<usize>) -> Vec<Handle> {
    let mut handles = Vec::new();
    for i in range {
        let mut entity = line(i);
        match i % 7 {
            0 => {}
            1 => entity.common.owner_handle = a,
            2 => entity.common.owner_handle = b,
            3 => entity.common.entity_mode = Some(1),
            4 => entity.common.owner_handle = Handle::new(0xFFFF_0000 + i as u64),
            5 => entity.common.handle = Handle::new(0x10_0000 + i as u64),
            _ => {
                let attribute = AttributeEntity::new(format!("T{i}"), "v".to_string());
                handles.push(
                    doc.add_entity(EntityType::AttributeEntity(attribute))
                        .unwrap(),
                );
                continue;
            }
        }
        handles.push(doc.add_entity(EntityType::Line(entity)).unwrap());
    }
    handles
}

fn mixed_document(n: usize) -> (CadDocument, Vec<Handle>) {
    mixed_document_reserving(n, None)
}

/// [`mixed_document`], with `reserve` passed to `reserve_entities` after the
/// block records are added and before the first entity.
fn mixed_document_reserving(n: usize, reserve: Option<usize>) -> (CadDocument, Vec<Handle>) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let a = add_block(&mut doc, "A");
    let b = add_block(&mut doc, "B");
    if let Some(additional) = reserve {
        doc.reserve_entities(additional);
    }
    let handles = add_mixed(&mut doc, a, b, 0..n);
    (doc, handles)
}

#[test]
fn add_entity_lists_each_entity_under_its_owner() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let a = add_block(&mut doc, "A");
    let ms = doc.header.model_space_block_handle;
    let ps = doc.header.paper_space_block_handle;

    let plain = doc.add_entity(EntityType::Line(line(0))).unwrap();
    let mut owned = line(1);
    owned.common.owner_handle = a;
    let owned = doc.add_entity(EntityType::Line(owned)).unwrap();
    let mut paper = line(2);
    paper.common.entity_mode = Some(1);
    let paper = doc.add_entity(EntityType::Line(paper)).unwrap();
    let mut stray = line(3);
    stray.common.owner_handle = Handle::new(0xFFFF_0000);
    let stray = doc.add_entity(EntityType::Line(stray)).unwrap();
    let attribute = AttributeEntity::new("TAG".to_string(), "v".to_string());
    let attribute = doc
        .add_entity(EntityType::AttributeEntity(attribute))
        .unwrap();

    let owner = |h| doc.get_entity(h).unwrap().common().owner_handle;
    assert_eq!(owner(plain), ms);
    assert_eq!(owner(owned), a);
    assert_eq!(owner(paper), ps);
    // An owner no record has falls back to model space, and the entity's
    // owner is set to match.
    assert_eq!(owner(stray), ms);

    let listed = |name: &str| doc.block_records.get(name).unwrap().entity_handles.clone();
    assert_eq!(listed("*Model_Space"), vec![plain, stray]);
    assert_eq!(listed("A"), vec![owned]);
    assert_eq!(listed("*Paper_Space"), vec![paper]);
    // An attribute is reachable by handle but listed by no record.
    assert!(doc.get_entity(attribute).is_some());
    assert!(doc
        .block_records
        .iter()
        .all(|r| !r.entity_handles.contains(&attribute)));
}

#[test]
fn add_entity_keeps_order_and_handles_of_a_long_run() {
    let (doc, handles) = mixed_document(20_000);
    let in_order: Vec<Handle> = doc.entities().map(|e| e.common().handle).collect();
    assert_eq!(in_order, handles);
    for (i, handle) in handles.iter().enumerate() {
        let entity = doc.get_entity(*handle).unwrap();
        assert_eq!(entity.common().handle, *handle, "entity {i}");
    }
    // An entity's own handle is kept, and later handles are allocated above it.
    assert_eq!(handles[5], Handle::new(0x10_0005));
    let fresh = handles[7];
    assert!(fresh.value() > 0x10_0005, "{fresh:?}");
    let mut sorted = handles.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), handles.len(), "duplicate handles");
}

#[test]
fn reserve_entities_leaves_the_same_document() {
    const N: usize = 20_000;
    let (plain, plain_handles) = mixed_document(N);
    let expected = layout(&plain);
    // Exactly enough, too little, far too much, and nothing at all.
    for reserve in [N, N / 3, N * 4, 0] {
        let (reserved, handles) = mixed_document_reserving(N, Some(reserve));
        assert_eq!(handles, plain_handles, "reserve {reserve}");
        assert_eq!(layout(&reserved), expected, "reserve {reserve}");
        for handle in &handles {
            assert_eq!(
                reserved.get_entity(*handle).map(|e| e.common().handle),
                Some(*handle),
                "reserve {reserve}"
            );
        }
    }
}

#[test]
fn reserve_entities_on_a_document_that_has_entities() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let a = add_block(&mut doc, "A");
    let b = add_block(&mut doc, "B");
    let mut handles = add_mixed(&mut doc, a, b, 0..1_000);
    let mut twin = CadDocument::with_version(DxfVersion::AC1032);
    let ta = add_block(&mut twin, "A");
    let tb = add_block(&mut twin, "B");
    let mut twin_handles = add_mixed(&mut twin, ta, tb, 0..1_000);

    doc.reserve_entities(5_000);
    handles.extend(add_mixed(&mut doc, a, b, 1_000..6_000));
    twin_handles.extend(add_mixed(&mut twin, ta, tb, 1_000..6_000));

    assert_eq!(handles, twin_handles);
    assert_eq!(layout(&doc), layout(&twin));
    let in_order: Vec<Handle> = doc.entities().map(|e| e.common().handle).collect();
    assert_eq!(in_order, handles);
}
