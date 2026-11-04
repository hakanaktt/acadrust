//! Tests writing of entity and layout reactors in DXF export.

use acadrust::entities::{Circle, EntityType, Line};
use acadrust::io::dxf::{DxfReader, DxfTextWriter, DxfWriter, SectionWriter};
use acadrust::types::{Handle, Vector3};
use acadrust::CadDocument;
use std::io::Cursor;

fn dxf_entity_reactors(text: &str, entity_type: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<(&str, &str)> = lines
        .chunks(2)
        .map(|p| (p[0], p.get(1).copied().unwrap_or("")))
        .collect();
    let start = pairs
        .iter()
        .position(|p| *p == ("0", entity_type))
        .unwrap_or_else(|| panic!("no {entity_type}"));
    let mut reactors = Vec::new();
    let mut inside = false;
    for &(code, value) in &pairs[start + 1..] {
        match (code, value) {
            ("0", _) => break,
            ("102", "{ACAD_REACTORS") => inside = true,
            ("102", "}") => inside = false,
            ("330", handle) if inside => reactors.push(handle.to_string()),
            _ => {}
        }
    }
    reactors
}

#[test]
fn test_entity_reactors_written_and_filtered() {
    let mut document = CadDocument::new();

    let line1 = Line::from_points(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    let line1_handle = document.add_entity(EntityType::Line(line1)).unwrap();

    let line2 = Line::from_points(Vector3::new(10.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0));
    let line2_handle = document.add_entity(EntityType::Line(line2)).unwrap();

    let mut circle = Circle::from_center_radius(Vector3::ZERO, 5.0);
    // Add valid handles, NULL handle, and non-existent handle
    circle.common.reactors.push(line1_handle);
    circle.common.reactors.push(Handle::NULL);
    circle.common.reactors.push(Handle::new(0xDEADBEEF));
    circle.common.reactors.push(line2_handle);

    let circle_handle = document
        .add_entity(EntityType::Circle(circle))
        .unwrap();

    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let text = String::from_utf8_lossy(&bytes);

    let reactors = dxf_entity_reactors(&text, "CIRCLE");
    // Only line1 and line2 should be written; NULL and non-existent handles filtered out
    assert_eq!(
        reactors,
        vec![format!("{line1_handle:X}"), format!("{line2_handle:X}")]
    );

    // Read back through DxfReader to verify complete roundtrip
    let read = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    let EntityType::Circle(read_circle) = read.get_entity(circle_handle).unwrap() else {
        panic!("expected circle");
    };
    assert_eq!(read_circle.common.reactors, vec![line1_handle, line2_handle]);
}

#[test]
fn test_entity_reactors_unfiltered_when_valid_handles_empty() {
    let mut document = CadDocument::new();

    let mut circle = Circle::from_center_radius(Vector3::ZERO, 5.0);
    let h1 = Handle::new(0x10);
    let h2 = Handle::new(0x20);
    circle.common.reactors.push(h1);
    circle.common.reactors.push(h2);

    document.add_entity(EntityType::Circle(circle)).unwrap();

    let mut buf = Vec::new();
    {
        let mut text_writer = DxfTextWriter::new(&mut buf);
        let mut section_writer = SectionWriter::new(&mut text_writer, 1, 1);
        // Note: valid_handles is empty because build_valid_handles was not called
        section_writer.write_entities(&document).unwrap();
    }

    let text = String::from_utf8_lossy(&buf);
    let reactors = dxf_entity_reactors(&text, "CIRCLE");
    assert_eq!(reactors, vec!["10", "20"]);
}
