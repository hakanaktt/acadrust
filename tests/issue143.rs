use std::collections::{HashMap, HashSet};
use std::io::Cursor;

use acadrust::entities::{Insert, Line};
use acadrust::io::dwg::dwg_stream_readers::object_reader::{tables, DwgObjectReader};
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::io::dwg::dwg_version::DwgVersion;
use acadrust::objects::ObjectType;
use acadrust::tables::BlockRecord;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, EntityType, Handle, Vector3};

fn drawing(version: DxfVersion) -> (CadDocument, Vec<(Handle, Handle)>) {
    let mut doc = CadDocument::with_version(version);
    doc.add_layout("Layout2").unwrap();
    doc.add_layout("Layout3").unwrap();
    let mut lines = Vec::new();
    for (index, name) in ["Layout1", "Layout2", "Layout3"].iter().enumerate() {
        let start = Vector3::new(index as f64 * 10.0, 1.0, 0.0);
        let handle = doc
            .add_entity_to_layout(
                EntityType::Line(Line::from_points(
                    start,
                    start + Vector3::new(2.0, 0.0, 0.0),
                )),
                name,
            )
            .unwrap();
        lines.push((
            handle,
            doc.get_entity(handle).unwrap().common().owner_handle,
        ));
    }
    for name in ["*U42", "*U99"] {
        let mut block = BlockRecord::new(name);
        block.handle = doc.allocate_handle();
        block.block_entity_handle = doc.allocate_handle();
        block.block_end_handle = doc.allocate_handle();
        block.flags.anonymous = true;
        doc.block_records.add(block).unwrap();
        doc.add_entity(EntityType::Insert(Insert::new(name, Vector3::ZERO)))
            .unwrap();
    }
    (doc, lines)
}

fn objects(doc: &CadDocument) -> DwgObjectReader {
    let (data, handles, _, _) = DwgObjectWriter::new(doc).unwrap().write();
    DwgObjectReader::new(
        data,
        doc.version,
        handles
            .into_iter()
            .map(|(h, p)| (h, i64::from(p)))
            .collect(),
    )
    .unwrap()
}

fn record_names(doc: &CadDocument, objects: &DwgObjectReader) -> HashMap<Handle, String> {
    doc.block_records
        .iter()
        .map(|block| {
            let offset = objects.offset_for(block.handle.value()).unwrap() as usize;
            let (code, mut reader) = objects.read_record_at(offset).unwrap();
            objects.read_common_non_entity_data(&mut reader, code);
            let data = tables::read_block_header(
                &mut reader,
                DwgVersion::from_dxf_version(doc.version).unwrap(),
            );
            assert!(reader.check_error().is_ok());
            (block.handle, data.name)
        })
        .collect()
}

fn marker_names(doc: &CadDocument, objects: &DwgObjectReader) -> HashMap<Handle, String> {
    doc.block_records
        .iter()
        .map(|block| {
            let (code, mut reader) = objects
                .read_record_at(
                    objects
                        .offset_for(block.block_entity_handle.value())
                        .unwrap() as usize,
                )
                .unwrap();
            objects.read_common_entity_data(&mut reader, code);
            let name = reader.read_variable_text();
            assert!(reader.check_error().is_ok());
            (block.handle, name)
        })
        .collect()
}

// Paper-space headers are stored bare; their BLOCK markers keep the numbered name.
fn assert_marker_names(
    doc: &CadDocument,
    objects: &DwgObjectReader,
    names: &HashMap<Handle, String>,
) -> HashMap<Handle, String> {
    let markers = marker_names(doc, objects);
    for block in doc.block_records.iter() {
        let header = &names[&block.handle];
        let marker = &markers[&block.handle];
        if header.eq_ignore_ascii_case("*Paper_Space") {
            assert!(
                marker.to_uppercase().starts_with("*PAPER_SPACE"),
                "{marker}"
            );
        } else {
            assert_eq!(marker, header);
        }
    }
    markers
}

