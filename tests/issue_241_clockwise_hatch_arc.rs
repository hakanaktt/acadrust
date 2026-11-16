//! Issue #241: Clockwise HATCH arc edges: since 0.6.8 DXF converts the stored angles and DWG does not.
//!
//! A clockwise HATCH arc edge stores its angles mirrored: the clockwise arc from 90° to 270°
//! is stored as 270° and 90° (in radians: 1.5 * PI and 0.5 * PI).
//! DWG and DXF must represent the same edge in the model and roundtrip consistently.
//! Hatch explosion (`flatten` and `explode`) must respect the stored angle convention
//! so that clockwise circular and elliptic edges evaluate on the correct side of the circle/ellipse.

use acadrust::document::CadDocument;
use acadrust::entities::hatch::{
    BoundaryEdge, BoundaryPath, BoundaryPathFlags, CircularArcEdge, EllipticArcEdge, HatchPattern,
    HatchPatternLine, HatchPatternType, LineEdge,
};
use acadrust::entities::{EntityType, Hatch};
use acadrust::io::dwg::{DwgReader, DwgWriter};
use acadrust::io::dxf::{DxfReader, DxfWriter};
use acadrust::types::{DxfVersion, Vector2};
use std::f64::consts::{FRAC_PI_2, PI};
use std::io::Cursor;

fn create_clockwise_arc_hatch_doc() -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    let mut hatch = Hatch::solid();
    let mut flags = BoundaryPathFlags::new();
    flags.set_external(true);
    let mut path = BoundaryPath::with_flags(flags);
    // Clockwise arc from 90° to 270°: stored as 270° (1.5 * PI) and 90° (0.5 * PI), ccw = false.
    path.edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(0.0, 0.0),
        radius: 5.0,
        start_angle: 1.5 * PI,
        end_angle: FRAC_PI_2,
        counter_clockwise: false,
    }));
    hatch.add_path(path);
    doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();
    doc
}

fn stored_dxf_angles(bytes: &[u8]) -> (f64, f64) {
    let text = String::from_utf8_lossy(bytes).replace('\r', "");
    let at = text.find("\nHATCH\n").expect("a HATCH record");
    let rest = &text[at..];
    let value = |code: &str| -> f64 {
        let key = format!("\n{code:>3}\n");
        let i = rest.find(&key).unwrap_or_else(|| panic!("no group {code}")) + key.len();
        rest[i..].lines().next().unwrap().trim().parse().unwrap()
    };
    (value("50"), value("51"))
}

