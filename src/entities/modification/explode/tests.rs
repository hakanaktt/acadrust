use super::*;
use crate::types::Vector2;

#[test]
fn test_atomic_entities_return_empty() {
    let line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 1.0, 1.0, 0.0));
    assert!(line.flatten().is_empty());

    let arc = EntityType::Arc(Arc::new());
    assert!(arc.flatten().is_empty());

    let point = EntityType::Point(Point::new());
    assert!(point.flatten().is_empty());

    let text = EntityType::Text(Text::new());
    assert!(text.flatten().is_empty());
}

#[test]
fn test_native_standalone_refusal_rules() {
    assert!(EntityType::Circle(Circle::new()).explode().is_empty());
    assert!(EntityType::Ellipse(Ellipse::new()).explode().is_empty());
    assert!(EntityType::Spline(Spline::new()).explode().is_empty());
    assert!(EntityType::Solid(Solid::new(
        Vector3::ZERO,
        Vector3::UNIT_X,
        Vector3::UNIT_Y,
        Vector3::UNIT_Y,
    ))
    .explode()
    .is_empty());
    assert!(EntityType::Face3D(Face3D::triangle(
        Vector3::ZERO,
        Vector3::UNIT_X,
        Vector3::UNIT_Y,
    ))
    .explode()
    .is_empty());
    let parts = EntityType::Helix(Box::new(Helix::new())).explode();
    assert_eq!(parts.len(), 1);
    assert!(matches!(parts[0], EntityType::Spline(_)));
}

#[test]
fn test_explode_circle() {
    let circle = Circle::from_center_radius(Vector3::new(5.0, 5.0, 0.0), 10.0);
    let entity = EntityType::Circle(circle);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        EntityType::Arc(arc) => {
            assert_eq!(arc.center, Vector3::new(5.0, 5.0, 0.0));
            assert!((arc.radius - 10.0).abs() < 1e-10);
            assert!((arc.start_angle - 0.0).abs() < 1e-10);
            assert!((arc.end_angle - 2.0 * PI).abs() < 1e-10);
        }
        _ => panic!("Expected Arc from circle explosion"),
    }
}

#[test]
fn test_explode_lwpolyline_lines_only() {
    let mut pl = LwPolyline::new();
    pl.add_point(Vector2::new(0.0, 0.0));
    pl.add_point(Vector2::new(10.0, 0.0));
    pl.add_point(Vector2::new(10.0, 10.0));

    let entity = EntityType::LwPolyline(pl);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], EntityType::Line(_)));
    assert!(matches!(&parts[1], EntityType::Line(_)));
}

#[test]
fn test_explode_lwpolyline_closed() {
    let mut pl = LwPolyline::new();
    pl.add_point(Vector2::new(0.0, 0.0));
    pl.add_point(Vector2::new(10.0, 0.0));
    pl.add_point(Vector2::new(10.0, 10.0));
    pl.close();

    let entity = EntityType::LwPolyline(pl);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 3); // 3 segments for closed triangle
}

#[test]
fn test_explode_lwpolyline_with_bulge() {
    let mut pl = LwPolyline::new();
    pl.add_point_with_bulge(Vector2::new(0.0, 0.0), 1.0); // semicircle
    pl.add_point(Vector2::new(10.0, 0.0));

    let entity = EntityType::LwPolyline(pl);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 1);
    assert!(matches!(&parts[0], EntityType::Arc(_)));
}

#[test]
fn test_explode_planar_lines_convert_ocs_to_wcs() {
    let normal = Vector3::UNIT_Y;
    let to_wcs = Matrix3::arbitrary_axis(normal);
    let start = Vector3::new(1.0, 2.0, 3.0);
    let end = Vector3::new(4.0, 5.0, 3.0);

    let mut lightweight = LwPolyline::new();
    lightweight.normal = normal;
    lightweight.elevation = 3.0;
    lightweight.add_point(Vector2::new(start.x, start.y));
    lightweight.add_point(Vector2::new(end.x, end.y));
    let mut heavy = Polyline2D::new();
    heavy.normal = normal;
    heavy.add_vertex(Vertex2D::new(start));
    heavy.add_vertex(Vertex2D::new(end));
    let solid = Solid::triangle(start, end, Vector3::new(7.0, 8.0, 3.0)).with_normal(normal);
    for entity in [
        EntityType::LwPolyline(lightweight),
        EntityType::Polyline2D(heavy),
        EntityType::Solid(solid),
    ] {
        let parts = entity.flatten();
        let EntityType::Line(line) = &parts[0] else {
            panic!("expected a WCS line");
        };
        assert_eq!(line.start, to_wcs * start);
        assert_eq!(line.end, to_wcs * end);
    }
}

