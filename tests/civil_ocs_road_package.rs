//! The road package OpenCAD Studio builds in code (`civil_packages::road`),
//! copied here: it must survive a write and read, lay out as a half section
//! and carry the codes its corridor uses.

use acadrust::classes::civil::pkt::{
    chain, layout, link, new_part_name, parameter_double, parameter_grade, parameter_side, point_delta, point_slope, read_pkt, shape, write_pkt,
    PktPackage, PktSide, PktValue, PointTargets,
};

/// A road's half section: a lane from the baseline, a shoulder, and a side
/// slope out from the shoulder, with the pavement under lane and shoulder.
pub fn road() -> PktPackage {
    const NAME: &str = "OCSRoad";
    let mut p = PktPackage::new_package(NAME, &new_part_name("OpenCAD Studio OCSRoad 1"));
    p.description = "Lane, shoulder and side slope of one side of a road".into();
    p.parameters = vec![
        parameter_side(PktSide::Right),
        parameter_double("LaneWidth", 3.5, "Lane width"),
        parameter_grade("LaneSlope", -0.02, "Lane cross slope"),
        parameter_double("ShoulderWidth", 1.0, "Shoulder width"),
        parameter_grade("ShoulderSlope", -0.04, "Shoulder cross slope"),
        parameter_double("PaveDepth", 0.3, "Pavement depth"),
        parameter_double("SideSlopeWidth", 3.0, "Side slope width"),
        parameter_grade("SideSlope", -0.5, "Side slope grade"),
    ];
    let none = PointTargets::default();
    let (flowchart, start) = chain(vec![
        point_delta(1, "P1", None, None, "0", "0", &["Crown"]),
        point_slope(2, "P2", "P1", Some(("L1", &["Top", "Pave"])), "[LaneSlope]", "[LaneWidth]", none, &["ETW"]),
        point_slope(3, "P3", "P2", Some(("L2", &["Top", "Pave"])), "[ShoulderSlope]", "[ShoulderWidth]", none, &["EPS"]),
        point_slope(4, "P4", "P3", Some(("L3", &["Top", "Daylight"])), "[SideSlope]", "[SideSlopeWidth]", none, &["Daylight"]),
        point_delta(5, "P5", Some("P3"), Some(("L4", &[])), "0", "[-PaveDepth]", &["EPS_Datum"]),
        point_delta(6, "P6", Some("P1"), None, "0", "[-PaveDepth]", &["Crown_Datum"]),
        link(7, "L5", "P5", "P6", &["Datum"]),
        link(8, "L6", "P6", "P1", &[]),
        shape(9, "S1", &["L1", "L2", "L4", "L5", "L6"], &["Pave"]),
    ]);
    p.flowchart = flowchart;
    p.start = start;
    p
}

#[test]
fn ocs_road_package_reads_back_and_lays_out_a_half_section() {
    let pkg = road();
    let bytes = write_pkt(&pkg).unwrap();
    let back = read_pkt(&bytes).unwrap();
    assert_eq!(back.name, "OCSRoad");
    let names: Vec<&str> = back.parameters.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Side", "LaneWidth", "LaneSlope", "ShoulderWidth", "ShoulderSlope", "PaveDepth", "SideSlopeWidth", "SideSlope"]);
    for (side, sign) in [(PktSide::Right, 1.0), (PktSide::Left, -1.0)] {
        let l = layout(&back, side, &[]).unwrap();
        let at: Vec<(f64, f64)> = l.points.iter().map(|p| ((p.offset * 1e6).round() / 1e6, (p.elevation * 1e6).round() / 1e6)).collect();
        assert_eq!(at, [(0.0, 0.0), (3.5 * sign, -0.07), (4.5 * sign, -0.11), (7.5 * sign, -1.61), (4.5 * sign, -0.41), (0.0, -0.3)]);
        assert_eq!(l.links.len(), 6);
        assert_eq!(l.shapes.len(), 1);
        for code in ["Crown", "ETW", "EPS", "Daylight", "Top", "Pave", "Datum"] {
            assert!(l.codes.iter().any(|c| c == code), "{code}");
        }
    }
    let wide = layout(&back, PktSide::Right, &[("LaneWidth".into(), PktValue::Double(4.0))]).unwrap();
    assert!((wide.points[1].offset - 4.0).abs() < 1e-9);
    assert!((wide.points[2].offset - 5.0).abs() < 1e-9);
}
