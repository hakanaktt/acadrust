/// Generate DWG + DXF test files for manual validation in CAD applications.
///
/// Output directory: `test_output/cad_validation/`
///
/// ## Usage
///     cargo run --bin generate_test_dwgs
///
/// ## Generated files
///
/// For each DWG version (R13, R14, R2000, R2004, R2007, R2010, R2013, R2018):
///   - `all_entities_{version}.dwg`  — 30+ entity types on a grid
///   - `all_entities_{version}.dxf`  — matching DXF for comparison
///   - `basic_geometry_{version}.dwg` — lines, circles, arcs, polylines
///   - `text_and_dims_{version}.dwg`  — text, mtext, all 7 dimension types
///   - `hatches_{version}.dwg`        — solid fill, pattern fill
///   - `blocks_attribs_{version}.dwg` — block inserts with attributes
///   - `3d_entities_{version}.dwg`    — 3D faces, meshes, polyface, solids
///
/// ## Validation steps
///
/// 1. **AutoCAD / BricsCAD**: Open each .dwg, run AUDIT, check entity count
/// 2. **ODA File Converter**: Batch-convert all .dwg → .dxf, compare
/// 3. **LibreCAD**: Open .dxf files, verify geometry renders
/// 4. **FreeCAD**: Import .dxf, check entity recognition
/// 5. **Teigha Viewer / ODA Viewer**: Quick visual inspection
///
/// Expected behavior per version:
///   - All files should open without "drawing recovery" prompts
///   - Entity count should match what was written
///   - Geometry should appear at correct coordinates
///   - Colors, layers, and text styles should be preserved

use acadrust::entities::*;
use acadrust::io::dwg::writer::dwg_writer::DwgWriter;
use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion, Vector2, Vector3};
use acadrust::{BlockRecord, CadDocument, DxfWriter, TableEntry};
use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Version table
// ---------------------------------------------------------------------------

const VERSIONS: &[(DxfVersion, &str, &str)] = &[
    (DxfVersion::AC1012, "AC1012", "R13"),
    (DxfVersion::AC1014, "AC1014", "R14"),
    (DxfVersion::AC1015, "AC1015", "R2000"),
    (DxfVersion::AC1018, "AC1018", "R2004"),
    (DxfVersion::AC1021, "AC1021", "R2007"),
    (DxfVersion::AC1024, "AC1024", "R2010"),
    (DxfVersion::AC1027, "AC1027", "R2013"),
    (DxfVersion::AC1032, "AC1032", "R2018"),
];

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_output")
        .join("cad_validation");
    fs::create_dir_all(&out_dir).expect("Failed to create output directory");

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          acadrust — DWG/DXF Test File Generator             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Output directory: {}", out_dir.display());
    println!();

    let mut total_files = 0;
    let mut failed = 0;

    let categories: Vec<(&str, fn(DxfVersion) -> CadDocument)> = vec![
        ("all_entities", build_all_entities as fn(DxfVersion) -> CadDocument),
        ("basic_geometry", build_basic_geometry),
        ("text_and_dims", build_text_and_dims),
        ("hatches", build_hatches),
        ("blocks_attribs", build_blocks_attribs),
        ("3d_entities", build_3d_entities),
    ];

    for &(version, code, label) in VERSIONS {
        println!("── {} ({}) ──────────────────────────────────", label, code);

        for &(category, builder) in &categories {
            match write_test_file(&out_dir, version, code, category, builder) {
                Ok((dwg, dxf)) => {
                    total_files += 2;
                    let dwg_size = fs::metadata(out_dir.join(&dwg)).map(|m| m.len()).unwrap_or(0);
                    let dxf_size = fs::metadata(out_dir.join(&dxf)).map(|m| m.len()).unwrap_or(0);
                    println!("  ✅ {} ({} bytes) + .dxf ({} bytes)", dwg, dwg_size, dxf_size);
                }
                Err(e) => {
                    failed += 1;
                    println!("  ❌ {}: {}", category, e);
                }
            }
        }

        println!();
    }

    // Summary
    println!("════════════════════════════════════════════════════════════════");
    println!("Total files generated: {}", total_files);
    if failed > 0 {
        println!("Failed: {} (see errors above)", failed);
    } else {
        println!("All files generated successfully! ✅");
    }
    println!();
    println!("Next steps for validation:");
    println!("  1. Open .dwg files in AutoCAD / BricsCAD / ODA Viewer");
    println!("  2. Run AUDIT command in AutoCAD to check for errors");
    println!("  3. Compare .dwg vs .dxf visual output side by side");
    println!("  4. Check ENTITIES section in each file (use LIST command)");
    println!();
    println!("ODA File Converter batch validation:");
    println!("  ODAFileConverter.exe \"{}\" \"{}\\oda_converted\" \"ACAD2018\" \"DXF\" \"0\" \"1\"",
        out_dir.display(), out_dir.display());
}

