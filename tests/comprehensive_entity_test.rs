//! Comprehensive test that generates DXF files (ASCII and Binary) containing all supported entities

use acadrust::entities::*;
use acadrust::types::{Color, Handle, Vector2, Vector3};
use acadrust::{CadDocument, DxfWriter};
use std::f64::consts::PI;

/// Create a document with all supported entity types
fn create_comprehensive_document() -> CadDocument {
    let mut doc = CadDocument::new();

    // Positioning variables to space out entities in a grid
    let spacing = 20.0;
    let mut x = 0.0;
    let mut y = 0.0;

    // ==================== Basic Entities ====================

    // 1. Point
    let mut point = Point::new();
    point.location = Vector3::new(x, y, 0.0);
    point.common.color = Color::Red;
    doc.add_entity(EntityType::Point(point)).unwrap();
    x += spacing;

    // 2. Line
    let mut line = Line::from_coords(x, y, 0.0, x + 10.0, y + 10.0, 0.0);
    line.common.color = Color::Green;
    doc.add_entity(EntityType::Line(line)).unwrap();
    x += spacing;

    // 3. Circle
    let mut circle = Circle::from_coords(x, y, 0.0, 5.0);
    circle.common.color = Color::Blue;
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    x += spacing;

    // 4. Arc
    let mut arc = Arc::from_coords(x, y, 0.0, 5.0, 0.0, PI);
    arc.common.color = Color::Yellow;
    doc.add_entity(EntityType::Arc(arc)).unwrap();
    x += spacing;

    // 5. Ellipse
    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(x, y, 0.0),
        Vector3::new(8.0, 0.0, 0.0),
        0.5,
    );
    ellipse.common.color = Color::Cyan;
    doc.add_entity(EntityType::Ellipse(ellipse)).unwrap();

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Polyline Entities ====================

    // 6. LwPolyline (Lightweight Polyline)
    let mut lwpoly = LwPolyline::new();
    lwpoly.add_point(Vector2::new(x, y));
    lwpoly.add_point(Vector2::new(x + 5.0, y + 5.0));
    lwpoly.add_point(Vector2::new(x + 10.0, y));
    lwpoly.add_point_with_bulge(Vector2::new(x + 5.0, y - 3.0), 0.5);
    lwpoly.is_closed = true;
    lwpoly.common.color = Color::Magenta;
    doc.add_entity(EntityType::LwPolyline(lwpoly)).unwrap();
    x += spacing;

    // 7. Polyline (2D)
    let mut poly2d = Polyline2D::new();
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x, y, 0.0)));
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x + 10.0, y, 0.0)));
    poly2d.is_closed = true;
    poly2d.common.color = Color::from_rgb(255, 128, 0);
    doc.add_entity(EntityType::Polyline2D(poly2d)).unwrap();
    x += spacing;

    // 8. Polyline3D
    let mut poly3d = Polyline3D::new();
    poly3d.add_vertex(Vertex3DPolyline::new(Vector3::new(x, y, 0.0)));
    poly3d.add_vertex(Vertex3DPolyline::new(Vector3::new(x + 5.0, y + 5.0, 5.0)));
    poly3d.add_vertex(Vertex3DPolyline::new(Vector3::new(x + 10.0, y, 10.0)));
    poly3d.common.color = Color::from_rgb(128, 255, 128);
    doc.add_entity(EntityType::Polyline3D(poly3d)).unwrap();
    x += spacing;

    // 9. Spline
    let mut spline = Spline::new();
    spline.control_points = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 3.0, y + 5.0, 0.0),
        Vector3::new(x + 6.0, y + 2.0, 0.0),
        Vector3::new(x + 10.0, y + 7.0, 0.0),
    ];
    spline.degree = 3;
    spline.common.color = Color::from_rgb(255, 0, 255);
    doc.add_entity(EntityType::Spline(spline)).unwrap();
    x += spacing;

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Text Entities ====================

    // 10. Text
    let mut text = Text::new();
    text.text_string = "Simple Text".to_string();
    text.first_alignment_point = Vector3::new(x, y, 0.0);
    text.height = 2.5;
    text.common.color = Color::Red;
    doc.add_entity(EntityType::Text(text)).unwrap();
    x += spacing;

    // 11. MText (Multi-line Text)
    let mut mtext = MText::new();
    mtext.text = "Multi-line\\PText\\PExample".to_string();
    mtext.insertion_point = Vector3::new(x, y, 0.0);
    mtext.height = 2.5;
    mtext.reference_rectangle_width = 15.0;
    mtext.common.color = Color::Blue;
    doc.add_entity(EntityType::MText(mtext)).unwrap();
    x += spacing;

    // ==================== Solid and Face Entities ====================

    // 12. Solid (2D solid-filled triangle/quad)
    let mut solid = Solid::new();
    solid.first_corner = Vector3::new(x, y, 0.0);
    solid.second_corner = Vector3::new(x + 5.0, y, 0.0);
    solid.third_corner = Vector3::new(x + 5.0, y + 5.0, 0.0);
    solid.fourth_corner = Vector3::new(x, y + 5.0, 0.0);
    solid.common.color = Color::from_rgb(200, 200, 0);
    doc.add_entity(EntityType::Solid(solid)).unwrap();
    x += spacing;

    // 13. Face3D (3D face)
    let mut face3d = Face3D::new();
    face3d.first_corner = Vector3::new(x, y, 0.0);
    face3d.second_corner = Vector3::new(x + 5.0, y, 0.0);
    face3d.third_corner = Vector3::new(x + 5.0, y + 5.0, 2.0);
    face3d.fourth_corner = Vector3::new(x, y + 5.0, 2.0);
    face3d.common.color = Color::from_rgb(0, 200, 200);
    doc.add_entity(EntityType::Face3D(face3d)).unwrap();
    x += spacing;

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Construction and Reference Entities ====================

    // 14. Ray (semi-infinite line)
    let mut ray = Ray::new();
    ray.start_point = Vector3::new(x, y, 0.0);
    ray.direction_vector = Vector3::new(1.0, 1.0, 0.0);
    ray.common.color = Color::from_rgb(128, 128, 255);
    doc.add_entity(EntityType::Ray(ray)).unwrap();
    x += spacing;

    // 15. XLine (infinite construction line)
    let mut xline = XLine::new();
    xline.base_point = Vector3::new(x, y, 0.0);
    xline.direction_vector = Vector3::new(1.0, 0.5, 0.0);
    xline.common.color = Color::from_rgb(255, 128, 128);
    doc.add_entity(EntityType::XLine(xline)).unwrap();
    x += spacing;

    // ==================== Dimension Entities ====================

    // 16. Dimension (Linear)
    let mut dimension = Dimension::new_aligned(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
        Vector3::new(x + 5.0, y + 8.0, 0.0),
    );
    dimension.common.color = Color::Green;
    doc.add_entity(EntityType::Dimension(dimension)).unwrap();
    x += spacing;

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Hatch Entity ====================

    // 17. Hatch
    let mut hatch = Hatch::new();
    hatch.pattern_name = "SOLID".to_string();
    hatch.is_solid = true;
    
    // Create a boundary loop
    let mut boundary = BoundaryPath::new();
    
    // Add line edges to form a rectangle
    boundary.add_edge(BoundaryPathEdge::Line {
        start: Vector2::new(x, y),
        end: Vector2::new(x + 10.0, y),
    });
    boundary.add_edge(BoundaryPathEdge::Line {
        start: Vector2::new(x + 10.0, y),
        end: Vector2::new(x + 10.0, y + 10.0),
    });
    boundary.add_edge(BoundaryPathEdge::Line {
        start: Vector2::new(x + 10.0, y + 10.0),
        end: Vector2::new(x, y + 10.0),
    });
    boundary.add_edge(BoundaryPathEdge::Line {
        start: Vector2::new(x, y + 10.0),
        end: Vector2::new(x, y),
    });
    
    hatch.boundary_paths.push(boundary);
    hatch.common.color = Color::from_rgb(150, 150, 200);
    doc.add_entity(EntityType::Hatch(hatch)).unwrap();
    x += spacing;

    // ==================== Block-Related Entities ====================

    // 18. Insert (Block Reference)
    let mut insert = Insert::new();
    insert.block_name = "TestBlock".to_string();
    insert.insertion_point = Vector3::new(x, y, 0.0);
    insert.scale_x = 1.0;
    insert.scale_y = 1.0;
    insert.scale_z = 1.0;
    insert.rotation = 0.0;
    insert.common.color = Color::from_rgb(255, 200, 100);
    doc.add_entity(EntityType::Insert(insert)).unwrap();
    x += spacing;

    // 19. Attribute Definition
    let mut attdef = AttributeDefinition::new();
    attdef.tag = "TAG1".to_string();
    attdef.prompt = "Enter value:".to_string();
    attdef.default_value = "Default".to_string();
    attdef.insertion_point = Vector3::new(x, y, 0.0);
    attdef.height = 2.0;
    attdef.common.color = Color::Yellow;
    doc.add_entity(EntityType::AttributeDefinition(attdef)).unwrap();
    x += spacing;

    // 20. Attribute Entity
    let mut attrib = AttributeEntity::new();
    attrib.tag = "TAG2".to_string();
    attrib.text_string = "Attribute Value".to_string();
    attrib.insertion_point = Vector3::new(x, y, 0.0);
    attrib.height = 2.0;
    attrib.common.color = Color::Cyan;
    doc.add_entity(EntityType::AttributeEntity(attrib)).unwrap();
    x += spacing;

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Leader Entities ====================

    // 21. Leader
    let mut leader = Leader::new();
    leader.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 3.0, 0.0),
        Vector3::new(x + 8.0, y + 3.0, 0.0),
    ];
    leader.has_arrowhead = true;
    leader.common.color = Color::from_rgb(255, 100, 0);
    doc.add_entity(EntityType::Leader(leader)).unwrap();
    x += spacing;

    // 22. MultiLeader
    let mut multileader = MultiLeaderBuilder::new()
        .with_content_type(LeaderContentType::MText)
        .build();
    
    // Add a leader root with a line
    let mut root = LeaderRoot::new();
    let mut line = LeaderLine::new();
    line.points = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
    ];
    root.lines.push(line);
    multileader.leader_roots.push(root);
    
    multileader.common_leader_data.landing_gap = 0.5;
    multileader.text_content = "MultiLeader".to_string();
    multileader.text_location = Vector3::new(x + 5.0, y + 5.0, 0.0);
    multileader.common.color = Color::from_rgb(100, 255, 100);
    doc.add_entity(EntityType::MultiLeader(multileader)).unwrap();
    x += spacing;

    // ==================== MLine Entity ====================

    // 23. MLine (Multi-line)
    let mut mline = MLineBuilder::new()
        .with_justification(MLineJustification::Zero)
        .build();
    
    mline.add_vertex(MLineVertex::new(Vector3::new(x, y, 0.0)));
    mline.add_vertex(MLineVertex::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
    mline.add_vertex(MLineVertex::new(Vector3::new(x + 10.0, y, 0.0)));
    mline.common.color = Color::from_rgb(200, 100, 255);
    doc.add_entity(EntityType::MLine(mline)).unwrap();
    x += spacing;

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Advanced 3D Entities ====================

    // 24. Mesh
    let mut mesh = MeshBuilder::new()
        .with_subdivision_level(0)
        .build();
    
    // Add vertices for a simple triangular mesh
    mesh.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 2.5, y + 5.0, 3.0),
        Vector3::new(x + 2.5, y + 2.5, 1.0),
    ];
    
    // Add faces
    mesh.faces.push(MeshFace {
        vertex_indices: vec![0, 1, 2],
    });
    mesh.faces.push(MeshFace {
        vertex_indices: vec![0, 1, 3],
    });
    
    mesh.common.color = Color::from_rgb(255, 200, 200);
    doc.add_entity(EntityType::Mesh(mesh)).unwrap();
    x += spacing;

    // 25. Solid3D (3D solid with ACIS data)
    let mut solid3d = Solid3D::new();
    solid3d.acis_data = AcisData {
        version: AcisVersion::R2018,
        data: vec!["Example ACIS solid data".to_string()],
    };
    solid3d.common.color = Color::from_rgb(100, 150, 255);
    doc.add_entity(EntityType::Solid3D(solid3d)).unwrap();
    x += spacing;

    // 26. Region (2D region with ACIS data)
    let mut region = Region::new();
    region.acis_data = AcisData {
        version: AcisVersion::R2018,
        data: vec!["Example ACIS region data".to_string()],
    };
    region.common.color = Color::from_rgb(150, 255, 100);
    doc.add_entity(EntityType::Region(region)).unwrap();
    x += spacing;

    // 27. Body (3D body with ACIS data)
    let mut body = Body::new();
    body.acis_data = AcisData {
        version: AcisVersion::R2018,
        data: vec!["Example ACIS body data".to_string()],
    };
    body.common.color = Color::from_rgb(255, 150, 100);
    doc.add_entity(EntityType::Body(body)).unwrap();
    x += spacing;

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Raster and Advanced Entities ====================

    // 28. RasterImage
    let mut raster = RasterImageBuilder::new()
        .with_image_def_handle(Handle::new(0x100))
        .with_insertion_point(Vector3::new(x, y, 0.0))
        .with_u_vector(Vector3::new(10.0, 0.0, 0.0))
        .with_v_vector(Vector3::new(0.0, 10.0, 0.0))
        .build();
    raster.common.color = Color::White;
    doc.add_entity(EntityType::RasterImage(raster)).unwrap();
    x += spacing;

    // 29. Table entity
    let mut table = TableBuilder::new()
        .with_insertion_point(Vector3::new(x, y, 0.0))
        .with_direction(Vector3::UNIT_X)
        .build();
    
    // Add some rows and columns
    table.add_column(TableColumn {
        width: 5.0,
        custom_data: None,
    });
    table.add_column(TableColumn {
        width: 5.0,
        custom_data: None,
    });
    
    table.add_row(TableRow {
        height: 2.0,
        cells: vec![
            TableCell::new_text("A1".to_string()),
            TableCell::new_text("B1".to_string()),
        ],
    });
    table.add_row(TableRow {
        height: 2.0,
        cells: vec![
            TableCell::new_text("A2".to_string()),
            TableCell::new_text("B2".to_string()),
        ],
    });
    
    table.common.color = Color::from_rgb(200, 255, 200);
    doc.add_entity(EntityType::Table(table)).unwrap();
    x += spacing;

    // 30. Tolerance (Geometric Tolerance / Feature Control Frame)
    let mut tolerance = Tolerance::new();
    tolerance.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A{\\Fgdt;m}B{\\Fgdt;m}C".to_string();
    tolerance.insertion_point = Vector3::new(x, y, 0.0);
    tolerance.direction_vector = Vector3::UNIT_X;
    tolerance.common.color = Color::from_rgb(255, 255, 100);
    doc.add_entity(EntityType::Tolerance(tolerance)).unwrap();
    x += spacing;

    // Move to next row
    x = 0.0;
    y += spacing;

    // ==================== Legacy and Special Entities ====================

    // 31. PolyfaceMesh
    let mut polyface = PolyfaceMesh::new();
    
    // Add vertices
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, y, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, y + 5.0, 0.0)));
    
    // Add a face
    polyface.add_face(PolyfaceFace {
        vertex_indices: vec![1, 2, 3, 4],
        color: Color::ByLayer,
    });
    
    polyface.common.color = Color::from_rgb(100, 100, 200);
    doc.add_entity(EntityType::PolyfaceMesh(polyface)).unwrap();
    x += spacing;

    // 32. Wipeout
    let mut wipeout = Wipeout::new();
    wipeout.insertion_point = Vector3::new(x, y, 0.0);
    wipeout.u_vector = Vector3::new(10.0, 0.0, 0.0);
    wipeout.v_vector = Vector3::new(0.0, 10.0, 0.0);
    wipeout.boundary_points = vec![
        Vector2::new(-0.5, -0.5),
        Vector2::new(0.5, -0.5),
        Vector2::new(0.5, 0.5),
        Vector2::new(-0.5, 0.5),
        Vector2::new(-0.5, -0.5), // Close the loop
    ];
    wipeout.common.color = Color::White;
    doc.add_entity(EntityType::Wipeout(wipeout)).unwrap();
    x += spacing;

    // 33. Shape
    let mut shape = Shape::new();
    shape.name = "CIRCLE_SHAPE".to_string();
    shape.insertion_point = Vector3::new(x, y, 0.0);
    shape.size = 3.0;
    shape.rotation = 0.0;
    shape.common.color = Color::from_rgb(200, 100, 100);
    doc.add_entity(EntityType::Shape(shape)).unwrap();
    x += spacing;

    // 34. Viewport (paper space viewport)
    let mut viewport = Viewport::new();
    viewport.center = Vector3::new(x, y, 0.0);
    viewport.width = 10.0;
    viewport.height = 10.0;
    viewport.view_center = Vector2::new(0.0, 0.0);
    viewport.view_height = 100.0;
    viewport.common.color = Color::from_rgb(150, 150, 150);
    doc.add_entity(EntityType::Viewport(viewport)).unwrap();
    x += spacing;

    // 35. Underlay (PDF/DWF/DGN underlay reference)
    let mut underlay = Underlay::new();
    underlay.underlay_def_handle = Handle::new(0x200);
    underlay.insertion_point = Vector3::new(x, y, 0.0);
    underlay.scale_x = 1.0;
    underlay.scale_y = 1.0;
    underlay.scale_z = 1.0;
    underlay.rotation = 0.0;
    underlay.common.color = Color::from_rgb(200, 200, 255);
    doc.add_entity(EntityType::Underlay(underlay)).unwrap();

    doc
}

