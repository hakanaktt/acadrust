use acadrust::entities::acis::{SatDocument, SatPointer, SatToken, Sense, Sidedness};
use acadrust::entities::dimension::DimensionLinear;
use acadrust::entities::hatch::{
    BoundaryEdge, BoundaryPath, BoundaryPathFlags, LineEdge, PolylineEdge,
};
use acadrust::entities::mesh::Mesh;
use acadrust::entities::mline::MLine;
use acadrust::entities::multileader::MultiLeader;
use acadrust::entities::polyface_mesh::PolyfaceMesh;
/// Generate DWG files for every supported entity type × every DWG version.
///
/// Output structure:  target/entities_dwg/<VERSION>/entity_<VERSION>_<TYPE>.dwg
///
/// Versions: AC1012 (R13), AC1014 (R14), AC1015 (R2000), AC1018 (R2004),
///           AC1021 (R2007), AC1024 (R2010), AC1027 (R2013), AC1032 (R2018)
///
/// Some entity types are only available from certain versions onwards.
/// MESH and MULTILEADER require class-based type codes (R2000+).
use acadrust::entities::*;
use acadrust::types::{DxfVersion, Vector2, Vector3};
use acadrust::{BlockRecord, CadDocument, DwgWriter, TableEntry};

/// All DWG versions to test
const VERSIONS: &[(DxfVersion, &str)] = &[
    (DxfVersion::AC1012, "AC1012"),
    (DxfVersion::AC1014, "AC1014"),
    (DxfVersion::AC1015, "AC1015"),
    (DxfVersion::AC1018, "AC1018"),
    (DxfVersion::AC1021, "AC1021"),
    (DxfVersion::AC1024, "AC1024"),
    (DxfVersion::AC1027, "AC1027"),
    (DxfVersion::AC1032, "AC1032"),
];