#[test]
fn test_dwg_and_dxf_give_the_same_circular_arc_edge() {
    let doc = create_clockwise_arc_hatch_doc();

    // Write to DWG and DXF
    let dwg_bytes = DwgWriter::write_to_vec(&doc).expect("DWG write");
    let dxf_bytes = DxfWriter::new(&doc).write_to_vec().expect("DXF write");

    // DXF file should hold the stored angles directly: 50 = 270.0, 51 = 90.0
    let (s_dxf, e_dxf) = stored_dxf_angles(&dxf_bytes);
    assert!(
        (s_dxf - 270.0).abs() < 1e-9 && (e_dxf - 90.0).abs() < 1e-9,
        "DXF should store angles 270 and 90, got {s_dxf} {e_dxf}"
    );

    // Read back DWG
    let dwg_doc = DwgReader::from_stream(Cursor::new(dwg_bytes))
        .read()
        .expect("DWG read");
    let dwg_edge = match &dwg_doc.entities().next().unwrap() {
        EntityType::Hatch(h) => match &h.paths[0].edges[0] {
            BoundaryEdge::CircularArc(a) => a.clone(),
            _ => panic!("expected CircularArc"),
        },
        _ => panic!("expected Hatch"),
    };

    // Read back DXF
    let dxf_doc = DxfReader::from_reader(Cursor::new(dxf_bytes))
        .unwrap()
        .read()
        .unwrap();
    let dxf_edge = match &dxf_doc.entities().next().unwrap() {
        EntityType::Hatch(h) => match &h.paths[0].edges[0] {
            BoundaryEdge::CircularArc(a) => a.clone(),
            _ => panic!("expected CircularArc"),
        },
        _ => panic!("expected Hatch"),
    };

    // Both should match the stored representation: start 270° (1.5 * PI), end 90° (FRAC_PI_2)
    assert!(
        (dwg_edge.start_angle - 1.5 * PI).abs() < 1e-6,
        "DWG start angle: {}",
        dwg_edge.start_angle
    );
    assert!(
        (dwg_edge.end_angle - FRAC_PI_2).abs() < 1e-6,
        "DWG end angle: {}",
        dwg_edge.end_angle
    );
    assert!(!dwg_edge.counter_clockwise);

    assert!(
        (dxf_edge.start_angle - 1.5 * PI).abs() < 1e-6,
        "DXF start angle should match DWG (stored angles), got {}",
        dxf_edge.start_angle
    );
    assert!(
        (dxf_edge.end_angle - FRAC_PI_2).abs() < 1e-6,
        "DXF end angle should match DWG (stored angles), got {}",
        dxf_edge.end_angle
    );
    assert!(!dxf_edge.counter_clockwise);

    // Saving DWG as DXF should keep the same stored angles in the DXF file
    let dwg_as_dxf_bytes = DxfWriter::new(&dwg_doc)
        .write_to_vec()
        .expect("DWG as DXF write");
    let (s_dwg_dxf, e_dwg_dxf) = stored_dxf_angles(&dwg_as_dxf_bytes);
    assert!(
        (s_dwg_dxf - 270.0).abs() < 1e-9 && (e_dwg_dxf - 90.0).abs() < 1e-9,
        "DWG saved as DXF should write 50=270, 51=90, got {s_dwg_dxf} {e_dwg_dxf}"
    );

    // And reading a DXF produced by external CAD (50=270, 51=90, 73=0) must give 1.5*PI and 0.5*PI
    let external_dxf = format!(
        "0\nSECTION\n2\nENTITIES\n0\nHATCH\n2\nSOLID\n70\n1\n71\n0\n91\n1\n92\n1\n93\n1\n72\n2\n10\n0.0\n20\n0.0\n40\n5.0\n50\n270.0\n51\n90.0\n73\n0\n97\n0\n0\nENDSEC\n0\nEOF\n"
    );
    let ext_doc = DxfReader::from_reader(Cursor::new(external_dxf.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let ext_edge = match &ext_doc.entities().next().unwrap() {
        EntityType::Hatch(h) => match &h.paths[0].edges[0] {
            BoundaryEdge::CircularArc(a) => a.clone(),
            _ => panic!("expected CircularArc"),
        },
        _ => panic!("expected Hatch"),
    };
    assert!(
        (ext_edge.start_angle - 1.5 * PI).abs() < 1e-6,
        "External DXF with 50=270 must read as start 270°, got {}",
        ext_edge.start_angle
    );
    assert!(
        (ext_edge.end_angle - FRAC_PI_2).abs() < 1e-6,
        "External DXF with 51=90 must read as end 90°, got {}",
        ext_edge.end_angle
    );
}

#[test]
fn test_flatten_clockwise_circular_arc_edge_reverses_endpoints() {
    // A Hatch with a clockwise circular arc edge from 90° to 270° (stored angles 270° and 90°).
    // The physical arc runs clockwise along the right half of the circle (passing through 0°).
    let mut hatch = Hatch::solid();
    let mut path = BoundaryPath::new();
    path.edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(0.0, 0.0),
        radius: 5.0,
        start_angle: 1.5 * PI, // 270°
        end_angle: FRAC_PI_2,  // 90°
        counter_clockwise: false,
    }));
    hatch.add_path(path);

    let parts = EntityType::Hatch(Box::new(hatch)).flatten();
    assert_eq!(parts.len(), 1);
    let EntityType::Arc(arc) = &parts[0] else {
        panic!("expected an Arc entity");
    };

    // The counter-clockwise Arc entity representing the right half must start at 270° and end at 90°
    assert!(
        (arc.start_angle - 1.5 * PI).abs() < 1e-6,
        "Flattened Arc start_angle should be 270° (1.5 * PI), got {}",
        arc.start_angle
    );
    assert!(
        (arc.end_angle - FRAC_PI_2).abs() < 1e-6,
        "Flattened Arc end_angle should be 90° (0.5 * PI), got {}",
        arc.end_angle
    );
}

