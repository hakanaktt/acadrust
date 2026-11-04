//! Tests writing of HATCH boundary handles in DXF export.

use acadrust::entities::hatch::{BoundaryEdge, BoundaryPath, LineEdge};
use acadrust::entities::{EntityType, Hatch, Line};
use acadrust::io::dxf::{DxfReader, DxfTextWriter, DxfWriter, SectionWriter};
use acadrust::types::{Handle, Vector2, Vector3};
use acadrust::CadDocument;
use std::io::Cursor;

#[test]
fn test_hatch_boundary_handles_written_and_filtered() {
    let mut document = CadDocument::new();

    let line1 = Line::from_points(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    let line1_handle = document.add_entity(EntityType::Line(line1)).unwrap();

    let line2 = Line::from_points(Vector3::new(10.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0));
    let line2_handle = document.add_entity(EntityType::Line(line2)).unwrap();

    let mut hatch = Hatch::solid();
    let mut path = BoundaryPath::new();
    path.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(0.0, 0.0),
        end: Vector2::new(10.0, 0.0),
    }));

    // Include valid handles, NULL handle, and non-existent handle
    path.boundary_handles.push(line1_handle);
    path.boundary_handles.push(Handle::NULL);
    path.boundary_handles.push(Handle::new(0xDEADBEEF));
    path.boundary_handles.push(line2_handle);

    hatch.paths.push(path);
    let hatch_handle = document
        .add_entity(EntityType::Hatch(Box::new(hatch)))
        .unwrap();

    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();

    let read = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();

    let EntityType::Hatch(read_hatch) = read.get_entity(hatch_handle).unwrap() else {
        panic!("expected hatch");
    };

    // Only existing entities (line1 and line2) should remain; NULL and non-existent handles filtered out
    assert_eq!(
        read_hatch.paths[0].boundary_handles,
        vec![line1_handle, line2_handle]
    );
}

#[test]
fn test_hatch_boundary_handles_unfiltered_when_valid_handles_empty() {
    let mut document = CadDocument::new();

    let mut hatch = Hatch::solid();
    let mut path = BoundaryPath::new();
    path.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(0.0, 0.0),
        end: Vector2::new(10.0, 0.0),
    }));
    let h1 = Handle::new(0x10);
    let h2 = Handle::new(0x20);
    path.boundary_handles.push(h1);
    path.boundary_handles.push(h2);
    hatch.paths.push(path);

    document
        .add_entity(EntityType::Hatch(Box::new(hatch)))
        .unwrap();

    let mut buf = Vec::new();
    {
        let mut text_writer = DxfTextWriter::new(&mut buf);
        let mut section_writer = SectionWriter::new(&mut text_writer, 1, 1);
        // Note: valid_handles is empty because build_valid_handles was not called
        section_writer.write_entities(&document).unwrap();
    }

    let output = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = output.lines().map(|l| l.trim()).collect();
    let idx_97 = lines
        .iter()
        .position(|&l| l == "97")
        .expect("code 97 present");
    assert_eq!(lines[idx_97 + 1], "2");
    assert_eq!(lines[idx_97 + 2], "330");
    assert_eq!(lines[idx_97 + 3], "10");
    assert_eq!(lines[idx_97 + 4], "330");
    assert_eq!(lines[idx_97 + 5], "20");
}