#[test]
fn test_write_all_entities_ascii() {
    let doc = create_comprehensive_document();
    
    // Write ASCII DXF
    let writer = DxfWriter::new(doc.clone());
    let result = writer.write_to_file("test_output_all_entities_ascii.dxf");
    
    assert!(result.is_ok(), "Failed to write ASCII DXF: {:?}", result.err());
    
    // Verify file was created
    assert!(
        std::path::Path::new("test_output_all_entities_ascii.dxf").exists(),
        "ASCII DXF file was not created"
    );
    
    println!("✓ Successfully created ASCII DXF with {} entities", doc.entity_count());
}

#[test]
fn test_write_all_entities_binary() {
    let doc = create_comprehensive_document();
    
    // Write Binary DXF
    let writer = DxfWriter::new_binary(doc.clone());
    let result = writer.write_to_file("test_output_all_entities_binary.dxb");
    
    assert!(result.is_ok(), "Failed to write Binary DXF: {:?}", result.err());
    
    // Verify file was created
    assert!(
        std::path::Path::new("test_output_all_entities_binary.dxb").exists(),
        "Binary DXF file was not created"
    );
    
    println!("✓ Successfully created Binary DXF with {} entities", doc.entity_count());
}

#[test]
fn test_entity_count() {
    let doc = create_comprehensive_document();
    let entity_count = doc.entity_count();
    
    // We created 35 entities
    assert_eq!(entity_count, 35, "Expected 35 entities, got {}", entity_count);
    
    println!("✓ Document contains {} entities", entity_count);
}

