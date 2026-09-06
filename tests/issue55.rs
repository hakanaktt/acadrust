//! Regression test for issue #55.
//!
//! `read_file_header_ac15` used to infer the AcDbObjects region as
//! `handles_seeker - aux_header_end`, assuming the AuxHeader physically
//! precedes the Handles section. Real-world R2000 (AC1015) files store
//! Template/AuxHeader at end-of-file, *after* Handles, so the inferred size
//! went negative, the AcDbObjects entry was never registered, the handle map
//! stayed empty and `read()` silently returned a document with 0 entities.
//!
//! Per the ODA convention the AcDbObjects region occupies the gap between the
//! end of the Classes section and the start of the Handles section, which is
//! what the reader now uses.

use std::io::Cursor;

use acadrust::entities::{Circle, EntityType, Line, Text};
use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn sample_document() -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 100.0, 50.0, 0.0,
    )))
    .unwrap();
    doc.add_entity(EntityType::Circle(Circle::from_coords(
        50.0, 50.0, 0.0, 25.0,
    )))
    .unwrap();
    doc
}

fn read_dwg(bytes: Vec<u8>) -> CadDocument {
    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    reader.read().expect("DWG read failed")
}

/// Repack the file so the Template and AuxHeader sections physically sit at
/// end-of-file, *after* the Handles section — the layout real-world R2000
/// files use (issue #55).
///
/// The section bytes are appended verbatim and only their locator seekers are
/// repointed; nothing else moves, so the absolute handle offsets stay valid.
fn move_template_and_aux_header_to_eof(bytes: &[u8]) -> Vec<u8> {
    // Locator records start at 0x19: 9 bytes each, ordered by section number
    // (number: u8, seeker: i32 LE, size: i32 LE). Numbers: 0=Header,
    // 1=Classes, 2=Handles, 3=ObjFreeSpace, 4=Template, 5=AuxHeader.
    const RECORDS: usize = 0x19;

    let parse = |n: usize| -> (i64, i64) {
        let o = RECORDS + n * 9;
        assert_eq!(bytes[o], n as u8, "unexpected locator record order");
        let seeker = i32::from_le_bytes(bytes[o + 1..o + 5].try_into().unwrap()) as i64;
        let size = i32::from_le_bytes(bytes[o + 5..o + 9].try_into().unwrap()) as i64;
        (seeker, size)
    };
    let patch_seeker = |data: &mut Vec<u8>, n: usize, seeker: i64| {
        let o = RECORDS + n * 9;
        data[o + 1..o + 5].copy_from_slice(&(seeker as i32).to_le_bytes());
    };

    let (template_seeker, template_size) = parse(4);
    let (aux_seeker, aux_size) = parse(5);
    assert!(template_size > 0 && aux_size > 0, "sections must have data");

    let mut out = bytes.to_vec();
    let template_off = out.len() as i64;
    out.extend_from_slice(
        &bytes[template_seeker as usize..(template_seeker + template_size) as usize],
    );
    let aux_off = out.len() as i64;
    out.extend_from_slice(&bytes[aux_seeker as usize..(aux_seeker + aux_size) as usize]);

    patch_seeker(&mut out, 4, template_off);
    patch_seeker(&mut out, 5, aux_off);
    out
}

#[test]
fn r2000_with_template_and_aux_header_after_handles_reads_fully() {
    let doc = sample_document();
    let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write failed");

    // Baseline: acadrust's own layout (Template/AuxHeader before Handles).
    let baseline = read_dwg(bytes.clone());
    let expected = baseline.entities().count();
    assert!(expected > 0, "baseline DWG must contain entities");

    // The relocated layout must read back exactly the same entities. On the
    // old aux-header-based inference the objects size went negative and this
    // returned an empty document.
    let repacked = move_template_and_aux_header_to_eof(&bytes);
    let rt = read_dwg(repacked);
    assert_eq!(
        rt.entities().count(),
        expected,
        "entities lost after relocating Template/AuxHeader behind Handles"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  Secondary issue: legacy string encoding (code page + MIF escapes)
// ═══════════════════════════════════════════════════════════════════════════
//
// Pre-Unicode DWG (R13–R2004) strings are stored in the code page recorded
// in the file header (e.g. ANSI_936 / GBK), and characters outside that code
// page are stored as MIF `\U+XXXX` escapes. The reader must apply both; the
// writer must emit MIF escapes rather than `&#NNNNN;` references.

fn text_document(code_page: &str, value: &str) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.header.code_page = code_page.to_string();
    doc.add_entity(EntityType::Text(Text::with_value(
        value,
        Vector3::new(0.0, 0.0, 0.0),
    )))
    .unwrap();
    doc
}

fn first_text_value(doc: &CadDocument) -> String {
    for entity in doc.entities() {
        if let EntityType::Text(t) = entity {
            return t.value.clone();
        }
    }
    panic!("no TEXT entity in document");
}

#[test]
fn gbk_codepage_text_roundtrip() {
    // Chinese text with the GBK code page decodes back to the same characters.
    let doc = text_document("ANSI_936", "中文文本");
    let rt = read_dwg(DwgWriter::write_to_vec(&doc).unwrap());
    assert_eq!(first_text_value(&rt), "中文文本");
    assert_eq!(rt.header.code_page, "GB2312");
}

#[test]
fn mif_escapes_in_dwg_strings_are_decoded() {
    // A file whose strings contain literal MIF \U+XXXX sequences (ASCII-safe
    // for any code page) must decode them into the actual characters.
    let doc = text_document("ANSI_936", "\\U+4E2D\\U+6587");
    let rt = read_dwg(DwgWriter::write_to_vec(&doc).unwrap());
    assert_eq!(first_text_value(&rt), "中文");
}

#[test]
fn unmappable_text_is_written_as_mif_escapes() {
    // Characters outside the (western) code page must be stored as MIF
    // escapes — not encoding_rs' HTML `&#NNNNN;` references — and must
    // decode back to the original text on read.
    let doc = text_document("ANSI_1252", "中文A");
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    assert!(text.contains("\\U+4E2D"), "expected MIF escape in output");
    assert!(
        !text.contains("&#"),
        "HTML character references are not valid DWG"
    );

    let rt = read_dwg(bytes);
    assert_eq!(first_text_value(&rt), "中文A");
}

#[test]
fn gbk_layer_names_roundtrip() {
    // Layer table records carry their names as legacy text too.
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.header.code_page = "ANSI_936".to_string();
    for name in ["ASCII_LAYER", "图层一", "图层二"] {
        let mut layer = Layer::new(name);
        layer.handle = doc.allocate_handle();
        layer.color = Color::from_index(1);
        doc.layers.add(layer).unwrap();
    }
    let mut text = Text::with_value("文本", Vector3::new(0.0, 0.0, 0.0));
    text.common.layer = "图层一".to_string();
    doc.add_entity(EntityType::Text(text)).unwrap();

    let rt = read_dwg(DwgWriter::write_to_vec(&doc).unwrap());
    for name in ["ASCII_LAYER", "图层一", "图层二"] {
        assert!(
            rt.layers.get(name).is_some(),
            "layer {name:?} missing after DWG round-trip"
        );
    }
    for entity in rt.entities() {
        if let EntityType::Text(t) = entity {
            assert_eq!(t.common.layer, "图层一", "entity layer assignment lost");
        }
    }
}