fn main() {
    let root = "target/entities_dwg";
    let mut total_ok = 0u32;
    let mut total_fail = 0u32;
    let mut total_skip = 0u32;

    for &(version, ver_str) in VERSIONS {
        let dir = format!("{}/{}", root, ver_str);
        std::fs::create_dir_all(&dir).unwrap();

        let mut ok = 0u32;
        let mut fail = 0u32;
        let mut skip = 0u32;

        println!("\n╔══════════════════════════════════════════════════════╗");
        println!(
            "║  {} ({})                                    ║",
            ver_str,
            version_label(version)
        );
        println!("╚══════════════════════════════════════════════════════╝");

        // ── Simple geometry (all versions) ──────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "POINT",
            &mut ok,
            &mut fail,
            &mut skip,
            || EntityType::Point(Point::from_coords(50.0, 50.0, 0.0)),
        );

        gen(
            version,
            ver_str,
            &dir,
            "LINE",
            &mut ok,
            &mut fail,
            &mut skip,
            || EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 100.0, 100.0, 0.0)),
        );

        gen(
            version,
            ver_str,
            &dir,
            "CIRCLE",
            &mut ok,
            &mut fail,
            &mut skip,
            || EntityType::Circle(Circle::from_coords(50.0, 50.0, 0.0, 25.0)),
        );

        gen(
            version,
            ver_str,
            &dir,
            "ARC",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Arc(Arc::from_coords(
                    50.0,
                    50.0,
                    0.0,
                    25.0,
                    0.0,
                    std::f64::consts::PI,
                ))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "ELLIPSE",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Ellipse(Ellipse::from_center_axes(
                    Vector3::new(50.0, 50.0, 0.0),
                    Vector3::new(40.0, 0.0, 0.0),
                    0.5,
                ))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "RAY",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Ray(Ray::new(
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(1.0, 1.0, 0.0),
                ))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "XLINE",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::XLine(XLine::new(
                    Vector3::new(50.0, 50.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                ))
            },
        );

        // ── Solid / Surface (all versions) ──────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "SOLID",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Solid(Solid::new(
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(10.0, 0.0, 0.0),
                    Vector3::new(10.0, 10.0, 0.0),
                    Vector3::new(0.0, 10.0, 0.0),
                ))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "FACE3D",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Face3D(Face3D::new(
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(10.0, 0.0, 0.0),
                    Vector3::new(10.0, 10.0, 5.0),
                    Vector3::new(0.0, 10.0, 5.0),
                ))
            },
        );

        // ── Text (all versions) ─────────────────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "TEXT",
            &mut ok,
            &mut fail,
            &mut skip,
            || EntityType::Text(Text::with_value("Hello World", Vector3::new(0.0, 0.0, 0.0))),
        );

        gen(
            version,
            ver_str,
            &dir,
            "MTEXT",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::MText(MText::with_value(
                    "Multi\\Pline\\PText",
                    Vector3::new(0.0, 0.0, 0.0),
                ))
            },
        );

        // ── Polylines (all versions) ────────────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "LWPOLYLINE",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::LwPolyline(LwPolyline::from_points(vec![
                    Vector2::new(0.0, 0.0),
                    Vector2::new(10.0, 0.0),
                    Vector2::new(10.0, 10.0),
                    Vector2::new(0.0, 10.0),
                ]))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "POLYLINE2D",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                let mut pl = Polyline2D::new();
                pl.add_vertex(Vertex2D::new(Vector3::new(0.0, 0.0, 0.0)));
                pl.add_vertex(Vertex2D::new(Vector3::new(20.0, 0.0, 0.0)));
                pl.add_vertex(Vertex2D::new(Vector3::new(20.0, 20.0, 0.0)));
                EntityType::Polyline2D(pl)
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "POLYLINE3D",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Polyline3D(Polyline3D::from_points(vec![
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(10.0, 0.0, 5.0),
                    Vector3::new(20.0, 10.0, 10.0),
                ]))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "SPLINE",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Spline(Spline::from_control_points(
                    3,
                    vec![
                        Vector3::new(0.0, 0.0, 0.0),
                        Vector3::new(5.0, 10.0, 0.0),
                        Vector3::new(10.0, 0.0, 0.0),
                        Vector3::new(15.0, 10.0, 0.0),
                    ],
                ))
            },
        );

        // ── Annotations (all versions) ──────────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "LEADER",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Leader(Leader::two_point(
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(10.0, 10.0, 0.0),
                ))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "DIMENSION",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Dimension(Dimension::Linear(DimensionLinear::new(
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(100.0, 0.0, 0.0),
                )))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "TOLERANCE",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Tolerance(Tolerance::with_text(
                    Vector3::new(10.0, 10.0, 0.0),
                    "{\\Fgdt;p}%%v0.5",
                ))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "SHAPE",
            &mut ok,
            &mut fail,
            &mut skip,
            || EntityType::Shape(Shape::with_name(Vector3::new(50.0, 50.0, 0.0), "BOX", 5.0)),
        );

        gen(
            version,
            ver_str,
            &dir,
            "VIEWPORT",
            &mut ok,
            &mut fail,
            &mut skip,
            || EntityType::Viewport(Viewport::new()),
        );

        gen_insert(
            version, ver_str, &dir, "INSERT", &mut ok, &mut fail, &mut skip,
        );

        // ── Hatch (all versions) ────────────────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "HATCH_SOLID",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                let mut hatch = Hatch::solid();
                let mut path = BoundaryPath::new();
                path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
                    vec![
                        Vector2::new(0.0, 0.0),
                        Vector2::new(100.0, 0.0),
                        Vector2::new(100.0, 100.0),
                        Vector2::new(0.0, 100.0),
                    ],
                    true,
                )));
                hatch.add_path(path);
                EntityType::Hatch(hatch)
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "HATCH_LINES",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                let mut hatch = Hatch::solid();
                let mut path = BoundaryPath::with_flags(BoundaryPathFlags::new());
                path.add_edge(BoundaryEdge::Line(LineEdge {
                    start: Vector2::new(0.0, 0.0),
                    end: Vector2::new(50.0, 0.0),
                }));
                path.add_edge(BoundaryEdge::Line(LineEdge {
                    start: Vector2::new(50.0, 0.0),
                    end: Vector2::new(50.0, 50.0),
                }));
                path.add_edge(BoundaryEdge::Line(LineEdge {
                    start: Vector2::new(50.0, 50.0),
                    end: Vector2::new(0.0, 0.0),
                }));
                hatch.add_path(path);
                EntityType::Hatch(hatch)
            },
        );

        // ── MLine (all versions) ────────────────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "MLINE",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::MLine(MLine::from_points(&[
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(50.0, 0.0, 0.0),
                    Vector3::new(50.0, 50.0, 0.0),
                ]))
            },
        );

        // ── PolyfaceMesh (all versions) ─────────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "POLYFACE",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                let mut pf = PolyfaceMesh::new();
                let v1 = pf.add_vertex_xyz(0.0, 0.0, 0.0);
                let v2 = pf.add_vertex_xyz(10.0, 0.0, 0.0);
                let v3 = pf.add_vertex_xyz(5.0, 10.0, 0.0);
                let v4 = pf.add_vertex_xyz(10.0, 10.0, 5.0);
                pf.add_triangle(v1, v2, v3);
                pf.add_triangle(v2, v4, v3);
                EntityType::PolyfaceMesh(pf)
            },
        );

        // ── MultiLeader (R2000+ / class-based) ─────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "MULTILEADER",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::MultiLeader(MultiLeader::with_text(
                    "Label",
                    Vector3::new(20.0, 20.0, 0.0),
                    vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)],
                ))
            },
        );

        // ── Mesh (R2000+ / class-based) ─────────────────────────────

        gen(
            version,
            ver_str,
            &dir,
            "MESH",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                EntityType::Mesh(Mesh::from_triangles(
                    vec![
                        Vector3::new(0.0, 0.0, 0.0),
                        Vector3::new(10.0, 0.0, 0.0),
                        Vector3::new(5.0, 10.0, 5.0),
                    ],
                    &[(0, 1, 2)],
                ))
            },
        );

        // ── ACIS entities (3DSOLID, REGION, BODY) ───────────────────

        gen(
            version,
            ver_str,
            &dir,
            "3DSOLID",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                let sat_doc = build_cylinder_sat();
                EntityType::Solid3D(Solid3D::from_sat(&sat_doc.to_sat_string()))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "REGION",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                let sat = "700 0 1 0\n\
                       @7 unknown 12 ACIS 7.0 NT 24 Wed Jan 01 00:00:00 2025 1.0 9.9999999999999995e-007 1e-010\n\
                       body $-1 $1 $-1 $-1 #\n\
                       lump $-1 $-1 $2 $0 #\n\
                       shell $-1 $-1 $-1 $3 $-1 $1 #\n\
                       face $-1 $-1 $-1 $4 $2 $5 forward single #\n\
                       loop $-1 $-1 $6 $3 #\n\
                       plane-surface $-1 0 0 0 0 0 1 1 0 0 forward_v I I I I #\n\
                       coedge $-1 $6 $6 $-1 $7 forward $4 $-1 #\n\
                       edge $-1 $8 0 $8 1 $6 $9 forward #\n\
                       vertex $-1 $7 $10 #\n\
                       straight-curve $-1 -5 -5 0 1 0 0 I I #\n\
                       point $-1 -5 -5 0 #\n\
                       End-of-ACIS-data\n";
                EntityType::Region(Region::from_sat(sat))
            },
        );

        gen(
            version,
            ver_str,
            &dir,
            "BODY",
            &mut ok,
            &mut fail,
            &mut skip,
            || {
                let sat = "700 0 1 0\n\
                       @7 unknown 12 ACIS 7.0 NT 24 Wed Jan 01 00:00:00 2025 1.0 9.9999999999999995e-007 1e-010\n\
                       body $-1 $1 $-1 $-1 #\n\
                       lump $-1 $-1 $2 $0 #\n\
                       shell $-1 $-1 $-1 $3 $-1 $1 #\n\
                       face $-1 $-1 $-1 $4 $2 $5 forward single #\n\
                       loop $-1 $-1 $6 $3 #\n\
                       plane-surface $-1 0 0 0 0 0 1 1 0 0 forward_v I I I I #\n\
                       coedge $-1 $6 $6 $-1 $7 forward $4 $-1 #\n\
                       edge $-1 $8 0 $8 1 $6 $9 forward #\n\
                       vertex $-1 $7 $10 #\n\
                       straight-curve $-1 -5 -5 0 1 0 0 I I #\n\
                       point $-1 -5 -5 0 #\n\
                       End-of-ACIS-data\n";
                EntityType::Body(Body::from_sat(sat))
            },
        );

        // ── Version summary ─────────────────────────────────────────

        println!(
            "  ── {} summary: {} OK, {} FAIL, {} SKIP",
            ver_str, ok, fail, skip
        );
        total_ok += ok;
        total_fail += fail;
        total_skip += skip;
    }

    // ── Grand total ─────────────────────────────────────────────────

    println!("\n╔══════════════════════════════════════════════════════╗");
    println!(
        "║  GRAND TOTAL: {} OK, {} FAIL, {} SKIP               ║",
        total_ok, total_fail, total_skip
    );
    println!("╚══════════════════════════════════════════════════════╝");

    if total_fail > 0 {
        std::process::exit(1);
    }
}

