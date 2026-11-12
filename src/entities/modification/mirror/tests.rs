use super::*;

#[test]
fn spline_mirror_covers_rational_periodic_closed_and_fit_data() {
    let mut spline = Spline::from_control_points(
        3,
        vec![
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(4.0, -1.0, 5.0),
            Vector3::new(7.0, 3.0, -2.0),
            Vector3::new(9.0, 8.0, 1.0),
        ],
    );
    spline.flags.closed = true;
    spline.flags.periodic = true;
    spline.flags.rational = true;
    spline.flags.planar = true;
    spline.weights = vec![1.0, 0.5, 2.0, 1.25];
    spline.fit_points = vec![Vector3::new(2.0, 3.0, 4.0), Vector3::new(5.0, 6.0, 7.0)];
    spline.begin_tangent = Vector3::new(1.0, 2.0, 0.0);
    spline.end_tangent = Vector3::new(-2.0, 1.0, 0.0);
    spline.normal = Vector3::new(1.0, 2.0, 3.0).normalize();
    let original = spline.clone();
    let transform =
        Transform::from_mirror_plane(Vector3::new(2.0, -3.0, 4.0), Vector3::new(1.0, 3.0, -2.0));
    spline.mirror_about_plane(Vector3::new(2.0, -3.0, 4.0), Vector3::new(1.0, 3.0, -2.0));
    for (actual, expected) in spline.control_points.iter().zip(&original.control_points) {
        close(*actual, transform.apply(*expected));
    }
    for (actual, expected) in spline.fit_points.iter().zip(&original.fit_points) {
        close(*actual, transform.apply(*expected));
    }
    close(
        spline.begin_tangent,
        transform.apply_rotation(original.begin_tangent),
    );
    close(
        spline.end_tangent,
        transform.apply_rotation(original.end_tangent),
    );
    close(
        spline.normal,
        transform.apply_rotation(original.normal).normalize(),
    );
    assert_eq!(spline.knots, original.knots);
    assert_eq!(spline.weights, original.weights);
    assert_eq!(spline.flags, original.flags);
    spline.mirror_about_plane(Vector3::new(2.0, -3.0, 4.0), Vector3::new(1.0, 3.0, -2.0));
    for (actual, expected) in spline.control_points.iter().zip(&original.control_points) {
        close(*actual, *expected);
    }
    for (actual, expected) in spline.fit_points.iter().zip(&original.fit_points) {
        close(*actual, *expected);
    }
}

#[test]
fn bspline_polyline_mirror_preserves_smoothing_and_vertex_metadata() {
    let mut polyline = Polyline2D::new();
    polyline.smooth_surface = SmoothSurfaceType::CubicBSpline;
    polyline.flags.set_closed(true);
    polyline.normal = Vector3::new(1.0, 2.0, 3.0).normalize();
    polyline.elevation = 4.0;
    for (index, point) in [
        Vector3::new(1.0, 2.0, 4.0),
        Vector3::new(4.0, -2.0, 4.0),
        Vector3::new(7.0, 3.0, 4.0),
        Vector3::new(9.0, 8.0, 4.0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut vertex = Vertex2D::new(point);
        vertex.bulge = if index % 2 == 0 { 0.25 } else { -0.5 };
        vertex.curve_tangent = index as f64 * 0.3;
        vertex.id = index as i32 + 10;
        polyline.add_vertex(vertex);
    }
    let original = polyline.clone();
    let transform = Transform::from_mirror_plane(Vector3::new(2.0, 4.0, -1.0), Vector3::UNIT_X);
    polyline.mirror_about_plane(Vector3::new(2.0, 4.0, -1.0), Vector3::UNIT_X);
    let old_ocs = Matrix3::arbitrary_axis(original.normal);
    let new_ocs = Matrix3::arbitrary_axis(polyline.normal);
    for (actual, expected) in polyline.vertices.iter().zip(&original.vertices) {
        close(
            new_ocs * actual.location,
            transform.apply(old_ocs * expected.location),
        );
        assert!((actual.bulge + expected.bulge).abs() < 1e-12);
        assert_eq!(actual.curve_tangent, expected.curve_tangent);
        assert_eq!(actual.id, expected.id);
    }
    assert_eq!(polyline.smooth_surface, original.smooth_surface);
    assert_eq!(polyline.flags, original.flags);
}

#[test]
fn bspline_polygon_mesh_mirror_preserves_grid_smoothing_and_closure() {
    let mut mesh = PolygonMeshEntity::new();
    mesh.m_vertex_count = 3;
    mesh.n_vertex_count = 4;
    mesh.m_smooth_density = 5;
    mesh.n_smooth_density = 6;
    mesh.smooth_type = SurfaceSmoothType::Cubic;
    mesh.flags
        .insert(PolygonMeshFlags::CLOSED_M | PolygonMeshFlags::CLOSED_N);
    mesh.normal = Vector3::new(1.0, -2.0, 3.0).normalize();
    mesh.elevation = 2.5;
    for row in 0..mesh.n_vertex_count {
        for column in 0..mesh.m_vertex_count {
            mesh.vertices.push(PolygonMeshVertex::at(Vector3::new(
                column as f64 * 2.0,
                row as f64 * 3.0,
                row as f64 + column as f64,
            )));
        }
    }
    let original = mesh.clone();
    let transform =
        Transform::from_mirror_plane(Vector3::new(2.0, 3.0, 4.0), Vector3::new(2.0, 1.0, -3.0));
    mesh.mirror_about_plane(Vector3::new(2.0, 3.0, 4.0), Vector3::new(2.0, 1.0, -3.0));
    let m = original.m_vertex_count as usize;
    for (index, actual) in mesh.vertices.iter().enumerate() {
        let row = index / m;
        let column = index % m;
        let expected = &original.vertices[row * m + (m - 1 - column)];
        close(actual.location, transform.apply(expected.location));
    }
    close(
        mesh.normal,
        transform.apply_rotation(original.normal).normalize(),
    );
    assert_eq!(mesh.m_vertex_count, original.m_vertex_count);
    assert_eq!(mesh.n_vertex_count, original.n_vertex_count);
    assert_eq!(mesh.m_smooth_density, original.m_smooth_density);
    assert_eq!(mesh.n_smooth_density, original.n_smooth_density);
    assert_eq!(mesh.smooth_type, original.smooth_type);
    assert_eq!(mesh.flags, original.flags);
}

#[test]
fn subdivision_mesh_mirror_preserves_faces_edges_creases_and_level() {
    let mut mesh = Mesh::from_quads(
        vec![
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(5.0, 2.0, 3.0),
            Vector3::new(5.0, 6.0, 3.0),
            Vector3::new(1.0, 6.0, 3.0),
        ],
        &[(0, 1, 2, 3)],
    );
    mesh.subdivision_level = 3;
    mesh.blend_crease = false;
    mesh.edges[0].crease = Some(1.75);
    let original = mesh.clone();
    mesh.mirror_x();
    assert_eq!(mesh.faces, original.faces);
    assert_eq!(mesh.edges, original.edges);
    assert_eq!(mesh.subdivision_level, original.subdivision_level);
    assert_eq!(mesh.blend_crease, original.blend_crease);
    for (actual, expected) in mesh.vertices.iter().zip(&original.vertices) {
        assert_eq!(*actual, Vector3::new(-expected.x, expected.y, expected.z));
    }
}

#[test]
fn solid3d_mirror_transforms_acis_placement_and_wireframe_without_losing_topology() {
    let sat = crate::entities::acis::primitives::build_box([2.0, 3.0, 4.0], 4.0, 6.0, 8.0);
    let mut solid = Solid3D::from_sat(&sat.to_sat_string());
    solid.point_of_reference = Vector3::new(2.0, 3.0, 4.0);
    solid.wires.push(Wire::from_points(vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(2.0, 3.0, 4.0),
    ]));
    let before_topology = solid.parse_sat().unwrap().faces().len();
    let original_wire = solid.wires[0].points.clone();
    let mirror =
        Transform::from_mirror_plane(Vector3::new(1.0, -2.0, 3.0), Vector3::new(2.0, 1.0, -1.0));
    solid.mirror_about_plane(Vector3::new(1.0, -2.0, 3.0), Vector3::new(2.0, 1.0, -1.0));
    close(
        solid.point_of_reference,
        mirror.apply(Vector3::new(2.0, 3.0, 4.0)),
    );
    for (actual, expected) in solid.wires[0].points.iter().zip(original_wire) {
        close(*actual, mirror.apply(expected));
    }
    assert_eq!(solid.parse_sat().unwrap().faces().len(), before_topology);
}

#[test]
fn all_acis_solid_family_entities_mirror_without_losing_brep_topology() {
    let sat = crate::entities::acis::primitives::build_box([0.0, 0.0, 0.0], 4.0, 5.0, 6.0);
    let topology = sat.faces().len();
    let mirror =
        Transform::from_mirror_plane(Vector3::new(3.0, -2.0, 1.0), Vector3::new(2.0, -1.0, 3.0));
    let wire = || {
        Wire::from_points(vec![
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(4.0, 5.0, 6.0),
        ])
    };
    let mut solid = Solid3D::from_sat(&sat.to_sat_string());
    solid.point_of_reference = Vector3::new(1.0, 2.0, 3.0);
    solid.wires.push(wire());
    let mut region = Region::from_sat(&sat.to_sat_string());
    region.point_of_reference = Vector3::new(1.0, 2.0, 3.0);
    region.wires.push(wire());
    let mut body = Body::from_sat(&sat.to_sat_string());
    body.point_of_reference = Vector3::new(1.0, 2.0, 3.0);
    body.wires.push(wire());
    let mut surface = Surface::new(SurfaceKind::Plane);
    surface.acis_data = AcisData::from_sat(&sat.to_sat_string());
    surface.point_of_reference = Vector3::new(1.0, 2.0, 3.0);
    surface.wires.push(wire());
    let mut entities = vec![
        EntityType::Solid3D(solid),
        EntityType::Region(region),
        EntityType::Body(body),
        EntityType::Surface(Box::new(surface)),
    ];
    for entity in &mut entities {
        entity.mirror3d(MirrorPlane::PointNormal {
            point: Vector3::new(3.0, -2.0, 1.0),
            normal: Vector3::new(2.0, -1.0, 3.0),
        });
        let (point, wires, parsed) = match entity {
            EntityType::Solid3D(value) => {
                (&value.point_of_reference, &value.wires, value.parse_sat())
            }
            EntityType::Region(value) => {
                (&value.point_of_reference, &value.wires, value.parse_sat())
            }
            EntityType::Body(value) => (&value.point_of_reference, &value.wires, value.parse_sat()),
            EntityType::Surface(value) => {
                (&value.point_of_reference, &value.wires, value.parse_sat())
            }
            _ => unreachable!(),
        };
        close(*point, mirror.apply(Vector3::new(1.0, 2.0, 3.0)));
        assert_eq!(parsed.unwrap().faces().len(), topology);
        close(
            wires[0].points[0],
            mirror.apply(Vector3::new(1.0, 2.0, 3.0)),
        );
        close(
            wires[0].points[1],
            mirror.apply(Vector3::new(4.0, 5.0, 6.0)),
        );
    }
}

#[test]
fn solid_and_trace_mirror_preserve_the_entity_family_and_storage_flags() {
    let corners = [
        Vector3::new(1.0, 2.0, 0.0),
        Vector3::new(5.0, 2.0, 0.0),
        Vector3::new(1.0, 6.0, 0.0),
        Vector3::new(5.0, 6.0, 0.0),
    ];
    let mut solid = Solid::new(corners[0], corners[1], corners[2], corners[3]);
    solid.is_trace = false;
    let mut trace = Solid::new(corners[0], corners[1], corners[2], corners[3]);
    trace.is_trace = true;
    let mut entities = [EntityType::Solid(solid), EntityType::Solid(trace)];
    if let EntityType::Solid(value) = &mut entities[0] {
        value.mirror_x();
    }
    if let EntityType::Solid(value) = &mut entities[1] {
        value.mirror_x();
    }
    let EntityType::Solid(solid) = &entities[0] else {
        unreachable!()
    };
    let EntityType::Solid(trace) = &entities[1] else {
        unreachable!()
    };
    assert!(!solid.is_trace);
    assert!(trace.is_trace);
    assert_eq!(solid.boundary_corners(), trace.boundary_corners());
}

#[test]
fn tilted_mtext_retains_its_world_direction_in_dwg_and_dxf() {
    use std::io::Cursor;
    for mirror_text in [false, true] {
        for normal in [
            Vector3::new(0.0, 0.6, 0.8),
            Vector3::new(1.0, 2.0, 3.0).normalize(),
        ] {
            let direction =
                Matrix3::arbitrary_axis(normal) * Vector3::new(0.4_f64.cos(), -0.4_f64.sin(), 0.0);
            let mut text = MText::with_value("Tilted", Vector3::new(2.0, 3.0, 4.0));
            text.normal = normal;
            text.rotation = direction.y.atan2(direction.x);
            text.dwg_x_direction = Some(direction);
            let mut entity = EntityType::MText(Box::new(text));
            entity.mirror3d_with_options(
                MirrorPlane::PointNormal {
                    point: Vector3::new(11.0, 22.0, 33.0),
                    normal: Vector3::new(1.0, -2.0, 3.0),
                },
                MirrorOptions {
                    mirror_text,
                    ..MirrorOptions::default()
                },
            );
            let expected = match &entity {
                EntityType::MText(text) => text.as_ref().clone(),
                _ => unreachable!(),
            };
            let mut document = CadDocument::with_version(DxfVersion::AC1032);
            let handle = document.add_entity(entity).unwrap();
            let dwg = crate::DwgWriter::write_to_vec(&document).unwrap();
            let dwg = crate::DwgReader::from_stream(Cursor::new(dwg))
                .read()
                .unwrap();
            let dxf = crate::DxfWriter::new(&document).write_to_vec().unwrap();
            let dxf = crate::DxfReader::from_reader(Cursor::new(dxf))
                .unwrap()
                .read()
                .unwrap();
            for restored in [&dwg, &dxf] {
                let EntityType::MText(actual) = restored.get_entity(handle).unwrap() else {
                    unreachable!()
                };
                close(
                    actual.dwg_x_direction.unwrap(),
                    expected.dwg_x_direction.unwrap(),
                );
                close(actual.normal, expected.normal);
                close(actual.insertion_point, expected.insertion_point);
                assert_eq!(actual.attachment_point, expected.attachment_point);
            }
        }
    }
}

#[test]
fn mtext_mirror_respects_an_edited_rotation_over_a_stale_direction_cache() {
    let mut text = MText::new();
    text.dwg_x_direction = Some(Vector3::UNIT_X);
    text.rotation = -0.4;
    text.apply_mirror_with_options(
        &Transform::from_mirror_x(),
        MirrorOptions {
            mirror_text: true,
            ..MirrorOptions::default()
        },
    );
    close(
        text.dwg_x_direction.unwrap(),
        Vector3::new(-0.4_f64.cos(), -0.4_f64.sin(), 0.0),
    );
}

#[test]
fn nearly_vertical_mtext_direction_survives_geometric_mirror_and_dwg_save() {
    use std::io::Cursor;
    let reflected_direction = Vector3::new(2e-14, -3e-14, 1.0);
    let mirror = Transform::from_mirror_plane(Vector3::ZERO, Vector3::new(1.0, 2.0, 3.0));
    let original_direction = mirror.apply_rotation(reflected_direction);
    let mut text = MText::with_value("Vertical", Vector3::ZERO);
    text.normal = original_direction.cross(&Vector3::UNIT_X).normalize();
    text.dwg_x_direction = Some(original_direction);
    text.rotation = original_direction.y.atan2(original_direction.x);
    text.apply_mirror_with_options(
        &mirror,
        MirrorOptions {
            mirror_text: true,
            ..MirrorOptions::default()
        },
    );
    close(text.dwg_x_direction.unwrap(), reflected_direction);
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let handle = document
        .add_entity(EntityType::MText(Box::new(text)))
        .unwrap();
    let bytes = crate::DwgWriter::write_to_vec(&document).unwrap();
    let restored = crate::DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .unwrap();
    let EntityType::MText(text) = restored.get_entity(handle).unwrap() else {
        unreachable!()
    };
    close(text.dwg_x_direction.unwrap(), reflected_direction);
}

