use std::io::Cursor;

use acadrust::io::dwg::dwg_stream_readers::{
    handle_reader::read_handles, merged_reader::DwgMergedReader, object_reader::DwgObjectReader,
};
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::dwg_version::DwgVersion;
use acadrust::objects::{
    ClassObject, ClassObjectData, DataTable, DataTableCellType, DataTableColumn, DataTableValue,
    LayerFilter, ObjectType,
};
use acadrust::tables::Layer;
use acadrust::types::DxfVersion;
use acadrust::types::Vector3;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};

fn table(document: &CadDocument) -> &DataTable {
    document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::ClassObject(ClassObject {
                data: ClassObjectData::DataTable(table),
                ..
            }) => Some(table),
            _ => None,
        })
        .expect("DATATABLE must remain a native object")
}

fn native_document(dwg: bool) -> CadDocument {
    if dwg {
        DwgReader::from_stream(Cursor::new(include_bytes!("datatable/point_object_id.dwg")))
            .read()
            .unwrap()
    } else {
        DxfReader::from_reader(Cursor::new(include_bytes!("datatable/point_object_id.dxf")))
            .unwrap()
            .read()
            .unwrap()
    }
}

fn table_handle(document: &CadDocument) -> Handle {
    *document
        .objects
        .iter()
        .find(|(_, object)| {
            matches!(
                object,
                ObjectType::ClassObject(ClassObject {
                    data: ClassObjectData::DataTable(_),
                    ..
                })
            )
        })
        .unwrap()
        .0
}

fn table_record(bytes: &[u8], handle: Handle) -> DwgMergedReader {
    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    let header = reader.read_file_header().unwrap();
    let handles =
        read_handles(&reader.get_section_buffer("AcDb:Handles", &header).unwrap()).unwrap();
    let offset = handles[&handle.value()];
    let objects = DwgObjectReader::new(
        reader
            .get_section_buffer("AcDb:AcDbObjects", &header)
            .unwrap(),
        DxfVersion::AC1032,
        handles,
    )
    .unwrap();
    let (code, mut record) = objects.read_record_at(offset as usize).unwrap();
    objects.read_common_non_entity_data(&mut record, code);
    record
}

fn read_table_prefix(record: &mut DwgMergedReader) {
    assert_eq!(record.read_bit_short(), 2);
    assert_eq!(record.read_bit_long(), 2);
    assert_eq!(record.read_bit_long(), 1);
    assert_eq!(record.read_variable_text(), "");
}

#[test]
fn reads_native_autocad_datatable_in_both_formats() {
    for dwg in [false, true] {
        let document = native_document(dwg);
        let table = table(&document);
        assert_eq!(table.row_count, 1);
        assert_eq!(table.columns.len(), 2);
        assert_eq!(table.columns[0].cell_type(), Some(DataTableCellType::Point));
        assert_eq!(table.columns[0].rows[0].point, Vector3::new(1.5, 2.5, 3.5));
        assert_eq!(
            table.columns[1].cell_type(),
            Some(DataTableCellType::ObjectId)
        );
        assert_eq!(table.columns[1].rows[0].handle, document.layers.handle());
        assert_eq!(table.columns[1].rows[0].handle, Handle::new(2));
    }
}

#[test]
fn edited_native_datatable_survives_dwg_and_both_dxf_encodings() {
    let mut document = native_document(true);
    for object in document.objects.values_mut() {
        if let ObjectType::ClassObject(ClassObject {
            data: ClassObjectData::DataTable(table),
            ..
        }) = object
        {
            table.columns[0].rows[0].point = Vector3::new(-8.25, 0.0, 12.5);
            table.columns[1].rows[0].handle = document.text_styles.handle();
        }
    }
    let expected = table(&document).clone();
    let dwg = DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&document).unwrap()))
        .read()
        .unwrap();
    assert_eq!(table(&dwg), &expected);
    for writer in [DxfWriter::new(&document), DxfWriter::new_binary(&document)] {
        let dxf = DxfReader::from_reader(Cursor::new(writer.write_to_vec().unwrap()))
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(table(&dxf), &expected);
    }
}

#[test]
fn dwg_writer_uses_native_point_and_soft_pointer_encodings() {
    let document = native_document(true);
    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let mut record = table_record(&bytes, table_handle(&document));
    read_table_prefix(&mut record);
    assert_eq!(record.read_bit_long(), DataTableCellType::Point as i32);
    assert_eq!(record.read_variable_text(), "Point");
    assert_eq!(record.read_3bit_double(), Vector3::new(1.5, 2.5, 3.5));
    assert_eq!(record.read_bit_long(), DataTableCellType::ObjectId as i32);
    assert_eq!(record.read_variable_text(), "ObjectId");
    assert_eq!(
        record.read_typed_handle(),
        (2, acadrust::io::dwg::DwgReferenceType::SoftPointer)
    );
}