// ---------------------------------------------------------------------------
// File writer helper
// ---------------------------------------------------------------------------

fn write_test_file(
    out_dir: &Path,
    version: DxfVersion,
    code: &str,
    category: &str,
    builder: fn(DxfVersion) -> CadDocument,
) -> std::result::Result<(String, String), String> {
    let mut doc = builder(version);
    doc.version = version;

    let dwg_name = format!("{}_{}.dwg", category, code);
    let dxf_name = format!("{}_{}.dxf", category, code);

    // Write DWG
    let dwg_bytes = DwgWriter::write(&doc)
        .map_err(|e| format!("DWG write error: {e}"))?;
    fs::write(out_dir.join(&dwg_name), &dwg_bytes)
        .map_err(|e| format!("DWG file write error: {e}"))?;

    // Write DXF (ASCII for maximum compatibility)
    let dxf_path = out_dir.join(&dxf_name);
    DxfWriter::new(doc)
        .write_to_file(&dxf_path)
        .map_err(|e| format!("DXF write error: {e}"))?;

    Ok((dwg_name, dxf_name))
}

// ---------------------------------------------------------------------------
// Document builders
// ---------------------------------------------------------------------------

/// Build a document with all supported entity types on a grid.
fn build_all_entities(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    add_test_layers(&mut doc);

    let sp = 30.0;
    let mut x = 0.0;
    let mut y = 0.0;

    // === Row 1: Basic geometry ===
    // Crosshair marker
    doc.add_entity(EntityType::Line(Line::from_coords(x - 1.0, y, 0.0, x + 1.0, y, 0.0))).unwrap();
    doc.add_entity(EntityType::Line(Line::from_coords(x, y - 1.0, 0.0, x, y + 1.0, 0.0))).unwrap();

    let mut p = Point::new();
    p.location = Vector3::new(x + 2.0, y, 0.0);
    p.common.layer = "Geometry".to_string();
    p.common.color = Color::RED;
    doc.add_entity(EntityType::Point(p)).unwrap();
    x += sp;

    let mut line = Line::from_coords(x, y, 0.0, x + 15.0, y + 10.0, 0.0);
    line.common.layer = "Geometry".to_string();
    line.common.color = Color::GREEN;
    doc.add_entity(EntityType::Line(line)).unwrap();
    x += sp;

    let mut circle = Circle::from_coords(x + 7.0, y + 5.0, 0.0, 5.0);
    circle.common.layer = "Geometry".to_string();
    circle.common.color = Color::BLUE;
    doc.add_entity(EntityType::Circle(circle)).unwrap();
    x += sp;

    let mut arc = Arc::from_coords(x + 7.0, y + 5.0, 0.0, 5.0, 0.0, PI * 1.5);
    arc.common.layer = "Geometry".to_string();
    arc.common.color = Color::YELLOW;
    doc.add_entity(EntityType::Arc(arc)).unwrap();
    x += sp;

    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(x + 7.0, y + 5.0, 0.0),
        Vector3::new(8.0, 0.0, 0.0),
        0.5,
    );
    ellipse.common.layer = "Geometry".to_string();
    ellipse.common.color = Color::CYAN;
    doc.add_entity(EntityType::Ellipse(ellipse)).unwrap();

    // === Row 2: Polylines ===
    x = 0.0;
    y += sp;

    let mut lwpoly = LwPolyline::new();
    lwpoly.add_point(Vector2::new(x, y));
    lwpoly.add_point(Vector2::new(x + 10.0, y));
    lwpoly.add_point(Vector2::new(x + 10.0, y + 10.0));
    lwpoly.add_point(Vector2::new(x, y + 10.0));
    lwpoly.is_closed = true;
    lwpoly.common.layer = "Polylines".to_string();
    lwpoly.common.color = Color::MAGENTA;
    doc.add_entity(EntityType::LwPolyline(lwpoly)).unwrap();
    x += sp;

    // LwPolyline with bulge (rounded corner)
    let mut lwbulge = LwPolyline::new();
    lwbulge.add_point(Vector2::new(x, y));
    lwbulge.add_point(Vector2::new(x + 10.0, y));
    lwbulge.add_point_with_bulge(Vector2::new(x + 10.0, y + 10.0), 0.5);
    lwbulge.add_point(Vector2::new(x, y + 10.0));
    lwbulge.is_closed = true;
    lwbulge.common.layer = "Polylines".to_string();
    doc.add_entity(EntityType::LwPolyline(lwbulge)).unwrap();
    x += sp;

    // LwPolyline with variable widths
    let mut lwwide = LwPolyline::new();
    lwwide.vertices.push(LwVertex {
        location: Vector2::new(x, y),
        bulge: 0.0,
        start_width: 0.0,
        end_width: 2.0,
    });
    lwwide.vertices.push(LwVertex {
        location: Vector2::new(x + 15.0, y + 5.0),
        bulge: 0.0,
        start_width: 2.0,
        end_width: 0.5,
    });
    lwwide.vertices.push(LwVertex {
        location: Vector2::new(x + 10.0, y + 10.0),
        bulge: 0.0,
        start_width: 0.5,
        end_width: 0.0,
    });
    lwwide.common.layer = "Polylines".to_string();
    lwwide.common.color = Color::from_index(3);
    doc.add_entity(EntityType::LwPolyline(lwwide)).unwrap();
    x += sp;

    let mut spline = Spline::new();
    spline.control_points = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 8.0, 0.0),
        Vector3::new(x + 10.0, y + 2.0, 0.0),
        Vector3::new(x + 15.0, y + 10.0, 0.0),
    ];
    spline.degree = 3;
    spline.knots = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    spline.common.layer = "Polylines".to_string();
    doc.add_entity(EntityType::Spline(spline)).unwrap();

    // === Row 3: Text ===
    x = 0.0;
    y += sp;

    let mut text = Text::with_value("Hello acadrust!", Vector3::new(x, y, 0.0)).with_height(3.0);
    text.common.layer = "Text".to_string();
    text.common.color = Color::RED;
    doc.add_entity(EntityType::Text(text)).unwrap();
    x += sp;

    let mut text2 = Text::with_value("Rotated 45deg", Vector3::new(x, y, 0.0)).with_height(2.5);
    text2.rotation = 45.0_f64.to_radians();
    text2.common.layer = "Text".to_string();
    doc.add_entity(EntityType::Text(text2)).unwrap();
    x += sp;

    let mut mtext = MText::new();
    mtext.value = "Multi-line text:\\PLine 2\\PLine 3".to_string();
    mtext.insertion_point = Vector3::new(x, y, 0.0);
    mtext.height = 2.5;
    mtext.rectangle_width = 20.0;
    mtext.common.layer = "Text".to_string();
    mtext.common.color = Color::BLUE;
    doc.add_entity(EntityType::MText(mtext)).unwrap();

    // === Row 4: Dimensions ===
    x = 0.0;
    y += sp;

    let dim_al = Dimension::Aligned(DimensionAligned::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 15.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_al)).unwrap();
    x += sp;

    let dim_lin = Dimension::Linear(DimensionLinear::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 12.0, y + 8.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_lin)).unwrap();
    x += sp;

    let dim_rad = Dimension::Radius(DimensionRadius::new(
        Vector3::new(x + 7.0, y + 5.0, 0.0),
        Vector3::new(x + 12.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_rad)).unwrap();
    x += sp;

    let dim_dia = Dimension::Diameter(DimensionDiameter::new(
        Vector3::new(x + 7.0, y + 5.0, 0.0),
        Vector3::new(x + 14.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_dia)).unwrap();

    // === Row 5: More dimensions + leaders ===
    x = 0.0;
    y += sp;

    let dim_a3 = Dimension::Angular3Pt(DimensionAngular3Pt::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 7.0, y + 7.0, 0.0),
        Vector3::new(x + 14.0, y, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_a3)).unwrap();
    x += sp;

    let dim_ord = Dimension::Ordinate(DimensionOrdinate::x_ordinate(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
    ));
    doc.add_entity(EntityType::Dimension(dim_ord)).unwrap();
    x += sp;

    let mut leader = Leader::new();
    leader.vertices = vec![
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 5.0, y + 5.0, 0.0),
        Vector3::new(x + 10.0, y + 5.0, 0.0),
    ];
    leader.arrow_enabled = true;
    leader.creation_type = LeaderCreationType::NoAnnotation;
    leader.common.layer = "Annotations".to_string();
    doc.add_entity(EntityType::Leader(leader)).unwrap();
    x += sp;

    let mut tolerance = Tolerance::new();
    tolerance.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A".to_string();
    tolerance.insertion_point = Vector3::new(x, y, 0.0);
    tolerance.direction = Vector3::UNIT_X;
    doc.add_entity(EntityType::Tolerance(tolerance)).unwrap();

    // === Row 6: Hatches ===
    x = 0.0;
    y += sp;

    let mut hatch = Hatch::new();
    hatch.pattern = HatchPattern::solid();
    hatch.is_solid = true;
    hatch.common.color = Color::from_index(1);
    hatch.paths.push(make_rect_boundary(x, y, 10.0, 10.0));
    doc.add_entity(EntityType::Hatch(hatch)).unwrap();
    x += sp;

    let mut phatch = Hatch::new();
    phatch.pattern = HatchPattern::new("ANSI31");
    phatch.is_solid = false;
    phatch.paths.push(make_rect_boundary(x, y, 12.0, 8.0));
    doc.add_entity(EntityType::Hatch(phatch)).unwrap();

    // === Row 7: Solids, faces, construction ===
    x = 0.0;
    y += sp;

    let solid = Solid::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
        Vector3::new(x + 10.0, y + 8.0, 0.0),
        Vector3::new(x, y + 8.0, 0.0),
    );
    doc.add_entity(EntityType::Solid(solid)).unwrap();
    x += sp;

    let face = Face3D::new(
        Vector3::new(x, y, 0.0),
        Vector3::new(x + 10.0, y, 0.0),
        Vector3::new(x + 10.0, y + 8.0, 5.0),
        Vector3::new(x, y + 8.0, 5.0),
    );
    doc.add_entity(EntityType::Face3D(face)).unwrap();
    x += sp;

    let ray = Ray::new(Vector3::new(x, y, 0.0), Vector3::new(1.0, 0.5, 0.0));
    doc.add_entity(EntityType::Ray(ray)).unwrap();
    x += sp;

    let xline = XLine::new(Vector3::new(x, y + 5.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
    doc.add_entity(EntityType::XLine(xline)).unwrap();

    // === Row 8: Viewport, Wipeout ===
    x = 0.0;
    y += sp;

    let mut viewport = Viewport::new();
    viewport.center = Vector3::new(x + 7.0, y + 5.0, 0.0);
    viewport.width = 14.0;
    viewport.height = 10.0;
    viewport.view_center = Vector3::new(0.0, 0.0, 0.0);
    viewport.view_height = 100.0;
    doc.add_entity(EntityType::Viewport(viewport)).unwrap();
    x += sp;

    let wipeout = Wipeout::rectangular(Vector3::new(x, y, 0.0), 12.0, 8.0);
    doc.add_entity(EntityType::Wipeout(wipeout)).unwrap();

    let count = doc.entities().count();
    println!("    -> {} entities created", count);

    doc
}

/// Build a document focused on basic geometry only.
fn build_basic_geometry(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    add_test_layers(&mut doc);

    // Lines at various angles — radial pattern
    for i in 0..12 {
        let angle = (i as f64) * PI / 6.0;
        let cx = 50.0;
        let cy = 50.0;
        let r = 20.0;
        let mut line = Line::from_coords(
            cx, cy, 0.0,
            cx + r * angle.cos(), cy + r * angle.sin(), 0.0,
        );
        line.common.color = Color::from_index((i % 7 + 1) as i16);
        line.common.layer = "Geometry".to_string();
        doc.add_entity(EntityType::Line(line)).unwrap();
    }

    // Concentric circles
    for r in 1..=5 {
        let mut circle = Circle::from_coords(50.0, 50.0, 0.0, r as f64 * 4.0);
        circle.common.layer = "Geometry".to_string();
        circle.common.color = Color::from_index(r as i16);
        doc.add_entity(EntityType::Circle(circle)).unwrap();
    }

    // Various arcs
    let sp = 25.0;
    for i in 0..4 {
        let start = (i as f64) * PI / 2.0;
        let end = start + PI / 3.0;
        let mut arc = Arc::from_coords(50.0 + sp, 50.0, 0.0, 15.0, start, end);
        arc.common.layer = "Geometry".to_string();
        arc.common.color = Color::from_index((i + 1) as i16);
        doc.add_entity(EntityType::Arc(arc)).unwrap();
    }

    // Ellipse
    let mut ellipse = Ellipse::from_center_axes(
        Vector3::new(50.0 + sp * 2.0, 50.0, 0.0),
        Vector3::new(15.0, 0.0, 0.0),
        0.4,
    );
    ellipse.common.layer = "Geometry".to_string();
    doc.add_entity(EntityType::Ellipse(ellipse)).unwrap();

    // Star polyline
    let mut star = LwPolyline::new();
    for i in 0..10 {
        let angle = (i as f64) * PI / 5.0 - PI / 2.0;
        let r = if i % 2 == 0 { 15.0 } else { 7.0 };
        star.add_point(Vector2::new(
            50.0 + sp * 3.0 + r * angle.cos(),
            50.0 + r * angle.sin(),
        ));
    }
    star.is_closed = true;
    star.common.layer = "Polylines".to_string();
    star.common.color = Color::YELLOW;
    doc.add_entity(EntityType::LwPolyline(star)).unwrap();

    // Spline S-curve
    let mut spline = Spline::new();
    spline.control_points = vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 20.0, 0.0),
        Vector3::new(20.0, -10.0, 0.0),
        Vector3::new(30.0, 15.0, 0.0),
        Vector3::new(40.0, 0.0, 0.0),
    ];
    spline.degree = 3;
    spline.knots = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
    spline.common.layer = "Polylines".to_string();
    doc.add_entity(EntityType::Spline(spline)).unwrap();

    doc
}

