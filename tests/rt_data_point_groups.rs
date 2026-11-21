//! Families 2 and 3 — point groups with overrides, and point group queries.
//!
//! Family 2 is READY: the overrides are asserted field-exact as
//! `PointGroupOverrides` reads them back (civil-write.md §3, samples
//! PG01–PG07, E-PG1/E-PG2).
//!
//! Family 3 is READY-WITH-LIMITS: the asserted surface is the group's
//! **stored member list** (`point_group_numbers()`), which the writer
//! resolves from the query over the points written. The query statement
//! records (`AECC_QUERY_STATEMENT`) themselves have no public decoder at
//! this tip, so their bits are not asserted here; queries the writer
//! refuses (custom queries with a NOT after AND/OR, a comparison opening
//! or closing two parentheses) are out of scope, as is the application's
//! member-mark behaviour on files it edits.

#[path = "rt_data/mod.rs"]
mod rt_data;

use acadrust::classes::civil::{
    add_cogo_point, add_point_group, add_point_group_query, add_point_property_classification, cogo_points, set_point_group, set_point_group_overrides, Comparison, PointAttribute, PointGroupClassification, PointGroupOverrides, PointQuery, QueryConstant, QueryExpression,
};
use acadrust::types::Vector3;

/// The group record with this name (its first text; `_All Points` has "").
fn group_by_name(back: &acadrust::CadDocument, name: &str) -> acadrust::classes::arch::ArchObject {
    rt_data::rt_data_objects(back, "AECC_POINT_PG")
        .into_iter()
        .find(|g| {
            let d = g.decode();
            // `_All Points` carries no name text, only its well-known class
            // text; a named group's first text is its name.
            d.strings.contains(&"AeccImpAllPointsPG".to_string()) && name == "_All Points"
                || d.strings.first().map(String::as_str) == Some(name)
        })
        .unwrap_or_else(|| panic!("point group {name} missing"))
}

fn group_numbers(back: &acadrust::CadDocument, name: &str) -> Vec<u32> {
    group_by_name(back, name).point_group_numbers()
}

fn four_points(doc: &mut acadrust::CadDocument) {
    let pts = [
        (1, Vector3::new(0.0, 0.0, 10.0), Some("TEST"), None),
        (2, Vector3::new(10.0, 0.0, 30.0), Some("test"), Some("Alpha")),
        (3, Vector3::new(20.0, 0.0, 12.0), Some("TESTS"), None),
        (4, Vector3::new(30.0, 0.0, 25.0), Some("ABC"), Some("Beta")),
    ];
    for (n, p, d, _) in pts {
        add_cogo_point(doc, Some(n), p, d, None, None).unwrap();
    }
    for (n, .., name) in pts {
        if let Some(name) = name {
            let h = rt_data::rt_data_entities(doc, "AECC_COGO_POINT")
                .into_iter()
                .find(|(_, d)| d.cogo_point_number() == Some(n))
                .unwrap()
                .0;
            acadrust::classes::civil::set_cogo_point_name(doc, h, Some(name)).unwrap();
        }
    }
}

#[test]
fn point_group_overrides_survive_field_exact() {
    let mut doc = rt_data::rt_data_drawing();
    four_points(&mut doc);
    // A group without overrides, and one with every override on.
    add_point_group(&mut doc, "Plain", Some("no overrides"), &[2, 4]).unwrap();
    add_point_property_classification(&mut doc, "C1").unwrap();
    // The overriding styles are the points' own (default) styles.
    let points = cogo_points(&doc);
    let overrides = PointGroupOverrides {
        point_style: Some(points[0].style),
        label_style: Some(points[0].label_style),
        elevation: Some(7.5),
        raw_description: Some("ZZ".into()),
        locked: true,
        classification: PointGroupClassification::Custom("C1".into()),
    };
    let full = add_point_group(&mut doc, "Full", None, &[1, 3]).unwrap();
    set_point_group_overrides(&mut doc, full, &overrides).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    // Members in stored order (ascending), the description kept.
    assert_eq!(group_numbers(&back, "Plain"), vec![2, 4]);
    assert_eq!(group_numbers(&back, "Full"), vec![1, 3]);
    assert_eq!(group_numbers(&back, "_All Points"), vec![1, 2, 3, 4]);

    let got = group_by_name(&back, "Full").point_group_overrides().expect("overrides read back");
    assert_eq!(got.point_style, Some(points[0].style), "overriding point style");
    assert_eq!(got.label_style, Some(points[0].label_style), "overriding label style");
    assert_eq!(got.elevation, Some(7.5), "fixed elevation");
    assert_eq!(got.raw_description.as_deref(), Some("ZZ"), "raw description override");
    assert!(got.locked, "points locked");
    assert_eq!(got.classification, PointGroupClassification::Custom("C1".into()), "classification");

    // A group written without overrides reads none.
    let plain_got = group_by_name(&back, "Plain").point_group_overrides().expect("recent record");
    assert_eq!(plain_got.point_style, None);
    assert_eq!(plain_got.label_style, None);
    assert_eq!(plain_got.elevation, None);
    assert_eq!(plain_got.raw_description, None);
    assert!(!plain_got.locked);
    assert_eq!(plain_got.classification, PointGroupClassification::None);
}

