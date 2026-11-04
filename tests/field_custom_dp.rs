//! `%<\AcVar CustomDP.<Name>>%`: AutoCAD's field code for a custom drawing
//! property resolves to the stored value verbatim.

use acadrust::fields::{self, FieldContext};
use acadrust::CadDocument;

struct Context;

impl FieldContext for Context {
    fn now_julian(&self) -> f64 {
        2460000.0
    }
    fn getvar(&self, name: &str) -> Option<String> {
        match name.to_lowercase().as_str() {
            "viewctr" => Some("3,4".into()),
            _ => None,
        }
    }
}

fn doc_with(props: &[(&str, &str)]) -> CadDocument {
    let mut doc = CadDocument::new();
    for (k, v) in props {
        doc.summary_info
            .custom_properties
            .push(((*k).into(), (*v).into()));
    }
    doc
}

fn eval(doc: &CadDocument, code: &str) -> Option<String> {
    fields::evaluate_code(doc, code, &[], None, &Context)
}

#[test]
fn comma_values_stay_verbatim() {
    let doc = doc_with(&[("Ratio", "12,5"), ("Triple", "1,2,3")]);
    assert_eq!(
        eval(&doc, r"%<\AcVar CustomDP.Ratio>%").as_deref(),
        Some("12,5")
    );
    assert_eq!(
        eval(&doc, r"%<\AcVar CustomDP.Triple>%").as_deref(),
        Some("1,2,3")
    );
}

#[test]
fn plain_value_resolves() {
    let doc = doc_with(&[("Client", "Acme")]);
    assert_eq!(
        eval(&doc, r"%<\AcVar CustomDP.Client>%").as_deref(),
        Some("Acme")
    );
}

#[test]
fn names_and_prefix_are_case_insensitive() {
    let doc = doc_with(&[("Client", "Acme")]);
    assert_eq!(
        eval(&doc, r"%<\AcVar CustomDP.CLIENT>%").as_deref(),
        Some("Acme")
    );
    assert_eq!(
        eval(&doc, r"%<\AcVar customdp.client>%").as_deref(),
        Some("Acme")
    );
}

#[test]
fn text_format_applies() {
    let doc = doc_with(&[("Client", "Acme")]);
    assert_eq!(
        eval(&doc, r#"%<\AcVar CustomDP.Client \f "%tc1">%"#).as_deref(),
        Some("ACME")
    );
}

#[test]
fn missing_property_is_unresolved() {
    let doc = doc_with(&[("Client", "Acme")]);
    assert_eq!(eval(&doc, r"%<\AcVar CustomDP.Nope>%"), None);
}

#[test]
fn missing_property_never_falls_to_a_sysvar() {
    let doc = doc_with(&[]);
    assert_eq!(eval(&doc, r"%<\AcVar CustomDP.viewctr>%"), None);
}

#[test]
fn empty_value_shows_no_value() {
    let doc = doc_with(&[("Blank", "")]);
    assert_eq!(
        eval(&doc, r"%<\AcVar CustomDP.Blank>%").as_deref(),
        Some("----")
    );
}

#[test]
fn bare_custom_name_still_resolves() {
    let doc = doc_with(&[("Client", "Acme")]);
    assert_eq!(eval(&doc, r"%<\AcVar Client>%").as_deref(), Some("Acme"));
}

#[test]
fn point_sysvar_is_still_formatted() {
    let doc = doc_with(&[]);
    assert_eq!(
        eval(&doc, r"%<\AcVar viewctr>%").as_deref(),
        Some("3.000000, 4.000000, 0.000000")
    );
}