/// Build a document focused on text and dimensions.
fn build_text_and_dims(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    add_test_layers(&mut doc);

    let sp = 30.0;
    let mut y = 0.0;

    // Various text heights
    for (i, height) in [1.5, 2.0, 3.0, 4.0, 5.0].iter().enumerate() {
        let mut text = Text::with_value(
            &format!("Height {:.1}", height),
            Vector3::new(0.0, y, 0.0),
        ).with_height(*height);
        text.common.layer = "Text".to_string();
        text.common.color = Color::from_index((i % 7 + 1) as i16);
        doc.add_entity(EntityType::Text(text)).unwrap();
        y += height + 3.0;
    }

    // MText with formatting
    let mut mtext = MText::new();
    mtext.value = "\\A1;Centered MText\\PWith multiple lines\\PAnd formatting".to_string();
    mtext.insertion_point = Vector3::new(sp, 0.0, 0.0);
    mtext.height = 2.5;
    mtext.rectangle_width = 25.0;
    mtext.common.layer = "Text".to_string();
    doc.add_entity(EntityType::MText(mtext)).unwrap();

    // All 7 dimension types
    y = sp * 2.0;

    // 1. Aligned
    doc.add_entity(EntityType::Dimension(Dimension::Aligned(DimensionAligned::new(
        Vector3::new(0.0, y, 0.0),
        Vector3::new(20.0, y, 0.0),
    )))).unwrap();

    // 2. Linear
    y += sp;
    doc.add_entity(EntityType::Dimension(Dimension::Linear(DimensionLinear::new(
        Vector3::new(0.0, y, 0.0),
        Vector3::new(15.0, y + 8.0, 0.0),
    )))).unwrap();

    // 3. Radius
    y += sp;
    doc.add_entity(EntityType::Dimension(Dimension::Radius(DimensionRadius::new(
        Vector3::new(10.0, y + 5.0, 0.0),
        Vector3::new(18.0, y + 5.0, 0.0),
    )))).unwrap();

    // 4. Diameter
    doc.add_entity(EntityType::Dimension(Dimension::Diameter(DimensionDiameter::new(
        Vector3::new(sp + 10.0, y + 5.0, 0.0),
        Vector3::new(sp + 20.0, y + 5.0, 0.0),
    )))).unwrap();

    // 5. Angular 3-point
    y += sp;
    doc.add_entity(EntityType::Dimension(Dimension::Angular3Pt(DimensionAngular3Pt::new(
        Vector3::new(0.0, y, 0.0),
        Vector3::new(10.0, y + 10.0, 0.0),
        Vector3::new(20.0, y, 0.0),
    )))).unwrap();

    // 6. Angular 2-line
    doc.add_entity(EntityType::Dimension(Dimension::Angular2Ln(DimensionAngular2Ln::new(
        Vector3::new(sp, y, 0.0),
        Vector3::new(sp + 10.0, y + 10.0, 0.0),
        Vector3::new(sp + 20.0, y, 0.0),
    )))).unwrap();

    // 7. Ordinate (X and Y)
    y += sp;
    doc.add_entity(EntityType::Dimension(Dimension::Ordinate(DimensionOrdinate::x_ordinate(
        Vector3::new(10.0, y, 0.0),
        Vector3::new(10.0, y + 6.0, 0.0),
    )))).unwrap();

    doc.add_entity(EntityType::Dimension(Dimension::Ordinate(DimensionOrdinate::y_ordinate(
        Vector3::new(sp + 10.0, y, 0.0),
        Vector3::new(sp + 16.0, y + 5.0, 0.0),
    )))).unwrap();

    // Leader
    let mut leader = Leader::new();
    leader.vertices = vec![
        Vector3::new(sp * 2.0, y, 0.0),
        Vector3::new(sp * 2.0 + 8.0, y + 5.0, 0.0),
        Vector3::new(sp * 2.0 + 15.0, y + 5.0, 0.0),
    ];
    leader.arrow_enabled = true;
    leader.creation_type = LeaderCreationType::NoAnnotation;
    leader.common.layer = "Annotations".to_string();
    doc.add_entity(EntityType::Leader(leader)).unwrap();

    // Tolerance
    let mut tol = Tolerance::new();
    tol.text = "{\\Fgdt;j}%%v{\\Fgdt;n}0.5{\\Fgdt;m}A".to_string();
    tol.insertion_point = Vector3::new(sp * 3.0, y, 0.0);
    tol.direction = Vector3::UNIT_X;
    doc.add_entity(EntityType::Tolerance(tol)).unwrap();

    doc
}