#[test]
fn test_explode_polyline2d_sits_at_its_elevation() {
    let normal = Vector3::UNIT_X;
    let mut heavy = Polyline2D::new();
    heavy.normal = normal;
    heavy.elevation = 2.0;
    heavy.thickness = -4.0;
    heavy.add_vertex(Vertex2D::new(Vector3::new(0.0, 0.0, 0.0)));
    heavy.add_vertex(Vertex2D::new(Vector3::new(10.0, 0.0, 0.0)));
    let parts = EntityType::Polyline2D(heavy).explode();
    let [EntityType::Line(line)] = &parts[..] else {
        panic!("expected one line, got {parts:?}");
    };
    let to_wcs = Matrix3::arbitrary_axis(normal);
    assert_eq!(line.start, to_wcs * Vector3::new(0.0, 0.0, 2.0));
    assert_eq!(line.end, to_wcs * Vector3::new(10.0, 0.0, 2.0));
    assert_eq!(line.thickness, -4.0);
}

#[test]
fn test_explode_hatch_clockwise_elliptic_edge_uses_geometric_parameters() {
    let mut path = BoundaryPath::new();
    path.edges.push(BoundaryEdge::EllipticArc(
        crate::entities::hatch::EllipticArcEdge {
            center: Vector2::ZERO,
            major_axis_endpoint: Vector2::new(4.0, 0.0),
            minor_axis_ratio: 0.5,
            start_angle: 0.3,
            end_angle: 1.2,
            counter_clockwise: false,
        },
    ));
    let mut hatch = Hatch::new();
    hatch.paths.push(path);
    let parts = EntityType::Hatch(Box::new(hatch)).flatten();
    let EntityType::Ellipse(ellipse) = &parts[0] else {
        panic!("expected ellipse");
    };
    let point = |parameter: f64| {
        ellipse.center
            + ellipse.major_axis * parameter.cos()
            + ellipse.normal.cross(&ellipse.major_axis)
                * (ellipse.minor_axis_ratio * parameter.sin())
    };
    let expected_start = Vector3::new(4.0 * 1.2f64.cos(), -2.0 * 1.2f64.sin(), 0.0);
    let expected_end = Vector3::new(4.0 * 0.3f64.cos(), -2.0 * 0.3f64.sin(), 0.0);
    assert!(point(ellipse.start_parameter).distance(&expected_start) < 1e-12);
    assert!(point(ellipse.end_parameter).distance(&expected_end) < 1e-12);
    assert!((ellipse.end_parameter - ellipse.start_parameter - 0.9).abs() < 1e-12);
}

#[test]
fn test_explode_solid_quad() {
    let first = Vector3::new(0.0, 0.0, 0.0);
    let second = Vector3::new(10.0, 0.0, 0.0);
    let third = Vector3::new(0.0, 10.0, 0.0);
    let fourth = Vector3::new(10.0, 10.0, 0.0);
    let solid = Solid::new(first, second, third, fourth);
    let entity = EntityType::Solid(solid);
    let parts = entity.flatten();
    let expected = [
        (first, second),
        (second, fourth),
        (fourth, third),
        (third, first),
    ];
    assert_eq!(parts.len(), expected.len());
    for (part, (start, end)) in parts.iter().zip(expected) {
        let EntityType::Line(line) = part else {
            panic!("expected line")
        };
        assert_eq!((line.start, line.end), (start, end));
    }
}

#[test]
fn test_explode_solid_triangle() {
    let solid = Solid::triangle(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(5.0, 10.0, 0.0),
    );
    let entity = EntityType::Solid(solid);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 3); // 3 edge lines
}

#[test]
fn test_explode_face3d() {
    let face = Face3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
        Vector3::new(0.0, 10.0, 0.0),
    );
    let entity = EntityType::Face3D(face);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 4); // All edges visible
}

#[test]
fn test_explode_face3d_invisible_edges() {
    let mut face = Face3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
        Vector3::new(0.0, 10.0, 0.0),
    );
    face.invisible_edges.set_first_invisible(true);
    face.invisible_edges.set_third_invisible(true);

    let entity = EntityType::Face3D(face);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 2); // Only 2 visible edges
}

#[test]
fn test_explode_polyline3d() {
    let pl = Polyline::from_points(vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
    ]);
    let entity = EntityType::Polyline(pl);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], EntityType::Line(_)));
}

#[test]
fn test_explode_leader() {
    let mut leader = Leader::new();
    leader.vertices.push(Vector3::new(0.0, 0.0, 0.0));
    leader.vertices.push(Vector3::new(10.0, 10.0, 0.0));
    leader.vertices.push(Vector3::new(20.0, 10.0, 0.0));

    let entity = EntityType::Leader(leader);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 2);
    assert!(matches!(&parts[0], EntityType::Line(_)));
}

