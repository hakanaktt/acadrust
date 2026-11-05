//! A multi-line attribute's value is set once: `AttributeEntity::set_value` also sets the text of
//! its embedded MTEXT, which DWG/DXF R2018+ write for a multi-line attribute and AutoCAD shows.
//! Before, the embedded MTEXT kept the old text, so a value edited in an application came back
//! as the old one after a DWG 2018 save and reopen.

use std::io::Cursor;

use acadrust::entities::attribute_definition::MTextFlag;
use acadrust::entities::{AttributeEntity, EntityType, Insert, MText};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

type Res = Result<(), Box<dyn std::error::Error>>;

fn notes(value: &str) -> AttributeEntity {
    let mut att = AttributeEntity::new("NOTES".to_string(), value.to_string());
    att.height = 2.0;
    att.is_multiline = true;
    att.mtext_flag = MTextFlag::MultiLine;
    att.embedded_mtext = Some(Box::new(MText::with_value(value, Vector3::ZERO)));
    att
}

/// A drawing holding a multi-line attribute whose value was set after it was added.
fn edited() -> Result<CadDocument, Box<dyn std::error::Error>> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut record = BlockRecord::new("TITLE");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    doc.block_records.add(record)?;
    let mut insert = Insert::new("TITLE".to_string(), Vector3::ZERO);
    insert.attributes.push(notes("1. Old"));
    let h = doc.add_entity(EntityType::Insert(insert))?;
    if let Some(EntityType::Insert(i)) = doc.get_entity_mut(h) {
        for a in i.attributes.iter_mut() {
            a.set_value("1. RD New\\P2. NAP");
        }
    }
    Ok(doc)
}

fn assert_new_text(back: &CadDocument) -> Res {
    let att = back
        .entities()
        .find_map(|e| match e {
            EntityType::Insert(i) if i.block_name == "TITLE" => i.attributes.first().cloned(),
            _ => None,
        })
        .ok_or("no TITLE attribute after reopen")?;
    assert_eq!(att.value, "1. RD New\\P2. NAP");
    assert_eq!(
        att.embedded_mtext.as_ref().map(|m| m.value.as_str()),
        Some("1. RD New\\P2. NAP")
    );
    Ok(())
}

#[test]
fn set_value_sets_the_embedded_mtext_too() {
    let mut att = notes("1. Old");
    att.set_value("1. RD New\\P2. NAP");
    assert_eq!(att.value, "1. RD New\\P2. NAP");
    assert_eq!(
        att.embedded_mtext.as_ref().map(|m| m.value.as_str()),
        Some("1. RD New\\P2. NAP")
    );
    // A single-line attribute gets no MTEXT.
    let mut single = AttributeEntity::new("TITLE".to_string(), "Titel".to_string());
    single.set_value("Papendrecht");
    assert!(single.embedded_mtext.is_none());
}

#[test]
fn an_edited_multi_line_attribute_reads_back_from_dwg_2018() -> Res {
    let doc = edited()?;
    let back = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc)?)).read()?;
    assert_new_text(&back)
}

#[test]
fn an_edited_multi_line_attribute_reads_back_from_dxf_2018() -> Res {
    let doc = edited()?;
    let bytes = DxfWriter::new(&doc).write_to_vec()?;
    let back = DxfReader::from_reader(Cursor::new(bytes))?.read()?;
    assert_new_text(&back)
}

#[test]
fn a_line_feed_in_the_value_becomes_an_mtext_paragraph_break() {
    let mut att = notes("1. Old");
    att.set_value("1. RD New\r\n2. NAP\n3. Last");
    assert_eq!(att.value, "1. RD New\r\n2. NAP\n3. Last");
    assert_eq!(
        att.embedded_mtext.as_ref().map(|m| m.value.as_str()),
        Some("1. RD New\\P2. NAP\\P3. Last")
    );
}
