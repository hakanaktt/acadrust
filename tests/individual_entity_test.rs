//! Individual entity testing - generates one file per entity type
//! This helps identify which entities cause compatibility issues

mod common;

use acadrust::entities::*;
use acadrust::types::{Color, Vector2, Vector3};
use acadrust::{CadDocument, DxfWriter};
use std::f64::consts::PI;

/// Create a document with a single entity type
fn create_single_entity_doc(entity_name: &str) -> Option<CadDocument> {
    let mut doc = CadDocument::new();
    let x = 10.0;
    let y = 10.0;

    match entity_name {
        "POINT" => {
            let mut point = Point::new();
            point.location = Vector3::new(x, y, 0.0);
            point.common.color = Color::RED;
            doc.add_entity(EntityType::Point(point)).ok()?;
        }
        "LINE" => {
            let line = Line::from_coords(x, y, 0.0, x + 10.0, y + 10.0, 0.0);
            doc.add_entity(EntityType::Line(line)).ok()?;
        }
        "CIRCLE" => {
            let circle = Circle::from_coords(x, y, 0.0, 5.0);
            doc.add_entity(EntityType::Circle(circle)).ok()?;
        }
        "ARC" => {
            let arc = Arc::from_coords(x, y, 0.0, 5.0, 0.0, PI);
            doc.add_entity(EntityType::Arc(arc)).ok()?;
        }
        "ELLIPSE" => {
            let ellipse = Ellipse::from_center_axes(
                Vector3::new(x, y, 0.0),
                Vector3::new(8.0, 0.0, 0.0),
                0.5,
            );
            doc.add_entity(EntityType::Ellipse(ellipse)).ok()?;
        }
        "LWPOLYLINE" => {
            let mut lwpoly = LwPolyline::new();
            // First vertex - straight to second
            lwpoly.add_point(Vector2::new(x, y));
            // Second vertex - arc to third (bulge = 1 creates a semicircle)
            let mut v2 = LwVertex::new(Vector2::new(x + 5.0, y + 5.0));
            v2.bulge = -0.5; // Creates a curved arc segment
            lwpoly.vertices.push(v2);
            // Third vertex - straight back to start
            lwpoly.add_point(Vector2::new(x + 10.0, y));
            lwpoly.is_closed = true;
            doc.add_entity(EntityType::LwPolyline(lwpoly)).ok()?;
        }
        "POLYLINE3D" => {
            let mut poly3d = Polyline3D::new();
            poly3d.add_vertex(Vector3::new(x, y, 0.0));
            poly3d.add_vertex(Vector3::new(x + 5.0, y + 5.0, 5.0));
            poly3d.add_vertex(Vector3::new(x + 10.0, y, 10.0));
            doc.add_entity(EntityType::Polyline3D(poly3d)).ok()?;
        }
        "SPLINE" => {
            let mut spline = Spline::new();
            spline.control_points = vec![
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 3.0, y + 5.0, 0.0),
                Vector3::new(x + 6.0, y + 2.0, 0.0),
                Vector3::new(x + 10.0, y + 7.0, 0.0),
            ];
            spline.degree = 3;
            // Add proper knot vector for a cubic spline with 4 control points
            // Knot vector length should be: degree + control_points.len() + 1 = 3 + 4 + 1 = 8
            spline.knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
            doc.add_entity(EntityType::Spline(spline)).ok()?;
        }
        "TEXT" => {
            let text = Text::with_value("Test", Vector3::new(x, y, 0.0))
                .with_height(2.5);
            doc.add_entity(EntityType::Text(text)).ok()?;
        }
        "MTEXT" => {
            let mut mtext = MText::new();
            mtext.value = "Multi\\PLine".to_string();
            mtext.insertion_point = Vector3::new(x, y, 0.0);
            mtext.height = 2.5;
            doc.add_entity(EntityType::MText(mtext)).ok()?;
        }
        "SOLID" => {
            let solid = Solid::new(
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y, 0.0),
                Vector3::new(x + 5.0, y + 5.0, 0.0),
                Vector3::new(x, y + 5.0, 0.0),
            );
            doc.add_entity(EntityType::Solid(solid)).ok()?;
        }
        "FACE3D" => {
            let face3d = Face3D::new(
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y, 0.0),
                Vector3::new(x + 5.0, y + 5.0, 2.0),
                Vector3::new(x, y + 5.0, 2.0),
            );
            doc.add_entity(EntityType::Face3D(face3d)).ok()?;
        }
        "RAY" => {
            let ray = Ray::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 1.0, 0.0));
            doc.add_entity(EntityType::Ray(ray)).ok()?;
        }
        "XLINE" => {
            let xline = XLine::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 0.5, 0.0));
            doc.add_entity(EntityType::XLine(xline)).ok()?;
        }
        "HATCH" => {
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
            doc.add_entity(EntityType::Hatch(hatch)).ok()?;
        }
        "INSERT" => {
            let insert = Insert::new("TestBlock", Vector3::new(x, y, 0.0));
            doc.add_entity(EntityType::Insert(insert)).ok()?;
        }
        "ATTDEF" => {
            let attdef = AttributeDefinition::new(
                "TAG".to_string(),
                "Prompt".to_string(),
                "Default".to_string(),
            );
            doc.add_entity(EntityType::AttributeDefinition(attdef)).ok()?;
        }
        "ATTRIB" => {
            let attrib = AttributeEntity::new("TAG".to_string(), "Value".to_string());
            doc.add_entity(EntityType::AttributeEntity(attrib)).ok()?;
        }
        "LEADER" => {
            let mut leader = Leader::new();
            leader.vertices = vec![
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y + 3.0, 0.0),
                Vector3::new(x + 8.0, y + 3.0, 0.0),
            ];
            doc.add_entity(EntityType::Leader(leader)).ok()?;
        }
        "MULTILEADER" => {
            let mut multileader = MultiLeaderBuilder::new().build();
            let mut root = LeaderRoot::new(0);
            let mut line = LeaderLine::new(0);
            line.points = vec![
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y + 5.0, 0.0),
            ];
            root.lines.push(line);
            multileader.context.leader_roots.push(root);
            doc.add_entity(EntityType::MultiLeader(multileader)).ok()?;
        }
        "MLINE" => {
            let mut mline = MLineBuilder::new()
                .justification(MLineJustification::Zero)
                .build();
            mline.add_vertex(Vector3::new(x, y, 0.0));
            mline.add_vertex(Vector3::new(x + 5.0, y + 5.0, 0.0));
            doc.add_entity(EntityType::MLine(mline)).ok()?;
        }
        "MESH" => {
            let mut mesh = MeshBuilder::new()
                .subdivision_level(0)
                .build();
            mesh.vertices = vec![
                Vector3::new(x, y, 0.0),
                Vector3::new(x + 5.0, y, 0.0),
                Vector3::new(x + 2.5, y + 5.0, 3.0),
            ];
            mesh.faces.push(MeshFace {
                vertices: vec![0, 1, 2],
            });
            doc.add_entity(EntityType::Mesh(mesh)).ok()?;
        }
        "SOLID3D" => {
            let mut solid3d = Solid3D::new();
            solid3d.acis_data.sat_data = "ACIS data".to_string();
            doc.add_entity(EntityType::Solid3D(solid3d)).ok()?;
        }
        "REGION" => {
            let mut region = Region::new();
            region.acis_data.sat_data = "ACIS data".to_string();
            doc.add_entity(EntityType::Region(region)).ok()?;
        }
        "BODY" => {
            let mut body = Body::new();
            body.acis_data.sat_data = "ACIS data".to_string();
            doc.add_entity(EntityType::Body(body)).ok()?;
        }
        "TABLE" => {
            let mut table = TableBuilder::new(2, 2).build();
            table.insertion_point = Vector3::new(x, y, 0.0);
            doc.add_entity(EntityType::Table(table)).ok()?;
        }
        "TOLERANCE" => {
            let mut tolerance = Tolerance::new();
            tolerance.text = "GD&T".to_string();
            tolerance.insertion_point = Vector3::new(x, y, 0.0);
            doc.add_entity(EntityType::Tolerance(tolerance)).ok()?;
        }
        "POLYFACE" => {
            let mut polyface = PolyfaceMesh::new();
            // Add vertices
            polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, y, 0.0)));
            polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y, 0.0)));
            polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
            // Add a face referencing the vertices (1-based indices)
            polyface.add_face(PolyfaceFace::triangle(1, 2, 3));
            doc.add_entity(EntityType::PolyfaceMesh(polyface)).ok()?;
        }
        "SHAPE" => {
            let mut shape = Shape::new();
            shape.shape_name = "SHAPE1".to_string();
            shape.insertion_point = Vector3::new(x, y, 0.0);
            doc.add_entity(EntityType::Shape(shape)).ok()?;
        }
        "VIEWPORT" => {
            let mut viewport = Viewport::new();
            viewport.center = Vector3::new(x, y, 0.0);
            viewport.width = 10.0;
            viewport.height = 10.0;
            doc.add_entity(EntityType::Viewport(viewport)).ok()?;
        }
        _ => return None,
    }

    Some(doc)
}