#[test]
fn test_explode_standard_mline_emits_style_elements() {
    let mline = MLine::from_points(&[Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0)]);
    let parts = EntityType::MLine(mline).explode();
    assert_eq!(parts.len(), 2);
    let ys: Vec<f64> = parts
        .iter()
        .map(|part| match part {
            EntityType::Line(line) => line.start.y,
            _ => panic!("expected line"),
        })
        .collect();
    assert!(ys.contains(&0.5) && ys.contains(&-0.5));
}

/// A callout the way ACAD stores it: the leader line holds only the
/// arrow tip, the root holds the bend (connection point) and the landing.
fn callout(tips: &[Vector3]) -> MultiLeader {
    let mut ml = MultiLeader::new();
    ml.context.arrowhead_size = 1.0;
    let mut root = LeaderRoot::new(0);
    root.connection_point = Vector3::new(10.0, 10.0, 0.0);
    root.direction = Vector3::new(2.0, 0.0, 0.0);
    root.landing_distance = 5.0;

    for (index, tip) in tips.iter().enumerate() {
        let mut line = LeaderLine::new(index as i32);
        line.points.push(*tip);
        root.lines.push(line);
    }

    ml.context.leader_roots.push(root);
    ml
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

fn solids(parts: &[EntityType]) -> Vec<&Solid> {
    parts
        .iter()
        .filter_map(|part| match part {
            EntityType::Solid(solid) => Some(solid),
            _ => None,
        })
        .collect()
}

#[test]
fn test_explode_multileader_single_vertex_line() {
    let parts = EntityType::MultiLeader(Box::new(callout(&[Vector3::ZERO]))).flatten();
    let arrows = solids(&parts);
    let lines = line_ends(&parts);

    // The arrowhead: tip at the first point, one unit long, 1/3 wide.
    assert_eq!(arrows.len(), 1);
    let base = Vector3::new(1.0, 1.0, 0.0) * (1.0 / 2f64.sqrt());
    assert_eq!(arrows[0].first_corner, Vector3::ZERO);
    let half_width = arrows[0].second_corner.distance(&arrows[0].third_corner) / 2.0;
    assert!((half_width - 1.0 / 6.0).abs() < 1e-9);
    assert!(((arrows[0].second_corner + arrows[0].third_corner) * 0.5).distance(&base) < 1e-9);

    // The shaft starts at the arrow base and reaches the connection
    // point; the landing follows the normalized direction.
    assert_eq!(lines.len(), 2);
    assert!(lines[0].0.distance(&base) < 1e-9);
    assert_eq!(lines[0].1, Vector3::new(10.0, 10.0, 0.0));
    assert_eq!(
        lines[1],
        (Vector3::new(10.0, 10.0, 0.0), Vector3::new(15.0, 10.0, 0.0))
    );
}

#[test]
fn test_native_multileader_text_uses_polyline_arrow_and_shaft() {
    let mut ml = callout(&[Vector3::ZERO]);
    ml.context.has_text_contents = true;
    ml.context.text_string = "NOTE".to_string();
    let parts = EntityType::MultiLeader(Box::new(ml)).explode();
    assert_eq!(parts.len(), 3);
    assert!(parts
        .iter()
        .any(|part| matches!(part, EntityType::LwPolyline(_))));
    assert!(parts.iter().any(|part| matches!(part, EntityType::Line(_))));
    assert!(parts
        .iter()
        .any(|part| matches!(part, EntityType::MText(_))));
}

#[test]
fn test_explode_multileader_does_not_repeat_the_connection_point() {
    let mut ml = callout(&[Vector3::ZERO]);
    ml.context.arrowhead_size = 0.0;
    ml.context.leader_roots[0].lines[0]
        .points
        .extend([Vector3::new(5.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)]);

    let lines = line_ends(&EntityType::MultiLeader(Box::new(ml)).flatten());

    assert_eq!(
        lines,
        vec![
            (Vector3::ZERO, Vector3::new(5.0, 0.0, 0.0)),
            (Vector3::new(5.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)),
            (Vector3::new(10.0, 10.0, 0.0), Vector3::new(15.0, 10.0, 0.0)),
        ]
    );
}

#[test]
fn test_explode_multileader_applies_segment_breaks() {
    let mut ml = callout(&[Vector3::ZERO]);
    ml.context.arrowhead_size = 0.0;
    ml.context.leader_roots[0].lines[0]
        .break_points
        .push(StartEndPointPair::new(
            Vector3::new(2.0, 2.0, 0.0),
            Vector3::new(3.0, 3.0, 0.0),
        ));
    let lines = line_ends(&EntityType::MultiLeader(Box::new(ml)).flatten());
    assert_eq!(
        lines,
        vec![
            (Vector3::ZERO, Vector3::new(2.0, 2.0, 0.0)),
            (Vector3::new(3.0, 3.0, 0.0), Vector3::new(10.0, 10.0, 0.0)),
            (Vector3::new(10.0, 10.0, 0.0), Vector3::new(15.0, 10.0, 0.0)),
        ]
    );
}

#[test]
fn test_explode_multileader_lines_share_one_landing() {
    let ml = callout(&[Vector3::ZERO, Vector3::new(20.0, 0.0, 0.0)]);
    let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

    // One arrowhead per leader line, never one on the landing.
    assert_eq!(solids(&parts).len(), 2);
    let landings = line_ends(&parts)
        .into_iter()
        .filter(|(start, _)| *start == Vector3::new(10.0, 10.0, 0.0))
        .count();
    assert_eq!(landings, 1);
}

#[test]
fn test_explode_multileader_each_root_draws_its_own_path() {
    let mut ml = callout(&[Vector3::ZERO]);
    let mut second = ml.context.leader_roots[0].clone();
    second.connection_point = Vector3::new(30.0, 10.0, 0.0);
    ml.context.leader_roots.push(second);

    let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

    assert_eq!(solids(&parts).len(), 2);
    assert_eq!(line_ends(&parts).len(), 4);
}

#[test]
fn test_explode_multileader_invisible_path_keeps_only_the_text() {
    let mut ml = callout(&[Vector3::ZERO]);
    ml.path_type = MultiLeaderPathType::Invisible;
    ml.context.has_text_contents = true;
    ml.context.text_string = "NOTE".to_string();

    let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

    assert_eq!(parts.len(), 1);
    assert!(matches!(&parts[0], EntityType::MText(_)));
}

#[test]
fn test_explode_multileader_without_dogleg_has_no_landing() {
    let mut ml = callout(&[Vector3::ZERO]);
    ml.enable_dogleg = false;

    let lines = line_ends(&EntityType::MultiLeader(Box::new(ml)).flatten());

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].1, Vector3::new(10.0, 10.0, 0.0));
}