/// Build a document focused on hatches.
fn build_hatches(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    add_test_layers(&mut doc);

    let sp = 30.0;
    let mut x = 0.0;

    // 1. Solid fill — rectangle
    let mut h1 = Hatch::new();
    h1.pattern = HatchPattern::solid();
    h1.is_solid = true;
    h1.common.color = Color::from_index(1);
    h1.paths.push(make_rect_boundary(x, 0.0, 15.0, 15.0));
    doc.add_entity(EntityType::Hatch(h1)).unwrap();
    x += sp;

    // 2. Solid fill — triangle
    let mut h2 = Hatch::new();
    h2.pattern = HatchPattern::solid();
    h2.is_solid = true;
    h2.common.color = Color::from_index(3);
    let mut tri = BoundaryPath::new();
    tri.edges.push(BoundaryEdge::Line(LineEdge { start: Vector2::new(x, 0.0), end: Vector2::new(x + 15.0, 0.0) }));
    tri.edges.push(BoundaryEdge::Line(LineEdge { start: Vector2::new(x + 15.0, 0.0), end: Vector2::new(x + 7.5, 13.0) }));
    tri.edges.push(BoundaryEdge::Line(LineEdge { start: Vector2::new(x + 7.5, 13.0), end: Vector2::new(x, 0.0) }));
    h2.paths.push(tri);
    doc.add_entity(EntityType::Hatch(h2)).unwrap();
    x += sp;

    // 3. Pattern fill — ANSI31
    let mut h3 = Hatch::new();
    h3.pattern = HatchPattern::new("ANSI31");
    h3.is_solid = false;
    h3.pattern_scale = 1.0;
    h3.paths.push(make_rect_boundary(x, 0.0, 15.0, 15.0));
    doc.add_entity(EntityType::Hatch(h3)).unwrap();
    x += sp;

    // 4. Pattern fill — different angle
    let mut h4 = Hatch::new();
    h4.pattern = HatchPattern::new("ANSI37");
    h4.is_solid = false;
    h4.pattern_scale = 2.0;
    h4.pattern_angle = PI / 4.0;
    h4.paths.push(make_rect_boundary(x, 0.0, 15.0, 15.0));
    doc.add_entity(EntityType::Hatch(h4)).unwrap();

    // Row 2 — hatch with circular arc boundary
    x = 0.0;
    let y2 = sp;

    let mut h5 = Hatch::new();
    h5.pattern = HatchPattern::solid();
    h5.is_solid = true;
    h5.common.color = Color::from_index(5);
    let mut arc_boundary = BoundaryPath::new();
    arc_boundary.edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(x + 7.5, y2 + 7.5),
        radius: 7.5,
        start_angle: 0.0,
        end_angle: 2.0 * PI,
        counter_clockwise: true,
    }));
    h5.paths.push(arc_boundary);
    doc.add_entity(EntityType::Hatch(h5)).unwrap();

    doc
}

