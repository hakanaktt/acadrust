use std::collections::BTreeMap;
use std::io::Cursor;

use acadrust::entities::Line;
use acadrust::io::dwg::dwg_stream_readers::{
    handle_reader::read_handles,
    object_reader::{tables, DwgObjectReader},
};
use acadrust::io::dwg::dwg_version::DwgVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, EntityType, Handle, Vector3};

fn paper_names(doc: &CadDocument) -> BTreeMap<Handle, String> {
    doc.block_records
        .iter()
        .filter(|block| block.is_paper_space())
        .map(|block| (block.handle, block.name.clone()))
        .collect()
}

fn assert_stored_names(bytes: &[u8], doc: &CadDocument) {
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
    let offset = objects
        .offset_for(doc.block_records.handle().value())
        .unwrap();
    let (code, mut record) = objects.read_record_at(offset as usize).unwrap();
    objects.read_common_non_entity_data(&mut record, code);
    let control = tables::read_block_control(&mut record);
    record.check_error().unwrap();
    assert_eq!(
        control.paper_space_handle,
        doc.header.paper_space_block_handle.value()
    );
    assert_eq!(
        control.model_space_handle,
        doc.header.model_space_block_handle.value()
    );
    for block in doc
        .block_records
        .iter()
        .filter(|block| block.is_paper_space())
    {
        let offset = objects.offset_for(block.handle.value()).unwrap();
        let (code, mut record) = objects.read_record_at(offset as usize).unwrap();
        objects.read_common_non_entity_data(&mut record, code);
        let header = tables::read_block_header(
            &mut record,
            DwgVersion::from_dxf_version(doc.version).unwrap(),
        );
        record.check_error().unwrap();
        assert_eq!(
            header.name, "*Paper_Space",
            "stored BLOCK_HEADER for {}",
            block.name
        );
        assert!(!header.anonymous, "paper space is not an anonymous block");
        let offset = objects.offset_for(header.block_entity_handle).unwrap();
        let (code, mut marker) = objects.read_record_at(offset as usize).unwrap();
        objects.read_common_entity_data(&mut marker, code);
        assert_eq!(
            marker.read_variable_text(),
            block.name,
            "BLOCK keeps the layout suffix"
        );
        marker.check_error().unwrap();
        assert_eq!(
            control.entry_handles.contains(&block.handle.value()),
            block.handle != doc.header.paper_space_block_handle,
        );
    }
}

#[test]
fn paper_space_headers_are_bare_and_roundtrips_preserve_layout_names() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut doc = CadDocument::with_version(version);
        for name in ["Layout2", "P3", "P4", "P5"] {
            doc.add_layout(name).unwrap();
        }
        // Non-sequential suffixes and a gap must survive independently of control order.
        doc.block_records
            .rename("*Paper_Space0", "*Paper_Space17")
            .unwrap();
        doc.block_records
            .rename("*Paper_Space3", "*Paper_Space0")
            .unwrap();
        for (index, name) in ["Layout1", "Layout2", "P3", "P4", "P5"].iter().enumerate() {
            doc.add_entity_to_layout(
                EntityType::Line(Line::from_points(
                    Vector3::new(index as f64, 0.0, 0.0),
                    Vector3::new(index as f64, 1.0, 0.0),
                )),
                name,
            )
            .unwrap();
        }
        let names = paper_names(&doc);
        let active = doc.header.paper_space_block_handle;
        let owners: Vec<_> = doc
            .entities()
            .filter(|entity| matches!(entity, EntityType::Line(_)))
            .map(|entity| (entity.common().handle, entity.common().owner_handle))
            .collect();
        assert_eq!(owners.len(), 5);
        for _ in 0..2 {
            let bytes = DwgWriter::write_to_vec(&doc).unwrap();
            assert_stored_names(&bytes, &doc);
            doc = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
            assert_eq!(
                paper_names(&doc),
                names,
                "layout names after a {version:?} round trip"
            );
            assert_eq!(doc.header.paper_space_block_handle, active);
            assert_eq!(
                doc.block_records.get("*Paper_Space").unwrap().handle,
                active
            );
            for (handle, owner) in &owners {
                assert_eq!(
                    doc.get_entity(*handle).unwrap().common().owner_handle,
                    *owner
                );
            }
        }
    }
}