#[test]
fn test_explode_multileader_custom_arrow_block_is_not_drawn_as_the_default() {
    let mut entity_level = callout(&[Vector3::ZERO]);
    entity_level.arrowhead_handle = Some(Handle::new(0x2A));

    let mut line_level = callout(&[Vector3::ZERO]);
    let line = &mut line_level.context.leader_roots[0].lines[0];
    line.arrowhead_handle = Some(Handle::new(0x2A));
    line.override_flags = LeaderLinePropertyOverrideFlags::ARROWHEAD;

    for ml in [entity_level, line_level] {
        let parts = EntityType::MultiLeader(Box::new(ml)).flatten();
        assert!(solids(&parts).is_empty());
        // The shaft is not trimmed when no arrowhead covers its start.
        assert_eq!(line_ends(&parts)[0].0, Vector3::ZERO);
    }
}

#[test]
fn test_explode_multileader_null_arrow_handle_is_the_default_arrow() {
    let mut ml = callout(&[Vector3::ZERO]);
    ml.arrowhead_handle = Some(Handle::NULL);

    assert_eq!(
        solids(&EntityType::MultiLeader(Box::new(ml)).flatten()).len(),
        1
    );
}

#[test]
fn test_explode_multileader_line_overrides_the_arrow_size() {
    let mut ml = callout(&[Vector3::ZERO]);
    let line = &mut ml.context.leader_roots[0].lines[0];
    line.arrowhead_size = 2.0;
    line.override_flags = LeaderLinePropertyOverrideFlags::ARROWHEAD_SIZE;

    let parts = EntityType::MultiLeader(Box::new(ml)).flatten();
    let base = (solids(&parts)[0].second_corner + solids(&parts)[0].third_corner) * 0.5;

    assert!((base.length() - 2.0).abs() < 1e-9);
}

#[test]
fn test_explode_multileader_arrow_longer_than_its_segment_is_dropped() {
    let mut ml = callout(&[Vector3::new(9.5, 10.0, 0.0)]);
    ml.context.arrowhead_size = 1.0;

    let parts = EntityType::MultiLeader(Box::new(ml)).flatten();

    assert!(solids(&parts).is_empty());
    assert_eq!(line_ends(&parts)[0].0, Vector3::new(9.5, 10.0, 0.0));
}

#[test]
fn test_explode_mtext() {
    let mtext = MText::with_value("Hello World", Vector3::new(5.0, 5.0, 0.0));
    let entity = EntityType::MText(Box::new(mtext));
    let parts = entity.flatten();
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        EntityType::Text(t) => {
            assert_eq!(t.value, "Hello World");
            // default attachment is top left: the baseline lies one text height below
            assert_eq!(t.insertion_point, Vector3::new(5.0, 5.0 - t.height, 0.0));
        }
        _ => panic!("Expected Text from MText explosion"),
    }
}

