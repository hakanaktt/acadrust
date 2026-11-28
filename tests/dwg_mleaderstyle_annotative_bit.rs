//! The order of an MLEADERSTYLE's is-annotative (DXF 296) and
//! property-changed (DXF 295) bits in a DWG record.
//!
//! `tests/datatable/point_object_id.dwg` and `.dxf` are one database saved in
//! both formats (see `tests/datatable/README.md`). Its "Standard" multileader
//! style has `295 1` and `296 0` in the DXF, so the DWG read must give the
//! same values: in the DWG record the is-annotative bit comes first.

use std::io::Cursor;

use acadrust::objects::{MultiLeaderStyle, ObjectType};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader};

const DWG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/datatable/point_object_id.dwg"
);
const DXF: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/datatable/point_object_id.dxf"
);

fn style(doc: &CadDocument, name: &str) -> MultiLeaderStyle {
    doc.objects
        .values()
        .find_map(|object| match object {
            ObjectType::MultiLeaderStyle(style) if style.name == name => Some(style.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no multileader style {name:?}"))
}

fn from_dwg(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read")
}

#[test]
fn the_dxf_fixture_has_a_changed_plain_standard_style() {
    let source = std::fs::read(DXF).expect("DXF fixture");
    let doc = DxfReader::from_reader(Cursor::new(source))
        .expect("DXF reader init")
        .read()
        .expect("DXF read");
    let standard = style(&doc, "Standard");
    assert!(!standard.is_annotative, "296 0");
    assert!(standard.property_changed, "295 1");
}

#[test]
fn a_dwg_style_reads_its_annotative_and_changed_bits_as_the_dxf_has_them() {
    let doc = from_dwg(std::fs::read(DWG).expect("DWG fixture"));
    let standard = style(&doc, "Standard");
    assert!(
        !standard.is_annotative,
        "the DXF of the same database has 296 0"
    );
    assert!(
        standard.property_changed,
        "the DXF of the same database has 295 1"
    );
}

#[test]
fn a_dwg_style_keeps_both_bits_through_a_write() {
    for (is_annotative, property_changed) in [(true, false), (false, true)] {
        let mut doc = CadDocument::with_version(DxfVersion::AC1032);
        for object in doc.objects.values_mut() {
            if let ObjectType::MultiLeaderStyle(style) = object {
                style.is_annotative = is_annotative;
                style.property_changed = property_changed;
            }
        }
        let restored = from_dwg(DwgWriter::write_to_vec(&doc).expect("DWG write"));
        let restored = style(&restored, "Standard");
        assert_eq!(restored.is_annotative, is_annotative);
        assert_eq!(restored.property_changed, property_changed);
    }
}
