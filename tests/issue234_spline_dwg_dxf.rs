//! Issue #234: DWG -> DXF: SPLINE data lost or invalid (fit-point splines and periodic spline)
//!
//! Tests that:
//! 1. Closed fit-point splines (where first fit point equals last fit point) retain their closed flag.
//! 2. Periodic splines are written to DXF as valid clamped B-splines where
//!    `knots.len() == control_points.len() + degree + 1` (AutoCAD writes 7 control points and 11 knots
//!    for a 4-point cubic periodic spline).

use acadrust::types::Handle;
use acadrust::{DwgReader, DxfReader, DxfWriter, EntityType};
use std::io::Cursor;
use std::path::Path;

fn load_fixture() -> acadrust::CadDocument {
    let path = Path::new("tests/fixtures/issue234/poly_self_intersection.dwg");
    DwgReader::from_file(path)
        .expect("open fixture dwg")
        .read()
        .expect("read fixture dwg")
}

#[test]
fn closed_fit_point_spline_retains_closed_flag() {
    let doc = load_fixture();
    let a0 = doc
        .entities()
        .find_map(|e| match e {
            EntityType::Spline(s) if s.common.handle == Handle::new(0xa0) => Some(s.clone()),
            _ => None,
        })
        .expect("spline A0");

    assert_eq!(a0.fit_points.len(), 5);
    assert_eq!(a0.fit_points.first(), a0.fit_points.last());
    assert!(
        a0.flags.closed,
        "fit-point spline with identical first and last fit points must be marked closed in memory"
    );

    // Write to DXF and read back
    let mut dxf_bytes = Vec::new();
    DxfWriter::new(&doc)
        .write_to_writer(&mut dxf_bytes)
        .expect("write dxf");

    let dxf_doc = DxfReader::from_reader(Cursor::new(dxf_bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf");

    let dxf_a0 = dxf_doc
        .entities()
        .find_map(|e| match e {
            EntityType::Spline(s) if s.common.handle == Handle::new(0xa0) => Some(s.clone()),
            _ => None,
        })
        .expect("dxf spline A0");

    assert!(
        dxf_a0.flags.closed,
        "spline A0 in DXF must have closed flag set"
    );
    assert!(
        dxf_a0.dxf_flags & 1 != 0,
        "spline A0 DXF group 70 must have bit 1 (closed) set, got {}",
        dxf_a0.dxf_flags
    );
}

#[test]
fn periodic_spline_writes_valid_nurbs_to_dxf() {
    let doc = load_fixture();
    let a2 = doc
        .entities()
        .find_map(|e| match e {
            EntityType::Spline(s) if s.common.handle == Handle::new(0xa2) => Some(s.clone()),
            _ => None,
        })
        .expect("spline A2");

    assert!(a2.flags.periodic);
    assert_eq!(a2.degree, 3);
    assert_eq!(a2.control_points.len(), 4);

    // Write to DXF and read back
    let mut dxf_bytes = Vec::new();
    DxfWriter::new(&doc)
        .write_to_writer(&mut dxf_bytes)
        .expect("write dxf");

    let dxf_doc = DxfReader::from_reader(Cursor::new(dxf_bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf");

    let dxf_a2 = dxf_doc
        .entities()
        .find_map(|e| match e {
            EntityType::Spline(s) if s.common.handle == Handle::new(0xa2) => Some(s.clone()),
            _ => None,
        })
        .expect("dxf spline A2");

    // DXF spline must be a valid NURBS: knots.len() == control_points.len() + degree + 1
    assert_eq!(
        dxf_a2.knots.len(),
        dxf_a2.control_points.len() + (dxf_a2.degree as usize) + 1,
        "DXF periodic spline must satisfy knots = control_points + degree + 1"
    );
    assert_eq!(dxf_a2.control_points.len(), 7, "expected 7 control points for clamped periodic cubic spline");
    assert_eq!(dxf_a2.knots.len(), 11, "expected 11 knots for clamped periodic cubic spline");
    assert!(
        dxf_a2.flags.closed,
        "periodic spline in DXF must be marked closed"
    );
    assert!(
        dxf_a2.flags.periodic,
        "periodic spline in DXF must retain periodic flag"
    );
}
