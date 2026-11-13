//! An anonymous block definition keeps its number on the BLOCK marker in DWG.
//!
//! AutoCAD stores an anonymous BLOCK_HEADER under the bare name (`*U`, `*D`)
//! and numbers it again on open, but the BLOCK marker carries the full name:
//! `anonymous-names.dwg` (AutoCAD 2025) has the headers `*U` and the markers
//! `*U2`, `*U4` and `*U5`. The writer stored the bare name on the marker as
//! well, so every anonymous definition of one kind went out under one marker
//! name. This crate's reader numbers bare records from BLOCK_CONTROL and does
//! not notice; a reader that names a definition after its marker sees them all
//! under one name.
use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;

use acadrust::entities::{Dimension, DimensionAligned, Insert, Line};
use acadrust::io::dwg::dwg_stream_readers::{
    handle_reader::read_handles,
    object_reader::{tables, DwgObjectReader},
};
use acadrust::io::dwg::dwg_version::DwgVersion;
use acadrust::tables::BlockRecord;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, EntityType, Vector3};

const VERSIONS: [DxfVersion; 6] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// What a DWG stores for each block record, by record handle: the
/// BLOCK_HEADER name, its anonymous flag and the name on its BLOCK marker.
fn stored_names(bytes: &[u8]) -> BTreeMap<u64, (String, bool, String)> {
    let doc = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    let info = reader.read_file_header().unwrap();
    let mut handles =
        read_handles(&reader.get_section_buffer("AcDb:Handles", &info).unwrap()).unwrap();
    for offset in handles.values_mut() {
        *offset -= info.objects_base_offset;
    }
    let objects = DwgObjectReader::new(
        reader
            .get_section_buffer("AcDb:AcDbObjects", &info)
            .unwrap(),
        doc.version,
        handles,
    )
    .unwrap();
    doc.block_records
        .iter()
        .map(|block| {
            let offset = objects.offset_for(block.handle.value()).unwrap();
            let (code, mut record) = objects.read_record_at(offset as usize).unwrap();
            objects.read_common_non_entity_data(&mut record, code);
            let header = tables::read_block_header(
                &mut record,
                DwgVersion::from_dxf_version(doc.version).unwrap(),
            );
            record.check_error().unwrap();
            let offset = objects.offset_for(header.block_entity_handle).unwrap();
            let (code, mut marker) = objects.read_record_at(offset as usize).unwrap();
            objects.read_common_entity_data(&mut marker, code);
            let marker_name = marker.read_variable_text();
            marker.check_error().unwrap();
            (
                block.handle.value(),
                (header.name, header.anonymous, marker_name),
            )
        })
        .collect()
}

/// `(header, anonymous, marker)` as AutoCAD stores an anonymous definition.
fn anonymous(base: &str, name: &str) -> (String, bool, String) {
    (base.to_string(), true, name.to_string())
}

/// The fixture written by AutoCAD keeps its marker numbers when it is saved
/// again, in every version.
#[test]
fn resaved_autocad_drawing_keeps_its_anonymous_markers() {
    let bytes = include_bytes!("anonymous-names.dwg");
    let definitions = [(0x7f, "*U2"), (0x8d, "*U4"), (0x94, "*U5")];
    let stored = stored_names(bytes);
    for (handle, name) in definitions {
        assert_eq!(
            stored[&handle],
            anonymous("*U", name),
            "AutoCAD's record {handle:X}"
        );
    }
    let mut doc = DwgReader::from_stream(Cursor::new(bytes.as_slice()))
        .read()
        .unwrap();
    for version in VERSIONS {
        doc.version = version;
        let stored = stored_names(&DwgWriter::write_to_vec(&doc).unwrap());
        for (handle, name) in definitions {
            assert_eq!(
                stored[&handle],
                anonymous("*U", name),
                "{version:?}: record {handle:X}"
            );
        }
    }
}

const NAMES: [&str; 5] = ["*D1", "*D2", "*D3", "*U4", "*U5"];

/// Three dimension blocks and two anonymous insert blocks, each holding a
/// line of its own, referenced from model space.
fn drawing(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    for (index, name) in NAMES.iter().enumerate() {
        let mut record = BlockRecord::new(*name);
        record.handle = doc.allocate_handle();
        record.block_entity_handle = doc.allocate_handle();
        record.block_end_handle = doc.allocate_handle();
        record.flags.anonymous = true;
        let block = record.handle;
        doc.block_records.add(record).unwrap();
        let start = Vector3::new(index as f64 * 10.0, 0.0, 0.0);
        let end = start + Vector3::new(5.0, 0.0, 0.0);
        let mut line = Line::from_points(start, end);
        line.common.owner_handle = block;
        doc.add_entity(EntityType::Line(line)).unwrap();
        let reference = if name.starts_with("*D") {
            let mut dimension = DimensionAligned::new(start, end);
            dimension.base.block_name = name.to_string();
            EntityType::Dimension(Box::new(Dimension::Aligned(dimension)))
        } else {
            EntityType::Insert(Insert::new(*name, Vector3::ZERO))
        };
        doc.add_entity(reference).unwrap();
    }
    doc
}

/// Several anonymous definitions of one kind come out of the writer under
/// distinct marker names, each the definition's own.
#[test]
fn anonymous_definitions_of_one_kind_keep_distinct_markers() {
    for version in VERSIONS {
        let doc = drawing(version);
        let stored = stored_names(&DwgWriter::write_to_vec(&doc).unwrap());
        let mut markers = HashSet::new();
        for name in NAMES {
            let block = doc.block_records.get(name).unwrap();
            assert_eq!(
                stored[&block.handle.value()],
                anonymous(&name[..2], name),
                "{version:?}: {name}"
            );
            markers.insert(stored[&block.handle.value()].2.clone());
        }
        assert_eq!(
            markers.len(),
            NAMES.len(),
            "{version:?}: one marker name per definition"
        );
    }
}
