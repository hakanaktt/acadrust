//! Issue #247: AC1032 DWG with an ACAD surface object (type_code=-5) fails the strict read ("Invalid DWG array count")
//!
//! An AC1032 (AutoCAD 2018) drawing with an ACAD_SURFACE entity carrying an extra modeler body
//! fails in strict read with:
//! "Parse error: Invalid record at handle 0x..., type_code=-5: Parse error: Invalid DWG array count -1498865535: ... record bits remain"
//!
//! The reader was passing `allow_extra: false` to `read_acis_entity_impl` for all surfaces,
//! so the extra modeler body in an AcDs-backed (R2013+) surface record was not consumed before
//! reading materials and revision, causing `safe_count` on the materials count to read the
//! extra modeler body header as a negative array count.

use std::io::Cursor;

use acadrust::entities::acis::primitives::build_box;
use acadrust::entities::acis::SabWriter;
use acadrust::entities::solid3d::AcisData;
use acadrust::entities::surface::{Surface, SurfaceKind};
use acadrust::entities::EntityType;
use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::entities::read_surface;
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::types::{Color, DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn sab() -> Vec<u8> {
    SabWriter::write(&build_box([10.0, 20.0, -5.0], 2.0, 3.0, 4.0))
}

fn ac1032_surface_record_with_extra_body(bit_offset: i64) -> (DwgMergedReader, Vec<u8>, i64) {
    let dxf_version = DxfVersion::AC1032;
    let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
    let mut writer = DwgBitWriter::new(version, dxf_version);
    for _ in 0..bit_offset {
        writer.write_bit(true);
    }
    let geometry = sab();
    // In R2013+ AcDs layout, primary modeler geometry lives in AcDs,
    // so no inline primary body is written.
    writer.write_bit(true); // wireframe cache present
    writer.write_bit(true); // reference point present
    writer.write_3bit_double(Vector3::new(10.0, 20.0, -5.0));
    writer.write_bit_long(2); // isolines
    writer.write_bit(true); // wire list present
    writer.write_bit_long(1); // 1 wire
    writer.write_byte(2); // visible edge
    writer.write_bit_long(-1); // selection marker
    writer.write_bit_short(256); // color: ByLayer
    writer.write_bit_long(0x80FA); // ACIS index
    writer.write_bit_long(2); // point count
    writer.write_3bit_double(Vector3::ZERO);
    writer.write_3bit_double(Vector3::new(10.0, 20.0, -5.0));
    writer.write_bit(false); // wire transform
    writer.write_bit_long(0); // silhouettes: 0

    // Extra modeler body gate: false indicates extra body is present
    writer.write_bit(false);
    writer.write_bit(false); // unknown bit
    writer.write_bit_short(2); // extra_version = 2 (SAB)
    writer.write_bytes(&geometry);

    // Materials tail (R2007+)
    writer.write_bit_long(0); // 0 materials

    // Revision block (R2013+)
    writer.write_bit(true); // has_guid
    writer.write_bit_long(0x12345678);
    writer.write_bit_short(0x1234);
    writer.write_bit_short(0x5678);
    writer.write_bytes(&[1, 2, 3, 4, 5, 6, 7, 8]);
    writer.write_bit_long(0x87654321_u32 as i32);

    // Surface isolines
    writer.write_bit_short(17);
    writer.write_bit_short(29);

    let end = writer.position_in_bits();
    let main = DwgBitReader::new(writer.into_bytes(), version, dxf_version);
    let mut reader = DwgMergedReader::from_readers(main, None, None, dxf_version);
    reader.set_handle_start(end);
    reader.set_main_data_end(end);
    reader.set_position_in_bits(bit_offset);
    (reader, geometry, end)
}

#[test]
fn ac1032_surface_with_extra_modeler_body_decodes_without_array_count_error() {
    let (mut reader, geometry, end) = ac1032_surface_record_with_extra_body(3);
    let surface = read_surface(
        &mut reader,
        DwgVersion::AC24,
        DxfVersion::AC1032,
        true, // has_ds_data
        SurfaceKind::Generic,
    );
    reader.check_error().unwrap();
    assert_eq!(reader.position_in_bits(), end);
    assert_eq!(surface.u_isolines, 17);
    assert_eq!(surface.v_isolines, 29);
    assert_eq!(surface.acis.wires[0].color, Color::ByLayer);
    let extra = surface
        .acis
        .extra_acis_data
        .expect("extra ACIS data must be present");
    assert_eq!(extra.sab_data, geometry);
}

#[test]
fn ac1032_surface_document_roundtrip_with_extra_modeler_body() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut surface = Surface::new(SurfaceKind::Generic);
    surface.acis_data = AcisData::from_sab(sab());
    surface.acis_data.wireframe_data_present = true;
    surface.acis_data.extra_acis_data = Some(Box::new(AcisData::from_sab(sab())));
    surface.u_isolines = 6;
    surface.v_isolines = 8;
    let handle = document
        .add_entity(EntityType::Surface(Box::new(surface)))
        .unwrap();

    let bytes = DwgWriter::write_to_vec(&document).unwrap();
    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    let restored = reader
        .read()
        .expect("strict read must succeed for AC1032 surface with extra modeler body");
    let EntityType::Surface(restored_surface) = restored.get_entity(handle).unwrap() else {
        panic!("expected surface entity");
    };
    assert_eq!(restored_surface.u_isolines, 6);
    assert_eq!(restored_surface.v_isolines, 8);
    assert!(restored_surface.acis_data.extra_acis_data.is_some());
}
