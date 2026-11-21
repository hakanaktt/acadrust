//! The AcDbDimension base groups of ARC_DIMENSION and
//! LARGE_RADIAL_DIMENSION — insertion point (12), horizontal direction
//! (51) and flip arrows (74/75) — survive a DXF write and read.
//!
//! The shared DIMENSION reader handles these groups, but the extended
//! reader used by the two entity types had no arms for them, so the
//! values were dropped — including on files the library itself writes.

use acadrust::entities::{
    Dimension, DimensionAligned, DimensionArc, DimensionBase, DimensionLargeRadial, EntityType,
};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("write dxf");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf")
}

fn set_base_groups(base: &mut DimensionBase) {
    base.insertion_point = Vector3::new(1.0, 2.0, 3.0);
    base.horizontal_direction = 0.5;
    base.flip_arrow1 = true;
    base.flip_arrow2 = true;
}

fn assert_base_groups(base: &DimensionBase) {
    let p = base.insertion_point;
    assert!(
        (p.x - 1.0).abs() < 1e-9 && (p.y - 2.0).abs() < 1e-9 && (p.z - 3.0).abs() < 1e-9,
        "insertion point lost: {p:?}"
    );
    assert!(
        (base.horizontal_direction - 0.5).abs() < 1e-9,
        "horizontal direction lost: {}",
        base.horizontal_direction
    );
    assert!(base.flip_arrow1, "flip arrow 1 lost");
    assert!(base.flip_arrow2, "flip arrow 2 lost");
}

#[test]
fn arc_dimension_base_groups_survive_dxf() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut dim = DimensionArc::default();
    dim.definition_point = Vector3::new(8.0, 8.0, 0.0);
    dim.base.definition_point = dim.definition_point;
    set_base_groups(&mut dim.base);
    let handle = doc
        .add_entity(EntityType::Dimension(Box::new(Dimension::Arc(dim))))
        .unwrap();
    match round_trip(&doc).get_entity(handle) {
        Some(EntityType::Dimension(d)) => match d.as_ref() {
            Dimension::Arc(arc) => assert_base_groups(&arc.base),
            other => panic!("not an arc dimension: {other:?}"),
        },
        other => panic!("dimension lost: {other:?}"),
    }
}

#[test]
fn large_radial_dimension_base_groups_survive_dxf() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut dim = DimensionLargeRadial::default();
    dim.definition_point = Vector3::new(8.0, 8.0, 0.0);
    dim.base.definition_point = dim.definition_point;
    set_base_groups(&mut dim.base);
    let handle = doc
        .add_entity(EntityType::Dimension(Box::new(Dimension::LargeRadial(dim))))
        .unwrap();
    match round_trip(&doc).get_entity(handle) {
        Some(EntityType::Dimension(d)) => match d.as_ref() {
            Dimension::LargeRadial(jogged) => assert_base_groups(&jogged.base),
            other => panic!("not a large radial dimension: {other:?}"),
        },
        other => panic!("dimension lost: {other:?}"),
    }
}

#[test]
fn dimension_base_groups_still_survive_dxf() {
    // Regression guard: the shared DIMENSION path keeps the same groups.
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut dim = DimensionAligned::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));
    set_base_groups(&mut dim.base);
    let handle = doc
        .add_entity(EntityType::Dimension(Box::new(Dimension::Aligned(dim))))
        .unwrap();
    match round_trip(&doc).get_entity(handle) {
        Some(EntityType::Dimension(d)) => assert_base_groups(d.base()),
        other => panic!("dimension lost: {other:?}"),
    }
}
