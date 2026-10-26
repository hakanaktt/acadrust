//! Elliptic hatch edge ends are circular angles in DXF and ellipse parameters
//! in the model.
//!
//! Issue #178: DXF groups 50/51 of an elliptic edge hold the angle of the end
//! point from the major axis, in degrees. The model, like DWG, holds the
//! ellipse parameter. With an axis ratio other than 1 the two differ, and an
//! edge read as if they were the same misses its neighbours.

use std::f64::consts::TAU;
use std::io::Cursor;

use acadrust::entities::{BoundaryEdge, EllipticArcEdge, EntityType};
use acadrust::types::Vector2;
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn read_dxf(text: &str) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("open dxf")
        .read()
        .expect("read dxf")
}

fn with_entities(entities: &str) -> CadDocument {
    let text = String::from_utf8(DxfWriter::new(&CadDocument::new()).write_to_vec().unwrap())
        .unwrap()
        .replace("\r\n", "\n");
    let at = text.find("ENTITIES\n").expect("entities section") + "ENTITIES\n".len();
    read_dxf(&format!("{}{entities}{}", &text[..at], &text[at..]))
}

fn hatch_edges(document: &CadDocument) -> Vec<BoundaryEdge> {
    let mut edges = Vec::new();
    for entity in document.entities() {
        if let EntityType::Hatch(hatch) = entity {
            for path in &hatch.paths {
                edges.extend(path.edges.iter().cloned());
            }
        }
    }
    edges
}

/// A HATCH whose single boundary path has the given edge records.
fn hatch_with_edges(edges: &[&str]) -> String {
    format!(
        "  0\nHATCH\n  5\nFFFF0\n100\nAcDbEntity\n  8\n0\n100\nAcDbHatch\n\
 10\n0.0\n 20\n0.0\n 30\n0.0\n210\n0.0\n220\n0.0\n230\n1.0\n  2\nSOLID\n\
 70\n1\n 71\n0\n 91\n1\n 92\n1\n 93\n{}\n{}\
 97\n0\n 75\n0\n 76\n1\n 98\n0\n",
        edges.len(),
        edges.concat()
    )
}

fn elliptic_edge(ratio: f64, start: f64, end: f64, ccw: bool) -> String {
    format!(
        " 72\n3\n 10\n0.0\n 20\n0.0\n 11\n10.0\n 21\n0.0\n 40\n{ratio}\n\
 50\n{start}\n 51\n{end}\n 73\n{}\n",
        i32::from(ccw)
    )
}

fn line_edge(from: (f64, f64), to: (f64, f64)) -> String {
    format!(
        " 72\n1\n 10\n{}\n 20\n{}\n 11\n{}\n 21\n{}\n",
        from.0, from.1, to.0, to.1
    )
}

/// Point of an elliptic edge at parameter `t`, as the model defines it.
fn point_at(edge: &EllipticArcEdge, t: f64) -> Vector2 {
    let (mx, my) = (edge.major_axis_endpoint.x, edge.major_axis_endpoint.y);
    let (nx, ny) = (-my * edge.minor_axis_ratio, mx * edge.minor_axis_ratio);
    Vector2::new(
        edge.center.x + mx * t.cos() + nx * t.sin(),
        edge.center.y + my * t.cos() + ny * t.sin(),
    )
}

fn elliptic(edges: &[BoundaryEdge]) -> &EllipticArcEdge {
    edges
        .iter()
        .find_map(|edge| match edge {
            BoundaryEdge::EllipticArc(edge) => Some(edge),
            _ => None,
        })
        .expect("an elliptic edge")
}

/// Groups 50/51 of each elliptic edge in written DXF text.
fn written_ends(text: &str) -> Vec<(f64, f64)> {
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    let mut ends = Vec::new();
    let mut elliptic = false;
    let mut start = None;
    for pair in lines.chunks(2) {
        let [code, value] = pair else { break };
        match (*code, elliptic) {
            ("72", _) => elliptic = *value == "3",
            ("50", true) => start = value.parse::<f64>().ok(),
            ("51", true) => {
                ends.push((start.take().unwrap(), value.parse::<f64>().unwrap()));
                elliptic = false;
            }
            _ => {}
        }
    }
    ends
}

#[test]
fn elliptic_edge_closes_a_pie_slice_boundary() {
    // Line (0,0)ÔÇô(10,0), elliptic arc 0┬░ÔÇô45┬░ (ratio 0.5), line back to the
    // center from where the 45┬░ ray meets the ellipse.
    let corner = 10.0 / 5.0_f64.sqrt();
    let document = with_entities(&hatch_with_edges(&[
        &line_edge((0.0, 0.0), (10.0, 0.0)),
        &elliptic_edge(0.5, 0.0, 45.0, true),
        &line_edge((corner, corner), (0.0, 0.0)),
    ]));
    let edges = hatch_edges(&document);
    let edge = elliptic(&edges);
    let expected_param = 2.0_f64.atan();
    assert!(
        (edge.end_angle - expected_param).abs() < 1e-12,
        "end parameter {} instead of {expected_param}",
        edge.end_angle
    );
    let start = point_at(edge, edge.start_angle);
    let end = point_at(edge, edge.end_angle);
    assert!((start.x - 10.0).abs() < 1e-9 && start.y.abs() < 1e-9);
    assert!(
        (end.x - corner).abs() < 1e-9 && (end.y - corner).abs() < 1e-9,
        "arc ends at ({}, {}), the next edge starts at ({corner}, {corner})",
        end.x,
        end.y
    );
}

#[test]
fn elliptic_edge_angles_round_trip_through_the_model() {
    // An AutoCAD-style edge whose end lies one turn past its start, plus a
    // clockwise edge (angles in the mirrored frame).
    for (ratio, start, end, ccw) in [
        (0.8439, 115.7092, 424.2908, true),
        (0.5, 30.0, 300.0, false),
        (0.25, 0.0, 360.0, true),
    ] {
        let document = with_entities(&hatch_with_edges(&[&elliptic_edge(ratio, start, end, ccw)]));
        let text = String::from_utf8(DxfWriter::new(&document).write_to_vec().unwrap()).unwrap();
        let ends = written_ends(&text);
        assert_eq!(ends.len(), 1, "written elliptic edges: {ends:?}");
        let (written_start, written_end) = ends[0];
        assert!(
            (written_start - start).abs() < 1e-9 && (written_end - end).abs() < 1e-9,
            "ratio {ratio}: wrote {written_start}/{written_end}, read {start}/{end}"
        );
    }
}

#[test]
fn full_ellipse_end_keeps_its_turn() {
    let document = with_entities(&hatch_with_edges(&[&elliptic_edge(0.5, 0.0, 360.0, true)]));
    let edges = hatch_edges(&document);
    let edge = elliptic(&edges);
    assert!(edge.start_angle.abs() < 1e-12);
    assert!(
        (edge.end_angle - TAU).abs() < 1e-12,
        "end {}",
        edge.end_angle
    );
}
