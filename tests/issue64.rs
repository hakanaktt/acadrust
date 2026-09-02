//! Regression test for issue #64 - a DXF round-trip wrote a ByBlock linetype
//! handle as the DIMSTYLE text style.
//!
//! The input replaces the default "Standard" text style with its own entry
//! (different handle) while its ByBlock linetype reuses the numeric handle
//! the defaults handed to the Standard text style. A Standard DIMSTYLE is
//! then generated from the defaults, whose `dimtxsty` still pointed at that
//! numeric handle - the writer emitted `340 -> ByBlock linetype`, and
//! consumers failed with `DXFTableEntryError: BYBLOCK`.

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::types::{Vector3, Vector3 as V3};
use acadrust::TableEntry;
use acadrust::{CadDocument, DxfReader, DxfWriter};

#[test]
fn dimstyle_text_style_does_not_point_at_linetype() {
    // Build the input the way the reporter's file is shaped: the file's
    // ByBlock linetype sits on the numeric handle the defaults give to the
    // Standard text style, the file's Standard text style is elsewhere, and
    // there is no Standard DIMSTYLE (so the default one is generated).
    let mut input = CadDocument::new();
    let default_text_style = input.text_styles.get("Standard").unwrap().handle;
    let own_text_style = input.allocate_handle();
    input
        .text_styles
        .get_mut("Standard")
        .unwrap()
        .set_handle(own_text_style);
    if let Some(lt) = input.line_types.get_mut("ByBlock") {
        lt.set_handle(default_text_style);
    }
    // The file does not carry a Standard DIMSTYLE.
    assert!(input.dim_styles.remove("Standard").is_some());
    input
        .add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 10.0, 0.0,
        )))
        .unwrap();

    let input_bytes = DxfWriter::new(&input).write_to_vec().expect("write input");

    // Round-trip through the reader/writer.
    let doc = DxfReader::from_reader(Cursor::new(input_bytes))
        .expect("reader")
        .read()
        .expect("read");
    let output = DxfWriter::new(&doc).write_to_vec().expect("write output");

    let text = String::from_utf8(output.clone()).unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();

    // Locate the Standard DIMSTYLE record and its 340 text-style handle.
    let mut dimstyle_text_style: Option<String> = None;
    for i in 1..lines.len() - 1 {
        if lines[i] == "DIMSTYLE" && lines[i - 1] == "  0" {
            let mut name = String::new();
            let mut txsty = String::new();
            let mut j = i + 1;
            while j < lines.len() - 1 && lines[j] != "  0" && lines[j] != "ENDTAB" {
                let code = lines[j].trim();
                match code {
                    "2" => name = lines[j + 1].trim().to_string(),
                    "340" => txsty = lines[j + 1].trim().to_string(),
                    _ => {}
                }
                j += 2;
            }
            if name.eq_ignore_ascii_case("Standard") {
                dimstyle_text_style = Some(txsty);
            }
        }
    }
    let txsty = dimstyle_text_style.expect("Standard DIMSTYLE missing from output");

    // The referenced handle must NOT be the ByBlock linetype (the handle the
    // defaults gave the Standard text style).
    assert_ne!(
        txsty,
        format!("{default_text_style:X}"),
        "DIMSTYLE text style points at the ByBlock linetype handle"
    );

    // The reference must resolve to the Standard text style in the output.
    let mut resolved: Option<String> = None;
    for i in 1..lines.len() - 1 {
        if lines[i] == "STYLE"
            && lines[i - 1] == "  0"
            && lines[i + 1] == "  5"
            && lines[i + 2].trim() == txsty
        {
            let mut k = i;
            while k < lines.len() - 1 && lines[k].trim() != "2" {
                k += 1;
            }
            resolved = Some(lines[k + 1].trim().to_string());
            break;
        }
    }
    assert_eq!(
        resolved.as_deref(),
        Some("Standard"),
        "DIMSTYLE 340 -> {txsty} does not resolve to the Standard text style"
    );

    // And the model must agree after a re-read.
    let rt = DxfReader::from_reader(Cursor::new(output))
        .expect("reader")
        .read()
        .expect("read");
    let ds = rt.dim_styles.get("Standard").expect("Standard dimstyle");
    let txsty_handle = ds.dimtxsty_handle;
    let target = rt
        .text_styles
        .iter()
        .find(|t| t.handle == txsty_handle)
        .expect("dimtxsty does not resolve to a text style");
    assert_eq!(
        target.name.to_ascii_uppercase(),
        "STANDARD",
        "dimtxsty resolves to a non-text-style object"
    );

    // Silence unused-import warnings for aliases kept for readability.
    let _ = V3::ZERO;
    let _ = Vector3::ZERO;
}
