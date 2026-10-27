//! `MTextParagraph::end_properties` (#229) is newer than the Serde shape of
//! `MTextParagraph`: input written before the field must still read.
#![cfg(feature = "serde")]

use acadrust::entities::mtext_format::{parse_mtext, MTextParagraph, SpanProperties};

#[test]
fn a_paragraph_serialized_without_end_properties_reads_with_the_default() {
    let paragraph = parse_mtext(r"{\C1;A}\PB", false).paragraphs.remove(0);
    let mut value = serde_json::to_value(&paragraph).unwrap();
    assert!(value
        .as_object_mut()
        .unwrap()
        .remove("end_properties")
        .is_some());
    let restored: MTextParagraph = serde_json::from_value(value).unwrap();
    assert_eq!(restored.spans, paragraph.spans);
    assert_eq!(restored.end_properties, SpanProperties::default());
}
