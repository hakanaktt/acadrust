// EXPLODE of a REGION that carries no wireframe takes its curves from the
// ACIS body: every edge becomes a LINE, CIRCLE, ELLIPSE or SPLINE in world
// coordinates. Three things decide where those curves land:
// - a DWG 2013 or later stores the body as binary ACIS (SAB), not SAT text;
// - the body's `transform` places it: world = scale·(p·M) + T;
// - an `intcurve` saved `reversed` runs against its B-spline, so a curve
//   parameter t is the spline parameter −t.

use acadrust::entities::acis::{SabWriter, SatDocument};
use acadrust::entities::{AcisData, EntityType, Region};
use acadrust::types::Vector3;

/// One edge of the loop: its start point (the end is the next edge's start),
/// its parameter range on the curve and the curve record without its index.
struct EdgeSpec {
    start: [f64; 2],
    params: [f64; 2],
    curve: String,
}

/// A planar REGION body as ACIS 7.0 SAT text: one face, one loop of `edges`
/// (a ring), the body moved by `shift`. Records: body, lump, transform, shell,
/// face, loop, plane-surface, then coedge, edge, vertex, point and curve for
/// each edge.
fn region_sat(shift: [f64; 2], edges: &[EdgeSpec]) -> String {
    let n = edges.len();
    let at = |i: usize, k: usize| 7 + 5 * (i % n) + k;
    let mut records = vec![
        "body $-1 -1 $-1 $1 $-1 $2".to_string(),
        "lump $-1 -1 $-1 $-1 $3 $0".to_string(),
        format!(
            "transform $-1 -1 1 0 0 0 1 0 0 0 1 {} {} 0 1 no_rotate no_reflect no_shear",
            shift[0], shift[1]
        ),
        "shell $-1 -1 $-1 $-1 $-1 $4 $-1 $1".to_string(),
        "face $-1 -1 $-1 $-1 $5 $3 $-1 $6 forward single out".to_string(),
        format!("loop $-1 -1 $-1 $-1 ${} $4", at(0, 0)),
        "plane-surface $-1 -1 $-1 0 0 0 0 0 1 1 0 0 forward_v I I I I".to_string(),
    ];
    for (i, e) in edges.iter().enumerate() {
        records.push(format!(
            "coedge $-1 -1 $-1 ${} ${} $-1 ${} forward $5 $-1",
            at(i + 1, 0),
            at(i + n - 1, 0),
            at(i, 1)
        ));
        records.push(format!(
            "edge $-1 -1 $-1 ${} {} ${} {} ${} ${} forward @7 unknown",
            at(i, 2),
            e.params[0],
            at(i + 1, 2),
            e.params[1],
            at(i, 0),
            at(i, 4)
        ));
        records.push(format!("vertex $-1 -1 $-1 ${} ${}", at(i, 1), at(i, 3)));
        records.push(format!("point $-1 -1 $-1 {} {} 0", e.start[0], e.start[1]));
        records.push(e.curve.clone());
    }
    let mut text = String::from(
        "700 0 1 0\n@8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2026\n1e-06 9.9999999999999995e-07\n",
    );
    for (i, record) in records.iter().enumerate() {
        text.push_str(&format!("-{i} {record} #\n"));
    }
    text.push_str("End-of-ACIS-data\n");
    text
}

fn straight_edge(from: [f64; 2], to: [f64; 2]) -> EdgeSpec {
    let d = [to[0] - from[0], to[1] - from[1]];
    let length = d[0].hypot(d[1]);
    EdgeSpec {
        start: from,
        params: [0.0, length],
        curve: format!(
            "straight-curve $-1 -1 $-1 {} {} 0 {} {} 0 I I",
            from[0],
            from[1],
            d[0] / length,
            d[1] / length
        ),
    }
}

/// A 10 x 10 square from (0, 0), body moved by `shift`.
fn square_sat(shift: [f64; 2]) -> String {
    let square = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    let edges: Vec<EdgeSpec> = (0..4)
        .map(|i| straight_edge(square[i], square[(i + 1) % 4]))
        .collect();
    region_sat(shift, &edges)
}

fn line_ends(parts: &[EntityType]) -> Vec<(Vector3, Vector3)> {
    parts
        .iter()
        .filter_map(|part| match part {
            EntityType::Line(line) => Some((line.start, line.end)),
            _ => None,
        })
        .collect()
}

fn has_segment(lines: &[(Vector3, Vector3)], a: [f64; 2], b: [f64; 2]) -> bool {
    let a = Vector3::new(a[0], a[1], 0.0);
    let b = Vector3::new(b[0], b[1], 0.0);
    lines.iter().any(|(s, e)| {
        (s.distance(&a) < 1e-9 && e.distance(&b) < 1e-9)
            || (s.distance(&b) < 1e-9 && e.distance(&a) < 1e-9)
    })
}

#[test]
fn binary_region_explodes_into_its_edges() {
    // The same square body saved as SAB, the way a DWG 2013+ holds it.
    let doc = SatDocument::parse(&square_sat([0.0, 0.0])).expect("SAT");
    let mut region = Region::new();
    region.acis_data = AcisData::from_sab(SabWriter::write(&doc));
    let lines = line_ends(&EntityType::Region(region).explode());
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(has_segment(&lines, [0.0, 0.0], [10.0, 0.0]), "{lines:?}");
    assert!(has_segment(&lines, [10.0, 10.0], [0.0, 10.0]), "{lines:?}");
}

