//! Family 5 — TIN surfaces: the write-audit pilot formalized and extended
//! to the definitions, operation by operation, and to the snapshot
//! semantics (civil-write.md §3c). READY: bit-exact assertions on the
//! triangulation and field-exact ones on the definitions.
//!
//! A definition's GUID is regenerated on every write (the record stores a
//! fresh identifier), so the tests compare operations, texts and sources
//! but not `TinDefinition::id`; they assert only that it reads back as 16
//! bytes.

#[path = "rt_data/mod.rs"]
mod rt_data;

use acadrust::classes::civil::{add_tin_surface, add_tin_surface_with, TinBoundaryType, TinBreaklineType, TinDefinition, TinLines, TinOperation};
use acadrust::types::Vector3;

fn pts3() -> Vec<Vector3> {
    vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 1.0), Vector3::new(0.0, 10.0, 2.0)]
}

#[test]
fn the_pilot_surface_reads_back_bit_exact() {
    let mut doc = rt_data::rt_data_drawing();
    let pts = vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(100.0, 0.0, 10.0),
        Vector3::new(100.0, 100.0, 20.0),
        Vector3::new(0.0, 100.0, 5.0),
        Vector3::new(37.5, 62.5, 12.25),
    ];
    add_tin_surface(&mut doc, "S", &pts, None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let data = rt_data::rt_data_entities(&back, "AECC_TIN_SURFACE").into_iter().next().unwrap().1;
    let tin = data.tin_surface().expect("the triangulation reads back");
    // The five points plus the four corner vertices.
    assert_eq!(tin.vertices.len(), 9);
    for p in &pts {
        assert!(tin.vertices.iter().any(|v| v.position == *p), "point {p:?} bit-exact among the vertices");
    }
    // Every data point is a real vertex (flag 0x80), corners 0x8440.
    let data_flags: Vec<i32> = tin.vertices.iter().filter(|v| v.flags & 0x80 != 0).map(|v| v.flags).collect();
    assert_eq!(data_flags.len(), 5);
    assert_eq!(tin.corners.len(), 4);
    // The pilot's visible-triangle set, by vertex index: the triangles
    // that touch a corner are hidden.
    let visible: Vec<[u32; 3]> = tin.triangles.iter().filter(|t| t.is_visible()).map(|t| t.vertices).collect();
    assert_eq!(visible, vec![[4, 5, 8], [5, 6, 8], [6, 7, 8], [7, 4, 8]]);

    // The one definition adds the points; the surface is not marked to
    // follow its sources.
    let (defs, auto_rebuild) = data.tin_definitions().expect("definitions read back");
    assert!(!auto_rebuild);
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].operation, TinOperation::AddPoints(pts));
    assert!(defs[0].texts.is_empty());
    assert!(defs[0].sources.is_empty());
    assert_eq!(defs[0].id.len(), 16);
}

