//! MLINESTYLE flags (`70`: fill, joints, caps) and true colors (`420`) survive
//! a DXF save and read.

use std::io::Cursor;

use acadrust::objects::{MLineStyle, MLineStyleElement, ObjectType};
use acadrust::types::{Color, DxfVersion, Handle};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn drawing(version: DxfVersion) -> (CadDocument, Handle) {
    let mut doc = CadDocument::with_version(version);
    let mut style = MLineStyle::new("LAB3");
    style.add_element(MLineStyleElement::full(
        0.5,
        Color::from_index(1),
        "BYLAYER",
    ));
    style.add_element(MLineStyleElement::full(
        0.0,
        Color::from_rgb(10, 200, 30),
        "BYLAYER",
    ));
    style.add_element(MLineStyleElement::full(-0.5, Color::ByBlock, "BYBLOCK"));
    style.fill_color = Color::from_rgb(200, 100, 50);
    style.flags.fill_on = true;
    style.flags.display_joints = true;
    style.flags.start_square_cap = true;
    style.flags.start_round_cap = true;
    style.flags.end_inner_arcs_cap = true;
    style.flags.end_round_cap = true;
    let handle = doc.allocate_handle();
    style.handle = handle;
    let dict = doc.header.acad_mlinestyle_dict_handle;
    style.owner = dict;
    doc.objects.insert(handle, ObjectType::MLineStyle(style));
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dict) {
        d.add_entry("LAB3", handle);
    }
    (doc, handle)
}

fn round_trip(doc: &CadDocument) -> (String, CadDocument) {
    let dxf = String::from_utf8(DxfWriter::new(doc).write_to_vec().unwrap()).unwrap();
    let back = DxfReader::from_reader(Cursor::new(dxf.clone().into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    (dxf, back)
}

fn style(doc: &CadDocument, handle: Handle) -> &MLineStyle {
    match doc.objects.get(&handle) {
        Some(ObjectType::MLineStyle(style)) => style,
        _ => panic!("MLINESTYLE lost"),
    }
}

#[test]
fn mlinestyle_flags_and_true_colors_survive_dxf() {
    let (doc, handle) = drawing(DxfVersion::AC1032);
    let (_, back) = round_trip(&doc);
    let (a, b) = (style(&doc, handle), style(&back, handle));
    assert_eq!(b.flags, a.flags, "70 flags");
    assert_eq!(b.fill_color, a.fill_color, "fill color");
    let colors = |s: &MLineStyle| s.elements.iter().map(|e| e.color).collect::<Vec<_>>();
    assert_eq!(colors(b), colors(a), "element colors");
}

#[test]
fn r2000_mlinestyle_keeps_flags_and_writes_no_true_color() {
    let (doc, handle) = drawing(DxfVersion::AC1015);
    let (dxf, back) = round_trip(&doc);
    assert_eq!(
        style(&back, handle).flags,
        style(&doc, handle).flags,
        "70 flags"
    );
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    // Group codes of the LAB3 record, from its name to the next `0`.
    let start = lines.windows(2).position(|w| w == ["2", "LAB3"]).unwrap();
    let codes: Vec<&str> = lines[start..]
        .iter()
        .step_by(2)
        .copied()
        .take_while(|code| *code != "0")
        .collect();
    assert!(codes.contains(&"70"), "no 70 in the MLINESTYLE");
    assert!(!codes.contains(&"420"), "420 in an R2000 MLINESTYLE");
    // The element's nearest color index, not ByLayer.
    let rgb = Color::from_rgb(10, 200, 30);
    let element = &style(&back, handle).elements[1];
    assert_eq!(element.color, Color::from_index(rgb.approximate_index()));
}
