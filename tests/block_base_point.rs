use acadrust::tables::TableEntry;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::io::Cursor;

#[test]
fn reserved_space_markers_roundtrip_without_entity_index_entries() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1021, DxfVersion::AC1032] {
        let document = CadDocument::with_version(version);
        for name in ["*Model_Space", "*Paper_Space"] {
            let block = document.block_records.get(name).unwrap();
            assert!(!block.block_entity_handle.is_null());
            assert!(!block.block_end_handle.is_null());
            assert!(document.get_entity(block.block_entity_handle).is_none());
            assert!(document.get_entity(block.block_end_handle).is_none());
        }
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let result = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        for name in ["*Model_Space", "*Paper_Space"] {
            let expected = document.block_records.get(name).unwrap();
            let actual = result.block_records.get(name).unwrap();
            assert_eq!(actual.block_entity_handle, expected.block_entity_handle);
            assert_eq!(actual.block_end_handle, expected.block_end_handle);
            let begin = result.get_entity(actual.block_entity_handle).unwrap();
            let end = result.get_entity(actual.block_end_handle).unwrap();
            assert!(matches!(begin, acadrust::entities::EntityType::Block(_)));
            assert!(matches!(end, acadrust::entities::EntityType::BlockEnd(_)));
            assert_eq!(begin.common().owner_handle, actual.handle);
            assert_eq!(end.common().owner_handle, actual.handle);
        }
    }
}

#[test]
fn writes_block_record_base_point_without_a_block_marker() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut document = CadDocument::with_version(version);
        let mut record = acadrust::tables::BlockRecord::new("Desk");
        record.set_handle(document.allocate_handle());
        record.block_end_handle = document.allocate_handle();
        record.base_point = Vector3::new(50.0, 25.0, 0.0);
        document.block_records.add(record).unwrap();

        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let mut reader = DwgReader::from_stream(Cursor::new(bytes));
        let roundtripped = reader.read().unwrap();
        assert_eq!(
            roundtripped.block_records.get("Desk").unwrap().base_point,
            Vector3::new(50.0, 25.0, 0.0),
            "failed for {version:?}"
        );
    }
}