fn version_label(v: DxfVersion) -> &'static str {
    match v {
        DxfVersion::AC1012 => "R13",
        DxfVersion::AC1014 => "R14",
        DxfVersion::AC1015 => "R2000",
        DxfVersion::AC1018 => "R2004",
        DxfVersion::AC1021 => "R2007",
        DxfVersion::AC1024 => "R2010",
        DxfVersion::AC1027 => "R2013",
        DxfVersion::AC1032 => "R2018",
        _ => "???",
    }
}

fn gen<F>(
    version: DxfVersion,
    ver_str: &str,
    dir: &str,
    name: &str,
    ok: &mut u32,
    fail: &mut u32,
    _skip: &mut u32,
    make_entity: F,
) where
    F: FnOnce() -> EntityType,
{
    let path = format!("{}/entity_{}_{}.dwg", dir, ver_str, name);
    let mut doc = CadDocument::with_version(version);
    let entity = make_entity();
    if let Err(e) = doc.add_entity(entity) {
        println!("  SKIP {:<20} add_entity error: {:?}", name, e);
        *_skip += 1;
        return;
    }
    match DwgWriter::write_to_file(&path, &doc) {
        Ok(()) => {
            let sz = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("  OK   {:<20} ({} bytes)", name, sz);
            *ok += 1;
        }
        Err(e) => {
            println!("  FAIL {:<20} {:?}", name, e);
            *fail += 1;
        }
    }
}

