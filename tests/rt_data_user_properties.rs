//! Family 4 — user-defined point properties: classifications, a property
//! of every kind (whole number, double, text, boolean, enumeration) and
//! the values points hold (civil-write.md §3 "User-defined properties",
//! samples UD01/UD02, E-UD1). READY: every definition field (bounds,
//! defaults, descriptions, choices) and every value reads back as written.
//! A property's key text is 32 identifier digits and the name; the digits
//! come from the writer's serial, so the tests assert the name and that
//! the keys are distinct, not the digits.

#[path = "rt_data/mod.rs"]
mod rt_data;

use acadrust::classes::civil::{
    add_cogo_point, add_point_choice_property, add_point_property, add_point_property_classification, point_properties, set_cogo_point_property, PropertyValue,
};
use acadrust::types::Vector3;

#[test]
fn user_property_definitions_and_values_survive_a_roundtrip() {
    let mut doc = rt_data::rt_data_drawing();
    add_cogo_point(&mut doc, Some(1), Vector3::new(0.0, 0.0, 0.0), None, None, None).unwrap();
    add_cogo_point(&mut doc, Some(2), Vector3::new(10.0, 0.0, 1.0), None, None, None).unwrap();

    add_point_property_classification(&mut doc, "C1").unwrap();
    add_point_property_classification(&mut doc, "C2").unwrap();
    let double_key = add_point_property(
        &mut doc,
        "C1",
        "P1 double",
        "a bounded double",
        PropertyValue::Double(2.5),
        true,
        Some(((PropertyValue::Double(0.0), true), (PropertyValue::Double(10.0), true))),
    )
    .unwrap();
    // Whole number with the full range (the kind's default bounds).
    let int_key = add_point_property(&mut doc, "C1", "I1 whole", "a whole number", PropertyValue::Integer(0), false, None).unwrap();
    // Text: no bounds at all.
    let text_key = add_point_property(&mut doc, "C1", "S1 text", "a text", PropertyValue::Text("ab".into()), true, None).unwrap();
    // Boolean without a default in use.
    let bool_key = add_point_property(&mut doc, "C2", "B1 bool", "a boolean", PropertyValue::Boolean(false), false, None).unwrap();
    // Enumeration with three choices, the default among them.
    let choice_key = add_point_choice_property(&mut doc, "C2", "Q1 choice", "an enumeration", &["A", "B", "C"], 1, true).unwrap();

    let (one, two) = (point_handle(&doc, 1), point_handle(&doc, 2));
    set_cogo_point_property(&mut doc, one, &double_key, Some(PropertyValue::Double(2.25))).unwrap();
    set_cogo_point_property(&mut doc, one, &int_key, Some(PropertyValue::Integer(7))).unwrap();
    set_cogo_point_property(&mut doc, one, &text_key, Some(PropertyValue::Text("xy".into()))).unwrap();
    set_cogo_point_property(&mut doc, one, &bool_key, Some(PropertyValue::Boolean(false))).unwrap();
    set_cogo_point_property(&mut doc, one, &choice_key, Some(PropertyValue::Choice(2))).unwrap();
    // The second point holds only two of the properties.
    set_cogo_point_property(&mut doc, two, &double_key, Some(PropertyValue::Double(4.5))).unwrap();
    set_cogo_point_property(&mut doc, two, &choice_key, Some(PropertyValue::Choice(0))).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let defs = point_properties(&back);
    assert_eq!(defs.len(), 5, "one property of each kind");

    let by_name = |name: &str| defs.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("property {name} missing"));
    let p1 = by_name("P1 double");
    assert_eq!(p1.classification, "C1");
    assert_eq!(p1.description, "a bounded double");
    assert_eq!(p1.default, PropertyValue::Double(2.5));
    assert!(p1.use_default);
    assert_eq!(p1.bounds, Some(((PropertyValue::Double(0.0), true), (PropertyValue::Double(10.0), true))), "bounds as written");
    assert!(p1.choices.is_empty());
    assert!(p1.key.ends_with("P1 double"), "the key text ends with the name");

    let i1 = by_name("I1 whole");
    assert_eq!(i1.classification, "C1");
    assert_eq!(i1.default, PropertyValue::Integer(0));
    assert!(!i1.use_default);
    assert_eq!(i1.bounds, Some(((PropertyValue::Integer(i32::MIN), true), (PropertyValue::Integer(i32::MAX), true))), "the full range by default");

    let s1 = by_name("S1 text");
    assert_eq!(s1.default, PropertyValue::Text("ab".into()));
    assert!(s1.use_default);
    assert_eq!(s1.bounds, None, "texts are not bounded");

    let b1 = by_name("B1 bool");
    assert_eq!(b1.classification, "C2");
    assert_eq!(b1.default, PropertyValue::Boolean(false));
    assert!(!b1.use_default);
    assert_eq!(b1.bounds, Some(((PropertyValue::Boolean(false), false), (PropertyValue::Boolean(true), false))), "booleans 0 and 1, no inclusive flag");

    let q1 = by_name("Q1 choice");
    assert_eq!(q1.classification, "C2");
    assert_eq!(q1.default, PropertyValue::Choice(1));
    assert_eq!(q1.choices, vec!["A".to_string(), "B".to_string(), "C".to_string()]);
    assert_eq!(q1.bounds, None);

    // The keys are distinct per property.
    let mut keys: Vec<&str> = defs.iter().map(|p| p.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), 5, "five distinct key texts");

    let points = acadrust::classes::civil::cogo_points(&back);
    let one = points.iter().find(|p| p.number == 1).unwrap();
    let want1 = vec![
        (double_key.as_str(), PropertyValue::Double(2.25)),
        (int_key.as_str(), PropertyValue::Integer(7)),
        (bool_key.as_str(), PropertyValue::Boolean(false)),
        (text_key.as_str(), PropertyValue::Text("xy".into())),
        (choice_key.as_str(), PropertyValue::Choice(2)),
    ];
    assert_eq!(one.properties.len(), want1.len(), "point 1 holds all five values");
    for (key, value) in want1 {
        assert!(
            one.properties.iter().any(|(k, v)| k == key && v == &value),
            "point 1 holds {key} = {value:?}"
        );
    }

    let two = points.iter().find(|p| p.number == 2).unwrap();
    assert_eq!(two.properties.len(), 2, "point 2 holds only the two properties set on it");
    assert!(two.properties.iter().any(|(k, v)| k == &double_key && v == &PropertyValue::Double(4.5)));
    assert!(two.properties.iter().any(|(k, v)| k == &choice_key && v == &PropertyValue::Choice(0)));
}

fn point_handle(doc: &acadrust::CadDocument, number: u32) -> acadrust::types::Handle {
    rt_data::rt_data_entities(doc, "AECC_COGO_POINT")
        .into_iter()
        .find(|(_, d)| d.cogo_point_number() == Some(number))
        .unwrap_or_else(|| panic!("point {number} missing"))
        .0
}
