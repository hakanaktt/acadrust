//! A MLINESTYLE element with linetype ByBlock keeps it through a DWG save
//! (before R2018 the element stores a linetype index, 0x7FFE for ByBlock).

use std::io::Cursor;

use acadrust::objects::{MLineStyle, MLineStyleElement, ObjectType};
use acadrust::tables::LineType;
use acadrust::types::{Color, DxfVersion};
use acadrust::{CadDocument, DwgReader, DwgWriter};

#[test]
fn byblock_element_linetype_survives_dwg() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1021,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut doc = CadDocument::with_version(version);
        let _ = doc.line_types.add(LineType::dashed());
        let mut style = MLineStyle::new("LAB3");
        style.add_element(MLineStyleElement::full(0.5, Color::from_index(1), "Dashed"));
        style.add_element(MLineStyleElement::full(0.0, Color::ByLayer, "ByLayer"));
        style.add_element(MLineStyleElement::full(-0.5, Color::ByBlock, "ByBlock"));
        let handle = doc.allocate_handle();
        style.handle = handle;
        let dict = doc.header.acad_mlinestyle_dict_handle;
        style.owner = dict;
        doc.objects.insert(handle, ObjectType::MLineStyle(style));
        if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dict) {
            d.add_entry("LAB3", handle);
        }

        let dwg = DwgWriter::write_to_vec(&doc).unwrap();
        let back = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
        let Some(ObjectType::MLineStyle(read)) = back.objects.get(&handle) else {
            panic!("{version:?}: MLINESTYLE lost");
        };
        let linetypes: Vec<String> = read
            .elements
            .iter()
            .map(|e| e.linetype.to_ascii_uppercase())
            .collect();
        assert_eq!(linetypes, ["DASHED", "BYLAYER", "BYBLOCK"], "{version:?}");
    }
}