/// Generate a DWG file with a proper block definition + INSERT reference.
///
/// Creates a user-defined block "TestBlock" containing a circle and a line,
/// then inserts it at (50, 50, 0). This avoids the cyclic reference that
/// would occur if INSERT referenced *Model_Space.
fn gen_insert(
    version: DxfVersion,
    ver_str: &str,
    dir: &str,
    name: &str,
    ok: &mut u32,
    fail: &mut u32,
    _skip: &mut u32,
) {
    let path = format!("{}/entity_{}_{}.dwg", dir, ver_str, name);
    let mut doc = CadDocument::with_version(version);

    // 1. Create a block record for "TestBlock"
    let mut br = BlockRecord::new("TestBlock");
    let br_handle = doc.allocate_handle();
    br.set_handle(br_handle);
    br.block_entity_handle = doc.allocate_handle();
    br.block_end_handle = doc.allocate_handle();
    if let Err(e) = doc.block_records.add(br) {
        println!("  SKIP {:<20} block_records.add error: {}", name, e);
        *_skip += 1;
        return;
    }

    // 2. Add geometry to the block by pre-setting owner_handle
    let mut circle = EntityType::Circle(Circle::from_coords(0.0, 0.0, 0.0, 10.0));
    circle.common_mut().owner_handle = br_handle;
    if let Err(e) = doc.add_entity(circle) {
        println!("  SKIP {:<20} add circle error: {:?}", name, e);
        *_skip += 1;
        return;
    }

    let mut line = EntityType::Line(Line::from_coords(-10.0, 0.0, 0.0, 10.0, 0.0, 0.0));
    line.common_mut().owner_handle = br_handle;
    if let Err(e) = doc.add_entity(line) {
        println!("  SKIP {:<20} add line error: {:?}", name, e);
        *_skip += 1;
        return;
    }

    // 3. Add an INSERT entity referencing "TestBlock"
    let insert = Insert::new("TestBlock", Vector3::new(50.0, 50.0, 0.0));
    if let Err(e) = doc.add_entity(EntityType::Insert(insert)) {
        println!("  SKIP {:<20} add_entity error: {:?}", name, e);
        *_skip += 1;
        return;
    }

    match DwgWriter::write_to_file(&path, &doc) {
        Ok(()) => {
            let sz = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            println!("  OK   {:<20} ({} bytes)", name, sz);
            *ok += 1;
        }
        Err(e) => {
            println!("  FAIL {:<20} {:?}", name, e);
            *fail += 1;
        }
    }
}

