//! A paper-space VIEWPORT keeps its plot style sheet, UCS, lighting and visual
//! style through a DXF write and read, in AutoCAD's group order.
//!
//! The writer left out 1, 74, 79, 345/346, 332/333, 348, 292, 282, 141, 142,
//! 63/421 and 361 and wrote 110/120/130 and 146 only when non-zero; the
//! reader did not read most of them and took the viewport's 348 for the
//! entity's own visual style, which the writer then put in AcDbEntity.

use acadrust::entities::{EntityType, Viewport};
use acadrust::types::{Color, DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

/// The AcDbViewport groups of an AutoCAD-made paper-space viewport (AutoCAD
/// 2016 DXF, AC1027), in order: 10 .. 72, 90, 1, 281, 71, 74, UCS, 79, 146,
/// 170, 61, 348, 292, 282, 141, 142, 63, 421.
const AUTOCAD_ORDER: &[i32] = &[
    10, 20, 30, 40, 41, 68, 69, 12, 22, 13, 23, 14, 24, 15, 25, 16, 26, 36, 17, 27, 37, 42, 43, 44,
    45, 50, 51, 72, 90, 1, 281, 71, 74, 110, 120, 130, 111, 121, 131, 112, 122, 132, 79, 146, 170,
    61, 348, 292, 282, 141, 142, 63, 421,
];

fn document() -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let visual_style = doc.allocate_handle();
    let mut style = acadrust::objects::VisualStyle::new();
    style.handle = visual_style;
    style.owner = doc.header.named_objects_dict_handle;
    style.description = "Wireframe".into();
    doc.objects.insert(
        visual_style,
        acadrust::objects::ObjectType::VisualStyle(style),
    );
    let mut vp = Viewport::new();
    vp.center = Vector3::new(150.0, 100.0, 0.0);
    vp.width = 100.0;
    vp.height = 80.0;
    vp.id = 2;
    vp.style_sheet = "monochrome.ctb".into();
    vp.ucs_per_viewport = true;
    vp.ucs_at_origin = true;
    vp.ucs_origin = Vector3::new(1.0, 2.0, 0.0);
    vp.ucs_x_axis = Vector3::new(1.0, 0.0, 0.0);
    vp.ucs_y_axis = Vector3::new(0.0, 1.0, 0.0);
    vp.ucs_ortho_type = 1;
    vp.elevation = 2.5;
    vp.grid_major = 5;
    vp.visual_style_handle = visual_style;
    vp.default_lighting = false;
    vp.default_lighting_type = 2;
    vp.brightness = 10.0;
    vp.contrast = 20.0;
    vp.ambient_color = Color::Rgb {
        r: 0x33,
        g: 0x33,
        b: 0x33,
    };
    let handle = doc
        .add_entity_to_layout(EntityType::Viewport(Box::new(vp)), "Layout1")
        .expect("viewport");
    (doc, handle, visual_style)
}

fn record(text: &str, handle: Handle) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let pairs: Vec<(i32, String)> = lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap_or(-1), c[1].trim().to_string()))
        .collect();
    let hex = format!("{:X}", handle.value());
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, "VIEWPORT".into()) && w[1] == (5, hex.clone()))
        .expect("VIEWPORT record");
    pairs[start + 1..]
        .iter()
        .take_while(|(code, _)| *code != 0)
        .cloned()
        .collect()
}

#[test]
fn paper_space_viewport_fields_survive_dxf() {
    let (doc, handle, visual_style) = document();
    let bytes = DxfWriter::new(&doc).write_to_vec().expect("write dxf");
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let Some(EntityType::Viewport(vp)) = back.get_entity(handle) else {
        panic!("viewport lost");
    };
    assert_eq!(vp.style_sheet, "monochrome.ctb");
    assert!(vp.ucs_per_viewport && vp.ucs_at_origin);
    assert_eq!(vp.ucs_origin, Vector3::new(1.0, 2.0, 0.0));
    assert_eq!(vp.ucs_ortho_type, 1);
    assert_eq!(vp.elevation, 2.5);
    assert_eq!(vp.visual_style_handle, visual_style);
    assert_eq!(vp.common.full_visual_style_handle, None);
    assert!(!vp.default_lighting);
    assert_eq!(vp.default_lighting_type, 2);
    assert_eq!((vp.brightness, vp.contrast), (10.0, 20.0));
    assert_eq!(
        vp.ambient_color,
        Color::Rgb {
            r: 0x33,
            g: 0x33,
            b: 0x33
        }
    );
}

#[test]
fn paper_space_viewport_groups_follow_autocad_order() {
    let (doc, handle, visual_style) = document();
    let text = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let rec = record(&text, handle);
    let split = rec
        .iter()
        .position(|p| *p == (100, "AcDbViewport".into()))
        .expect("AcDbViewport marker");
    assert!(
        !rec[..split].iter().any(|(code, _)| *code == 348),
        "348 written in AcDbEntity: {:?}",
        &rec[..split]
    );
    let codes: Vec<i32> = rec[split + 1..]
        .iter()
        .map(|(code, _)| *code)
        .filter(|code| AUTOCAD_ORDER.contains(code))
        .collect();
    assert_eq!(codes, AUTOCAD_ORDER);
    let vs = format!("{:X}", visual_style.value());
    assert!(rec[split + 1..].iter().any(|p| *p == (348, vs.clone())));
}
