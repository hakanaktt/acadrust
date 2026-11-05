//! A block reference's SEQEND never shares its handle with another record in the written DWG.
//!
//! A reference copied in an application keeps the SEQEND handle of its source: the copy gets new
//! ATTRIB handles, and nothing renews that one. Two records were then written under one handle
//! and the reader kept one, so a filled title block came back one attribute short.

use std::collections::HashSet;
use std::io::Cursor;

use acadrust::entities::{AttributeDefinition, AttributeEntity, EntityType, Insert, Line};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

type Res = Result<(), Box<dyn std::error::Error>>;

const TAGS: [&str; 3] = ["client1", "project_number", "drawn"];

fn drawing() -> Result<CadDocument, Box<dyn std::error::Error>> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut record = BlockRecord::new("TITLE");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let owner = record.handle;
    doc.block_records.add(record)?;
    let mut frame = Line::from_points(Vector3::ZERO, Vector3::new(420.0, 0.0, 0.0));
    frame.common.owner_handle = owner;
    doc.add_entity(EntityType::Line(frame))?;
    for tag in TAGS {
        let mut def = AttributeDefinition::new(tag.into(), tag.into(), String::new());
        def.common.owner_handle = owner;
        doc.add_entity(EntityType::AttributeDefinition(def))?;
    }
    Ok(doc)
}

/// A title block whose ATTRIBs have fresh handles (as a copy gives them) and whose SEQEND handle
/// is `seqend` (as a copy keeps it).
fn title_block(
    doc: &mut CadDocument,
    value: &str,
    seqend: Option<Handle>,
) -> Result<(Handle, Vec<Handle>), Box<dyn std::error::Error>> {
    let mut insert = Insert::new("TITLE".to_string(), Vector3::ZERO);
    let mut handles = Vec::new();
    for tag in TAGS {
        let mut att = AttributeEntity::new(tag.into(), format!("{value} {tag}"));
        att.common.handle = doc.allocate_handle();
        handles.push(att.common.handle);
        insert.attributes.push(att);
    }
    insert.seqend_handle = seqend;
    Ok((doc.add_entity(EntityType::Insert(insert))?, handles))
}

fn values(doc: &CadDocument, insert: Handle) -> Vec<String> {
    match doc.get_entity(insert) {
        Some(EntityType::Insert(i)) => i.attributes.iter().map(|a| a.value.clone()).collect(),
        _ => Vec::new(),
    }
}

fn seqend_of(doc: &CadDocument, insert: Handle) -> Option<Handle> {
    match doc.get_entity(insert) {
        Some(EntityType::Insert(i)) => i.seqend_handle,
        _ => None,
    }
}

fn assert_filled(doc: &CadDocument, insert: Handle, value: &str) {
    let want: Vec<String> = TAGS.iter().map(|t| format!("{value} {t}")).collect();
    assert_eq!(values(doc, insert), want, "title block {value}");
}

fn assert_no_handle_twice(doc: &CadDocument) {
    let mut seen = HashSet::new();
    for e in doc.entities() {
        if let EntityType::Insert(i) = e {
            for a in &i.attributes {
                assert!(
                    seen.insert(a.common.handle),
                    "ATTRIB handle {:?} twice",
                    a.common.handle
                );
            }
            if let Some(s) = i.seqend_handle {
                assert!(seen.insert(s), "SEQEND handle {s:?} twice");
            }
        }
    }
}

#[test]
fn a_copied_reference_gets_its_own_seqend_and_no_attribute_is_lost() -> Res {
    let mut doc = drawing()?;
    let (first, _) = title_block(&mut doc, "A", None)?;
    let read = dwg(&doc)?;
    let seqend = seqend_of(&read, first).ok_or("no SEQEND handle read")?;
    let mut doc = read;
    // A copy of the title block: fresh ATTRIB handles, the original's SEQEND handle.
    let (copy, _) = title_block(&mut doc, "B", Some(seqend))?;
    let back = dwg(&doc)?;
    assert_filled(&back, first, "A");
    assert_filled(&back, copy, "B");
    let seqends = [seqend_of(&back, first), seqend_of(&back, copy)];
    assert!(seqends[0].is_some() && seqends[1].is_some(), "{seqends:?}");
    assert_ne!(seqends[0], seqends[1]);
    assert_no_handle_twice(&back);
    Ok(())
}

