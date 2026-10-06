use std::io::Cursor;

use acadrust::entities::{EntityType, Spline};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

#[test]
fn dwg_spline_exposes_form_when_flags_are_not_stored() {
    let points = vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(5.0, 10.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
    ];

    for version in [DxfVersion::AC1015, DxfVersion::AC1027] {
        let mut fit = Spline::from_fit_points(points.clone());
        fit.flags.closed = true;
        assert_eq!(fit.dwg_scenario, None);

        let mut control = Spline::from_control_points(2, points.clone());
        control.flags.closed = true;
        assert_eq!(control.dwg_scenario, None);

        let mut document = CadDocument::with_version(version);
        document.add_entity(EntityType::Spline(fit)).unwrap();
        document.add_entity(EntityType::Spline(control)).unwrap();

        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let splines: Vec<_> = read
            .entities()
            .filter_map(|entity| match entity {
                EntityType::Spline(spline) => Some(spline),
                _ => None,
            })
            .collect();

        assert_eq!(splines.len(), 2, "{version:?}");
        let fit = splines.iter().find(|s| s.dwg_scenario == Some(2)).unwrap();
        assert_eq!(
            fit.flags.closed,
            version == DxfVersion::AC1027,
            "{version:?}"
        );
        assert!(!fit.flags.periodic, "{version:?}");

        let control = splines.iter().find(|s| s.dwg_scenario == Some(1)).unwrap();
        assert!(control.flags.closed, "{version:?}");
    }
}
