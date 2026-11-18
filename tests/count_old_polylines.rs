use acadrust::entities::{LwPolyline, LwVertex, Polyline2D, Vertex2D};
use acadrust::types::{Handle, Vector2, Vector3};
use acadrust::{count, CadDocument, EntityType};

fn old(pts: &[(f64, f64)], closed: bool) -> EntityType {
    let mut pl = Polyline2D::new();
    for &(x, y) in pts {
        pl.add_vertex(Vertex2D::new(Vector3::new(x, y, 0.0)));
    }
    if closed {
        pl.close();
    }
    EntityType::Polyline2D(pl)
}

fn light(pts: &[(f64, f64)], closed: bool) -> EntityType {
    let mut pl = LwPolyline::new();
    for &(x, y) in pts {
        pl.vertices.push(LwVertex::new(Vector2::new(x, y)));
    }
    pl.is_closed = closed;
    EntityType::LwPolyline(pl)
}

fn matches(a: EntityType, b: EntityType) -> bool {
    let mut doc = CadDocument::new();
    let ha: Handle = doc.add_entity(a).unwrap();
    let hb: Handle = doc.add_entity(b).unwrap();
    count::group_matches(&doc, &[ha], None)
        .iter()
        .any(|g| g.contains(&hb))
}

const L: [(f64, f64); 3] = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
const SHIFTED: [(f64, f64); 3] = [(100.0, 50.0), (110.0, 50.0), (110.0, 60.0)];
const OTHER: [(f64, f64); 4] = [(0.0, 0.0), (3.0, 0.0), (3.0, 7.0), (-4.0, 9.0)];

#[test]
fn old_polylines_of_different_shape_are_not_similar() {
    assert!(!matches(old(&L, false), old(&OTHER, false)));
    assert!(!matches(old(&L, true), old(&OTHER, true)));
}

#[test]
fn old_polylines_of_same_shape_are_similar() {
    assert!(matches(old(&L, false), old(&SHIFTED, false)));
    assert!(matches(old(&L, true), old(&SHIFTED, true)));
}

#[test]
fn old_polylines_closed_and_open_are_not_similar() {
    assert!(!matches(old(&L, true), old(&L, false)));
}

#[test]
fn light_polylines_still_compare_by_shape() {
    assert!(matches(light(&L, false), light(&SHIFTED, false)));
    assert!(!matches(light(&L, false), light(&OTHER, false)));
    assert!(!matches(light(&L, true), light(&L, false)));
}

#[test]
fn light_and_old_polylines_are_not_similar() {
    assert!(!matches(light(&L, false), old(&L, false)));
}
