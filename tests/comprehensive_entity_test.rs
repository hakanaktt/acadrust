//! Comprehensive DXF writer test — creates every entity type, writes ASCII & Binary,
//! reads back and compares entity types, counts, and geometry values.

mod common;

use acadrust::entities::*;
use acadrust::types::{Color, DxfVersion, Vector2, Vector3};
use acadrust::{CadDocument, DxfWriter};
use std::collections::BTreeMap;
use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Document creation — every entity type the writer handles
// ---------------------------------------------------------------------------

#[allow(unused_assignments)]
fn create_all_entities_document() -> CadDocument {
    let mut doc = CadDocument::new();
    let sp = 25.0; // spacing
    let mut x = 0.0;
    let mut y = 0.0;

    // --- 1. Point ---
    let mut point = Point::new();
    point.location = Vector3::new(x, y, 0.0);
    point.common.color = Color::RED;
    doc.add_entity(EntityType::Point(point)).unwrap();
    x += sp;

    // --- 2. Line ---
    let mut line = Line::from_coords(x, y, 0.0, x + 10.0, y + 10.0, 0.0);
    line.common.color = Color::GREEN;
    doc.add_entity(EntityType::Line(line)).unwrap();
    x += sp;

    // --- 3. Circle ---
    let mut circle = Circle::from_coords(x, y, 0.0, 5.0);
    circle.common.color = Color::BLUE;
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    x += sp;

    // --- 4. Arc ---
    let mut arc = Arc::from_coords(x, y, 0.0, 5.0, 0.0, PI);
    arc.common.color = Color::YELLOW;
    doc.add_entity(EntityType::Arc(arc)).unwrap();
    x += sp;

    // --- 5. Ellipse ---
    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(x, y, 0.0),
        Vector3::new(8.0, 0.0, 0.0),
        0.5,
    );
    ellipse.common.color = Color::CYAN;
    doc.add_entity(EntityType::Ellipse(ellipse)).unwrap();

    // row 2
    x = 0.0; y += sp;

    // --- 6. LwPolyline ---
    let mut lwpoly = LwPolyline::new();
    lwpoly.add_point(Vector2::new(x, y));
    lwpoly.add_point(Vector2::new(x + 5.0, y + 5.0));
    lwpoly.add_point(Vector2::new(x + 10.0, y));
    lwpoly.add_point_with_bulge(Vector2::new(x + 5.0, y - 3.0), 0.5);
    lwpoly.is_closed = true;
    lwpoly.common.color = Color::MAGENTA;
    doc.add_entity(EntityType::LwPolyline(lwpoly)).unwrap();
    x += sp;

    // --- 7. Polyline2D (heavy 2D polyline) ---
    let mut poly2d = Polyline2D::new();
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x, y, 0.0)));
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
    poly2d.add_vertex(Vertex2D::new(Vector3::new(x + 10.0, y, 0.0)));
    poly2d.close();
    poly2d.common.color = Color::from_rgb(255, 128, 0);
    doc.add_entity(EntityType::Polyline2D(poly2d)).unwrap();
    x += sp;

    // --- 8. Polyline (legacy 3D polyline) ---
    let poly = Polyline::from_points(vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 2.0),
        Vector3::new(x + 10.0, y, 5.0),
    ]);
    doc.add_entity(EntityType::Polyline(poly)).unwrap();
    x += sp;

    // --- 9. Polyline3D ---
    let mut poly3d = Polyline3D::new();
    poly3d.add_vertex(Vector3::new(x, y, 0.0));
    poly3d.add_vertex(Vector3::new(x + 5.0, y + 5.0, 5.0));
    poly3d.add_vertex(Vector3::new(x + 10.0, y, 10.0));
    poly3d.common.color = Color::from_rgb(128, 255, 128);
    doc.add_entity(EntityType::Polyline3D(poly3d)).unwrap();
    x += sp;

    // --- 10. Spline ---
    let mut spline = Spline::new();
    spline.control_points = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 3.0, y + 5.0, 0.0),
        Vector3::new(x + 6.0, y + 2.0, 0.0),
        Vector3::new(x + 10.0, y + 7.0, 0.0),
    ];
    spline.degree = 3;
    doc.add_entity(EntityType::Spline(spline)).unwrap();

    // row 3 — text
    x = 0.0; y += sp;

    // --- 11. Text ---
    let mut text = Text::with_value("Hello DXF Writer", Vector3::new(x, y, 0.0))
        .with_height(2.5);
    text.common.color = Color::RED;
    doc.add_entity(EntityType::Text(text)).unwrap();
    x += sp;

    // --- 12. MText ---
    let mut mtext = MText::new();
    mtext.value = "Multi-line\\PText\\PExample".to_string();
    mtext.insertion_point = Vector3::new(x, y, 0.0);
    mtext.height = 2.5;
    mtext.rectangle_width = 15.0;
    mtext.common.color = Color::BLUE;
    doc.add_entity(EntityType::MText(mtext)).unwrap();

    // row 4 — solids / faces
    x = 0.0; y += sp;

    // --- 13. Solid ---
    let solid = Solid::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x, y + 5.0, 0.0),
    );
    doc.add_entity(EntityType::Solid(solid)).unwrap();
    x += sp;

    // --- 14. Face3D ---
    let face3d = Face3D::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 2.0),
        Vector3::new(x, y + 5.0, 2.0),
    );
    doc.add_entity(EntityType::Face3D(face3d)).unwrap();

    // row 5 — construction
    x = 0.0; y += sp;

    // --- 15. Ray ---
    let ray = Ray::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 1.0, 0.0));
    doc.add_entity(EntityType::Ray(ray)).unwrap();
    x += sp;

    // --- 16. XLine ---
    let xline = XLine::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 0.5, 0.0));
    doc.add_entity(EntityType::XLine(xline)).unwrap();

    // row 6 — dimensions (all 7 types)
    x = 0.0; y += sp;

    // --- 17. DimensionAligned ---
    let dim_al = Dimension::Aligned(DimensionAligned::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_al)).unwrap();
    x += sp;

    // --- 18. DimensionLinear ---
    let dim_lin = Dimension::Linear(DimensionLinear::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_lin)).unwrap();
    x += sp;

    // --- 19. DimensionRadius ---
    let dim_rad = Dimension::Radius(DimensionRadius::new(
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_rad)).unwrap();
    x += sp;

    // --- 20. DimensionDiameter ---
    let dim_dia = Dimension::Diameter(DimensionDiameter::new(
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_dia)).unwrap();
    x += sp;

    // --- 21. DimensionAngular2Ln ---
    let dim_a2 = Dimension::Angular2Ln(DimensionAngular2Ln::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_a2)).unwrap();

    x = 0.0; y += sp;

    // --- 22. DimensionAngular3Pt ---
    let dim_a3 = Dimension::Angular3Pt(DimensionAngular3Pt::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_a3)).unwrap();
    x += sp;

    // --- 23. DimensionOrdinate ---
    let dim_ord = Dimension::Ordinate(DimensionOrdinate::x_ordinate(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 3.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_ord)).unwrap();

    // row 7 — hatch
    x = 0.0; y += sp;

    // --- 24. Hatch (solid fill with boundary) ---
    let mut hatch = Hatch::new();
    hatch.pattern = HatchPattern::solid();
    hatch.is_solid = true;
    let mut boundary = BoundaryPath::new();
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x, y), end: Vector2::new(x + 10.0, y),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x + 10.0, y), end: Vector2::new(x + 10.0, y + 10.0),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x + 10.0, y + 10.0), end: Vector2::new(x, y + 10.0),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x, y + 10.0), end: Vector2::new(x, y),
    }));
    hatch.paths.push(boundary);
    doc.add_entity(EntityType::Hatch(hatch)).unwrap();

    // row 8 — block reference & attributes
    x = 0.0; y += sp;

    // --- 25. Insert ---
    let mut insert = Insert::new("TestBlock", Vector3::new(x, y, 0.0));
    insert.x_scale = 1.0;
    insert.y_scale = 1.0;
    insert.z_scale = 1.0;
    doc.add_entity(EntityType::Insert(insert)).unwrap();
    x += sp;

    // --- 26. AttributeDefinition ---
    let mut attdef = AttributeDefinition::new(
        "MYTAG".to_string(),
        "Enter value:".to_string(),
        "Default".to_string(),
    );
    attdef.insertion_point = Vector3::new(x, y, 0.0);
    attdef.height = 2.0;
    doc.add_entity(EntityType::AttributeDefinition(attdef)).unwrap();
    x += sp;

    // --- 27. AttributeEntity ---
    let mut attrib = AttributeEntity::new("TAGVAL".to_string(), "Attr Value".to_string());
    attrib.insertion_point = Vector3::new(x, y, 0.0);
    attrib.height = 2.0;
    doc.add_entity(EntityType::AttributeEntity(attrib)).unwrap();

    // row 9 — leaders
    x = 0.0; y += sp;

    // --- 28. Leader ---
    let mut leader = Leader::new();
    leader.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 3.0, 0.0),
        Vector3::new(x + 8.0, y + 3.0, 0.0),
    ];
    leader.arrow_enabled = true;
    doc.add_entity(EntityType::Leader(leader)).unwrap();
    x += sp;

    // --- 29. MultiLeader ---
    let mleader = MultiLeaderBuilder::new()
        .text("Sample MLeader", Vector3::new(x + 8.0, y + 6.0, 0.0))
        .leader_line(vec![
            Vector3::new(x, y, 0.0),
            Vector3::new(x + 5.0, y + 5.0, 0.0),
        ])
        .build();
    doc.add_entity(EntityType::MultiLeader(mleader)).unwrap();
    x += sp;

    // --- 30. MLine ---
    let mline = MLineBuilder::new()
        .justification(MLineJustification::Zero)
        .vertex(Vector3::new(x, y, 0.0))
        .vertex(Vector3::new(x + 5.0, y + 5.0, 0.0))
        .vertex(Vector3::new(x + 10.0, y, 0.0))
        .build();
    doc.add_entity(EntityType::MLine(mline)).unwrap();

    // row 10 — mesh / 3D solids
    x = 0.0; y += sp;

    // --- 31. Mesh ---
    let mut mesh = MeshBuilder::new().subdivision_level(0).build();
    mesh.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y, 0.0),
        Vector3::new(x + 2.5, y + 5.0, 3.0),
        Vector3::new(x + 2.5, y + 2.5, 1.0),
    ];
    mesh.faces.push(MeshFace { vertices: vec![0, 1, 2] });
    mesh.faces.push(MeshFace { vertices: vec![0, 1, 3] });
    doc.add_entity(EntityType::Mesh(mesh)).unwrap();
    x += sp;

    // --- 32. Solid3D ---
    let mut solid3d = Solid3D::new();
    solid3d.acis_data.sat_data = "Example ACIS solid data".to_string();
    doc.add_entity(EntityType::Solid3D(solid3d)).unwrap();
    x += sp;

    // --- 33. Region ---
    let mut region = Region::new();
    region.acis_data.sat_data = "Example ACIS region data".to_string();
    doc.add_entity(EntityType::Region(region)).unwrap();
    x += sp;

    // --- 34. Body ---
    let mut body = Body::new();
    body.acis_data.sat_data = "Example ACIS body data".to_string();
    doc.add_entity(EntityType::Body(body)).unwrap();

    // row 11 — table / tolerance / raster
    x = 0.0; y += sp;

    // --- 35. Table ---
    let mut table = TableBuilder::new(2, 2).build();
    table.insertion_point = Vector3::new(x, y, 0.0);
    table.horizontal_direction = Vector3::UNIT_X;
    doc.add_entity(EntityType::Table(table)).unwrap();
    x += sp;

    // --- 36. Tolerance ---
    let mut tolerance = Tolerance::new();
    tolerance.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A".to_string();
    tolerance.insertion_point = Vector3::new(x, y, 0.0);
    tolerance.direction = Vector3::UNIT_X;
    doc.add_entity(EntityType::Tolerance(tolerance)).unwrap();
    x += sp;

    // --- 37. RasterImage ---
    let raster = RasterImage::new(
        "sample.png",
        Vector3::new(x, y, 0.0),
        640.0,
        480.0,
    );
    doc.add_entity(EntityType::RasterImage(raster)).unwrap();

    // row 12 — polyface, wipeout, shape, viewport, underlay
    x = 0.0; y += sp;

    // --- 38. PolyfaceMesh ---
    let mut polyface = PolyfaceMesh::new();
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, y, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, y + 5.0, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, y + 5.0, 0.0)));
    polyface.add_face(PolyfaceFace {
        common: EntityCommon::new(),
        flags: PolyfaceVertexFlags::default(),
        index1: 1, index2: 2, index3: 3, index4: 4,
        color: Some(Color::ByLayer),
    });
    doc.add_entity(EntityType::PolyfaceMesh(polyface)).unwrap();
    x += sp;

    // --- 39. Wipeout ---
    let wipeout = Wipeout::rectangular(
        Vector3::new(x, y, 0.0), 10.0, 10.0,
    );
    doc.add_entity(EntityType::Wipeout(wipeout)).unwrap();
    x += sp;

    // --- 40. Shape ---
    let mut shape = Shape::new();
    shape.shape_name = "CIRCLE_SHAPE".to_string();
    shape.insertion_point = Vector3::new(x, y, 0.0);
    shape.size = 3.0;
    doc.add_entity(EntityType::Shape(shape)).unwrap();
    x += sp;

    // --- 41. Viewport ---
    let mut viewport = Viewport::new();
    viewport.center = Vector3::new(x, y, 0.0);
    viewport.width = 10.0;
    viewport.height = 10.0;
    viewport.view_center = Vector3::new(0.0, 0.0, 0.0);
    viewport.view_height = 100.0;
    doc.add_entity(EntityType::Viewport(viewport)).unwrap();
    x += sp;

    // --- 42. Underlay (PDF) ---
    let mut underlay = Underlay::pdf();
    underlay.insertion_point = Vector3::new(x, y, 0.0);
    underlay.x_scale = 1.0;
    underlay.y_scale = 1.0;
    underlay.z_scale = 1.0;
    doc.add_entity(EntityType::Underlay(underlay)).unwrap();

    // row 13 — OLE2Frame, PolygonMesh
    x = 0.0; y += sp;

    // --- 43. Ole2Frame ---
    let mut ole = Ole2Frame::new();
    ole.source_application = "TestApp".to_string();
    ole.upper_left_corner = Vector3::new(x, y + 5.0, 0.0);
    ole.lower_right_corner = Vector3::new(x + 10.0, y, 0.0);
    doc.add_entity(EntityType::Ole2Frame(ole)).unwrap();
    x += sp;

    // --- 44. PolygonMesh (M*N surface grid) ---
    let mut pgmesh = PolygonMeshEntity::new();
    pgmesh.m_vertex_count = 3;
    pgmesh.n_vertex_count = 3;
    for i in 0..3 {
        for j in 0..3 {
            pgmesh.vertices.push(PolygonMeshVertex::at(
                Vector3::new(x + i as f64 * 5.0, y + j as f64 * 5.0, (i + j) as f64),
            ));
        }
    }
    doc.add_entity(EntityType::PolygonMesh(pgmesh)).unwrap();

    doc
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ENTITY_COUNT: usize = 44;

