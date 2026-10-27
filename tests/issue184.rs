//! Issue #184: `VBA_PROJECT`'s `BL` is the size of the `AcDb:VBAProject`
//! section payload, not the number of bytes stored inline in the object
//! record. A strict read must accept the small record that only carries that
//! size instead of treating it as an array count that exceeds the record.

use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::class_object::read_class_object_data;
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::dwg_version::DwgVersion;
use acadrust::objects::{ClassObjectData, VbaProject};
use acadrust::types::DxfVersion;

const DXF_VERSION: DxfVersion = DxfVersion::AC1032;
const DWG_VERSION: DwgVersion = DwgVersion::AC24;

fn main_reader(build: impl FnOnce(&mut DwgBitWriter)) -> DwgMergedReader {
    let mut writer = DwgBitWriter::new(DWG_VERSION, DXF_VERSION);
    build(&mut writer);
    let main = DwgBitReader::new(writer.into_bytes(), DWG_VERSION, DXF_VERSION);
    DwgMergedReader::from_readers(main, None, None, DXF_VERSION)
}

fn read_vba(reader: &mut DwgMergedReader) -> VbaProject {
    match read_class_object_data(reader, "VBA_PROJECT", DWG_VERSION, DXF_VERSION) {
        Some(ClassObjectData::VbaProject(project)) => project,
        other => panic!("expected VBA_PROJECT, got {other:?}"),
    }
}

#[test]
fn section_sized_bl_does_not_invalidate_the_record() {
    // LibreDWG reports data_size 7680 with only 5 bits left in the record.
    let mut reader = main_reader(|writer| {
        writer.write_bit_long(7680);
        for _ in 0..5 {
            writer.write_bit(false);
        }
    });

    let project = read_vba(&mut reader);

    reader
        .check_error()
        .expect("the BL is a section size, not an inline array count");
    assert!(
        project.storage.encode().is_empty(),
        "there are no inline bytes to store"
    );
}

#[test]
fn inline_payload_that_fits_the_record_is_still_read() {
    let payload = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    let mut reader = main_reader(|writer| {
        writer.write_bit_long(payload.len() as i32);
        writer.write_bytes(&payload);
    });

    let project = read_vba(&mut reader);

    reader.check_error().expect("valid inline VBA payload");
    assert_eq!(project.storage.encode(), payload);
}

#[test]
fn section_size_is_checked_against_main_bits_only() {
    // The text and handle streams have plenty of bits, but they are not part
    // of the object's inline main data and must not be consumed as payload.
    let mut writer = DwgBitWriter::new(DWG_VERSION, DXF_VERSION);
    writer.write_bit_long(8);
    for _ in 0..5 {
        writer.write_bit(false);
    }
    let main_bits = writer.position_in_bits();
    let main = DwgBitReader::new(writer.into_bytes(), DWG_VERSION, DXF_VERSION);
    let text = DwgBitReader::new(vec![0; 16], DWG_VERSION, DXF_VERSION);
    let handle = DwgBitReader::new(vec![0; 16], DWG_VERSION, DXF_VERSION);
    let mut reader = DwgMergedReader::from_readers(main, Some(text), Some(handle), DXF_VERSION);
    reader.set_main_data_end(main_bits);

    let project = read_vba(&mut reader);

    reader
        .check_error()
        .expect("text and handle bits are not inline payload");
    assert!(project.storage.encode().is_empty());
}
