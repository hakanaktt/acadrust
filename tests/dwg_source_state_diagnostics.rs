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
