//! Test document builders — consolidated from multiple test files.
//!
//! The `create_all_entities_document()` function produces a document containing
//! one instance of every entity type the library supports, laid out on a grid
//! for visual inspection.

#![allow(dead_code)]

use acadrust::entities::*;
use acadrust::objects::{MLineStyle, ObjectType};
use acadrust::types::{Color, Vector2, Vector3};
use acadrust::{BlockRecord, CadDocument, TableEntry};
use std::f64::consts::PI;

/// Create a document containing every supported entity type (44 entities),
/// laid out in a visible grid with 25-unit spacing.
///
/// This is the canonical "all entities" builder used by roundtrip, comparison,
/// and output tests. Any new entity type should be added here.
pub fn create_all_entities_document() -> CadDocument {
    let mut doc = CadDocument::new();
    let sp = 25.0;
    let mut x = 0.0;
    let mut y = 0.0;

    // Row 1 — basic geometry
    // 1. Point
    let mut point = Point::new();
    point.location = Vector3::new(x, y, 0.0);
    point.common.color = Color::RED;
    doc.add_entity(EntityType::Point(point)).unwrap();
    x += sp;

    // 2. Line
    let mut line = Line::from_coords(x, y, 0.0, x + 10.0, y + 10.0, 0.0);
    line.common.color = Color::GREEN;
    doc.add_entity(EntityType::Line(line)).unwrap();
    x += sp;

    // 3. Circle
    let mut circle = Circle::from_coords(x, y, 0.0, 5.0);
    circle.common.color = Color::BLUE;
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    x += sp;

    // 4. Arc
    let mut arc = Arc::from_coords(x, y, 0.0, 5.0, 0.0, PI);
    arc.common.color = Color::YELLOW;
    doc.add_entity(EntityType::Arc(arc)).unwrap();
    x += sp;

    // 5. Ellipse
    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(x, y, 0.0),
        Vector3::new(8.0, 0.0, 0.0),
        0.5,
    );
    ellipse.common.color = Color::CYAN;
    doc.add_entity(EntityType::Ellipse(ellipse)).unwrap();

    // Row 2 — polylines
    x = 0.0;
    y += sp;

    // 6. LwPolyline (closed with bulge)
    let mut lwpoly = LwPolyline::new();
    lwpoly.add_point(Vector2::new(x, y));
    lwpoly.add_point(Vector2::new(x + 5.0, y + 5.0));
    lwpoly.add_point(Vector2::new(x + 10.0, y));
    lwpoly.add_point_with_bulge(Vector2::new(x + 5.0, y - 3.0), 0.5);
    lwpoly.is_closed = true;
    lwpoly.common.color = Color::MAGENTA;
    doc.add_entity(EntityType::LwPolyline(lwpoly)).unwrap();
    x += sp;

    // 7. Polyline2D
    let mut poly2d = Polyline2D::new();
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x, y, 0.0)));
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x + 10.0, y, 0.0)));
    poly2d.close();
    poly2d.common.color = Color::from_rgb(255, 128, 0);
    doc.add_entity(EntityType::Polyline2D(poly2d)).unwrap();
    x += sp;

    // 8. Polyline (legacy 3D)
    let poly = Polyline::from_points(vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 2.0),
        Vector3::new(x + 10.0, y, 5.0),
    ]);
    doc.add_entity(EntityType::Polyline(poly)).unwrap();
    x += sp;

    // 9. Polyline3D
    let mut poly3d = Polyline3D::new();
    poly3d.add_vertex(Vector3::new(x, y, 0.0));
    poly3d.add_vertex(Vector3::new(x + 5.0, y + 5.0, 5.0));
    poly3d.add_vertex(Vector3::new(x + 10.0, y, 10.0));
    poly3d.common.color = Color::from_rgb(128, 255, 128);
    doc.add_entity(EntityType::Polyline3D(poly3d)).unwrap();
    x += sp;

    // 10. Spline
    let mut spline = Spline::new();
    spline.control_points = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 3.0, y + 5.0, 0.0),
        Vector3::new(x + 6.0, y + 2.0, 0.0),
        Vector3::new(x + 10.0, y + 7.0, 0.0),
    ];
    spline.degree = 3;
    spline.knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    doc.add_entity(EntityType::Spline(spline)).unwrap();

    // Row 3 — text
    x = 0.0;
    y += sp;

    // 11. Text
    let mut text = Text::with_value("Hello DXF Writer", Vector3::new(x, y, 0.0)).with_height(2.5);
    text.common.color = Color::RED;
    doc.add_entity(EntityType::Text(text)).unwrap();
    x += sp;

    // 12. MText
    let mut mtext = MText::new();
    mtext.value = "Multi-line\\PText\\PExample".to_string();
    mtext.insertion_point = Vector3::new(x, y, 0.0);
    mtext.height = 2.5;
    mtext.rectangle_width = 15.0;
    mtext.common.color = Color::BLUE;
    doc.add_entity(EntityType::MText(mtext)).unwrap();

    // Row 4 — solids / faces
    x = 0.0;
    y += sp;

    // 13. Solid
    let solid = Solid::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x, y + 5.0, 0.0),
    );
    doc.add_entity(EntityType::Solid(solid)).unwrap();
    x += sp;

    // 14. Face3D
    let face3d = Face3D::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 2.0),
        Vector3::new(x, y + 5.0, 2.0),
    );
    doc.add_entity(EntityType::Face3D(face3d)).unwrap();

    // Row 5 — construction
    x = 0.0;
    y += sp;

    // 15. Ray
    let ray = Ray::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 1.0, 0.0));
    doc.add_entity(EntityType::Ray(ray)).unwrap();
    x += sp;

    // 16. XLine
    let xline = XLine::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 0.5, 0.0));
    doc.add_entity(EntityType::XLine(xline)).unwrap();

    // Row 6 — dimensions (7 types)
    x = 0.0;
    y += sp;

    // 17. DimensionAligned
    let dim_al = Dimension::Aligned(DimensionAligned::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_al)).unwrap();
    x += sp;

    // 18. DimensionLinear
    let dim_lin = Dimension::Linear(DimensionLinear::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_lin)).unwrap();
    x += sp;

    // 19. DimensionRadius
    let dim_rad = Dimension::Radius(DimensionRadius::new(
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_rad)).unwrap();
    x += sp;

    // 20. DimensionDiameter
    let dim_dia = Dimension::Diameter(DimensionDiameter::new(
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_dia)).unwrap();
    x += sp;

    // 21. DimensionAngular2Ln
    let dim_a2 = Dimension::Angular2Ln(DimensionAngular2Ln::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_a2)).unwrap();

    x = 0.0;
    y += sp;

    // 22. DimensionAngular3Pt
    let dim_a3 = Dimension::Angular3Pt(DimensionAngular3Pt::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_a3)).unwrap();
    x += sp;

    // 23. DimensionOrdinate
    let dim_ord = Dimension::Ordinate(DimensionOrdinate::x_ordinate(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 3.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_ord)).unwrap();

    // Row 7 — hatch
    x = 0.0;
    y += sp;

    // 24. Hatch (solid fill)
    let mut hatch = Hatch::new();
    hatch.pattern = HatchPattern::solid();
    hatch.is_solid = true;
    let mut boundary = BoundaryPath::new();
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x, y),
        end: Vector2::new(x + 10.0, y),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x + 10.0, y),
        end: Vector2::new(x + 10.0, y + 10.0),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x + 10.0, y + 10.0),
        end: Vector2::new(x, y + 10.0),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x, y + 10.0),
        end: Vector2::new(x, y),
    }));
    hatch.paths.push(boundary);
    doc.add_entity(EntityType::Hatch(hatch)).unwrap();

    // Row 8 — block reference & attributes
    x = 0.0;
    y += sp;

    // 25. Insert — create a valid block record for "TestBlock" first
    let mut test_block = BlockRecord::new("TestBlock");
    test_block.set_handle(doc.allocate_handle());
    test_block.block_entity_handle = doc.allocate_handle();
    test_block.block_end_handle = doc.allocate_handle();
    doc.block_records.add(test_block).ok();

    let mut insert = Insert::new("TestBlock", Vector3::new(x, y, 0.0));
    insert.x_scale = 1.0;
    insert.y_scale = 1.0;
    insert.z_scale = 1.0;
    doc.add_entity(EntityType::Insert(insert)).unwrap();
    x += sp;

    // 26. AttributeDefinition
    let mut attdef = AttributeDefinition::new(
        "MYTAG".to_string(),
        "Enter value:".to_string(),
        "Default".to_string(),
    );
    attdef.insertion_point = Vector3::new(x, y, 0.0);
    attdef.height = 2.0;
    doc.add_entity(EntityType::AttributeDefinition(attdef))
        .unwrap();

    // 27. (ATTRIB skipped — only valid as INSERT sub-entity)

    // Row 9 — leaders
    x = 0.0;
    y += sp;

    // 28. Leader
    let mut leader = Leader::new();
    leader.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 3.0, 0.0),
        Vector3::new(x + 8.0, y + 3.0, 0.0),
    ];
    leader.arrow_enabled = true;
    leader.creation_type = LeaderCreationType::NoAnnotation;
    doc.add_entity(EntityType::Leader(leader)).unwrap();
    x += sp;

    // 29. MultiLeader
    let mut mleader = MultiLeaderBuilder::new()
        .text("Sample MLeader", Vector3::new(x + 8.0, y + 6.0, 0.0))
        .leader_line(vec![
            Vector3::new(x, y, 0.0),
            Vector3::new(x + 5.0, y + 5.0, 0.0),
        ])
        .build();
    mleader.line_type_handle = Some(doc.header.bylayer_linetype_handle);
    doc.add_entity(EntityType::MultiLeader(mleader)).unwrap();
    x += sp;

    // 30. MLine — create Standard MLineStyle in OBJECTS
    let mut ml_style = MLineStyle::standard();
    ml_style.handle = doc.allocate_handle();
    let ml_style_handle = ml_style.handle;
    doc.objects
        .insert(ml_style.handle, ObjectType::MLineStyle(ml_style));

    let mut mline = MLineBuilder::new()
        .justification(MLineJustification::Zero)
        .vertex(Vector3::new(x, y, 0.0))
        .vertex(Vector3::new(x + 5.0, y + 5.0, 0.0))
        .vertex(Vector3::new(x + 10.0, y, 0.0))
        .build();
    mline.style_handle = Some(ml_style_handle);
    doc.add_entity(EntityType::MLine(mline)).unwrap();

    // Row 10 — mesh
    x = 0.0;
    y += sp;

    // 31. Mesh
    let mut mesh = MeshBuilder::new().subdivision_level(0).build();
    mesh.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 2.5, y + 5.0, 3.0),
        Vector3::new(x + 2.5, y + 2.5, 1.0),
    ];
    mesh.faces.push(MeshFace {
        vertices: vec![0, 1, 2],
    });
    mesh.faces.push(MeshFace {
        vertices: vec![0, 1, 3],
    });
    doc.add_entity(EntityType::Mesh(mesh)).unwrap();

    // Row 11 — table / tolerance
    x = 0.0;
    y += sp;

    // 32. Table — create anonymous block record
    let mut table_block = BlockRecord::new("*T1");
    table_block.set_handle(doc.allocate_handle());
    table_block.block_entity_handle = doc.allocate_handle();
    table_block.block_end_handle = doc.allocate_handle();
    let table_block_handle = table_block.handle();
    doc.block_records.add(table_block).ok();

    let mut table = TableBuilder::new(2, 2).build();
    table.insertion_point = Vector3::new(x, y, 0.0);
    table.horizontal_direction = Vector3::UNIT_X;
    table.block_record_handle = Some(table_block_handle);
    doc.add_entity(EntityType::Table(table)).unwrap();
    x += sp;

    // 33. Tolerance
    let mut tolerance = Tolerance::new();
    tolerance.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A".to_string();
    tolerance.insertion_point = Vector3::new(x, y, 0.0);
    tolerance.direction = Vector3::UNIT_X;
    doc.add_entity(EntityType::Tolerance(tolerance)).unwrap();
    x += sp;

    // 34. RasterImage
    let raster = RasterImage::new("sample.png", Vector3::new(x, y, 0.0), 640.0, 480.0);
    doc.add_entity(EntityType::RasterImage(raster)).unwrap();

    // Row 12 — polyface, wipeout, shape, viewport, underlay
    x = 0.0;
    y += sp;

    // 35. PolyfaceMesh
    let mut polyface = PolyfaceMesh::new();
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, y, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, y + 5.0, 0.0)));
    polyface.add_face(PolyfaceFace {
        common: EntityCommon::new(),
        flags: PolyfaceVertexFlags::default(),
        index1: 1,
        index2: 2,
        index3: 3,
        index4: 4,
        color: Some(Color::ByLayer),
    });
    doc.add_entity(EntityType::PolyfaceMesh(polyface)).unwrap();
    x += sp;

    // 36. Wipeout
    let wipeout = Wipeout::rectangular(Vector3::new(x, y, 0.0), 10.0, 10.0);
    doc.add_entity(EntityType::Wipeout(wipeout)).unwrap();
    x += sp;

    // 37. (Shape skipped — requires valid .shx file)
    x += sp;

    // 38. Viewport
    let mut viewport = Viewport::new();
    viewport.center = Vector3::new(x, y, 0.0);
    viewport.width = 10.0;
    viewport.height = 10.0;
    viewport.view_center = Vector3::new(0.0, 0.0, 0.0);
    viewport.view_height = 100.0;
    doc.add_entity(EntityType::Viewport(viewport)).unwrap();
    x += sp;

    // 39. Underlay (PDF)
    let mut underlay = Underlay::pdf();
    underlay.insertion_point = Vector3::new(x, y, 0.0);
    underlay.x_scale = 1.0;
    underlay.y_scale = 1.0;
    underlay.z_scale = 1.0;
    doc.add_entity(EntityType::Underlay(underlay)).unwrap();

    // Row 13 — OLE2Frame, PolygonMesh
    x = 0.0;
    y += sp;

    // 40. Ole2Frame
    let mut ole = Ole2Frame::new();
    ole.source_application = "TestApp".to_string();
    ole.upper_left_corner = Vector3::new(x, y + 5.0, 0.0);
    ole.lower_right_corner = Vector3::new(x + 10.0, y, 0.0);
    doc.add_entity(EntityType::Ole2Frame(ole)).unwrap();
    x += sp;

    // 41. PolygonMesh (M×N surface grid)
    let mut pgmesh = PolygonMeshEntity::new();
    pgmesh.m_vertex_count = 3;
    pgmesh.n_vertex_count = 3;
    for i in 0..3 {
        for j in 0..3 {
            pgmesh.vertices.push(PolygonMeshVertex::at(Vector3::new(
                x + i as f64 * 5.0,
                y + j as f64 * 5.0,
                (i + j) as f64,
            )));
        }
    }
    doc.add_entity(EntityType::PolygonMesh(pgmesh)).unwrap();

    doc
}

