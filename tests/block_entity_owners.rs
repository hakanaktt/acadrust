//! An entity in a block's entity list is owned by that block in DWG, whatever
//! owner handle it came in with. Group 330 of an entity inside a DXF BLOCK can
//! name `*Model_Space`: the reader keeps the entity in the block, but the DWG
//! writer took the entity mode from that owner and wrote the entity in
//! model-space mode (2) inside the block's list. This crate's reader places
//! entities by the list and does not notice; a reader that goes by the mode
//! drops the entity from the block.
use acadrust::entities::{Circle, Insert, Line, MText};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, EntityType, Handle};
use std::io::Cursor;

/// An empty block record `name`, with handles for its BLOCK and ENDBLK.
fn add_block(document: &mut CadDocument, name: &str) -> Handle {
    let mut record = BlockRecord::new(name);
    record.handle = document.allocate_handle();
    record.block_entity_handle = document.allocate_handle();
    record.block_end_handle = document.allocate_handle();
    let handle = record.handle;
    document.block_records.add(record).unwrap();
    handle
}

/// `entities` added to the entity list of the block record `block`.
fn add_to_block(
    document: &mut CadDocument,
    block: Handle,
    entities: Vec<EntityType>,
) -> Vec<Handle> {
    entities
        .into_iter()
        .map(|mut entity| {
            entity.common_mut().owner_handle = block;
            document.add_entity(entity).unwrap()
        })
        .collect()
}

fn insert(name: &str) -> EntityType {
    EntityType::Insert(Insert::new(name, Vector3::new(0.0, 0.0, 0.0)))
}

/// Block `B` holding a LINE, a CIRCLE and an MTEXT, inserted in model space.
fn drawing(version: DxfVersion) -> (CadDocument, Vec<Handle>) {
    let mut document = CadDocument::with_version(version);
    let block = add_block(&mut document, "B");
    let members = add_to_block(
        &mut document,
        block,
        vec![
            EntityType::Line(Line::from_points(
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(10.0, 0.0, 0.0),
            )),
            EntityType::Circle(Circle::from_center_radius(Vector3::new(5.0, 5.0, 0.0), 2.5)),
            EntityType::MText(Box::new(MText::with_value(
                "note",
                Vector3::new(0.0, 8.0, 0.0),
            ))),
        ],
    );
    document.add_entity(insert("B")).unwrap();
    (document, members)
}

fn record(document: &CadDocument, name: &str) -> Handle {
    document.block_records.get(name).unwrap().handle
}

/// The owner handle of `members` pointed at `owner`; they stay in their
/// block's entity list.
fn set_owner(document: &mut CadDocument, members: &[Handle], owner: Handle) {
    for handle in members {
        document
            .get_entity_mut(*handle)
            .unwrap()
            .common_mut()
            .owner_handle = owner;
    }
}