#[test]
fn test_explode_mtext_paragraphs_and_formatting() {
    let mut mtext = MText::with_value("{\\fArial|b1;Line 1}\\PLine 2", Vector3::new(0.0, 0.0, 0.0));
    mtext.height = 2.0;
    let parts = EntityType::MText(Box::new(mtext)).flatten();
    let values: Vec<&str> = parts
        .iter()
        .map(|p| match p {
            EntityType::Text(t) => t.value.as_str(),
            _ => panic!("Expected Text"),
        })
        .collect();
    assert_eq!(values, ["Line 1", "Line 2"]);
    // second line one line spacing (5/3 of the height) lower
    let y = |i: usize| match &parts[i] {
        EntityType::Text(t) => t.insertion_point.y,
        _ => unreachable!(),
    };
    assert!((y(0) - y(1) - 2.0 * 5.0 / 3.0).abs() < 1e-9);
}

#[test]
fn test_explode_mtext_preserves_formatted_runs() {
    let mtext = MText::with_value(r"A{\C1;RED}B{\H2x;BIG}", Vector3::ZERO);
    let parts = EntityType::MText(Box::new(mtext)).explode();
    assert_eq!(parts.len(), 4);
    let texts: Vec<_> = parts
        .iter()
        .map(|part| match part {
            EntityType::Text(text) => text,
            _ => panic!("expected text run"),
        })
        .collect();
    assert_eq!(texts[0].value, "A");
    assert_eq!(texts[1].value, "RED");
    assert_eq!(texts[1].common.color, Color::from_index(1));
    assert_eq!(texts[2].value, "B");
    assert!((texts[3].height - 2.0).abs() < 1e-12);
}

#[test]
fn test_explode_mtext_uses_tilted_normal_axes() {
    let mut mtext = MText::with_value("A\nB", Vector3::ZERO);
    mtext.normal = Vector3::new(0.0, 1.0, 0.0);
    mtext.dwg_x_direction = Some(Vector3::UNIT_X);
    let parts = EntityType::MText(Box::new(mtext)).explode();
    let texts: Vec<_> = parts
        .iter()
        .filter_map(|part| match part {
            EntityType::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 2);
    assert!(texts
        .iter()
        .all(|text| text.insertion_point.y.abs() < 1e-12));
    assert!(texts[1].insertion_point.z > texts[0].insertion_point.z);
    assert_eq!(texts[0].normal, Vector3::new(0.0, 1.0, 0.0));
}

#[test]
fn test_explode_mtext_static_columns_flow_across_gutter() {
    let mut mtext = MText::with_value("A\nB\nC", Vector3::ZERO);
    mtext.rectangle_width = 10.0;
    mtext.column_data.column_type = 1;
    mtext.column_data.column_count = 2;
    mtext.column_data.gutter = 2.0;
    let parts = EntityType::MText(Box::new(mtext)).explode();
    let texts: Vec<_> = parts
        .iter()
        .filter_map(|part| match part {
            EntityType::Text(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(texts.len(), 3);
    assert!((texts[2].insertion_point.x - 12.0).abs() < 1e-12);
    assert!((texts[2].insertion_point.y - texts[0].insertion_point.y).abs() < 1e-12);
}

#[test]
fn test_explode_mesh_triangles() {
    let mesh = Mesh::from_triangles(
        vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(5.0, 10.0, 0.0),
        ],
        &[(0, 1, 2)],
    );
    let entity = EntityType::Mesh(mesh);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 1);
    assert!(matches!(&parts[0], EntityType::Face3D(_)));
}

#[test]
fn test_explode_ellipse() {
    let ellipse = Ellipse::from_center_axes(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0), 0.5);
    let entity = EntityType::Ellipse(ellipse);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 1);
    assert!(matches!(&parts[0], EntityType::Spline(_)));
}

#[test]
fn test_explode_inherits_common_properties() {
    let mut circle = Circle::from_center_radius(Vector3::ZERO, 5.0);
    circle.common.layer = "WALLS".to_string();
    circle.common.color = Color::Index(1);
    circle.common.linetype = "DASHED".to_string();

    let entity = EntityType::Circle(circle);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 1);
    let common = parts[0].common();
    assert_eq!(common.layer, "WALLS");
    assert_eq!(common.color, Color::Index(1));
    assert_eq!(common.linetype, "DASHED");
    assert!(common.handle.is_null()); // handle not assigned yet
}

