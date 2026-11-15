//! Numbers of a SAB body are written to SAT text in the ACIS form, like the
//! numbers of `SatToken::Float` and `SatToken::Position`: no long runs of
//! zeros, a three-digit exponent (`6.123233995737218e-017`, `2.5e+015`).

use acadrust::entities::acis::{SabReader, SabWriter, SatDocument, SatToken};
use acadrust::entities::{AcisData, EntityType, Region};
use acadrust::{CadDocument, DxfVersion, DxfWriter};

const BODY: &str = "700 0 1 0
@8 acadrust @8 ACIS 7.0 @24 Thu Jan 01 00:00:00 2026
1e-06 9.9999999999999995e-07
body $-1 -1 $-1 $-1 $1 $-1 #
point $-1 -1 $-1 0 0 0 #
straight-curve $-1 -1 $-1 0 0 0 1 0 0 I I #
End-of-ACIS-data
";

fn double(value: f64) -> SatToken {
    SatToken::Sab {
        tag: 0x06,
        data: value.to_le_bytes().to_vec(),
    }
}

fn vector(tag: u8, x: f64, y: f64, z: f64) -> SatToken {
    let mut data = Vec::new();
    for value in [x, y, z] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    SatToken::Sab { tag, data }
}

fn set_tokens(doc: &mut SatDocument, entity: &str, tokens: Vec<SatToken>) {
    let record = doc
        .records
        .iter_mut()
        .find(|record| record.entity_type == entity)
        .expect("record");
    record.raw_text = None;
    record.tokens = tokens;
}

/// A body as AutoCAD ASM stores it in SAB: a point with a tiny coordinate, a
/// line with a huge one and a direction with a 1e-17 component.
fn body(x: f64, y: f64, z: f64) -> SatDocument {
    let mut doc = SatDocument::parse(BODY).expect("SAT");
    set_tokens(&mut doc, "point", vec![vector(0x13, x, y, z)]);
    set_tokens(
        &mut doc,
        "straight-curve",
        vec![vector(0x13, x, y, z), vector(0x14, 0.0, 1.0, 0.0)],
    );
    doc
}

fn record_line<'a>(text: &'a str, entity: &str) -> &'a str {
    text.lines()
        .find(|line| line.starts_with(entity))
        .unwrap_or_else(|| panic!("no {entity} record in {text}"))
}

fn via_sab(doc: &SatDocument) -> String {
    SabReader::read(&SabWriter::write(doc))
        .unwrap()
        .to_sat_string()
}

#[test]
fn coordinates_are_written_with_an_exponent() {
    let text = via_sab(&body(
        6.123233995737218e-17,
        -1.8456401178492946e-13,
        2.5e15,
    ));
    let line = record_line(&text, "point");
    assert!(
        line.contains("6.123233995737218e-017 -1.8456401178492946e-013 2.5e+015"),
        "{text}"
    );
}

#[test]
fn single_doubles_are_written_with_an_exponent() {
    let mut doc = SatDocument::parse(BODY).expect("SAT");
    set_tokens(
        &mut doc,
        "straight-curve",
        vec![double(6.123233995737218e-17), double(2.5e15), double(0.125)],
    );
    let text = via_sab(&doc);
    let line = record_line(&text, "straight-curve");
    assert!(
        line.contains("6.123233995737218e-017 2.5e+015 0.125"),
        "{text}"
    );
}

#[test]
fn dxf_of_a_binary_body_writes_the_numbers_with_an_exponent() {
    let mut region = Region::new();
    region.acis_data = AcisData::from_sab(SabWriter::write(&body(
        6.123233995737218e-17,
        -1.8456401178492946e-13,
        2.5e15,
    )));
    assert!(region.acis_data.sat_data.is_empty());
    let mut document = CadDocument::with_version(DxfVersion::AC1015);
    document.add_entity(EntityType::Region(region)).unwrap();
    let dxf = String::from_utf8(DxfWriter::new(&document).write_to_vec().unwrap()).unwrap();

    let lines: Vec<&str> = dxf.lines().collect();
    let sat: Vec<String> = lines
        .chunks(2)
        .filter(|pair| pair.len() == 2 && pair[0].trim() == "1")
        .map(|pair| AcisData::decode_sat(pair[1]))
        .collect();
    let text = sat.join("\n");
    assert!(
        record_line(&text, "point")
            .contains("6.123233995737218e-017 -1.8456401178492946e-013 2.5e+015"),
        "{text}"
    );
}

#[test]
fn ordinary_numbers_are_written_as_before() {
    let text = via_sab(&body(1.0, 2.5, 0.125));
    assert!(
        record_line(&text, "point").contains(" 1 2.5 0.125"),
        "{text}"
    );
}

#[test]
fn integer_tags_stay_integers() {
    let mut doc = SatDocument::parse(BODY).expect("SAT");
    set_tokens(
        &mut doc,
        "straight-curve",
        vec![
            SatToken::Sab {
                tag: 0x04,
                data: 1_000_000i32.to_le_bytes().to_vec(),
            },
            SatToken::Sab {
                tag: 0x15,
                data: (-7i32).to_le_bytes().to_vec(),
            },
            SatToken::Sab {
                tag: 0x17,
                data: 12_345_678_901i64.to_le_bytes().to_vec(),
            },
        ],
    );
    let text = via_sab(&doc);
    assert!(
        record_line(&text, "straight-curve").contains("1000000 -7 12345678901"),
        "{text}"
    );
}
