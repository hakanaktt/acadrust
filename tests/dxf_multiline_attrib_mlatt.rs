//! Multiline ATTRIB/ATTDEF in DXF before R2018: written as AutoCAD down-saves
//! them (one attribute per line, `<TAG>_001`, ... plus an `ACAD_MLATT` record)
//! and read back as the one multiline attribute an R2018 DXF gives.

use std::io::Cursor;

use acadrust::entities::multileader::LineSpacingStyle;
use acadrust::entities::{
    AttachmentPoint, AttributeDefinition, AttributeEntity, EntityType, Insert, MText, MTextFlag,
};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn mtext(value: &str, at: Vector3) -> MText {
    let mut m = MText::with_value(value, at);
    m.height = 2.5;
    m.rectangle_width = 30.0;
    m.line_spacing_factor = 1.5;
    m.line_spacing_style = LineSpacingStyle::Exactly;
    m.attachment_point = AttachmentPoint::TopLeft;
    m
}

/// Block LABML with a 2-line ATTDEF and an INSERT with a 3-line ATTRIB.
fn drawing(version: DxfVersion) -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::with_version(version);
    let mut record = BlockRecord::new("LABML");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let block = record.handle;
    doc.block_records.add(record).unwrap();
    let mut def = AttributeDefinition::new(
        "NOTE".into(),
        "Note".into(),
        "Default one\\PDefault two".into(),
    );
    def.common.owner_handle = block;
    def.insertion_point = Vector3::new(2.0, 3.0, 0.0);
    def.height = 2.5;
    def.is_multiline = true;
    def.mtext_flag = MTextFlag::MultiLine;
    def.line_count = 2;
    def.embedded_mtext = Some(Box::new(mtext(
        "Default one\\PDefault two",
        Vector3::new(2.0, 3.0, 0.0),
    )));
    let def = doc
        .add_entity(EntityType::AttributeDefinition(def))
        .unwrap();
    let mut att = AttributeEntity::simple("NOTE", "Alpha one\\PBeta two\\PGamma three");
    att.insertion_point = Vector3::new(12.0, 13.0, 0.0);
    att.height = 2.5;
    att.is_multiline = true;
    att.mtext_flag = MTextFlag::MultiLine;
    att.line_count = 3;
    att.embedded_mtext = Some(Box::new(mtext(
        "Alpha one\\PBeta two\\PGamma three",
        Vector3::new(12.0, 13.0, 0.0),
    )));
    let mut insert = Insert::new("LABML", Vector3::new(10.0, 10.0, 0.0));
    insert.attributes.push(att);
    let insert = doc.add_entity(EntityType::Insert(insert)).unwrap();
    (doc, def, insert)
}

