//! Generate DWG/DXF files for CAD-application validation.
//!
//! Two groups, written to `target/cad_validate/`:
//!
//! 1. **Primitives** — ACIS solids built directly by `acadrust`'s own
//!    `entities::acis::primitives` builders (box, wedge, pyramid, cylinder,
//!    cone, sphere, torus). These establish whether the SAT/SAB writer and the
//!    DWG/DXF container are sound, independent of any geometry kernel.
//!
//! 2. **Boolean results** — solids produced by `acadrust-geom`'s
//!    `boolean_union` / `boolean_subtract` / `boolean_intersect`, converted to
//!    ACIS via `build_planar_body`, then written out. These exercise the
//!    kernel's B-Rep output through the same container.
//!
//! Splitting them this way is the point: if a primitive opens cleanly and the
//! matching boolean does not, the defect is in the kernel, not in the writer.
//!
//! Run with `cargo run --example cad_validate_gen`.

use acadrust::entities::acis::primitives;
use acadrust::entities::EntityType;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgWriter, DxfWriter};

use acadrust_geom::boolean::{boolean_intersect, boolean_subtract, boolean_union};
use acadrust_geom::brep::BRepSolid;
use acadrust_geom::point::Point3;
use acadrust_geom::tolerance::Tolerance;
use acadrust_geom::vector::Vec3;

use std::path::Path;

/// Target version. AC1032 (R2018) is what BricsCAD V20 opens natively.
const VERSION: DxfVersion = DxfVersion::AC1032;
const VER_STR: &str = "AC1032";

fn main() {
    let root = "target/cad_validate";
    let prim_dir = format!("{}/primitives", root);
    let bool_dir = format!("{}/booleans", root);
    std::fs::create_dir_all(&prim_dir).unwrap();
    std::fs::create_dir_all(&bool_dir).unwrap();

    let mut manifest: Vec<ManifestRow> = Vec::new();

    println!("== Group 1: ACIS primitives (acadrust builders) ==");
    gen_primitives(&prim_dir, &mut manifest);

    println!("\n== Group 2: boolean results (acadrust-geom kernel) ==");
    gen_booleans(&bool_dir, &mut manifest);

    // Manifest lets the BricsCAD audit step correlate a file back to what
    // produced it, and records expected volume where it is known in closed form.
    let mut csv = String::from("file,group,source,entity,faces,expected_volume\n");
    for r in &manifest {
        csv.push_str(&format!(
            "{},{},{},{},{},{}\n",
            r.file,
            r.group,
            r.source,
            r.entity,
            r.faces
                .map(|f| f.to_string())
                .unwrap_or_else(|| "-".into()),
            r.expected_volume
                .map(|v| format!("{:.6}", v))
                .unwrap_or_else(|| "-".into())
        ));
    }
    let mpath = format!("{}/manifest.csv", root);
    std::fs::write(&mpath, csv).unwrap();

    let n_ok = manifest.len();
    println!(
        "\nWrote {} file pairs (DWG+DXF) to {}",
        n_ok,
        Path::new(root).display()
    );
    println!("Manifest: {}", mpath);
}

struct ManifestRow {
    file: String,
    group: &'static str,
    source: &'static str,
    entity: &'static str,
    faces: Option<usize>,
    expected_volume: Option<f64>,
}

// ── Group 1: primitives straight from acadrust's ACIS builders ────────

