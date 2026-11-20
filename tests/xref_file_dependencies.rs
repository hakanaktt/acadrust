use acadrust::{document::FileDependency, types::DxfVersion, CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

fn dependency() -> FileDependency {
    FileDependency {
        feature: "Acad:XRef".into(),
        file_name: "../Refs/grond plan.dwg".into(),
        found_path: String::new(),
        fingerprint_guid: Some("{E5B4A9AF-CF20-4FC9-A30F-FFB20FD987E1}".into()),
        version_guid: Some("{48AE354E-C38E-40C2-BA64-B971A4C89005}".into()),
        timestamp: 123,
        file_size: 456,
        affects_graphics: true,
        reference_count: 2,
    }
}

#[test]
fn dwg_dependency_identity_survives_offline_roundtrip() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1032] {
        let mut doc = CadDocument::with_version(version);
        doc.header.fingerprint_guid = "{11111111-2222-3333-4444-555555555555}".into();
        let mut other = dependency();
        other.feature = "Acad:Image".into();
        other.file_name = "image.png".into();
        other.fingerprint_guid = None;
        other.version_guid = None;
        other.affects_graphics = false;
        doc.file_dependencies = vec![dependency(), other];
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert_eq!(read.file_dependencies, doc.file_dependencies, "{version:?}");
        assert_eq!(read.header.fingerprint_guid, doc.header.fingerprint_guid);
    }
}

#[test]
fn unicode_dependency_location_survives_modern_dwg() {
    let mut doc = CadDocument::new();
    let mut dep = dependency();
    dep.file_name = "../參考/grond 🏠.dwg".into();
    doc.file_dependencies.push(dep);
    let read = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    assert_eq!(read.file_dependencies, doc.file_dependencies);
}

#[test]
fn empty_sparse_dependency_section_is_a_valid_empty_list() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1032] {
        let doc = CadDocument::with_version(version);
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert!(read.file_dependencies.is_empty());
    }
}

#[test]
fn older_dwg_has_no_dependency_list_and_still_saves() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1015);
    doc.file_dependencies.push(dependency());
    let read = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
        .read()
        .unwrap();
    assert!(read.file_dependencies.is_empty());
}

#[cfg(feature = "serde")]
#[test]
fn dependencies_are_part_of_the_serialized_document() {
    let mut doc = CadDocument::new();
    doc.file_dependencies.push(dependency());
    let bytes = serde_json::to_vec(&doc).unwrap();
    let read: CadDocument = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(read.file_dependencies, doc.file_dependencies);
}
