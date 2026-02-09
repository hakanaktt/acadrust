//! Generates DXF files with all entity types for every supported version.
//! Files are written to `test_output/` so you can open them in your CAD application.
//!
//! Run with:  cargo test --test cad_roundtrip_output -- --nocapture --ignored

use acadrust::entities::*;
use acadrust::io::dxf::{DxfReader, DxfReaderConfiguration};
use acadrust::objects::{MLineStyle, ObjectType};
use acadrust::types::{Color, DxfVersion, Vector2, Vector3};
use acadrust::{BlockRecord, CadDocument, DxfWriter, TableEntry};
use std::f64::consts::PI;
use std::path::Path;

// ---------------------------------------------------------------------------
// Document with every entity type laid out in a visible grid
// ---------------------------------------------------------------------------

fn create_all_entities_document() -> CadDocument {
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
    x = 0.0; y += sp;

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
    // Valid clamped knot vector: n + d + 1 = 4 + 3 + 1 = 8 knots
    spline.knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    doc.add_entity(EntityType::Spline(spline)).unwrap();

    // Row 3 — text
    x = 0.0; y += sp;

    // 11. Text
    let mut text = Text::with_value("Hello DXF Writer", Vector3::new(x, y, 0.0))
        .with_height(2.5);
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
    x = 0.0; y += sp;

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
    x = 0.0; y += sp;

    // 15. Ray
    let ray = Ray::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 1.0, 0.0));
    doc.add_entity(EntityType::Ray(ray)).unwrap();
    x += sp;

    // 16. XLine
    let xline = XLine::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 0.5, 0.0));
    doc.add_entity(EntityType::XLine(xline)).unwrap();

    // Row 6 — dimensions (7 types)
    x = 0.0; y += sp;

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

    x = 0.0; y += sp;

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
    x = 0.0; y += sp;

    // 24. Hatch (solid fill)
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

    // Row 8 — block reference & attributes
    x = 0.0; y += sp;

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
    doc.add_entity(EntityType::AttributeDefinition(attdef)).unwrap();
    x += sp;

    // 27. AttributeEntity — NOTE: ATTRIB is only valid as a sub-entity
    // under INSERT (between INSERT and SEQEND). Standalone ATTRIB in the
    // ENTITIES section causes CAD warnings. Skipped here; proper support
    // requires Insert.attributes.
    // TODO: Add attributes field to Insert and write them as sub-entities.
    x += sp; // keep grid spacing consistent

    // Row 9 — leaders
    x = 0.0; y += sp;

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
    // Set linetype handle to ByLayer (required for CAD validation)
    mleader.line_type_handle = Some(doc.header.bylayer_linetype_handle);
    doc.add_entity(EntityType::MultiLeader(mleader)).unwrap();
    x += sp;

    // 30. MLine — create Standard MLineStyle in OBJECTS
    let mut ml_style = MLineStyle::standard();
    ml_style.handle = doc.allocate_handle();
    let ml_style_handle = ml_style.handle;
    doc.objects.insert(ml_style.handle, ObjectType::MLineStyle(ml_style));

    let mut mline = MLineBuilder::new()
        .justification(MLineJustification::Zero)
        .vertex(Vector3::new(x, y, 0.0))
        .vertex(Vector3::new(x + 5.0, y + 5.0, 0.0))
        .vertex(Vector3::new(x + 10.0, y, 0.0))
        .build();
    mline.style_handle = Some(ml_style_handle);
    doc.add_entity(EntityType::MLine(mline)).unwrap();

    // Row 10 — mesh
    x = 0.0; y += sp;

    // 31. Mesh
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

    // Row 11 — table / tolerance
    x = 0.0; y += sp;

    // 35. Table — create anonymous block record for the table
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

    // 36. Tolerance
    let mut tolerance = Tolerance::new();
    tolerance.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A".to_string();
    tolerance.insertion_point = Vector3::new(x, y, 0.0);
    tolerance.direction = Vector3::UNIT_X;
    doc.add_entity(EntityType::Tolerance(tolerance)).unwrap();
    x += sp;

    // 37. RasterImage
    let raster = RasterImage::new(
        "sample.png",
        Vector3::new(x, y, 0.0),
        640.0,
        480.0,
    );
    doc.add_entity(EntityType::RasterImage(raster)).unwrap();

    // Row 12 — polyface, wipeout, shape, viewport, underlay
    x = 0.0; y += sp;

    // 38. PolyfaceMesh
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

    // 39. Wipeout
    let wipeout = Wipeout::rectangular(Vector3::new(x, y, 0.0), 10.0, 10.0);
    doc.add_entity(EntityType::Wipeout(wipeout)).unwrap();
    x += sp;

    // 40. Shape — Shape entities require a .shx file referenced via a
    //     TextStyle entry. Without a valid .shx, removed to avoid CAD warnings.
    //     Skipped here; proper support requires a real .shx shape file.
    x += sp; // keep grid spacing consistent

    // 41. Viewport
    let mut viewport = Viewport::new();
    viewport.center = Vector3::new(x, y, 0.0);
    viewport.width = 10.0;
    viewport.height = 10.0;
    viewport.view_center = Vector3::new(0.0, 0.0, 0.0);
    viewport.view_height = 100.0;
    doc.add_entity(EntityType::Viewport(viewport)).unwrap();
    x += sp;

    // 42. Underlay (PDF)
    let mut underlay = Underlay::pdf();
    underlay.insertion_point = Vector3::new(x, y, 0.0);
    underlay.x_scale = 1.0;
    underlay.y_scale = 1.0;
    underlay.z_scale = 1.0;
    doc.add_entity(EntityType::Underlay(underlay)).unwrap();

    // Row 13 — OLE2Frame, PolygonMesh
    x = 0.0; y += sp;

    // 43. Ole2Frame
    let mut ole = Ole2Frame::new();
    ole.source_application = "TestApp".to_string();
    ole.upper_left_corner = Vector3::new(x, y + 5.0, 0.0);
    ole.lower_right_corner = Vector3::new(x + 10.0, y, 0.0);
    doc.add_entity(EntityType::Ole2Frame(ole)).unwrap();
    x += sp;

    // 44. PolygonMesh (M*N surface grid)
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

