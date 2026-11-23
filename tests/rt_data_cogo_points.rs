//! Family 1 — COGO points: the write-audit pilot formalized as a suite.
//! A point written with `add_cogo_point` (number, position, description,
//! name) survives a save and reopen bit-exact; edits (`set_cogo_point`,
//! `set_cogo_point_name`, `remove_cogo_point`) rewrite the database and the
//! `_All Points` group from the points read back (civil-write.md §3).

#[path = "rt_data/mod.rs"]
mod rt_data;

use acadrust::classes::civil::{add_cogo_point, add_point_group, cogo_points, remove_cogo_point, set_cogo_point, set_cogo_point_name};
use acadrust::types::Vector3;

fn point_by_number<'a>(points: &'a [acadrust::classes::civil::CogoPoint], number: u32) -> &'a acadrust::classes::civil::CogoPoint {
    points.iter().find(|p| p.number == number).unwrap_or_else(|| panic!("point {number} missing"))
}

/// A point group's numbers by name (its record's first text).
fn group_numbers(back: &acadrust::CadDocument, name: &str) -> Vec<u32> {
    rt_data::rt_data_objects(back, "AECC_POINT_PG")
        .into_iter()
        .find(|g| {
            let d = g.decode();
            d.strings.contains(&"AeccImpAllPointsPG".to_string()) && name == "_All Points"
                || d.strings.first().map(String::as_str) == Some(name)
        })
        .expect("point group record")
        .point_group_numbers()
}

#[test]
fn cogo_points_survive_a_roundtrip_bit_exact() {
    let mut doc = rt_data::rt_data_drawing();
    // Positions exercise all three doubles' bits, including a negative
    // elevation and values that are not representable decimals.
    let p1 = Vector3::new(100.2, 200.8, 10.125);
    let p2 = Vector3::new(150.1, 250.6, 12.5);
    let p3 = Vector3::new(-7.3, 8.9, -0.75);
    add_cogo_point(&mut doc, Some(1), p1, Some("TEST"), None, None).unwrap();
    add_cogo_point(&mut doc, Some(2), p2, None, None, None).unwrap();
    add_cogo_point(&mut doc, Some(3), p3, Some("GPS"), None, None).unwrap();
    let h2 = doc
        .entities()
        .find_map(|e| match e {
            acadrust::entities::EntityType::Extended(x) => match &x.data {
                acadrust::entities::ExtendedEntityData::Arch(d) if d.cogo_point_number() == Some(2) => Some(x.common.handle),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    set_cogo_point_name(&mut doc, h2, Some("Alpha")).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let points = cogo_points(&back);
    assert_eq!(points.len(), 3);

    let got1 = point_by_number(&points, 1);
    assert_eq!(got1.position, p1, "position bit-exact");
    assert_eq!(got1.description.as_deref(), Some("TEST"));
    assert_eq!(got1.name, None);
    assert_ne!(got1.style, 0, "a point without a style gets the default point style");
    assert_ne!(got1.label_style, 0, "and the default label style");

    let got2 = point_by_number(&points, 2);
    assert_eq!(got2.position, p2);
    assert_eq!(got2.description, None);
    assert_eq!(got2.name.as_deref(), Some("Alpha"));

    let got3 = point_by_number(&points, 3);
    assert_eq!(got3.position, p3);
    assert_eq!(got3.description.as_deref(), Some("GPS"));
    assert_eq!(got3.name, None);

    // The `_All Points` group holds every point in stored order.
    assert_eq!(group_numbers(&back, "_All Points"), vec![1, 2, 3]);
}

#[test]
fn cogo_point_edits_rewrite_the_database_and_the_groups() {
    let mut doc = rt_data::rt_data_drawing();
    add_cogo_point(&mut doc, Some(1), Vector3::new(0.0, 0.0, 0.0), None, None, None).unwrap();
    let h2 = add_cogo_point(&mut doc, Some(2), Vector3::new(10.0, 0.0, 5.0), Some("OLD"), None, None).unwrap();
    let h3 = add_cogo_point(&mut doc, Some(3), Vector3::new(20.0, 0.0, 7.0), None, None, None).unwrap();
    add_point_group(&mut doc, "G", Some("grp"), &[1, 3]).unwrap();

    set_cogo_point(&mut doc, h2, 5, Vector3::new(12.5, -3.25, 6.75), Some("NEW")).unwrap();
    set_cogo_point_name(&mut doc, h2, Some("renamed")).unwrap();
    remove_cogo_point(&mut doc, h3).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let points = cogo_points(&back);
    assert_eq!(points.iter().map(|p| p.number).collect::<Vec<_>>(), vec![1, 5], "point 3 removed, 2 renumbered 5");
    let five = point_by_number(&points, 5);
    assert_eq!(five.position, Vector3::new(12.5, -3.25, 6.75));
    assert_eq!(five.description.as_deref(), Some("NEW"));
    assert_eq!(five.name.as_deref(), Some("renamed"));
    // A named group rewrites with the members its query selects among the
    // points that are left: its number list (1, 3) now selects only 1.
    assert_eq!(group_numbers(&back, "G"), vec![1]);
    assert_eq!(group_numbers(&back, "_All Points"), vec![1, 5]);
}
