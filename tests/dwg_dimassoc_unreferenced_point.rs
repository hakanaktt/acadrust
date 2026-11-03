//! A DIMASSOC whose `Perpendicular` point reference chains an `_End` "last
//! point" reference that snaps to no object. AutoCAD stores such a reference
//! without a main sub-entity (no subent type, GS marker or xref paths);
//! reading those fields anyway ran past the end of the record.
//! See `fixtures/associative/README.md` for the fixture.

use std::io::Cursor;

use acadrust::objects::{AssocDimensionAssociation, AssociativeData, ObjectType};
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

fn assert_autocad_references(association: &AssocDimensionAssociation) {
    assert_eq!(association.associativity, 2, "second point only");
    let chain = &association.references[1];
    assert_eq!(
        chain.len(),
        2,
        "perpendicular reference plus its last point"
    );

    let perpendicular = &chain[0];
    assert_eq!(perpendicular.osnap_type, 8);
    assert_eq!(perpendicular.xrefs.len(), 1);
    assert_eq!(perpendicular.main_subent_type, 2);
    assert_eq!(perpendicular.main_gs_marker, 4);
    assert_eq!(perpendicular.osnap_distance, 0.5);
    assert!(perpendicular.has_last_point_reference);

    let last_point = &chain[1];
    assert_eq!(last_point.osnap_type, 1);
    assert!(last_point.xrefs.is_empty());
    assert_eq!(last_point.osnap_distance, 1.0);
    assert_eq!(last_point.osnap_point.z, 2.0e50);
    assert!(!last_point.has_last_point_reference);
}

#[test]
fn reference_without_objects_has_no_main_subentity() {
    assert_autocad_references(association(&fixture()));
}

#[test]
fn reference_without_objects_survives_dwg_and_dxf_saves() {
    let source = fixture();

    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&source).unwrap()))
        .read()
        .unwrap();
    assert_autocad_references(association(&dwg));

    let text = DxfWriter::new(&source).write_to_vec().unwrap();
    let dxf = DxfReader::from_reader(Cursor::new(text.clone()))
        .unwrap()
        .read()
        .unwrap();
    assert_autocad_references(association(&dxf));

    // Like AutoCAD's own DXF: the last point reference goes straight from
    // its osnap type to its distance, with no 331/73/91 in between.
    let lines: Vec<String> = String::from_utf8_lossy(&text)
        .lines()
        .map(|line| line.trim().to_string())
        .collect();
    let second = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| *line == "AcDbOsnapPointRef")
        .nth(1)
        .map(|(index, _)| index)
        .expect("two point references in the DXF");
    let codes: Vec<&str> = (1..)
        .step_by(2)
        .map(|offset| lines[second + offset].as_str())
        .take(3)
        .collect();
    assert_eq!(
        codes,
        ["72", "40", "10"],
        "group codes after the last point reference"
    );
}

/// An intersection reference (osnap 6 or 11) with no intersection object
/// stores no intersection subentity either, as the main reference does:
/// AutoCAD drawings carry such references, and reading the fields anyway
/// ran the record out of bits.
#[test]
fn intersection_reference_without_objects_has_no_intersection_subentity() {
    let mut source = fixture();
    match source.objects.get_mut(&Handle::new(DIMASSOC)) {
        Some(ObjectType::Associative(object)) => match &mut object.data {
            AssociativeData::DimensionAssociation(association) => {
                let reference = &mut association.references[1][0];
                reference.osnap_type = 11;
                reference.intersection_objects.clear();
            }
            other => panic!("expected a dimension association, got {other:?}"),
        },
        other => panic!("expected an associative object, got {other:?}"),
    }

    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&source).unwrap()))
        .read()
        .unwrap();
    let chain = &association(&dwg).references[1];
    assert_eq!(chain[0].osnap_type, 11);
    assert!(chain[0].has_last_point_reference);
    assert_eq!(chain[1].osnap_type, 1, "the chained reference still reads");

    let text = DxfWriter::new(&source).write_to_vec().unwrap();
    let lines: Vec<String> = String::from_utf8_lossy(&text)
        .lines()
        .map(|line| line.trim().to_string())
        .collect();
    let first = lines
        .iter()
        .position(|line| line == "AcDbOsnapPointRef")
        .expect("a point reference in the DXF");
    let after_point: Vec<&str> = (first + 1..lines.len())
        .step_by(2)
        .map(|index| lines[index].as_str())
        .skip_while(|code| *code != "30")
        .skip(1)
        .take(1)
        .collect();
    assert_eq!(after_point, ["75"], "no 74/92 after the osnap point");
}
