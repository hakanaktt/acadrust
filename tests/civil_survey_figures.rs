//! Survey figures of the application's tutorial drawings: the census, the
//! elevation range and the traverse vertices the records decode to.
//!
//! Oracles:
//! - elevations: the live application on Survey-3.dwg
//!   (`SurveyFigure.MinElevation` / `MaxElevation`, execute_civil_code,
//!   2026-11-24) — verbatim.
//! - traverse vertices: the live application's `BaseCurve` per-PI reads on
//!   Survey-3.dwg (2026-11-24), verified under the shared solved-point
//!   tolerance (`alignment::same_solved_point`, user ruling 2026-11-25) —
//!   the record caches a display cache ~250 ulps off on x/y, exact on z
//!   (save BLDG9's first vertex). LOT3's closed-loop lengths recompute
//!   within the same tolerance of the live `Length2D` / `Length3D`.

use acadrust::classes::civil::SurveyFigure;
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::io::dwg::DwgReader;
use acadrust::types::Vector3;
use std::path::Path;

fn figures(dir: &std::ffi::OsStr, drawing: &str) -> Vec<SurveyFigure> {
    let path = Path::new(dir).join(drawing);
    let doc = DwgReader::from_file(&path).unwrap().read().unwrap();
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_SVFIGURE" => a.survey_figure(),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn geometries(dir: &std::ffi::OsStr, drawing: &str) -> Vec<(SurveyFigure, acadrust::classes::civil::SurveyFigureGeometry)> {
    let path = Path::new(dir).join(drawing);
    let doc = DwgReader::from_file(&path).unwrap().read().unwrap();
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_SVFIGURE" => {
                    a.survey_figure().zip(a.survey_figure_geometry())
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Whether two solved points are the same point, within `1e-9` of the
/// coordinate magnitude — the shared solved-point tolerance
/// (`alignment::same_solved_point`, user ruling 2026-11-25).
fn same_solved_point(a: Vector3, b: Vector3) -> bool {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt() <= 1e-9 * (1.0 + a.x.abs().max(a.y.abs()))
}

/// Census gate: every figure of the drawing decodes, with vertices and the
/// elevation range (85 figures, none without geometry).
fn assert_census(all: &[(SurveyFigure, acadrust::classes::civil::SurveyFigureGeometry)]) {
    assert_eq!(all.len(), 85);
    assert!(all.iter().all(|(_, g)| g.vertices.len() >= 2
        && g.max_elevation >= g.min_elevation));
}

/// The traverse cache of the LOT figures the live application read
/// (`BaseCurve` PIs, Survey-3.dwg, 2026-11-24). z exact; x/y within the
/// shared solved-point tolerance.
fn lot_oracles() -> Vec<(&'static str, usize, Vec<Vector3>)> {
    let v = |(x, y, z): (f64, f64, f64)| Vector3::new(x, y, z);
    vec![
        ("LOT1", 6, vec![
            v((5490.00218411535, 5091.003428266384, 257.38900127007406)),
            v((5377.000455851201, 5091.001570122782, 258.82401471588656)),
            v((5377.00016316073, 5191.0018932973035, 250.84800252883036)),
            v((5527.001876537222, 5191.0031041018665, 253.8480146189126)),
            v((5527.000877730083, 5128.000432210043, 255.144004393946)),
        ]),
        ("LOT2", 6, vec![
            v((5577.002274211962, 5128.001360047143, 255.14399376057386)),
            v((5577.001495737582, 5191.003089806996, 253.848016884816)),
            v((5727.0024043424055, 5191.00150414044, 249.3479723221435)),
            v((5727.00277562812, 5091.001209222246, 254.14498080149892)),
            v((5614.001193954609, 5091.001569766551, 255.5799996093144)),
        ]),
        ("LOT3", 4, vec![
            v((5527.001920632552, 5291.005948364269, 251.76497247364622)),
            v((5527.001876537222, 5191.0031041018665, 253.8480146189126)),
            v((5377.00016316073, 5191.0018932973035, 250.84800252883036)),
            v((5376.998685977422, 5290.99992560735, 247.2650398122157)),
        ]),
        ("LOT4", 4, vec![
            v((5726.996094007045, 5290.999814832117, 247.26503601862123)),
            v((5727.0024043424055, 5191.00150414044, 249.3479723221435)),
            v((5577.001495737582, 5191.003089806996, 253.848016884816)),
            v((5577.001826027874, 5291.00531787239, 251.76497636501475)),
        ]),
    ]
}

#[test]
fn survey_2b_figures_census_lot_vertices_and_elevations() {
    // The application's tutorial drawing, from `ACADRUST_CIVIL_TUTORIALS_DIR`.
    let Some(dir) = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR") else { return };
    let path = Path::new(&dir).join("Survey-2B.dwg");
    if !path.exists() {
        return;
    }
    let all = geometries(&dir, "Survey-2B.dwg");
    assert_census(&all);
    let names = figures(&dir, "Survey-2B.dwg");
    assert_eq!(names.iter().find(|f| f.name == "LOT1").unwrap().site, "Survey Site");
    assert_oracles(&all);
}

#[test]
fn survey_3_figures_census_lot_vertices_and_elevations() {
    // The application's tutorial drawing, from `ACADRUST_CIVIL_TUTORIALS_DIR`.
    let Some(dir) = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR") else { return };
    let path = Path::new(&dir).join("Survey-3.dwg");
    if !path.exists() {
        return;
    }
    let all = geometries(&dir, "Survey-3.dwg");
    assert_census(&all);
    // the live application's `MinElevation` / `MaxElevation`, verbatim
    // (Survey-3.dwg, 2026-11-24; the drawing holds two figures named EP1)
    let ep1: Vec<_> = all.iter().filter(|(f, _)| f.name == "EP1").collect();
    assert_eq!(ep1.len(), 2);
    assert!(ep1.iter().any(|(_, g)| g.min_elevation == 249.22999136342133
        && g.max_elevation == 255.4399951035045));
    assert!(ep1.iter().any(|(_, g)| g.min_elevation == 255.02000236074278
        && g.max_elevation == 263.89100227799565));
    assert_oracles(&all);
}

fn assert_oracles(all: &[(SurveyFigure, acadrust::classes::civil::SurveyFigureGeometry)]) {
    for (name, count, pis) in lot_oracles() {
        let (_, g) = all.iter().find(|(f, _)| f.name == name).unwrap();
        // the closure entry (a closed figure with curves repeats its first
        // vertex), then the PIs, each within the shared solved-point
        // tolerance of the live application's, z bit-exact
        assert_eq!(g.vertices.len(), count, "{name} vertex count");
        let closure = if count > pis.len() { 1 } else { 0 };
        for (i, pi) in pis.iter().enumerate() {
            let stored = g.vertices[i];
            assert!(same_solved_point(stored, *pi), "{name} v[{i}] within the shared tolerance");
            assert_eq!(stored.z, pi.z, "{name} v[{i}] z exact");
        }
        if closure == 1 {
            let last = g.vertices.last().unwrap();
            assert!(same_solved_point(*last, g.vertices[0]) && last.z == g.vertices[0].z,
                "{name} closure repeats v0");
        }
    }
    // LOT3, all-straight closed: the lengths recompute within the shared
    // solved-point tolerance of the live Length2D / Length3D (the record's
    // x/y cache is ~250 ulps off; z exact)
    let (_, g) = all.iter().find(|(f, _)| f.name == "LOT3").unwrap();
    let pis = &g.vertices;
    let side = |a: Vector3, b: Vector3| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
    let side3 = |a: Vector3, b: Vector3| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
    let n = pis.len();
    let l2: f64 = (0..n).map(|i| side(pis[i], pis[(i + 1) % n])).sum();
    let l3: f64 = (0..n).map(|i| side3(pis[i], pis[(i + 1) % n])).sum();
    let tol = |m: f64| 1e-9 * (1.0 + m);
    assert!((l2 - 500.00582474078817).abs() <= tol(500.0), "LOT3 Length2D within tolerance: {l2}");
    assert!((l3 - 500.1891641228053).abs() <= tol(500.0), "LOT3 Length3D within tolerance: {l3}");
}
