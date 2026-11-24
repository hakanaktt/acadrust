//! The point database of drawings the application wrote.
//!
//! Two record forms the application writes and this crate's writer does
//! not (the D016 packet-1 read gap W1-5): the per-record marker short is
//! `0x0421` (and `0x0621` for points a survey created), not the `0x0420`
//! this crate writes, and the database carries a leading tagged list
//! before the named points where the crate's records keep an empty one.
//! Without both, `cogo_points` decoded no application point database at
//! all (0 of 538 in Survey-3.dwg, 0 of 2405 in Points-4a.dwg).

use acadrust::classes::civil::{cogo_points, CogoPoint};
use acadrust::DwgReader;

/// The application's tutorial drawing, from `ACADRUST_CIVIL_TUTORIALS_DIR`.
fn app_drawing(name: &str) -> Option<acadrust::CadDocument> {
    let dir = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR")?;
    let path = std::path::Path::new(&dir).join(name);
    if !path.exists() {
        return None;
    }
    Some(DwgReader::from_file(&path).unwrap().read().unwrap())
}

fn by_number<'a>(points: &'a [CogoPoint], number: u32) -> &'a CogoPoint {
    points.iter().find(|p| p.number == number).unwrap_or_else(|| panic!("point {number} missing"))
}

/// Survey-3.dwg: 538 points a survey created (marker `0x0621`, a leading
/// tagged list of 0, raw descriptions of all points, names of none).
/// Positions and raw descriptions of the application's oracle reads
/// (execute_civil_code `CogoPoint.Easting/Northing/Elevation/RawDescription`,
/// 2026-11-24).
#[test]
fn survey_created_points_decode() {
    let Some(doc) = app_drawing("Survey-3.dwg") else { return };
    let points = cogo_points(&doc);
    assert_eq!(points.len(), 538);
    let p = by_number(&points, 456);
    assert_eq!(p.position.x, 5675.002124308634);
    assert_eq!(p.position.y, 5040.000833228553);
    assert_eq!(p.position.z, 255.95998686447933);
    assert_eq!(p.name, None);
    let p = by_number(&points, 816);
    assert_eq!(p.position.x, 5518.053784082379);
    assert_eq!(p.position.y, 5445.828551114028);
    assert_eq!(p.position.z, 247.7690529209087);
    // every point has a raw description, none has a name
    assert_eq!(points.iter().filter(|p| p.description.is_some()).count(), 538);
    assert_eq!(points.iter().filter(|p| p.name.is_some()).count(), 0);
}

/// Points-4a.dwg: 2405 points of the create-points commands (marker
/// `0x0421`, a leading tagged list of 51, raw descriptions of all points,
/// names of none). The first three points exactly as the application
/// reads them (execute_civil_code, D017 packet 2, 2026-11-24:
/// E=19857.2577 N=20260.4556 Z=100.3058 name= etc.).
#[test]
fn create_points_commands_points_decode() {
    let Some(doc) = app_drawing("Points-4a.dwg") else { return };
    let points = cogo_points(&doc);
    assert_eq!(points.len(), 2405);
    for (number, x, y, z) in [
        (1, 19857.2577, 20260.4556, 100.3058),
        (2, 19875.4801, 20207.7198, 101.0929),
        (3, 19887.3953, 20692.713, 96.4194),
    ] {
        let p = by_number(&points, number);
        assert_eq!(p.position.x, x, "point {number} easting");
        assert_eq!(p.position.y, y, "point {number} northing");
        assert_eq!(p.position.z, z, "point {number} elevation");
        assert_eq!(p.name, None, "point {number} name");
    }
}

/// The same database in the drawing the application saves the points
/// tutorial's work into keeps all 2405 points, and a drawing where the
/// points carry their own label styles (Points-4a: 51) maps them.
#[test]
fn point_label_styles_and_counts_hold_across_the_tutorial_set() {
    for (name, count) in [("Points-2.dwg", 2405), ("Points-3.dwg", 2405), ("Points-4e.dwg", 2405), ("Survey-2B.dwg", 538)] {
        let Some(doc) = app_drawing(name) else { continue };
        let points = cogo_points(&doc);
        assert_eq!(points.len(), count, "{name}");
        assert!(points.iter().all(|p| p.position.x.is_finite() && p.number > 0), "{name}");
    }
    let Some(doc) = app_drawing("Points-4a.dwg") else { return };
    let points = cogo_points(&doc);
    let styled_labels = points.iter().filter(|p| p.label_style != 0).count();
    assert_eq!(styled_labels, 51, "Points-4a label styles");
}
