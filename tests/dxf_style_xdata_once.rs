//! A TrueType STYLE carries one ACAD xdata record (font family + flags).
//!
//! The writer wrote the record from the model and then the ACAD record the
//! reader had kept from the source file, so every DXF read and write of a
//! TrueType style doubled it.

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn write(doc: &CadDocument) -> String {
    String::from_utf8(DxfWriter::new(doc).write_to_vec().expect("write dxf")).unwrap()
}

fn read(text: String) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.into_bytes()))
        .expect("open dxf")
        .read()
        .expect("read dxf")
}

/// The xdata pairs (1000+) of the STYLE record with handle `handle`.
fn style_xdata(text: &str, handle: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let pairs: Vec<(i32, String)> = lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap_or(-1), c[1].trim().to_string()))
        .collect();
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, "STYLE".into()) && w[1] == (5, handle.into()))
        .expect("STYLE record");
    pairs[start + 1..]
        .iter()
        .take_while(|(code, _)| *code != 0)
        .filter(|(code, _)| *code >= 1000)
        .cloned()
        .collect()
}

#[test]
fn truetype_style_writes_its_acad_xdata_once_across_dxf_round_trips() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let style = doc.text_styles.get_mut("Standard").expect("Standard style");
    style.font_file = "arial.ttf".into();
    style.true_type_font = "Arial".into();
    style.true_type_font_flags = 34;
    let handle = format!("{:X}", style.handle.value());
    let expected = vec![
        (1001, "ACAD".to_string()),
        (1000, "Arial".to_string()),
        (1071, "34".to_string()),
    ];
    let first = write(&doc);
    assert_eq!(style_xdata(&first, &handle), expected, "first save");
    let second = write(&read(first));
    assert_eq!(
        style_xdata(&second, &handle),
        expected,
        "after a DXF round trip"
    );
    let third = write(&read(second));
    assert_eq!(style_xdata(&third, &handle), expected, "after two");
}

#[test]
fn changed_truetype_font_replaces_the_kept_acad_record() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let style = doc.text_styles.get_mut("Standard").expect("Standard style");
    style.true_type_font = "Arial".into();
    style.true_type_font_flags = 34;
    let handle = format!("{:X}", style.handle.value());
    let mut back = read(write(&doc));
    let style = back.text_styles.get_mut("Standard").unwrap();
    style.true_type_font = "Calibri".into();
    let text = write(&back);
    let acad: Vec<_> = style_xdata(&text, &handle);
    assert_eq!(
        acad,
        vec![
            (1001, "ACAD".to_string()),
            (1000, "Calibri".to_string()),
            (1071, "34".to_string()),
        ]
    );
}
