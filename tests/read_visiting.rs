//! Visitor hook: drop decoded entities before they are stored on the document.

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::{CadDocument, DwgReader, DwgWriter};

#[test]
fn read_visiting_drops_model_lines_from_the_document() {
    let mut doc = CadDocument::default();
    for i in 0..64 {
        let mut line = Line::from_coords(i as f64, 0.0, 0.0, i as f64 + 1.0, 0.0, 0.0);
        line.common.layer = "SET".to_string();
        doc.add_entity(EntityType::Line(line)).expect("line");
    }

    let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let mut dropped = 0u32;
    let rebuilt = DwgReader::from_stream(Cursor::new(bytes))
        .read_visiting(|_, entity| {
            if matches!(entity, EntityType::Line(_)) {
                dropped += 1;
                None
            } else {
                Some(entity)
            }
        })
        .expect("read_visiting");

    assert!(
        dropped >= 64,
        "visitor should see the written lines, dropped={dropped}"
    );
    assert_eq!(
        rebuilt
            .entities()
            .filter(|entity| matches!(entity, EntityType::Line(_)))
            .count(),
        0,
        "dropped lines must not remain on the document"
    );
}

#[test]
fn read_keeps_every_entity() {
    let mut doc = CadDocument::default();
    doc.add_entity(EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0)))
        .expect("line");
    let bytes = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let rebuilt = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read");
    assert!(
        rebuilt
            .entities()
            .any(|entity| matches!(entity, EntityType::Line(_))),
        "default read() must keep entities"
    );
}