#[test]
fn a_seqend_handle_an_attribute_holds_gets_its_own_and_no_attribute_is_lost() -> Res {
    let mut doc = drawing()?;
    // A title block pasted from another drawing, whose SEQEND handle this drawing gives to an
    // ATTRIB of a later title block. It is written first.
    let taken = doc.allocate_handle();
    let (pasted, _) = title_block(&mut doc, "D", Some(taken))?;
    let (third, _) = title_block(&mut doc, "C", None)?;
    if let Some(EntityType::Insert(i)) = doc.get_entity_mut(third) {
        i.attributes[0].common.handle = taken;
    }
    let back = dwg(&doc)?;
    assert_filled(&back, pasted, "D");
    assert_filled(&back, third, "C");
    assert!(
        seqend_of(&back, pasted).is_some(),
        "the reference has no SEQEND"
    );
    assert_no_handle_twice(&back);
    Ok(())
}

#[test]
fn a_seqend_handle_an_entity_holds_leaves_that_entity_in_place() -> Res {
    let mut doc = drawing()?;
    let line = Line::from_points(Vector3::ZERO, Vector3::new(5.0, 0.0, 0.0));
    let held = doc.add_entity(EntityType::Line(line))?;
    let (pasted, _) = title_block(&mut doc, "D", Some(held))?;
    let back = dwg(&doc)?;
    assert!(
        matches!(back.get_entity(held), Some(EntityType::Line(_))),
        "the line under {held:?} is gone"
    );
    assert_filled(&back, pasted, "D");
    let seqend = seqend_of(&back, pasted);
    assert!(seqend.is_some(), "the reference has no SEQEND");
    assert_ne!(seqend, Some(held));
    Ok(())
}

#[test]
fn a_seqend_handle_an_object_holds_leaves_that_object_in_place() -> Res {
    let mut doc = dwg(&drawing()?)?;
    let held = doc
        .objects
        .keys()
        .min_by_key(|h| h.value())
        .copied()
        .ok_or("no object read")?;
    let (pasted, _) = title_block(&mut doc, "D", Some(held))?;
    let back = dwg(&doc)?;
    assert!(
        back.objects.contains_key(&held),
        "the object under {held:?} is gone"
    );
    assert_filled(&back, pasted, "D");
    let seqend = seqend_of(&back, pasted);
    assert!(seqend.is_some(), "the reference has no SEQEND");
    assert_ne!(seqend, Some(held));
    Ok(())
}

#[test]
fn the_document_written_keeps_the_seqend_handle_it_was_given() -> Res {
    let mut doc = drawing()?;
    let taken = doc.allocate_handle();
    let (pasted, _) = title_block(&mut doc, "D", Some(taken))?;
    let (third, _) = title_block(&mut doc, "C", None)?;
    if let Some(EntityType::Insert(i)) = doc.get_entity_mut(third) {
        i.attributes[0].common.handle = taken;
    }
    let _ = dwg(&doc)?;
    assert_eq!(seqend_of(&doc, pasted), Some(taken));
    Ok(())
}

#[test]
fn an_unshared_seqend_handle_is_kept() -> Res {
    let mut doc = drawing()?;
    let (first, _) = title_block(&mut doc, "A", None)?;
    let read = dwg(&doc)?;
    let before = seqend_of(&read, first);
    let again = dwg(&read)?;
    let after = seqend_of(&again, first);
    assert!(before.is_some());
    assert_eq!(before, after);
    Ok(())
}

fn dwg(doc: &CadDocument) -> Result<CadDocument, Box<dyn std::error::Error>> {
    let bytes = DwgWriter::write_to_vec(doc)?;
    Ok(DwgReader::from_stream(Cursor::new(bytes)).read()?)
}