fn gen_primitives(dir: &str, manifest: &mut Vec<ManifestRow>) {
    // (name, SatDocument, closed-form volume)
    let cases: Vec<(&'static str, acadrust::SatDocument, Option<f64>)> = vec![
        (
            "box",
            primitives::build_box([0.0, 0.0, 0.0], 10.0, 10.0, 10.0),
            Some(1000.0),
        ),
        (
            "wedge",
            primitives::build_wedge([0.0, 0.0, 0.0], 10.0, 10.0, 10.0),
            Some(500.0),
        ),
        (
            "pyramid",
            primitives::build_pyramid([0.0, 0.0, 0.0], 10.0, 10.0),
            Some(1000.0 / 3.0),
        ),
        (
            "cylinder",
            primitives::build_cylinder([0.0, 0.0, 0.0], 5.0, 10.0),
            Some(std::f64::consts::PI * 25.0 * 10.0),
        ),
        (
            "cone",
            primitives::build_cone([0.0, 0.0, 0.0], 5.0, 10.0),
            Some(std::f64::consts::PI * 25.0 * 10.0 / 3.0),
        ),
        (
            "sphere",
            primitives::build_sphere([0.0, 0.0, 0.0], 5.0),
            Some(4.0 / 3.0 * std::f64::consts::PI * 125.0),
        ),
        (
            "torus",
            primitives::build_torus([0.0, 0.0, 0.0], 10.0, 3.0),
            Some(2.0 * std::f64::consts::PI * std::f64::consts::PI * 10.0 * 9.0),
        ),
    ];

    for (name, sat, vol) in cases {
        let mut solid = acadrust::entities::solid3d::Solid3D::new();
        solid.set_sat_document(&sat);
        let stem = format!("prim_{}", name);
        if write_pair(dir, &stem, EntityType::Solid3D(solid)) {
            manifest.push(ManifestRow {
                file: stem,
                group: "primitive",
                source: "acadrust::primitives",
                entity: "3DSOLID",
                faces: None,
                expected_volume: vol,
            });
        }
    }
}

// ── Group 2: boolean results from the geometry kernel ─────────────────

fn tol() -> Tolerance {
    Tolerance::default()
}

fn gen_booleans(dir: &str, manifest: &mut Vec<ManifestRow>) {
    let pi = std::f64::consts::PI;

    // Overlapping boxes: every boolean result is exact in closed form, so a
    // failure here cannot be blamed on curved-surface approximation.
    let b1 = || BRepSolid::make_box(Point3::ORIGIN, 10.0, 10.0, 10.0);
    let b2 = || BRepSolid::make_box(Point3::new(5.0, 5.0, 5.0), 10.0, 10.0, 10.0);
    // Overlap is a 5×5×5 cube = 125.
    emit(dir, manifest, "box_union_box", boolean_union(&b1(), &b2(), &tol()), Some(2000.0 - 125.0));
    emit(dir, manifest, "box_subtract_box", boolean_subtract(&b1(), &b2(), &tol()), Some(1000.0 - 125.0));
    emit(dir, manifest, "box_intersect_box", boolean_intersect(&b1(), &b2(), &tol()), Some(125.0));

    // Box vs cylinder: a through-hole case, the classic CAD regression.
    let bx = || BRepSolid::make_box(Point3::ORIGIN, 10.0, 10.0, 10.0);
    let cy = || BRepSolid::make_cylinder(Point3::new(0.0, 0.0, -8.0), Vec3::Z, 3.0, 16.0);
    emit(dir, manifest, "box_subtract_cyl", boolean_subtract(&bx(), &cy(), &tol()), Some(1000.0 - pi * 9.0 * 10.0));
    emit(dir, manifest, "box_intersect_cyl", boolean_intersect(&bx(), &cy(), &tol()), Some(pi * 9.0 * 10.0));
    emit(dir, manifest, "box_union_cyl", boolean_union(&bx(), &cy(), &tol()), None);

    // Box vs sphere, overlapping.
    let bx4 = || BRepSolid::make_box(Point3::ORIGIN, 10.0, 10.0, 10.0);
    let sp = || BRepSolid::make_sphere(Point3::new(4.0, 4.0, 4.0), 5.0);
    emit(dir, manifest, "box_union_sphere", boolean_union(&bx4(), &sp(), &tol()), None);
    emit(dir, manifest, "box_subtract_sphere", boolean_subtract(&bx4(), &sp(), &tol()), None);
    emit(dir, manifest, "box_intersect_sphere", boolean_intersect(&bx4(), &sp(), &tol()), None);

    // Sphere vs sphere: lens volume is closed form,
    //   V = π(4r + d)(2r - d)²/12  for equal radii r at centre distance d.
    let r = 5.0;
    let d = 4.0;
    let s1 = || BRepSolid::make_sphere(Point3::ORIGIN, r);
    let s2 = || BRepSolid::make_sphere(Point3::new(d, 0.0, 0.0), r);
    let lens = pi * (4.0 * r + d) * (2.0 * r - d) * (2.0 * r - d) / 12.0;
    let sphere_vol = 4.0 / 3.0 * pi * r * r * r;
    emit(dir, manifest, "sphere_intersect_sphere", boolean_intersect(&s1(), &s2(), &tol()), Some(lens));
    emit(dir, manifest, "sphere_union_sphere", boolean_union(&s1(), &s2(), &tol()), Some(2.0 * sphere_vol - lens));
    emit(dir, manifest, "sphere_subtract_sphere", boolean_subtract(&s1(), &s2(), &tol()), Some(sphere_vol - lens));

    // Containment: B strictly inside A. The kernel audit found this path exact,
    // so it separates container/writer problems from intersection-curve ones.
    let big = || BRepSolid::make_box(Point3::ORIGIN, 20.0, 20.0, 20.0);
    let small = || BRepSolid::make_sphere(Point3::ORIGIN, 5.0);
    emit(dir, manifest, "contained_subtract", boolean_subtract(&big(), &small(), &tol()), Some(8000.0 - sphere_vol));
    emit(dir, manifest, "contained_intersect", boolean_intersect(&big(), &small(), &tol()), Some(sphere_vol));

    // Cone and torus participants, to cover the remaining primitive surfaces.
    let cn = || BRepSolid::make_cone(Point3::ORIGIN, Vec3::Z, 5.0, 12.0);
    let cs = || BRepSolid::make_sphere(Point3::new(0.0, 0.0, 4.0), 3.0);
    emit(dir, manifest, "cone_subtract_sphere", boolean_subtract(&cn(), &cs(), &tol()), None);
    let tr = || BRepSolid::make_torus(Point3::ORIGIN, Vec3::Z, 10.0, 3.0);
    let tb = || BRepSolid::make_box(Point3::new(10.0, 0.0, 0.0), 8.0, 8.0, 8.0);
    emit(dir, manifest, "torus_subtract_box", boolean_subtract(&tr(), &tb(), &tol()), None);
}

