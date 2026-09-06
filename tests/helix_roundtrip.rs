//! Round-trip coverage for the HELIX entity (AcDbHelix).
//!
//! A helix is a spline plus generating parameters, so the wire record is the
//! full spline record followed by the helix fields. These tests lock in both
//! the DWG codec (spline reader/writer reuse + helix param tail) and the DXF
//! codec (subclass-disambiguated shared group codes 10/11/12/40/41/42).

use std::io::Cursor;

use acadrust::entities::{EntityType, Helix, HelixConstraint, Spline};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn dwg_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("DWG write failed");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed")
}

fn dxf_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc)
        .write_to_vec()
        .expect("DXF write failed");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

fn sample_helix() -> Helix {
    let mut h = Helix::new();
    h.spline = Spline::from_control_points(
        3,
        vec![
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 1.0),
            Vector3::new(-1.0, 0.0, 2.0),
            Vector3::new(0.0, -1.0, 3.0),
        ],
    );
    h.major_version = 29;
    h.maintenance_version = 0;
    h.axis_base_point = Vector3::new(2.0, 3.0, 0.0);
    h.start_point = Vector3::new(3.0, 3.0, 0.0);
    h.axis_vector = Vector3::new(0.0, 0.0, 1.0);
    h.radius = 1.5;
    h.turns = 4.0;
    h.turn_height = 0.75;
    h.handedness = true;
    h.constraint = HelixConstraint::Turns;
    h
}

fn roundtrip_helix(via_dwg: bool, version: DxfVersion) -> Helix {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::Helix(sample_helix())).unwrap();
    let rt = if via_dwg {
        dwg_roundtrip(&doc)
    } else {
        dxf_roundtrip(&doc)
    };
    let found = rt.entities().find_map(|e| match e {
        EntityType::Helix(h) => Some(h.clone()),
        _ => None,
    });
    found.expect("HELIX missing after roundtrip")
}

fn assert_helix(h: &Helix, label: &str) {
    // Helix parameters.
    assert_eq!(h.radius, 1.5, "{label}: radius");
    assert_eq!(h.turns, 4.0, "{label}: turns");
    assert_eq!(h.turn_height, 0.75, "{label}: turn height");
    assert!(h.handedness, "{label}: handedness");
    assert_eq!(h.constraint, HelixConstraint::Turns, "{label}: constraint");
    assert_eq!(
        h.axis_base_point,
        Vector3::new(2.0, 3.0, 0.0),
        "{label}: axis base point"
    );
    assert_eq!(
        h.start_point,
        Vector3::new(3.0, 3.0, 0.0),
        "{label}: start point"
    );
    assert_eq!(
        h.axis_vector,
        Vector3::new(0.0, 0.0, 1.0),
        "{label}: axis vector"
    );
    assert_eq!(h.major_version, 29, "{label}: major version");
    // Embedded spline geometry.
    assert_eq!(h.spline.degree, 3, "{label}: spline degree");
    assert_eq!(
        h.spline.control_points.len(),
        4,
        "{label}: control point count"
    );
    assert_eq!(
        h.spline.control_points[0],
        Vector3::new(1.0, 0.0, 0.0),
        "{label}: first control point"
    );
    assert_eq!(
        h.spline.control_points[3],
        Vector3::new(0.0, -1.0, 3.0),
        "{label}: last control point"
    );
}

#[test]
fn helix_dwg_roundtrip() {
    // Exercise both the pre-2013 and R2013+ spline scenario branches.
    for version in [DxfVersion::AC1018, DxfVersion::AC1032] {
        let h = roundtrip_helix(true, version);
        assert_helix(&h, &format!("DWG {version:?}"));
    }
}

#[test]
fn helix_dxf_roundtrip() {
    let h = roundtrip_helix(false, DxfVersion::AC1032);
    assert_helix(&h, "DXF");
}
