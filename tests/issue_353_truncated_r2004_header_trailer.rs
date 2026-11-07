//! An AC1018 file whose header section ends before the R2004 trailer still reads.
//!
//! `tests/r2004_short_header_trailer.dwg` was written by LibreDWG's `dwgwrite`. Its header section is
//! 424 bytes and stops three bits after the four trailing BitShorts, so the two BitLongs and the
//! bit that AutoCAD writes after them are missing. The reader failed the whole file with
//! "Truncated DWG stream at bit 3389".
use acadrust::{CadDocument, DwgReader, DxfVersion, EntityType};
use std::io::Cursor;

fn read() -> Result<CadDocument, acadrust::DxfError> {
    let bytes = include_bytes!("r2004_short_header_trailer.dwg").to_vec();
    DwgReader::from_stream(Cursor::new(bytes)).read()
}

#[test]
fn the_file_reads() {
    let result = read();
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn the_header_before_the_trailer_is_kept() {
    let doc = read().expect("the file reads");
    assert_eq!(doc.version, DxfVersion::AC1018);
}

#[test]
fn the_entities_after_the_header_are_read() {
    let doc = read().expect("the file reads");
    assert_eq!(
        doc.entities()
            .filter(|e| matches!(e, EntityType::Insert(_)))
            .count(),
        1
    );
}