#[test]
fn mirrored_dwg_entities_persist_after_raw_record_invalidation() {
    use std::io::Cursor;
    for version in [DxfVersion::AC1015, DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut source = CadDocument::with_version(version);
        let entities = [
            EntityType::Line(Line::from_points(
                Vector3::new(2.0, 3.0, 4.0),
                Vector3::new(5.0, 7.0, 9.0),
            )),
            EntityType::Arc(Arc::from_center_radius_angles(
                Vector3::new(3.0, 4.0, 5.0),
                2.0,
                0.3,
                2.1,
            )),
            EntityType::Text(Text::with_value("Persist", Vector3::new(2.0, 4.0, 0.0))),
            EntityType::MText(Box::new(MText::with_value(
                "First\\PSecond",
                Vector3::new(6.0, 8.0, 1.0),
            ))),
            EntityType::Solid(Solid::triangle(
                Vector3::new(1.0, 2.0, 0.0),
                Vector3::new(3.0, 2.0, 0.0),
                Vector3::new(1.0, 5.0, 0.0),
            )),
        ];
        let handles: Vec<_> = entities
            .into_iter()
            .map(|entity| source.add_entity(entity).unwrap())
            .collect();
        let bytes = crate::DwgWriter::write_to_vec(&source).unwrap();
        let mut document = crate::DwgReader::from_stream(Cursor::new(bytes))
            .read()
            .unwrap();
        let mut expected = Vec::new();
        for handle in &handles {
            let source = document.get_entity(*handle).unwrap();
            assert!(source.common().raw_record.is_some(), "{version:?}");
            let mut entity = source.clone();
            entity.mirror_with_options(
                Vector3::ZERO,
                Vector3::UNIT_Y,
                MirrorOptions {
                    mirror_text: true,
                    ..MirrorOptions::default()
                },
            );
            assert!(entity.common().raw_record.is_none());
            expected.push(canonical_entity_pairs(entity.clone()));
            *document.get_entity_mut(*handle).unwrap() = entity;
        }
        let bytes = crate::DwgWriter::write_to_vec(&document).unwrap();
        let restored = crate::DwgReader::from_stream(Cursor::new(bytes))
            .read()
            .unwrap();
        for (handle, expected) in handles.iter().zip(expected) {
            let actual = canonical_entity_pairs(restored.get_entity(*handle).unwrap().clone());
            assert_eq!(actual, expected, "{version:?}, {handle:?}");
        }
    }
}

#[test]
fn flattening_a_mirrored_native_triangle_preserves_visible_edges() {
    for bits in 0..16 {
        let mut entity = EntityType::Face3D(
            Face3D::triangle(Vector3::ZERO, Vector3::UNIT_X, Vector3::UNIT_Y)
                .with_invisible_edges(InvisibleEdgeFlags::from_bits(bits)),
        );
        let expected: Vec<_> = entity
            .flatten()
            .into_iter()
            .filter_map(|part| {
                if let EntityType::Line(line) = part {
                    Some((
                        Vector3::new(-line.start.x, line.start.y, line.start.z),
                        Vector3::new(-line.end.x, line.end.y, line.end.z),
                    ))
                } else {
                    None
                }
            })
            .collect();
        entity.mirror_x();
        let actual: Vec<_> = entity
            .flatten()
            .into_iter()
            .filter_map(|part| {
                if let EntityType::Line(line) = part {
                    Some((line.start, line.end))
                } else {
                    None
                }
            })
            .collect();
        assert_same_edges(&actual, &expected);
    }
}

mod polylines_regressions {
    use super::*;

    #[test]
    fn mirror_mline_matches_native_scale_and_preserves_segment_parameters() {
        let mut mline = MLine::from_points(&[
            Vector3::new(1.0, 2.0, 0.0),
            Vector3::new(10.0, 8.0, 0.0),
            Vector3::new(20.0, 4.0, 0.0),
        ]);
        mline.scale_factor = 20.0;
        mline.vertices[0].segments = vec![MLineSegment {
            parameters: vec![-20.0, 0.0],
            area_fill_parameters: vec![1.0, 3.0],
        }];
        let original = mline.clone();
        mirror_mline(&mut mline, &Transform::from_mirror_x());
        assert_eq!(mline.scale_factor, -20.0);
        for (actual, source) in mline.vertices.iter().zip(&original.vertices) {
            assert_eq!(
                actual.position,
                Vector3::new(-source.position.x, source.position.y, source.position.z)
            );
            assert_eq!(actual.segments, source.segments);
        }
        mirror_mline(&mut mline, &Transform::from_mirror_x());
        assert_eq!(mline.scale_factor, original.scale_factor);
        for (actual, source) in mline.vertices.iter().zip(&original.vertices) {
            assert!((actual.position - source.position).length() < 1e-12);
            assert!((actual.direction - source.direction).length() < 1e-12);
            assert!((actual.miter - source.miter).length() < 1e-12);
            assert_eq!(actual.segments, source.segments);
        }
    }

    #[test]
    fn nonreflecting_transform_does_not_negate_mline_scale() {
        let mut mline = MLine::new();
        mline.scale_factor = -2.0;
        mirror_mline(&mut mline, &Transform::from_translation(Vector3::UNIT_X));
        assert_eq!(mline.scale_factor, -2.0);
    }
}

mod leaders_regressions {
    use super::*;

    #[test]
    fn mirror_leader_distinguishes_offset_vectors_from_world_points() {
        let mut leader = Leader::new();
        leader.vertices = vec![Vector3::new(1.0, 2.0, 3.0), Vector3::new(5.0, 6.0, 7.0)];
        leader.origin = Vector3::new(3.0, -4.0, 5.0);
        leader.block_offset = Vector3::new(2.0, -1.0, 4.0);
        leader.annotation_offset = Vector3::new(-3.0, 5.0, 1.0);
        let original = leader.clone();
        let mirror = Transform::from_mirror_plane(
            Vector3::new(10.0, 20.0, 30.0),
            Vector3::new(1.0, 2.0, 3.0),
        );
        let mut dispatched = EntityType::Leader(leader.clone());
        dispatched.apply_mirror(&mirror);
        leader.apply_mirror(&mirror);
        assert_eq!(dispatched, EntityType::Leader(leader.clone()));
        assert!((leader.origin - mirror.apply(original.origin)).length() < 1e-12);
        assert!(
            (leader.block_offset - mirror.apply_rotation(original.block_offset)).length() < 1e-12
        );
        assert!(
            (leader.annotation_offset - mirror.apply_rotation(original.annotation_offset)).length()
                < 1e-12
        );
        for (actual, original) in leader.vertices.iter().zip(&original.vertices) {
            assert!((*actual - mirror.apply(*original)).length() < 1e-12);
        }
        leader.apply_mirror(&mirror);
        assert!((leader.origin - original.origin).length() < 1e-12);
        assert!((leader.block_offset - original.block_offset).length() < 1e-12);
        assert!((leader.annotation_offset - original.annotation_offset).length() < 1e-12);
    }
}

mod hatch_regressions {
    use super::*;
    use crate::types::Vector2;

    fn patterned_hatch() -> Hatch {
        let mut pattern = HatchPattern::new("ANSI31");
        pattern.add_line(HatchPatternLine {
            angle: std::f64::consts::FRAC_PI_4 + 0.2,
            base_point: Vector2::new(3.0, 4.0),
            offset: Vector2::new(-2.64634, 1.75429),
            dash_lengths: vec![1.0, -0.5],
        });
        let mut hatch = Hatch::with_pattern(pattern);
        hatch.pattern_angle = 0.2;
        hatch.record_pattern_origin(Vector2::new(3.0, 4.0));
        hatch.gradient_color.enabled = true;
        hatch.gradient_color.angle = 0.3;
        let mut path = BoundaryPath::new();
        path.edges.push(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(1.0, 2.0),
            end: Vector2::new(10.0, 8.0),
        }));
        hatch.add_path(path);
        hatch
    }

    #[test]
    fn mirrhatch_off_preserves_pattern_and_origin_while_moving_boundaries() {
        let mut hatch = patterned_hatch();
        let pattern = hatch.pattern.clone();
        let xdata = hatch.common.extended_data.clone();
        mirror_hatch(&mut hatch, &Transform::from_mirror_x());
        assert_eq!(hatch.pattern, pattern);
        assert_eq!(hatch.pattern_angle, 0.2);
        assert_eq!(hatch.gradient_color.angle, 0.3);
        assert_eq!(hatch.common.extended_data, xdata);
        let BoundaryEdge::Line(edge) = &hatch.paths[0].edges[0] else {
            panic!("Expected line edge");
        };
        assert_eq!(edge.start, Vector2::new(-1.0, 2.0));
        assert_eq!(edge.end, Vector2::new(-10.0, 8.0));
    }

    #[test]
    fn mirrhatch_on_reflects_line_families_and_preserves_authored_angle() {
        let mut hatch = patterned_hatch();
        mirror_hatch_with_options(
            &mut hatch,
            &Transform::from_mirror_x(),
            MirrorOptions {
                mirror_hatch: true,
                ..MirrorOptions::default()
            },
        );
        let line = &hatch.pattern.lines[0];
        assert!(
            (line.angle - (std::f64::consts::PI - std::f64::consts::FRAC_PI_4 - 0.2)).abs() < 1e-12
        );
        assert_eq!(line.base_point, Vector2::new(-3.0, 4.0));
        assert_eq!(line.offset, Vector2::new(2.64634, 1.75429));
        assert_eq!(line.dash_lengths, vec![1.0, -0.5]);
        assert_eq!(hatch.pattern_angle, 0.2);
        assert_eq!(hatch.pattern_origin(), Vector2::new(-3.0, 4.0));
        assert!((hatch.gradient_color.angle - (std::f64::consts::PI - 0.3)).abs() < 1e-12);
    }
}

mod blocks_regressions {
    use super::*;
    use crate::types::Vector2;

