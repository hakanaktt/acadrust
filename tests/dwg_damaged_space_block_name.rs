//! A DWG whose model or paper space block record has a damaged name: the header still names
//! both records by handle, so their entities stay in model and paper space, on DXF output too.

use acadrust::entities::{EntityType, Line};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReadOptions, DwgReader, DwgWriter, DxfWriter};
use std::io::Cursor;

const LINES: usize = 200;

/// An AC1015 DWG of `LINES` model space lines.
fn dwg() -> Vec<u8> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    for i in 0..LINES {
        let x = i as f64;
        doc.add_entity(EntityType::Line(Line::from_points(
            Vector3::new(x, 0.0, 0.0),
            Vector3::new(x, 10.0, 0.0),
        )))
        .unwrap();
    }
    DwgWriter::write_to_vec(&doc).unwrap()
}

/// `bytes` with every `from` replaced by `to`, a name of the same length; how many there were.
fn damaged(bytes: &[u8], from: &[u8], to: &[u8]) -> (Vec<u8>, usize) {
    assert_eq!(from.len(), to.len());
    let mut out = bytes.to_vec();
    let mut n = 0;
    let mut i = 0;
    while i + from.len() <= out.len() {
        if &out[i..i + from.len()] == from {
            out[i..i + from.len()].copy_from_slice(to);
            n += 1;
            i += from.len();
        } else {
            i += 1;
        }
    }
    (out, n)
}

fn read(bytes: Vec<u8>) -> acadrust::io::read::ReadOutcome {
    DwgReader::from_stream_with_options(Cursor::new(bytes), DwgReadOptions::failsafe())
        .read_with_stats()
        .unwrap()
}

/// The entity types of the DXF output's ENTITIES section.
fn dxf_entities(doc: &CadDocument) -> usize {
    let text = String::from_utf8_lossy(&DxfWriter::new(doc).write_to_vec().unwrap()).to_string();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let start = lines
        .windows(2)
        .position(|w| w == ["2", "ENTITIES"])
        .expect("an ENTITIES section");
    lines[start..]
        .chunks(2)
        .take_while(|p| p != &["0", "ENDSEC"])
        .filter(|p| p == &["0", "LINE"])
        .count()
}

#[test]
fn a_damaged_model_space_name_keeps_the_drawing_in_model_space() {
    let (bytes, n) = damaged(&dwg(), b"*Model_Space", b"*ModelXSpace");
    assert!(n > 0, "the name is in the file");
    let out = read(bytes);
    let doc = &out.document;
    assert_eq!(doc.model_space_entities().count(), LINES);
    assert!(doc.block_records.get("*Model_Space").is_some());
    assert!(doc.block_records.get("*ModelXSpace").is_none());
    assert_eq!(dxf_entities(doc), LINES, "the lines are in ENTITIES");
    assert!(
        out.stats
            .diagnostics
            .iter()
            .any(|d| d.message.contains("*ModelXSpace")),
        "the repair is reported: {:?}",
        out.stats.diagnostics
    );
}

#[test]
fn a_damaged_paper_space_name_is_given_back() {
    let (bytes, n) = damaged(&dwg(), b"*Paper_Space", b"*PaperXSpace");
    assert!(n > 0, "the name is in the file");
    let doc = read(bytes).document;
    assert!(doc.block_records.get("*Paper_Space").is_some());
    assert!(doc.block_records.get("*PaperXSpace").is_none());
    assert_eq!(doc.model_space_entities().count(), LINES);
}

/// Control: an undamaged file reads as before, with no repair reported.
#[test]
fn an_undamaged_file_reports_no_repair() {
    let out = read(dwg());
    assert_eq!(out.document.model_space_entities().count(), LINES);
    assert_eq!(dxf_entities(&out.document), LINES);
    assert!(
        out.stats.diagnostics.is_empty(),
        "{:?}",
        out.stats.diagnostics
    );
}
