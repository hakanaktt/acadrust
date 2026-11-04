//! Edge cases of the pre-R2018 multiline attribute layout (ACAD_MLATT):
//! an attribute that already has an extension dictionary, and broken records.

use std::io::Cursor;

use acadrust::entities::{
    AttributeDefinition, AttributeEntity, EntityType, Insert, Line, MText, MTextFlag,
};
use acadrust::objects::ObjectType;
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

/// Block LABML with a 2-line ATTDEF, an INSERT with a 2-line ATTRIB, a LINE.
fn drawing() -> (CadDocument, Handle, Handle, Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1027);
    let mut record = BlockRecord::new("LABML");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let block = record.handle;
    doc.block_records.add(record).unwrap();
    let mut def = AttributeDefinition::new("NOTE".into(), "Note".into(), "D one\\PD two".into());
    def.common.owner_handle = block;
    def.height = 2.5;
    def.is_multiline = true;
    def.mtext_flag = MTextFlag::MultiLine;
    def.embedded_mtext = Some(Box::new(MText::with_value("D one\\PD two", Vector3::ZERO)));
    let def = doc
        .add_entity(EntityType::AttributeDefinition(def))
        .unwrap();
    let mut att = AttributeEntity::simple("NOTE", "A one\\PA two");
    att.height = 2.5;
    att.is_multiline = true;
    att.mtext_flag = MTextFlag::MultiLine;
    att.embedded_mtext = Some(Box::new(MText::with_value("A one\\PA two", Vector3::ZERO)));
    let mut insert = Insert::new("LABML", Vector3::new(10.0, 10.0, 0.0));
    insert.attributes.push(att);
    let insert = doc.add_entity(EntityType::Insert(insert)).unwrap();
    let line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 1.0, 0.0);
    let line = doc.add_entity(EntityType::Line(line)).unwrap();
    (doc, def, insert, line)
}

fn write(doc: &CadDocument) -> String {
    String::from_utf8(DxfWriter::new(doc).write_to_vec().unwrap()).unwrap()
}

fn read(dxf: &str) -> CadDocument {
    DxfReader::from_reader(Cursor::new(dxf.as_bytes().to_vec()))
        .unwrap()
        .read()
        .unwrap()
}

/// Apply `edit` to the value line of every `code` group whose value is `old`.
fn edit_group(dxf: &str, code: &str, old: &str, new: Option<&str>) -> String {
    let lines: Vec<&str> = dxf.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        if lines[i].trim() == code && lines[i + 1].trim() == old {
            if let Some(new) = new {
                out.push(lines[i].to_string());
                out.push(new.to_string());
            }
        } else {
            out.push(lines[i].to_string());
            out.push(lines[i + 1].to_string());
        }
        i += 2;
    }
    out.join("\n") + "\n"
}

fn hex(handle: Handle) -> String {
    format!("{:X}", handle.value())
}

fn attrib_values(doc: &CadDocument, insert: Handle) -> Vec<String> {
    match doc.get_entity(insert) {
        Some(EntityType::Insert(i)) => i.attributes.iter().map(|a| a.value.clone()).collect(),
        _ => panic!("INSERT lost"),
    }
}

#[test]
fn an_existing_extension_dictionary_survives() {
    let (mut doc, def, ..) = drawing();
    doc.ensure_xrecord(def, "LABDATA");
    let back = read(&write(&doc));
    let EntityType::AttributeDefinition(a) = back.get_entity(def).unwrap() else {
        panic!()
    };
    assert!(a.is_multiline, "ATTDEF not merged");
    // Linked from the ATTDEF itself, not only findable by owner.
    let linked = a
        .common
        .xdictionary_handle
        .expect("extension dictionary unlinked");
    let Some(ObjectType::Dictionary(d)) = back.objects.get(&linked) else {
        panic!("linked dictionary missing");
    };
    assert!(
        d.get("LABDATA").is_some(),
        "own XRECORD not in the linked dictionary"
    );
    assert!(
        d.get("ACAD_MLATT").is_none(),
        "ACAD_MLATT left in the dictionary"
    );
    assert!(back.xrecord(def, "ACAD_MLATT").is_none(), "ACAD_MLATT left");
}

#[test]
fn a_340_naming_the_attribute_itself_keeps_it() {
    let (doc, _, insert, _) = drawing();
    let dxf = write(&doc);
    // The record's 340 names the ATTRIB's second line; point it at the ATTRIB.
    let first = read_lines_handle(&dxf, "A one");
    let written = read_lines_handle(&dxf, "A two");
    let broken = edit_group(&dxf, "340", &written, Some(&first));
    assert_ne!(broken, dxf, "340 not found");
    let back = read(&broken);
    let values = attrib_values(&back, insert);
    assert!(
        values.iter().any(|v| v.contains("A one")),
        "ATTRIB text lost: {values:?}"
    );
}

#[test]
fn a_340_naming_another_entity_does_not_delete_it() {
    let (doc, _, _, line) = drawing();
    let dxf = write(&doc);
    let written = read_lines_handle(&dxf, "D two");
    let broken = edit_group(&dxf, "340", &written, Some(&hex(line)));
    let back = read(&broken);
    assert!(back.get_entity(line).is_some(), "the LINE was deleted");
}

#[test]
fn a_record_without_text_keeps_the_lines_text() {
    let (doc, _, insert, _) = drawing();
    let dxf = write(&doc);
    let broken = edit_group(&dxf, "1", "A one\\PA two", None);
    let back = read(&broken);
    assert_eq!(attrib_values(&back, insert), ["A one\\PA two"]);
}

/// Handle (group 5) of the entity whose text (group 1) is `text`.
fn read_lines_handle(dxf: &str, text: &str) -> String {
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    let at = lines
        .windows(2)
        .position(|w| w == ["1", text])
        .expect("line text");
    let start = (0..at)
        .rev()
        .find(|i| lines[*i] == "0" && i % 2 == 0)
        .unwrap();
    let five = (start..at).step_by(2).find(|i| lines[*i] == "5").unwrap();
    lines[five + 1].to_string()
}