/// Convert a boolean result to ACIS and write it. Faceted: each B-Rep face
/// becomes a planar ACIS face through its loop vertices, which is what
/// `build_planar_body` accepts. Curved faces are therefore tessellated by
/// their existing edge topology rather than exported analytically.
fn emit(
    dir: &str,
    manifest: &mut Vec<ManifestRow>,
    name: &'static str,
    result: std::result::Result<BRepSolid, acadrust_geom::boolean::BooleanError>,
    expected_volume: Option<f64>,
) {
    let solid = match result {
        Ok(s) => s,
        Err(e) => {
            println!("  SKIP {:<26} boolean failed: {:?}", name, e);
            return;
        }
    };

    let (verts, faces, shell_of_face) = match brep_to_planar_soup(&solid) {
        Some(v) => v,
        None => {
            println!("  SKIP {:<26} no usable planar faces", name);
            return;
        }
    };
    let n_faces = faces.len();

    // Multi-shell results are expressible: `build_planar_body_shells` emits one
    // ACIS lump per shell, chained through `next_lump`. Subtracting a fully
    // interior solid leaves an outer boundary plus a disjoint cavity, and each
    // becomes its own shell rather than being flattened into one (which is what
    // ACIS rejects as "Entities in shell are not connected").

    let sat = match primitives::build_planar_body_shells(&verts, &faces, &shell_of_face) {
        Some(s) => s,
        None => {
            // ACIS requires a closed manifold shell: every edge used by exactly
            // two coedges. Boolean output that fails this still has usable face
            // geometry, so fall back to POLYFACE_MESH — an unstructured face
            // set with no manifold requirement. This keeps every case testable
            // in BricsCAD and separates "bad topology" from "bad geometry".
            println!(
                "  MESH {:<26} ACIS rejected ({} verts / {} faces), wrote POLYFACE_MESH",
                name,
                verts.len(),
                n_faces
            );
            emit_mesh(dir, manifest, name, &verts, &faces, expected_volume);
            return;
        }
    };

    let mut s3d = acadrust::entities::solid3d::Solid3D::new();
    s3d.set_sat_document(&sat);
    let stem = format!("bool_{}", name);
    // A void shell has to go out as inline SAT; see `write_pair_ver`.
    let multi_shell = solid.num_shells() > 1;
    let (ver, vtag) = if multi_shell {
        (DxfVersion::AC1024, "AC1024")
    } else {
        (VERSION, VER_STR)
    };
    if write_pair_ver(dir, &stem, EntityType::Solid3D(s3d), ver, vtag) {
        println!(
            "  OK   {:<26} {} verts, {} faces",
            name,
            verts.len(),
            n_faces
        );
        manifest.push(ManifestRow {
            file: stem,
            group: "boolean",
            source: "acadrust-geom::boolean",
            entity: "3DSOLID",
            faces: Some(n_faces),
            expected_volume,
        });
    }
}

