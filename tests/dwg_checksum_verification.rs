use acadrust::{types::DxfVersion, CadDocument, DwgReadOptions, DwgReader, DwgWriter};
use std::io::Cursor;

fn checksum_mismatches(bytes: &[u8]) -> usize {
    // Failsafe: damage the checksum catches may also break the decode.
    let options = DwgReadOptions {
        failsafe: true,
        ..Default::default()
    };
    let outcome = DwgReader::from_stream_with_options(Cursor::new(bytes), options)
        .read_with_stats()
        .unwrap();
    outcome
        .stats
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "checksum-mismatch")
        .count()
}

fn written(version: DxfVersion) -> Vec<u8> {
    let mut document = CadDocument::new();
    document.version = version;
    DwgWriter::write_to_vec(&document).unwrap()
}

/// A file as written passes its checksums; one flipped bit is reported.
#[test]
fn damaged_bytes_are_reported_as_checksum_mismatch() {
    // AC1015: a byte of the file header, before its CRC.
    let mut bytes = written(DxfVersion::AC1015);
    assert_eq!(checksum_mismatches(&bytes), 0);
    bytes[0x0C] ^= 0x01;
    assert!(checksum_mismatches(&bytes) > 0);

    // AC1018+: a byte of the first page's data, past its 32-byte header.
    for version in [DxfVersion::AC1018, DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut bytes = written(version);
        assert_eq!(checksum_mismatches(&bytes), 0, "{version:?}");
        bytes[0x100 + 32 + 1] ^= 0x01;
        assert!(checksum_mismatches(&bytes) > 0, "{version:?}");
    }
}
