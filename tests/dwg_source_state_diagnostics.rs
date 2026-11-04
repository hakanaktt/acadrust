use std::io::Cursor;

use acadrust::entities::{EntityType, LwPolyline, LwVertex};
use acadrust::tables::linetype::LineTypeComplexContent;
use acadrust::tables::{LineType, LineTypeComplexData, LineTypeElement};
use acadrust::types::{DxfVersion, Vector2};
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// A drawing keeps a nonfinite bulge and a shape segment without a style as
/// stored; the read reports both instead of looking clean (acadrust #139, #141).
#[test]
fn stored_inconsistencies_are_reported() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut polyline = LwPolyline::new();
    polyline.add_point(Vector2::new(0.0, 0.0));
    let mut vertex = LwVertex::new(Vector2::new(1.0, 0.0));
    vertex.bulge = f64::NAN;
    polyline.add_vertex(vertex);
    polyline.add_point(Vector2::new(2.0, 1.0));
    document.add_entity(EntityType::LwPolyline(polyline)).unwrap();

    let mut linetype = LineType::new("UNSTYLED");
    let mut dash = LineTypeElement::dash(5.0);
    dash.complex = Some(LineTypeComplexData {
        content: LineTypeComplexContent::Shape { shape_number: 1 },
        ..LineTypeComplexData::default()
    });
    linetype.elements.push(dash);
    linetype.elements.push(LineTypeElement::space(2.0));
    linetype.pattern_length = 7.0;
    document.line_types.add(linetype).unwrap();

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let outcome = DwgReader::from_stream(Cursor::new(bytes))
        .read_with_stats()
        .unwrap();
    let codes: Vec<&str> = outcome
        .stats
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert!(codes.contains(&"nonfinite-bulge"), "{codes:?}");
    assert!(codes.contains(&"shape-without-style"), "{codes:?}");
    assert!(!codes.contains(&"block-marker-mismatch"), "{codes:?}");
}

/// An unnamed ordinary STYLE that complex linetypes draw shapes from is the
/// shape-file style behind them, stored without its shape-file flag; the
/// read names that role instead of reporting a bare unnamed style (#311).
#[test]
fn an_unnamed_style_names_the_linetypes_that_use_it() {
    use acadrust::tables::TextStyle;
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = TextStyle::new("");
    style.font_file = "ltypeshp.shx".into();
    style.is_shape_file = false;
    let handle = acadrust::types::Handle::new(0x500);
    style.handle = handle;
    document.text_styles.add_allow_duplicate(style);

    let mut linetype = LineType::new("ZIGZAG");
    let mut dash = LineTypeElement::dash(0.0);
    dash.complex = Some(LineTypeComplexData {
        content: LineTypeComplexContent::Shape { shape_number: 131 },
        style_handle: handle,
        ..LineTypeComplexData::default()
    });
    linetype.elements.push(dash);
    linetype.elements.push(LineTypeElement::space(1.0));
    linetype.pattern_length = 1.0;
    document.line_types.add(linetype).unwrap();

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let outcome = DwgReader::from_stream(Cursor::new(bytes))
        .read_with_stats()
        .unwrap();
    let message = outcome
        .stats
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "unnamed-text-style")
        .map(|diagnostic| diagnostic.message.clone())
        .expect("the unnamed style is reported");
    assert!(message.contains("ZIGZAG") && message.contains("ltypeshp.shx"), "{message}");
}