/// Build a document with block inserts and attributes.
fn build_blocks_attribs(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    add_test_layers(&mut doc);

    // Create a block "SimpleBox"
    let mut block_rec = BlockRecord::new("SimpleBox");
    block_rec.set_handle(doc.allocate_handle());
    block_rec.block_entity_handle = doc.allocate_handle();
    block_rec.block_end_handle = doc.allocate_handle();
    doc.block_records.add(block_rec).ok();

    // Insert the block at multiple locations with different scales/rotations
    for i in 0..5 {
        let mut insert = Insert::new("SimpleBox", Vector3::new(i as f64 * 25.0, 0.0, 0.0));
        insert.x_scale = 1.0 + i as f64 * 0.2;
        insert.y_scale = 1.0 + i as f64 * 0.2;
        insert.z_scale = 1.0;
        insert.rotation = (i as f64) * 15.0_f64.to_radians();
        insert.common.layer = "Blocks".to_string();
        doc.add_entity(EntityType::Insert(insert)).unwrap();
    }

    // Attribute definitions
    for (i, (tag, prompt, default)) in [
        ("TITLE", "Enter title:", "DRAWING TITLE"),
        ("AUTHOR", "Enter author:", "John Doe"),
        ("DATE", "Enter date:", "2026-02-14"),
        ("SCALE", "Enter scale:", "1:100"),
    ].iter().enumerate() {
        let mut attdef = AttributeDefinition::new(
            tag.to_string(),
            prompt.to_string(),
            default.to_string(),
        );
        attdef.insertion_point = Vector3::new(0.0, 30.0 + i as f64 * 5.0, 0.0);
        attdef.height = 2.5;
        attdef.common.layer = "Annotations".to_string();
        doc.add_entity(EntityType::AttributeDefinition(attdef)).unwrap();
    }

    doc
}

