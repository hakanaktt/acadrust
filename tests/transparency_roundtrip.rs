use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::types::{DxfVersion, Handle, Transparency};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn document() -> (CadDocument, Vec<(Handle, Transparency)>) {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut expected = Vec::new();
    for (index, transparency) in [
        Transparency::BY_LAYER,
        Transparency::BY_BLOCK,
        Transparency::OPAQUE,
        Transparency::T_50,
    ]
    .into_iter()
    .enumerate()
    {
        let mut line = Line::from_coords(0.0, index as f64, 0.0, 1.0, index as f64, 0.0);
        line.common.transparency = transparency;
        let handle = document.add_entity(EntityType::Line(line)).unwrap();
        expected.push((handle, transparency));
    }
    (document, expected)
}

fn assert_transparency(document: &CadDocument, expected: &[(Handle, Transparency)]) {
    for (handle, transparency) in expected {
        let entity = document.get_entity(*handle).expect("entity");
        assert_eq!(entity.common().transparency, *transparency);
    }
}

#[test]
fn entity_transparency_methods_survive_dxf_roundtrip() {
    let (document, expected) = document();
    let bytes = DxfWriter::new(&document).write_to_vec().expect("DXF write");
    let roundtripped = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader")
        .read()
        .expect("DXF read");
    assert_transparency(&roundtripped, &expected);
}

#[test]
fn entity_transparency_methods_survive_dwg_roundtrip() {
    let (document, expected) = document();
    let bytes = DwgWriter::write_to_vec(&document).expect("DWG write");
    let roundtripped = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read");
    assert_transparency(&roundtripped, &expected);
}
