use std::io::Cursor;

use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion};

#[test]
fn measurement_roundtrips_independently_of_insertion_units() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1032,
    ] {
        for measurement in [0, 1] {
            let mut document = CadDocument::with_version(version);
            document.header.insertion_units = 0;
            document.header.measurement = measurement;

            let bytes = DwgWriter::write_to_vec(&document).unwrap();
            let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

            assert_eq!(read.header.measurement, measurement, "{version:?}");
            assert_eq!(read.header.insertion_units, 0, "{version:?}");
        }
    }
}
