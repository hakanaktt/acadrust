use acadrust::entities::acis::{SatDocument, SatSplineSurface};

const HEADER: &str = "22300 1 1 0\n\
33 Open Design Alliance ACIS Builder 14 ACIS 223.00 NT 24 Mon Oct 26 07:34:15 2026\n\
1 1e-006 1e-010\n";

fn surface(record: &str) -> acadrust::entities::acis::SatBSplineSurface {
    let document = SatDocument::parse(&format!("{HEADER}{record}\nEnd-of-ACIS-data\n")).unwrap();
    let record = document
        .records
        .iter()
        .find(|record| record.entity_type == "spline-surface")
        .unwrap();
    SatSplineSurface::from_record(record)
        .unwrap()
        .bspline(&document)
        .unwrap()
}

fn point(control: [f64; 4]) -> [f64; 3] {
    [
        control[0] / control[3],
        control[1] / control[3],
        control[2] / control[3],
    ]
}

fn close(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|c| (a[c] - b[c]).abs() < 1e-9)
}

// A sweep elbow saved by AutoCAD: a `rot_spl_sur` with no fitted spline
// (approximation level 2), so only its profile, axis and ranges (#2271).
#[test]
fn a_revolution_saved_without_its_spline_is_rebuilt_from_its_profile() {
    let spline = surface(
        "spline-surface $-1 -1 $-1 forward { rot_spl_sur 20900 intcurve F { int_int_cur 21500 0 \
         nubs 1 0 2 0 1 1 1 10 0 0 10 0 5 } I I 0 0 0 0 0 1 2 F -1 F 0 F 0 F 1.5707963267948966 \
         2 0 0 0 0 0 0 0 0 0 I } I I F 0 F 1.5707963267948966 #",
    );
    // `u` runs against the profile, as the range says: -1..0 for 0..1.
    assert_eq!(spline.u_knots, vec![-1.0, -1.0, 0.0, 0.0]);
    assert_eq!(spline.degree_v, 2);
    assert_eq!(spline.control_count_u, 2);
    assert_eq!(spline.control_count_v, 3);
    let rows: Vec<[f64; 3]> = spline
        .control_points
        .iter()
        .step_by(spline.control_count_u)
        .map(|control| point(*control))
        .collect();
    // The profile's end (t = 1, so u = -1) turned a quarter turn about +z.
    assert!(close(rows[0], [10.0, 0.0, 5.0]));
    assert!(close(rows[1], [10.0, 10.0, 5.0]));
    assert!(close(rows[2], [0.0, 10.0, 5.0]));
    let middle = &spline.control_points[spline.control_count_u..2 * spline.control_count_u];
    assert!(middle
        .iter()
        .all(|control| (control[3] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12));

    // Halfway along the arc the surface is still 10 from the axis.
    let [p0, p1, p2] = [0, 1, 2].map(|row| spline.control_points[row * spline.control_count_u]);
    let [b0, b1, b2] = [0.25, 0.5, 0.25];
    let w = b0 * p0[3] + b1 * p1[3] + b2 * p2[3];
    let x = (b0 * p0[0] + b1 * p1[0] + b2 * p2[0]) / w;
    let y = (b0 * p0[1] + b1 * p1[1] + b2 * p2[1]) / w;
    assert!(((x * x + y * y).sqrt() - 10.0).abs() < 1e-9);
}

#[test]
fn a_revolution_past_a_quarter_turn_takes_more_arcs() {
    let spline = surface(
        "spline-surface $-1 -1 $-1 forward { rot_spl_sur 20900 intcurve F { int_int_cur 21500 0 \
         nubs 1 0 2 0 1 1 1 10 0 0 10 0 5 } I I 0 0 0 0 0 1 2 F 0 F 1 F 0 F 3.141592653589793 \
         2 0 0 0 0 0 0 0 0 0 I } I I F 0 F 3.141592653589793 #",
    );
    // A range along the profile keeps it as it is.
    assert_eq!(spline.u_knots, vec![0.0, 0.0, 1.0, 1.0]);
    assert_eq!(spline.control_count_v, 5);
    let half = std::f64::consts::FRAC_PI_2;
    let pi = std::f64::consts::PI;
    assert_eq!(spline.v_knots, vec![0.0, 0.0, 0.0, half, half, pi, pi, pi]);
    let last = point(spline.control_points[4 * spline.control_count_u]);
    assert!(close(last, [-10.0, 0.0, 0.0]));
}

// The straight leg of the same sweep: a spline profile plus a line, saved
// with its `u` range against the profile. Built along the profile, the
// face's pcurves fell off it and the leg was not drawn.
#[test]
fn a_linear_sum_follows_a_range_saved_against_its_profile() {
    let spline = surface(
        "spline-surface $-1 -1 $-1 forward { sum_spl_sur 20900 intcurve F { int_int_cur 21500 0 \
         nubs 1 0 2 0 1 1 1 0 0 0 10 0 0 } I I straight 0 0 0 0 0 1 I I 0 0 0 2 F -1 F 0 F 0 F 5 \
         2 0 0 0 0 0 0 0 0 0 I } I I F 0 F 5 #",
    );
    assert_eq!(spline.u_knots, vec![-1.0, -1.0, 0.0, 0.0]);
    assert_eq!(spline.v_knots, vec![0.0, 0.0, 5.0, 5.0]);
    assert!(close(point(spline.control_points[0]), [10.0, 0.0, 0.0]));
    assert!(close(point(spline.control_points[3]), [0.0, 0.0, 5.0]));
}
