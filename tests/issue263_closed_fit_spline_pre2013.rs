//! Issue #263: a closed fit-point spline written to a DWG older than R2013
//! must reopen closed. The fit-data body of those versions has no closed
//! bit; the format (AutoCAD, ZWCAD) states closure by repeating the first
//! fit point at the end of the list, which the reader already understands.

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

fn roundtrip(version: DxfVersion, spline: Spline) -> Spline {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::Spline(spline)).unwrap();
    let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write failed");
    let read = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed");
    read.entities()
        .find_map(|e| match e {
            EntityType::Spline(s) => Some(s.clone()),
            _ => None,
        })
        .expect("SPLINE missing after roundtrip")
}

#[test]
fn closed_fit_spline_stays_closed_before_r2013() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut spline = Spline::from_fit_points(fit_points());
        spline.flags.closed = true;
        spline.flags.periodic = true;

        let read = roundtrip(version, spline);
        assert_eq!(read.dwg_scenario, Some(2), "{version:?}: fit-point record");
        assert!(read.flags.closed, "{version:?}: closed flag lost");
        assert_eq!(
            &read.fit_points[..4],
            &fit_points()[..],
            "{version:?}: fit points changed"
        );
        // Either the four authored points (R2013+, closed bit) or the same
        // points closed by a repeated first point (earlier versions).
        assert!(
            read.fit_points.len() == 4
                || (read.fit_points.len() == 5 && read.fit_points[4] == fit_points()[0]),
            "{version:?}: fit points {:?}",
            read.fit_points
        );
    }
}

#[test]
fn already_closed_fit_list_is_not_extended() {
    let mut points = fit_points();
    points.push(points[0]);
    let mut spline = Spline::from_fit_points(points.clone());
    spline.flags.closed = true;

    let read = roundtrip(DxfVersion::AC1018, spline);
    assert!(read.flags.closed);
    assert_eq!(read.fit_points, points);
}

#[test]
fn open_fit_spline_keeps_its_points() {
    let read = roundtrip(DxfVersion::AC1018, Spline::from_fit_points(fit_points()));
    assert!(!read.flags.closed);
    assert_eq!(read.fit_points, fit_points());
}
