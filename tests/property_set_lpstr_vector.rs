//! A property set's `VT_VECTOR | VT_LPSTR` value, as Office writes it in the
//! `\x05DocumentSummaryInformation` stream of an embedded Excel 97-2003 worksheet (the sheet
//! names of PIDDSI_DOCPARTS): each element is a 4-byte length and its characters, with no padding
//! between one element and the next. Read with padding, the second element starts inside the
//! first ("heet2..."), and what is written back makes Excel refuse the embedded object.

use acadrust::compound_file::{CompoundPropertyValue, CompoundStreamContent};

const DOC_SUMMARY: &str = "\u{5}DocumentSummaryInformation";
/// PIDDSI_DOCPARTS: the titles of the document's parts (for a workbook, its sheets).
const DOC_PARTS: u32 = 0x0D;
const VT_LPSTR: u32 = 0x1E;
const VT_VECTOR: u32 = 0x1000;

/// A `\x05DocumentSummaryInformation` stream holding the code page (1252) and the sheet names as a
/// vector of `VT_LPSTR`, each element padded to 4 bytes or not.
fn stream(names: &[&str], padded: bool) -> Vec<u8> {
    let mut code_page = 2u32.to_le_bytes().to_vec(); // VT_I2
    code_page.extend_from_slice(&1252u16.to_le_bytes());
    code_page.extend_from_slice(&[0, 0]);
    let mut parts = (VT_VECTOR | VT_LPSTR).to_le_bytes().to_vec();
    parts.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for name in names {
        let mut chars = name.as_bytes().to_vec();
        chars.push(0);
        parts.extend_from_slice(&(chars.len() as u32).to_le_bytes());
        parts.extend_from_slice(&chars);
        while padded && parts.len() % 4 != 0 {
            parts.push(0);
        }
    }
    while parts.len() % 4 != 0 {
        parts.push(0);
    }
    // The section: its size, two properties, their offsets, then the values.
    let table = 8 + 2 * 8;
    let mut section = Vec::new();
    section.extend_from_slice(&((table + code_page.len() + parts.len()) as u32).to_le_bytes());
    section.extend_from_slice(&2u32.to_le_bytes());
    section.extend_from_slice(&1u32.to_le_bytes());
    section.extend_from_slice(&(table as u32).to_le_bytes());
    section.extend_from_slice(&DOC_PARTS.to_le_bytes());
    section.extend_from_slice(&((table + code_page.len()) as u32).to_le_bytes());
    section.extend_from_slice(&code_page);
    section.extend_from_slice(&parts);
    // The stream header: byte order, version, system, class id, one section at offset 48.
    let mut out = vec![0xFE, 0xFF, 0, 0];
    out.extend_from_slice(&0x0002_0005u32.to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&[
        0x02, 0xD5, 0xCD, 0xD5, 0x9C, 0x2E, 0x1B, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2B, 0x2C, 0xF9,
        0xAE,
    ]);
    out.extend_from_slice(&48u32.to_le_bytes());
    out.extend_from_slice(&section);
    out
}

/// The strings of the stream's PIDDSI_DOCPARTS vector.
fn doc_parts(content: &CompoundStreamContent) -> Vec<String> {
    let CompoundStreamContent::PropertySet(stream) = content else {
        panic!("not read as a property set: {content:?}")
    };
    let parts = stream
        .sets
        .iter()
        .flat_map(|set| &set.properties)
        .find(|property| property.id == DOC_PARTS)
        .expect("PIDDSI_DOCPARTS");
    let CompoundPropertyValue::Vector { values, .. } = &parts.value else {
        panic!("not a vector: {:?}", parts.value)
    };
    values
        .iter()
        .map(|value| match value {
            CompoundPropertyValue::AnsiString { value, .. } => value.clone(),
            other => panic!("not a string: {other:?}"),
        })
        .collect()
}

const SHEETS: [&str; 3] = ["Sheet1", "Sheet2", "Sheet3"];

#[test]
fn an_unpadded_lpstr_vector_reads_each_element() {
    let read = CompoundStreamContent::decode(DOC_SUMMARY, &stream(&SHEETS, false));
    assert_eq!(doc_parts(&read), SHEETS);
}

#[test]
fn an_unpadded_lpstr_vector_is_written_back_with_the_same_elements() {
    let read = CompoundStreamContent::decode(DOC_SUMMARY, &stream(&SHEETS, false));
    let again = CompoundStreamContent::decode(DOC_SUMMARY, &read.encode());
    assert_eq!(doc_parts(&again), SHEETS);
}

/// Elements padded to 4 bytes, as the specification draws them, still read as before.
#[test]
fn a_padded_lpstr_vector_reads_each_element() {
    let read = CompoundStreamContent::decode(DOC_SUMMARY, &stream(&SHEETS, true));
    assert_eq!(doc_parts(&read), SHEETS);
}

/// Names whose length with the terminator is already a multiple of 4 have no padding either way.
#[test]
fn an_lpstr_vector_of_aligned_names_reads_each_element() {
    let names = ["Abc", "Sheet12", "Q1"];
    for padded in [false, true] {
        let read = CompoundStreamContent::decode(DOC_SUMMARY, &stream(&names, padded));
        assert_eq!(doc_parts(&read), names, "padded: {padded}");
    }
}
