//! A DIMASSOC point reference with osnap type 0 (a geometry point, no object
//! snap) that names an object stores its main sub-entity (subent type, GS
//! marker, xref paths) like any other reference with objects: whether the
//! sub-entity is stored depends on the object count only. A reference with no
//! objects (the `_End` last point) still stores none.
//! Built from `fixtures/associative/dimassoc-perpendicular.dwg`.

use std::io::Cursor;

use acadrust::objects::{
    AssocDimensionAssociation, AssocDimensionReference, AssociativeData, ObjectType,
};
use acadrust::types::Handle;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

const DIMASSOC: u64 = 0x401;

fn fixture() -> CadDocument {
    DwgReader::from_stream(Cursor::new(include_bytes!(
        "fixtures/associative/dimassoc-perpendicular.dwg"
    )))
    .read()
    .expect("AutoCAD's DIMASSOC reads without recovery")
}

fn association(document: &CadDocument) -> &AssocDimensionAssociation {
    match &document.objects[&Handle::new(DIMASSOC)] {
        ObjectType::Associative(object) => match &object.data {
            AssociativeData::DimensionAssociation(association) => association,
            other => panic!("expected a dimension association, got {other:?}"),
        },
        other => panic!("expected an associative object, got {other:?}"),
    }
}

fn first_reference_mut(document: &mut CadDocument) -> &mut AssocDimensionReference {
    match document.objects.get_mut(&Handle::new(DIMASSOC)) {
        Some(ObjectType::Associative(object)) => match &mut object.data {
            AssociativeData::DimensionAssociation(association) => &mut association.references[1][0],
            other => panic!("expected a dimension association, got {other:?}"),
        },
        other => panic!("expected an associative object, got {other:?}"),
    }
}

/// The fixture with its perpendicular reference turned into a geometry point
/// reference (osnap 0) that keeps its object, subent type 2, GS marker 4 and
/// gains an xref path.
fn geometry_point_source() -> CadDocument {
    let mut source = fixture();
    let reference = first_reference_mut(&mut source);
    assert_eq!(reference.xrefs.len(), 1);
    reference.osnap_type = 0;
    reference.xref_paths = vec!["XREF".to_string()];
    source
}

fn assert_geometry_point_references(association: &AssocDimensionAssociation) {
    let chain = &association.references[1];
    assert_eq!(
        chain.len(),
        2,
        "geometry point reference plus its last point"
    );

    let geometry = &chain[0];
    assert_eq!(geometry.osnap_type, 0);
    assert_eq!(geometry.xrefs.len(), 1);
    assert_eq!(geometry.main_subent_type, 2, "subent type");
    assert_eq!(geometry.main_gs_marker, 4, "GS marker");
    assert_eq!(geometry.xref_paths, ["XREF"], "xref paths");
    assert_eq!(geometry.osnap_distance, 0.5);
    assert!(geometry.has_last_point_reference);

    let last_point = &chain[1];
    assert_eq!(last_point.osnap_type, 1);
    assert!(last_point.xrefs.is_empty());
    assert_eq!(last_point.osnap_distance, 1.0);
    assert_eq!(last_point.osnap_point.z, 2.0e50);
}

fn dxf_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|line| line.trim().to_string())
        .collect()
}

/// Group codes of the first `AcDbOsnapPointRef` from its osnap type (72) to
/// its osnap distance (40).
fn first_reference_codes(lines: &[String]) -> Vec<String> {
    let start = lines
        .iter()
        .position(|line| line == "AcDbOsnapPointRef")
        .expect("a point reference in the DXF");
    let mut codes = Vec::new();
    for index in (start + 1..lines.len()).step_by(2) {
        codes.push(lines[index].clone());
        if lines[index] == "40" {
            break;
        }
    }
    codes
}

#[test]
fn main_subentity_depends_on_object_count_only() {
    assert!(AssocDimensionReference::stores_main_subentity(0, 1));
    assert!(AssocDimensionReference::stores_main_subentity(8, 1));
    assert!(!AssocDimensionReference::stores_main_subentity(0, 0));
    assert!(!AssocDimensionReference::stores_main_subentity(1, 0));
}

#[test]
fn geometry_point_reference_keeps_subentity_through_dwg() {
    let source = geometry_point_source();
    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&source).unwrap()))
        .read()
        .unwrap();
    assert_geometry_point_references(association(&dwg));
}

#[test]
fn geometry_point_reference_keeps_subentity_through_dxf() {
    let source = geometry_point_source();
    let text = DxfWriter::new(&source).write_to_vec().unwrap();
    assert_eq!(
        first_reference_codes(&dxf_lines(&text)),
        ["72", "331", "73", "91", "301", "40"],
        "group codes of the geometry point reference"
    );

    let dxf = DxfReader::from_reader(Cursor::new(text))
        .unwrap()
        .read()
        .unwrap();
    assert_geometry_point_references(association(&dxf));
}

/// A DXF in which the reference with osnap 0 carries 331, 73 and 91, as
/// written for a geometry point reference with an object, reads its
/// sub-entity back.
#[test]
fn dxf_reader_reads_subentity_of_geometry_point_reference() {
    // The fixture written to DXF has the perpendicular reference's 73/91;
    // only its osnap type is switched to 0 in the text.
    let text = DxfWriter::new(&fixture()).write_to_vec().unwrap();
    let text = String::from_utf8(text).unwrap();
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == "AcDbOsnapPointRef")
        .expect("a point reference in the DXF");
    assert_eq!(lines[start + 1].trim(), "72");
    assert_eq!(lines[start + 2].trim(), "8");
    lines[start + 2] = "0\n";
    let patched: String = lines.concat();

    let dxf = DxfReader::from_reader(Cursor::new(patched.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let reference = &association(&dxf).references[1][0];
    assert_eq!(reference.osnap_type, 0);
    assert_eq!(reference.xrefs.len(), 1);
    assert_eq!(reference.main_subent_type, 2, "subent type");
    assert_eq!(reference.main_gs_marker, 4, "GS marker");
    assert_eq!(reference.osnap_distance, 0.5);
    assert!(reference.has_last_point_reference);
}

/// Control: a reference with osnap 0 and no object stores no sub-entity, in
/// DWG and DXF alike.
#[test]
fn geometry_point_reference_without_objects_has_no_subentity() {
    let mut source = fixture();
    let reference = first_reference_mut(&mut source);
    reference.osnap_type = 0;
    reference.xrefs.clear();
    reference.main_subent_type = 0;
    reference.main_gs_marker = 0;

    let text = DxfWriter::new(&source).write_to_vec().unwrap();
    assert_eq!(first_reference_codes(&dxf_lines(&text)), ["72", "40"]);
    let dxf = DxfReader::from_reader(Cursor::new(text))
        .unwrap()
        .read()
        .unwrap();
    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&source).unwrap()))
        .read()
        .unwrap();
    for document in [&dxf, &dwg] {
        let chain = &association(document).references[1];
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].osnap_type, 0);
        assert!(chain[0].xrefs.is_empty());
        assert_eq!(chain[0].osnap_distance, 0.5);
        assert!(chain[0].has_last_point_reference);
        assert_eq!(chain[1].osnap_type, 1, "the chained reference still reads");
    }
}