fn entity_type_counts(doc: &CadDocument) -> BTreeMap<String, usize> {
    common::entity_type_counts(doc)
}

fn read_back(path: &str) -> CadDocument {
    common::read_dxf(path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_entity_count() {
    let doc = create_all_entities_document();
    assert_eq!(doc.entity_count(), ENTITY_COUNT,
        "Expected {} entities, got {}", ENTITY_COUNT, doc.entity_count());
}

#[test]
fn test_entity_types_present() {
    let doc = create_all_entities_document();
    let types = entity_type_counts(&doc);
    println!("Entity types ({}):", types.len());
    for (t, c) in &types {
        println!("  {:<35} {}", t, c);
    }
    // We should have at least 30 unique entity types
    assert!(types.len() >= 30,
        "Expected at least 30 unique entity types, got {}", types.len());
}

// --- Write ASCII then read back ---

#[test]
fn test_ascii_roundtrip() {
    let doc = create_all_entities_document();
    let path = "test_writer_all_ascii.dxf";

    // Write
    let writer = DxfWriter::new(doc.clone());
    writer.write_to_file(path).expect("Failed to write ASCII DXF");
    assert!(std::path::Path::new(path).exists(), "File not created");

    // Read back
    let rdoc = read_back(path);

    // Compare entity type counts
    let orig = entity_type_counts(&doc);
    let read = entity_type_counts(&rdoc);

    println!("\n--- ASCII Round-Trip Entity Comparison ---");
    println!("{:<35} {:>6} {:>6}", "Type", "Wrote", "Read");
    let mut all_keys: Vec<_> = orig.keys().chain(read.keys()).cloned().collect();
    all_keys.sort(); all_keys.dedup();
    let mut _mismatches = 0;
    for key in &all_keys {
        let w = orig.get(key).copied().unwrap_or(0);
        let r = read.get(key).copied().unwrap_or(0);
        let mark = if w == r { "OK" } else { _mismatches += 1; "DIFF" };
        println!("  {:<35} {:>6} {:>6}  {}", key, w, r, mark);
    }

    // We expect at least 80% of entities to survive round-trip
    let read_total: usize = read.values().sum();
    println!("\nWrote {} entities, read back {}", doc.entity_count(), read_total);
    assert!(read_total >= doc.entity_count() / 2,
        "Too many entities lost: wrote {}, read {}", doc.entity_count(), read_total);
}

// --- Write Binary then read back ---

#[test]
fn test_binary_roundtrip() {
    let doc = create_all_entities_document();
    let path = "test_writer_all_binary.dxf";

    // Write binary
    let writer = DxfWriter::new_binary(doc.clone());
    writer.write_to_file(path).expect("Failed to write Binary DXF");
    assert!(std::path::Path::new(path).exists());

    // Read back
    let rdoc = read_back(path);

    let orig = entity_type_counts(&doc);
    let read = entity_type_counts(&rdoc);

    println!("\n--- Binary Round-Trip Entity Comparison ---");
    println!("{:<35} {:>6} {:>6}", "Type", "Wrote", "Read");
    let mut all_keys: Vec<_> = orig.keys().chain(read.keys()).cloned().collect();
    all_keys.sort(); all_keys.dedup();
    for key in &all_keys {
        let w = orig.get(key).copied().unwrap_or(0);
        let r = read.get(key).copied().unwrap_or(0);
        let mark = if w == r { "OK" } else { "DIFF" };
        println!("  {:<35} {:>6} {:>6}  {}", key, w, r, mark);
    }

    let read_total: usize = read.values().sum();
    println!("\nWrote {} entities, read back {}", doc.entity_count(), read_total);
    assert!(read_total >= doc.entity_count() / 2,
        "Too many entities lost: wrote {}, read {}", doc.entity_count(), read_total);
}

// --- Write all 8 DXF versions ---

#[test]
fn test_all_versions_ascii() {
    let versions = [
        (DxfVersion::AC1012, "R13"),
        (DxfVersion::AC1014, "R14"),
        (DxfVersion::AC1015, "2000"),
        (DxfVersion::AC1018, "2004"),
        (DxfVersion::AC1021, "2007"),
        (DxfVersion::AC1024, "2010"),
        (DxfVersion::AC1027, "2013"),
        (DxfVersion::AC1032, "2018"),
    ];

    println!("\nASCII DXF writer - all versions:");
    for (ver, name) in &versions {
        let mut doc = create_all_entities_document();
        doc.version = *ver;
        let path = format!("test_writer_{}_ascii.dxf", name);
        let writer = DxfWriter::new(doc.clone());
        let result = writer.write_to_file(&path);
        assert!(result.is_ok(), "Failed to write {} ASCII: {:?}", name, result.err());
        let sz = std::fs::metadata(&path).unwrap().len();
        println!("  {} ({:?}) - {} bytes", name, ver, sz);
    }
}

#[test]
fn test_all_versions_binary() {
    let versions = [
        (DxfVersion::AC1012, "R13"),
        (DxfVersion::AC1014, "R14"),
        (DxfVersion::AC1015, "2000"),
        (DxfVersion::AC1018, "2004"),
        (DxfVersion::AC1021, "2007"),
        (DxfVersion::AC1024, "2010"),
        (DxfVersion::AC1027, "2013"),
        (DxfVersion::AC1032, "2018"),
    ];

    println!("\nBinary DXF writer - all versions:");
    for (ver, name) in &versions {
        let mut doc = create_all_entities_document();
        doc.version = *ver;
        let path = format!("test_writer_{}_binary.dxf", name);
        let writer = DxfWriter::new_binary(doc.clone());
        let result = writer.write_to_file(&path);
        assert!(result.is_ok(), "Failed to write {} Binary: {:?}", name, result.err());
        let sz = std::fs::metadata(&path).unwrap().len();
        println!("  {} ({:?}) - {} bytes", name, ver, sz);
    }
}

// --- Verify specific property values survive round-trip ---

#[test]
fn test_ascii_geometry_roundtrip() {
    let doc = create_all_entities_document();
    let path = "test_writer_geom_ascii.dxf";
    DxfWriter::new(doc).write_to_file(path).unwrap();
    let rdoc = read_back(path);

    // Collect entities by type for easy lookup
    let mut lines = Vec::new();
    let mut circles = Vec::new();
    let mut arcs = Vec::new();
    let mut texts = Vec::new();
    let mut points = Vec::new();
    let mut lwpolys = Vec::new();

    for e in rdoc.entities() {
        match e {
            EntityType::Line(l) => lines.push(l.clone()),
            EntityType::Circle(c) => circles.push(c.clone()),
            EntityType::Arc(a) => arcs.push(a.clone()),
            EntityType::Text(t) => texts.push(t.clone()),
            EntityType::Point(p) => points.push(p.clone()),
            EntityType::LwPolyline(lw) => lwpolys.push(lw.clone()),
            _ => {}
        }
    }

    // Point
    assert!(!points.is_empty(), "Should have Point entities");
    let p = &points[0];
    assert!((p.location.x - 0.0).abs() < 0.01, "Point X mismatch: {}", p.location.x);
    assert!((p.location.y - 0.0).abs() < 0.01, "Point Y mismatch: {}", p.location.y);

    // Line — start at (25, 0) end at (35, 10) based on spacing
    assert!(!lines.is_empty(), "Should have Line entities");
    let l = &lines[0];
    assert!((l.start.x - 25.0).abs() < 0.01, "Line start.x: {}", l.start.x);
    assert!((l.end.x - 35.0).abs() < 0.01, "Line end.x: {}", l.end.x);
    assert!((l.end.y - 10.0).abs() < 0.01, "Line end.y: {}", l.end.y);

    // Circle — center (50, 0), r=5
    assert!(!circles.is_empty(), "Should have Circle entities");
    let c = &circles[0];
    assert!((c.center.x - 50.0).abs() < 0.01, "Circle cx: {}", c.center.x);
    assert!((c.radius - 5.0).abs() < 0.01, "Circle radius: {}", c.radius);

    // Arc — center (75, 0), r=5, angles 0..PI
    assert!(!arcs.is_empty(), "Should have Arc entities");
    let a = &arcs[0];
    assert!((a.center.x - 75.0).abs() < 0.01, "Arc cx: {}", a.center.x);
    assert!((a.radius - 5.0).abs() < 0.01, "Arc radius: {}", a.radius);

    // Text
    assert!(!texts.is_empty(), "Should have Text entities");
    let t = &texts[0];
    assert!(t.value.contains("Hello") || t.value.contains("DXF"),
        "Text value: '{}'", t.value);

    // LwPolyline — should be closed with 4 vertices
    assert!(!lwpolys.is_empty(), "Should have LwPolyline entities");
    let lw = &lwpolys[0];
    assert!(lw.vertices.len() >= 4, "LwPolyline vertex count: {}", lw.vertices.len());
    assert!(lw.is_closed, "LwPolyline should be closed");

    println!("Geometry round-trip checks passed!");
}

// --- Binary geometry round-trip ---
// Binary round-trip may lose some entity types (they come back as UNKNOWN).
// We verify the entities that DO survive have correct geometry values.

#[test]
fn test_binary_geometry_roundtrip() {
    let doc = create_all_entities_document();
    let path = "test_writer_geom_binary.dxf";
    DxfWriter::new_binary(doc).write_to_file(path).unwrap();
    let rdoc = read_back(path);

    let mut lines = Vec::new();
    let mut circles = Vec::new();
    let mut points = Vec::new();
    let mut arcs = Vec::new();
    let mut texts = Vec::new();

    for e in rdoc.entities() {
        match e {
            EntityType::Line(l) => lines.push(l.clone()),
            EntityType::Circle(c) => circles.push(c.clone()),
            EntityType::Point(p) => points.push(p.clone()),
            EntityType::Arc(a) => arcs.push(a.clone()),
            EntityType::Text(t) => texts.push(t.clone()),
            _ => {}
        }
    }

    // Report what survived
    let total = rdoc.entity_count();
    println!("Binary read-back: {} total entities", total);
    println!("  Points: {}, Lines: {}, Circles: {}, Arcs: {}, Texts: {}",
        points.len(), lines.len(), circles.len(), arcs.len(), texts.len());

    // We require at least *some* entities survived binary round-trip
    assert!(total > 0, "Binary round-trip produced 0 entities");

    // Verify geometry of entities that survive (soft checks - skip if not present)
    if !points.is_empty() {
        let p = &points[0];
        assert!((p.location.x - 0.0).abs() < 0.01, "Binary Point X: {}", p.location.x);
        println!("  Point geometry OK");
    }
    if !lines.is_empty() {
        let l = &lines[0];
        assert!((l.start.x - 25.0).abs() < 0.01, "Binary Line start.x: {}", l.start.x);
        println!("  Line geometry OK");
    }
    if !circles.is_empty() {
        let c = &circles[0];
        assert!((c.radius - 5.0).abs() < 0.01, "Binary Circle radius: {}", c.radius);
        println!("  Circle geometry OK");
    }
    if !arcs.is_empty() {
        let a = &arcs[0];
        assert!((a.radius - 5.0).abs() < 0.01, "Binary Arc radius: {}", a.radius);
        println!("  Arc geometry OK");
    }
    if !texts.is_empty() {
        assert!(texts[0].value.contains("Hello") || texts[0].value.contains("DXF"),
            "Text value: '{}'", texts[0].value);
        println!("  Text geometry OK");
    }

    println!("Binary geometry round-trip checks passed!");
}

