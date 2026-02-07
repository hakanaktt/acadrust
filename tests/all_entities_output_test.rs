//! Comprehensive test that generates DXF files (ASCII and Binary) containing all supported entities
//! This test creates sample instances of every entity type supported by the library

use acadrust::entities::*;
use acadrust::types::{Color, Vector2, Vector3};
use acadrust::{CadDocument, DxfWriter};
use std::f64::consts::PI;

/// Create a document with examples of all supported entity types
fn create_all_entities_document() -> CadDocument {
    let mut doc = CadDocument::new();

    // Grid positioning
    let spacing = 20.0;
    let mut x = 0.0;
    let mut y = 0.0;

    // ==================== Basic Geometric Entities ====================

    // 1. Point
    let mut point = Point::new();
    point.location = Vector3::new(x, y, 0.0);
    point.common.color = Color::RED;
    doc.add_entity(EntityType::Point(point)).unwrap();
    x += spacing;

    // 2. Line
    let mut line = Line::from_coords(x, y, 0.0, x + 10.0, y + 10.0, 0.0);
    line.common.color = Color::GREEN;
    doc.add_entity(EntityType::Line(line)).unwrap();
    x += spacing;

    // 3. Circle
    let mut circle = Circle::from_coords(x, y, 0.0, 5.0);
    circle.common.color = Color::BLUE;
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    x += spacing;

    // 4. Arc
    let mut arc = Arc::from_coords(x, y, 0.0, 5.0, 0.0, PI);
    arc.common.color = Color::YELLOW;
    doc.add_entity(EntityType::Arc(arc)).unwrap();
    x += spacing;

    // 5. Ellipse
    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(x, y, 0.0),
        Vector3::new(8.0, 0.0, 0.0),
        0.5,
    );
    ellipse.common.color = Color::CYAN;
    doc.add_entity(EntityType::Ellipse(ellipse)).unwrap();

    // Next row
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
    lwpoly.common.color = Color::MAGENTA;
    doc.add_entity(EntityType::LwPolyline(lwpoly)).unwrap();
    x += spacing;

    // 7. Polyline3D
    let mut poly3d = Polyline3D::new();
    poly3d.add_vertex(Vector3::new(x, y, 0.0));
    poly3d.add_vertex(Vector3::new(x + 5.0, y + 5.0, 5.0));
    poly3d.add_vertex(Vector3::new(x + 10.0, y, 10.0));
    poly3d.common.color = Color::from_rgb(128, 255, 128);
    doc.add_entity(EntityType::Polyline3D(poly3d)).unwrap();
    x += spacing;

    // 8. Spline
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

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Text Entities ====================

    // 9. Text
    let mut text = Text::with_value("Hello DXF", Vector3::new(x, y, 0.0))
        .with_height(2.5);
    text.common.color = Color::RED;
    doc.add_entity(EntityType::Text(text)).unwrap();
    x += spacing;

    // 10. MText
    let mut mtext = MText::new();
    mtext.value = "Multi-line\\PText\\PExample".to_string();
    mtext.insertion_point = Vector3::new(x, y, 0.0);
    mtext.height = 2.5;
    mtext.rectangle_width = 15.0;
    mtext.common.color = Color::BLUE;
    doc.add_entity(EntityType::MText(mtext)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Solid and Face Entities ====================

    // 11. Solid
    let solid = Solid::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x, y + 5.0, 0.0),
    );
    doc.add_entity(EntityType::Solid(solid)).unwrap();
    x += spacing;

    // 12. Face3D
    let mut face3d = Face3D::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 2.0),
        Vector3::new(x, y + 5.0, 2.0),
    );
    face3d.common.color = Color::from_rgb(0, 200, 200);
    doc.add_entity(EntityType::Face3D(face3d)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Construction Entities ====================

    // 13. Ray
    let mut ray = Ray::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 1.0, 0.0));
    ray.common.color = Color::from_rgb(128, 128, 255);
    doc.add_entity(EntityType::Ray(ray)).unwrap();
    x += spacing;

    // 14. XLine
    let mut xline = XLine::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 0.5, 0.0));
    xline.common.color = Color::from_rgb(255, 128, 128);
    doc.add_entity(EntityType::XLine(xline)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Hatch Entity ====================

    // 15. Hatch
    let mut hatch = Hatch::new();
    hatch.pattern = HatchPattern::solid();
    hatch.is_solid = true;
    
    // Create a simple rectangular boundary
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
    hatch.common.color = Color::from_rgb(150, 150, 200);
    doc.add_entity(EntityType::Hatch(hatch)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Block-Related Entities ====================

    // 16. Insert
    let mut insert = Insert::new("TestBlock", Vector3::new(x, y, 0.0));
    insert.x_scale = 1.0;
    insert.y_scale = 1.0;
    insert.z_scale = 1.0;
    insert.rotation = 0.0;
    insert.common.color = Color::from_rgb(255, 200, 100);
    doc.add_entity(EntityType::Insert(insert)).unwrap();
    x += spacing;

    // 17. Attribute Definition
    let mut attdef = AttributeDefinition::new(
        "TAG1".to_string(),
        "Enter value:".to_string(),
        "Default".to_string(),
    );
    attdef.insertion_point = Vector3::new(x, y, 0.0);
    attdef.height = 2.0;
    attdef.common.color = Color::YELLOW;
    doc.add_entity(EntityType::AttributeDefinition(attdef)).unwrap();
    x += spacing;

    // 18. Attribute Entity
    let mut attrib = AttributeEntity::new("TAG2".to_string(), "Value".to_string());
    attrib.insertion_point = Vector3::new(x, y, 0.0);
    attrib.height = 2.0;
    attrib.common.color = Color::CYAN;
    doc.add_entity(EntityType::AttributeEntity(attrib)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Leader Entities ====================

    // 19. Leader
    let mut leader = Leader::new();
    leader.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 3.0, 0.0),
        Vector3::new(x + 8.0, y + 3.0, 0.0),
    ];
    leader.arrow_enabled = true;
    leader.common.color = Color::from_rgb(255, 100, 0);
    doc.add_entity(EntityType::Leader(leader)).unwrap();
    x += spacing;

    // 20. MultiLeader
    let mut multileader = MultiLeaderBuilder::new().build();
    
    let mut root = LeaderRoot::new(0);
    let mut line = LeaderLine::new(0);
    line.points = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
    ];
    root.lines.push(line);
    multileader.context.leader_roots.push(root);
    
    multileader.context.text_location = Vector3::new(x + 5.0, y + 5.0, 0.0);
    multileader.common.color = Color::from_rgb(100, 255, 100);
    doc.add_entity(EntityType::MultiLeader(multileader)).unwrap();
    x += spacing;

    // 21. MLine
    let mut mline = MLineBuilder::new()
        .justification(MLineJustification::Zero)
        .build();
    
    mline.add_vertex(Vector3::new(x, y, 0.0));
    mline.add_vertex(Vector3::new(x + 5.0, y + 5.0, 0.0));
    mline.add_vertex(Vector3::new(x + 10.0, y, 0.0));
    mline.common.color = Color::from_rgb(200, 100, 255);
    doc.add_entity(EntityType::MLine(mline)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Advanced Entities ====================

    // 22. Mesh
    let mut mesh = MeshBuilder::new()
        .subdivision_level(0)
        .build();
    
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
    
    mesh.common.color = Color::from_rgb(255, 200, 200);
    doc.add_entity(EntityType::Mesh(mesh)).unwrap();
    x += spacing;

    // 23. Solid3D
    let mut solid3d = Solid3D::new();
    solid3d.acis_data.sat_data = "Example ACIS solid data".to_string();
    solid3d.common.color = Color::from_rgb(100, 150, 255);
    doc.add_entity(EntityType::Solid3D(solid3d)).unwrap();
    x += spacing;

    // 24. Region
    let mut region = Region::new();
    region.acis_data.sat_data = "Example ACIS region data".to_string();
    region.common.color = Color::from_rgb(150, 255, 100);
    doc.add_entity(EntityType::Region(region)).unwrap();
    x += spacing;

    // 25. Body
    let mut body = Body::new();
    body.acis_data.sat_data = "Example ACIS body data".to_string();
    body.common.color = Color::from_rgb(255, 150, 100);
    doc.add_entity(EntityType::Body(body)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Table and Advanced Entities ====================

    // 26. Table
    let mut table = TableBuilder::new(2, 2).build();
    table.insertion_point = Vector3::new(x, y, 0.0);
    table.horizontal_direction = Vector3::UNIT_X;
    table.common.color = Color::from_rgb(200, 255, 200);
    doc.add_entity(EntityType::Table(table)).unwrap();
    x += spacing;

    // 27. Tolerance
    let mut tolerance = Tolerance::new();
    tolerance.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A{\\Fgdt;m}B".to_string();
    tolerance.insertion_point = Vector3::new(x, y, 0.0);
    tolerance.direction = Vector3::UNIT_X;
    tolerance.common.color = Color::from_rgb(255, 255, 100);
    doc.add_entity(EntityType::Tolerance(tolerance)).unwrap();
    x += spacing;

    // Next row
    x = 0.0;
    y += spacing;

    // ==================== Legacy and Specialized Entities ====================

    // 28. PolyfaceMesh
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
    
    polyface.common.color = Color::from_rgb(100, 100, 200);
    doc.add_entity(EntityType::PolyfaceMesh(polyface)).unwrap();
    x += spacing;

    // 29. Shape
    let mut shape = Shape::new();
    shape.shape_name = "CIRCLE_SHAPE".to_string();
    shape.insertion_point = Vector3::new(x, y, 0.0);
    shape.size = 3.0;
    shape.rotation = 0.0;
    shape.common.color = Color::from_rgb(200, 100, 100);
    doc.add_entity(EntityType::Shape(shape)).unwrap();
    x += spacing;

    // 30. Viewport
    let mut viewport = Viewport::new();
    viewport.center = Vector3::new(x, y, 0.0);
    viewport.width = 10.0;
    viewport.height = 10.0;
    viewport.view_center = Vector3::new(0.0, 0.0, 0.0);
    viewport.view_height = 100.0;
    viewport.common.color = Color::from_rgb(150, 150, 150);
    doc.add_entity(EntityType::Viewport(viewport)).unwrap();

    doc
}

#[test]
fn test_write_all_entities_ascii() {
    let doc = create_all_entities_document();
    
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
    let doc = create_all_entities_document();
    
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
    let doc = create_all_entities_document();
    let entity_count = doc.entity_count();
    
    // We created 30 entities
    assert_eq!(entity_count, 30, "Expected 30 entities, got {}", entity_count);
    
    println!("✓ Document contains {} entities", entity_count);
}

#[test]
fn test_all_entity_types_present() {
    let doc = create_all_entities_document();
    
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
    assert!(type_names.len() >= 20, "Expected at least 20 unique entity types");
}

#[test]
fn test_document_structure() {
    let doc = create_all_entities_document();
    
    // Check document has required structure
    assert!(doc.entity_count() > 0, "Document should have entities");
    
    println!("✓ Document structure:");
    println!("  Version: {:?}", doc.version);
    println!("  Entities: {}", doc.entity_count());
    
    // Verify each entity has a valid handle
    for entity in doc.entities() {
        assert!(!entity.as_entity().handle().is_null(), "Entity should have valid handle");
    }
}

#[test]
fn test_write_all_versions_ascii() {
    use acadrust::types::DxfVersion;
    
    let versions = vec![
        (DxfVersion::AC1012, "R13"),
        (DxfVersion::AC1014, "R14"),
        (DxfVersion::AC1015, "2000"),
        (DxfVersion::AC1018, "2004"),
        (DxfVersion::AC1021, "2007"),
        (DxfVersion::AC1024, "2010"),
        (DxfVersion::AC1027, "2013"),
        (DxfVersion::AC1032, "2018"),
    ];
    
    println!("\n📝 Generating ASCII DXF files for all versions:");
    
    for (version, name) in versions {
        let mut doc = create_all_entities_document();
        doc.version = version;
        
        let filename = format!("test_output_version_{}_ascii.dxf", name);
        let writer = DxfWriter::new(doc.clone());
        let result = writer.write_to_file(&filename);
        
        assert!(result.is_ok(), "Failed to write {} ASCII DXF: {:?}", name, result.err());
        assert!(
            std::path::Path::new(&filename).exists(),
            "File {} was not created", filename
        );
        
        let file_size = std::fs::metadata(&filename).unwrap().len();
        println!("  ✓ {} ({:?}) - {} bytes", name, version, file_size);
    }
    
    println!("✓ Successfully created ASCII DXF files for all 8 versions");
}

#[test]
fn test_write_all_versions_binary() {
    use acadrust::types::DxfVersion;
    
    let versions = vec![
        (DxfVersion::AC1012, "R13"),
        (DxfVersion::AC1014, "R14"),
        (DxfVersion::AC1015, "2000"),
        (DxfVersion::AC1018, "2004"),
        (DxfVersion::AC1021, "2007"),
        (DxfVersion::AC1024, "2010"),
        (DxfVersion::AC1027, "2013"),
        (DxfVersion::AC1032, "2018"),
    ];
    
    println!("\n💾 Generating Binary DXF files for all versions:");
    
    for (version, name) in versions {
        let mut doc = create_all_entities_document();
        doc.version = version;
        
        let filename = format!("test_output_version_{}_binary.dxb", name);
        let writer = DxfWriter::new_binary(doc.clone());
        let result = writer.write_to_file(&filename);
        
        assert!(result.is_ok(), "Failed to write {} Binary DXF: {:?}", name, result.err());
        assert!(
            std::path::Path::new(&filename).exists(),
            "File {} was not created", filename
        );
        
        let file_size = std::fs::metadata(&filename).unwrap().len();
        println!("  ✓ {} ({:?}) - {} bytes", name, version, file_size);
    }
    
    println!("✓ Successfully created Binary DXF files for all 8 versions");
}