#[test]
fn a_plain_group_renames_and_takes_new_members() {
    let mut doc = rt_data::rt_data_drawing();
    four_points(&mut doc);
    let g = add_point_group(&mut doc, "G1", Some("first"), &[2, 4]).unwrap();
    set_point_group(&mut doc, g, "G1 renamed", Some("second"), &[3, 1]).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    assert_eq!(group_numbers(&back, "G1 renamed"), vec![1, 3]);
    assert!(rt_data::rt_data_objects(&back, "AECC_POINT_PG")
        .iter()
        .all(|g| g.decode().strings.first().map(String::as_str) != Some("G1")), "the old name is gone");
}

#[test]
fn standard_queries_select_their_members() {
    let mut doc = rt_data::rt_data_drawing();
    four_points(&mut doc);
    // (name, query, expected members in stored order).
    let wanted: Vec<(&str, PointQuery, Vec<u32>)> = vec![
        ("N", PointQuery::Numbers(vec![2, 4]), vec![2, 4]),
        ("E", PointQuery::Elevation(Comparison::Greater, 25.0), vec![2]),
        ("EL", PointQuery::Elevation(Comparison::LessOrEqual, 12.0), vec![1, 3]),
        ("ER", PointQuery::ElevationRange(11.0, 26.0), vec![3, 4]),
        // Descriptions match over the whole text, case ignored: `test`
        // selects TEST and test, not TESTS.
        ("RD", PointQuery::RawDescription(vec!["TEST".into(), "ABC".into()]), vec![1, 2, 4]),
        // The writer adds no description keys, so a full description is
        // the raw one: the same members.
        ("FD", PointQuery::FullDescription(vec!["T?ST".into()]), vec![1, 2]),
        ("NM", PointQuery::Names(vec!["alpha".into(), "b*".into()]), vec![2, 4]),
        ("EX", PointQuery::Excluding(None, Box::new(PointQuery::Numbers(vec![1]))), vec![2, 3, 4]),
    ];
    for (name, query, _) in &wanted {
        add_point_group_query(&mut doc, name, None, query).unwrap();
    }

    let back = rt_data::rt_data_roundtrip(&doc);
    for (name, _, members) in &wanted {
        assert_eq!(group_numbers(&back, name), *members, "members of query group {name}");
    }
}

#[test]
fn a_custom_query_selects_its_members() {
    let mut doc = rt_data::rt_data_drawing();
    four_points(&mut doc);
    // n <= 2 OR (z > 20 AND raw description = test): 1, 2.
    let query = PointQuery::Custom(QueryExpression::Or(
        Box::new(QueryExpression::Compare(PointAttribute::Number, Comparison::LessOrEqual, QueryConstant::Number(2))),
        Box::new(QueryExpression::Parens(Box::new(QueryExpression::And(
            Box::new(QueryExpression::Compare(PointAttribute::Elevation, Comparison::Greater, QueryConstant::Elevation(20.0))),
            Box::new(QueryExpression::Compare(PointAttribute::RawDescription, Comparison::Equal, QueryConstant::Pattern("test".into()))),
        )))),
    ));
    add_point_group_query(&mut doc, "Custom", Some("custom query"), &query).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    assert_eq!(group_numbers(&back, "Custom"), vec![1, 2]);
}