    #[test]
    fn mirror_insert_preserves_signed_array_positions_in_world() {
        for normal in [Vector3::UNIT_Z, Vector3::new(1.0, 2.0, 3.0).normalize()] {
            for column_spacing in [-7.0, 7.0] {
                for row_spacing in [-5.0, 5.0] {
                    let mut insert = Insert::new("ARRAY", Vector3::new(2.0, 3.0, 4.0));
                    insert.normal = normal;
                    insert.rotation = 0.43;
                    insert.column_count = 3;
                    insert.row_count = 2;
                    insert.column_spacing = column_spacing;
                    insert.row_spacing = row_spacing;
                    insert.set_x_scale(-2.0);
                    insert.set_y_scale(3.0);
                    let mirror = Transform::from_mirror_plane(
                        Vector3::new(3.0, 1.0, -2.0),
                        Vector3::new(2.0, -1.0, 3.0),
                    );
                    let old_ocs = Matrix3::arbitrary_axis(insert.normal);
                    let old_transform = insert.get_transform();
                    let expected: Vec<_> = insert
                        .array_points()
                        .into_iter()
                        .map(|point| mirror.apply(old_ocs * point))
                        .collect();
                    let mut dispatched = EntityType::Insert(insert.clone());
                    dispatched.apply_mirror(&mirror);
                    insert.apply_mirror(&mirror);
                    assert_eq!(dispatched, EntityType::Insert(insert.clone()));
                    let new_ocs = Matrix3::arbitrary_axis(insert.normal);
                    for (actual, expected) in insert.array_points().into_iter().zip(expected) {
                        assert!((new_ocs * actual - expected).length() < 1e-10);
                    }
                    for point in [
                        Vector3::ZERO,
                        Vector3::UNIT_X,
                        Vector3::UNIT_Y,
                        Vector3::UNIT_Z,
                    ] {
                        assert!(
                            (insert.get_transform().apply(point)
                                - mirror.apply(old_transform.apply(point)))
                            .length()
                                < 1e-10
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn mirror_underlay_reflects_world_clip_geometry_and_is_an_involution() {
        for normal in [Vector3::UNIT_Z, Vector3::new(1.0, -2.0, 3.0).normalize()] {
            let mut underlay = Underlay::pdf();
            underlay.insertion_point = Vector3::new(2.0, 3.0, 4.0);
            underlay.rotation = 0.37;
            underlay.normal = normal;
            underlay.x_scale = -2.0;
            underlay.y_scale = 3.0;
            underlay.clip_boundary_vertices = vec![
                Vector2::new(-1.0, 2.0),
                Vector2::new(4.0, -3.0),
                Vector2::new(5.0, 6.0),
            ];
            let original = underlay.world_clip_boundary();
            let mirror = Transform::from_mirror_plane(
                Vector3::new(-2.0, 1.0, 3.0),
                Vector3::new(1.0, 3.0, -2.0),
            );
            let mut dispatched = EntityType::Underlay(underlay.clone());
            dispatched.apply_mirror(&mirror);
            underlay.apply_mirror(&mirror);
            assert_eq!(dispatched, EntityType::Underlay(underlay.clone()));
            for (actual, source) in underlay.world_clip_boundary().into_iter().zip(&original) {
                assert!((actual - mirror.apply(*source)).length() < 1e-10);
            }
            underlay.apply_mirror(&mirror);
            for (actual, expected) in underlay.world_clip_boundary().into_iter().zip(original) {
                assert!((actual - expected).length() < 1e-10);
            }
        }
    }
}

#[test]
fn test_mirror_x_line() {
    let mut entity = EntityType::Line(Line::from_points(
        Vector3::new(1.0, 2.0, 0.0),
        Vector3::new(3.0, 4.0, 0.0),
    ));
    entity.mirror_x();
    if let EntityType::Line(line) = &entity {
        assert!((line.start.x - (-1.0)).abs() < 1e-10);
        assert!((line.start.y - 2.0).abs() < 1e-10);
        assert!((line.end.x - (-3.0)).abs() < 1e-10);
        assert!((line.end.y - 4.0).abs() < 1e-10);
    } else {
        panic!("Expected Line");
    }
}

#[test]
fn mirror_solid_preserves_storage_order() {
    let mut solid = Solid::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(1.0, 1.0, 0.0),
    );
    let original_second = solid.second_corner;
    let original_fourth = solid.fourth_corner;
    mirror_solid(&mut solid, &Transform::from_mirror_x());
    assert_eq!(
        solid.second_corner,
        Vector3::new(-original_second.x, original_second.y, original_second.z)
    );
    assert_eq!(
        solid.fourth_corner,
        Vector3::new(-original_fourth.x, original_fourth.y, original_fourth.z)
    );
}

#[test]
fn test_mirror_lwpolyline_negates_bulge() {
    let mut lw = LwPolyline::new();
    lw.vertices.push(LwVertex {
        location: crate::types::Vector2::new(0.0, 0.0),
        bulge: 0.5,
        start_width: 0.0,
        end_width: 0.0,
        vertex_id: 0,
    });
    lw.vertices.push(LwVertex {
        location: crate::types::Vector2::new(10.0, 0.0),
        bulge: -0.3,
        start_width: 0.0,
        end_width: 0.0,
        vertex_id: 0,
    });
    mirror_lwpolyline(&mut lw, &Transform::from_mirror_x());
    assert!((lw.vertices[0].bulge - (-0.5)).abs() < 1e-10);
    assert!((lw.vertices[1].bulge - 0.3).abs() < 1e-10);
}

use crate::types::{DxfVersion, Matrix3, Vector2};
use std::f64::consts::TAU;

fn close(actual: Vector3, expected: Vector3) {
    assert!(
        (actual - expected).length() < 1e-9,
        "actual {actual:?}, expected {expected:?}"
    );
}

fn reflect(point: Vector3, origin: Vector3, normal: Vector3) -> Vector3 {
    let normal = normal.normalize();
    point - normal * (2.0 * (point - origin).dot(&normal))
}

fn same_edge(actual: (Vector3, Vector3), expected: (Vector3, Vector3)) -> bool {
    let near = |a: Vector3, b: Vector3| (a - b).length() < 1e-9;
    (near(actual.0, expected.0) && near(actual.1, expected.1))
        || (near(actual.0, expected.1) && near(actual.1, expected.0))
}

fn edges(corners: &[Vector3]) -> Vec<(Vector3, Vector3)> {
    (0..corners.len())
        .map(|index| (corners[index], corners[(index + 1) % corners.len()]))
        .collect()
}

fn assert_same_edges(actual: &[(Vector3, Vector3)], expected: &[(Vector3, Vector3)]) {
    assert_eq!(actual.len(), expected.len());
    for &edge in expected {
        assert!(
            actual.iter().any(|&candidate| same_edge(candidate, edge)),
            "missing edge {edge:?} in {actual:?}"
        );
    }
}

fn face_visible_edges(face: &Face3D) -> Vec<(Vector3, Vector3)> {
    edges(&[
        face.first_corner,
        face.second_corner,
        face.third_corner,
        face.fourth_corner,
    ])
    .into_iter()
    .enumerate()
    .filter(|(index, (a, b))| {
        face.invisible_edges.bits() & (1 << index) == 0 && (*a - *b).length() > 1e-10
    })
    .map(|(_, edge)| edge)
    .collect()
}

#[test]
fn arbitrary_mirror_plane_has_expected_distance_and_is_an_involution() {
    let origin = Vector3::new(4.0, -3.0, 2.0);
    let normal = Vector3::new(2.0, -5.0, 3.0);
    let original = Line::from_points(Vector3::new(-2.0, 7.0, 4.0), Vector3::new(11.0, -6.0, 3.0));
    let mut line = original.clone();
    line.mirror_about_plane(origin, normal);
    close(line.start, reflect(original.start, origin, normal));
    close(line.end, reflect(original.end, origin, normal));
    assert!(
        ((line.end - line.start).length() - (original.end - original.start).length()).abs() < 1e-9
    );
    line.mirror_about_plane(origin, normal);
    close(line.start, original.start);
    close(line.end, original.end);
}

#[test]
fn mirror_line_uses_xy_direction_and_preserves_height() {
    let first = Vector3::new(4.0, -2.0, 11.0);
    let second = Vector3::new(7.0, 2.0, -7.0);
    let original = Vector3::new(8.0, 1.0, 5.0);
    let mut line = Line::from_points(original, first);
    line.mirror_about_line(first, second);
    close(
        line.start,
        reflect(original, first, Vector3::new(-4.0, 3.0, 0.0)),
    );
    assert_eq!(line.start.z, original.z);
    close(line.end, first);
}

#[test]
fn arc_mirror_preserves_reflected_sweep_for_every_normal() {
    let origin = Vector3::new(3.0, -4.0, 2.0);
    let normal = Vector3::new(1.0, 3.0, -2.0);
    for arc_normal in [
        Vector3::UNIT_Z,
        -Vector3::UNIT_Z,
        Vector3::new(2.0, -1.0, 4.0).normalize(),
    ] {
        for (start, sweep) in [(0.3, 1.7), (5.9, 0.8), (0.2, 4.7)] {
            let mut arc = Arc::from_center_radius_angles(
                Vector3::new(2.0, -5.0, 7.0),
                3.0,
                start,
                start + sweep,
            );
            arc.normal = arc_normal;
            let original = arc.clone();
            arc.mirror_about_plane(origin, normal);
            close(
                arc.center_wcs(),
                reflect(original.center_wcs(), origin, normal),
            );
            assert!((arc.sweep_angle() - sweep).abs() < 1e-9);
            for fraction in [0.0, 0.2, 0.5, 0.8, 1.0] {
                let actual = arc.point_at_angle_wcs(arc.start_angle + arc.sweep_angle() * fraction);
                let old =
                    original.point_at_angle_wcs(original.start_angle + sweep * (1.0 - fraction));
                close(actual, reflect(old, origin, normal));
            }
        }
    }
}

fn ellipse_point(ellipse: &Ellipse, parameter: f64) -> Vector3 {
    ellipse.center
        + ellipse.major_axis * parameter.cos()
        + ellipse.normal.normalize().cross(&ellipse.major_axis)
            * (ellipse.minor_axis_ratio * parameter.sin())
}

#[test]
fn elliptical_arc_mirror_preserves_every_sample_of_the_curve() {
    let origin = Vector3::new(2.0, 3.0, -1.0);
    let normal = Vector3::new(4.0, -1.0, 2.0);
    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(4.0, -2.0, 5.0),
        Vector3::new(3.0, 4.0, 0.0),
        0.3,
    );
    ellipse.start_parameter = 5.5;
    ellipse.end_parameter = 7.4;
    let original = ellipse.clone();
    ellipse.mirror_about_plane(origin, normal);
    for fraction in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let actual = ellipse_point(&ellipse, ellipse.start_parameter + 1.9 * fraction);
        let old = ellipse_point(&original, original.start_parameter + 1.9 * fraction);
        close(actual, reflect(old, origin, normal));
    }
}

#[test]
fn mirrored_lwpolyline_preserves_ocs_vertex_positions_and_bulge_curve() {
    let mut polyline = LwPolyline::new();
    polyline.normal = Vector3::new(1.0, 2.0, 3.0).normalize();
    polyline.elevation = 5.0;
    polyline.vertices = vec![
        LwVertex::with_bulge(Vector2::new(2.0, 3.0), 0.7),
        LwVertex::with_bulge(Vector2::new(7.0, -1.0), -0.4),
    ];
    let original = polyline.clone();
    let origin = Vector3::new(3.0, -4.0, 7.0);
    let normal = Vector3::new(2.0, -1.0, 4.0);
    polyline.mirror_about_plane(origin, normal);
    let old_basis = Matrix3::arbitrary_axis(original.normal);
    let new_basis = Matrix3::arbitrary_axis(polyline.normal);
    for (old, new) in original.vertices.iter().zip(&polyline.vertices) {
        let old_world =
            old_basis * Vector3::new(old.location.x, old.location.y, original.elevation);
        let new_world =
            new_basis * Vector3::new(new.location.x, new.location.y, polyline.elevation);
        close(new_world, reflect(old_world, origin, normal));
        assert!((new.bulge + old.bulge).abs() < 1e-9);
    }
}

#[test]
fn mirrored_solid_keeps_the_reflected_boundary() {
    let mut solid = Solid::new(
        Vector3::new(1.0, 2.0, 0.0),
        Vector3::new(5.0, 2.0, 0.0),
        Vector3::new(1.0, 4.0, 0.0),
        Vector3::new(5.0, 4.0, 0.0),
    );
    let expected = edges(&solid.boundary_corners())
        .into_iter()
        .map(|(a, b)| (Vector3::new(-a.x, a.y, a.z), Vector3::new(-b.x, b.y, b.z)))
        .collect::<Vec<_>>();
    solid.mirror_x();
    assert_eq!(solid.first_corner, Vector3::new(-1.0, 2.0, 0.0));
    assert_eq!(solid.second_corner, Vector3::new(-5.0, 2.0, 0.0));
    assert_eq!(solid.third_corner, Vector3::new(-1.0, 4.0, 0.0));
    assert_eq!(solid.fourth_corner, Vector3::new(-5.0, 4.0, 0.0));
    assert_same_edges(&edges(&solid.boundary_corners()), &expected);
}

#[test]
fn mirrored_triangular_solid_stays_a_triangle() {
    let mut solid = Solid::triangle(Vector3::ZERO, Vector3::UNIT_X, Vector3::UNIT_Y);
    solid.mirror_x();
    assert!(solid.is_triangle());
    assert_eq!(solid.boundary_corners().len(), 3);
}

#[test]
fn mirrored_triangular_face_stays_a_triangle() {
    let mut face = Face3D::triangle(Vector3::ZERO, Vector3::UNIT_X, Vector3::UNIT_Y);
    face.mirror_x();
    assert_eq!(face.first_corner, Vector3::UNIT_Y);
    assert_eq!(face.second_corner, face.first_corner);
    assert_eq!(face.third_corner, -Vector3::UNIT_X);
    assert_eq!(face.fourth_corner, Vector3::ZERO);
    assert!(face.is_triangle());
    assert_eq!(face.corners().len(), 3);
    assert!((face.area() - 0.5).abs() < 1e-9);
}

#[test]
fn face_mirror_preserves_the_geometric_edges_marked_invisible() {
    for (bits, triangle) in (0..16).flat_map(|bits| [(bits, true), (bits, false)]) {
        let mut face = Face3D::new(
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(5.0, 2.0, 3.0),
            Vector3::new(5.0, 4.0, 3.0),
            Vector3::new(1.0, 4.0, 3.0),
        )
        .with_invisible_edges(InvisibleEdgeFlags::from_bits(bits));
        if triangle {
            face.fourth_corner = face.third_corner;
        }
        let expected = face_visible_edges(&face)
            .into_iter()
            .map(|(a, b)| (Vector3::new(-a.x, a.y, a.z), Vector3::new(-b.x, b.y, b.z)))
            .collect::<Vec<_>>();
        face.mirror_x();
        assert_eq!(
            face.invisible_edges.bits(),
            (bits & 10) | ((bits & 1) << 2) | ((bits & 4) >> 2)
        );
        assert_same_edges(&face_visible_edges(&face), &expected);
    }
}

#[test]
fn face_area_and_corners_accept_every_adjacent_duplicate() {
    let a = Vector3::new(1.0, 2.0, 3.0);
    let b = Vector3::new(5.0, 2.0, 3.0);
    let c = Vector3::new(5.0, 6.0, 3.0);
    for corners in [[a, a, b, c], [a, b, b, c], [a, b, c, c], [a, b, c, a]] {
        let face = Face3D::new(corners[0], corners[1], corners[2], corners[3]);
        assert!(face.is_triangle());
        assert_eq!(face.corners(), [a, b, c]);
        assert_eq!(face.area(), 8.0);
    }
}

#[test]
fn face_apply_mirror_reorders_only_for_reflections() {
    let mut face = Face3D::new(
        Vector3::ZERO,
        Vector3::UNIT_X,
        Vector3::UNIT_X + Vector3::UNIT_Y,
        Vector3::UNIT_Y,
    )
    .with_invisible_edges(InvisibleEdgeFlags::FIRST);
    face.apply_mirror(&Transform::from_translation(Vector3::new(4.0, 2.0, 0.0)));
    assert_eq!(face.first_corner, Vector3::new(4.0, 2.0, 0.0));
    assert_eq!(face.second_corner, Vector3::new(5.0, 2.0, 0.0));
    assert_eq!(face.invisible_edges, InvisibleEdgeFlags::FIRST);
}

#[test]
fn mirrored_mesh_keeps_native_face_and_crease_indices() {
    let mut mesh = Mesh::from_quads(
        vec![
            Vector3::new(1.0, 2.0, 0.0),
            Vector3::new(10.0, 2.0, 0.0),
            Vector3::new(10.0, 12.0, 0.0),
            Vector3::new(1.0, 12.0, 0.0),
        ],
        &[(0, 1, 2, 3)],
    );
    mesh.edges[0].crease = Some(2.5);
    let faces = mesh.faces.clone();
    let edges = mesh.edges.clone();
    mesh.mirror_x();
    assert_eq!(mesh.faces, faces);
    assert_eq!(mesh.edges, edges);
    assert_eq!(mesh.vertices[0], Vector3::new(-1.0, 2.0, 0.0));
}

#[test]
fn elliptical_arc_native_parameters_follow_the_reflected_basis() {
    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(2.0, 3.0, 5.0),
        Vector3::new(4.0, 0.0, 0.0),
        0.5,
    );
    ellipse.start_parameter = 0.3;
    ellipse.end_parameter = 2.4;
    ellipse.mirror_x();
    close(ellipse.center, Vector3::new(-2.0, 3.0, 5.0));
    close(ellipse.major_axis, Vector3::new(-4.0, 0.0, 0.0));
    close(ellipse.normal, -Vector3::UNIT_Z);
    assert!((ellipse.start_parameter - 0.3).abs() < 1e-12);
    assert!((ellipse.end_parameter - 2.4).abs() < 1e-12);
}

#[test]
fn polyface_native_indices_keep_the_signs_on_their_geometric_edges() {
    for (mut face, expected) in [
        (PolyfaceFace::triangle(-1, 2, -3), [3, -2, -1, 0]),
        (PolyfaceFace::quad(-1, 2, -3, 4), [-4, 3, -2, 1]),
    ] {
        let original = face.clone();
        face.reverse();
        assert_eq!(
            [face.index1, face.index2, face.index3, face.index4],
            expected
        );
        face.reverse();
        assert_eq!(face, original);
    }
}

fn polyface_visible_edges(mesh: &PolyfaceMesh) -> Vec<(Vector3, Vector3)> {
    let face = &mesh.faces[0];
    let indices = [face.index1, face.index2, face.index3, face.index4];
    let count = face.vertex_count();
    (0..count)
        .filter(|&index| indices[index] > 0)
        .map(|index| {
            let a = mesh.vertices[indices[index].unsigned_abs() as usize - 1].location;
            let b =
                mesh.vertices[indices[(index + 1) % count].unsigned_abs() as usize - 1].location;
            (a, b)
        })
        .collect()
}

#[test]
fn polyface_mirror_keeps_invisibility_on_the_same_geometric_edges() {
    for triangle in [true, false] {
        let mut mesh = PolyfaceMesh::new();
        mesh.vertices = [
            Vector3::ZERO,
            Vector3::UNIT_X,
            Vector3::UNIT_X + Vector3::UNIT_Y,
            Vector3::UNIT_Y,
        ]
        .into_iter()
        .map(PolyfaceVertex::new)
        .collect();
        mesh.faces.push(if triangle {
            PolyfaceFace::triangle(-1, 2, 3)
        } else {
            PolyfaceFace::quad(-1, 2, 3, 4)
        });
        let expected = polyface_visible_edges(&mesh)
            .into_iter()
            .map(|(a, b)| (Vector3::new(-a.x, a.y, a.z), Vector3::new(-b.x, b.y, b.z)))
            .collect::<Vec<_>>();
        mesh.mirror_x();
        assert_same_edges(&polyface_visible_edges(&mesh), &expected);
    }
}

#[test]
fn mirror_invalidates_verbatim_dwg_geometry_records() {
    let raw = RawRecord {
        data: vec![1, 2, 3],
        handle_bits: 0,
        version: DxfVersion::AC1027,
    };
    let mut entities = vec![
        EntityType::Arc(Arc::new()),
        EntityType::Ellipse(Ellipse::new()),
        EntityType::LwPolyline(LwPolyline::new()),
        EntityType::Text(Text::new()),
        EntityType::MText(Box::new(MText::new())),
        EntityType::Hatch(Box::new(Hatch::new())),
        EntityType::Face3D(Face3D::triangle(
            Vector3::ZERO,
            Vector3::UNIT_X,
            Vector3::UNIT_Y,
        )),
        EntityType::Solid(Solid::triangle(
            Vector3::ZERO,
            Vector3::UNIT_X,
            Vector3::UNIT_Y,
        )),
        EntityType::Mesh(Mesh::new()),
        EntityType::PolyfaceMesh(PolyfaceMesh::new()),
        EntityType::Shape(Shape::new()),
    ];
    for entity in &mut entities {
        entity.common_mut().raw_record = Some(raw.clone().into());
        entity.mirror_x();
        assert!(
            entity.common().raw_record.is_none(),
            "{} kept stale geometry",
            entity.as_entity().entity_type()
        );
    }
}

#[test]
fn shape_axis_mirror_methods_match_entity_dispatch() {
    for mirror_x in [true, false] {
        let mut shape = Shape::with_name(Vector3::new(3.0, 4.0, 5.0), "BOX", 2.0);
        shape.rotation = 0.4;
        let mut entity = EntityType::Shape(shape.clone());
        if mirror_x {
            shape.mirror_x();
            entity.mirror_x();
        } else {
            shape.mirror_y();
            entity.mirror_y();
        }
        let EntityType::Shape(expected) = entity else {
            unreachable!()
        };
        close(shape.insertion_point, expected.insertion_point);
        assert!((shape.rotation - expected.rotation).abs() < 1e-9);
        assert!((shape.relative_x_scale - expected.relative_x_scale).abs() < 1e-9);
    }
}

#[test]
fn invalid_mirror_axes_leave_entities_unchanged() {
    let original = EntityType::Solid(Solid::triangle(
        Vector3::ZERO,
        Vector3::UNIT_X,
        Vector3::UNIT_Y,
    ));
    let mut entity = original.clone();
    entity.mirror_about_plane(Vector3::ZERO, Vector3::ZERO);
    assert_eq!(entity, original);
    entity.mirror_about_line(Vector3::ZERO, Vector3::new(0.0, 0.0, 4.0));
    assert_eq!(entity, original);
    entity.mirror_about_plane(Vector3::ZERO, Vector3::new(f64::NAN, 0.0, 0.0));
    assert_eq!(entity, original);
}

#[test]
fn insert_mirror_composes_world_placement_without_changing_block_contents() {
    let origin = Vector3::new(4.0, -3.0, 2.0);
    let normal = Vector3::new(2.0, -1.0, 3.0);
    for block_normal in [Vector3::UNIT_Z, Vector3::new(1.0, 2.0, 4.0).normalize()] {
        let mut insert =
            Insert::new("BLOCK", Vector3::new(2.0, 3.0, 4.0)).with_scale(-2.0, 3.0, 0.5);
        insert.rotation = 0.7;
        insert.normal = block_normal;
        let old = insert.get_transform();
        insert.mirror_about_plane(origin, normal);
        let new = insert.get_transform();
        for point in [
            Vector3::ZERO,
            Vector3::UNIT_X,
            Vector3::UNIT_Y,
            Vector3::UNIT_Z,
            Vector3::new(2.0, -1.0, 3.0),
        ] {
            close(new.apply(point), reflect(old.apply(point), origin, normal));
        }
        assert_eq!(insert.block_name, "BLOCK");
    }
}

#[test]
fn minsert_mirror_preserves_every_array_cell_location() {
    for (column_spacing, row_spacing) in [(10.0, 20.0), (-10.0, 20.0), (10.0, -20.0)] {
        let mut insert = Insert::new("BLOCK", Vector3::new(4.0, 3.0, 2.0)).with_array(
            3,
            2,
            column_spacing,
            row_spacing,
        );
        insert.rotation = 0.6;
        let expected = insert
            .array_points()
            .into_iter()
            .map(|point| Vector3::new(-point.x, point.y, point.z))
            .collect::<Vec<_>>();
        insert.mirror_x();
        for (actual, expected) in insert.array_points().into_iter().zip(expected) {
            close(actual, expected);
        }
    }
}

#[test]
fn mirror_plane_transforms_are_scale_independent() {
    let origin = Vector3::new(1.0, -2.0, 3.0);
    let normal = Vector3::new(3.0, 4.0, -5.0);
    let point = Vector3::new(-8.0, 7.0, 4.0);
    let expected = reflect(point, origin, normal);
    for scale in [1e-6, 0.5, 2.0, 1e6, -2.0] {
        close(
            Transform::from_mirror_plane(origin, normal * scale).apply(point),
            expected,
        );
    }
    let mut ellipse = Ellipse::new();
    ellipse.mirror_about_plane(origin, normal);
    assert!((ellipse.end_parameter - ellipse.start_parameter - TAU).abs() < 1e-9);
}

use crate::types::Handle;
use crate::CadDocument;

fn policy_entities() -> Vec<EntityType> {
    let position = Vector3::new(5.0, 2.0, 3.0);
    let text = Text::with_value("Label", position);
    let mtext = MText::with_value("First\\PSecond", position);
    let mut attribute = AttributeEntity::simple("LABEL", "Attribute");
    attribute.insertion_point = position;
    let mut definition = AttributeDefinition::simple("LABEL");
    definition.default_value = "Definition".to_owned();
    definition.insertion_point = position;
    let mut insert = Insert::new("LABEL_BLOCK", position);
    insert.attributes.push(attribute.clone());
    let mut pattern = HatchPattern::new("ANSI31");
    pattern.add_line(HatchPatternLine {
        angle: 0.3,
        base_point: Vector2::new(1.0, 2.0),
        offset: Vector2::new(-1.0, 3.0),
        dash_lengths: vec![2.0, -1.0],
    });
    vec![
        EntityType::Text(text),
        EntityType::MText(Box::new(mtext)),
        EntityType::AttributeEntity(attribute),
        EntityType::AttributeDefinition(definition),
        EntityType::Insert(insert),
        EntityType::Hatch(Box::new(Hatch::with_pattern(pattern))),
    ]
}

#[test]
fn trait_and_enum_apply_the_same_text_and_hatch_options() {
    let first = Vector3::new(1.0, -2.0, 9.0);
    let second = Vector3::new(1.0, 3.0, -4.0);
    for options in [
        MirrorOptions::default(),
        MirrorOptions {
            mirror_text: true,
            mirror_hatch: true,
            text_width: Some(9.0),
        },
    ] {
        for source in policy_entities() {
            let transform = Transform::from_mirror_x();
            let mut from_enum = source.clone();
            let mut from_trait = source.clone();
            from_enum.apply_mirror(&transform);
            from_trait.as_entity_mut().apply_mirror(&transform);
            assert_eq!(
                from_trait,
                from_enum,
                "{}",
                source.as_entity().entity_type()
            );

            let mut from_enum = source.clone();
            let mut from_trait = source.clone();
            from_enum.mirror_with_options(first, second, options);
            from_trait
                .as_entity_mut()
                .mirror_with_options(first, second, options);
            assert_eq!(
                from_trait,
                from_enum,
                "{}",
                source.as_entity().entity_type()
            );

            let plane = MirrorPlane::YZ {
                point: Vector3::new(1.0, 0.0, 0.0),
            };
            let mut from_enum = source.clone();
            let mut from_trait = source;
            from_enum.mirror3d_with_options(plane, options);
            from_trait
                .as_entity_mut()
                .mirror3d_with_options(plane, options);
            assert_eq!(from_trait, from_enum);
        }
    }
}

#[test]
fn trait_text_options_control_glyphs_and_exact_readable_width() {
    let source = Text::with_value("Label", Vector3::new(5.0, 2.0, 3.0));
    let mut readable = source.clone();
    readable.apply_mirror_with_options(
        &Transform::from_mirror_x(),
        MirrorOptions {
            text_width: Some(9.0),
            ..MirrorOptions::default()
        },
    );
    assert!((readable.insertion_point.x - -14.0).abs() < 1e-12);
    assert_eq!(readable.generation_flags & 2, 0);

    let mut reflected = source;
    reflected.apply_mirror_with_options(
        &Transform::from_mirror_x(),
        MirrorOptions {
            mirror_text: true,
            ..MirrorOptions::default()
        },
    );
    assert_eq!(reflected.insertion_point, Vector3::new(-5.0, 2.0, 3.0));
    assert_eq!(reflected.generation_flags & 2, 2);
}

#[test]
fn document_mirror_uses_mirrtext_and_preserves_entity_identity() {
    for mirror_text in [false, true] {
        for spatial in [false, true] {
            let mut document = CadDocument::new();
            document.header.mirror_text = mirror_text;
            let handle = document
                .add_entity(EntityType::Text(Text::with_value(
                    "Label",
                    Vector3::new(5.0, 2.0, 3.0),
                )))
                .unwrap();
            let mut expected = document.get_entity(handle).unwrap().clone();
            let original_owner = expected.common().owner_handle;
            let original_count = document.entity_count();
            let options = MirrorOptions {
                mirror_text,
                ..MirrorOptions::default()
            };
            if spatial {
                let plane = MirrorPlane::YZ {
                    point: Vector3::ZERO,
                };
                expected.mirror3d_with_options(plane, options);
                assert!(document.mirror3d_entity(handle, plane));
            } else {
                expected.mirror_with_options(Vector3::ZERO, Vector3::UNIT_Y, options);
                assert!(document.mirror_entity(handle, Vector3::ZERO, Vector3::UNIT_Y));
            }
            assert_eq!(document.get_entity(handle), Some(&expected));
            assert_eq!(expected.common().handle, handle);
            assert_eq!(expected.common().owner_handle, original_owner);
            assert_eq!(document.entity_count(), original_count);
            let EntityType::Text(text) = expected else {
                panic!("expected TEXT")
            };
            assert_eq!(text.generation_flags & 2 != 0, mirror_text);
        }
    }
}

#[test]
fn document_explicit_options_override_header_and_accept_exact_width() {
    let mut document = CadDocument::new();
    document.header.mirror_text = true;
    let handle = document
        .add_entity(EntityType::Text(Text::with_value(
            "Label",
            Vector3::new(5.0, 2.0, 3.0),
        )))
        .unwrap();
    assert!(document.mirror_entity_with_options(
        handle,
        Vector3::ZERO,
        Vector3::UNIT_Y,
        MirrorOptions {
            text_width: Some(9.0),
            ..MirrorOptions::default()
        },
    ));
    let EntityType::Text(text) = document.get_entity(handle).unwrap() else {
        panic!("expected TEXT")
    };
    assert!((text.insertion_point.x - -14.0).abs() < 1e-12);
    assert_eq!(text.generation_flags & 2, 0);
}

#[test]
fn planar_mirror_and_mirror3d_have_distinct_height_behavior() {
    let source = Line::from_points(Vector3::new(2.0, 3.0, 7.0), Vector3::new(5.0, 6.0, 8.0));
    let mut planar = source.clone();
    planar.mirror(Vector3::new(0.0, 0.0, 50.0), Vector3::new(1.0, 0.0, -20.0));
    assert_eq!(planar.start, Vector3::new(2.0, -3.0, 7.0));
    let mut spatial = source.clone();
    spatial.mirror3d(MirrorPlane::XY {
        point: Vector3::new(0.0, 0.0, 2.0),
    });
    assert_eq!(spatial.start, Vector3::new(2.0, 3.0, -3.0));

    let mut dispatched = EntityType::Line(source.clone());
    dispatched.mirror3d(MirrorPlane::XY {
        point: Vector3::new(0.0, 0.0, 2.0),
    });
    assert_eq!(dispatched, EntityType::Line(spatial));
    let mut document = CadDocument::new();
    let handle = document.add_entity(EntityType::Line(source)).unwrap();
    *dispatched.common_mut() = document.get_entity(handle).unwrap().common().clone();
    assert!(document.mirror3d_entity(
        handle,
        MirrorPlane::XY {
            point: Vector3::new(0.0, 0.0, 2.0)
        }
    ));
    assert_eq!(document.get_entity(handle), Some(&dispatched));
}

#[test]
fn mirror3d_plane_selectors_resolve_the_same_world_planes() {
    let point = Vector3::new(1.0, 2.0, 3.0);
    for (plane, expected) in [
        (MirrorPlane::XY { point }, Vector3::new(5.0, 6.0, -1.0)),
        (MirrorPlane::YZ { point }, Vector3::new(-3.0, 6.0, 7.0)),
        (MirrorPlane::ZX { point }, Vector3::new(5.0, -2.0, 7.0)),
        (
            MirrorPlane::PointNormal {
                point,
                normal: Vector3::UNIT_Z,
            },
            Vector3::new(5.0, 6.0, -1.0),
        ),
        (
            MirrorPlane::ThreePoint {
                first: point,
                second: point + Vector3::UNIT_X,
                third: point + Vector3::UNIT_Y,
            },
            Vector3::new(5.0, 6.0, -1.0),
        ),
    ] {
        let mut line = Line::from_points(Vector3::new(5.0, 6.0, 7.0), point);
        let mut dispatched = EntityType::Line(line.clone());
        line.mirror3d(plane);
        dispatched.mirror3d(plane);
        assert_eq!(line.start, expected);
        assert_eq!(line.end, point);
        assert_eq!(dispatched, EntityType::Line(line));
    }
}

#[test]
fn invalid_document_mirrors_leave_the_entity_untouched() {
    let mut document = CadDocument::new();
    let handle = document
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::new(2.0, 3.0, 4.0),
            Vector3::new(5.0, 6.0, 7.0),
        )))
        .unwrap();
    let original = document.get_entity(handle).unwrap().clone();
    assert!(!document.mirror_entity(handle, Vector3::ZERO, Vector3::UNIT_Z));
    assert!(!document.mirror3d_entity(
        handle,
        MirrorPlane::PointNormal {
            point: Vector3::ZERO,
            normal: Vector3::ZERO
        }
    ));
    assert!(!document.mirror3d_entity(
        handle,
        MirrorPlane::ThreePoint {
            first: Vector3::ZERO,
            second: Vector3::UNIT_X,
            third: Vector3::UNIT_X * 2.0,
        }
    ));
    assert!(!document.mirror_entity(Handle::NULL, Vector3::ZERO, Vector3::UNIT_Y));
    assert_eq!(document.get_entity(handle), Some(&original));

    let mut unknown = UnknownEntity::new("CUSTOM_ENTITY");
    unknown.raw_dxf_codes = Some(vec![(10, "2.5".to_owned())]);
    let handle = document.add_entity(EntityType::Unknown(unknown)).unwrap();
    let original = document.get_entity(handle).unwrap().clone();
    assert!(!document.mirror_entity(handle, Vector3::ZERO, Vector3::UNIT_Y));
    assert_eq!(document.get_entity(handle), Some(&original));
}

