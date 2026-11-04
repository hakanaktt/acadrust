//! Formula (`AcExpr`) fields whose operands are child fields, and the object
//! properties those children read.

use std::f64::consts::{PI, TAU};

use acadrust::document::FieldDef;
use acadrust::entities::hatch::{
    BoundaryEdge, BoundaryPath, BoundaryPathFlags, CircularArcEdge, EllipticArcEdge, LineEdge,
    PolylineEdge,
};
use acadrust::entities::{AttributeEntity, Circle, Hatch, HatchStyleType, Insert, Line};
use acadrust::fields::{self, FieldContext};
use acadrust::types::{Handle, Vector2, Vector3};
use acadrust::{CadDocument, EntityType};

struct Context;

impl FieldContext for Context {
    fn now_julian(&self) -> f64 {
        2460000.0
    }
}

/// Register an `AcExpr` field with `children` as its `%<\_FldIdx N>%` operands,
/// the way AutoCAD stores a formula: each child is a FIELD owned by the formula.
fn formula(doc: &mut CadDocument, code: &str, children: &[(&str, &[Handle])]) -> Handle {
    let evaluator = |code: &str| {
        code.trim_start_matches('\\')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    };
    let parent = doc.allocate_handle();
    doc.fields.insert(
        parent,
        FieldDef {
            handle: parent,
            owner: Handle::NULL,
            evaluator: evaluator(code),
            code: code.into(),
            objects: Vec::new(),
        },
    );
    for (code, objects) in children {
        let handle = doc.allocate_handle();
        doc.fields.insert(
            handle,
            FieldDef {
                handle,
                owner: parent,
                evaluator: evaluator(code),
                code: (*code).into(),
                objects: objects.to_vec(),
            },
        );
    }
    parent
}

fn resolve(doc: &CadDocument, field: Handle) -> Option<String> {
    fields::resolve_handle(doc, field, Handle::NULL, &Context)
}

fn object_value(doc: &CadDocument, code: &str, object: Handle) -> Option<String> {
    fields::evaluate_code(doc, code, &[object], None, &Context)
}

fn circle(doc: &mut CadDocument, radius: f64) -> Handle {
    doc.add_entity(EntityType::Circle(Circle::from_center_radius(
        Vector3::ZERO,
        radius,
    )))
    .unwrap()
}

#[test]
fn formula_combines_nested_object_fields() {
    let mut doc = CadDocument::new();
    let small = circle(&mut doc, 1.0);
    let large = circle(&mut doc, 2.0);
    // Each operand displays rounded to a whole number; the formula uses the
    // full-precision area and its own format, not the operands'.
    let area = "\\AcObjProp Object(%<\\_ObjIdx 0>%).Area \\f \"%lu2%pr0\"";
    let children: &[(&str, &[Handle])] = &[(area, &[small]), (area, &[large])];
    let sum = formula(
        &mut doc,
        "\\AcExpr (%<\\_FldIdx 0>%+%<\\_FldIdx 1>%) \\f \"%lu2%pr2\"",
        children,
    );
    let difference = formula(
        &mut doc,
        "\\AcExpr (%<\\_FldIdx 1>%-%<\\_FldIdx 0>%) \\f \"%lu2%pr2\"",
        children,
    );

    assert_eq!(
        resolve(&doc, sum).as_deref(),
        Some(format!("{:.2}", 5.0 * PI).as_str())
    );
    assert_eq!(
        resolve(&doc, difference).as_deref(),
        Some(format!("{:.2}", 3.0 * PI).as_str())
    );

    // The Field dialog previews a formula with its operands written inline;
    // the operand's `\f` belongs to it, the formula's own comes last.
    let inline = format!(
        "\\AcExpr (%<\\AcObjProp Object(%<\\_ObjId {}>%).Area \\f \"%lu2%pr0\">%*2) \\f \"%lu2%pr2\"",
        u64::from(large)
    );
    assert_eq!(
        fields::evaluate_code(&doc, &inline, &[], None, &Context).as_deref(),
        Some(format!("{:.2}", 8.0 * PI).as_str())
    );
}

#[test]
fn formula_reads_the_number_a_nested_field_shows() {
    let mut doc = CadDocument::new();
    for (text, expected) in [
        ("+1.25", "1.25"),
        ("-3", "-3.00"),
        ("1,234.50", "1234.50"),
        ("1.234,50", "1234.50"),
        ("12,5", "12.50"),
        ("Kot: 7.5 m", "7.50"),
    ] {
        let diesel = format!("\\AcDiesel {text}");
        let field = formula(
            &mut doc,
            "\\AcExpr (%<\\_FldIdx 0>%) \\f \"%lu2%pr2\"",
            &[(diesel.as_str(), &[])],
        );
        assert_eq!(resolve(&doc, field).as_deref(), Some(expected), "{text}");
    }
}

