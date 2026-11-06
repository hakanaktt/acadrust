//! An SPDS format entity (#320; `ExtendedEntityData::Format`, C++ class
//! `mcsDbObjectFormat`) is written to DWG only from compatible raw data.
//!
//! The vendor schema is not public: acadrust keeps the record's raw bytes and
//! nothing else. On a save to another version, or from DXF, the writer used to
//! emit an empty record under a class code, which ODA refuses and acadrust's
//! own reader can't read back. Custom and Arch records with no compatible raw
//! data are refused with a clear error instead; a format entity follows the
//! same contract.

use acadrust::{
    classes::DxfClass,
    entities::{EntityType, ExtendedEntity, ExtendedEntityData, FormatData},
    types::DxfVersion,
    CadDocument, DwgReader, DwgWriter,
};
use std::io::Cursor;

/// A drawing of `version` whose class table declares the format class the way
/// nanoCAD does (`SPDSFORMAT`), with one format entity.
fn document(version: DxfVersion, data: FormatData) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let classes: Vec<DxfClass> = doc
        .classes
        .iter()
        .filter(|class| class.cpp_class_name != "mcsDbObjectFormat")
        .cloned()
        .collect();
    doc.classes.clear();
    for class in classes {
        doc.classes.add_or_update(class);
    }
    doc.classes
        .add_or_update(DxfClass::new_entity("SPDSFORMAT", "mcsDbObjectFormat"));
    doc.add_entity(EntityType::Extended(Box::new(ExtendedEntity {
        common: Default::default(),
        data: ExtendedEntityData::Format(data),
    })))
    .unwrap();
    doc.version = version;
    doc
}

fn raw(version: DxfVersion) -> FormatData {
    FormatData {
        raw_dwg_data: Some(vec![0x5A; 16]),
        raw_dwg_version: Some(version),
        ..Default::default()
    }
}

fn assert_refused(doc: &CadDocument, case: &str) {
    match DwgWriter::write_to_vec(doc) {
        Err(error) => assert!(
            error
                .to_string()
                .contains("compatible raw DWG data is required"),
            "{case}: {error}"
        ),
        Ok(bytes) => {
            let read = DwgReader::from_stream(Cursor::new(bytes)).read();
            panic!(
                "{case}: written without its data; reading it back gives {:?}",
                read.map(|_| "a drawing").map_err(|error| error.to_string())
            );
        }
    }
}

#[test]
fn format_encoded_for_another_version_is_refused() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018, DxfVersion::AC1027] {
        let doc = document(version, raw(DxfVersion::AC1032));
        assert_refused(&doc, &format!("{version:?}"));
    }
}

#[test]
fn format_without_dwg_data_is_refused() {
    // Read from DXF: only the group codes are kept.
    let data = FormatData {
        raw_dxf_codes: Some(vec![(100, "mcsDbObjectFormat".to_string())]),
        ..Default::default()
    };
    let doc = document(DxfVersion::AC1032, data);
    assert_refused(&doc, "DXF source");
}

#[test]
fn format_in_its_own_version_is_not_refused() {
    // Raw data of the target version is copied as it is (the bytes here are a
    // stand-in: a real SPDS record needs nanoCAD to produce).
    let doc = document(DxfVersion::AC1032, raw(DxfVersion::AC1032));
    DwgWriter::write_to_vec(&doc).unwrap();
}
