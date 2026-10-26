//! A clockwise arc edge of a HATCH boundary is stored in a DXF with the complementary angles
//! (360 minus each), which is what AutoCAD writes and what ezdxf reads
//! (libviprs/acadsharp-rs-tests#159). The writer put the raw angles, and the reader took them
//! raw, so the pair round-tripped and AutoCAD, ezdxf and anything else saw the other half of
//! the circle.

use acadrust::document::CadDocument;
use acadrust::entities::hatch::{BoundaryEdge, BoundaryPath, BoundaryPathFlags, CircularArcEdge};
use acadrust::entities::{EntityType, Hatch};
use acadrust::io::dxf::{DxfReader, DxfWriter};
use acadrust::types::{DxfVersion, Vector2};
use std::f64::consts::{FRAC_PI_2, PI};
use std::io::Cursor;

fn doc_with(start: f64, end: f64, counter_clockwise: bool) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    let mut hatch = Hatch::solid();
    let mut flags = BoundaryPathFlags::new();
    flags.set_external(true);
    let mut path = BoundaryPath::with_flags(flags);
    path.edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(70.0, 5.0),
        radius: 5.0,
        start_angle: start,
        end_angle: end,
        counter_clockwise,
    }));
    hatch.add_path(path);
    doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();
    doc
}

/// The values of the 50 and 51 groups of the first HATCH record, in degrees.
fn stored_angles(bytes: &[u8]) -> (f64, f64) {
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

fn read_back(bytes: Vec<u8>) -> (f64, f64, bool) {
    let doc = DxfReader::from_reader(Cursor::new(bytes)).unwrap().read().unwrap();
    for e in doc.entities() {
        if let EntityType::Hatch(h) = e {
            if let BoundaryEdge::CircularArc(a) = &h.paths[0].edges[0] {
                return (a.start_angle, a.end_angle, a.counter_clockwise);
            }
        }
    }
    panic!("no hatch arc edge came back");
}

#[test]
fn issue_159_a_clockwise_arc_edge_is_stored_with_complementary_angles() {
    // Clockwise from 90 to 270 degrees: the right half of the circle.
    let bytes = DxfWriter::new(&doc_with(FRAC_PI_2, 3.0 * FRAC_PI_2, false))
        .write_to_vec()
        .unwrap();
    let (s, e) = stored_angles(&bytes);
    assert!((s - 270.0).abs() < 1e-9 && (e - 90.0).abs() < 1e-9, "stored {s} {e}");
}

#[test]
fn issue_159_a_clockwise_arc_edge_reads_back_as_it_was_written() {
    let bytes = DxfWriter::new(&doc_with(FRAC_PI_2, 3.0 * FRAC_PI_2, false))
        .write_to_vec()
        .unwrap();
    let (s, e, ccw) = read_back(bytes);
    assert!((s - FRAC_PI_2).abs() < 1e-9 && (e - 3.0 * FRAC_PI_2).abs() < 1e-9, "{s} {e}");
    assert!(!ccw);
}

#[test]
fn issue_159_a_dxf_that_autocad_would_write_reads_as_the_same_arc() {
    // What AutoCAD stores for the clockwise arc from 90 to 270 degrees: 270 and 90 with the
    // counter-clockwise flag 0. Made here from a counter-clockwise edge of those two angles,
    // whose flag is the only thing to change.
    let bytes = DxfWriter::new(&doc_with(1.5 * PI, FRAC_PI_2, true)).write_to_vec().unwrap();
    let text = String::from_utf8_lossy(&bytes).replace('\r', "");
    let at = text.find("\nHATCH\n").expect("a HATCH record");
    let flag = text[at..].find("\n 73\n     1\n").expect("the arc's flag") + at;
    let edited = format!("{}\n 73\n     0\n{}", &text[..flag], &text[flag + "\n 73\n     1\n".len()..]);
    let (s, e, ccw) = read_back(edited.into_bytes());
    assert!(!ccw, "the flag is 0");
    assert!((s - FRAC_PI_2).abs() < 1e-9 && (e - 1.5 * PI).abs() < 1e-9, "read {s} {e}");
}

#[test]
fn issue_159_a_counter_clockwise_arc_edge_keeps_its_angles() {
    let bytes = DxfWriter::new(&doc_with(0.25 * PI, 0.75 * PI, true)).write_to_vec().unwrap();
    let (s, e) = stored_angles(&bytes);
    assert!((s - 45.0).abs() < 1e-9 && (e - 135.0).abs() < 1e-9, "stored {s} {e}");
    let (s, e, ccw) = read_back(bytes);
    assert!(ccw && (s - 0.25 * PI).abs() < 1e-9 && (e - 0.75 * PI).abs() < 1e-9);
}