/// Write a face soup as POLYFACE_MESH. Used when ACIS conversion is blocked by
/// non-manifold topology. DXF polyface vertex indices are `i16` and 1-based,
/// and faces carry at most 4 vertices, so larger loops are fan-triangulated.
fn emit_mesh(
    dir: &str,
    manifest: &mut Vec<ManifestRow>,
    name: &'static str,
    verts: &[[f64; 3]],
    faces: &[Vec<Vec<usize>>],
    expected_volume: Option<f64>,
) {
    use acadrust::entities::polyface_mesh::PolyfaceMesh;

    if verts.len() > i16::MAX as usize {
        println!("  SKIP {:<26} {} verts exceeds i16 index range", name, verts.len());
        return;
    }

    let mut mesh = PolyfaceMesh::new();
    for v in verts {
        mesh.add_vertex_xyz(v[0], v[1], v[2]);
    }
    let mut n_emitted = 0usize;
    // POLYFACE_MESH has no loop structure, so hole loops are emitted as
    // additional faces; this path is only a fallback for solids ACIS rejects.
    for f in faces.iter().flat_map(|lps| lps.iter()) {
        let idx: Vec<i16> = f.iter().map(|&i| (i + 1) as i16).collect();
        match idx.len() {
            3 => {
                mesh.add_triangle(idx[0], idx[1], idx[2]);
                n_emitted += 1;
            }
            4 => {
                mesh.add_quad(idx[0], idx[1], idx[2], idx[3]);
                n_emitted += 1;
            }
            n if n > 4 => {
                for k in 1..n - 1 {
                    mesh.add_triangle(idx[0], idx[k], idx[k + 1]);
                    n_emitted += 1;
                }
            }
            _ => {}
        }
    }

    let stem = format!("mesh_{}", name);
    if write_pair(dir, &stem, EntityType::PolyfaceMesh(mesh)) {
        manifest.push(ManifestRow {
            file: stem,
            group: "boolean-mesh",
            source: "acadrust-geom::boolean",
            entity: "POLYFACE_MESH",
            faces: Some(n_emitted),
            expected_volume,
        });
    }
}