/// Build a document with 3D entities.
fn build_3d_entities(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = version;
    add_test_layers(&mut doc);

    let sp = 30.0;
    let mut x = 0.0;

    // 3D Face — simple quad
    let face = Face3D::new(
        Vector3::new(x, 0.0, 0.0),
        Vector3::new(x + 10.0, 0.0, 0.0),
        Vector3::new(x + 10.0, 10.0, 5.0),
        Vector3::new(x, 10.0, 5.0),
    );
    doc.add_entity(EntityType::Face3D(face)).unwrap();
    x += sp;

    // Box faces
    let face2 = Face3D::new(
        Vector3::new(x, 0.0, 5.0),
        Vector3::new(x + 10.0, 0.0, 5.0),
        Vector3::new(x + 10.0, 10.0, 5.0),
        Vector3::new(x, 10.0, 5.0),
    );
    doc.add_entity(EntityType::Face3D(face2)).unwrap();

    let face3 = Face3D::new(
        Vector3::new(x, 0.0, 0.0),
        Vector3::new(x + 10.0, 0.0, 0.0),
        Vector3::new(x + 10.0, 0.0, 5.0),
        Vector3::new(x, 0.0, 5.0),
    );
    doc.add_entity(EntityType::Face3D(face3)).unwrap();
    x += sp;

    // 3D Polyline
    let mut poly3d = Polyline3D::new();
    poly3d.add_vertex(Vector3::new(x, 0.0, 0.0));
    poly3d.add_vertex(Vector3::new(x + 5.0, 5.0, 3.0));
    poly3d.add_vertex(Vector3::new(x + 10.0, 0.0, 6.0));
    poly3d.add_vertex(Vector3::new(x + 15.0, 5.0, 9.0));
    poly3d.common.layer = "3D".to_string();
    poly3d.common.color = Color::CYAN;
    doc.add_entity(EntityType::Polyline3D(poly3d)).unwrap();
    x += sp;

    // Polyface mesh — pyramid
    let mut polyface = PolyfaceMesh::new();
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, 0.0, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 10.0, 0.0, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 10.0, 10.0, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x, 10.0, 0.0)));
    polyface.add_vertex(PolyfaceVertex::new(Vector3::new(x + 5.0, 5.0, 8.0)));

    // Base
    polyface.add_face(PolyfaceFace {
        common: EntityCommon::new(),
        flags: PolyfaceVertexFlags::default(),
        index1: 1, index2: 2, index3: 3, index4: 4,
        color: Some(Color::ByLayer),
    });
    // Side faces
    for &(a, b) in &[(1, 2), (2, 3), (3, 4), (4, 1)] {
        polyface.add_face(PolyfaceFace {
            common: EntityCommon::new(),
            flags: PolyfaceVertexFlags::default(),
            index1: a, index2: b, index3: 5, index4: 0,
            color: Some(Color::ByLayer),
        });
    }
    polyface.common.layer = "3D".to_string();
    doc.add_entity(EntityType::PolyfaceMesh(polyface)).unwrap();

    // Polygon mesh (4×4 surface with sinusoidal elevation)
    let y = sp;
    let mut pgmesh = PolygonMeshEntity::new();
    pgmesh.m_vertex_count = 4;
    pgmesh.n_vertex_count = 4;
    for i in 0..4 {
        for j in 0..4 {
            let z = ((i as f64) * PI / 3.0).sin() * ((j as f64) * PI / 3.0).cos() * 5.0;
            pgmesh.vertices.push(PolygonMeshVertex::at(Vector3::new(
                i as f64 * 5.0,
                y + j as f64 * 5.0,
                z,
            )));
        }
    }
    pgmesh.common.layer = "3D".to_string();
    doc.add_entity(EntityType::PolygonMesh(pgmesh)).unwrap();

    doc
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn add_test_layers(doc: &mut CadDocument) {
    let layers = [
        ("Geometry", Color::WHITE),
        ("Polylines", Color::GREEN),
        ("Text", Color::CYAN),
        ("Annotations", Color::YELLOW),
        ("Hatches", Color::RED),
        ("Blocks", Color::MAGENTA),
        ("3D", Color::BLUE),
    ];
    for (name, color) in &layers {
        let mut layer = Layer::new(*name);
        layer.color = *color;
        let _ = doc.layers.add(layer);
    }
}

fn make_rect_boundary(x: f64, y: f64, w: f64, h: f64) -> BoundaryPath {
    let mut boundary = BoundaryPath::new();
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x, y),
        end: Vector2::new(x + w, y),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x + w, y),
        end: Vector2::new(x + w, y + h),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x + w, y + h),
        end: Vector2::new(x, y + h),
    }));
    boundary.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(x, y + h),
        end: Vector2::new(x, y),
    }));
    boundary
}