#[test]
fn versioned_object_property_points_are_in_the_current_ucs() {
    let mut doc = CadDocument::new();
    let line = doc
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::ZERO,
            Vector3::new(10.0, 20.0, 0.0),
        )))
        .unwrap();
    // A UCS 5 units up and turned a quarter turn counter-clockwise.
    doc.header.model_space_ucs_origin = Vector3::new(0.0, 5.0, 0.0);
    doc.header.model_space_ucs_x_axis = Vector3::new(0.0, 1.0, 0.0);
    doc.header.model_space_ucs_y_axis = Vector3::new(-1.0, 0.0, 0.0);
    let end = |evaluator: &str, coordinate: u8| {
        let code = format!(
            "\\{evaluator} Object(%<\\_ObjIdx 0>%).EndPoint \\f \"%lu2%pt{coordinate}%pr2\""
        );
        object_value(&doc, &code, line)
    };

    // The Properties-palette evaluator reports the UCS location ...
    assert_eq!(end("AcObjProp.16.2", 1).as_deref(), Some("15.00"));
    assert_eq!(end("AcObjProp.16.2", 2).as_deref(), Some("-10.00"));
    // ... and the plain one the WCS location.
    assert_eq!(end("AcObjProp", 1).as_deref(), Some("10.00"));
    assert_eq!(end("AcObjProp", 2).as_deref(), Some("20.00"));

    // A level mark: the end point's height in the UCS, plus a fixed offset.
    let child = "\\AcObjProp.16.2 Object(%<\\_ObjIdx 0>%).EndPoint \\f \"%lu2%pt2%pr3\"";
    let field = formula(
        &mut doc,
        "\\AcExpr (%<\\_FldIdx 0>%+0.5) \\f \"%lu2%pr2\"",
        &[(child, &[line])],
    );
    assert_eq!(resolve(&doc, field).as_deref(), Some("-9.50"));
}

#[test]
fn attribute_text_string_feeds_a_formula() {
    let mut doc = CadDocument::new();
    let mut attribute = AttributeEntity::new("KOT".into(), "+1.25".into());
    attribute.common.handle = doc.allocate_handle();
    let attribute_handle = attribute.common.handle;
    let mut insert = Insert::new("LEVEL", Vector3::ZERO);
    insert.attributes.push(attribute);
    doc.add_entity(EntityType::Insert(insert)).unwrap();

    let text = "\\AcObjProp Object(%<\\_ObjIdx 0>%).TextString";
    assert_eq!(
        object_value(&doc, text, attribute_handle).as_deref(),
        Some("+1.25")
    );
    let field = formula(
        &mut doc,
        "\\AcExpr (%<\\_FldIdx 0>%*2) \\f \"%lu2%pr2\"",
        &[(text, &[attribute_handle])],
    );
    assert_eq!(resolve(&doc, field).as_deref(), Some("2.50"));
}

fn line_loop(points: &[(f64, f64)], flags: BoundaryPathFlags) -> BoundaryPath {
    let mut path = BoundaryPath::with_flags(flags);
    for (i, &(x, y)) in points.iter().enumerate() {
        let (nx, ny) = points[(i + 1) % points.len()];
        path.add_edge(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(x, y),
            end: Vector2::new(nx, ny),
        }));
    }
    path
}

fn square(half: f64, flags: BoundaryPathFlags) -> BoundaryPath {
    line_loop(
        &[(-half, -half), (half, -half), (half, half), (-half, half)],
        flags,
    )
}

fn hatch_area(doc: &mut CadDocument, paths: Vec<BoundaryPath>, style: HatchStyleType) -> String {
    let mut hatch = Hatch::solid();
    hatch.paths = paths;
    hatch.style = style;
    let handle = doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();
    let code = "\\AcObjProp Object(%<\\_ObjIdx 0>%).Area \\f \"%lu2%pr4\"";
    object_value(doc, code, handle).expect("hatch area")
}

