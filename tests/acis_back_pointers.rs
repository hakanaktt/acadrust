//! ACIS topology back-pointer regression tests.
//!
//! ACIS topology is doubly linked: a coedge references its edge and the edge
//! must reference a coedge back; an edge references its vertices and each
//! vertex must reference an edge back. Builders that only wrote the forward
//! direction produced bodies that BricsCAD rejected outright with
//!
//!   Modeling operation error: Entities in shell are not connected
//!   coedge's edge doesn't point to coedge. Fatal topology error.
//!   edge without backptr
//!   vertex without edge
//!   Validation: Invalid   Default: Removed
//!
//! "Removed" means the solid was discarded, so the file opened but the geometry
//! was gone. `SatDocument::validate()` did not catch this because it checks
//! pointer targets exist, not that reverse links are populated.
//!
//! Verified against BricsCAD V20 `_.RECOVER` with `AUDITCTL 1`; before the fix
//! all five multi-face primitives reported the errors above, after it all report
//! `Errors: 0`. Reproduce with `tests/cad_validate_recover.ps1`.

use acadrust::entities::acis::{primitives, SatDocument, SatToken};

/// Every `edge` record must have a non-null coedge back-pointer (token 5), and
/// every `vertex` record a non-null edge back-pointer (token 1).
fn assert_back_pointers(sat: &SatDocument, what: &str) {
    let mut edges = 0usize;
    let mut verts = 0usize;
    let mut bad_edges = Vec::new();
    let mut bad_verts = Vec::new();

    for (i, rec) in sat.records.iter().enumerate() {
        match rec.entity_type.as_str() {
            "edge" => {
                edges += 1;
                match rec.tokens.get(5) {
                    Some(SatToken::Pointer(p)) if p.0 >= 0 => {}
                    _ => bad_edges.push(i),
                }
            }
            "vertex" => {
                verts += 1;
                match rec.tokens.get(1) {
                    Some(SatToken::Pointer(p)) if p.0 >= 0 => {}
                    _ => bad_verts.push(i),
                }
            }
            _ => {}
        }
    }

    assert!(
        edges > 0,
        "{}: expected edge records, found none (test would vacuously pass)",
        what
    );
    assert!(
        bad_edges.is_empty(),
        "{}: {} of {} edge records have a null coedge back-pointer \
         (ACIS: \"edge without backptr\"), records {:?}",
        what,
        bad_edges.len(),
        edges,
        bad_edges
    );
    assert!(
        bad_verts.is_empty(),
        "{}: {} of {} vertex records have a null edge back-pointer \
         (ACIS: \"vertex without edge\"), records {:?}",
        what,
        bad_verts.len(),
        verts,
        bad_verts
    );
}

/// Each coedge's edge must point back at *a* coedge of that same edge.
fn assert_coedge_edge_consistency(sat: &SatDocument, what: &str) {
    for (i, rec) in sat.records.iter().enumerate() {
        if rec.entity_type != "coedge" {
            continue;
        }
        let edge_idx = match rec.tokens.get(4) {
            Some(SatToken::Pointer(p)) if p.0 >= 0 => p.0,
            _ => panic!("{}: coedge {} has no edge pointer", what, i),
        };
        let edge = sat
            .records
            .get(edge_idx as usize)
            .unwrap_or_else(|| panic!("{}: coedge {} points at missing edge {}", what, i, edge_idx));
        let back = match edge.tokens.get(5) {
            Some(SatToken::Pointer(p)) => p.0,
            _ => -1,
        };
        assert!(
            back >= 0,
            "{}: coedge {} -> edge {} which has no coedge back-pointer \
             (ACIS: \"coedge's edge doesn't point to coedge. Fatal topology error.\")",
            what,
            i,
            edge_idx
        );
        // The back-pointer must be a coedge referencing this same edge.
        let bc = sat.records.get(back as usize).unwrap_or_else(|| {
            panic!("{}: edge {} back-pointer {} is not a record", what, edge_idx, back)
        });
        assert_eq!(
            bc.entity_type, "coedge",
            "{}: edge {} back-pointer {} is a {}, not a coedge",
            what, edge_idx, back, bc.entity_type
        );
        match bc.tokens.get(4) {
            Some(SatToken::Pointer(p)) => assert_eq!(
                p.0, edge_idx,
                "{}: edge {} points at coedge {} whose edge is {}",
                what, edge_idx, back, p.0
            ),
            _ => panic!("{}: coedge {} has no edge pointer", what, back),
        }
    }
}

#[test]
fn box_has_back_pointers() {
    let sat = primitives::build_box([0.0, 0.0, 0.0], 10.0, 10.0, 10.0);
    assert_back_pointers(&sat, "build_box");
    assert_coedge_edge_consistency(&sat, "build_box");
}

#[test]
fn wedge_has_back_pointers() {
    let sat = primitives::build_wedge([0.0, 0.0, 0.0], 10.0, 10.0, 10.0);
    assert_back_pointers(&sat, "build_wedge");
    assert_coedge_edge_consistency(&sat, "build_wedge");
}

#[test]
fn pyramid_has_back_pointers() {
    let sat = primitives::build_pyramid([0.0, 0.0, 0.0], 10.0, 10.0);
    assert_back_pointers(&sat, "build_pyramid");
    assert_coedge_edge_consistency(&sat, "build_pyramid");
}

#[test]
fn cylinder_has_back_pointers() {
    let sat = primitives::build_cylinder([0.0, 0.0, 0.0], 5.0, 10.0);
    assert_back_pointers(&sat, "build_cylinder");
    assert_coedge_edge_consistency(&sat, "build_cylinder");
}

#[test]
fn cone_has_back_pointers() {
    let sat = primitives::build_cone([0.0, 0.0, 0.0], 5.0, 10.0);
    assert_back_pointers(&sat, "build_cone");
    assert_coedge_edge_consistency(&sat, "build_cone");
}

#[test]
fn planar_body_has_back_pointers() {
    // Unit cube as an explicit face soup, wound CCW seen from outside.
    let v = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let f = vec![
        vec![0, 3, 2, 1], // bottom (-Z)
        vec![4, 5, 6, 7], // top (+Z)
        vec![0, 1, 5, 4], // front (-Y)
        vec![2, 3, 7, 6], // back (+Y)
        vec![1, 2, 6, 5], // right (+X)
        vec![3, 0, 4, 7], // left (-X)
    ];
    let sat = primitives::build_planar_body(&v, &f).expect("cube should build");
    assert_back_pointers(&sat, "build_planar_body");
    assert_coedge_edge_consistency(&sat, "build_planar_body");
}

/// Sphere and torus are single-face closed surfaces with no edges or vertices
/// at all, which is why they always passed the BricsCAD audit. Guard that they
/// stay edge-free so the tests above keep measuring what they claim to.
#[test]
fn single_face_primitives_have_no_edges() {
    for (name, sat) in [
        ("sphere", primitives::build_sphere([0.0, 0.0, 0.0], 5.0)),
        ("torus", primitives::build_torus([0.0, 0.0, 0.0], 10.0, 3.0)),
    ] {
        let edges = sat.records.iter().filter(|r| r.entity_type == "edge").count();
        assert_eq!(
            edges, 0,
            "{} unexpectedly has {} edge records; it is meant to be a single \
             closed surface with no topology",
            name, edges
        );
    }
}