#[test]
fn object_id_cells_follow_handle_collision_repairs() {
    let mut document = CadDocument::new();
    let old_handle = Handle::new(0x1234);
    let mut layer = Layer::new("COLLISION");
    layer.handle = old_handle;
    document.layers.add(layer).unwrap();
    let mut target = ClassObject::new(ClassObjectData::LayerFilter(LayerFilter {
        names: vec!["target".into()],
    }));
    target.handle = old_handle;
    document
        .objects
        .insert(old_handle, ObjectType::ClassObject(target));
    let mut object = ClassObject::new(ClassObjectData::DataTable(DataTable {
        row_count: 1,
        columns: (5..=9)
            .map(|value_type| DataTableColumn {
                value_type,
                name: format!("Reference {value_type}"),
                rows: vec![DataTableValue {
                    handle: old_handle,
                    ..Default::default()
                }],
            })
            .collect(),
        ..Default::default()
    }));
    object.handle = Handle::new(0x1235);
    document
        .objects
        .insert(object.handle, ObjectType::ClassObject(object));

    document.resolve_references();
    let repaired = document
        .objects
        .iter()
        .find_map(|(handle, object)| match object {
            ObjectType::ClassObject(ClassObject {
                data: ClassObjectData::LayerFilter(_),
                ..
            }) => Some(*handle),
            _ => None,
        })
        .unwrap();
    assert_ne!(repaired, old_handle);
    for column in &table(&document).columns {
        assert_eq!(column.rows[0].handle, repaired);
    }
}

#[test]
fn writers_reject_unsupported_cells_and_inconsistent_row_counts() {
    for (value_type, row_count) in [(6, 1), (99, 1), (1, 2), (1, -1)] {
        let mut document = CadDocument::new();
        let mut object = ClassObject::new(ClassObjectData::DataTable(DataTable {
            row_count,
            columns: vec![DataTableColumn {
                value_type,
                name: "Invalid".into(),
                rows: vec![DataTableValue::default()],
            }],
            ..Default::default()
        }));
        object.handle = document.allocate_handle();
        document
            .objects
            .insert(object.handle, ObjectType::ClassObject(object));
        assert!(DwgWriter::write_to_vec(&document).is_err());
        assert!(DxfWriter::new(&document).write_to_vec().is_err());
    }
}

#[test]
fn unsupported_dwg_cell_types_preserve_the_complete_record() {
    for cell_type in [6, 99] {
        let mut document = native_document(true);
        let handle = table_handle(&document);
        let mut record = table_record(include_bytes!("datatable/point_object_id.dwg"), handle);
        read_table_prefix(&mut record);
        let type_position = record.position_in_bits();
        assert_eq!(record.read_bit_long(), 4);
        let old_end = record.position_in_bits();
        let raw = record.raw_merged_data();
        let mut patch = DwgBitWriter::new(DwgVersion::AC24, DxfVersion::AC1032);
        patch.write_bytes(&raw);
        patch.set_position_in_bits(type_position);
        patch.write_bit_long(cell_type);
        assert_eq!(
            patch.position_in_bits(),
            old_end,
            "replacement must preserve framing"
        );
        patch.set_position_in_bits(raw.len() as i64 * 8);
        let raw = patch.into_bytes();
        document.objects.insert(
            handle,
            ObjectType::Unknown {
                type_name: "DATATABLE".into(),
                handle,
                owner: document.header.named_objects_dict_handle,
                raw_dxf_codes: None,
                raw_dwg_data: Some(raw.clone()),
                raw_dwg_handle_bits: record.get_handle_bits(),
                raw_dwg_version: Some(DxfVersion::AC1032),
            },
        );
        for _ in 0..2 {
            let bytes = DwgWriter::write_to_vec(&document).unwrap();
            document = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
            let ObjectType::Unknown {
                raw_dwg_data: Some(saved),
                ..
            } = &document.objects[&handle]
            else {
                panic!("unsupported table must remain opaque");
            };
            assert_eq!(saved, &raw);
            let ObjectType::Dictionary(root) =
                &document.objects[&document.header.named_objects_dict_handle]
            else {
                panic!("missing root dictionary");
            };
            assert_eq!(root.get("Review93DATATABLE"), Some(handle));
        }
    }
}