#[test]
fn paper_space_names_and_anonymous_bases_are_consistent() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        for active_layout in [0, 2] {
            let (mut doc, lines) = drawing(version);
            let old_paper = doc.header.paper_space_block_handle;
            let active = lines[active_layout].1;
            let old_active_name = doc
                .block_records
                .iter()
                .find(|b| b.handle == active)
                .unwrap()
                .name
                .clone();
            doc.header.paper_space_block_handle = active;
            let objects = objects(&doc);
            let names = record_names(&doc, &objects);
            let markers = assert_marker_names(&doc, &objects, &names);
            let regular: Vec<_> = doc
                .block_records
                .iter()
                .filter(|block| !block.flags.anonymous)
                .collect();
            let unique: HashSet<_> = regular
                .iter()
                .map(|block| markers[&block.handle].to_uppercase())
                .collect();
            assert_eq!(
                unique.len(),
                regular.len(),
                "duplicate names in {version:?}"
            );
            assert_eq!(names[&active], "*Paper_Space");
            assert_eq!(markers[&active], "*Paper_Space");
            if active != old_paper {
                assert_eq!(names[&old_paper], "*Paper_Space");
                assert_eq!(markers[&old_paper], old_active_name);
            }
            for name in ["*U42", "*U99"] {
                let block = doc.block_records.get(name).unwrap();
                assert_eq!(names[&block.handle], "*U");
            }

            let (code, mut reader) = objects
                .read_record_at(
                    objects
                        .offset_for(doc.block_records.handle().value())
                        .unwrap() as usize,
                )
                .unwrap();
            objects.read_common_non_entity_data(&mut reader, code);
            let control = tables::read_block_control(&mut reader);
            assert_eq!(control.paper_space_handle, active.value());
            assert_eq!(
                control.model_space_handle,
                doc.header.model_space_block_handle.value()
            );
            assert!(!control.entry_handles.contains(&active.value()));
            assert_eq!(control.entry_count as usize + 2, names.len());
            if active != old_paper {
                assert!(control.entry_handles.contains(&old_paper.value()));
            }
        }
    }
}

fn roundtrip(doc: &CadDocument) -> CadDocument {
    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(doc).unwrap()))
        .read()
        .unwrap()
}

#[test]
fn changing_active_layout_preserves_cached_entity_owners_and_layout_links() {
    let (source, lines) = drawing(DxfVersion::AC1032);
    let mut loaded = roundtrip(&source);
    let old_paper = lines[0].1;
    let active = lines[2].1;
    assert_eq!(
        loaded.get_entity(lines[0].0).unwrap().common().entity_mode,
        Some(1)
    );
    loaded.header.paper_space_block_handle = active;
    let bytes = DwgWriter::write_to_vec(&loaded).unwrap();
    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    let info = reader.read_file_header().unwrap();
    let header_bytes = reader.get_section_buffer("AcDb:Header", &info).unwrap();
    let header = acadrust::io::dwg::dwg_stream_readers::header_reader::read_header(
        &header_bytes,
        loaded.version,
        info.acad_maintenance_version,
    )
    .unwrap();
    assert_eq!(header.paper_space_block_handle, active);
    for _ in 0..2 {
        let objects = objects(&loaded);
        let names = record_names(&loaded, &objects);
        assert_marker_names(&loaded, &objects, &names);
        loaded = roundtrip(&loaded);
        assert_eq!(loaded.header.paper_space_block_handle, active);
        assert_eq!(
            loaded.block_records.get("*Paper_Space").unwrap().handle,
            active
        );
        assert_eq!(
            loaded.block_records.get("*Paper_Space1").unwrap().handle,
            old_paper
        );
        for (handle, owner) in &lines {
            assert_eq!(
                loaded.get_entity(*handle).unwrap().common().owner_handle,
                *owner
            );
            let block = loaded
                .block_records
                .iter()
                .find(|b| b.handle == *owner)
                .unwrap();
            assert!(block.entity_handles.contains(handle));
            assert!(
                matches!(loaded.objects.get(&block.layout), Some(ObjectType::Layout(layout)) if layout.block_record == *owner)
            );
        }
        for original in ["*U42", "*U99"] {
            let original_handle = source.block_records.get(original).unwrap().handle;
            let block = loaded
                .block_records
                .iter()
                .find(|block| block.handle == original_handle)
                .unwrap();
            assert!(loaded.entities().any(
                |entity| matches!(entity, EntityType::Insert(insert) if insert.block_name == block.name)
            ));
        }
    }
}

#[test]
fn invalid_active_paper_handle_falls_back_to_a_real_unique_record() {
    let (mut doc, _) = drawing(DxfVersion::AC1032);
    let expected = doc.header.paper_space_block_handle;
    doc.header.paper_space_block_handle = Handle::new(0xdead);
    let loaded = roundtrip(&doc);
    assert_eq!(loaded.header.paper_space_block_handle, expected);
    let names: HashSet<_> = loaded
        .block_records
        .iter()
        .map(|b| b.name.to_uppercase())
        .collect();
    assert_eq!(names.len(), doc.block_records.len());
    assert_eq!(
        loaded.block_records.get("*Paper_Space").unwrap().handle,
        expected
    );
}