#[test]
fn test_individual_entities_r13() {
    use acadrust::types::DxfVersion;
    
    let entities = vec![
        "POINT", "LINE", "CIRCLE", "ARC", "ELLIPSE", "LWPOLYLINE", "POLYLINE3D",
        "SPLINE", "TEXT", "MTEXT", "SOLID", "FACE3D", "RAY", "XLINE", "HATCH",
        "INSERT", "ATTDEF", "ATTRIB", "LEADER", "MULTILEADER", "MLINE", "MESH",
        "SOLID3D", "REGION", "BODY", "TABLE", "TOLERANCE", "POLYFACE", "SHAPE", "VIEWPORT",
    ];
    
    println!("\n🔍 Testing individual entities for R13 (AC1012):\n");
    
    let mut success_count = 0;
    let mut fail_count = 0;
    
    for entity_name in entities {
        if let Some(mut doc) = create_single_entity_doc(entity_name) {
            doc.version = DxfVersion::AC1012;
            
            let filename = format!("entity_R13_{}.dxf", entity_name);
            let writer = DxfWriter::new(doc);
            let result = writer.write_to_file(&filename);
            
            match result {
                Ok(_) => {
                    let size = std::fs::metadata(&filename).unwrap().len();
                    println!("  ✓ {} - {} bytes", entity_name, size);
                    success_count += 1;
                }
                Err(e) => {
                    println!("  ✗ {} - ERROR: {:?}", entity_name, e);
                    fail_count += 1;
                }
            }
        }
    }
    
    println!("\n📊 Results: {} success, {} failed", success_count, fail_count);
}

