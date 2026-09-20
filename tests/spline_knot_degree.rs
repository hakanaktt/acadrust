//! Knot-less control-point splines whose degree exceeds what the control
//! point count allows must be written with generated knots that match the
//! written degree instead of overflowing during knot generation.

use std::io::Cursor;

use acadrust::entities::{EntityType, Spline};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

#[test]
fn knotless_two_point_cubic_spline_roundtrips_through_dwg() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1032] {
        let mut spline = Spline::new();
        spline.degree = 3;
        spline.control_points = vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 5.0, 0.0)];
        assert!(spline.knots.is_empty());

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

        let n = spline.control_points.len();
        assert_eq!(n, 2, "{version:?}: control points");
        assert!(spline.degree >= 1, "{version:?}: degree {}", spline.degree);
        assert!(
            spline.degree as usize <= n - 1,
            "{version:?}: degree {}",
            spline.degree
        );
        assert_eq!(
            spline.knots.len(),
            n + spline.degree as usize + 1,
            "{version:?}: knot count"
        );
    }
}
