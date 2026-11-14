//! The common header of a DGN line-style component (`AcDbLSComponent`) in the
//! order the file stores it (#372): BL version, BL component type, the name,
//! RC[16] UID, BD scale, RC flags. The name is in the string stream from R2007
//! on and a TV after the component type in R2004 and older, where it was read
//! as the first field.

use acadrust::io::dwg::dwg_reference_type::DwgReferenceType;
use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::dgn_linestyle::read_dgn_line_style_data;
use acadrust::io::dwg::dwg_stream_writers::DwgMergedWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::objects::{
    DgnLineStyleData, DgnLsComponentData, DgnLsComponentType, DgnLsSymbolComponent,
};
use acadrust::types::{DxfVersion, Handle};

const UID: [u8; 16] = [
    0x91, 0x0e, 0x5c, 0x23, 0x6b, 0xd4, 0x41, 0x7a, 0xb2, 0x38, 0xe7, 0x0f, 0x54, 0xc9, 0x1d, 0x86,
];
const NAME: &str = "Rail - Symbol";
const BLOCK: u64 = 0x4c;

const VERSIONS: [DxfVersion; 5] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// A symbol component record body as the file stores it. Returns a reader
/// over it and the bit where the main data ends.
fn symbol_component_bits(dxf_version: DxfVersion) -> (DwgMergedReader, i64) {
    let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
    let mut writer = DwgMergedWriter::new(version, dxf_version);
    writer.write_bit_long(1); // version
    writer.write_bit_long(1); // symbol component
    writer.write_variable_text(NAME); // a TV here in R2004 and older
    writer.write_bytes(&UID);
    writer.write_bit_double(2.0); // scale
    writer.write_bytes(&[3]); // flags
    writer.write_bit_double(0.5); // stored unit scale
    writer.write_bit_double(4.0); // unit scale
    writer.write_bit(true); // has unit scale
    writer.write_bit(false); // is 3D
    writer.write_handle(DwgReferenceType::HardPointer, BLOCK);
    let end = writer.main().position_in_bits();
    let split_text = version.r2007_plus();
    let mut reader = DwgMergedReader::from_readers(
        DwgBitReader::new(writer.main().to_bytes_snapshot(), version, dxf_version),
        split_text
            .then(|| DwgBitReader::new(writer.text().to_bytes_snapshot(), version, dxf_version)),
        Some(DwgBitReader::new(
            writer.handle_writer().to_bytes_snapshot(),
            version,
            dxf_version,
        )),
        dxf_version,
    );
    reader.set_main_data_end(end);
    (reader, end)
}

#[test]
fn a_component_reads_its_name_after_its_type() {
    for dxf_version in VERSIONS {
        let (mut reader, end) = symbol_component_bits(dxf_version);
        let data = read_dgn_line_style_data(&mut reader, "LSSYMBOLCOMPONENT")
            .unwrap_or_else(|| panic!("{dxf_version:?}: not a component"));
        reader
            .check_error()
            .unwrap_or_else(|error| panic!("{dxf_version:?}: {error}"));
        assert_eq!(
            data,
            DgnLineStyleData::Component {
                kind: DgnLsComponentType::Symbol,
                description: NAME.to_string(),
                version: 1,
                component_uid: UID,
                scale: 2.0,
                property_flags: 3,
                component: DgnLsComponentData::Symbol(DgnLsSymbolComponent {
                    stored_unit_scale: 0.5,
                    unit_scale: 4.0,
                    has_unit_scale: true,
                    is_3d: false,
                    block: Handle::from(BLOCK),
                }),
                properties: Vec::new(),
            },
            "{dxf_version:?}"
        );
        assert_eq!(reader.position_in_bits(), end, "{dxf_version:?}");
    }
}