#[test]
fn test_individual_entities_r14() {
    use acadrust::types::DxfVersion;
    
    let entities = vec![
        "POINT", "LINE", "CIRCLE", "ARC", "ELLIPSE", "LWPOLYLINE", "POLYLINE3D",
        "SPLINE", "TEXT", "MTEXT", "SOLID", "FACE3D", "RAY", "XLINE", "HATCH",
        "INSERT", "ATTDEF", "ATTRIB", "LEADER", "MULTILEADER", "MLINE", "MESH",
        "SOLID3D", "REGION", "BODY", "TABLE", "TOLERANCE", "POLYFACE", "SHAPE", "VIEWPORT",
    ];
    
    println!("\n🔍 Testing individual entities for R14 (AC1014):\n");
    
    let mut success_count = 0;
    let mut fail_count = 0;
    
    for entity_name in entities {
        if let Some(mut doc) = create_single_entity_doc(entity_name) {
            doc.version = DxfVersion::AC1014;
            
            let filename = format!("entity_R14_{}.dxf", entity_name);
            let writer = DxfWriter::new(doc);
            let result = writer.write_to_file(&filename);
            
            match result {
                Ok(_) => {
                    let size = std::fs::metadata(&filename).unwrap().len();
                    println!("  ✓ {} - {} bytes", entity_name, size);
                    success_count += 1;
                }
                Err(e) => {
                    println!("  ✗ {} - ERROR: {:?}", entity_name, e);
                    fail_count += 1;
                }
            }
        }
    }
    
    println!("\n📊 Results: {} success, {} failed", success_count, fail_count);
}

#[test]
fn test_individual_entities_2010() {
    use acadrust::types::DxfVersion;
    
    let entities = vec![
        "POINT", "LINE", "CIRCLE", "ARC", "ELLIPSE", "LWPOLYLINE", "POLYLINE3D",
        "SPLINE", "TEXT", "MTEXT", "SOLID", "FACE3D", "RAY", "XLINE", "HATCH",
        "INSERT", "ATTDEF", "ATTRIB", "LEADER", "MULTILEADER", "MLINE", "MESH",
        "SOLID3D", "REGION", "BODY", "TABLE", "TOLERANCE", "POLYFACE", "SHAPE", "VIEWPORT",
    ];
    
    println!("\n🔍 Testing individual entities for 2010 (AC1024):\n");
    
    let mut success_count = 0;
    let mut fail_count = 0;
    
    for entity_name in entities {
        if let Some(mut doc) = create_single_entity_doc(entity_name) {
            doc.version = DxfVersion::AC1024;
            
            let filename = format!("entity_2010_{}.dxf", entity_name);
            let writer = DxfWriter::new(doc);
            let result = writer.write_to_file(&filename);
            
            match result {
                Ok(_) => {
                    let size = std::fs::metadata(&filename).unwrap().len();
                    println!("  ✓ {} - {} bytes", entity_name, size);
                    success_count += 1;
                }
                Err(e) => {
                    println!("  ✗ {} - ERROR: {:?}", entity_name, e);
                    fail_count += 1;
                }
            }
        }
    }
    
    println!("\n📊 Results: {} success, {} failed", success_count, fail_count);
}

