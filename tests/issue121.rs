use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::io::dwg::dwg_document_builder::DwgDocumentBuilder;
use acadrust::io::dwg::dwg_stream_readers::object_reader::tables::read_block_header;
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn layout_memberships(document: &CadDocument) -> BTreeMap<String, (Handle, Vec<Handle>)> {
    document
        .objects
        .values()
        .filter_map(|object| match object {
            ObjectType::Layout(layout) => {
                let block = document
                    .block_records
                    .iter()
                    .find(|block| block.handle == layout.block_record)
                    .unwrap();
                for handle in &block.entity_handles {
                    let entity = document.get_entity(*handle).unwrap();
                    assert_eq!(entity.common().owner_handle, block.handle);
                }
                Some((
                    layout.name.clone(),
                    (block.handle, block.entity_handles.clone()),
                ))
            }
            _ => None,
        })
        .collect()
}

fn source_document(version: DxfVersion) -> CadDocument {
    let mut document = CadDocument::with_version(version);
    document.add_layout("Layout2").unwrap();
    document.add_layout("Layout3").unwrap();
    let empty = document.add_layout("Empty").unwrap();
    let ObjectType::Layout(layout) = document.objects.get_mut(&empty).unwrap() else {
        unreachable!();
    };
    let viewport = layout.viewport;
    let owner = layout.block_record;
    layout.viewport = Handle::NULL;
    layout.viewports.clear();
    document.remove_entity(viewport).unwrap();
    document
        .block_records
        .iter_mut()
        .find(|block| block.handle == owner)
        .unwrap()
        .entity_handles
        .clear();

    // Interleave allocations between layouts and use nonconsecutive handles
    // whose numeric order differs from their block's linked-list order.
    for (layout, handle) in [
        ("Layout2", 0x3000),
        ("Layout3", 0x5000),
        ("Layout2", 0x1000),
        ("Layout1", 0x7000),
        ("Layout3", 0x4000),
        ("Layout2", 0x8000),
        ("Layout2", 0x8001),
        ("Layout2", 0x8002),
        ("Layout2", 0x8003),
    ] {
        let mut line = Line::from_coords(handle as f64, 0.0, 0.0, handle as f64, 1.0, 0.0);
        line.common.handle = Handle::new(handle);
        document
            .add_entity_to_layout(EntityType::Line(line), layout)
            .unwrap();
    }
    let mut model = Line::from_coords(0.0, 0.0, 0.0, 2.0, 2.0, 0.0);
    model.common.handle = Handle::new(0x2000);
    document.add_entity(EntityType::Line(model)).unwrap();
    document
}

#[test]
fn r2000_preserves_each_layouts_ownership_and_order_across_repeated_roundtrips() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1018] {
        let mut document = source_document(version);
        let expected = layout_memberships(&document);
        assert!(expected["Empty"].1.is_empty());
        assert_eq!(expected["Layout1"].1.len(), 1);
        assert_eq!(expected["Layout2"].1.len(), 7);
        assert_eq!(expected["Layout3"].1.len(), 3);
        for _ in 0..2 {
            let bytes = DwgWriter::write_to_vec(&document).unwrap();
            document = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
            assert_eq!(layout_memberships(&document), expected, "{version:?}");
        }
    }
}

#[test]
fn r2000_visitor_filtering_preserves_layout_ownership_and_chain_order() {
    let source = source_document(DxfVersion::AC1015);
    let bytes = DwgWriter::write_to_vec(&source).unwrap();
    for dropped_handle in [Handle::new(0x1000), Handle::new(0x8001)] {
        let mut expected = layout_memberships(&source);
        expected
            .get_mut("Layout2")
            .unwrap()
            .1
            .retain(|handle| *handle != dropped_handle);
        let mut dropped = 0;
        let read = DwgReader::from_stream(Cursor::new(bytes.clone()))
            .read_visiting(|_, entity| {
                if entity.common().handle == dropped_handle {
                    dropped += 1;
                    None
                } else {
                    Some(entity)
                }
            })
            .unwrap();
        assert_eq!(dropped, 1);
        assert!(read.get_entity(dropped_handle).is_none());
        assert_eq!(layout_memberships(&read), expected);
    }
}

#[test]
fn r2000_block_chain_order_is_independent_of_record_offsets_and_numeric_handles() {
    let source = source_document(DxfVersion::AC1015);
    let expected = layout_memberships(&source);
    let (bytes, mut records, _, _) = DwgObjectWriter::new(&source).unwrap().write();
    records.sort_unstable_by_key(|(_, offset)| *offset);
    let mut reordered = Vec::with_capacity(bytes.len());
    let mut offsets = HashMap::new();
    // Relocate whole records without changing their handle links or payloads.
    for index in (0..records.len()).rev() {
        let (handle, start) = records[index];
        let end = records
            .get(index + 1)
            .map(|(_, offset)| *offset as usize)
            .unwrap_or(bytes.len());
        offsets.insert(handle, reordered.len() as i64);
        reordered.extend_from_slice(&bytes[start as usize..end]);
    }
    let objects = DwgObjectReader::new(reordered, source.version, offsets).unwrap();
    let mut read = CadDocument::with_version(source.version);
    let outcome = DwgDocumentBuilder::new(objects).build_with_stats(&mut read);
    assert_eq!(outcome.skipped_records, 0);
    assert_eq!(layout_memberships(&read), expected);
}

#[test]
fn r2000_block_headers_retain_empty_singleton_and_multi_entity_chain_endpoints() {
    let source = source_document(DxfVersion::AC1015);
    let (bytes, records, _, _) = DwgObjectWriter::new(&source).unwrap().write();
    let offsets = records
        .into_iter()
        .map(|(handle, offset)| (handle, offset as i64))
        .collect();
    let objects = DwgObjectReader::new(bytes, source.version, offsets).unwrap();
    for block in source.block_records.iter() {
        let offset = objects.offset_for(block.handle.value()).unwrap() as usize;
        let (type_code, mut reader) = objects.read_record_at(offset).unwrap();
        objects.read_common_non_entity_data(&mut reader, type_code);
        let data = read_block_header(&mut reader, objects.version());
        assert_eq!(
            data.first_entity_handle,
            Some(
                block
                    .entity_handles
                    .first()
                    .map_or(0, |handle| handle.value())
            )
        );
        assert_eq!(
            data.last_entity_handle,
            Some(
                block
                    .entity_handles
                    .last()
                    .map_or(0, |handle| handle.value())
            )
        );
        assert!(data.entity_handles.is_empty());
    }
    for (handle, expected_next) in [(0x3000, Some(0x1000)), (0x8001, None), (0x8002, None)] {
        let offset = objects.offset_for(handle).unwrap() as usize;
        let (type_code, mut reader) = objects.read_record_at(offset).unwrap();
        let data = objects.read_common_entity_data(&mut reader, type_code);
        assert_eq!(data.next_entity_handle, expected_next);
    }
}