/// Extract a welded vertex list plus per-face loop lists from a `BRepSolid`.
///
/// Each face becomes `vec![outer_loop, inner_loop, ...]`. Inner loops are carried
/// through rather than dropped: a boolean that pierces a face leaves a hole loop
/// there, and discarding it would emit a solid whose face is unpierced — and
/// whose edge use counts no longer balance, so ACIS would reject it.
///
/// Vertices are welded on a 1e-6 grid so both sides of a seam reference one
/// vertex record.
fn brep_to_planar_soup(
    solid: &BRepSolid,
) -> Option<(Vec<[f64; 3]>, Vec<Vec<Vec<usize>>>, Vec<usize>)> {
    use std::collections::HashMap;

    let key = |p: Point3| {
        (
            (p.x * 1e6).round() as i64,
            (p.y * 1e6).round() as i64,
            (p.z * 1e6).round() as i64,
        )
    };

    let mut verts: Vec<[f64; 3]> = Vec::new();
    let mut index: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let mut faces: Vec<Vec<Vec<usize>>> = Vec::new();
    let mut shell_of_face: Vec<usize> = Vec::new();

    // Which shell each face belongs to, so multi-shell results (an outer
    // boundary plus a cavity) become one ACIS lump per shell.
    let mut shell_index: HashMap<usize, usize> = HashMap::new();
    for (si, (_, sh)) in solid.shells.iter().enumerate() {
        for f in &sh.faces {
            shell_index.insert(f.raw().index, si);
        }
    }

    for (fid, face) in solid.faces.iter() {
        let mut face_loops: Vec<Vec<usize>> = Vec::new();

        let loop_ids = std::iter::once(face.outer_loop).chain(face.inner_loops.iter().copied());
        for lid in loop_ids {
            let mut loop_idx: Vec<usize> = Vec::new();
            for he_id in solid.loop_half_edges(lid) {
                let he = match solid.half_edges.get(he_id.raw()) {
                    Some(h) => h,
                    None => continue,
                };
                let p = match solid.vertices.get(he.start_vertex.raw()) {
                    Some(v) => v.point,
                    None => continue,
                };
                let k = key(p);
                let idx = *index.entry(k).or_insert_with(|| {
                    verts.push([p.x, p.y, p.z]);
                    verts.len() - 1
                });
                // Drop consecutive duplicates introduced by welding.
                if loop_idx.last() != Some(&idx) {
                    loop_idx.push(idx);
                }
            }
            // Close-the-loop duplicate.
            if loop_idx.len() > 1 && loop_idx.first() == loop_idx.last() {
                loop_idx.pop();
            }
            if loop_idx.len() >= 3 {
                face_loops.push(loop_idx);
            }
        }

        if !face_loops.is_empty() {
            shell_of_face.push(shell_index.get(&fid.index).copied().unwrap_or(0));
            faces.push(face_loops);
        }
    }

    if faces.is_empty() || verts.len() < 4 {
        return None;
    }
    Some((verts, faces, shell_of_face))
}

// ── Writing ──────────────────────────────────────────────────────────

/// Write the same entity as both DWG and DXF. Two containers, one geometry:
/// if DWG fails and DXF opens, the fault is in the DWG writer rather than
/// in the geometry.
fn write_pair(dir: &str, stem: &str, entity: EntityType) -> bool {
    write_pair_ver(dir, stem, entity, VERSION, VER_STR)
}

/// Write the same entity as both DWG and DXF at a given version.
///
/// Solids carrying a void shell are written at AC1024 rather than AC1032.
/// Measured in BricsCAD V20 with a minimal hollow cube: AC1015 through AC1024
/// audit clean, AC1027 and AC1032 report "Data stream is empty" and discard the
/// solid. That boundary is exactly `needs_sab()` in the DXF writer — AC1027+
/// routes ACIS through SAB in the ACDSDATA section, and our SAB encoding of a
/// chained `next_shell` is not accepted even though the byte layout matches the
/// SAT that is accepted, and round-trips through our own SabReader with zero
/// validation errors. The geometry is valid ACIS; only the SAB path is at fault.
fn write_pair_ver(
    dir: &str,
    stem: &str,
    entity: EntityType,
    version: DxfVersion,
    ver_str: &str,
) -> bool {
    let mut doc = CadDocument::with_version(version);
    if let Err(e) = doc.add_entity(entity) {
        println!("  FAIL {:<26} add_entity: {:?}", stem, e);
        return false;
    }

    let dwg_path = format!("{}/{}_{}.dwg", dir, stem, ver_str);
    let dxf_path = format!("{}/{}_{}.dxf", dir, stem, ver_str);

    let dwg_ok = match DwgWriter::write_to_file(&dwg_path, &doc) {
        Ok(()) => true,
        Err(e) => {
            println!("  FAIL {:<26} DWG write: {:?}", stem, e);
            false
        }
    };
    let dxf_ok = match DxfWriter::new(&doc).write_to_file(&dxf_path) {
        Ok(()) => true,
        Err(e) => {
            println!("  FAIL {:<26} DXF write: {:?}", stem, e);
            false
        }
    };
    dwg_ok && dxf_ok
}
