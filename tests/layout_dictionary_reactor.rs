//! A layout made by `add_layout` names the layout dictionary that owns it
//! as its reactor, in memory, in DXF and in DWG. Without it, deleting the
//! layout in another program leaves the dictionary's entry pointing at an
//! erased object.

use acadrust::objects::ObjectType;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter, Handle};
use std::io::Cursor;

fn reactors(doc: &CadDocument, name: &str) -> (Handle, Vec<Handle>) {
    let layout = doc
        .objects
        .values()
        .find_map(|o| match o {
            ObjectType::Layout(l) if l.name == name => Some(l),
            _ => None,
        })
        .expect("the layout");
    (layout.owner, layout.reactors.clone())
}

fn document() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    doc.add_layout("NEW SHEET").unwrap();
    doc
}

#[test]
fn a_new_layout_reacts_to_its_dictionary() {
    let doc = document();
    let dictionary = doc.header.acad_layout_dict_handle;
    assert_eq!(reactors(&doc, "NEW SHEET"), (dictionary, vec![dictionary]));
    // The layouts of a new drawing as well.
    assert_eq!(reactors(&doc, "Model"), (dictionary, vec![dictionary]));
    assert_eq!(reactors(&doc, "Layout1"), (dictionary, vec![dictionary]));
}

#[test]
fn the_reactor_survives_dxf_and_dwg() {
    let doc = document();
    let dictionary = doc.header.acad_layout_dict_handle;

    let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
    let back = DxfReader::from_reader(Cursor::new(dxf)).unwrap().read().unwrap();
    assert_eq!(reactors(&back, "NEW SHEET").1, vec![dictionary]);

    let dwg = DwgWriter::write_to_vec(&doc).unwrap();
    let back = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    assert_eq!(reactors(&back, "NEW SHEET").1, vec![dictionary]);
}
