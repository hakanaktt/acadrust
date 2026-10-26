//! A DWG header can carry a code page index this crate has no mapping for.
//! The reader falls back to ANSI_1252 so the file still opens, but it must say
//! so instead of doing it silently.

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::notification::NotificationType;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn dwg_with_code_page(index: u16) -> Vec<u8> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 10.0, 10.0, 0.0,
    )))
    .unwrap();
    let mut bytes = DwgWriter::write_to_vec(&doc).unwrap();
    // AC1015 file header: the code page index is a u16 at 0x13.
    bytes[0x13..0x15].copy_from_slice(&index.to_le_bytes());
    bytes
}

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

#[test]
fn unknown_code_page_index_raises_a_warning() {
    let doc = read(dwg_with_code_page(999));
    let warned = doc
        .notifications
        .of_type(NotificationType::Warning)
        .iter()
        .any(|n| n.message.contains("999") && n.message.to_lowercase().contains("code page"));
    assert!(warned, "no warning for unknown code page 999");
}

#[test]
fn unknown_code_page_index_still_opens_with_the_1252_fallback() {
    let doc = read(dwg_with_code_page(999));
    assert_eq!(doc.header.code_page, "ANSI_1252");
    assert_eq!(doc.entities().count(), 1);
}

#[test]
fn known_code_page_index_raises_no_code_page_warning() {
    let doc = read(dwg_with_code_page(29));
    assert_eq!(doc.header.code_page, "ANSI_1251");
    assert!(!doc
        .notifications
        .iter()
        .any(|n| n.message.to_lowercase().contains("code page")));
}