#[test]
fn test_explode_spline() {
    let pts = vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(3.0, 10.0, 0.0),
        Vector3::new(7.0, 10.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
    ];
    let spline = Spline::from_control_points(3, pts);
    let entity = EntityType::Spline(spline);
    let parts = entity.flatten();
    // one clamped cubic span sampled with 16 segments
    assert_eq!(parts.len(), 16);
    let ends: Vec<(Vector3, Vector3)> = parts
        .iter()
        .map(|p| match p {
            EntityType::Line(l) => (l.start, l.end),
            _ => panic!("Expected Line"),
        })
        .collect();
    // the curve, not the control polygon: starts/ends at the end points
    // and passes through the Bezier midpoint (5, 7.5)
    assert_eq!(ends[0].0, Vector3::new(0.0, 0.0, 0.0));
    assert!((ends[15].1 - Vector3::new(10.0, 0.0, 0.0)).length() < 1e-9);
    assert!((ends[7].1 - Vector3::new(5.0, 7.5, 0.0)).length() < 1e-9);
}

#[test]
fn test_nurbs_rational_quarter_circle() {
    // quadratic rational Bezier with weights 1, sqrt(2)/2, 1 is an exact quarter circle
    let pts = [
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(1.0, 1.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    ];
    let w = [1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0];
    let knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    for p in nurbs_points(2, &pts, &w, &knots, 8) {
        assert!((p.length() - 1.0).abs() < 1e-12);
    }
}

#[test]
fn test_explode_dimension() {
    let dim = DimensionLinear::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));
    let entity = EntityType::Dimension(Box::new(Dimension::Linear(dim)));
    let parts = entity.flatten();
    // Should contain dimension lines + text
    assert!(parts.len() >= 2);
    assert!(parts.iter().any(|p| matches!(p, EntityType::MText(_))));
    assert!(parts.iter().any(|p| matches!(p, EntityType::Line(_))));
}

#[test]
fn test_explode_region_preserves_acis_analytic_edges() {
    let sat = crate::entities::acis::primitives::build_cylinder([0.0, 0.0, 0.0], 2.0, 3.0);
    let region = Region::from_sat(&sat.to_sat_string());
    let parts = EntityType::Region(region).explode();
    let circles: Vec<_> = parts
        .iter()
        .filter_map(|part| match part {
            EntityType::Circle(circle) => Some(circle),
            _ => None,
        })
        .collect();
    assert_eq!(circles.len(), 4);
    assert!(circles
        .iter()
        .all(|circle| (circle.radius - 2.0).abs() < 1e-9));
}

#[test]
fn test_explode_linear_dimension_projects_onto_offset_line() {
    let mut dimension =
        DimensionLinear::horizontal(Vector3::new(1.0, 2.0, 0.0), Vector3::new(11.0, 4.0, 0.0));
    dimension.definition_point = Vector3::new(11.0, 8.0, 0.0);
    let parts = EntityType::Dimension(Box::new(Dimension::Linear(dimension))).flatten();
    let lines = line_ends(&parts);
    assert_eq!(lines.len(), 4);
    assert_eq!(
        lines[0],
        (Vector3::new(1.0, 2.0, 0.0), Vector3::new(1.0, 8.0, 0.0))
    );
    assert_eq!(
        lines[1],
        (Vector3::new(11.0, 4.0, 0.0), Vector3::new(11.0, 8.0, 0.0))
    );
    assert_eq!(lines[2].0, Vector3::new(1.0, 8.0, 0.0));
    assert_eq!(lines[3].1, Vector3::new(11.0, 8.0, 0.0));
}

#[test]
fn test_explode_aligned_dimension_projects_onto_offset_line() {
    let mut dimension = DimensionAligned::new(Vector3::ZERO, Vector3::new(3.0, 4.0, 0.0));
    dimension.definition_point = Vector3::new(-1.0, 7.0, 0.0);
    let parts = EntityType::Dimension(Box::new(Dimension::Aligned(dimension))).flatten();
    let lines = line_ends(&parts);
    assert!(lines[0].1.distance(&Vector3::new(-4.0, 3.0, 0.0)) < 1e-12);
    assert!(lines[1].1.distance(&Vector3::new(-1.0, 7.0, 0.0)) < 1e-12);
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[2], (Vector3::ZERO, Vector3::new(3.0, 4.0, 0.0)));
}