#[test]
fn test_explode_hatch_with_clockwise_circular_arc_fills_correct_side() {
    // Boundary of right semicircle:
    // 1. Line from (0, -5) to (0, 5)
    // 2. Clockwise circular arc from (0, 5) to (0, -5) (center (0, 0), radius 5, stored start 270°, end 90°, ccw=false).
    let mut hatch = Hatch::with_pattern(HatchPattern::new("LINES"));
    hatch.pattern_type = HatchPatternType::Custom;
    hatch.pattern.lines.push(HatchPatternLine {
        angle: 0.0, // horizontal lines
        base_point: Vector2::ZERO,
        offset: Vector2::new(0.0, 1.0),
        dash_lengths: vec![],
    });

    let mut path = BoundaryPath::new();
    path.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(0.0, -5.0),
        end: Vector2::new(0.0, 5.0),
    }));
    path.edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(0.0, 0.0),
        radius: 5.0,
        start_angle: 1.5 * PI, // stored 270°
        end_angle: FRAC_PI_2,  // stored 90°
        counter_clockwise: false,
    }));
    hatch.add_path(path);

    let parts = EntityType::Hatch(Box::new(hatch)).explode();
    assert!(!parts.is_empty(), "Explode should produce pattern lines");

    // All exploded line segments must be on the right side (x >= -1e-6)
    for p in &parts {
        if let EntityType::Line(line) = p {
            assert!(
                line.start.x >= -1e-6 && line.end.x >= -1e-6,
                "Pattern line was placed on the wrong side: ({}, {}) to ({}, {})",
                line.start.x,
                line.start.y,
                line.end.x,
                line.end.y
            );
        }
    }

    // Specifically, the line at y = 0 must span from x ~ 0 to x ~ 5
    let center_line = parts
        .iter()
        .find_map(|p| {
            if let EntityType::Line(l) = p {
                if l.start.y.abs() < 1e-6 && l.end.y.abs() < 1e-6 {
                    return Some(l);
                }
            }
            None
        })
        .expect("expected pattern line at y = 0");

    let min_x = center_line.start.x.min(center_line.end.x);
    let max_x = center_line.start.x.max(center_line.end.x);
    assert!(min_x.abs() < 0.1, "min_x should be near 0, got {}", min_x);
    assert!(
        (max_x - 5.0).abs() < 0.1,
        "max_x should be near 5.0, got {}",
        max_x
    );
}

#[test]
fn test_explode_hatch_with_clockwise_elliptic_arc_fills_correct_side() {
    // Boundary of right semi-ellipse:
    // 1. Line from (0, -5) to (0, 5)
    // 2. Clockwise elliptic arc from (0, 5) to (0, -5): center (0, 0), major (10, 0), ratio 0.5.
    // Stored parameters: start 270° (1.5 * PI), end 90° (FRAC_PI_2), ccw=false.
    let mut hatch = Hatch::with_pattern(HatchPattern::new("LINES"));
    hatch.pattern_type = HatchPatternType::Custom;
    hatch.pattern.lines.push(HatchPatternLine {
        angle: 0.0, // horizontal lines
        base_point: Vector2::ZERO,
        offset: Vector2::new(0.0, 1.0),
        dash_lengths: vec![],
    });

    let mut path = BoundaryPath::new();
    path.edges.push(BoundaryEdge::Line(LineEdge {
        start: Vector2::new(0.0, -5.0),
        end: Vector2::new(0.0, 5.0),
    }));
    path.edges.push(BoundaryEdge::EllipticArc(EllipticArcEdge {
        center: Vector2::new(0.0, 0.0),
        major_axis_endpoint: Vector2::new(10.0, 0.0),
        minor_axis_ratio: 0.5,
        start_angle: 1.5 * PI, // stored 270°
        end_angle: FRAC_PI_2,  // stored 90°
        counter_clockwise: false,
    }));
    hatch.add_path(path);

    let parts = EntityType::Hatch(Box::new(hatch)).explode();
    assert!(!parts.is_empty(), "Explode should produce pattern lines");

    // All exploded line segments must be on the right side (x >= -1e-6)
    for p in &parts {
        if let EntityType::Line(line) = p {
            assert!(
                line.start.x >= -1e-6 && line.end.x >= -1e-6,
                "Pattern line was placed on the wrong side: ({}, {}) to ({}, {})",
                line.start.x,
                line.start.y,
                line.end.x,
                line.end.y
            );
        }
    }

    // The line at y = 0 must span from x ~ 0 to x ~ 10
    let center_line = parts
        .iter()
        .find_map(|p| {
            if let EntityType::Line(l) = p {
                if l.start.y.abs() < 1e-6 && l.end.y.abs() < 1e-6 {
                    return Some(l);
                }
            }
            None
        })
        .expect("expected pattern line at y = 0");

    let min_x = center_line.start.x.min(center_line.end.x);
    let max_x = center_line.start.x.max(center_line.end.x);
    assert!(min_x.abs() < 0.1, "min_x should be near 0, got {}", min_x);
    assert!(
        (max_x - 10.0).abs() < 0.1,
        "max_x should be near 10.0, got {}",
        max_x
    );
}