#[test]
fn inherent_shape_mirrors_match_trait_geometry() {
    for mirror_x in [false, true] {
        let mut shape = Shape::new();
        shape.insertion_point = Vector3::new(2.0, 3.0, 4.0);
        shape.rotation = 0.3;
        let mut from_trait = shape.clone();
        if mirror_x {
            shape.mirror_x();
            Entity::mirror_x(&mut from_trait);
            assert_eq!(shape.insertion_point, Vector3::new(-2.0, 3.0, 4.0));
        } else {
            shape.mirror_y();
            Entity::mirror_y(&mut from_trait);
            assert_eq!(shape.insertion_point, Vector3::new(2.0, -3.0, 4.0));
        }
        assert_eq!(shape, from_trait);
    }
}

#[derive(Clone, Copy)]
pub(crate) struct NativeMirrorCase {
    pub(crate) name: &'static str,
    pub(crate) source: &'static str,
    pub(crate) expected: &'static str,
}

fn native_entity(fragment: &str) -> EntityType {
    let lines: Vec<_> = fragment.lines().collect();
    let fragment: String = lines
        .chunks_exact(2)
        .map(|pair| {
            let value = pair[1]
                .parse::<f64>()
                .ok()
                .filter(|value| value.abs() < 1e-12);
            format!(
                "{}\n{}\n",
                pair[0],
                if value.is_some() { "0" } else { pair[1] }
            )
        })
        .collect();
    let drawing = format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{fragment}0\nENDSEC\n0\nEOF\n");
    let document = crate::DxfReader::from_reader(std::io::Cursor::new(drawing.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let entity = document
        .entities()
        .next()
        .expect("native fixture entity")
        .clone();
    entity
}

fn canonical_entity_pairs(entity: EntityType) -> Vec<(i32, String)> {
    let mut document = crate::CadDocument::with_version(crate::types::DxfVersion::AC1032);
    document.add_entity(entity).unwrap();
    let bytes = crate::DxfWriter::new(&document).write_to_vec().unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let lines: Vec<_> = text.lines().collect();
    let pairs: Vec<_> = lines
        .chunks_exact(2)
        .map(|pair| {
            (
                pair[0].trim().parse::<i32>().unwrap(),
                pair[1].trim().to_owned(),
            )
        })
        .collect();
    let start = pairs
        .windows(2)
        .position(|pair| {
            pair[0] == (0, "SECTION".to_owned()) && pair[1] == (2, "ENTITIES".to_owned())
        })
        .unwrap()
        + 2;
    pairs[start..]
        .iter()
        .take_while(|pair| **pair != (0, "ENDSEC".to_owned()))
        .filter(|pair| !matches!(pair.0, 5 | 330 | 360))
        .map(|(code, value)| {
            (
                *code,
                if value.parse::<f64>() == Ok(0.0) {
                    "0".to_owned()
                } else {
                    value.clone()
                },
            )
        })
        .collect()
}

pub(crate) fn assert_native_case(case: NativeMirrorCase) -> Result<(), String> {
    let mut actual = std::panic::catch_unwind(|| native_entity(case.source))
        .map_err(|_| format!("{}: source DXF fixture could not be parsed", case.name))?;
    // Avoid a degrees/radians round trip changing the last bit of fixture
    // rotations, which affects the native readability choice at exactly 45 degrees.
    if let (EntityType::Text(text), Some((_, angle))) = (&mut actual, case.name.rsplit_once("_R")) {
        text.rotation = angle.parse().unwrap();
    }
    let expected = std::panic::catch_unwind(|| native_entity(case.expected))
        .map_err(|_| format!("{}: expected DXF fixture could not be parsed", case.name))?;
    let options = MirrorOptions {
        mirror_text: case.name.contains("_M1_") || case.name.ends_with("_M1"),
        mirror_hatch: case.name == "HATCH_M1",
        text_width: Some(
            if case.name.starts_with("INSERT_") || case.name.starts_with("MINSERT_") {
                25.666666666666668
            } else {
                20.0
            },
        ),
    };
    if case.name.starts_with("MIRROR3D_") {
        actual.mirror3d_with_options(
            MirrorPlane::XY {
                point: Vector3::ZERO,
            },
            options,
        );
    } else {
        let second = if case.name.contains("_Y_") || case.name.ends_with("_Y") {
            Vector3::UNIT_X
        } else if case.name.contains("_D_") || case.name.ends_with("_D") {
            Vector3::new(1.0, 1.0, 0.0)
        } else {
            Vector3::UNIT_Y
        };
        actual.mirror_with_options(Vector3::ZERO, second, options);
    }
    let actual = canonical_entity_pairs(actual);
    let expected = canonical_entity_pairs(expected);
    if actual.len() != expected.len() {
        return Err(format!(
            "{}: pair count {} != {}\nactual {actual:?}\nexpected {expected:?}",
            case.name,
            actual.len(),
            expected.len()
        ));
    }
    let mut differences = Vec::new();
    for (actual, expected) in actual.iter().zip(&expected) {
        let equal = if actual.0 != expected.0 {
            false
        } else if let (Ok(a), Ok(b)) = (actual.1.parse::<f64>(), expected.1.parse::<f64>()) {
            let difference = if (50..=55).contains(&actual.0) {
                (a - b + 180.0).rem_euclid(360.0) - 180.0
            } else {
                a - b
            };
            let tolerance = if (50..=55).contains(&actual.0) {
                1e-3
            } else {
                1e-5 * a.abs().max(b.abs()).max(1.0)
            };
            difference.abs() <= tolerance
        } else {
            actual.1 == expected.1
        };
        if !equal {
            differences.push(format!("{actual:?} != {expected:?}"));
        }
    }
    if differences.is_empty() {
        Ok(())
    } else {
        Err(format!("{}: {}", case.name, differences.join(", ")))
    }
}

// Captured with ACAD 2027 (ACADVER 26.0), native MIRROR/MIRROR3D, 2026-10-18.
// Fixtures are semantic DXF records; unstable database identities are omitted.

const NATIVE_MIRROR_CASES: &[NativeMirrorCase] = &[
    NativeMirrorCase { name: "SPLINE_CONTROL", source: "0\nSPLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSpline\n70\n0\n71\n3\n72\n9\n73\n5\n74\n0\n42\n1e-9\n43\n1e-10\n40\n0\n40\n0\n40\n0\n40\n0\n40\n0.5\n40\n1\n40\n1\n40\n1\n40\n1\n10\n0\n20\n0\n30\n0\n10\n2\n20\n4\n30\n1\n10\n4\n20\n-2\n30\n3\n10\n7\n20\n5\n30\n-1\n10\n10\n20\n0\n30\n2\n", expected: "0\nSPLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSpline\n70\n0\n71\n3\n72\n9\n73\n5\n74\n0\n42\n1e-9\n43\n1e-10\n40\n0\n40\n0\n40\n0\n40\n0\n40\n0.5\n40\n1\n40\n1\n40\n1\n40\n1\n10\n0\n20\n0\n30\n0\n10\n-2\n20\n4\n30\n1\n10\n-4\n20\n-2\n30\n3\n10\n-7\n20\n5\n30\n-1\n10\n-10\n20\n0\n30\n2\n" },
    NativeMirrorCase { name: "SPLINE_RATIONAL_PERIODIC", source: "0\nSPLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSpline\n70\n0\n71\n2\n72\n7\n73\n4\n74\n0\n42\n1e-9\n43\n1e-10\n40\n0\n40\n0\n40\n0\n40\n0.5\n40\n1\n40\n1\n40\n1\n10\n0\n20\n0\n30\n0\n10\n2\n20\n3\n30\n1\n10\n6\n20\n-1\n30\n2\n10\n9\n20\n4\n30\n0\n", expected: "0\nSPLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSpline\n70\n0\n71\n2\n72\n7\n73\n4\n74\n0\n42\n1e-9\n43\n1e-10\n40\n0\n40\n0\n40\n0\n40\n0.5\n40\n1\n40\n1\n40\n1\n10\n0\n20\n0\n30\n0\n10\n-2\n20\n3\n30\n1\n10\n-6\n20\n-1\n30\n2\n10\n-9\n20\n4\n30\n0\n" },
    NativeMirrorCase { name: "SPLINE_FIT", source: "0\nSPLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSpline\n70\n1056\n71\n3\n72\n11\n73\n7\n74\n5\n42\n1e-9\n43\n1e-10\n44\n0.00001\n12\n1\n22\n0\n32\n0\n13\n-1\n23\n0\n33\n0\n40\n0\n40\n0\n40\n0\n40\n0\n40\n4.58258\n40\n11.2158\n40\n19.8182\n40\n26.3756\n40\n26.3756\n40\n26.3756\n40\n26.3756\n10\n0\n20\n0\n30\n0\n10\n1.52753\n20\n0\n30\n0\n10\n1.82969\n20\n8.73684\n30\n0.884263\n10\n4.75104\n20\n-9.05485\n30\n5.38306\n10\n4.48639\n20\n11.2777\n30\n-4.27965\n10\n12.1858\n20\n0\n30\n2\n10\n10\n20\n0\n30\n2\n11\n0\n21\n0\n31\n0\n11\n2\n21\n4\n31\n1\n11\n4\n21\n-2\n31\n3\n11\n7\n21\n5\n31\n-1\n11\n10\n21\n0\n31\n2\n", expected: "0\nSPLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSpline\n70\n1056\n71\n3\n72\n11\n73\n7\n74\n5\n42\n1e-9\n43\n1e-10\n44\n0.00001\n12\n-1\n22\n0\n32\n0\n13\n1\n23\n0\n33\n0\n40\n0\n40\n0\n40\n0\n40\n0\n40\n4.58258\n40\n11.2158\n40\n19.8182\n40\n26.3756\n40\n26.3756\n40\n26.3756\n40\n26.3756\n10\n0\n20\n0\n30\n0\n10\n-1.52753\n20\n0\n30\n0\n10\n-1.82969\n20\n8.73684\n30\n0.884263\n10\n-4.75104\n20\n-9.05485\n30\n5.38306\n10\n-4.48639\n20\n11.2777\n30\n-4.27965\n10\n-12.1858\n20\n0\n30\n2\n10\n-10\n20\n0\n30\n2\n11\n0\n21\n0\n31\n0\n11\n-2\n21\n4\n31\n1\n11\n-4\n21\n-2\n31\n3\n11\n-7\n21\n5\n31\n-1\n11\n-10\n21\n0\n31\n2\n" },
    NativeMirrorCase { name: "POLYLINE_BSPLINE", source: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDb2dPolyline\n66\n1\n10\n0\n20\n0\n30\n0\n70\n0\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n0\n72\n0\n73\n0\n74\n0\n75\n6\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n3\n20\n1\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n6\n20\n-1\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n9\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDb2dPolyline\n66\n1\n10\n0\n20\n0\n30\n0\n70\n0\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n0\n72\n0\n73\n0\n74\n0\n75\n6\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n-3\n20\n1\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n-6\n20\n-1\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDb2dVertex\n10\n-9\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n91\n0\n70\n0\n50\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "POLYGONMESH_BSPLINE", source: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolygonMesh\n66\n1\n10\n0\n20\n0\n30\n0\n70\n16\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n3\n72\n3\n73\n2\n74\n2\n75\n6\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n3\n20\n0\n30\n1\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n6\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n0\n20\n3\n30\n2\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n3\n20\n3\n30\n4\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n6\n20\n3\n30\n2\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n0\n20\n6\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n3\n20\n6\n30\n1\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n6\n20\n6\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolygonMesh\n66\n1\n10\n0\n20\n0\n30\n0\n70\n16\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n3\n72\n3\n73\n2\n74\n2\n75\n6\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n-6\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n-3\n20\n0\n30\n1\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n-6\n20\n3\n30\n2\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n-3\n20\n3\n30\n4\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n0\n20\n3\n30\n2\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n-6\n20\n6\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n-3\n20\n6\n30\n1\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolygonMeshVertex\n10\n0\n20\n6\n30\n0\n40\n0\n41\n0\n42\n0\n70\n64\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A1_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A1_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A1_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A1_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A1_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A1_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A2_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A2_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A2_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A2_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A2_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A2_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A2_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A3_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A3_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A3_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A3_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A3_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A3_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A3_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A4_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A4_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A4_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A4_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A4_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A4_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A4_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A5_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A5_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A5_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A5_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A5_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A5_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A6_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A6_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A6_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A6_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A6_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A6_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A6_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A7_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A7_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A7_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A7_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A7_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A7_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A7_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A8_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A8_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A8_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A8_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A8_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A8_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A8_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A9_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A9_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A9_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A9_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A9_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_X_A9_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A1_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A1_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A1_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A1_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A1_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A1_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A2_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A2_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A2_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A2_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A2_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A2_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A2_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A3_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A3_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A3_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A3_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A3_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A3_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A3_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A4_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A4_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A4_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A4_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A4_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A4_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A4_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A5_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A5_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A5_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A5_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A5_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A5_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A6_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A6_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A6_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A6_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A6_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A6_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A6_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A7_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A7_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A7_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A7_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A7_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A7_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A7_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A8_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A8_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A8_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A8_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A8_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A8_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A8_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A9_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A9_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.24533346310494\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A9_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n222.49039804740613\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A9_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n153.73546263170738\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A9_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n84.98052721600857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_Y_A9_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.954883272720448\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A1_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A1_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A1_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A1_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A1_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A1_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A2_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A2_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A2_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A2_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A2_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A2_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A2_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A3_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A3_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A3_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A3_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A3_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A3_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A3_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A4_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A4_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A4_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A4_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A4_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A4_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A4_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A5_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A5_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A5_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A5_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A5_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A5_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A6_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A6_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A6_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A6_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A6_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A6_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A6_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A7_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A7_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A7_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A7_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A7_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A7_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A7_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A8_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A8_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A8_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A8_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A8_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A8_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A8_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A9_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A9_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n21.245045860332873\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A9_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.675463\n21\n0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n132.49018758825645\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A9_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n243.73510013306193\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A9_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.996165\n21\n-0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n354.9800126778675\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M0_D_A9_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.373877\n21\n-0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n291.95465521347694\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A1_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A1_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A1_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A1_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A1_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A1_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A2_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A2_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A2_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A2_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A2_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A2_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A2_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A3_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A3_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A3_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A3_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A3_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A3_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A3_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A4_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A4_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A4_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A4_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A4_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A4_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A4_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A5_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A5_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A5_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A5_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A5_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A5_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A6_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A6_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A6_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A6_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A6_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A6_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A6_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A7_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A7_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A7_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A7_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A7_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A7_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A7_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A8_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A8_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A8_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A8_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A8_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A8_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A8_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A9_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A9_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A9_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A9_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A9_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_X_A9_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A1_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A1_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A1_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A1_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A1_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A1_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A2_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A2_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A2_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A2_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A2_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A2_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A2_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A3_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A3_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A3_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A3_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A3_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A3_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A3_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A4_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A4_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A4_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A4_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A4_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A4_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A4_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A5_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A5_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A5_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A5_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A5_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A5_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A6_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A6_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A6_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A6_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A6_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A6_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A6_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A7_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A7_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A7_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A7_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A7_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A7_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A7_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A8_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A8_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A8_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A8_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A8_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A8_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A8_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A9_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n179.99984796050427\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A9_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.362358\n21\n-0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n248.7547833762031\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A9_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.737394\n21\n-0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n317.5097187919019\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A9_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.896758\n21\n0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n26.26478639925353\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A9_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.087499\n21\n0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n95.01989370229086\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_Y_A9_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.927478\n21\n0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.04525116668142\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A1_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A1_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A1_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A1_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A1_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A1_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A2_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A2_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A2_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A2_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A2_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A2_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A2_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n2\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A3_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A3_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A3_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A3_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A3_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A3_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A3_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A4_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A4_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A4_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A4_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A4_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A4_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A4_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n4\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A5_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A5_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A5_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A5_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A5_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A5_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A6_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A6_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A6_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A6_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A6_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A6_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A6_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n6\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A7_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A7_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A7_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A7_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A7_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A7_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A7_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A8_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A8_R0.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A8_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A8_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A8_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A8_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A8_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n8\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A9_R0.0", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n1\n21\n0\n31\n0\n42\n14.1667\n43\n6.66667\n50\n0\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0\n21\n1\n31\n0\n42\n14.1667\n43\n6.66667\n50\n90.0002104591497\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A9_R1.2", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.362358\n21\n0.932039\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.75493541569878\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.932039\n21\n0.362358\n31\n0\n42\n14.1667\n43\n6.66667\n50\n158.7551458748485\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A9_R2.4", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.737394\n21\n0.675463\n31\n0\n42\n14.1667\n43\n6.66667\n50\n137.50987083139756\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.675463\n21\n-0.737394\n31\n0\n42\n14.1667\n43\n6.66667\n50\n227.5100812905473\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A9_R3.6", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n-0.896758\n21\n-0.44252\n31\n0\n42\n14.1667\n43\n6.66667\n50\n206.26480624709637\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.44252\n21\n-0.896758\n31\n0\n42\n14.1667\n43\n6.66667\n50\n296.26501670624606\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A9_R4.8", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.087499\n21\n-0.996165\n31\n0\n42\n14.1667\n43\n6.66667\n50\n275.0197416627951\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.996165\n21\n0.087499\n31\n0\n42\n14.1667\n43\n6.66667\n50\n5.019740538920655\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_ROT_M1_D_A9_R5.9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.927478\n21\n-0.373877\n31\n0\n42\n14.1667\n43\n6.66667\n50\n338.0450991271857\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.373877\n21\n0.927478\n31\n0\n42\n14.1667\n43\n6.66667\n50\n68.0450407075317\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "SHAPE_SECOND_X", source: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n-3\n20\n5\n30\n0\n40\n2.45017\n2\nBOX\n50\n337.0819570735708\n41\n-0.816271\n51\n348.54111297618726\n210\n0\n220\n0\n230\n1\n", expected: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n3\n20\n5\n30\n0\n40\n2.40133\n2\nBOX\n50\n22.91831180523293\n41\n-0.832873\n51\n11.459155902616464\n210\n0\n220\n0\n230\n1\n" },
    NativeMirrorCase { name: "SHAPE_SECOND_Y", source: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n3\n20\n-5\n30\n0\n40\n2.45017\n2\nBOX\n50\n157.08153615527135\n41\n-0.816271\n51\n348.54111297618726\n210\n0\n220\n0\n230\n1\n", expected: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n3\n20\n5\n30\n0\n40\n2.40133\n2\nBOX\n50\n22.91831180523293\n41\n-0.832873\n51\n11.459155902616464\n210\n0\n220\n0\n230\n1\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F0_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-23\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F0_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-21.4212\n20\n12.7884\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F0_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-10.2472\n20\n23.6408\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F0_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n11.7479\n20\n18.5093\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F0_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n14.9352\n20\n-3.85041\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F0_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-4.74998\n20\n-14.9233\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F0_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-21.5496\n20\n-2.47753\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F2_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n17\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F2_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n15.4212\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F2_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.24716\n20\n-13.6408\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F2_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-17.7479\n20\n-8.50926\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F2_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-20.9352\n20\n13.8504\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F2_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-1.25002\n20\n24.9233\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F2_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n15.5496\n20\n12.4775\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F4_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-23\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F4_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-21.4212\n20\n12.7884\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F4_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-10.2472\n20\n23.6408\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F4_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n11.7479\n20\n18.5093\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F4_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n14.9352\n20\n-3.85041\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F4_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-4.74998\n20\n-14.9233\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F4_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-21.5496\n20\n-2.47753\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F6_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n17\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F6_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n15.4212\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F6_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.24716\n20\n-13.6408\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F6_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-17.7479\n20\n-8.50926\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F6_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-20.9352\n20\n13.8504\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F6_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-1.25002\n20\n24.9233\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_F6_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n15.5496\n20\n12.4775\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_ALIGN1", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.78939\n20\n1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-22.2106\n20\n8.89418\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n-13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_ALIGN2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-5.42122\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-13\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n5.42122\n21\n-2.78837\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_ALIGN3", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-13\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n-3\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_X_ALIGN5", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-13\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n-3\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "MTEXT_M0_X_A1", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M0_X_A5", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M0_X_A9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "ATTDEF_M0_X", source: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n", expected: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-21.4212\n20\n12.7884\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n" },
    NativeMirrorCase { name: "INSERT_M0_X", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n-20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n337.0819570735708\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-46.6667\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MINSERT_M0_X", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n2\n71\n2\n44\n12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n-20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n337.0819570735708\n70\n2\n71\n2\n44\n-12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-46.6667\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F0_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-7.5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F0_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.02645\n20\n-7.30265\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F0_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n0.669902\n20\n-5.90589\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F0_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n1.31134\n20\n-3.15652\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F0_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.1063\n20\n-2.7581\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F0_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5.49041\n20\n-5.21875\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F0_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.93469\n20\n-7.3187\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F2_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-7.5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F2_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.02645\n20\n-7.30265\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F2_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n0.669902\n20\n-5.90589\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F2_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n1.31134\n20\n-3.15652\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F2_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.1063\n20\n-2.7581\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F2_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5.49041\n20\n-5.21875\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F2_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.93469\n20\n-7.3187\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F4_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-2.5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F4_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.97355\n20\n-2.69735\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F4_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5.3301\n20\n-4.09411\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F4_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.68866\n20\n-6.84348\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F4_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n1.8937\n20\n-7.2419\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F4_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n0.509588\n20\n-4.78125\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F4_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.06531\n20\n-2.6813\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F6_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-2.5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F6_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.97355\n20\n-2.69735\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F6_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5.3301\n20\n-4.09411\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F6_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.68866\n20\n-6.84348\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F6_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n1.8937\n20\n-7.2419\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F6_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n0.509588\n20\n-4.78125\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_F6_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.06531\n20\n-2.6813\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_ALIGN1", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.78939\n20\n1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.81584\n20\n-3.40847\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n12.0265\n21\n-7.30265\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_ALIGN2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-5.42122\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-6.39477\n20\n0.485714\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n12.0265\n21\n-7.30265\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_ALIGN3", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-6.25\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n13\n21\n-6.25\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_Y_ALIGN5", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-7.5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n13\n21\n-7.5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "MTEXT_M0_Y_A1", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M0_Y_A5", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M0_Y_A9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n337.0819570735708\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "ATTDEF_M0_Y", source: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n", expected: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.02645\n20\n-7.30265\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n" },
    NativeMirrorCase { name: "INSERT_M0_Y", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n-20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n157.08153615527135\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n-23.5\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MINSERT_M0_Y", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n2\n71\n2\n44\n12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n-20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n157.08153615527135\n70\n2\n71\n2\n44\n-12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n-23.5\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F0_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n90.0002104591497\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F0_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.30265\n20\n2.02645\n30\n0\n40\n2.5\n1\nMirror Test\n50\n67.08189865391678\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F0_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5.90589\n20\n0.669902\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.245045860332873\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F0_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n18.5093\n20\n-11.7479\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F0_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.7581\n20\n4.1063\n30\n0\n40\n2.5\n1\nMirror Test\n50\n243.73510013306193\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F0_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-14.9233\n20\n4.74998\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F0_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-2.47753\n20\n21.5496\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F2_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n90.0002104591497\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F2_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.30265\n20\n2.02645\n30\n0\n40\n2.5\n1\nMirror Test\n50\n67.08189865391678\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F2_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5.90589\n20\n0.669902\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.245045860332873\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F2_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-8.50926\n20\n17.7479\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F2_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.7581\n20\n4.1063\n30\n0\n40\n2.5\n1\nMirror Test\n50\n243.73510013306193\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F2_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n24.9233\n20\n1.25002\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F2_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n12.4775\n20\n-15.5496\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F4_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n90.0002104591497\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F4_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.69735\n20\n3.97355\n30\n0\n40\n2.5\n1\nMirror Test\n50\n67.08189865391678\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F4_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.09411\n20\n5.3301\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.245045860332873\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F4_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n18.5093\n20\n-11.7479\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F4_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.2419\n20\n1.8937\n30\n0\n40\n2.5\n1\nMirror Test\n50\n243.73510013306193\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F4_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-14.9233\n20\n4.74998\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F4_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-2.47753\n20\n21.5496\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F6_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n90.0002104591497\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F6_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n2.69735\n20\n3.97355\n30\n0\n40\n2.5\n1\nMirror Test\n50\n67.08189865391678\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F6_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n4.09411\n20\n5.3301\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.245045860332873\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F6_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-8.50926\n20\n17.7479\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F6_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.2419\n20\n1.8937\n30\n0\n40\n2.5\n1\nMirror Test\n50\n243.73510013306193\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F6_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n24.9233\n20\n1.25002\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_F6_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n12.4775\n20\n-15.5496\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_ALIGN1", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.78939\n20\n1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.40847\n20\n2.81584\n30\n0\n40\n2.5\n1\nMirror Test\n50\n67.08189865391678\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n7.30265\n21\n12.0265\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_ALIGN2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-5.42122\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-0.485714\n20\n-6.39477\n30\n0\n40\n2.5\n1\nMirror Test\n50\n67.08189865391678\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n7.30265\n21\n12.0265\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_ALIGN3", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n6.25\n20\n3\n30\n0\n40\n1.25\n1\nMirror Test\n50\n90.0002104591497\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n6.25\n21\n13\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M0_D_ALIGN5", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n90.0002104591497\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n7.5\n21\n13\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "MTEXT_M0_D_A1", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n7\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M0_D_A5", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M0_D_A9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n3\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n67.08189865391678\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "ATTDEF_M0_D", source: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n", expected: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n7.30265\n20\n2.02645\n30\n0\n40\n2.5\n1\nMirror Test\n50\n67.08189865391678\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n" },
    NativeMirrorCase { name: "INSERT_M0_D", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n247.08174661442106\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n23.5\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n90.0002104591497\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MINSERT_M0_D", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n2\n71\n2\n44\n12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n247.08174661442106\n70\n2\n71\n2\n44\n-12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n23.5\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n90.0002104591497\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F0_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F0_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F0_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F0_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F0_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F0_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F0_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F2_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F2_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F2_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F2_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F2_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F2_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F2_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F4_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F4_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F4_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F4_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F4_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F4_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F4_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F6_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F6_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F6_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.24533346310494\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F6_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n222.49039804740613\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F6_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n153.73546263170738\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F6_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n84.98052721600857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_F6_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n21.954883272720448\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_ALIGN1", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.78939\n20\n1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3.78939\n20\n1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n1\n11\n-13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_ALIGN2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-5.42122\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5.42122\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n2\n11\n-13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_ALIGN3", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n3\n11\n-13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_X_ALIGN5", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n5\n11\n-13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "MTEXT_M1_X_A1", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M1_X_A5", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M1_X_A9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n-0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "ATTDEF_M1_X", source: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n", expected: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n337.0819570735708\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n" },
    NativeMirrorCase { name: "INSERT_M1_X", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n-20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n337.0819570735708\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MINSERT_M1_X", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n2\n71\n2\n44\n12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n-20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n337.0819570735708\n70\n2\n71\n2\n44\n-12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F0_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n179.99984796050427\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F0_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n157.08153615527135\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F0_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n111.2449125448055\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F0_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n42.49014901644526\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F0_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n333.73531059221165\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F0_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n264.9803751765129\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F0_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.9550177121223\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F2_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n179.99984796050427\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F2_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n157.08153615527135\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F2_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n111.2449125448055\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F2_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n42.49014901644526\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F2_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n333.73531059221165\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F2_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n264.9803751765129\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F2_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.9550177121223\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F4_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n179.99984796050427\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F4_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n157.08153615527135\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F4_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n111.2449125448055\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F4_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n42.49014901644526\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F4_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n333.73531059221165\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F4_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n264.9803751765129\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F4_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.9550177121223\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F6_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n179.99984796050427\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F6_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n157.08153615527135\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F6_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n111.2449125448055\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F6_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n42.49014901644526\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F6_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n333.73531059221165\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F6_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n264.9803751765129\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_F6_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.9550177121223\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_ALIGN1", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.78939\n20\n1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.78939\n20\n-1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n157.08153615527135\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n1\n11\n13\n21\n-5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_ALIGN2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-5.42122\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-5.42122\n20\n2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n157.08153615527135\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n2\n11\n13\n21\n-5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_ALIGN3", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n179.99984796050427\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n3\n11\n13\n21\n-5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_Y_ALIGN5", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n179.99984796050427\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n5\n11\n13\n21\n-5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "MTEXT_M1_Y_A1", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M1_Y_A5", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M1_Y_A9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.921061\n21\n-0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n202.91815976573722\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "ATTDEF_M1_Y", source: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n", expected: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n-5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n157.08153615527135\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n" },
    NativeMirrorCase { name: "INSERT_M1_Y", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n-20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n157.08153615527135\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n-21\n30\n0\n40\n2.5\n1\nInstance value\n50\n179.99984796050427\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MINSERT_M1_Y", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n2\n71\n2\n44\n12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n-20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n157.08153615527135\n70\n2\n71\n2\n44\n-12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n-21\n30\n0\n40\n2.5\n1\nInstance value\n50\n179.99984796050427\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F0_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n270.000058419654\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F0_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n247.08174661442106\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F0_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.2451230039552\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F0_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F0_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n63.735252172557644\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F0_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F0_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F2_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n270.000058419654\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F2_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n247.08174661442106\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F2_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.2451230039552\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F2_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F2_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n63.735252172557644\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F2_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F2_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F4_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n270.000058419654\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F4_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n247.08174661442106\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F4_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.2451230039552\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F4_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F4_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n63.735252172557644\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F4_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F4_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F6_R0.0", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n270.000058419654\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F6_R0.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n247.08174661442106\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F6_R1.2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n68.75493541569878\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n201.2451230039552\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F6_R2.4", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n137.50987083139756\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n132.49018758825645\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F6_R3.6", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n206.26480624709637\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n63.735252172557644\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F6_R4.8", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n275.0197416627951\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n354.9800126778675\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_F6_R5.9", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n338.0450991271857\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n6\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n291.95465521347694\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n4\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_ALIGN1", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3.78939\n20\n1.10582\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n1\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n1.10582\n20\n3.78939\n30\n0\n40\n2.5\n1\nMirror Test\n50\n247.08174661442106\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n1\n11\n5\n21\n13\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_ALIGN2", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-5.42122\n20\n-2.78837\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n2\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n-2.78837\n20\n-5.42122\n30\n0\n40\n2.5\n1\nMirror Test\n50\n247.08174661442106\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n2\n11\n5\n21\n13\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_ALIGN3", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n1.25\n1\nMirror Test\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n3\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n1.25\n1\nMirror Test\n50\n270.000058419654\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n3\n11\n5\n21\n13\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "TEXT_M1_D_ALIGN5", source: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n0\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n5\n11\n13\n21\n5\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n", expected: "0\nTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n270.000058419654\n41\n0.4\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n5\n11\n5\n21\n13\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbText\n73\n0\n" },
    NativeMirrorCase { name: "MTEXT_M1_D_A1", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n1\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M1_D_A5", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n5\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "MTEXT_M1_D_A9", source: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n3\n20\n5\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n1\n11\n0.921061\n21\n0.389418\n31\n0\n42\n14.1667\n43\n6.66667\n50\n22.91831180523293\n73\n1\n44\n1\n", expected: "0\nMTEXT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMText\n10\n5\n20\n3\n30\n0\n40\n2.5\n41\n30\n46\n0\n71\n9\n72\n5\n1\nFirst\\PSecond\n7\nStandard\n210\n0\n220\n0\n230\n-1\n11\n0.389418\n21\n0.921061\n31\n0\n42\n14.1667\n43\n6.66667\n50\n112.91852226438264\n73\n1\n44\n1\n" },
    NativeMirrorCase { name: "ATTDEF_M1_D", source: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n3\n20\n5\n30\n0\n40\n2.5\n1\nMirror Test\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n", expected: "0\nATTDEF\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n5\n20\n3\n30\n0\n40\n2.5\n1\nMirror Test\n50\n247.08174661442106\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttributeDefinition\n280\n0\n3\nPrompt\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n" },
    NativeMirrorCase { name: "INSERT_M1_D", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbBlockReference\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n247.08174661442106\n70\n0\n71\n0\n44\n0\n45\n0\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n270.000058419654\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MINSERT_M1_D", source: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n2\n42\n3\n43\n1\n50\n22.91831180523293\n70\n2\n71\n2\n44\n12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n0\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n0\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nINSERT\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMInsertBlock\n66\n1\n2\nAM_BLOCK\n10\n20\n20\n20\n30\n0\n41\n-2\n42\n3\n43\n1\n50\n247.08174661442106\n70\n2\n71\n2\n44\n-12\n45\n14\n210\n0\n220\n0\n230\n1\n0\nATTRIB\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbText\n10\n21\n20\n21\n30\n0\n40\n2.5\n1\nInstance value\n50\n270.000058419654\n41\n0.8\n51\n11.459155902616464\n7\nStandard\n71\n2\n72\n0\n11\n0\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n100\nAcDbAttribute\n280\n0\n2\nTAG\n70\n0\n73\n0\n74\n0\n280\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "SOLID_QUAD", source: "0\nSOLID\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbTrace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n1\n22\n12\n32\n0\n13\n10\n23\n12\n33\n0\n39\n0\n210\n0\n220\n0\n230\n1\n", expected: "0\nSOLID\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbTrace\n10\n-1\n20\n2\n30\n0\n11\n-10\n21\n2\n31\n0\n12\n-1\n22\n12\n32\n0\n13\n-10\n23\n12\n33\n0\n39\n0\n210\n0\n220\n0\n230\n1\n" },
    NativeMirrorCase { name: "SOLID_TRIANGLE", source: "0\nSOLID\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbTrace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n1\n22\n12\n32\n0\n13\n1\n23\n12\n33\n0\n39\n0\n210\n0\n220\n0\n230\n1\n", expected: "0\nSOLID\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbTrace\n10\n-1\n20\n2\n30\n0\n11\n-10\n21\n2\n31\n0\n12\n-1\n22\n12\n32\n0\n13\n-1\n23\n12\n33\n0\n39\n0\n210\n0\n220\n0\n230\n1\n" },
    NativeMirrorCase { name: "FACE_QUAD", source: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n10\n22\n12\n32\n0\n13\n1\n23\n12\n33\n0\n70\n5\n", expected: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n-1\n20\n12\n30\n0\n11\n-10\n21\n12\n31\n0\n12\n-10\n22\n2\n32\n0\n13\n-1\n23\n2\n33\n0\n70\n5\n" },
    NativeMirrorCase { name: "FACE_TRIANGLE", source: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n10\n22\n12\n32\n0\n13\n10\n23\n12\n33\n0\n70\n5\n", expected: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n-10\n20\n12\n30\n0\n11\n-10\n21\n12\n31\n0\n12\n-10\n22\n2\n32\n0\n13\n-1\n23\n2\n33\n0\n70\n5\n" },
    NativeMirrorCase { name: "FACE_FLAG1", source: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n10\n22\n12\n32\n0\n13\n1\n23\n12\n33\n0\n70\n1\n", expected: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n-1\n20\n12\n30\n0\n11\n-10\n21\n12\n31\n0\n12\n-10\n22\n2\n32\n0\n13\n-1\n23\n2\n33\n0\n70\n4\n" },
    NativeMirrorCase { name: "FACE_FLAG2", source: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n10\n22\n12\n32\n0\n13\n1\n23\n12\n33\n0\n70\n2\n", expected: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n-1\n20\n12\n30\n0\n11\n-10\n21\n12\n31\n0\n12\n-10\n22\n2\n32\n0\n13\n-1\n23\n2\n33\n0\n70\n2\n" },
    NativeMirrorCase { name: "FACE_FLAG4", source: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n10\n22\n12\n32\n0\n13\n1\n23\n12\n33\n0\n70\n4\n", expected: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n-1\n20\n12\n30\n0\n11\n-10\n21\n12\n31\n0\n12\n-10\n22\n2\n32\n0\n13\n-1\n23\n2\n33\n0\n70\n1\n" },
    NativeMirrorCase { name: "FACE_FLAG8", source: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n1\n20\n2\n30\n0\n11\n10\n21\n2\n31\n0\n12\n10\n22\n12\n32\n0\n13\n1\n23\n12\n33\n0\n70\n8\n", expected: "0\n3DFACE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFace\n10\n-1\n20\n12\n30\n0\n11\n-10\n21\n12\n31\n0\n12\n-10\n22\n2\n32\n0\n13\n-1\n23\n2\n33\n0\n70\n8\n" },
    NativeMirrorCase { name: "POLYFACE_TRIANGLE", source: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyFaceMesh\n66\n1\n10\n0\n20\n0\n30\n0\n70\n64\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n4\n72\n1\n73\n0\n74\n0\n75\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n1\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n10\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n10\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n1\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFaceRecord\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n128\n50\n0\n71\n-1\n72\n2\n73\n-3\n74\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyFaceMesh\n66\n1\n10\n0\n20\n0\n30\n0\n70\n64\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n4\n72\n1\n73\n0\n74\n0\n75\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-1\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-10\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-10\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-1\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFaceRecord\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n128\n50\n0\n71\n3\n72\n-2\n73\n-1\n74\n0\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "POLYFACE_QUAD", source: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyFaceMesh\n66\n1\n10\n0\n20\n0\n30\n0\n70\n64\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n4\n72\n1\n73\n0\n74\n0\n75\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n1\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n10\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n10\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n1\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFaceRecord\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n128\n50\n0\n71\n-1\n72\n2\n73\n-3\n74\n4\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n", expected: "0\nPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyFaceMesh\n66\n1\n10\n0\n20\n0\n30\n0\n70\n64\n40\n0\n41\n0\n210\n0\n220\n0\n230\n1\n71\n4\n72\n1\n73\n0\n74\n0\n75\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-1\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-10\n20\n2\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-10\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbVertex\n100\nAcDbPolyFaceMeshVertex\n10\n-1\n20\n12\n30\n0\n40\n0\n41\n0\n42\n0\n70\n192\n50\n0\n71\n0\n72\n0\n73\n0\n74\n0\n0\nVERTEX\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbFaceRecord\n10\n0\n20\n0\n30\n0\n40\n0\n41\n0\n42\n0\n70\n128\n50\n0\n71\n-4\n72\n3\n73\n-2\n74\n1\n0\nSEQEND\n100\nAcDbEntity\n67\n0\n8\n0\n" },
    NativeMirrorCase { name: "MESH_QUAD", source: "0\nMESH\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSubDMesh\n71\n2\n72\n0\n91\n0\n92\n4\n10\n1\n20\n2\n30\n0\n10\n10\n20\n2\n30\n0\n10\n10\n20\n12\n30\n0\n10\n1\n20\n12\n30\n0\n93\n5\n90\n4\n90\n0\n90\n1\n90\n2\n90\n3\n94\n4\n90\n0\n90\n1\n90\n1\n90\n2\n90\n2\n90\n3\n90\n0\n90\n3\n95\n4\n140\n0\n140\n0\n140\n0\n140\n0\n90\n0\n", expected: "0\nMESH\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbSubDMesh\n71\n2\n72\n0\n91\n0\n92\n4\n10\n-1\n20\n2\n30\n0\n10\n-10\n20\n2\n30\n0\n10\n-10\n20\n12\n30\n0\n10\n-1\n20\n12\n30\n0\n93\n5\n90\n4\n90\n0\n90\n1\n90\n2\n90\n3\n94\n4\n90\n0\n90\n1\n90\n1\n90\n2\n90\n2\n90\n3\n90\n0\n90\n3\n95\n4\n140\n0\n140\n0\n140\n0\n140\n0\n90\n0\n" },
    NativeMirrorCase { name: "SHAPE_X", source: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n3\n20\n5\n30\n0\n40\n2.5\n2\nBOX\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n210\n0\n220\n0\n230\n1\n", expected: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n-3\n20\n5\n30\n0\n40\n2.45017\n2\nBOX\n50\n337.0819570735708\n41\n-0.816271\n51\n348.54111297618726\n210\n0\n220\n0\n230\n1\n" },
    NativeMirrorCase { name: "SHAPE_Y", source: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n3\n20\n5\n30\n0\n40\n2.5\n2\nBOX\n50\n22.91831180523293\n41\n0.8\n51\n11.459155902616464\n210\n0\n220\n0\n230\n1\n", expected: "0\nSHAPE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbShape\n10\n3\n20\n-5\n30\n0\n40\n2.45017\n2\nBOX\n50\n157.08153615527135\n41\n-0.816271\n51\n348.54111297618726\n210\n0\n220\n0\n230\n1\n" },
    NativeMirrorCase { name: "ARC_(0.0 0.0 1.0)", source: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n2\n20\n3\n30\n5\n40\n4\n210\n0\n220\n0\n230\n1\n100\nAcDbArc\n50\n17.188733853924695\n51\n137.50987083139756\n", expected: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n-2\n20\n3\n30\n5\n40\n4\n210\n0\n220\n0\n230\n1\n100\nAcDbArc\n50\n42.49014901644526\n51\n162.8111141065796\n" },
    NativeMirrorCase { name: "LWPOLY_(0.0 0.0 1.0)", source: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n2\n20\n3\n40\n0\n41\n0\n42\n0.5\n91\n0\n10\n10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0\n220\n0\n230\n1\n", expected: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n-2\n20\n3\n40\n0\n41\n0\n42\n-0.5\n91\n0\n10\n-10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0\n220\n0\n230\n1\n" },
    NativeMirrorCase { name: "ELLIPSE_(0.0 0.0 1.0)", source: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n2\n20\n3\n30\n5\n11\n4\n21\n0\n31\n0\n210\n0\n220\n0\n230\n1\n40\n0.5\n41\n0.3\n42\n2.4\n", expected: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n-2\n20\n3\n30\n5\n11\n-4\n21\n0\n31\n0\n210\n0\n220\n0\n230\n-1\n40\n0.5\n41\n0.3\n42\n2.4\n" },
    NativeMirrorCase { name: "ARC_(0.0 1.0 0.0)", source: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n2\n20\n3\n30\n5\n40\n4\n210\n0\n220\n1\n230\n0\n100\nAcDbArc\n50\n17.188733853924695\n51\n137.50987083139756\n", expected: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n-2\n20\n3\n30\n5\n40\n4\n210\n0\n220\n1\n230\n0\n100\nAcDbArc\n50\n42.49014901644526\n51\n162.8111141065796\n" },
    NativeMirrorCase { name: "LWPOLY_(0.0 1.0 0.0)", source: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n2\n20\n3\n40\n0\n41\n0\n42\n0.5\n91\n0\n10\n10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0\n220\n1\n230\n0\n", expected: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n-2\n20\n3\n40\n0\n41\n0\n42\n-0.5\n91\n0\n10\n-10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0\n220\n1\n230\n0\n" },
    NativeMirrorCase { name: "ELLIPSE_(0.0 1.0 0.0)", source: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n2\n20\n3\n30\n5\n11\n-4\n21\n0\n31\n0\n210\n0\n220\n1\n230\n0\n40\n0.5\n41\n0.3\n42\n2.4\n", expected: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n-2\n20\n3\n30\n5\n11\n4\n21\n0\n31\n0\n210\n0\n220\n-1\n230\n0\n40\n0.5\n41\n0.3\n42\n2.4\n" },
    NativeMirrorCase { name: "ARC_(0.0 0.6 0.8)", source: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n2\n20\n3\n30\n5\n40\n4\n210\n0\n220\n0.6\n230\n0.8\n100\nAcDbArc\n50\n17.188733853924695\n51\n137.50987083139756\n", expected: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n-2\n20\n3\n30\n5\n40\n4\n210\n0\n220\n0.6\n230\n0.8\n100\nAcDbArc\n50\n42.49014901644526\n51\n162.8111141065796\n" },
    NativeMirrorCase { name: "LWPOLY_(0.0 0.6 0.8)", source: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n2\n20\n3\n40\n0\n41\n0\n42\n0.5\n91\n0\n10\n10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0\n220\n0.6\n230\n0.8\n", expected: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n-2\n20\n3\n40\n0\n41\n0\n42\n-0.5\n91\n0\n10\n-10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0\n220\n0.6\n230\n0.8\n" },
    NativeMirrorCase { name: "ELLIPSE_(0.0 0.6 0.8)", source: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n2\n20\n3\n30\n5\n11\n-4\n21\n0\n31\n0\n210\n0\n220\n0.6\n230\n0.8\n40\n0.5\n41\n0.3\n42\n2.4\n", expected: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n-2\n20\n3\n30\n5\n11\n4\n21\n0\n31\n0\n210\n0\n220\n-0.6\n230\n-0.8\n40\n0.5\n41\n0.3\n42\n2.4\n" },
    NativeMirrorCase { name: "ARC_(0.6 0.0 0.8)", source: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n2\n20\n3\n30\n5\n40\n4\n210\n0.6\n220\n0\n230\n0.8\n100\nAcDbArc\n50\n17.188733853924695\n51\n137.50987083139756\n", expected: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n-2\n20\n3\n30\n5\n40\n4\n210\n-0.6\n220\n0\n230\n0.8\n100\nAcDbArc\n50\n42.49014901644526\n51\n162.8111141065796\n" },
    NativeMirrorCase { name: "LWPOLY_(0.6 0.0 0.8)", source: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n2\n20\n3\n40\n0\n41\n0\n42\n0.5\n91\n0\n10\n10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0.6\n220\n0\n230\n0.8\n", expected: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n-2\n20\n3\n40\n0\n41\n0\n42\n-0.5\n91\n0\n10\n-10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n-0.6\n220\n0\n230\n0.8\n" },
    NativeMirrorCase { name: "ELLIPSE_(0.6 0.0 0.8)", source: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n2\n20\n3\n30\n5\n11\n0\n21\n4\n31\n0\n210\n0.6\n220\n0\n230\n0.8\n40\n0.5\n41\n0.3\n42\n2.4\n", expected: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n-2\n20\n3\n30\n5\n11\n0\n21\n4\n31\n0\n210\n0.6\n220\n0\n230\n-0.8\n40\n0.5\n41\n0.3\n42\n2.4\n" },
    NativeMirrorCase { name: "ARC_(0.36 0.48 0.8)", source: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n2\n20\n3\n30\n5\n40\n4\n210\n0.36\n220\n0.48\n230\n0.8\n100\nAcDbArc\n50\n17.188733853924695\n51\n137.50987083139756\n", expected: "0\nARC\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbCircle\n10\n-2\n20\n3\n30\n5\n40\n4\n210\n-0.36\n220\n0.48\n230\n0.8\n100\nAcDbArc\n50\n42.49014901644526\n51\n162.8111141065796\n" },
    NativeMirrorCase { name: "LWPOLY_(0.36 0.48 0.8)", source: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n2\n20\n3\n40\n0\n41\n0\n42\n0.5\n91\n0\n10\n10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n0.36\n220\n0.48\n230\n0.8\n", expected: "0\nLWPOLYLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbPolyline\n90\n2\n70\n0\n43\n0\n38\n5\n39\n0\n10\n-2\n20\n3\n40\n0\n41\n0\n42\n-0.5\n91\n0\n10\n-10\n20\n8\n40\n0\n41\n0\n42\n0\n91\n0\n210\n-0.36\n220\n0.48\n230\n0.8\n" },
    NativeMirrorCase { name: "ELLIPSE_(0.36 0.48 0.8)", source: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n2\n20\n3\n30\n5\n11\n-3.2\n21\n2.4\n31\n0\n210\n0.36\n220\n0.48\n230\n0.8\n40\n0.5\n41\n0.3\n42\n2.4\n", expected: "0\nELLIPSE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbEllipse\n10\n-2\n20\n3\n30\n5\n11\n3.2\n21\n2.4\n31\n0\n210\n0.36\n220\n-0.48\n230\n-0.8\n40\n0.5\n41\n0.3\n42\n2.4\n" },
    NativeMirrorCase { name: "HATCH_M0", source: "0\nHATCH\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbHatch\n10\n0\n20\n0\n30\n0\n210\n0\n220\n0\n230\n1\n2\nANSI31\n70\n0\n71\n0\n91\n1\n92\n2\n72\n0\n73\n1\n93\n4\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n97\n0\n75\n0\n76\n1\n52\n11.459155902616464\n41\n1\n77\n0\n78\n1\n53\n56.4591465406323\n43\n0\n44\n0\n45\n-2.64634\n46\n1.75429\n79\n0\n98\n0\n", expected: "0\nHATCH\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbHatch\n10\n0\n20\n0\n30\n0\n210\n0\n220\n0\n230\n1\n2\nANSI31\n70\n0\n71\n0\n91\n1\n92\n2\n72\n0\n73\n1\n93\n4\n10\n0\n20\n0\n10\n-10\n20\n0\n10\n-10\n20\n10\n10\n0\n20\n10\n97\n0\n75\n0\n76\n1\n52\n11.459155902616464\n41\n1\n77\n0\n78\n1\n53\n56.4591465406323\n43\n0\n44\n0\n45\n-2.64634\n46\n1.75429\n79\n0\n98\n0\n" },
    NativeMirrorCase { name: "HATCH_M1", source: "0\nHATCH\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbHatch\n10\n0\n20\n0\n30\n0\n210\n0\n220\n0\n230\n1\n2\nANSI31\n70\n0\n71\n0\n91\n1\n92\n2\n72\n0\n73\n1\n93\n4\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n10\n0\n20\n10\n97\n0\n75\n0\n76\n1\n52\n11.459155902616464\n41\n1\n77\n0\n78\n1\n53\n56.4591465406323\n43\n0\n44\n0\n45\n-2.64634\n46\n1.75429\n79\n0\n98\n0\n", expected: "0\nHATCH\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbHatch\n10\n0\n20\n0\n30\n0\n210\n0\n220\n0\n230\n1\n2\nANSI31\n70\n0\n71\n0\n91\n1\n92\n2\n72\n0\n73\n1\n93\n4\n10\n0\n20\n0\n10\n-10\n20\n0\n10\n-10\n20\n10\n10\n0\n20\n10\n97\n0\n75\n0\n76\n1\n52\n11.459155902616464\n41\n1\n77\n0\n78\n1\n53\n123.54058682831297\n43\n0\n44\n0\n45\n2.64634\n46\n1.75429\n79\n0\n98\n0\n" },
    NativeMirrorCase { name: "MLINE", source: "0\nMLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMline\n2\nSTANDARD\n40\n20\n70\n0\n71\n1\n72\n3\n73\n2\n10\n1\n20\n2\n30\n0\n210\n0\n220\n0\n230\n1\n11\n1\n21\n2\n31\n0\n12\n0.83205\n22\n0.5547\n32\n0\n13\n-0.5547\n23\n0.83205\n33\n0\n74\n2\n41\n0\n41\n0\n75\n0\n74\n2\n41\n-20\n41\n0\n75\n0\n11\n10\n21\n8\n31\n0\n12\n0.928477\n22\n-0.371391\n32\n0\n13\n-0.103562\n23\n0.994623\n33\n0\n74\n2\n41\n0\n41\n0\n75\n0\n74\n2\n41\n-22.5983\n41\n0\n75\n0\n11\n20\n21\n4\n31\n0\n12\n0.928477\n22\n-0.371391\n32\n0\n13\n0.371391\n23\n0.928477\n33\n0\n74\n2\n41\n0\n41\n0\n75\n0\n74\n2\n41\n-20\n41\n0\n75\n0\n", expected: "0\nMLINE\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbMline\n2\nSTANDARD\n40\n-20\n70\n0\n71\n1\n72\n3\n73\n2\n10\n-1\n20\n2\n30\n0\n210\n0\n220\n0\n230\n1\n11\n-1\n21\n2\n31\n0\n12\n-0.83205\n22\n0.5547\n32\n0\n13\n0.5547\n23\n0.83205\n33\n0\n74\n2\n41\n0\n41\n0\n75\n0\n74\n2\n41\n-20\n41\n0\n75\n0\n11\n-10\n21\n8\n31\n0\n12\n-0.928477\n22\n-0.371391\n32\n0\n13\n0.103562\n23\n0.994623\n33\n0\n74\n2\n41\n0\n41\n0\n75\n0\n74\n2\n41\n-22.5983\n41\n0\n75\n0\n11\n-20\n21\n4\n31\n0\n12\n-0.928477\n22\n-0.371391\n32\n0\n13\n-0.371391\n23\n0.928477\n33\n0\n74\n2\n41\n0\n41\n0\n75\n0\n74\n2\n41\n-20\n41\n0\n75\n0\n" },
    NativeMirrorCase { name: "DIM_LINEAR", source: "0\nDIMENSION\n102\n{ACAD_XDICTIONARY\n102\n}\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbDimension\n280\n0\n2\n*D7\n10\n10\n20\n10\n30\n0\n11\n5.5\n21\n215\n31\n0\n12\n0\n22\n0\n32\n0\n70\n32\n1\n\n71\n5\n72\n1\n41\n1\n42\n9\n73\n0\n74\n0\n75\n0\n52\n0\n53\n0\n54\n0\n51\n0\n210\n0\n220\n0\n230\n1\n3\nAnnotative-UK\n100\nAcDbAlignedDimension\n13\n1\n23\n2\n33\n0\n14\n10\n24\n4\n34\n0\n15\n0\n25\n0\n35\n0\n16\n0\n26\n0\n36\n0\n40\n0\n50\n0\n100\nAcDbRotatedDimension\n", expected: "0\nDIMENSION\n102\n{ACAD_XDICTIONARY\n102\n}\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbDimension\n280\n0\n2\n*D7\n10\n-10\n20\n10\n30\n0\n11\n-5.5\n21\n215\n31\n0\n12\n0\n22\n0\n32\n0\n70\n32\n1\n\n71\n5\n72\n1\n41\n1\n42\n9\n73\n0\n74\n0\n75\n0\n52\n0\n53\n0\n54\n0\n51\n0\n210\n0\n220\n0\n230\n1\n3\nAnnotative-UK\n100\nAcDbAlignedDimension\n13\n-1\n23\n2\n33\n0\n14\n-10\n24\n4\n34\n0\n15\n0\n25\n0\n35\n0\n16\n0\n26\n0\n36\n0\n40\n0\n50\n179.99984796050427\n100\nAcDbRotatedDimension\n" },
    NativeMirrorCase { name: "DIM_ALIGNED", source: "0\nDIMENSION\n102\n{ACAD_XDICTIONARY\n102\n}\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbDimension\n280\n0\n2\n*D8\n10\n8.30588\n20\n11.6235\n30\n0\n11\n-40.6649\n21\n210.742\n31\n0\n12\n0\n22\n0\n32\n0\n70\n33\n1\n\n71\n5\n72\n1\n41\n1\n42\n9.21954\n73\n0\n74\n0\n75\n0\n52\n0\n53\n0\n54\n0\n51\n0\n210\n0\n220\n0\n230\n1\n3\nAnnotative-UK\n100\nAcDbAlignedDimension\n13\n1\n23\n2\n33\n0\n14\n10\n24\n4\n34\n0\n15\n0\n25\n0\n35\n0\n16\n0\n26\n0\n36\n0\n40\n0\n50\n0\n", expected: "0\nDIMENSION\n102\n{ACAD_XDICTIONARY\n102\n}\n100\nAcDbEntity\n67\n0\n8\n0\n100\nAcDbDimension\n280\n0\n2\n*D8\n10\n-8.30588\n20\n11.6235\n30\n0\n11\n40.6649\n21\n210.742\n31\n0\n12\n0\n22\n0\n32\n0\n70\n33\n1\n\n71\n5\n72\n1\n41\n1\n42\n9.21954\n73\n0\n74\n0\n75\n0\n52\n0\n53\n0\n54\n0\n51\n0\n210\n0\n220\n0\n230\n1\n3\nAnnotative-UK\n100\nAcDbAlignedDimension\n13\n-1\n23\n2\n33\n0\n14\n-10\n24\n4\n34\n0\n15\n0\n25\n0\n35\n0\n16\n0\n26\n0\n36\n0\n40\n0\n50\n0\n" },
];

#[test]
fn native_2027_planar_mirror_scenarios() {
    assert_eq!(NATIVE_MIRROR_CASES.len(), 628);
    let failures: Vec<_> = NATIVE_MIRROR_CASES
        .iter()
        .copied()
        .filter_map(|case| assert_native_case(case).err())
        .collect();
    assert!(
        failures.is_empty(),
        "{} native cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn native_geometry_spline_modes_use_the_same_planar_transform() {
    let control_points = vec![
        Vector3::new(2.0, 1.0, 1.0),
        Vector3::new(5.0, 8.0, 2.5),
        Vector3::new(9.0, -2.0, 4.0),
        Vector3::new(14.0, 6.0, 5.5),
    ];
    let mut closed = Spline::from_control_points(3, control_points.clone());
    closed.flags.closed = true;
    let mut periodic = Spline::from_control_points(3, control_points.clone());
    periodic.flags.periodic = true;
    let mut rational = Spline::from_control_points(3, control_points.clone());
    rational.flags.rational = true;
    rational.weights = vec![1.0, 0.5, 2.0, 1.0];
    let mut fit = Spline::from_fit_points(vec![
        Vector3::new(1.0, 1.0, 0.0),
        Vector3::new(4.0, 8.0, 0.0),
        Vector3::new(9.0, -1.0, 0.0),
        Vector3::new(14.0, 6.0, 0.0),
    ]);
    fit.degree = 3;
    let mut linear = Spline::from_control_points(1, control_points.clone());
    linear.flags.linear = true;
    let mut splines = vec![
        Spline::from_control_points(3, control_points),
        closed,
        periodic,
        rational,
        fit,
        linear,
    ];
    let first = Vector3::new(2.5, -3.0, 0.0);
    let second = Vector3::new(13.5, 4.0, 0.0);
    let transform = Transform::from_mirror_line(first, second);
    for spline in &mut splines {
        let original = spline.clone();
        spline.mirror_about_line(first, second);
        for (actual, expected) in spline.control_points.iter().zip(&original.control_points) {
            close(*actual, transform.apply(*expected));
        }
        for (actual, expected) in spline.fit_points.iter().zip(&original.fit_points) {
            close(*actual, transform.apply(*expected));
        }
        assert_eq!(spline.knots, original.knots);
        assert_eq!(spline.weights, original.weights);
        assert_eq!(spline.flags, original.flags);
    }
    assert!(splines[1].flags.closed);
    assert!(splines[2].flags.periodic);
    assert!(splines[3].flags.rational);
    assert!(splines[5].flags.linear);
}

#[test]
fn native_polygon_mesh_closure_and_surface_fit_modes_survive_mirror() {
    let modes = [
        (PolygonMeshFlags::empty(), SurfaceSmoothType::NoSmooth),
        (PolygonMeshFlags::CLOSED_M, SurfaceSmoothType::NoSmooth),
        (PolygonMeshFlags::CLOSED_N, SurfaceSmoothType::NoSmooth),
        (
            PolygonMeshFlags::CLOSED_M | PolygonMeshFlags::CLOSED_N,
            SurfaceSmoothType::NoSmooth,
        ),
        (PolygonMeshFlags::SPLINE_FIT, SurfaceSmoothType::Quadratic),
        (PolygonMeshFlags::SPLINE_FIT, SurfaceSmoothType::Cubic),
        (PolygonMeshFlags::CURVE_FIT, SurfaceSmoothType::Bezier),
    ];
    // Native POLYMESH MIRROR cases use the world Y axis as the mirror line;
    // ACAD reverses each stored M row for this reflection.
    let first = Vector3::ZERO;
    let second = Vector3::UNIT_Y;
    let transform = Transform::from_mirror_line(first, second);
    for (extra_flags, smooth_type) in modes {
        let mut mesh = PolygonMeshEntity::new();
        mesh.m_vertex_count = 3;
        mesh.n_vertex_count = 3;
        mesh.m_smooth_density = 5;
        mesh.n_smooth_density = 6;
        mesh.smooth_type = smooth_type;
        mesh.flags.insert(extra_flags);
        mesh.vertices = (0..3)
            .flat_map(|row| {
                (0..3).map(move |column| {
                    PolygonMeshVertex::at(Vector3::new(
                        2.0 + column as f64 * 6.0,
                        2.0 + row as f64 * 6.0,
                        1.0 + row as f64 + column as f64,
                    ))
                })
            })
            .collect();
        let original = mesh.clone();
        mesh.mirror_about_line(first, second);
        let m = original.m_vertex_count as usize;
        for (index, actual) in mesh.vertices.iter().enumerate() {
            let row = index / m;
            let column = index % m;
            let expected = &original.vertices[row * m + (m - 1 - column)];
            close(actual.location, transform.apply(expected.location));
        }
        assert_eq!(mesh.flags, original.flags);
        assert_eq!(mesh.smooth_type, original.smooth_type);
        assert_eq!(mesh.m_vertex_count, original.m_vertex_count);
        assert_eq!(mesh.n_vertex_count, original.n_vertex_count);
        assert_eq!(mesh.m_smooth_density, original.m_smooth_density);
        assert_eq!(mesh.n_smooth_density, original.n_smooth_density);
    }
}

#[test]
fn native_mirror3d_spline_mesh_and_solid_preserve_world_geometry() {
    let point = Vector3::new(1.0, -2.0, 3.0);
    let normal = Vector3::new(2.0, 1.0, -3.0);
    let transform = Transform::from_mirror_plane(point, normal);

    let mut spline = Spline::from_control_points(
        3,
        vec![
            Vector3::new(2.0, 1.0, 2.0),
            Vector3::new(5.0, 8.0, 5.0),
            Vector3::new(9.0, -2.0, 8.0),
            Vector3::new(14.0, 6.0, 11.0),
        ],
    );
    let spline_original = spline.clone();
    spline.mirror_about_plane(point, normal);
    for (actual, expected) in spline
        .control_points
        .iter()
        .zip(&spline_original.control_points)
    {
        close(*actual, transform.apply(*expected));
    }

    let mut mesh = Mesh::from_quads(
        vec![
            Vector3::new(2.0, 2.0, 2.0),
            Vector3::new(14.0, 2.0, 4.0),
            Vector3::new(14.0, 14.0, 6.0),
            Vector3::new(2.0, 14.0, 3.0),
        ],
        &[(0, 1, 2, 3)],
    );
    mesh.subdivision_level = 2;
    let mesh_original = mesh.clone();
    mesh.mirror_about_plane(point, normal);
    for (actual, expected) in mesh.vertices.iter().zip(&mesh_original.vertices) {
        close(*actual, transform.apply(*expected));
    }
    assert_eq!(mesh.faces, mesh_original.faces);
    assert_eq!(mesh.edges, mesh_original.edges);
    assert_eq!(mesh.subdivision_level, mesh_original.subdivision_level);

    let sat = crate::entities::acis::primitives::build_box([2.0, 3.0, 4.0], 4.0, 6.0, 8.0);
    let mut solid = Solid3D::from_sat(&sat.to_sat_string());
    solid.point_of_reference = Vector3::new(2.0, 3.0, 4.0);
    let before_faces = solid.parse_sat().unwrap().faces().len();
    let before_point = solid.point_of_reference;
    solid.mirror_about_plane(point, normal);
    close(solid.point_of_reference, transform.apply(before_point));
    assert_eq!(solid.parse_sat().unwrap().faces().len(), before_faces);
}

#[test]
fn native_planar_solid3d_mirror_preserves_sat_topology() {
    let sat = crate::entities::acis::primitives::build_box([6.0, 4.0, 2.0], 12.0, 7.0, 3.0);
    let mut solid = Solid3D::from_sat(&sat.to_sat_string());
    solid.point_of_reference = Vector3::new(6.0, 4.0, 2.0);
    let original_point = solid.point_of_reference;
    let before_faces = solid.parse_sat().unwrap().faces().len();
    let first = Vector3::new(2.5, -3.0, 0.0);
    let second = Vector3::new(13.5, 4.0, 0.0);
    let transform = Transform::from_mirror_line(first, second);
    solid.mirror_about_line(first, second);
    close(solid.point_of_reference, transform.apply(original_point));
    assert_eq!(solid.parse_sat().unwrap().faces().len(), before_faces);
}