#[test]
fn hatch_area_integrates_curved_edges() {
    let mut doc = CadDocument::new();

    // A full circle as one counter-clockwise arc edge.
    let mut disc = BoundaryPath::with_flags(BoundaryPathFlags::EXTERNAL);
    disc.add_edge(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(5.0, 5.0),
        radius: 2.0,
        start_angle: 0.0,
        end_angle: TAU,
        counter_clockwise: true,
    }));
    assert_eq!(
        hatch_area(&mut doc, vec![disc], HatchStyleType::Normal),
        format!("{:.4}", 4.0 * PI)
    );

    // A full ellipse, 3 by 1.5 semi-axes, with a tilted major axis.
    let mut ellipse = BoundaryPath::with_flags(BoundaryPathFlags::EXTERNAL);
    ellipse.add_edge(BoundaryEdge::EllipticArc(EllipticArcEdge {
        center: Vector2::new(1.0, 2.0),
        major_axis_endpoint: Vector2::new(1.8, 2.4),
        minor_axis_ratio: 0.5,
        start_angle: 0.0,
        end_angle: TAU,
        counter_clockwise: true,
    }));
    assert_eq!(
        hatch_area(&mut doc, vec![ellipse], HatchStyleType::Normal),
        format!("{:.4}", 4.5 * PI)
    );

    // A half disc: a diameter, then a clockwise arc back through (0, -1).
    let mut half = BoundaryPath::with_flags(BoundaryPathFlags::EXTERNAL);
    half.add_edge(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(-1.0, 0.0),
        end: Vector2::new(1.0, 0.0),
    }));
    half.add_edge(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::ZERO,
        radius: 1.0,
        start_angle: 0.0,
        end_angle: PI,
        counter_clockwise: false,
    }));
    assert_eq!(
        hatch_area(&mut doc, vec![half], HatchStyleType::Normal),
        format!("{:.4}", PI / 2.0)
    );

    // A 4 x 2 rectangle whose right side is a half circle bulging outward.
    let mut pline = PolylineEdge::new(Vec::new(), true);
    pline.add_vertex(Vector2::new(0.0, 0.0), 0.0);
    pline.add_vertex(Vector2::new(4.0, 0.0), 1.0);
    pline.add_vertex(Vector2::new(4.0, 2.0), 0.0);
    pline.add_vertex(Vector2::new(0.0, 2.0), 0.0);
    let mut bulged = BoundaryPath::with_flags(BoundaryPathFlags::EXTERNAL);
    bulged.add_edge(BoundaryEdge::Polyline(pline));
    assert_eq!(
        hatch_area(&mut doc, vec![bulged], HatchStyleType::Normal),
        format!("{:.4}", 8.0 + PI / 2.0)
    );
}

#[test]
fn hatch_area_leaves_out_islands_by_style() {
    let mut doc = CadDocument::new();
    let nested = || {
        vec![
            square(5.0, BoundaryPathFlags::EXTERNAL),
            square(3.0, BoundaryPathFlags::DEFAULT),
            square(1.0, BoundaryPathFlags::DEFAULT),
        ]
    };
    // 10 x 10, minus the 6 x 6 island, plus the 2 x 2 island inside it.
    assert_eq!(
        hatch_area(&mut doc, nested(), HatchStyleType::Normal),
        "68.0000"
    );
    assert_eq!(
        hatch_area(&mut doc, nested(), HatchStyleType::Outer),
        "64.0000"
    );
    assert_eq!(
        hatch_area(&mut doc, nested(), HatchStyleType::Ignore),
        "100.0000"
    );

    // Neighbouring outer loops that share an edge, drawn 3 mm apart, are not
    // islands of each other; only the small loop inside the first one is.
    let room = line_loop(
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)],
        BoundaryPathFlags::EXTERNAL,
    );
    let niche = line_loop(
        &[(0.0, 0.0), (1.2, 0.003), (1.2, -1.0), (0.0, -1.0)],
        BoundaryPathFlags::EXTERNAL,
    );
    let column = line_loop(
        &[(2.0, 1.0), (2.5, 1.0), (2.5, 1.5), (2.0, 1.5)],
        BoundaryPathFlags::OUTERMOST,
    );
    assert_eq!(
        hatch_area(&mut doc, vec![room, niche, column], HatchStyleType::Normal),
        format!("{:.4}", 12.0 + 1.2 * (1.0 + 1.003) / 2.0 - 0.25)
    );

    // A circular island, drawn clockwise as AutoCAD often stores islands.
    let mut hole = BoundaryPath::with_flags(BoundaryPathFlags::DEFAULT);
    hole.add_edge(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(1.0, 1.0),
        radius: 2.0,
        start_angle: TAU,
        end_angle: 0.0,
        counter_clockwise: false,
    }));
    assert_eq!(
        hatch_area(
            &mut doc,
            vec![square(5.0, BoundaryPathFlags::EXTERNAL), hole],
            HatchStyleType::Normal
        ),
        format!("{:.4}", 100.0 - 4.0 * PI)
    );
}
