//! An MLEADERSTYLE read from an AutoCAD DXF is written back with the same
//! groups and values.
//!
//! The reader skipped 171, 172, 90, 40, 41, 46, 47, 49, 140, 141, 143, 177,
//! 293-295, 297 and 298 (they came back as defaults) and the writer left out
//! 179, 271, 272, 273 and 298, which AutoCAD writes from R2010 / R2013.

use acadrust::{DxfReader, DxfWriter};
use std::io::Cursor;

/// The fixture written by AutoCAD (see `tests/datatable/README.md`).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/datatable/point_object_id.dxf"
);

/// Group code / value pairs of the MLEADERSTYLE record with handle `handle`,
/// without the reactor block (not modelled by this object).
fn mleaderstyle_pairs(text: &str, handle: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let pairs: Vec<(i32, String)> = lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap_or(-1), c[1].trim().to_string()))
        .collect();
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, "MLEADERSTYLE".into()) && w[1] == (5, handle.into()))
        .expect("MLEADERSTYLE record");
    let mut out = Vec::new();
    let mut in_reactors = false;
    for (code, value) in &pairs[start + 1..] {
        match (*code, value.as_str()) {
            (0, _) => break,
            (102, "{ACAD_REACTORS") => in_reactors = true,
            (102, "}") => in_reactors = false,
            _ if in_reactors => {}
            _ => out.push((*code, value.clone())),
        }
    }
    out
}

#[test]
fn autocad_mleaderstyle_round_trips_through_dxf() {
    let source = std::fs::read_to_string(FIXTURE).expect("fixture");
    let doc = DxfReader::from_reader(Cursor::new(source.clone().into_bytes()))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let written = String::from_utf8(
        DxfWriter::new(&doc)
            .with_version(doc.version)
            .write_to_vec()
            .expect("write dxf"),
    )
    .unwrap();
    let before = mleaderstyle_pairs(&source, "67");
    assert!(
        before.iter().any(|p| *p == (298, "1".into())),
        "fixture check"
    );
    assert_eq!(mleaderstyle_pairs(&written, "67"), before);
}

#[test]
fn mleaderstyle_colours_round_trip_through_dxf() {
    use acadrust::objects::{MultiLeaderStyle, ObjectType};
    use acadrust::types::{Color, DxfVersion};
    use acadrust::CadDocument;

    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = MultiLeaderStyle::new("Colours");
    style.handle = doc.allocate_handle();
    style.owner_handle = doc.header.named_objects_dict_handle;
    style.line_color = Color::ByLayer;
    style.text_color = Color::Rgb {
        r: 255,
        g: 128,
        b: 0,
    };
    style.block_content_color = Color::Index(3);
    let handle = style.handle;
    doc.objects
        .insert(handle, ObjectType::MultiLeaderStyle(style));

    let bytes = DxfWriter::new(&doc).write_to_vec().expect("write dxf");
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    match back.objects.get(&handle) {
        Some(ObjectType::MultiLeaderStyle(s)) => {
            assert_eq!(s.line_color, Color::ByLayer);
            assert_eq!(
                s.text_color,
                Color::Rgb {
                    r: 255,
                    g: 128,
                    b: 0
                }
            );
            assert_eq!(s.block_content_color, Color::Index(3));
        }
        other => panic!("MLEADERSTYLE lost: {other:?}"),
    }
}

fn style_round_trip(
    style: acadrust::objects::MultiLeaderStyle,
) -> (String, acadrust::objects::MultiLeaderStyle) {
    use acadrust::objects::ObjectType;
    use acadrust::types::DxfVersion;
    use acadrust::CadDocument;

    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut style = style;
    style.handle = doc.allocate_handle();
    style.owner_handle = doc.header.named_objects_dict_handle;
    let handle = style.handle;
    doc.objects
        .insert(handle, ObjectType::MultiLeaderStyle(style));
    let bytes = DxfWriter::new(&doc).write_to_vec().expect("write dxf");
    let text = String::from_utf8(bytes.clone()).unwrap();
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    match back.objects.get(&handle) {
        Some(ObjectType::MultiLeaderStyle(s)) => (text, s.clone()),
        other => panic!("MLEADERSTYLE lost: {other:?}"),
    }
}

#[test]
fn mleaderstyle_default_text_round_trips_in_group_300() {
    let mut style = acadrust::objects::MultiLeaderStyle::new("Notes");
    style.default_text = "TYP.".into();
    let (_, back) = style_round_trip(style);
    assert_eq!(back.default_text, "TYP.");
    assert_ne!(back.description, "TYP.");
}

#[test]
fn mleaderstyle_description_is_written_in_group_3() {
    // A style read from DWG has a description and no name of its own.
    let mut style = acadrust::objects::MultiLeaderStyle::new("");
    style.description = "Standard".into();
    let (text, back) = style_round_trip(style);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let at = (0..lines.len() - 1)
        .find(|&i| {
            lines[i] == "MLEADERSTYLE"
                && lines[i - 1] == "0"
                && lines[i + 1] == "5"
                && lines[i + 2] == format!("{:X}", back.handle.value())
        })
        .expect("MLEADERSTYLE record");
    let three = (at + 1..lines.len() - 1)
        .step_by(2)
        .take_while(|&i| lines[i] != "0")
        .find(|&i| lines[i] == "3")
        .map(|i| lines[i + 1]);
    assert_eq!(three, Some("Standard"));
    assert_eq!(back.description, "Standard");
}

#[test]
fn mleaderstyles_are_named_from_their_dictionary_keys() {
    use acadrust::objects::{MultiLeaderStyle, ObjectType};
    use acadrust::types::DxfVersion;
    use acadrust::CadDocument;

    // AutoCAD's two default styles: keys "Standard" and "Annotative", both
    // with the description (group 3) "Standard".
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let root = doc.header.named_objects_dict_handle;
    let dictionary = match doc.objects.get(&root) {
        Some(ObjectType::Dictionary(d)) => d.get("ACAD_MLEADERSTYLE").expect("dictionary"),
        _ => panic!("named object dictionary"),
    };
    let standard = match doc.objects.get(&dictionary) {
        Some(ObjectType::Dictionary(d)) => d.get("Standard").expect("Standard style"),
        _ => panic!("ACAD_MLEADERSTYLE"),
    };
    if let Some(ObjectType::MultiLeaderStyle(s)) = doc.objects.get_mut(&standard) {
        s.description = "Standard".into();
    }
    let mut annotative = MultiLeaderStyle::new("Annotative");
    annotative.handle = doc.allocate_handle();
    annotative.owner_handle = dictionary;
    annotative.description = "Standard".into();
    annotative.is_annotative = true;
    let annotative_handle = annotative.handle;
    doc.objects
        .insert(annotative_handle, ObjectType::MultiLeaderStyle(annotative));
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dictionary) {
        d.entries.push(("Annotative".into(), annotative_handle));
    }

    let bytes = DxfWriter::new(&doc).write_to_vec().expect("write dxf");
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let name = |handle| match back.objects.get(&handle) {
        Some(ObjectType::MultiLeaderStyle(s)) => (s.name.clone(), s.description.clone()),
        other => panic!("MLEADERSTYLE lost: {other:?}"),
    };
    assert_eq!(name(standard), ("Standard".into(), "Standard".into()));
    assert_eq!(
        name(annotative_handle),
        ("Annotative".into(), "Standard".into())
    );
}