#[test]
fn region_explode_applies_the_body_transform() {
    // The body sits at (100, 200): its transform moves every edge there.
    let region = Region::from_sat(&square_sat([100.0, 200.0]));
    let lines = line_ends(&EntityType::Region(region).explode());
    assert_eq!(lines.len(), 4, "{lines:?}");
    for (a, b) in [
        ([100.0, 200.0], [110.0, 200.0]),
        ([110.0, 200.0], [110.0, 210.0]),
        ([110.0, 210.0], [100.0, 210.0]),
        ([100.0, 210.0], [100.0, 200.0]),
    ] {
        assert!(has_segment(&lines, a, b), "{a:?}-{b:?}: {lines:?}");
    }
}

#[test]
fn region_explode_places_arc_edges_by_the_body_transform() {
    // A half disc of radius 5 centred on the body origin, body at (100, 200):
    // the arc's centre lands on (100, 200).
    let arc = EdgeSpec {
        start: [5.0, 0.0],
        params: [0.0, std::f64::consts::PI],
        curve: "ellipse-curve $-1 -1 $-1 0 0 0 0 0 1 5 0 0 1 I I".to_string(),
    };
    let edges = [arc, straight_edge([-5.0, 0.0], [5.0, 0.0])];
    let region = Region::from_sat(&region_sat([100.0, 200.0], &edges));
    let parts = EntityType::Region(region).explode();
    let ellipse = parts
        .iter()
        .find_map(|part| match part {
            EntityType::Ellipse(ellipse) => Some(ellipse),
            _ => None,
        })
        .expect("arc edge");
    assert!(
        ellipse.center.distance(&Vector3::new(100.0, 200.0, 0.0)) < 1e-9,
        "{:?}",
        ellipse.center
    );
    assert!((ellipse.major_axis.length() - 5.0).abs() < 1e-9);
}

#[test]
fn reversed_intcurve_edge_explodes_to_its_own_span() {
    // The spline runs (0, 0) -> (10, 0) over [0, 10]; the intcurve is saved
    // `reversed`, so the edge's curve range [-5, -2] is the spline span
    // [2, 5], from (5, 0) to (2, 0).
    let spline = EdgeSpec {
        start: [5.0, 0.0],
        params: [-5.0, -2.0],
        curve: "intcurve-curve $-1 -1 $-1 reversed { exactcur nubs 1 open 2 0 1 10 1 0 0 0 10 0 0 0 } I I"
            .to_string(),
    };
    let edges = [spline, straight_edge([2.0, 0.0], [5.0, 0.0])];
    let region = Region::from_sat(&region_sat([0.0, 0.0], &edges));
    let parts = EntityType::Region(region).explode();
    let spline = parts
        .iter()
        .find_map(|part| match part {
            EntityType::Spline(spline) => Some(spline),
            _ => None,
        })
        .expect("spline edge");
    let points = if spline.fit_points.is_empty() {
        &spline.control_points
    } else {
        &spline.fit_points
    };
    assert!(
        points
            .iter()
            .all(|p| p.x > 2.0 - 1e-9 && p.x < 5.0 + 1e-9 && p.y.abs() < 1e-9),
        "span [2, 5]: {points:?}"
    );
    let ends = [points[0], points[points.len() - 1]];
    assert!(ends
        .iter()
        .any(|p| p.distance(&Vector3::new(2.0, 0.0, 0.0)) < 1e-9));
    assert!(ends
        .iter()
        .any(|p| p.distance(&Vector3::new(5.0, 0.0, 0.0)) < 1e-9));
}

#[test]
fn rational_spline_edge_keeps_cartesian_control_points() {
    // A quarter circle of radius 5 as a rational quadratic intcurve over its
    // whole domain: control points (5, 0), (5, 5) with weight √2/2, (0, 5).
    // The SPLINE gets the points themselves, not the weighted (x·w, y·w).
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let arc = EdgeSpec {
        start: [5.0, 0.0],
        params: [0.0, 1.0],
        curve: format!(
            "intcurve-curve $-1 -1 $-1 forward {{ exactcur nurbs 2 open 2 0 2 1 2 5 0 0 1 5 5 0 {w} 0 5 0 1 }} I I"
        ),
    };
    let edges = [
        arc,
        straight_edge([0.0, 5.0], [0.0, 0.0]),
        straight_edge([0.0, 0.0], [5.0, 0.0]),
    ];
    let region = Region::from_sat(&region_sat([0.0, 0.0], &edges));
    let parts = EntityType::Region(region).explode();
    let spline = parts
        .iter()
        .find_map(|part| match part {
            EntityType::Spline(spline) => Some(spline),
            _ => None,
        })
        .expect("spline edge");
    assert_eq!(spline.control_points.len(), 3, "{spline:?}");
    assert!(
        spline.control_points[1].distance(&Vector3::new(5.0, 5.0, 0.0)) < 1e-9,
        "{:?}",
        spline.control_points
    );
    assert!(
        (spline.weights[1] - w).abs() < 1e-12,
        "{:?}",
        spline.weights
    );
}
