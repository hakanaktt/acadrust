use std::io::Cursor;

use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion};

#[test]
fn current_lineweight_roundtrips_as_a_table_index() {
    const VERSIONS: [DxfVersion; 4] = [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1032,
    ];
    const WEIGHTS: [i16; 27] = [
        -3, -2, -1, 0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120,
        140, 158, 200, 211,
    ];

    for version in VERSIONS {
        for weight in WEIGHTS {
            let mut document = CadDocument::with_version(version);
            document.header.current_line_weight = weight;
            document.header.end_caps = 2;
            document.header.join_style = 1;

            let bytes = DwgWriter::write_to_vec(&document).unwrap();
            let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();

            assert_eq!(read.header.current_line_weight, weight, "{version:?}");
            assert_eq!(read.header.end_caps, 2, "{version:?}, weight {weight}");
            assert_eq!(read.header.join_style, 1, "{version:?}, weight {weight}");
        }
    }
}
