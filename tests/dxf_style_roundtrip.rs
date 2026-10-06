
use std::io::Cursor;

use acadrust::tables::{DimStyle, TextStyle};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn dxf_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("DXF write failed");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

#[test]
fn text_style_generation_flags_survive() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = TextStyle::new("Mirrored");
    style.flags.backward = true;
    style.flags.upside_down = true;
    doc.text_styles.add(style).unwrap();
    let rt = dxf_roundtrip(&doc);
    let style = rt.text_styles.get("Mirrored").expect("style missing");
    assert!(style.flags.backward, "backward");
    assert!(style.flags.upside_down, "upside down");

    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = TextStyle::new("OnlyBackward");
    style.flags.backward = true;
    doc.text_styles.add(style).unwrap();
    let style = dxf_roundtrip(&doc).text_styles.get("OnlyBackward").cloned().unwrap();
    assert!(style.flags.backward && !style.flags.upside_down);
}

#[test]
fn text_style_oblique_angle_is_degrees_on_the_wire() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = TextStyle::new("Slanted");
    style.oblique_angle = 15f64.to_radians();
    doc.text_styles.add(style).unwrap();
    // The DXF group 50 of a STYLE record is in degrees.
    let text = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = lines.iter().position(|l| *l == "Slanted").expect("style record");
    // `at` is the value of group 2; group/value pairs run from one line earlier.
    let group50 = lines[at - 1..]
        .chunks(2)
        .take_while(|pair| pair[0] != "0")
        .find(|pair| pair[0] == "50")
        .map(|pair| pair[1].parse::<f64>().unwrap())
        .expect("group 50");
    assert!((group50 - 15.0).abs() < 1e-9, "written {group50}, expected 15 degrees");
    let rt = dxf_roundtrip(&doc);
    let style = rt.text_styles.get("Slanted").unwrap();
    assert!((style.oblique_angle - 15f64.to_radians()).abs() < 1e-9, "read back {}", style.oblique_angle);
}

#[test]
fn dimension_style_text_style_name_is_resolved_from_its_handle() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut text = TextStyle::new("Heading");
    text.handle = doc.allocate_handle();
    let handle = text.handle;
    doc.text_styles.add(text).unwrap();
    let mut dim = DimStyle::new("Metric");
    dim.dimtxsty = "Heading".into();
    dim.dimtxsty_handle = handle;
    dim.handle = doc.allocate_handle();
    doc.dim_styles.add(dim).unwrap();
    let rt = dxf_roundtrip(&doc);
    let dim = rt.dim_styles.get("Metric").expect("dim style missing");
    assert_eq!(dim.dimtxsty_handle, handle);
    assert_eq!(dim.dimtxsty, "Heading");
}
