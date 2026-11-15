//! USERI1-5 / USERR1-5 header variables: the DXF reader reads them, so a DXF save must
//! write them, or a DXF round trip resets them to 0.
use acadrust::{CadDocument, DxfReader, DxfVersion, DxfWriter};
use std::io::Cursor;

fn with_user_vars(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    let h = &mut doc.header;
    (
        h.user_int1,
        h.user_int2,
        h.user_int3,
        h.user_int4,
        h.user_int5,
    ) = (1, -2, 3, 400, 32767);
    (
        h.user_real1,
        h.user_real2,
        h.user_real3,
        h.user_real4,
        h.user_real5,
    ) = (7.25, -0.5, 1.5e-3, 3844.0, 1.0e10);
    doc
}

#[test]
fn dxf_writes_user_integer_and_real_variables() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let bytes = DxfWriter::new(&with_user_vars(version))
            .write_to_vec()
            .unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        for name in ["$USERI1", "$USERI5", "$USERR1", "$USERR5"] {
            assert!(text.contains(name), "{version:?}: {name} not written");
        }
        let rt = DxfReader::from_reader(Cursor::new(bytes))
            .unwrap()
            .read()
            .unwrap();
        let h = &rt.header;
        assert_eq!(
            (
                h.user_int1,
                h.user_int2,
                h.user_int3,
                h.user_int4,
                h.user_int5
            ),
            (1, -2, 3, 400, 32767),
            "{version:?}"
        );
        assert_eq!(
            (
                h.user_real1,
                h.user_real2,
                h.user_real3,
                h.user_real4,
                h.user_real5
            ),
            (7.25, -0.5, 1.5e-3, 3844.0, 1.0e10),
            "{version:?}"
        );
    }
}