/// Build a cylinder SAT with radius 5, height 10, along Z-axis.
///
/// Bottom circle center at (0,0,0), top at (0,0,10).
/// 3 faces (bottom cap, top cap, lateral), 3 edges, 2 vertices.
fn build_cylinder_sat() -> SatDocument {
    let mut sat = SatDocument::new_body();
    let body_idx = SatPointer::new(0);
    let ptr = |i: i32| SatPointer::new(i);

    let tau = std::f64::consts::TAU;

    // Points
    let p0 = sat.add_point(5.0, 0.0, 0.0); // bottom seam
    let p1 = sat.add_point(5.0, 0.0, 10.0); // top seam

    // Surfaces
    let surf_bot = sat.add_plane_surface([0.0, 0.0, 0.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0]);
    let surf_top = sat.add_plane_surface([0.0, 0.0, 10.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    let surf_cyl = sat.add_cone_surface(
        [0.0, 0.0, 0.0], // center
        [0.0, 0.0, 1.0], // axis
        [5.0, 0.0, 0.0], // major-axis (radius = 5)
        1.0,             // ratio (circular)
        1.0,             // cos(half-angle) = 1 → cylinder
        0.0,             // sin(half-angle) = 0 → cylinder
    );

    // Curves
    let crv_bot = sat.add_ellipse_curve([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [5.0, 0.0, 0.0], 1.0);
    let crv_top = sat.add_ellipse_curve([0.0, 0.0, 10.0], [0.0, 0.0, 1.0], [5.0, 0.0, 0.0], 1.0);
    let crv_seam = sat.add_straight_curve([5.0, 0.0, 0.0], [0.0, 0.0, 1.0]);

    // Vertices
    let v0 = sat.add_vertex(SatPointer::NULL, ptr(p0));
    let v1 = sat.add_vertex(SatPointer::NULL, ptr(p1));

    // Edges
    let e_bot = sat.add_edge(
        ptr(v0),
        0.0,
        ptr(v0),
        tau,
        SatPointer::NULL,
        ptr(crv_bot),
        Sense::Forward,
    );
    let e_top = sat.add_edge(
        ptr(v1),
        0.0,
        ptr(v1),
        tau,
        SatPointer::NULL,
        ptr(crv_top),
        Sense::Forward,
    );
    let e_seam = sat.add_edge(
        ptr(v0),
        0.0,
        ptr(v1),
        10.0,
        SatPointer::NULL,
        ptr(crv_seam),
        Sense::Forward,
    );

    // Coedge indices
    let base = sat.records.len() as i32;
    let co = |i: i32| base + i;
    let loop_base = base + 6;
    let face_base = base + 9;
    let shell_idx = base + 12;
    let lump_idx = base + 13;

    // Bottom cap coedge
    sat.add_coedge(
        ptr(co(0)),
        ptr(co(0)),
        ptr(co(4)),
        ptr(e_bot),
        Sense::Reversed,
        ptr(loop_base),
    );
    // Top cap coedge
    sat.add_coedge(
        ptr(co(1)),
        ptr(co(1)),
        ptr(co(2)),
        ptr(e_top),
        Sense::Forward,
        ptr(loop_base + 1),
    );
    // Lateral face coedges (4)
    sat.add_coedge(
        ptr(co(5)),
        ptr(co(3)),
        ptr(co(1)),
        ptr(e_top),
        Sense::Reversed,
        ptr(loop_base + 2),
    );
    sat.add_coedge(
        ptr(co(2)),
        ptr(co(4)),
        ptr(co(5)),
        ptr(e_seam),
        Sense::Forward,
        ptr(loop_base + 2),
    );
    sat.add_coedge(
        ptr(co(3)),
        ptr(co(5)),
        ptr(co(0)),
        ptr(e_bot),
        Sense::Forward,
        ptr(loop_base + 2),
    );
    sat.add_coedge(
        ptr(co(4)),
        ptr(co(2)),
        ptr(co(3)),
        ptr(e_seam),
        Sense::Reversed,
        ptr(loop_base + 2),
    );

    // Loops
    sat.add_loop(SatPointer::NULL, ptr(co(0)), ptr(face_base));
    sat.add_loop(SatPointer::NULL, ptr(co(1)), ptr(face_base + 1));
    sat.add_loop(SatPointer::NULL, ptr(co(2)), ptr(face_base + 2));

    // Faces
    sat.add_face(
        ptr(face_base + 1),
        ptr(loop_base),
        ptr(shell_idx),
        ptr(surf_bot),
        Sense::Forward,
        Sidedness::Single,
    );
    sat.add_face(
        ptr(face_base + 2),
        ptr(loop_base + 1),
        ptr(shell_idx),
        ptr(surf_top),
        Sense::Forward,
        Sidedness::Single,
    );
    sat.add_face(
        SatPointer::NULL,
        ptr(loop_base + 2),
        ptr(shell_idx),
        ptr(surf_cyl),
        Sense::Forward,
        Sidedness::Single,
    );

    // Shell → Lump → Body
    sat.add_shell(ptr(face_base), ptr(lump_idx));
    sat.add_lump(ptr(shell_idx), body_idx);

    if let Some(body_rec) = sat.record_mut(0) {
        body_rec.tokens[1] = SatToken::Pointer(ptr(lump_idx));
    }

    sat
}
