//! An ASM body keeps the fields of a `transform` record as one literal string
//! in SAB (tag 0x12). In SAT those are the record's own fields, not a counted
//! string: `transform $-1 -1 1 0 0 0 ... no_shear #`.

use acadrust::entities::acis::{SabReader, SabWriter, SatDocument, SatToken};
use acadrust::entities::{AcisData, EntityType, Region};
use acadrust::{CadDocument, DxfVersion, DxfWriter};

const TRANSFORM_FIELDS: &str =
    "1 0 0 0 1 0 0 0 1 -780398.97062141041 57711.560415780958 0 1 no_rotate no_reflect no_shear ";

const TRANSFORM_LINE: &str = "transform $-1 -1 1 0 0 0 1 0 0 0 1 -780398.97062141041 \
57711.560415780958 0 1 no_rotate no_reflect no_shear #";

const BODY: &str = "700 0 1 0
@8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2026
1e-06 9.9999999999999995e-07
body $-1 -1 $-1 $-1 $1 $-1 #
transform $-1 -1 1 0 0 0 1 0 0 0 1 0 0 0 1 no_rotate no_reflect no_shear #
End-of-ACIS-data
";

fn sab_literal(text: &str) -> SatToken {
    let mut data = (text.len() as u32).to_le_bytes().to_vec();
    data.extend_from_slice(text.as_bytes());
    SatToken::Sab { tag: 0x12, data }
}

/// The body as ODA ACIS Builder 223 stores it in SAB: the transform record
/// carries its fields as one tag 0x12 literal string.
fn literal_transform_document() -> SatDocument {
    let mut doc = SatDocument::parse(BODY).expect("SAT");
    let transform = doc
        .records
        .iter_mut()
        .find(|record| record.entity_type == "transform")
        .expect("transform record");
    transform.raw_text = None;
    transform.tokens = vec![sab_literal(TRANSFORM_FIELDS)];
    doc
}

fn transform_line(text: &str) -> &str {
    text.lines()
        .find(|line| line.starts_with("transform"))
        .unwrap_or_else(|| panic!("no transform record in {text}"))
}

#[test]
fn literal_string_is_written_as_the_record_fields() {
    let doc = SabReader::read(&SabWriter::write(&literal_transform_document())).unwrap();
    let text = doc.to_sat_string();
    assert_eq!(transform_line(&text), TRANSFORM_LINE, "{text}");
}

#[test]
fn dxf_of_a_binary_body_writes_the_transform_without_a_count() {
    let mut region = Region::new();
    region.acis_data = AcisData::from_sab(SabWriter::write(&literal_transform_document()));
    assert!(region.acis_data.sat_data.is_empty());
    let mut document = CadDocument::with_version(DxfVersion::AC1015);
    document.add_entity(EntityType::Region(region)).unwrap();
    let dxf = String::from_utf8(DxfWriter::new(&document).write_to_vec().unwrap()).unwrap();

    // Group code 1 lines carry the SAT text, one record per line, ciphered.
    let lines: Vec<&str> = dxf.lines().collect();
    let sat: Vec<String> = lines
        .chunks(2)
        .filter(|pair| pair.len() == 2 && pair[0].trim() == "1")
        .map(|pair| AcisData::decode_sat(pair[1]))
        .collect();
    let text = sat.join("\n");
    assert_eq!(transform_line(&text), TRANSFORM_LINE, "{text}");
}

#[test]
fn counted_string_stays_counted() {
    let mut doc = literal_transform_document();
    let body = doc
        .records
        .iter_mut()
        .find(|record| record.entity_type == "body")
        .unwrap();
    body.raw_text = None;
    body.tokens = vec![SatToken::String("abc ".into())];
    let text = SabReader::read(&SabWriter::write(&doc))
        .unwrap()
        .to_sat_string();
    let line = text.lines().find(|l| l.starts_with("body")).unwrap();
    assert_eq!(line, "body $-1 -1 @4 abc  #", "{text}");
}

#[test]
fn literal_string_survives_the_sab_round_trip() {
    let doc = SabReader::read(&SabWriter::write(&literal_transform_document())).unwrap();
    let transform = doc
        .records
        .iter()
        .find(|record| record.entity_type == "transform")
        .unwrap();
    assert_eq!(transform.tokens, [sab_literal(TRANSFORM_FIELDS)]);
    let again = SabReader::read(&SabWriter::write(&doc)).unwrap();
    assert_eq!(again.records, doc.records);
}
