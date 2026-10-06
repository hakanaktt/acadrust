use std::io::Cursor;

use acadrust::entities::{BoundaryEdge, BoundaryPath, Hatch};
use acadrust::{CadDocument, Color, DwgReadOptions, DwgReader, DwgWriter, EntityType};

// Six MPOLYGONs generated as DXF and re-saved as a 2013 DWG by AutoCAD; see
// mpolygon-fixture.md for the shapes and their expected values.
fn mpolygons_of(doc: &CadDocument) -> Vec<Hatch> {
    doc.model_space_entities()
        .filter_map(|entity| match entity {
            EntityType::Hatch(hatch) if hatch.is_mpolygon => Some(hatch.clone()),
            _ => None,
        })
        .collect()
}

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream_with_options(Cursor::new(bytes), DwgReadOptions::failsafe())
        .read()
        .unwrap()
}

fn mpolygons() -> Vec<Hatch> {
    mpolygons_of(&read(include_bytes!("mpolygon-fixture.dwg").to_vec()))
}

fn vertices(path: &BoundaryPath) -> Vec<(f64, f64, f64)> {
    assert_eq!(path.edges.len(), 1, "an MPOLYGON loop is a single polyline");
    let BoundaryEdge::Polyline(polyline) = &path.edges[0] else {
        panic!("MPOLYGON loop is not a polyline");
    };
    assert!(polyline.is_closed);
    polyline.vertices.iter().map(|v| (v.x, v.y, v.z)).collect()
}

fn square(mpolygons: &[Hatch]) -> &Hatch {
    mpolygons
        .iter()
        .find(|h| {
            h.paths.len() == 1
                && vertices(&h.paths[0])
                    .iter()
                    .any(|v| v.0 == 10.0 && v.1 == 10.0)
        })
        .expect("shape 1 missing")
}

#[test]
fn fixture_contains_the_six_mpolygons() {
    assert_eq!(mpolygons().len(), 6);
}

#[test]
fn solid_square_reads_its_loop_and_both_colors() {
    let mpolygons = mpolygons();
    let hatch = square(&mpolygons);
    let mut loop_vertices = vertices(&hatch.paths[0]);
    loop_vertices.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        loop_vertices,
        vec![
            (0.0, 0.0, 0.0),
            (0.0, 10.0, 0.0),
            (10.0, 0.0, 0.0),
            (10.0, 10.0, 0.0)
        ]
    );
    assert_eq!(hatch.common.color, Color::Index(1));
    assert_eq!(hatch.mpolygon_hatch_color, Color::Index(5));
    assert!(!hatch.is_associative);
    assert!(hatch.mpolygon_invalid_loops.is_empty());
}

#[test]
fn square_with_hole_reads_two_loops() {
    let mpolygons = mpolygons();
    let hatch = mpolygons
        .iter()
        .find(|h| h.paths.len() == 2)
        .expect("shape 2 missing");
    let mut loops: Vec<Vec<(f64, f64, f64)>> = hatch
        .paths
        .iter()
        .map(|p| {
            let mut v = vertices(p);
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v
        })
        .collect();
    loops.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(
        loops,
        vec![
            // Outer square (20,0)-(30,10).
            vec![
                (20.0, 0.0, 0.0),
                (20.0, 10.0, 0.0),
                (30.0, 0.0, 0.0),
                (30.0, 10.0, 0.0)
            ],
            // Hole (22,2)-(26,6).
            vec![
                (22.0, 2.0, 0.0),
                (22.0, 6.0, 0.0),
                (26.0, 2.0, 0.0),
                (26.0, 6.0, 0.0)
            ],
        ]
    );
}

#[test]
fn arc_shape_carries_a_non_zero_bulge() {
    let mpolygons = mpolygons();
    let hatch = mpolygons
        .iter()
        .find(|h| {
            h.paths
                .iter()
                .any(|p| vertices(p).iter().any(|v| v.0 == 50.0 && v.1 == 0.0))
        })
        .expect("shape 3 missing");
    assert_eq!(hatch.paths.len(), 1);
    let loop_vertices = vertices(&hatch.paths[0]);
    assert_eq!(loop_vertices.len(), 4);
    for (x, y, bulge) in loop_vertices {
        let expected = if (x, y) == (50.0, 0.0) { -1.0 } else { 0.0 };
        assert_eq!(bulge, expected, "bulge at ({x}, {y})");
    }
}

#[test]
fn pattern_shape_is_not_solid_and_has_pattern_lines() {
    let mpolygons = mpolygons();
    let hatch = mpolygons
        .iter()
        .find(|h| !h.is_solid)
        .expect("shape 4 missing");
    assert!(!hatch.pattern.lines.is_empty());
    assert!(!hatch.paths.is_empty());
}

#[test]
fn true_color_fill_and_bylayer_entity() {
    let mpolygons = mpolygons();
    let hatch = mpolygons
        .iter()
        .find(|h| h.common.color == Color::ByLayer)
        .expect("shape 5 missing");
    // The true-colour fill is stored as a one-colour gradient (two identical
    // stops); the fill colour itself stays ByLayer.
    let rgb = Color::from_rgb(200, 100, 50);
    assert!(hatch.gradient_color.enabled);
    assert_eq!(hatch.gradient_color.colors.len(), 2);
    assert!(hatch.gradient_color.colors.iter().all(|c| c.color == rgb));
    assert_eq!(hatch.common.linetype, "DASHED");
}

#[test]
fn bow_tie_stays_a_valid_loop() {
    let mpolygons = mpolygons();
    let hatch = mpolygons
        .iter()
        .find(|h| {
            h.paths
                .iter()
                .any(|p| vertices(p).iter().any(|v| v.0 == 110.0 && v.1 == 10.0))
        })
        .expect("shape 6 missing");
    assert_eq!(hatch.paths.len(), 1);
    assert_eq!(vertices(&hatch.paths[0]).len(), 4);
    assert!(hatch.mpolygon_invalid_loops.is_empty());
}

fn write_and_read(mpolygons: &[Hatch]) -> Vec<Hatch> {
    let mut doc = CadDocument::new();
    for hatch in mpolygons {
        doc.add_entity(EntityType::Hatch(hatch.clone())).unwrap();
    }
    mpolygons_of(&read(DwgWriter::write_to_vec(&doc).unwrap()))
}

#[test]
fn write_read_roundtrip_keeps_loops_and_colors() {
    let before = mpolygons();
    let after = write_and_read(&before);
    assert_eq!(after.len(), before.len());
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.paths, b.paths);
        assert_eq!(a.mpolygon_hatch_color, b.mpolygon_hatch_color);
        assert_eq!(a.mpolygon_invalid_loops, b.mpolygon_invalid_loops);
    }
}

#[test]
fn write_read_roundtrip_keeps_invalid_loops() {
    let mut hatch = square(&mpolygons()).clone();
    hatch.mpolygon_invalid_loops = vec![hatch.paths[0].clone()];
    let after = write_and_read(&[hatch.clone()]);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].paths, hatch.paths);
    assert_eq!(
        after[0].mpolygon_invalid_loops,
        hatch.mpolygon_invalid_loops
    );
}
