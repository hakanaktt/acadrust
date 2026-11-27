//! A dynamic block's visibility parameter (BLOCKVISIBILITYPARAMETER) keeps
//! its reactors through a DXF.
//!
//! The parameter's own type has no reactor list; the document keeps its
//! reactors beside it (`object_reactors`), where the DWG reader puts them.
//! The DXF writer wrote the parameter's record without an `{ACAD_REACTORS`
//! group and the DXF reader did not keep the group it read.
//! An object that reacts to the parameter (a block properties table that sets
//! the visibility state, or the dictionary that owns it) is no longer told of
//! its changes after the drawing goes through a DXF.

use acadrust::{
    objects::{BlockVisibilityParameter, Dictionary, ObjectType},
    types::DxfVersion,
    CadDocument, DxfReader, DxfWriter, Handle,
};
use std::io::Cursor;

/// A drawing with a visibility parameter in a dictionary of its own under
/// the named object dictionary; the dictionary reacts to the parameter.
/// Returns the drawing, the parameter and the dictionary.
fn drawing(version: DxfVersion) -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::with_version(version);
    let root = doc.header.named_objects_dict_handle;
    let holder = doc.allocate_handle();
    let parameter = doc.allocate_handle();

    let mut dictionary = Dictionary::new();
    dictionary.handle = holder;
    dictionary.owner = root;
    dictionary.entries.push(("Visibility".into(), parameter));
    doc.objects
        .insert(holder, ObjectType::Dictionary(dictionary));
    if let Some(ObjectType::Dictionary(named)) = doc.objects.get_mut(&root) {
        named.add_entry("VISIBILITY_HOLDER", holder);
    }

    let value = BlockVisibilityParameter {
        handle: parameter,
        owner: holder,
        name: "Visibility".into(),
        ..Default::default()
    };
    doc.block_visibility_params.insert(parameter, value.clone());
    doc.objects
        .insert(parameter, ObjectType::BlockVisibilityParameter(value));
    doc.add_object_reactor(parameter, holder);
    (doc, parameter, holder)
}

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn reactors(doc: &CadDocument, handle: Handle) -> Vec<Handle> {
    assert!(
        matches!(
            doc.objects.get(&handle),
            Some(ObjectType::BlockVisibilityParameter(_))
        ),
        "the parameter is read back"
    );
    doc.object_reactors(handle).to_vec()
}

/// The reactors listed in the BLOCKVISIBILITYPARAMETER record of a DXF.
fn dxf_record_reactors(dxf: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(dxf);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let start = lines
        .windows(2)
        .position(|pair| pair == ["0", "BLOCKVISIBILITYPARAMETER"])
        .expect("the parameter's record is in the DXF");
    let record: Vec<(&str, &str)> = lines[start + 2..]
        .chunks(2)
        .map(|pair| (pair[0], pair.get(1).copied().unwrap_or_default()))
        .take_while(|(code, _)| *code != "0")
        .collect();
    let Some(from) = record
        .iter()
        .position(|pair| *pair == ("102", "{ACAD_REACTORS"))
    else {
        return Vec::new();
    };
    record[from + 1..]
        .iter()
        .take_while(|(code, _)| *code == "330")
        .map(|(_, value)| value.to_ascii_uppercase())
        .collect()
}

#[test]
fn the_dxf_writer_writes_the_parameters_reactors() {
    let (doc, _, holder) = drawing(DxfVersion::AC1032);
    let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
    assert_eq!(dxf_record_reactors(&dxf), [format!("{:X}", holder.value())]);
}

#[test]
fn a_dxf_round_trip_keeps_the_parameters_reactors() {
    let (doc, parameter, holder) = drawing(DxfVersion::AC1032);
    let back = read_dxf(DxfWriter::new(&doc).write_to_vec().unwrap());
    assert_eq!(reactors(&back, parameter), [holder]);
}