#[test]
fn test_explode_dimension_uses_dimstyle_geometry_and_formatting() {
    let mut dimension = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    dimension.definition_point = Vector3::new(0.0, 5.0, 0.0);
    dimension.base.text_middle_point = Vector3::new(5.0, 5.0, 0.0);
    dimension.base.style_name = "CUSTOM".to_owned();

    let mut style = crate::tables::DimStyle::new("CUSTOM");
    style.dimscale = 1.0;
    style.dimasz = 1.0;
    style.dimexo = 1.0;
    style.dimexe = 2.0;
    style.dimgap = 0.5;
    style.dimtxt = 3.0;
    style.dimlfac = 2.0;
    style.dimdec = 1;
    style.dimpost = "<> mm".to_owned();

    let parts = explode_dimension_with_style(&Dimension::Linear(dimension), Some(&style));
    let lines = line_ends(&parts);
    assert_eq!(lines.len(), 4);
    assert_eq!(
        lines[0],
        (Vector3::new(0.0, 1.0, 0.0), Vector3::new(0.0, 7.0, 0.0))
    );
    assert_eq!(
        lines[1],
        (Vector3::new(10.0, 1.0, 0.0), Vector3::new(10.0, 7.0, 0.0))
    );
    assert_eq!(
        lines[2],
        (Vector3::new(1.0, 5.0, 0.0), Vector3::new(4.5, 5.0, 0.0))
    );
    assert_eq!(
        lines[3],
        (Vector3::new(5.5, 5.0, 0.0), Vector3::new(9.0, 5.0, 0.0))
    );

    let arrows = solids(&parts);
    assert_eq!(arrows.len(), 2);
    let arrow_base = (arrows[0].second_corner + arrows[0].third_corner) * 0.5;
    assert!((arrows[0].first_corner.distance(&arrow_base) - 1.0).abs() < 1e-12);
    let text = parts
        .iter()
        .find_map(|part| match part {
            EntityType::MText(text) => Some(text),
            _ => None,
        })
        .expect("dimension text");
    assert_eq!(text.value, "\\A1;20.0 mm");
    assert_eq!(text.height, 3.0);
    assert_eq!(text.style, "Standard");
}

#[test]
fn test_explode_dimension_dimstyle_suppression_and_text_blank() {
    let mut dimension = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    dimension.definition_point = Vector3::new(0.0, 5.0, 0.0);
    dimension.base.text_middle_point = Vector3::new(5.0, 5.0, 0.0);
    dimension.base.set_text_override(Some(" ".to_owned()));
    let mut style = crate::tables::DimStyle::new("CUSTOM");
    style.dimse1 = true;
    style.dimse2 = true;
    style.dimsd1 = true;
    style.dimsd2 = true;
    let parts = explode_dimension_with_style(&Dimension::Linear(dimension), Some(&style));
    assert!(line_ends(&parts).is_empty());
    assert!(solids(&parts).len() == 2);
    assert!(!parts
        .iter()
        .any(|part| matches!(part, EntityType::MText(_))));
}

#[test]
fn test_document_explode_dimension_resolves_named_dimstyle() {
    let mut document = crate::document::CadDocument::new();
    let mut style = crate::tables::DimStyle::new("DOC_STYLE");
    style.dimtxt = 4.0;
    style.dimdec = 0;
    style.dimlfac = 3.0;
    document.dim_styles.add(style).expect("new style");

    let mut dimension = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(2.0, 0.0, 0.0));
    dimension.definition_point = Vector3::new(0.0, 5.0, 0.0);
    dimension.base.text_middle_point = Vector3::new(1.0, 5.0, 0.0);
    dimension.base.style_name = "DOC_STYLE".to_owned();
    let parts = document.explode_entity(&EntityType::Dimension(Box::new(Dimension::Linear(
        dimension,
    ))));

    let text = parts
        .iter()
        .find_map(|part| match part {
            EntityType::MText(text) => Some(text),
            _ => None,
        })
        .expect("dimension text");
    assert_eq!(text.height, 4.0);
    assert_eq!(text.value, "\\A1;6");
    assert!(parts.iter().all(|part| part.common().handle.is_valid()));
}

#[test]
fn test_document_explode_dimension_gives_its_block() {
    let mut document = crate::document::CadDocument::new();
    let mut block = crate::tables::BlockRecord::new("*D1");
    block.handle = document.allocate_handle();
    let mut drawn = Line::from_points(Vector3::new(0.0, 5.0, 0.0), Vector3::new(2.0, 5.0, 0.0));
    drawn.common.color = Color::ByBlock;
    let line = document.add_entity(EntityType::Line(drawn)).unwrap();
    block.entity_handles.push(line);
    document.block_records.add(block).unwrap();

    let mut dimension = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(2.0, 0.0, 0.0));
    dimension.definition_point = Vector3::new(0.0, 5.0, 0.0);
    dimension.base.block_name = "*D1".to_owned();
    dimension.base.common.color = Color::from_index(3);
    let parts = document.explode_entity(&EntityType::Dimension(Box::new(Dimension::Linear(
        dimension,
    ))));

    let [EntityType::Line(part)] = &parts[..] else {
        panic!("expected the block's line, got {parts:?}");
    };
    assert_eq!(part.start, Vector3::new(0.0, 5.0, 0.0));
    assert_eq!(part.end, Vector3::new(2.0, 5.0, 0.0));
    assert_eq!(part.common.color, Color::from_index(3));
    assert!(part.common.handle.is_valid());
}

