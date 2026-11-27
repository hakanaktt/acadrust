//! The TIN point-list denominator: a surface record keeps its full
//! triangulation database — the drawn mesh plus the hidden scaffold (the
//! four extent corners and every vertex only hidden triangles use) — while
//! the application's live `TinSurface.Vertices`/`Triangles` collections
//! surface the drawn mesh only. The live counts are the drawn subsets
//! exactly, on every tutorial surface with a real triangulation.
//!
//! Recorded 2026-11-27 (execute_civil_code, cap.night.*.census.txt): the
//! application reads 778/8572/35677/2405 vertices and 1388/16868/70602/4593
//! triangles off Grading-1, Align-7A, Labels-Surface and Surface-3, where
//! the records store 782/10158/35815/2410 vertices (the deltas +4/+1586/
//! +138/+5 are exactly the hidden-triangle-only vertices) and 1558/20310/
//! 71624/4814 triangles. The N-2 adjudication: no over-read (the count word
//! bounds the list; every stored vertex has an incident triangle), the
//! record is faithful — a denominator difference, like the F3 rule.

use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::DwgReader;
use std::collections::HashSet;

/// The application's tutorial drawing, from `ACADRUST_CIVIL_TUTORIALS_DIR`.
fn app_drawing(name: &str) -> Option<acadrust::CadDocument> {
    let dir = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR")?;
    let path = std::path::Path::new(&dir).join(name);
    if !path.exists() {
        return None;
    }
    Some(DwgReader::from_file(&path).unwrap().read().unwrap())
}

fn surface(name: &str) -> Option<acadrust::classes::civil::TinSurface> {
    let doc = app_drawing(name)?;
    for e in doc.entities() {
        let EntityType::Extended(x) = e else { continue };
        let ExtendedEntityData::Arch(a) = &x.data else { continue };
        if a.dxf_name == "AECC_TIN_SURFACE" {
            return a.tin_surface();
        }
    }
    None
}

/// The live vertex count: vertices referenced by at least one drawn
/// triangle (the application's `TinSurface.Vertices` denominator).
fn drawn_vertices(
    tin: &acadrust::classes::civil::TinSurface,
) -> HashSet<u32> {
    tin.triangles
        .iter()
        .filter(|t| t.is_visible())
        .flat_map(|t| t.vertices)
        .filter(|v| (*v as usize) < tin.vertices.len())
        .collect()
}

/// The live collections read the drawn mesh exactly: `Grading-1` 778/1388,
/// `Align-7A` 8572/16868, `Labels-Surface` 35677/70602, `Surface-3`
/// 2405/4593 (cap.night.*.census.txt), while the records store
/// 782/10158/35815/2410 vertices and 1558/20310/71624/4814 triangles —
/// every stored vertex has an incident triangle, so the stored lists are no
/// over-read; the extras are the hidden scaffold.
#[test]
fn the_live_collections_count_the_drawn_mesh_of_the_stored_lists() {
    for (drawing, live_vertices, live_triangles) in [
        ("Grading-1.dwg", 778, 1388),
        ("Align-7A.dwg", 8572, 16868),
        ("Labels-Surface.dwg", 35677, 70602),
        ("Surface-3.dwg", 2405, 4593),
    ] {
        let Some(tin) = surface(drawing) else { continue };
        assert!(tin.vertices.len() >= live_vertices + 4, "stored vertices, {drawing}");
        let drawn = drawn_vertices(&tin);
        assert_eq!(drawn.len(), live_vertices, "live vertices, {drawing}");
        assert_eq!(
            tin.triangles.iter().filter(|t| t.is_visible()).count(),
            live_triangles,
            "live triangles, {drawing}"
        );
        // No over-read: every stored vertex keeps an incident triangle, so
        // the stored list is exactly drawn mesh + hidden-only vertices.
        let any: HashSet<u32> = tin
            .triangles
            .iter()
            .flat_map(|t| t.vertices)
            .filter(|v| (*v as usize) < tin.vertices.len())
            .collect();
        assert_eq!(any.len(), tin.vertices.len(), "every vertex referenced, {drawing}");
        // The scaffold: four corner-flag vertices, each used only by hidden
        // triangles, among the hidden-only set.
        let corners: Vec<usize> = tin
            .vertices
            .iter()
            .enumerate()
            .filter(|(_, v)| v.flags & 0x40 != 0)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(corners.len(), 4, "corners, {drawing}");
        assert!(
            corners
                .iter()
                .all(|c| tin.vertices[*c].triangle >= 0
                    && !tin.triangles[tin.vertices[*c].triangle as usize].is_visible()),
            "a corner's incident triangle is hidden, {drawing}"
        );
        assert!(corners.iter().all(|c| !drawn.contains(&(*c as u32))), "corner not drawn, {drawing}");
        let hidden_only = tin.vertices.len() - drawn.len();
        assert!(hidden_only >= 4, "at least the corners are hidden-only, {drawing}");
    }
}

/// The unbuilt shell keeps the four corners at the drawing extremes and no
/// triangles; the live collections read 0/0 — the same denominator, at the
/// empty end of the same rule (Survey-5C.dwg, cap.night.Survey-5C.dwg
/// .census.txt; the #656 shell form).
#[test]
fn the_shells_corners_stay_out_of_the_live_collections() {
    let Some(tin) = surface("Survey-5C.dwg") else { return };
    assert_eq!(tin.vertices.len(), 4);
    assert!(tin.triangles.is_empty());
    assert!(drawn_vertices(&tin).is_empty());
}
