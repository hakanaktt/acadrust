//! Issue 222: the DXF header reader skipped `$PELLIPSE` and `$PUCSBASE`.

use acadrust::DxfReader;
use std::io::Cursor;

fn header(vars: &str) -> acadrust::document::HeaderVariables {
    let text = format!(
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1032\n{vars}  0\nENDSEC\n  0\nEOF\n"
    );
    DxfReader::from_reader(Cursor::new(text.into_bytes()))
        .expect("open")
        .read()
        .expect("read")
        .header
}

#[test]
fn pellipse_as_an_integer_is_read() {
    assert!(header("  9\n$PELLIPSE\n 70\n     1\n").create_ellipse_as_polyline);
    assert!(!header("  9\n$PELLIPSE\n 70\n     0\n").create_ellipse_as_polyline);
}

#[test]
fn pellipse_as_a_bool_is_read() {
    assert!(header("  9\n$PELLIPSE\n290\n     1\n").create_ellipse_as_polyline);
    assert!(!header("  9\n$PELLIPSE\n290\n     0\n").create_ellipse_as_polyline);
}

#[test]
fn pucsbase_is_read() {
    assert_eq!(header("  9\n$PUCSBASE\n  2\nMyPaperUcs\n").paper_ucs_base, "MyPaperUcs");
}

#[test]
fn neighbours_of_the_new_arms_are_unaffected() {
    let h = header("  9\n$PELLIPSE\n 70\n     1\n  9\n$PUCSBASE\n  2\nP\n  9\n$CSHADOW\n280\n     2\n");
    assert_eq!(h.cast_shadow, 2);
}