fn round_trip(version: DxfVersion) -> (String, CadDocument, Handle, Handle) {
    let (doc, def, insert) = drawing(version);
    let dxf = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let back = DxfReader::from_reader(Cursor::new(dxf.clone().into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    (dxf, back, def, insert)
}

fn attdef(doc: &CadDocument, handle: Handle) -> AttributeDefinition {
    match doc.get_entity(handle) {
        Some(EntityType::AttributeDefinition(a)) => a.clone(),
        _ => panic!("ATTDEF lost"),
    }
}

fn attribs(doc: &CadDocument, handle: Handle) -> Vec<AttributeEntity> {
    match doc.get_entity(handle) {
        Some(EntityType::Insert(i)) => i.attributes.clone(),
        _ => panic!("INSERT lost"),
    }
}

fn block_attdefs(doc: &CadDocument) -> usize {
    doc.entities_in_block("LABML")
        .filter(|e| matches!(e, EntityType::AttributeDefinition(_)))
        .count()
}

/// What a DXF read gives of a multiline attribute, minus the handle-level
/// identity (the R2018 and older files assign them the same way).
fn compare(version: DxfVersion) {
    let (reference_dxf, reference, def, insert) = round_trip(DxfVersion::AC1032);
    // R2018 keeps its embedded objects: no lines, no ACAD_MLATT.
    assert!(
        !reference_dxf.contains("ACAD_MLATT"),
        "ACAD_MLATT in an R2018 DXF"
    );
    assert!(!reference_dxf.contains("NOTE_001"), "R2018 lines split");
    let (dxf, back, _, _) = round_trip(version);
    assert!(
        dxf.contains("ACAD_MLATT"),
        "{version:?}: no ACAD_MLATT record"
    );
    assert!(dxf.contains("NOTE_002"), "{version:?}: lines not split");

    let (a, b) = (attdef(&reference, def), attdef(&back, def));
    assert_eq!(block_attdefs(&back), 1, "{version:?}: ATTDEF lines left");
    assert_eq!(b.tag, a.tag, "{version:?} ATTDEF tag");
    assert_eq!(b.default_value, a.default_value, "{version:?} ATTDEF value");
    assert_eq!(
        b.is_multiline, a.is_multiline,
        "{version:?} ATTDEF multiline"
    );
    assert_eq!(b.mtext_flag, a.mtext_flag, "{version:?} ATTDEF flag");
    assert_eq!(b.line_count, a.line_count, "{version:?} ATTDEF line count");
    assert_eq!(
        b.embedded_mtext, a.embedded_mtext,
        "{version:?} ATTDEF MTEXT"
    );
    assert_eq!(
        b.insertion_point, a.insertion_point,
        "{version:?} ATTDEF insertion"
    );
    assert_eq!(
        b.common.xdictionary_handle, None,
        "{version:?} ATTDEF xdictionary"
    );

    let (a, b) = (attribs(&reference, insert), attribs(&back, insert));
    assert_eq!(b.len(), 1, "{version:?}: ATTRIB lines left");
    let (a, b) = (&a[0], &b[0]);
    assert_eq!(b.tag, a.tag, "{version:?} ATTRIB tag");
    assert_eq!(b.value, a.value, "{version:?} ATTRIB value");
    assert_eq!(
        b.is_multiline, a.is_multiline,
        "{version:?} ATTRIB multiline"
    );
    assert_eq!(b.mtext_flag, a.mtext_flag, "{version:?} ATTRIB flag");
    assert_eq!(b.line_count, a.line_count, "{version:?} ATTRIB line count");
    assert_eq!(
        b.embedded_mtext, a.embedded_mtext,
        "{version:?} ATTRIB MTEXT"
    );
    assert_eq!(
        b.insertion_point, a.insertion_point,
        "{version:?} ATTRIB insertion"
    );
    assert_eq!(
        b.common.xdictionary_handle, None,
        "{version:?} ATTRIB xdictionary"
    );
}

#[test]
fn r2000_multiline_attributes_round_trip_through_acad_mlatt() {
    compare(DxfVersion::AC1015);
}

#[test]
fn r2007_multiline_attributes_round_trip_through_acad_mlatt() {
    compare(DxfVersion::AC1021);
}

#[test]
fn r2013_multiline_attributes_round_trip_through_acad_mlatt() {
    compare(DxfVersion::AC1027);
}

#[test]
fn r2013_dxf_has_autocad_line_layout() {
    let (dxf, ..) = round_trip(DxfVersion::AC1027);
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    let has = |code: &str, value: &str| lines.windows(2).any(|w| w == [code, value]);
    for tag in ["NOTE_001", "NOTE_002", "NOTE_003"] {
        assert!(has("2", tag), "no {tag}");
    }
    for value in [
        "Alpha one",
        "Beta two",
        "Gamma three",
        "Default one",
        "Default two",
    ] {
        assert!(has("1", value), "no line {value}");
    }
    assert!(
        has("1", "Alpha one\\PBeta two\\PGamma three"),
        "no full text"
    );
    assert!(
        has("102", "ACAD_MATT_HEIGHT"),
        "no ACAD_XREC_ROUNDTRIP height"
    );
    assert!(has("3", "ACAD_MLATT"), "no ACAD_MLATT entry");
    assert!(
        has("3", "ACAD_XREC_ROUNDTRIP"),
        "no ACAD_XREC_ROUNDTRIP entry"
    );
    assert!(
        !has("101", "Embedded Object"),
        "R2018 embedded object in an R2013 DXF"
    );
}
