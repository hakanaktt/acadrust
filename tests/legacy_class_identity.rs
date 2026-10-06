use acadrust::{
    entities::{EntityType, Line, MultiLeader},
    types::{DxfVersion, Vector3},
    CadDocument, DwgReader, DwgWriter,
};
use std::io::Cursor;

fn roundtrip(doc: &CadDocument) -> CadDocument {
    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(doc).unwrap()))
        .read()
        .unwrap()
}

#[test]
fn same_version_writes_preserve_class_ids_and_untouched_leaders() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1021);
    // Model the class table loaded from an existing file, which may declare
    // more classes than the compact profile of a newly created R2007 file.
    doc.dwg_source_version = Some(doc.version);
    let classes: Vec<_> = doc
        .classes
        .iter()
        .map(|c| (c.dxf_name.clone(), c.class_number))
        .collect();
    let leader = doc
        .add_entity(EntityType::MultiLeader(Box::new(MultiLeader::with_text(
            "Synthetic leader",
            Vector3::new(3.0, 3.0, 0.0),
            vec![Vector3::ZERO, Vector3::UNIT_X],
        ))))
        .unwrap();
    let mut source = roundtrip(&doc);
    for (name, number) in classes {
        assert_eq!(
            source.classes.get_by_name(&name).map(|c| c.class_number),
            Some(number),
            "class {name}"
        );
    }
    assert!(source
        .get_entity(leader)
        .unwrap()
        .common()
        .raw_record
        .is_some());
    source
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::ZERO,
            Vector3::UNIT_Y,
        )))
        .unwrap();
    let saved = roundtrip(&source);
    assert!(matches!(
        saved.get_entity(leader),
        Some(EntityType::MultiLeader(_))
    ));
    assert_eq!(saved.entities().count(), source.entities().count());
}
