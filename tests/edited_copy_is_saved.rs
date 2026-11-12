//! An entity written back into the document as an edited copy is saved as
//! edited.
//!
//! An entity read from DWG keeps the record it was read from, and the DWG
//! writer copies that record instead of encoding the entity again. A caller
//! edits a copy of the entity field by field and writes it back through
//! `get_entity_mut`, `entities_mut`, `replace_entity_arc` or
//! `restore_entity_arc`. The record of the entity as it was read must not
//! come back with the copy: the saved drawing holds the edit.
use acadrust::entities::Line;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, EntityType, Handle};
use std::io::Cursor;
use std::sync::Arc;

const VERSIONS: [DxfVersion; 4] = [
    DxfVersion::AC1018,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

fn reread(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

/// A drawing read from DWG with two lines: the one the test edits, from
/// (0, 0) to (10, 0), and one it leaves alone.
fn read_drawing(version: DxfVersion) -> (CadDocument, Handle, Handle) {
    let mut source = CadDocument::with_version(version);
    let edited = source
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
        )))
        .unwrap();
    let untouched = source
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::new(0.0, 5.0, 0.0),
            Vector3::new(10.0, 5.0, 0.0),
        )))
        .unwrap();
    let document = reread(&source);
    for handle in [edited, untouched] {
        assert!(
            document
                .get_entity(handle)
                .unwrap()
                .common()
                .raw_record
                .is_some(),
            "{version:?}: a line read from DWG keeps its record"
        );
    }
    (document, edited, untouched)
}

/// A copy of the line with its end moved to (25, 0), as a caller edits it:
/// by its fields.
fn lengthened_copy(document: &CadDocument, handle: Handle) -> EntityType {
    let mut copy = document.get_entity(handle).unwrap().clone();
    let EntityType::Line(line) = &mut copy else {
        panic!("expected the line")
    };
    line.end = Vector3::new(25.0, 0.0, 0.0);
    copy
}

fn line_end(document: &CadDocument, handle: Handle) -> Vector3 {
    match document.get_entity(handle) {
        Some(EntityType::Line(line)) => line.end,
        other => panic!("expected the line, got {other:?}"),
    }
}

/// The saved drawing holds the edit, and the line nobody touched is still
/// copied from its record.
fn assert_saved_as_edited(
    version: DxfVersion,
    document: &CadDocument,
    edited: Handle,
    untouched: Handle,
) {
    let source_record = document
        .get_entity(untouched)
        .unwrap()
        .common()
        .raw_record
        .clone()
        .expect("the untouched line keeps its record");
    let saved = reread(document);
    assert_eq!(
        line_end(&saved, edited),
        Vector3::new(25.0, 0.0, 0.0),
        "{version:?}: the saved DWG holds the line as it was before the edit"
    );
    assert_eq!(
        saved
            .get_entity(untouched)
            .unwrap()
            .common()
            .raw_record
            .as_ref()
            .map(|record| &record.data),
        Some(&source_record.data),
        "{version:?}: the untouched line is copied from its record"
    );
}

#[test]
fn a_copy_written_through_get_entity_mut_is_saved_as_edited() {
    for version in VERSIONS {
        let (mut document, edited, untouched) = read_drawing(version);
        let copy = lengthened_copy(&document, edited);
        *document.get_entity_mut(edited).unwrap() = copy;
        assert_saved_as_edited(version, &document, edited, untouched);
    }
}

#[test]
fn a_copy_written_through_entities_mut_is_saved_as_edited() {
    for version in VERSIONS {
        let (mut document, edited, untouched) = read_drawing(version);
        let copy = lengthened_copy(&document, edited);
        // The line the test leaves alone is written back as it is.
        let same = document.get_entity(untouched).unwrap().clone();
        for entity in document.entities_mut() {
            if entity.common().handle == edited {
                *entity = copy.clone();
            } else if entity.common().handle == untouched {
                *entity = same.clone();
            }
        }
        let saved = reread(&document);
        assert_eq!(
            line_end(&saved, edited),
            Vector3::new(25.0, 0.0, 0.0),
            "{version:?}: the saved DWG holds the line as it was before the edit"
        );
        assert_eq!(
            line_end(&saved, untouched),
            Vector3::new(10.0, 5.0, 0.0),
            "{version:?}"
        );
    }
}

#[test]
fn a_copy_written_through_replace_entity_arc_is_saved_as_edited() {
    for version in VERSIONS {
        let (mut document, edited, untouched) = read_drawing(version);
        let copy = lengthened_copy(&document, edited);
        assert!(document
            .replace_entity_arc(edited, Arc::new(copy))
            .is_some());
        assert_saved_as_edited(version, &document, edited, untouched);
    }
}

#[test]
fn a_copy_restored_after_removal_is_saved_as_edited() {
    for version in VERSIONS {
        let (mut document, edited, untouched) = read_drawing(version);
        let copy = lengthened_copy(&document, edited);
        assert!(document.remove_entity_arc(edited).is_some());
        assert_eq!(document.restore_entity_arc(Arc::new(copy)), Some(edited));
        assert_saved_as_edited(version, &document, edited, untouched);
    }
}