#[test]
fn definitions_survive_operation_by_operation() {
    let mut doc = rt_data::rt_data_drawing();
    // A 5x5 grid of points, a breakline across it, an outer boundary
    // around everything, an added vertex, an elevation edit, a raise.
    let grid: Vec<Vector3> = (0..5)
        .flat_map(|i| (0..5).map(move |j| Vector3::new(i as f64 * 25.0, j as f64 * 25.0, 10.0 + i as f64 + j as f64)))
        .collect();
    let lines = |pl: Vec<Vector3>| TinLines { objects: 0, polylines: vec![pl], mid_ordinate: 1.5, maximum_distance: 2.0, weeding_distance: 0.25, weeding_angle: 0.1 };
    let definitions = vec![
        TinDefinition { operation: TinOperation::AddPoints(grid.clone()), id: [0; 16], texts: vec![], sources: vec![] },
        TinDefinition {
            operation: TinOperation::Breaklines(TinBreaklineType::Standard, lines(vec![Vector3::new(0.0, 50.0, 12.0), Vector3::new(100.0, 50.0, 14.0)])),
            id: [0; 16],
            texts: vec!["BL1".into()],
            sources: vec![],
        },
        TinDefinition {
            operation: TinOperation::Boundary {
                boundary_type: TinBoundaryType::Outer,
                non_destructive: false,
                lines: lines(vec![Vector3::new(-10.0, -10.0, 0.0), Vector3::new(110.0, -10.0, 0.0), Vector3::new(110.0, 110.0, 0.0), Vector3::new(-10.0, 110.0, 0.0), Vector3::new(-10.0, -10.0, 0.0)]),
            },
            id: [0; 16],
            texts: vec!["BND".into()],
            sources: vec![],
        },
        TinDefinition { operation: TinOperation::AddVertex(Vector3::new(50.0, 75.0, 33.0)), id: [0; 16], texts: vec![], sources: vec![] },
        TinDefinition { operation: TinOperation::Raise(5.0), id: [0; 16], texts: vec![], sources: vec![] },
        TinDefinition { operation: TinOperation::VertexElevation(Vector3::new(0.0, 0.0, 99.5)), id: [0; 16], texts: vec![], sources: vec![] },
    ];
    add_tin_surface_with(&mut doc, "S", &definitions, true, None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let data = rt_data::rt_data_entities(&back, "AECC_TIN_SURFACE").into_iter().next().unwrap().1;
    let (defs, auto_rebuild) = data.tin_definitions().expect("definitions read back");
    assert!(auto_rebuild, "the auto-rebuild flag reads back");
    assert_eq!(defs.len(), 6, "one definition per operation");
    for (got, want) in defs.iter().zip(&definitions) {
        assert_eq!(got.operation, want.operation, "operation in order");
        assert_eq!(got.texts, want.texts, "texts in order");
        assert_eq!(got.sources, want.sources);
        assert_eq!(got.id.len(), 16);
    }
    // The breakline's line parameters survive with the bounds as written.
    match &defs[1].operation {
        TinOperation::Breaklines(kind, l) => {
            assert_eq!(kind, &TinBreaklineType::Standard);
            assert_eq!(l.objects, 0);
            assert_eq!(l.polylines, vec![vec![Vector3::new(0.0, 50.0, 12.0), Vector3::new(100.0, 50.0, 14.0)]]);
            assert_eq!(l.mid_ordinate, 1.5);
            assert_eq!(l.maximum_distance, 2.0);
            assert_eq!(l.weeding_distance, 0.25);
            assert_eq!(l.weeding_angle, 0.1);
        }
        other => panic!("definition 2 is {other:?}"),
    }
    match &defs[2].operation {
        TinOperation::Boundary { boundary_type, non_destructive, lines } => {
            assert_eq!(boundary_type, &TinBoundaryType::Outer);
            assert!(!non_destructive);
            assert_eq!(lines.polylines.len(), 1);
        }
        other => panic!("definition 3 is {other:?}"),
    }

    // The triangulation shows the definitions' effect: the added vertex is
    // there, the edited point's elevation is the edit, and the raise
    // lifted every other vertex by 5.
    let tin = data.tin_surface().expect("the triangulation reads back");
    assert!(tin.vertices.iter().any(|v| v.position == Vector3::new(50.0, 75.0, 38.0)), "the added vertex, raised with the rest");
    let raised = |z: f64| z + 5.0;
    for p in &grid {
        // A point at a vertex's plan position replaces its elevation: the
        // added vertex sits on the grid point (50, 75).
        let want_z = if p.x == 50.0 && p.y == 75.0 {
            38.0
        } else if p.x == 0.0 && p.y == 0.0 {
            99.5
        } else if p.x == 100.0 && p.y == 50.0 {
            // The breakline ends on this grid point; a point at a
            // vertex's plan position replaces its elevation (14, raised).
            19.0
        } else {
            raised(p.z)
        };
        assert!(
            tin.vertices.iter().any(|v| v.position.x == p.x && v.position.y == p.y && v.position.z == want_z),
            "grid point ({}, {}) at elevation {want_z}",
            p.x,
            p.y
        );
    }
    // The outer boundary lies outside every data point: a triangle whose
    // every vertex is inside it stays visible (the corner triangles reach
    // outside and are hidden by it).
    let inside = |p: &Vector3| (-10.0..=110.0).contains(&p.x) && (-10.0..=110.0).contains(&p.y);
    let corner = |i: u32| tin.vertices[i as usize].flags & 0x8440 != 0;
    for t in &tin.triangles {
        let all_in = t.vertices.iter().all(|&v| !corner(v) && inside(&tin.vertices[v as usize].position));
        assert_eq!(t.is_visible(), all_in, "triangle {:?} visibility", t.vertices);
    }
}

#[test]
fn a_hide_boundary_hides_the_triangles_inside_it() {
    let mut doc = rt_data::rt_data_drawing();
    let grid: Vec<Vector3> = (0..5)
        .flat_map(|i| (0..5).map(move |j| Vector3::new(i as f64 * 25.0, j as f64 * 25.0, i as f64 + j as f64)))
        .collect();
    let box_lines = TinLines { objects: 0, polylines: vec![vec![Vector3::new(20.0, 20.0, 0.0), Vector3::new(80.0, 20.0, 0.0), Vector3::new(80.0, 80.0, 0.0), Vector3::new(20.0, 80.0, 0.0), Vector3::new(20.0, 20.0, 0.0)]], mid_ordinate: 0.0, maximum_distance: 0.0, weeding_distance: 0.0, weeding_angle: 0.0 };
    let definitions = vec![
        TinDefinition { operation: TinOperation::AddPoints(grid.clone()), id: [0; 16], texts: vec![], sources: vec![] },
        TinDefinition { operation: TinOperation::Boundary { boundary_type: TinBoundaryType::Hide, non_destructive: false, lines: box_lines }, id: [0; 16], texts: vec!["HIDE".into()], sources: vec![] },
    ];
    add_tin_surface_with(&mut doc, "S", &definitions, false, None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let data = rt_data::rt_data_entities(&back, "AECC_TIN_SURFACE").into_iter().next().unwrap().1;
    let tin = data.tin_surface().unwrap();
    // Documented rules: a triangle that uses a corner is hidden anyway,
    // and a destructive hide boundary hides the triangles whose every
    // vertex lies inside it.
    let inside = |p: &Vector3| (20.0..=80.0).contains(&p.x) && (20.0..=80.0).contains(&p.y);
    let want_hidden: Vec<[u32; 3]> = tin
        .triangles
        .iter()
        .filter(|t| {
            t.vertices.iter().any(|&v| tin.vertices[v as usize].flags & 0x8440 != 0)
                || t.vertices.iter().all(|&v| inside(&tin.vertices[v as usize].position))
        })
        .map(|t| t.vertices)
        .collect();
    assert!(!want_hidden.is_empty(), "the box covers the central triangles");
    for t in &tin.triangles {
        assert_eq!(t.is_visible(), !want_hidden.contains(&t.vertices), "triangle {:?} visibility", t.vertices);
    }
}

#[test]
fn the_snapshot_keeps_the_triangulation_before_it() {
    let mut doc = rt_data::rt_data_drawing();
    let pts = pts3();
    // Definitions after the snapshot raise the surface, not the snapshot.
    let definitions = vec![
        TinDefinition { operation: TinOperation::AddPoints(pts.clone()), id: [0; 16], texts: vec![], sources: vec![] },
        TinDefinition { operation: TinOperation::Kind40(0), id: [0; 16], texts: vec![], sources: vec![] },
        TinDefinition { operation: TinOperation::Raise(5.0), id: [0; 16], texts: vec![], sources: vec![] },
    ];
    add_tin_surface_with(&mut doc, "S", &definitions, false, None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let data = rt_data::rt_data_entities(&back, "AECC_TIN_SURFACE").into_iter().next().unwrap().1;
    let (defs, auto_rebuild) = data.tin_definitions().unwrap();
    assert!(!auto_rebuild);
    assert_eq!(defs.len(), 3);
    assert_eq!(defs[1].operation, TinOperation::Kind40(0));

    // The surface has the raise; the snapshot keeps the elevation the
    // definitions before it built. Positions bit-exact.
    let tin = data.tin_surface().unwrap();
    for p in &pts {
        assert!(tin.vertices.iter().any(|v| v.position.x == p.x && v.position.y == p.y && v.position.z == p.z + 5.0));
    }
    let snapshot = data.tin_snapshot().expect("the snapshot reads back");
    for p in &pts {
        assert!(snapshot.vertices.iter().any(|v| v.position == *p), "snapshot point {p:?} bit-exact, unraised");
    }
    // Same triangles in both.
    let tri = |t: &acadrust::classes::civil::TinSurface| t.triangles.iter().map(|t| t.vertices).collect::<Vec<_>>();
    assert_eq!(tri(&tin), tri(&snapshot));
}