#[test]
fn test_explode_polyface_mesh() {
    let mut mesh = PolyfaceMesh::new();
    mesh.add_vertex(PolyfaceVertex::new(Vector3::new(0.0, 0.0, 0.0)));
    mesh.add_vertex(PolyfaceVertex::new(Vector3::new(10.0, 0.0, 0.0)));
    mesh.add_vertex(PolyfaceVertex::new(Vector3::new(5.0, 10.0, 0.0)));
    mesh.add_face(PolyfaceFace::triangle(1, 2, 3));

    let entity = EntityType::PolyfaceMesh(mesh);
    let parts = entity.flatten();
    assert_eq!(parts.len(), 1);
    assert!(matches!(&parts[0], EntityType::Face3D(_)));
}

#[test]
fn test_explode_polyface_low_vertex_faces() {
    let mut mesh = PolyfaceMesh::new();
    mesh.add_vertex(PolyfaceVertex::new(Vector3::ZERO));
    mesh.add_vertex(PolyfaceVertex::new(Vector3::UNIT_X));
    mesh.add_face(PolyfaceFace::triangle(1, 0, 0));
    mesh.add_face(PolyfaceFace::triangle(1, 2, 0));
    let parts = EntityType::PolyfaceMesh(mesh).explode();
    assert!(matches!(parts[0], EntityType::Point(_)));
    assert!(matches!(parts[1], EntityType::Line(_)));
}

#[test]
fn test_explode_box_solid_into_planar_regions() {
    let sat = crate::entities::acis::primitives::build_box([0.0, 0.0, 0.0], 10.0, 10.0, 10.0);
    let solid = Solid3D::from_sat(&sat.to_sat_string());
    let parts = EntityType::Solid3D(solid).explode();
    assert_eq!(parts.len(), 6);
    assert!(parts
        .iter()
        .all(|part| matches!(part, EntityType::Region(_))));
}

#[test]
fn test_explode_curved_cylinder_into_surface_and_planar_regions() {
    let sat = crate::entities::acis::primitives::build_cylinder([0.0, 0.0, 0.0], 2.0, 3.0);
    let solid = Solid3D::from_sat(&sat.to_sat_string());
    let parts = EntityType::Solid3D(solid).explode();
    assert_eq!(
        parts
            .iter()
            .filter(|part| matches!(part, EntityType::Surface(_)))
            .count(),
        1
    );
    assert_eq!(
        parts
            .iter()
            .filter(|part| matches!(part, EntityType::Region(_)))
            .count(),
        2
    );
}

#[test]
fn test_native_table_graphics_have_grid_and_cell_text() {
    let mut table = Table::new(Vector3::ZERO, 2, 2);
    table.set_cell_text(0, 0, "Title");
    table.set_cell_text(1, 0, "A");
    table.set_cell_text(1, 1, "B");
    let parts = EntityType::Table(Box::new(table)).explode();
    assert_eq!(
        parts
            .iter()
            .filter(|part| matches!(part, EntityType::Line(_)))
            .count(),
        7
    );
    assert_eq!(
        parts
            .iter()
            .filter(|part| matches!(part, EntityType::MText(_)))
            .count(),
        3
    );
}

#[test]
fn test_document_explode_entity_assigns_handles() {
    use crate::document::CadDocument;

    let mut doc = CadDocument::new();
    let mut polyline = LwPolyline::new();
    polyline.add_point(Vector2::ZERO);
    polyline.add_point(Vector2::new(5.0, 0.0));
    polyline.common.owner_handle = Handle::new(0x100);

    let entity = EntityType::LwPolyline(polyline);
    let parts = doc.explode_entity(&entity);
    assert_eq!(parts.len(), 1);
    assert!(parts[0].common().handle.is_valid());
    assert_eq!(parts[0].common().owner_handle, Handle::new(0x100));
}

#[test]
fn test_document_explode_dimension_resolves_dimstyle() {
    use crate::document::CadDocument;

    let mut document = CadDocument::new();
    let mut style = crate::tables::DimStyle::new("CUSTOM");
    style.dimlfac = 2.0;
    style.dimdec = 1;
    style.dimtxt = 3.0;
    style.dimpost = "<> mm".to_string();
    document.dim_styles.add(style).unwrap();

    let mut dimension = DimensionLinear::horizontal(Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
    dimension.definition_point = Vector3::new(0.0, 5.0, 0.0);
    dimension.base.text_middle_point = Vector3::new(5.0, 5.0, 0.0);
    dimension.base.style_name = "CUSTOM".to_string();
    let parts = document.explode_entity(&EntityType::Dimension(Box::new(Dimension::Linear(
        dimension,
    ))));
    let text = parts
        .iter()
        .find_map(|part| match part {
            EntityType::MText(text) => Some(text),
            _ => None,
        })
        .expect("dimension MTEXT");
    assert_eq!(text.value, "\\A1;20.0 mm");
    assert!((text.height - 3.0).abs() < 1e-12);
}
