//! A 3DSOLID / REGION / BODY / SURFACE without ACIS data is written to DXF as
//! one empty `1 ""` group. Read back, it must still have no data.

use std::io::Cursor;

use acadrust::entities::acis::primitives::build_box;
use acadrust::entities::solid3d::{AcisData, Body, Region, Solid3D};
use acadrust::entities::surface::{Surface, SurfaceKind};
use acadrust::{CadDocument, DxfReader, DxfVersion, DxfWriter, EntityType, Handle};

const VERSIONS: [DxfVersion; 3] = [DxfVersion::AC1015, DxfVersion::AC1024, DxfVersion::AC1027];

fn round_trip(version: DxfVersion, entity: EntityType) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let handle = document.add_entity(entity).unwrap();
    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let read = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    (read, handle)
}

fn acis_of(document: &CadDocument, handle: Handle) -> &AcisData {
    match document.get_entity(handle).expect("entity kept") {
        EntityType::Solid3D(e) => &e.acis_data,
        EntityType::Region(e) => &e.acis_data,
        EntityType::Body(e) => &e.acis_data,
        EntityType::Surface(e) => &e.acis_data,
        other => panic!("unexpected entity {other:?}"),
    }
}

fn assert_empty(version: DxfVersion, name: &str, entity: EntityType) {
    let (document, handle) = round_trip(version, entity);
    let acis = acis_of(&document, handle);
    assert!(
        !acis.has_data(),
        "{name} {version:?}: empty body came back with data {:?}",
        acis.sat_data
    );
    assert!(
        acis.sat_data.is_empty(),
        "{name} {version:?}: sat_data {:?}",
        acis.sat_data
    );
}

#[test]
fn empty_solid3d_stays_empty() {
    for version in VERSIONS {
        assert_empty(version, "3DSOLID", EntityType::Solid3D(Solid3D::new()));
    }
}

#[test]
fn empty_region_stays_empty() {
    for version in VERSIONS {
        assert_empty(version, "REGION", EntityType::Region(Region::new()));
    }
}

#[test]
fn empty_body_stays_empty() {
    for version in VERSIONS {
        assert_empty(version, "BODY", EntityType::Body(Body::new()));
    }
}

#[test]
fn empty_surface_stays_empty() {
    for version in VERSIONS {
        for kind in [SurfaceKind::Generic, SurfaceKind::Plane] {
            assert_empty(
                version,
                "SURFACE",
                EntityType::Surface(Box::new(Surface::new(kind))),
            );
        }
    }
}

// Control: a body with data keeps it.
#[test]
fn non_empty_body_keeps_its_data() {
    let sat = build_box([0.0, 0.0, 0.0], 2.0, 3.0, 4.0).to_sat_string();
    for version in [DxfVersion::AC1015, DxfVersion::AC1024] {
        let mut solid = Solid3D::new();
        solid.acis_data = AcisData::from_sat(&sat);
        let (document, handle) = round_trip(version, EntityType::Solid3D(solid));
        let acis = acis_of(&document, handle);
        assert!(acis.has_data(), "{version:?}: data lost");
        assert!(
            acis.sat_data.contains("body"),
            "{version:?}: sat_data {:?}",
            acis.sat_data
        );
    }
}
