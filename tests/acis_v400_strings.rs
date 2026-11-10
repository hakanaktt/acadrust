//! Strings in pre-7.0 SAT record data are `<length> <text>`, without the `@`
//! that ACIS 7.0 puts in front of the length.

use acadrust::entities::acis::{
    upgrade_records_from_v400, SabReader, SabWriter, SatDocument, SatRecord, SatToken,
};

/// The refinement and vertex template a body carries in SAT 4.0. Every
/// other field of the refinement is a label; the first one, `grid `, is five
/// characters long and so ends in a space.
const V400_REFINEMENT: &str = "eye_refinement $-1 5 grid  1 3 tri 1 4 surf 0 3 adj 0 4 grad 0 \
9 postcheck 0 4 stol 0.020000000000000004 4 ntol 30 4 dsil 0 8 flatness 0 7 pixarea 0 \
4 hmax 0 6 gridar 0 5 mgrid 3000 5 ugrid 0 5 vgrid 0 10 end_fields #";

fn v400_document() -> String {
    format!(
        "400 4 1 0
4 test 8 ACIS 4.0 24 Thu Jan 01 00:00:00 2026
1 9.9999999999999995e-07 1e-10
body $1 $-1 $-1 $-1 #
ref_vt-eye-attrib $-1 $-1 $-1 $0 $2 $3 #
{V400_REFINEMENT}
vertex_template $-1 3 0 1 8 #
End-of-ACIS-data
"
    )
}

/// The same refinement as ACIS 7.0 saves it.
const V700_DOCUMENT: &str = "700 0 1 0
@4 test @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2026
1 9.9999999999999995e-07 1e-10
body $1 -1 $-1 $-1 $-1 $-1 #
ref_vt-eye-attrib $-1 -1 $-1 $-1 $0 $2 $3 #
eye_refinement $-1 -1 @5 grid  1 @3 tri 1 @4 surf 0 @3 adj 0 @4 grad 0 @9 postcheck 0 \
@4 stol 0.020000000000000004 @4 ntol 30 @4 dsil 0 @8 flatness 0 @7 pixarea 0 @4 hmax 0 \
@6 gridar 0 @5 mgrid 3000 @5 ugrid 0 @5 vgrid 0 @10 end_fields #
vertex_template $-1 -1 3 0 1 8 #
End-of-ACIS-data
";

const LABELS: [&str; 17] = [
    "grid ",
    "tri",
    "surf",
    "adj",
    "grad",
    "postcheck",
    "stol",
    "ntol",
    "dsil",
    "flatness",
    "pixarea",
    "hmax",
    "gridar",
    "mgrid",
    "ugrid",
    "vgrid",
    "end_fields",
];

fn record<'a>(doc: &'a SatDocument, entity_type: &str) -> &'a SatRecord {
    doc.records
        .iter()
        .find(|record| record.entity_type == entity_type)
        .unwrap_or_else(|| panic!("missing {entity_type}"))
}

#[test]
fn v400_refinement_labels_are_strings() {
    let doc = SatDocument::parse(&v400_document()).unwrap();
    let tokens = &record(&doc, "eye_refinement").tokens;
    let labels: Vec<_> = tokens.iter().step_by(2).collect();
    let expected: Vec<_> = LABELS
        .iter()
        .map(|label| SatToken::String(label.to_string()))
        .collect();
    assert_eq!(labels, expected.iter().collect::<Vec<_>>());
    assert_eq!(tokens.len(), 2 * LABELS.len() - 1);
    assert_eq!(tokens[1], SatToken::Integer(1));
    assert_eq!(tokens[13], SatToken::Float(0.020000000000000004));
    assert_eq!(tokens[27], SatToken::Integer(3000));
}

#[test]
fn v400_numbers_outside_strings_stay_numbers() {
    let doc = SatDocument::parse(&v400_document()).unwrap();
    assert_eq!(
        record(&doc, "vertex_template").tokens,
        [3, 0, 1, 8].map(SatToken::Integer)
    );
}

#[test]
fn v400_refinement_round_trips_as_sat_400() {
    let text = SatDocument::parse(&v400_document())
        .unwrap()
        .to_sat_string();
    assert!(text.lines().any(|line| line == V400_REFINEMENT), "{text}");
}

#[test]
fn v400_refinement_upgrades_to_counted_strings() {
    let mut doc = SatDocument::parse(&v400_document()).unwrap();
    upgrade_records_from_v400(&mut doc);
    let text = doc.to_sat_string();
    let line = text
        .lines()
        .find(|line| line.starts_with("eye_refinement"))
        .unwrap();
    assert!(
        line.starts_with("eye_refinement $-1 -1 @5 grid  1 @3 tri 1 @4 surf 0 "),
        "{line}"
    );
    assert!(line.ends_with(" 0 @10 end_fields #"), "{line}");
}

#[test]
fn v400_refinement_writes_the_sab_of_its_v700_form() {
    let sab_tokens = |doc: SatDocument| {
        let read = SabReader::read(&SabWriter::write(&doc)).unwrap();
        record(&read, "eye_refinement").tokens.clone()
    };
    let v400 = sab_tokens(SatDocument::parse(&v400_document()).unwrap());
    let v700 = sab_tokens(SatDocument::parse(V700_DOCUMENT).unwrap());
    let labels: Vec<_> = v700.iter().step_by(2).map(SatToken::as_string).collect();
    assert_eq!(labels, LABELS.map(Some));
    assert_eq!(v400, v700);
}
