//! A closed fit-point spline written to an R2013+ DWG must keep its fit
//! points in AutoCAD. AutoCAD drops the fit data of a record whose spline
//! flags 1 carry the "is closed" bit (4) and reads it as a periodic
//! control-point spline. It writes closed fit splines itself with flags 1
//! = 9 and the first fit point repeated at the end of the list.

use std::io::Cursor;

use acadrust::entities::{EntityType, Spline};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn fit_points() -> Vec<Vector3> {
    vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(2.0, 3.0, 0.0),
        Vector3::new(5.0, -1.0, 0.0),
        Vector3::new(8.0, 2.0, 0.0),
    ]
}

fn closed_fit_points() -> Vec<Vector3> {
    let mut points = fit_points();
    points.push(points[0]);
    points
}

fn roundtrip(version: DxfVersion, spline: Spline) -> Spline {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::Spline(spline)).unwrap();
    let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write failed");
    let read = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed");
    let spline = read
        .entities()
        .find_map(|e| match e {
            EntityType::Spline(s) => Some(s.clone()),
            _ => None,
        })
        .expect("SPLINE missing after roundtrip");
    spline
}

#[test]
fn closed_fit_spline_is_written_like_autocad_r2013_plus() {
    for version in [DxfVersion::AC1027, DxfVersion::AC1032] {
        for periodic in [false, true] {
            let mut spline = Spline::from_fit_points(fit_points());
            spline.flags.closed = true;
            spline.flags.periodic = periodic;

            let read = roundtrip(version, spline);
            let label = format!("{version:?}, periodic {periodic}");
            assert_eq!(read.dwg_scenario, Some(2), "{label}: fit-point record");
            assert_eq!(read.dwg_flags1, 9, "{label}: spline flags 1");
            assert_eq!(read.fit_points, closed_fit_points(), "{label}: fit points");
            assert!(read.flags.closed, "{label}: closed flag lost");
        }
    }
}

#[test]
fn already_closed_fit_list_is_not_extended_r2013_plus() {
    let mut spline = Spline::from_fit_points(closed_fit_points());
    spline.flags.closed = true;

    let read = roundtrip(DxfVersion::AC1032, spline);
    assert_eq!(read.dwg_flags1, 9);
    assert_eq!(read.fit_points, closed_fit_points());
    assert!(read.flags.closed);
}

#[test]
fn open_fit_spline_is_unchanged_r2013_plus() {
    let read = roundtrip(DxfVersion::AC1032, Spline::from_fit_points(fit_points()));
    assert_eq!(read.dwg_flags1, 9);
    assert_eq!(read.fit_points, fit_points());
    assert!(!read.flags.closed);
}
