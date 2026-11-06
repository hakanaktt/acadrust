//! Signed AcDbText thickness on ATTDEF/ATTRIB, including multiline attributes.
//! Source group 39 is injected into a valid DXF, so the regression compiles
//! against the old public model and fails on lost data rather than a missing field.
use acadrust::entities::{AttributeDefinition, AttributeEntity, Insert, MText, MTextFlag};
use acadrust::tables::BlockRecord;
use acadrust::{
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter, EntityType, Vector3,
};
use std::{collections::BTreeMap, io::Cursor};

type Record = Vec<(i32, String)>;

fn records(bytes: &[u8]) -> Vec<Record> {
    let text = std::str::from_utf8(bytes).unwrap();
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len() % 2, 0);
    let mut records: Vec<Record> = Vec::new();
    for pair in lines.chunks_exact(2) {
        let code = pair[0].trim().parse().unwrap();
        if code == 0 || records.is_empty() {
            records.push(Vec::new());
        }
        records
            .last_mut()
            .unwrap()
            .push((code, pair[1].trim().into()));
    }
    records
}

fn attribute(record: &Record) -> Option<(String, String)> {
    let kind = &record.first()?.1;
    if kind != "ATTDEF" && kind != "ATTRIB" {
        return None;
    }
    Some((
        kind.clone(),
        record.iter().find(|(code, _)| *code == 2)?.1.clone(),
    ))
}

fn expected(kind: &str, tag: &str) -> f64 {
    match (kind, tag) {
        ("ATTDEF", "NEG") => -3.25,
        ("ATTDEF", "POS") => 2.5,
        ("ATTRIB", "NEG") => -1.75,
        ("ATTRIB", "POS") => 4.5,
        (_, "ZERO") => 0.0,
        _ => panic!("unexpected attribute {kind}/{tag}"),
    }
}

fn source(multiline: bool) -> Vec<u8> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut block = BlockRecord::new("DEPTH");
    block.handle = doc.allocate_handle();
    block.block_entity_handle = doc.allocate_handle();
    block.block_end_handle = doc.allocate_handle();
    let owner = block.handle;
    doc.block_records.add(block).unwrap();
    let mut insert = Insert::new("DEPTH", Vector3::new(10.0, 0.0, 0.0));
    for tag in ["NEG", "POS", "ZERO"] {
        let mut definition = AttributeDefinition::new(tag.into(), "Depth".into(), "Default".into());
        definition.common.owner_handle = owner;
        let mut value = AttributeEntity::simple(tag, "Instance");
        if multiline {
            definition.is_multiline = true;
            definition.mtext_flag = MTextFlag::MultiLine;
            definition.line_count = 2;
            definition.embedded_mtext =
                Some(Box::new(MText::with_value("Default\\Ptwo", Vector3::ZERO)));
            value.is_multiline = true;
            value.mtext_flag = MTextFlag::MultiLine;
            value.line_count = 2;
            value.embedded_mtext =
                Some(Box::new(MText::with_value("Instance\\Ptwo", Vector3::ZERO)));
        }
        doc.add_entity(EntityType::AttributeDefinition(definition))
            .unwrap();
        insert.attributes.push(value);
    }
    doc.add_entity(EntityType::Insert(insert)).unwrap();
    let mut output = String::new();
    for mut record in records(&DxfWriter::new(&doc).write_to_vec().unwrap()) {
        if let Some((kind, tag)) = attribute(&record) {
            let thickness = expected(&kind, &tag);
            if thickness != 0.0 {
                // First height belongs to AcDbText, before any embedded MTEXT.
                let at = record.iter().position(|(code, _)| *code == 40).unwrap();
                record.insert(at, (39, thickness.to_string()));
            }
        }
        for (code, value) in record {
            output.push_str(&format!("{code}\n{value}\n"));
        }
    }
    output.into_bytes()
}

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn assert_thickness(doc: &CadDocument, inherited: bool) {
    let mut actual = BTreeMap::new();
    for record in records(&DxfWriter::new(doc).write_to_vec().unwrap()) {
        if let Some(key) = attribute(&record) {
            let thickness = record
                .iter()
                .find(|(code, _)| *code == 39)
                .map_or(0.0, |(_, value)| value.parse::<f64>().unwrap());
            assert!(
                actual.insert(key, thickness).is_none(),
                "duplicate attribute"
            );
        }
    }
    assert_eq!(actual.len(), 6, "all definitions and owned values remain");
    for ((kind, tag), value) in actual {
        let source_kind = if inherited { "ATTDEF" } else { &kind };
        assert_eq!(value, expected(source_kind, &tag), "{kind}/{tag} thickness");
    }
}

#[test]
fn ascii_dxf_preserves_signed_attribute_thickness() {
    assert_thickness(&read_dxf(source(false)), false);
}

#[test]
fn ascii_dxf_preserves_signed_multiline_attribute_thickness() {
    assert_thickness(&read_dxf(source(true)), false);
}

fn binary_dxf(multiline: bool) {
    let doc = read_dxf(source(multiline));
    let bytes = DxfWriter::new_binary(&doc).write_to_vec().unwrap();
    assert_thickness(&read_dxf(bytes), false);
}

#[test]
fn binary_dxf_preserves_signed_attribute_thickness() {
    binary_dxf(false);
}

#[test]
fn binary_dxf_preserves_signed_multiline_attribute_thickness() {
    binary_dxf(true);
}

fn dwg(multiline: bool) {
    let doc = read_dxf(source(multiline));
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let back = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    assert_thickness(&back, false);
}

#[test]
fn dwg_preserves_signed_attribute_thickness() {
    dwg(false);
}

#[test]
fn dwg_preserves_signed_multiline_attribute_thickness() {
    dwg(true);
}

#[test]
fn attributes_created_from_definitions_inherit_thickness() {
    let mut doc = read_dxf(source(false));
    let definitions: Vec<_> = doc
        .entities()
        .filter_map(|entity| match entity {
            EntityType::AttributeDefinition(value) => Some(value.clone()),
            _ => None,
        })
        .collect();
    for entity in doc.entities_mut() {
        if let EntityType::Insert(insert) = entity {
            insert.attributes = definitions
                .iter()
                .map(|definition| {
                    AttributeEntity::from_definition(definition, Some("Instance".into()))
                })
                .collect();
        }
    }
    assert_thickness(&doc, true);
}

#[cfg(feature = "serde")]
fn serde_thickness<T: serde::Serialize + serde::de::DeserializeOwned>(value: T) {
    let mut legacy = serde_json::to_value(value).unwrap();
    legacy.as_object_mut().unwrap().remove("thickness");
    let decoded: T = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap()["thickness"], 0.0);
    for thickness in [-3.25, 2.5] {
        let mut source = legacy.clone();
        source["thickness"] = serde_json::json!(thickness);
        let decoded: T = serde_json::from_value(source).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap()["thickness"],
            thickness
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde_retains_signed_thickness_and_defaults_legacy_records_to_zero() {
    serde_thickness(AttributeDefinition::simple("DEPTH"));
    serde_thickness(AttributeEntity::simple("DEPTH", "Instance"));
}