fn via_dwg(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

/// `members` came back from DWG as entities of `B`: listed by it and owned by
/// it explicitly (mode 0).
fn assert_written_in_block(back: &CadDocument, members: &[Handle], context: &str) {
    let block = back.block_records.get("B").unwrap();
    let stray: Vec<_> = members
        .iter()
        .filter_map(|handle| {
            let common = back.get_entity(*handle).unwrap().common();
            let found = (common.owner_handle, common.entity_mode);
            let listed = block.entity_handles.contains(handle);
            (found != (block.handle, Some(0)) || !listed).then_some((*handle, found, listed))
        })
        .collect();
    assert!(
        stray.is_empty(),
        "{context}: {} of {} entities were not written in their block {:?}, \
         first (handle, (owner, mode), listed): {:?}",
        stray.len(),
        members.len(),
        block.handle,
        stray.first()
    );
}

/// `text` with group 330 of the `entities` records changed from `from` to `to`.
fn with_owner(text: &str, entities: &[Handle], from: Handle, to: Handle) -> String {
    let hex = |handle: Handle| format!("{:X}", handle.value());
    let entities: Vec<String> = entities.iter().map(|h| hex(*h)).collect();
    let (from, to) = (hex(from), hex(to));
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::with_capacity(text.len());
    let mut member = false;
    for pair in lines.chunks(2) {
        let code = pair[0].trim();
        let mut value = pair.get(1).copied().unwrap_or_default();
        match code {
            "0" => member = false,
            "5" => member = entities.iter().any(|h| h == value.trim()),
            "330" if member && value.trim() == from => value = &to,
            _ => {}
        }
        out.push_str(pair[0]);
        out.push('\n');
        out.push_str(value);
        out.push('\n');
    }
    out
}

/// The reported case: a DXF whose block entities name `*Model_Space` in 330.
#[test]
fn dxf_block_entities_owned_by_model_space_are_written_in_their_block() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let (source, members) = drawing(version);
        let model = record(&source, "*Model_Space");
        let text = String::from_utf8(DxfWriter::new(&source).write_to_vec().unwrap()).unwrap();
        let text = with_owner(&text, &members, record(&source, "B"), model);
        let from_dxf = DxfReader::from_reader(Cursor::new(text.into_bytes()))
            .unwrap()
            .read()
            .unwrap();
        for handle in &members {
            assert_eq!(
                from_dxf.get_entity(*handle).unwrap().common().owner_handle,
                model,
                "the DXF repro must carry the model-space owner"
            );
        }

        assert_written_in_block(&via_dwg(&from_dxf), &members, &format!("{version:?}"));
    }
}

/// The same through the API, with model space or paper space as the owner.
#[test]
fn block_entities_owned_by_another_space_are_written_in_their_block() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        for space in ["*Model_Space", "*Paper_Space"] {
            let (mut source, members) = drawing(version);
            let owner = record(&source, space);
            set_owner(&mut source, &members, owner);

            let context = format!("{version:?}, owner {space}");
            assert_written_in_block(&via_dwg(&source), &members, &context);
        }
    }
}

/// A block long enough for the parallel entity writer (R2004 and later): its
/// first 1,103 plain entities are written in parallel, then an INSERT and the
/// 50 lines after it one by one.
#[test]
fn entities_of_a_large_block_owned_by_model_space_are_written_in_it() {
    let (mut source, mut members) = drawing(DxfVersion::AC1032);
    add_block(&mut source, "C");
    let line = |i: i32| {
        let x = f64::from(i);
        EntityType::Line(Line::from_points(
            Vector3::new(x, 0.0, 0.0),
            Vector3::new(x, 1.0, 0.0),
        ))
    };
    let mut entities: Vec<EntityType> = (0..1_150).map(line).collect();
    entities.insert(1_100, insert("C"));
    let block = record(&source, "B");
    members.extend(add_to_block(&mut source, block, entities));
    let model = record(&source, "*Model_Space");
    set_owner(&mut source, &members, model);

    assert_written_in_block(&via_dwg(&source), &members, "R2018");
}

/// An entity read from DWG keeps the record it was read from, and with it the
/// model-space mode. Moved into a block's list, it must be written in the
/// block all the same, not copied as read.
#[test]
fn an_entity_read_from_dwg_and_moved_into_a_block_is_written_in_it() {
    let (mut source, mut members) = drawing(DxfVersion::AC1032);
    let line = source
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::new(0.0, 20.0, 0.0),
            Vector3::new(10.0, 20.0, 0.0),
        )))
        .unwrap();
    let mut read = via_dwg(&source);
    assert!(
        read.get_entity(line).unwrap().common().raw_record.is_some(),
        "the read LINE must keep its verbatim record"
    );
    let records = &mut read.block_records;
    let model = records.get_mut("*Model_Space").unwrap();
    model.entity_handles.retain(|handle| *handle != line);
    records.get_mut("B").unwrap().entity_handles.push(line);
    members.push(line);

    assert_written_in_block(&via_dwg(&read), &members, "R2018");
}