/// Create a minimal document with a single entity of the given type.
///
/// Returns `None` if the entity name is not recognized.
pub fn create_single_entity_doc(entity_name: &str) -> Option<CadDocument> {
    let mut doc = CadDocument::new();
    let x = 10.0;
    let y = 10.0;

    match entity_name {
        "POINT" => {
            let mut p = Point::new();
            p.location = Vector3::new(x, y, 0.0);
            p.common.color = Color::RED;
            doc.add_entity(EntityType::Point(p)).ok()?;
        }
        "LINE" => {
            let l = Line::from_coords(x, y, 0.0, x + 10.0, y + 10.0, 0.0);
            doc.add_entity(EntityType::Line(l)).ok()?;
        }
        "CIRCLE" => {
            let c = Circle::from_coords(x, y, 0.0, 5.0);
            doc.add_entity(EntityType::Circle(c)).ok()?;
        }
        "ARC" => {
            let a = Arc::from_coords(x, y, 0.0, 5.0, 0.0, PI);
            doc.add_entity(EntityType::Arc(a)).ok()?;
        }
        "ELLIPSE" => {
            let e = Ellipse::from_center_axes(
                Vector3::new(x, y, 0.0),
                Vector3::new(8.0, 0.0, 0.0),
                0.5,
            );
            doc.add_entity(EntityType::Ellipse(e)).ok()?;
        }
        "TEXT" => {
            let t = Text::with_value("Test", Vector3::new(x, y, 0.0)).with_height(2.5);
            doc.add_entity(EntityType::Text(t)).ok()?;
        }
        "MTEXT" => {
            let mut m = MText::new();
            m.value = "Multi-line\\PTest".to_string();
            m.insertion_point = Vector3::new(x, y, 0.0);
            m.height = 2.5;
            m.rectangle_width = 15.0;
            doc.add_entity(EntityType::MText(m)).ok()?;
        }
        "LWPOLYLINE" => {
            let mut lw = LwPolyline::new();
            lw.add_point(Vector2::new(x, y));
            lw.add_point(Vector2::new(x + 5.0, y + 5.0));
            lw.add_point(Vector2::new(x + 10.0, y));
            lw.is_closed = true;
            doc.add_entity(EntityType::LwPolyline(lw)).ok()?;
        }
        "SOLID" => {
            let s = Solid::new(
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y, 0.0),
                Vector3::new(x + 5.0, y + 5.0, 0.0),
                Vector3::new(x, y + 5.0, 0.0),
            );
            doc.add_entity(EntityType::Solid(s)).ok()?;
        }
        "3DFACE" => {
            let f = Face3D::new(
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y, 0.0),
                Vector3::new(x + 5.0, y + 5.0, 2.0),
                Vector3::new(x, y + 5.0, 2.0),
            );
            doc.add_entity(EntityType::Face3D(f)).ok()?;
        }
        "RAY" => {
            let r = Ray::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 1.0, 0.0));
            doc.add_entity(EntityType::Ray(r)).ok()?;
        }
        "XLINE" => {
            let xl = XLine::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 0.5, 0.0));
            doc.add_entity(EntityType::XLine(xl)).ok()?;
        }
        "LEADER" => {
            let mut l = Leader::new();
            l.vertices = vec![
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y + 3.0, 0.0),
                Vector3::new(x + 8.0, y + 3.0, 0.0),
            ];
            l.arrow_enabled = true;
            l.creation_type = LeaderCreationType::NoAnnotation;
            doc.add_entity(EntityType::Leader(l)).ok()?;
        }
        "TOLERANCE" => {
            let mut t = Tolerance::new();
            t.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A".to_string();
            t.insertion_point = Vector3::new(x, y, 0.0);
            t.direction = Vector3::UNIT_X;
            doc.add_entity(EntityType::Tolerance(t)).ok()?;
        }
        "SPLINE" => {
            let mut s = Spline::new();
            s.control_points = vec![
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 3.0, y + 5.0, 0.0),
                Vector3::new(x + 6.0, y + 2.0, 0.0),
                Vector3::new(x + 10.0, y + 7.0, 0.0),
            ];
            s.degree = 3;
            s.knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
            doc.add_entity(EntityType::Spline(s)).ok()?;
        }
        _ => return None,
    }

    Some(doc)
}