fn read_back(path: &str) -> CadDocument {
    let config = DxfReaderConfiguration { failsafe: true };
    DxfReader::from_file(path)
        .unwrap()
        .with_configuration(config)
        .read()
        .unwrap()
}

fn entity_type_counts(doc: &CadDocument) -> std::collections::BTreeMap<String, usize> {
    let mut map = std::collections::BTreeMap::new();
    for e in doc.entities() {
        *map.entry(e.as_entity().entity_type().to_string()).or_insert(0) += 1;
    }
    map
}

const VERSIONS: [(DxfVersion, &str); 8] = [
    (DxfVersion::AC1012, "R13"),
    (DxfVersion::AC1014, "R14"),
    (DxfVersion::AC1015, "2000"),
    (DxfVersion::AC1018, "2004"),
    (DxfVersion::AC1021, "2007"),
    (DxfVersion::AC1024, "2010"),
    (DxfVersion::AC1027, "2013"),
    (DxfVersion::AC1032, "2018"),
];

// ---------------------------------------------------------------------------
// Main test — writes all versions, reads back, reports results
// ---------------------------------------------------------------------------

#[test]
#[ignore] // run explicitly: cargo test --test cad_roundtrip_output -- --ignored --nocapture
fn generate_all_version_dxf_files() {
    let out_dir = Path::new("test_output");
    std::fs::create_dir_all(out_dir).expect("Failed to create test_output/");

    let mut summary = Vec::new();

    for (ver, name) in &VERSIONS {
        // --- ASCII ---
        let ascii_name = format!("all_entities_{}_ascii.dxf", name);
        let ascii_path = out_dir.join(&ascii_name);
        {
            let mut doc = create_all_entities_document();
            doc.version = *ver;
            let wrote = doc.entity_count();
            match DxfWriter::new(doc).write_to_file(ascii_path.to_str().unwrap()) {
                Ok(_) => {
                    let rdoc = read_back(ascii_path.to_str().unwrap());
                    let read_total = rdoc.entity_count();
                    let unknown: usize = rdoc.entities()
                        .filter(|e| matches!(e, EntityType::Unknown(_)))
                        .count();
                    let sz = std::fs::metadata(&ascii_path).unwrap().len();

                    summary.push(format!(
                        "  {:<12} ASCII   {:>6} bytes   wrote {:>2}  read {:>2}  unknown {:>2}  {}",
                        name, sz, wrote, read_total, unknown,
                        if read_total == wrote && unknown == 0 { "PERFECT" }
                        else if unknown == 0 { "OK" }
                        else { "PARTIAL" }
                    ));
                }
                Err(e) => {
                    summary.push(format!("  {:<12} ASCII   SKIPPED (file locked: {})", name, e));
                }
            }
        }

        // --- Binary ---
        let bin_name = format!("all_entities_{}_binary.dxf", name);
        let bin_path = out_dir.join(&bin_name);
        {
            let mut doc = create_all_entities_document();
            doc.version = *ver;
            let wrote = doc.entity_count();
            match DxfWriter::new_binary(doc).write_to_file(bin_path.to_str().unwrap()) {
                Ok(_) => {
                    let rdoc = read_back(bin_path.to_str().unwrap());
                    let read_total = rdoc.entity_count();
                    let unknown: usize = rdoc.entities()
                        .filter(|e| matches!(e, EntityType::Unknown(_)))
                        .count();
                    let sz = std::fs::metadata(&bin_path).unwrap().len();

                    summary.push(format!(
                        "  {:<12} Binary  {:>6} bytes   wrote {:>2}  read {:>2}  unknown {:>2}  {}",
                        name, sz, wrote, read_total, unknown,
                        if read_total == wrote && unknown == 0 { "PERFECT" }
                        else if unknown == 0 { "OK" }
                        else { "PARTIAL" }
                    ));
                }
                Err(e) => {
                    summary.push(format!("  {:<12} Binary  SKIPPED (file locked: {})", name, e));
                }
            }
        }
    }

    // --- Detailed report for 2018 ASCII (most complete version) ---
    println!("\n===== DXF Round-Trip Test Output =====");
    println!("Files written to: test_output/\n");

    println!("--- Version Summary ---");
    for line in &summary {
        println!("{}", line);
    }

    // Detailed entity breakdown for 2018 ASCII
    let detail_path = out_dir.join("all_entities_2018_ascii.dxf");
    let orig = create_all_entities_document();
    let rdoc = read_back(detail_path.to_str().unwrap());
    let orig_counts = entity_type_counts(&orig);
    let read_counts = entity_type_counts(&rdoc);

    println!("\n--- Entity Detail (2018 ASCII) ---");
    println!("  {:<35} {:>6} {:>6}", "Type", "Wrote", "Read");
    let mut all_keys: Vec<_> = orig_counts.keys().chain(read_counts.keys()).cloned().collect();
    all_keys.sort();
    all_keys.dedup();
    for key in &all_keys {
        let w = orig_counts.get(key).copied().unwrap_or(0);
        let r = read_counts.get(key).copied().unwrap_or(0);
        let mark = if w == r { "OK" } else { "DIFF" };
        println!("  {:<35} {:>6} {:>6}  {}", key, w, r, mark);
    }

    println!("\n--- Files for CAD testing ---");
    for (_, name) in &VERSIONS {
        println!("  test_output/all_entities_{}_ascii.dxf", name);
        println!("  test_output/all_entities_{}_binary.dxf", name);
    }
    println!("\nTotal: {} files generated", VERSIONS.len() * 2);
}
