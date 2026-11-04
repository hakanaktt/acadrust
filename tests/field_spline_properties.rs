//! Object property fields read from a spline: its Area and Length.

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use acadrust::entities::Spline;
use acadrust::fields::{self, FieldContext};
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, EntityType};

struct Context;

impl FieldContext for Context {
    fn now_julian(&self) -> f64 {
        2460000.0
    }
}

fn property(doc: &CadDocument, spline: Handle, name: &str) -> Option<String> {
    let code = format!("\\AcObjProp Object(%<\\_ObjIdx 0>%).{name} \\f \"%lu2%pr4\"");
    fields::evaluate_code(doc, &code, &[spline], None, &Context)
}

#[test]
fn a_rational_spline_circle_measures_as_the_circle() {
    // A circle of radius 2 around (1, 1) as a closed quadratic NURBS: eight
    // arcs of 45°, the corner points weighted 1/√2.
    let mut circle = Spline::new();
    circle.degree = 2;
    circle.control_points = [
        (3.0, 1.0),
        (3.0, 3.0),
        (1.0, 3.0),
        (-1.0, 3.0),
        (-1.0, 1.0),
        (-1.0, -1.0),
        (1.0, -1.0),
        (3.0, -1.0),
        (3.0, 1.0),
    ]
    .iter()
    .map(|&(x, y)| Vector3::new(x, y, 0.0))
    .collect();
    circle.weights = (0..9)
        .map(|i| if i % 2 == 1 { FRAC_1_SQRT_2 } else { 1.0 })
        .collect();
    circle.knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 4.0];
    circle.flags.closed = true;
    circle.flags.rational = true;
    let mut doc = CadDocument::new();
    let handle = doc.add_entity(EntityType::Spline(circle)).unwrap();

    assert_eq!(
        property(&doc, handle, "Area").as_deref(),
        Some(format!("{:.4}", 4.0 * PI).as_str())
    );
    assert_eq!(
        property(&doc, handle, "Length").as_deref(),
        Some(format!("{:.4}", 4.0 * PI).as_str())
    );
}

#[test]
fn an_open_spline_measures_its_arc_and_the_area_its_chord_closes() {
    // y = 2x − x² from (0, 0) to (2, 0) as a quadratic Bézier.
    let arch = Spline::from_control_points(
        2,
        vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 2.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
        ],
    );
    let mut doc = CadDocument::new();
    let handle = doc.add_entity(EntityType::Spline(arch)).unwrap();

    // ∫₀² (2x − x²) dx = 4/3; ∫₀² √(1 + (2 − 2x)²) dx = √5 + asinh(2) / 2.
    assert_eq!(property(&doc, handle, "Area").as_deref(), Some("1.3333"));
    let length = 5f64.sqrt() + 2f64.asinh() / 2.0;
    assert_eq!(
        property(&doc, handle, "Length").as_deref(),
        Some(format!("{length:.4}").as_str())
    );
}

#[test]
fn a_spline_known_only_by_fit_points_is_not_measured() {
    // Without control points the curve through the fit points is unknown;
    // measuring the fit-point polygon would show a wrong figure.
    let mut fitted = Spline::new();
    fitted.fit_points = vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 1.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0),
    ];
    let mut doc = CadDocument::new();
    let handle = doc.add_entity(EntityType::Spline(fitted)).unwrap();

    assert_eq!(property(&doc, handle, "Area"), None);
    assert_eq!(property(&doc, handle, "Length"), None);
}