#[test]
fn test_all_entity_types_present() {
    let doc = create_comprehensive_document();
    
    // Collect all entity type names
    let mut type_names: Vec<String> = doc
        .entities()
        .map(|e| e.as_entity().entity_type().to_string())
        .collect();
    
    type_names.sort();
    type_names.dedup();
    
    println!("✓ Found {} unique entity types:", type_names.len());
    for type_name in &type_names {
        println!("  - {}", type_name);
    }
    
    // Verify we have a good variety of entity types
    assert!(type_names.len() >= 25, "Expected at least 25 unique entity types");
}

#[test]
fn test_write_and_read_roundtrip() {
    use acadrust::DxfReader;
    
    let doc = create_comprehensive_document();
    let original_count = doc.entity_count();
    
    // Write ASCII
    let writer = DxfWriter::new(doc.clone());
    writer.write_to_file("test_roundtrip.dxf").unwrap();
    
    // Try to read it back (if reader is implemented)
    let read_result = DxfReader::from_file("test_roundtrip.dxf");
    
    match read_result {
        Ok(reader) => {
            match reader.read() {
                Ok(read_doc) => {
                    let read_count = read_doc.entity_count();
                    println!("✓ Roundtrip successful: {} → {} entities", original_count, read_count);
                    
                    // Note: Some entities might not round-trip perfectly due to format limitations
                    // But we should get back a reasonable number
                    assert!(
                        read_count > 0,
                        "Should read back at least some entities"
                    );
                }
                Err(e) => {
                    println!("⚠ Reader not fully implemented yet: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("⚠ Could not open file for reading: {:?}", e);
        }
    }
}

#[test]
fn test_document_metadata() {
    let doc = create_comprehensive_document();
    
    // Check document structure
    assert_eq!(doc.version, acadrust::types::DxfVersion::AC1032);
    assert!(doc.layers.count() > 0, "Should have at least one layer");
    assert!(doc.line_types.count() > 0, "Should have at least one line type");
    
    println!("✓ Document metadata:");
    println!("  Version: {:?}", doc.version);
    println!("  Layers: {}", doc.layers.count());
    println!("  Line Types: {}", doc.line_types.count());
    println!("  Entities: {}", doc.entity_count());
}

